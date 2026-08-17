use std::error::Error as StdError;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::error::AppError;
use crate::lanzou::core::share::file_download_url;

/// 下载任务信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    /// 前端任务 id（用于进度事件匹配）
    pub id: String,
    /// 分享链接
    pub url: String,
    /// 提取码
    pub pwd: Option<String>,
    /// 保存目录
    pub dir: String,
    /// 解析后的文件名（可选，分享解析后填充）
    pub name: Option<String>,
}

/// 下载进度事件载荷
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub id: String,
    pub name: String,
    /// 已下载字节
    pub downloaded: u64,
    /// 总字节（未知时为 0）
    pub total: u64,
    /// 速度 bytes/s
    pub speed: u64,
    /// 保存路径（下载完成时）
    pub file_path: Option<String>,
}

/// 解析分享链接 + 下载文件到本地，带进度事件
#[tauri::command]
pub async fn lanzou_download(
    app: AppHandle,
    state: tauri::State<'_, crate::state::AppState>,
    task: DownloadTask,
) -> Result<(), AppError> {
    // clone client 释放锁，避免下载期间阻塞其它命令
    let client = {
        let guard = state.client.lock().await;
        guard.clone()
    };
    download_url(&app, &state, &client, &task).await
}

/// 按 file_id/folder_id 直接下载（内部解析分享链接）
#[tauri::command]
pub async fn lanzou_download_by_id(
    app: AppHandle,
    state: tauri::State<'_, crate::state::AppState>,
    task_id: String,
    id: String,
    is_folder: bool,
    dir: Option<String>,
    name: Option<String>,
) -> Result<(), AppError> {
    // clone client 释放锁，避免下载期间阻塞其它命令
    let client = {
        let guard = state.client.lock().await;
        guard.clone()
    };
    // 解析分享链接
    let detail = if is_folder {
        crate::lanzou::core::ops::folder_detail(&client, &id).await?
    } else {
        crate::lanzou::core::ops::file_detail(&client, &id).await?
    };
    let url = detail
        .url
        .ok_or_else(|| AppError::Lanzou("未获取到分享链接".into()))?;
    let task = DownloadTask {
        id: task_id,
        url,
        pwd: detail.pwd,
        dir: dir.unwrap_or_default(),
        name: name.or(detail.name),
    };
    download_url(&app, &state, &client, &task).await
}

/// 实际下载流程（dir 为空时使用系统下载目录）
/// 外层重试：响应体读取/解码偶发失败（如 CDN 中断、压缩解码失败）时，
/// 基于已写入的 .download 临时文件断点续传重试，避免一次抖动就整个失败
pub async fn download_url(
    app: &AppHandle,
    state: &crate::state::AppState,
    client: &crate::lanzou::client::LanzouClient,
    task: &DownloadTask,
) -> Result<(), AppError> {
    for attempt in 0..3 {
        match download_url_attempt(app, state, client, task).await {
            Ok(()) => return Ok(()),
            Err(AppError::Http(e)) if attempt < 2 && is_transient(&e) => {
                log::warn!(
                    "download_url[{}]: 尝试 {}/2 失败: {e}，稍后续传重试",
                    task.id,
                    attempt + 1
                );
                tokio::time::sleep(std::time::Duration::from_millis(500 * (attempt + 1))).await;
            }
            Err(e) => {
                // 清理取消标志（成功/"已取消"路径由内层清理，其余由这里兜底）
                state.finish_cancel(&task.id).await;
                return Err(e);
            }
        }
    }
    Ok(())
}

/// 判断错误是否为可重试的瞬时网络/解码错误
fn is_transient(e: &reqwest::Error) -> bool {
    if e.is_timeout() || e.is_connect() || e.is_body() || e.is_decode() {
        return true;
    }
    // 连接被服务端中断 / 拥塞等
    matches!(e.source(), Some(src) if is_io_reset(src))
}

fn is_io_reset(src: &dyn StdError) -> bool {
    let s = src.to_string();
    s.contains("reset") || s.contains("Connection reset") || s.contains("broken pipe")
}

/// 单次下载尝试（支持断点续传 Range）
async fn download_url_attempt(
    app: &AppHandle,
    state: &crate::state::AppState,
    client: &crate::lanzou::client::LanzouClient,
    task: &DownloadTask,
) -> Result<(), AppError> {
    // 注册取消标志
    let cancel_flag = state.register_cancel(&task.id).await;
    let cancelled = || cancel_flag.load(std::sync::atomic::Ordering::SeqCst);

    // 解析分享类型与直链（文件场景一次完成，避免重复请求）
    let download = file_download_url(client, &task.url, task.pwd.as_deref()).await?;
    if cancelled() {
        state.finish_cancel(&task.id).await;
        return Err(AppError::Lanzou("已取消".into()));
    }
    let name = task.name.clone().unwrap_or_else(|| download.name.clone());
    // 文件名兜底：为空时生成唯一名
    let name = if name.trim().is_empty() {
        format!("download_{}", chrono::Utc::now().timestamp())
    } else {
        name
    };

    // 保存路径
    let dir = if task.dir.is_empty() {
        dirs::download_dir().unwrap_or_else(std::env::temp_dir)
    } else {
        PathBuf::from(&task.dir)
    };
    std::fs::create_dir_all(&dir)?;
    let save_path = dir.join(sanitize(&name));
    // 断点续传：使用 .download 临时文件，恢复时追加写入
    let tmp_path = dir.join(format!("{}.download", sanitize(&name)));
    let save_path_str = save_path.to_string_lossy().into_owned();
    let tmp_path_str = tmp_path.to_string_lossy().into_owned();
    log::info!("download_url[{}]: save to {}", task.id, save_path_str);

    // 已下载的字节数（断点续传起点）
    let mut resume_from = std::fs::metadata(&tmp_path).map(|m| m.len()).unwrap_or(0);

    // 流式下载（支持断点续传 Range）
    let url = download.url.clone();
    let mut builder = client.share_request_builder(&url, Some(&task.url))?;
    // 关闭内容压缩，避免 Range 续传与 gzip 解码冲突导致 "error decoding response body"
    builder = builder.header(reqwest::header::ACCEPT_ENCODING, "identity");
    if resume_from > 0 {
        builder = builder.header(reqwest::header::RANGE, format!("bytes={resume_from}-"));
    }
    let response = builder.send().await?;
    if !response.status().is_success() && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
    {
        state.finish_cancel(&task.id).await;
        return Err(AppError::Lanzou(format!(
            "下载失败: HTTP {}",
            response.status()
        )));
    }
    let total = if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        // 206：从 Content-Range 解析完整大小，避免把剩余字节数当成 total
        let partial_len = response.content_length().unwrap_or(0);
        response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.rsplit('/').next())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(resume_from + partial_len)
    } else {
        // 服务器忽略 Range 返回 200：从头下载并截断旧临时文件
        if resume_from > 0 {
            resume_from = 0;
            std::fs::File::create(&tmp_path)?;
        }
        response.content_length().unwrap_or(0)
    };
    log::info!(
        "download_url[{}]: total={} resume_from={}",
        task.id,
        total,
        resume_from
    );

    let mut file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&tmp_path)
    {
        Ok(f) => f,
        Err(e) => {
            log::error!(
                "download_url[{}]: open tmp {} error: {e}",
                task.id,
                tmp_path_str
            );
            return Err(AppError::Io(e));
        }
    };
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = resume_from;
    let mut last_emit = std::time::Instant::now();
    let mut speed_win = (resume_from, std::time::Instant::now());
    let mut cancelled_midway = false;

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        if cancelled() {
            cancelled_midway = true;
            break;
        }
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                log::error!("download_url[{}]: stream error: {e}", task.id);
                return Err(AppError::Http(e));
            }
        };
        if let Err(e) = std::io::Write::write_all(&mut file, &chunk) {
            log::error!("download_url[{}]: write error: {e}", task.id);
            return Err(AppError::Io(e));
        }
        downloaded += chunk.len() as u64;

        // 进度事件：每 200ms
        let now = std::time::Instant::now();
        if now.duration_since(last_emit) >= std::time::Duration::from_millis(200) {
            let dt = now.duration_since(speed_win.1).as_secs_f64();
            let speed = if dt > 0.0 {
                ((downloaded - speed_win.0) as f64 / dt) as u64
            } else {
                0
            };
            speed_win = (downloaded, now);
            log::info!(
                "download_url[{}]: emit downloaded={} total={} speed={}",
                task.id,
                downloaded,
                total,
                speed
            );
            let _ = app.emit(
                "download:progress",
                DownloadProgress {
                    id: task.id.clone(),
                    name: name.clone(),
                    downloaded,
                    total,
                    speed,
                    file_path: None,
                },
            );
            last_emit = now;
        }
    }

    // 取消：保留 .download 临时文件以便断点续传
    if cancelled_midway || cancelled() {
        state.finish_cancel(&task.id).await;
        return Err(AppError::Lanzou("已取消".into()));
    }

    // 完成：临时文件重命名为正式文件名（若已存在则自动加序号）
    let final_path = unique_path(&dir, &save_path);
    std::fs::rename(&tmp_path, &final_path)?;
    let final_path_str = final_path.to_string_lossy().into_owned();

    let _ = app.emit(
        "download:progress",
        DownloadProgress {
            id: task.id.clone(),
            name,
            downloaded,
            total,
            speed: 0,
            file_path: Some(final_path_str),
        },
    );
    log::info!(
        "download_url[{}]: 完成 downloaded={} total={}",
        task.id,
        downloaded,
        total
    );
    state.finish_cancel(&task.id).await;
    Ok(())
}

fn sanitize(name: &str) -> String {
    sanitize_filename::sanitize(name)
}

/// 若目标文件已存在，生成带序号的新路径（如 name (1).ext）
pub fn unique_path(dir: &std::path::Path, target: &std::path::Path) -> std::path::PathBuf {
    if !target.exists() {
        return target.to_path_buf();
    }
    let file_stem = target
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = target
        .extension()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut n = 1;
    loop {
        let candidate = if ext.is_empty() {
            dir.join(format!("{file_stem} ({n})"))
        } else {
            dir.join(format!("{file_stem} ({n}).{ext}"))
        };
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

/// 取消指定下载任务
#[tauri::command]
pub async fn lanzou_cancel_transfer(
    state: tauri::State<'_, crate::state::AppState>,
    task_id: String,
) -> Result<(), AppError> {
    state.cancel_task(&task_id).await;
    Ok(())
}
