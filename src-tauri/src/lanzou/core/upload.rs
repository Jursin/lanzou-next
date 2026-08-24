use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::error::AppError;
use crate::lanzou::client::LanzouClient;
use crate::lanzou::core::ops::mkdir;

/// 上传任务
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadTask {
    pub id: String,
    /// 本地文件/文件夹路径
    pub path: String,
    /// 目标文件夹 id
    pub folder_id: i64,
    /// 上传文件名（可自定义）
    pub name: Option<String>,
    /// 超出大小限制的文件是否分片上传（false/None 时超限文件被跳过/报错）
    pub chunk_oversized: Option<bool>,
}

/// 上传进度事件
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadProgress {
    pub id: String,
    pub name: String,
    pub uploaded: u64,
    pub total: u64,
    pub speed: u64,
}

/// 分片上传创建的云端子文件夹事件（用于删除未完成任务时清理云端文件夹）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkFolderEvent {
    pub task_id: String,
    pub folder_id: i64,
}

/// 上传单个文件或整个文件夹到指定文件夹（文件夹自动创建同名目录并递归上传）
#[tauri::command]
pub async fn lanzou_upload(
    app: AppHandle,
    state: tauri::State<'_, crate::state::AppState>,
    task: UploadTask,
) -> Result<(), AppError> {
    // clone client 释放锁，避免上传期间阻塞其它命令
    let client = {
        let guard = state.client.lock().await;
        guard.clone()
    };
    // 注册取消标志，支持上传中途暂停
    let cancel_flag = state.register_cancel(&task.id).await;
    let cancelled = || cancel_flag.load(Ordering::SeqCst);

    let path = PathBuf::from(&task.path);
    if !path.exists() {
        state.finish_cancel(&task.id).await;
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("文件或文件夹不存在: {}", task.path),
        )));
    }
    let total = if path.is_dir() {
        total_dir_size(&path)
    } else {
        std::fs::metadata(&path)?.len()
    };
    let name = task.name.clone().unwrap_or_else(|| {
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    });

    log::info!("lanzou_upload: 开始上传 \"{name}\" ({} bytes) -> folder_id={}", total, task.folder_id);

    // 累计进度计数器（整个任务共享，文件/文件夹统一）
    let progress = Arc::new(AtomicU64::new(0));
    emit_progress(&app, &task.id, &name, 0, total);
    // 后台定时上报进度（每 200ms），同时驱动前端刷新耗时
    let reporter = spawn_progress_reporter(
        app.clone(),
        task.id.clone(),
        name.clone(),
        progress.clone(),
        total,
        cancel_flag.clone(),
    );

    // 账号单文件大小限制（字节）+ 分片大小（字节）：获取失败时回退为不限制
    let account_max = crate::lanzou::core::profile::account_max_size(&client)
        .await
        .ok()
        .flatten();
    let split_mb = crate::commands::config::config_get(app.clone())?
        .split_size
        .unwrap_or(100) as u64;
    let split_bytes = split_mb
        .saturating_mul(1024 * 1024)
        .min(account_max.unwrap_or(u64::MAX));
    let chunk_oversized = task.chunk_oversized.unwrap_or(false);

    let result = if path.is_dir() {
        upload_dir(
            &app,
            &task.id,
            &client,
            &name,
            &path,
            task.folder_id,
            account_max,
            split_bytes,
            chunk_oversized,
            progress.clone(),
            cancel_flag.clone(),
        )
        .await
        .map(|_| ())
    } else {
        let size = std::fs::metadata(&path)?.len();
        let oversized = account_max.is_some_and(|m| size > m);
        if oversized && !chunk_oversized {
            Err(AppError::Lanzou(format!(
                "文件大小超出账号限制（{}），已跳过",
                format_size(account_max.unwrap_or(0))
            )))
        } else if oversized {
            upload_chunked(
                &app,
                &task.id,
                &client,
                &path,
                &name,
                task.folder_id,
                split_bytes,
                progress.clone(),
                cancel_flag.clone(),
            )
            .await
            .map(|_| ())
        } else {
            upload_file_stream(
                &client,
                &task.path,
                task.name.as_deref(),
                task.folder_id,
                progress.clone(),
                cancel_flag.clone(),
            )
            .await
            .map(|_| ())
        }
    };

    reporter.stop().await;
    state.finish_cancel(&task.id).await;
    if cancelled() {
        log::info!("lanzou_upload: \"{name}\" 已取消");
        return Err(AppError::Lanzou("已取消".into()));
    }
    result?;
    log::info!("lanzou_upload: \"{name}\" 上传完成");
    emit_progress(&app, &task.id, &name, total, total);
    Ok(())
}

fn emit_progress(app: &AppHandle, id: &str, name: &str, uploaded: u64, total: u64) {
    let _ = app.emit(
        "upload:progress",
        UploadProgress {
            id: id.into(),
            name: name.into(),
            uploaded,
            total,
            speed: 0,
        },
    );
}

/// 后台进度上报循环，stop 后立即退出
struct ProgressReporter {
    handle: tokio::task::JoinHandle<()>,
    done: Arc<AtomicBool>,
}

fn spawn_progress_reporter(
    app: AppHandle,
    task_id: String,
    name: String,
    progress: Arc<AtomicU64>,
    total: u64,
    cancel: Arc<AtomicBool>,
) -> ProgressReporter {
    let done = Arc::new(AtomicBool::new(false));
    let done_loop = done.clone();
    let handle = tokio::spawn(async move {
        // 保留最近 1s 的采样点，计算平滑速度，避免字节读取突发造成速度抖动
        let mut samples: std::collections::VecDeque<(Instant, u64)> =
            std::collections::VecDeque::new();
        while !done_loop.load(Ordering::SeqCst) {
            if cancel.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
            let cur = progress.load(Ordering::SeqCst);
            let now = Instant::now();
            samples.push_back((now, cur));
            while samples.len() > 1 {
                if now.duration_since(samples.front().unwrap_or(&(now, cur)).0)
                    <= Duration::from_secs(1)
                {
                    break;
                }
                samples.pop_front();
            }
            let speed = if let Some(&(t0, b0)) = samples.front() {
                let dt = now.duration_since(t0).as_secs_f64();
                if dt > 0.0 {
                    ((cur - b0) as f64 / dt) as u64
                } else {
                    0
                }
            } else {
                0
            };
            let _ = app.emit(
                "upload:progress",
                UploadProgress {
                    id: task_id.clone(),
                    name: name.clone(),
                    uploaded: cur,
                    total,
                    speed,
                },
            );
        }
    });
    ProgressReporter { handle, done }
}

impl ProgressReporter {
    async fn stop(self) {
        self.done.store(true, Ordering::SeqCst);
        self.handle.abort();
        let _ = self.handle.await;
    }
}

/// 递归统计目录内文件总大小
fn total_dir_size(dir: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += total_dir_size(&p);
            } else if let Ok(meta) = std::fs::metadata(&p) {
                total += meta.len();
            }
        }
    }
    total
}

/// 递归上传目录：先创建远程文件夹，再逐个上传文件/子目录，进度累计到共享计数器
/// 超限文件按 chunk_oversized 决定分片上传或跳过
#[allow(clippy::too_many_arguments)]
async fn upload_dir(
    app: &AppHandle,
    task_id: &str,
    client: &LanzouClient,
    remote_name: &str,
    dir: &Path,
    parent_id: i64,
    account_max: Option<u64>,
    split_bytes: u64,
    chunk_oversized: bool,
    progress: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
) -> Result<(), AppError> {
    if cancel.load(Ordering::SeqCst) {
        return Err(AppError::Lanzou("已取消".into()));
    }
    let folder_id = mkdir(client, parent_id, remote_name, None).await?;
    let folder_id: i64 = folder_id
        .parse()
        .map_err(|_| AppError::Lanzou(format!("创建文件夹失败: {remote_name}")))?;

    for entry in std::fs::read_dir(dir)? {
        if cancel.load(Ordering::SeqCst) {
            return Err(AppError::Lanzou("已取消".into()));
        }
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            let sub_name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            Box::pin(upload_dir(
                app,
                task_id,
                client,
                &sub_name,
                &p,
                folder_id,
                account_max,
                split_bytes,
                chunk_oversized,
                progress.clone(),
                cancel.clone(),
            ))
            .await?;
        } else {
            let size = std::fs::metadata(&p)?.len();
            let oversized = account_max.is_some_and(|m| size > m);
            if oversized && !chunk_oversized {
                continue;
            }
            if oversized {
                let file_name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                upload_chunked(
                    app,
                    task_id,
                    client,
                    &p,
                    &file_name,
                    folder_id,
                    split_bytes,
                    progress.clone(),
                    cancel.clone(),
                )
                .await?;
            } else {
                upload_file_stream(
                    client,
                    p.to_str().unwrap_or_default(),
                    None,
                    folder_id,
                    progress.clone(),
                    cancel.clone(),
                )
                .await
                .map(|_| ())?;
            }
        }
    }
    Ok(())
}

/// 流式上传单个文件（整文件），读取时累计进度，返回上传后的文件名
pub async fn upload_file_stream(
    client: &LanzouClient,
    path_str: &str,
    custom_name: Option<&str>,
    folder_id: i64,
    progress: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
) -> Result<String, AppError> {
    if cancel.load(Ordering::SeqCst) {
        return Err(AppError::Lanzou("已取消".into()));
    }
    let path = PathBuf::from(path_str);
    if !path.is_file() {
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("文件不存在: {path_str}"),
        )));
    }
    let total = std::fs::metadata(&path)?.len();
    let name = custom_name.map(String::from).unwrap_or_else(|| {
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    upload_file_range(client, &path, &name, folder_id, 0, total, progress, cancel).await
}

/// 上传文件指定字节区间（分片上传用），name 为云端文件名
#[allow(clippy::too_many_arguments)]
async fn upload_file_range(
    client: &LanzouClient,
    path: &Path,
    name: &str,
    folder_id: i64,
    start: u64,
    len: u64,
    progress: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
) -> Result<String, AppError> {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    let mut file = tokio::fs::File::open(path).await?;
    if start > 0 {
        file.seek(std::io::SeekFrom::Start(start)).await?;
    }
    let progress_reader = progress.clone();
    let stream = futures_util::stream::unfold(Some((file, len)), move |state| {
        let progress = progress_reader.clone();
        async move {
            let (mut f, mut remaining) = match state {
                Some(s) => s,
                None => return None,
            };
            if remaining == 0 {
                return None;
            }
            let mut buf = vec![0u8; (64 * 1024).min(remaining as usize)];
            match f.read(&mut buf).await {
                Ok(0) => None,
                Ok(n) => {
                    progress.fetch_add(n as u64, Ordering::SeqCst);
                    remaining -= n as u64;
                    buf.truncate(n);
                    Some((Ok::<Vec<u8>, std::io::Error>(buf), Some((f, remaining))))
                }
                Err(e) => Some((Err(e), None)),
            }
        }
    });

    let file_name_for_form = name.to_string();
    let form = reqwest::multipart::Form::new()
        .text("task", "1")
        .text("vie", "2")
        .text("ve", "2")
        .text("id", "WU_FILE_0")
        .text("folder_id_bb_n", folder_id.to_string())
        .text("size", len.to_string())
        .text("name", name.to_string())
        .part(
            "upload_file",
            reqwest::multipart::Part::stream(reqwest::Body::wrap_stream(stream))
                .file_name(file_name_for_form)
                .mime_str("application/octet-stream")?,
        );

    let url = client.base_url().join("html5up.php")?;
    let referer = client.base_url().as_str().trim_end_matches('/').to_string();
    // 取消支持：取消时中止上传请求
    let response = tokio::select! {
        res = client.multipart_post(url.path(), Some(&referer), form) => res?,
        _ = cancel_watcher(cancel) => {
            return Err(AppError::Lanzou("已取消".into()));
        }
    };
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(AppError::Lanzou(format!(
            "HTTP {status}: {}",
            truncate(&body)
        )));
    }
    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| AppError::Lanzou(format!("非标准 JSON: {e} ({})", truncate(&body))))?;
    let zt = value["zt"].as_i64().unwrap_or(0);
    if zt != 1 {
        let msg = value["info"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| "上传失败".into());
        return Err(AppError::Lanzou(msg));
    }

    Ok(name.to_string())
}

/// 分片上传：在目标文件夹创建同名子文件夹，按字节区间切分后逐片上传
/// 分片命名：`文件名.序号.伪后缀`（如 movie.mp4.001.ct.ke），序号补零到总片数位数
#[allow(clippy::too_many_arguments)]
pub(crate) async fn upload_chunked(
    app: &AppHandle,
    task_id: &str,
    client: &LanzouClient,
    path: &Path,
    file_name: &str,
    parent_id: i64,
    split_bytes: u64,
    progress: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
) -> Result<(), AppError> {
    if cancel.load(Ordering::SeqCst) {
        return Err(AppError::Lanzou("已取消".into()));
    }
    let size = std::fs::metadata(path)?.len();
    let count = size.div_ceil(split_bytes);
    let width = count.to_string().len();
    // 子文件夹名与原始文件名一致，存放所有分片
    let folder_id = mkdir(client, parent_id, file_name, None).await?;
    let folder_id: i64 = folder_id
        .parse()
        .map_err(|_| AppError::Lanzou(format!("创建文件夹失败: {file_name}")))?;
    // 上报子文件夹 id，供前端在删除未完成任务时清理云端文件夹
    let _ = app.emit(
        "upload:chunk-folder",
        ChunkFolderEvent {
            task_id: task_id.to_string(),
            folder_id,
        },
    );

    for i in 0..count {
        if cancel.load(Ordering::SeqCst) {
            return Err(AppError::Lanzou("已取消".into()));
        }
        let start = i * split_bytes;
        let len = split_bytes.min(size - start);
        let part_name = format!("{}.{:0width$}.{}", file_name, i + 1, part_suffix());
        upload_file_range(
            client,
            path,
            &part_name,
            folder_id,
            start,
            len,
            progress.clone(),
            cancel.clone(),
        )
        .await?;
    }
    Ok(())
}

/// 生成伪后缀：从安全后缀列表随机取两个拼成（如 "ct.ke"），规避蓝奏云类型限制
fn part_suffix() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let i1 = rng.gen_range(0..SAFE_SUFFIX_LIST.len());
    let i2 = rng.gen_range(0..SAFE_SUFFIX_LIST.len());
    format!("{}.{}", SAFE_SUFFIX_LIST[i1], SAFE_SUFFIX_LIST[i2])
}

/// 蓝奏云允许上传的安全伪后缀（来自参考项目配置）
const SAFE_SUFFIX_LIST: &[&str] = &[
    "ct",
    "ke",
    "w3x",
    "mobi",
    "azw",
    "azw3",
    "osk",
    "osz",
    "xpa",
    "cpk",
    "lua",
    "gho",
    "ttc",
    "txf",
    "bat",
    "imazingapp",
    "xapk",
    "conf",
    "rp",
    "rplib",
    "mobileconfig",
    "appimage",
    "lolgezi",
    "cad",
    "hwt",
    "ce",
    "xmind",
    "bds",
    "bdi",
    "ssf",
    "it",
    "pkg",
    "cfg",
];

fn truncate(s: &str) -> String {
    if s.len() > 200 {
        format!("{}...", &s[..200])
    } else {
        s.to_string()
    }
}

/// 超限文件信息（上传预检结果项）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OversizedFile {
    /// 本地路径
    pub path: String,
    /// 文件名
    pub name: String,
    /// 相对于所选根目录的路径
    pub rel_path: String,
    /// 文件大小（字节）
    pub size: u64,
}

/// 上传预检结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecheckResult {
    /// 账号单文件大小限制（字节），无法获取时为 None（视为不限制）
    pub max_size: Option<u64>,
    /// 超出限制的文件列表
    pub oversized: Vec<OversizedFile>,
}

/// 扫描所选路径（文件/文件夹），返回超出账号限制的文件清单
pub fn scan_oversized(root: &Path, account_max: Option<u64>) -> Vec<OversizedFile> {
    let Some(max) = account_max else {
        return Vec::new();
    };
    if root.is_dir() {
        let mut out = Vec::new();
        scan_oversized_dir(root, root, max, &mut out);
        out
    } else {
        let size = std::fs::metadata(root).map(|m| m.len()).unwrap_or(0);
        if size > max {
            let name = root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            vec![OversizedFile {
                path: root.to_string_lossy().into_owned(),
                name: name.clone(),
                rel_path: name,
                size,
            }]
        } else {
            Vec::new()
        }
    }
}

fn scan_oversized_dir(root: &Path, dir: &Path, max: u64, out: &mut Vec<OversizedFile>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            scan_oversized_dir(root, &p, max, out);
            continue;
        }
        let Ok(meta) = std::fs::metadata(&p) else {
            continue;
        };
        if meta.len() <= max {
            continue;
        }
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let rel_path = p
            .strip_prefix(root)
            .map(|r| r.to_string_lossy().into_owned())
            .unwrap_or_else(|_| name.clone());
        out.push(OversizedFile {
            path: p.to_string_lossy().into_owned(),
            name,
            rel_path,
            size: meta.len(),
        });
    }
}

/// 字节数格式化为可读大小（K/M/G/T）
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{v:.0} {}", UNITS[i])
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

/// 一直等待直到取消标志被置位（配合 tokio::select! 中止请求）
async fn cancel_watcher(flag: Arc<AtomicBool>) {
    while !flag.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
