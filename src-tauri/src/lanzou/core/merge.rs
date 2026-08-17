use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::error::AppError;
use crate::lanzou::client::LanzouClient;
use crate::lanzou::core::ops::file_detail;
use crate::lanzou::core::share::file_download_url;

/// 合并下载任务（多选分片文件后本地合并）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeDownloadTask {
    /// 前端任务 id（用于进度事件匹配）
    pub id: String,
    /// 分片文件（云端 file_id + 文件名）
    pub files: Vec<MergePart>,
    /// 保存目录
    pub dir: String,
    /// 合并后是否保留分片文件（false 时合并成功后删除）
    pub keep_parts: bool,
}

/// 单个分片文件（云端 file_id + 文件名）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergePart {
    pub id: String,
    pub name: String,
}

/// 从分片文件名解析序号（如 movie.mp4.001.ct.ke -> 1）
pub fn parse_part_index(name: &str) -> Option<u64> {
    let re = regex::Regex::new(r"\.(\d+)\.\w+\.\w+$").ok()?;
    re.captures(name)?.get(1)?.as_str().parse().ok()
}

/// 计算合并文件名：取共同前缀，去掉序号补零部分与尾部 `_ - .` 分隔符
pub fn merged_name(names: &[String]) -> String {
    let Some(first) = names.first() else {
        return String::new();
    };
    let prefix = names.iter().skip(1).fold(first.clone(), |acc, n| {
        let mut end = 0;
        let mut bytes = acc.bytes().zip(n.bytes());
        for (a, b) in bytes.by_ref() {
            if a != b {
                break;
            }
            end += 1;
        }
        acc[..end].to_string()
    });
    let mut result = prefix.as_str();
    for _ in 0..4 {
        let next = result
            .trim_end_matches('0')
            .trim_end_matches(['_', '-', '.', ' ', '\t']);
        if next.len() == result.len() {
            break;
        }
        result = next;
    }
    result.trim().to_string()
}

/// 合并下载：逐片下载到本地临时目录 -> 按序号合并 -> 按 keep_parts 决定是否删除分片，返回合并后的文件路径
pub async fn merge_download(
    app: &AppHandle,
    client: &LanzouClient,
    task: &MergeDownloadTask,
    cancel: Arc<AtomicBool>,
) -> Result<PathBuf, AppError> {
    if task.files.is_empty() {
        return Err(AppError::Lanzou("未选择分片文件".into()));
    }
    let cancelled = || cancel.load(Ordering::SeqCst);
    if cancelled() {
        return Err(AppError::Lanzou("已取消".into()));
    }
    // 按文件名中的序号排序
    let mut files = task.files.clone();
    files.sort_by_key(|f| parse_part_index(&f.name).unwrap_or(0));

    // 目录准备
    let dir = if task.dir.is_empty() {
        dirs::download_dir().unwrap_or_else(std::env::temp_dir)
    } else {
        PathBuf::from(&task.dir)
    };
    std::fs::create_dir_all(&dir)?;
    let merged_name = merged_name(&files.iter().map(|f| f.name.clone()).collect::<Vec<_>>());
    let merged_name = if merged_name.is_empty() {
        "merged".to_string()
    } else {
        merged_name
    };
    let parts_dir = dir.join(format!("{}.parts", merged_name));
    std::fs::create_dir_all(&parts_dir)?;

    // 预检：解析全部分片直链并探测大小，得到合并后总大小（进度条单调递增、总大小一步到位）
    // 探测失败（如 ERROR:102 拦截）时退回增量模式，但仍继续尝试下载
    struct ResolvedPart {
        part: MergePart,
        direct: String,
        referer: String,
    }
    let mut resolved: Vec<ResolvedPart> = Vec::new();
    let mut total_size: u64 = 0;
    let mut total_known = true;
    for part in &files {
        if cancelled() {
            return Err(AppError::Lanzou("已取消".into()));
        }
        let detail = file_detail(client, &part.id).await?;
        let url = detail
            .url
            .ok_or_else(|| AppError::Lanzou(format!("未获取到分享链接: {}", part.name)))?;
        let download = file_download_url(client, &url, detail.pwd.as_deref()).await?;
        match probe_size(client, &download.url, &url).await {
            Ok(size) if size > 0 => {
                total_size += size;
                resolved.push(ResolvedPart {
                    part: part.clone(),
                    direct: download.url,
                    referer: url,
                });
            }
            _ => {
                total_known = false;
                resolved.push(ResolvedPart {
                    part: part.clone(),
                    direct: download.url,
                    referer: url,
                });
            }
        }
    }
    if !total_known {
        total_size = 0;
    }
    // 初始进度事件：下载 0，总大小 = 预检总量，进度条从一开始就显示正确总大小
    let _ = app.emit(
        "download:progress",
        crate::lanzou::core::download::DownloadProgress {
            id: task.id.clone(),
            name: merged_name.clone(),
            downloaded: 0,
            total: total_size,
            speed: 0,
            file_path: None,
        },
    );

    // 逐片下载（实时进度 + 完整性校验 + 失败重试；重试时重新解析直链 + 间隔，规避反爬限流）
    let mut total_downloaded: u64 = 0;
    for (i, rp) in resolved.iter().enumerate() {
        if cancelled() {
            return Err(AppError::Lanzou("已取消".into()));
        }
        // 相邻分片间短暂停顿，避免连续大流量下载触发蓝奏反爬（ERROR:102）
        if i > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        }
        let target = parts_dir.join(sanitize(&rp.part.name));
        let display_name = format!("{} ({}/{})", merged_name, i + 1, resolved.len());
        let mut attempt = 0;
        let bytes = loop {
            if cancelled() {
                return Err(AppError::Lanzou("已取消".into()));
            }
            // 首次用预检直链；失败重试时重新解析拿新签名 URL
            let (direct, referer) = if attempt == 0 {
                (rp.direct.clone(), rp.referer.clone())
            } else {
                let detail = file_detail(client, &rp.part.id).await?;
                let url = detail.url.ok_or_else(|| {
                    AppError::Lanzou(format!("未获取到分享链接: {}", rp.part.name))
                })?;
                let download = file_download_url(client, &url, detail.pwd.as_deref()).await?;
                (download.url, url)
            };
            match download_to_file(
                app,
                &task.id,
                &display_name,
                client,
                &direct,
                &referer,
                &target,
                total_downloaded,
                total_size,
                cancel.clone(),
            )
            .await
            {
                Ok(b) => break b,
                Err(_) if cancelled() => return Err(AppError::Lanzou("已取消".into())),
                Err(e) if attempt < 2 => {
                    attempt += 1;
                    log::warn!(
                        "merge_download[{}]: 分片 {} 第 {attempt} 次失败: {e}，重试",
                        task.id,
                        rp.part.name
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(1200 * attempt as u64))
                        .await;
                }
                Err(e) => return Err(e),
            }
        };
        total_downloaded += bytes;
        // 分片完成事件：累计下载量 + 已知总大小（进度条单调递增）
        let _ = app.emit(
            "download:progress",
            crate::lanzou::core::download::DownloadProgress {
                id: task.id.clone(),
                name: display_name,
                downloaded: total_downloaded,
                total: total_size,
                speed: 0,
                file_path: None,
            },
        );
    }

    // 合并
    let final_path = crate::lanzou::core::download::unique_path(&dir, &dir.join(&merged_name));
    {
        let mut out = std::fs::File::create(&final_path)?;
        let mut buf = vec![0u8; 64 * 1024];
        for part in &files {
            let src = parts_dir.join(sanitize(&part.name));
            let mut f = std::fs::File::open(&src)?;
            loop {
                use std::io::Read;
                let n = f.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                use std::io::Write;
                out.write_all(&buf[..n])?;
            }
        }
    }
    // 兜底校验：合并结果大小必须与下载总量一致，否则删除并报错，避免静默产出损坏文件
    let merged_size = std::fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0);
    if merged_size != total_downloaded {
        let _ = std::fs::remove_file(&final_path);
        return Err(AppError::Lanzou(format!(
            "合并结果大小异常: {} ≠ {}",
            merged_size, total_downloaded
        )));
    }

    // 删除分片
    if !task.keep_parts {
        std::fs::remove_dir_all(&parts_dir)?;
    }

    let final_path_str = final_path.to_string_lossy().into_owned();
    let _ = app.emit(
        "download:progress",
        crate::lanzou::core::download::DownloadProgress {
            id: task.id.clone(),
            name: merged_name.clone(),
            downloaded: total_downloaded,
            total: total_downloaded,
            speed: 0,
            file_path: Some(final_path_str),
        },
    );
    log::info!(
        "merge_download[{}]: 完成 -> {}",
        task.id,
        final_path.display()
    );
    Ok(final_path)
}

/// 探测直链文件大小：发送 Range: bytes=0-0 请求，从 Content-Range 解析总大小。
/// 服务器忽略 Range（返回 200）时用 Content-Length 兜底；被反爬拦截（HTML）时返回 0
async fn probe_size(client: &LanzouClient, url: &str, referer: &str) -> Result<u64, AppError> {
    let mut builder = client.share_request_builder(url, Some(referer))?;
    builder = builder.header(reqwest::header::ACCEPT_ENCODING, "identity");
    builder = builder.header(reqwest::header::RANGE, "bytes=0-0");
    let response = builder.send().await?;
    if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        if let Some(v) = response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
        {
            if let Some(total) = v
                .rsplit('/')
                .next()
                .and_then(|s| s.trim().parse::<u64>().ok())
            {
                return Ok(total);
            }
        }
    }
    Ok(response.content_length().unwrap_or(0))
}

/// 流式下载单个分片到指定路径，实时上报累计进度，返回下载字节数。
/// total_size 为预检的合并后总大小（0 表示未知，退回增量模式）。
/// 校验与 Content-Length 一致，不一致视为下载不完整（由外层重试）
#[allow(clippy::too_many_arguments)]
async fn download_to_file(
    app: &AppHandle,
    task_id: &str,
    display_name: &str,
    client: &LanzouClient,
    url: &str,
    referer: &str,
    target: &Path,
    base_done: u64,
    total_size: u64,
    cancel: Arc<AtomicBool>,
) -> Result<u64, AppError> {
    let mut builder = client.share_request_builder(url, Some(referer))?;
    builder = builder.header(reqwest::header::ACCEPT_ENCODING, "identity");
    let response = builder.send().await?;
    if !response.status().is_success() {
        return Err(AppError::Lanzou(format!(
            "下载分片失败: HTTP {}",
            response.status()
        )));
    }
    // 反爬/错误页检测：响应 content-type 是 HTML 时，直链下载被蓝奏拦截（如 ERROR:102），不是真实文件
    let is_html_ct = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.to_lowercase().contains("text/html"))
        .unwrap_or(false);
    let part_total = response.content_length().unwrap_or(0);
    // 已知合并总大小就用它（进度单调递增）；未知时退回增量（当前分片累计）
    let total = if total_size > 0 {
        total_size
    } else {
        base_done + part_total
    };
    let mut stream = response.bytes_stream();
    // 先读第一块，确认不是 HTML 错误页/验证页
    let first = match stream.next().await {
        Some(Ok(c)) => c,
        Some(Err(e)) => return Err(AppError::Http(e)),
        None => return Err(AppError::Lanzou("下载分片响应为空".into())),
    };
    if is_html_ct || looks_like_html(&first) {
        let text = String::from_utf8_lossy(&first);
        // 若为 acw 反爬挑战页：计算 cookie 后报错，由外层重试（新直链 + 已带 cookie）
        if text.contains("acw_sc__v2") {
            if let Some(cookie) = crate::lanzou::matcher::calc_acw_sc_v2(&text) {
                client.set_cookie("acw_sc__v2", &cookie);
            }
            return Err(AppError::Lanzou("下载分片触发反爬验证，重试中".into()));
        }
        let code = regex::Regex::new(r"ERROR:\d+")
            .ok()
            .and_then(|re| re.find(&text).map(|m| m.as_str().to_string()))
            .unwrap_or_else(|| "HTML 页面".into());
        return Err(AppError::Lanzou(format!("下载分片被拦截: {code}")));
    }
    let mut file = std::fs::File::create(target)?;
    use std::io::Write;
    file.write_all(&first)?;
    let mut downloaded: u64 = first.len() as u64;
    let mut last_emit = std::time::Instant::now();
    // 速度采样：以本次分片已下载字节为基准，窗口 1s 平滑
    let mut speed_win = (0u64, std::time::Instant::now());
    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::SeqCst) {
            return Err(AppError::Lanzou("已取消".into()));
        }
        let chunk = chunk?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        let now = std::time::Instant::now();
        if now.duration_since(last_emit) >= std::time::Duration::from_millis(200) {
            last_emit = now;
            let dt = now.duration_since(speed_win.1).as_secs_f64();
            let speed = if dt > 0.0 {
                ((downloaded - speed_win.0) as f64 / dt) as u64
            } else {
                0
            };
            speed_win = (downloaded, now);
            let _ = app.emit(
                "download:progress",
                crate::lanzou::core::download::DownloadProgress {
                    id: task_id.into(),
                    name: display_name.into(),
                    downloaded: base_done + downloaded,
                    total,
                    speed,
                    file_path: None,
                },
            );
        }
    }
    // 完整性校验：Content-Length 已知但字节不足则报错（网络抖动被截断时由外层重试）
    if part_total > 0 && downloaded != part_total {
        return Err(AppError::Lanzou(format!(
            "分片下载不完整: 预期 {} 字节，实际 {} 字节",
            part_total, downloaded
        )));
    }
    Ok(downloaded)
}

/// 判断响应首块是否为 HTML（蓝奏错误页/验证页），此时不能当作文件数据写入
fn looks_like_html(chunk: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&chunk[..chunk.len().min(1024)]);
    let head = head.trim_start();
    head.starts_with("<!doctype")
        || head.starts_with("<html")
        || head.starts_with("<head")
        || head.starts_with("<body")
        || head.contains("ERROR:")
}

fn sanitize(name: &str) -> String {
    sanitize_filename::sanitize(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merged_name_strips_padding() {
        let names = vec![
            "test video.mp4.001.ct.ke".to_string(),
            "test video.mp4.002.ct.ke".to_string(),
            "test video.mp4.003.ct.ke".to_string(),
        ];
        assert_eq!(merged_name(&names), "test video.mp4");
    }

    #[test]
    fn test_merged_name_no_padding() {
        let names = vec![
            "a.1.ct.ke".to_string(),
            "a.2.ct.ke".to_string(),
            "a.3.ct.ke".to_string(),
        ];
        assert_eq!(merged_name(&names), "a");
    }

    #[test]
    fn test_merged_name_keeps_real_trailing_digits() {
        // 原文件名以数字结尾，不应被误删
        let names = vec![
            "file0.mp4.001.ct.ke".to_string(),
            "file0.mp4.002.ct.ke".to_string(),
        ];
        assert_eq!(merged_name(&names), "file0.mp4");
    }

    #[test]
    fn test_parse_part_index() {
        assert_eq!(parse_part_index("test video.mp4.001.ct.ke"), Some(1));
        assert_eq!(parse_part_index("test video.mp4.002.ct.ke"), Some(2));
        assert_eq!(parse_part_index("普通文件.txt"), None);
    }
}
