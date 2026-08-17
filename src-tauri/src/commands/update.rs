use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::error::AppError;
use crate::state::AppState;

/// 更新检查源
pub const UPDATE_API_URL: &str = "https://api.github.com/repos/Jursin/lanzou-next/releases";

/// 最新版本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub name: String,
    pub url: String,
    pub published_at: Option<String>,
    pub is_prerelease: bool,
    /// 当前平台安装包下载地址（null 表示无匹配资产）
    pub asset_url: Option<String>,
    /// 安装包文件名
    pub asset_name: Option<String>,
}

/// 下载进度事件
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDownloadProgress {
    pub downloaded: u64,
    pub total: u64,
}

/// 检查更新
#[tauri::command]
pub async fn check_for_update(
    state: State<'_, AppState>,
    beta: Option<bool>,
) -> Result<Option<UpdateInfo>, AppError> {
    let client = state.client.lock().await;
    let beta = beta.unwrap_or(false);

    let url = if beta {
        format!("{UPDATE_API_URL}?per_page=20")
    } else {
        format!("{UPDATE_API_URL}/latest")
    };
    let resp = client.share_request(&url, None).await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        return Err(AppError::Update(format!("更新接口返回 HTTP {status}")));
    }

    let release = if beta {
        parse_latest_pre_release(&body)?
    } else {
        parse_release(&body)?
    };
    let Some(release) = release else {
        return Ok(None);
    };

    let current = env!("CARGO_PKG_VERSION");
    if compare_version(&release.tag_name, current) <= 0 {
        return Ok(None);
    }
    Ok(Some(release.into_info()))
}

/// 下载并安装更新
#[tauri::command]
pub async fn download_and_install(
    app: AppHandle,
    state: State<'_, AppState>,
    info: UpdateInfo,
) -> Result<(), AppError> {
    let asset_url = info
        .asset_url
        .ok_or_else(|| AppError::Update("无可用安装包".into()))?;
    let asset_name = info
        .asset_name
        .ok_or_else(|| AppError::Update("安装包信息缺失".into()))?;

    // 下载到系统临时目录
    let tmp_dir = std::env::temp_dir();
    let installer_path = tmp_dir.join(&asset_name);

    // 流式下载
    let client = state.client.lock().await;
    let resp = client.share_request(&asset_url, None).await?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AppError::Update(format!("下载失败: HTTP {status}")));
    }
    let total = resp.content_length().unwrap_or(0);
    let mut stream = resp.bytes_stream();

    use futures_util::StreamExt;
    use std::io::Write;
    let mut file = std::fs::File::create(&installer_path)?;
    let mut downloaded: u64 = 0;
    let mut last_emit = std::time::Instant::now();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;

        if last_emit.elapsed() >= std::time::Duration::from_millis(300) {
            let _ = app.emit(
                "update:download-progress",
                UpdateDownloadProgress {
                    downloaded,
                    total,
                },
            );
            last_emit = std::time::Instant::now();
        }
    }
    file.flush()?;

    // 下载完成事件
    let _ = app.emit(
        "update:download-progress",
        UpdateDownloadProgress {
            downloaded,
            total,
        },
    );

    drop(file);
    drop(client);

    // 启动安装程序并退出应用
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let _ = Command::new(&installer_path)
            .args(["/S"])
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        // .deb 包：用 pkexec 调用 dpkg 安装
        if asset_name.ends_with(".deb") {
            use std::process::Command;
            let _ = Command::new("pkexec")
                .args(["dpkg", "-i", installer_path.to_str().unwrap_or("")])
                .spawn();
        }
    }

    // 等待安装程序启动后退出当前应用
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    app.exit(0);

    Ok(())
}

#[derive(Deserialize, Clone)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    html_url: String,
    published_at: Option<String>,
    prerelease: bool,
    assets: Option<Vec<GitHubAsset>>,
}

#[derive(Deserialize, Clone)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

impl GitHubRelease {
    fn into_info(self) -> UpdateInfo {
        let (asset_url, asset_name) = find_platform_asset(&self.assets);
        UpdateInfo {
            version: self.tag_name.trim_start_matches('v').to_string(),
            name: self.name.unwrap_or_else(|| self.tag_name.clone()),
            url: self.html_url,
            published_at: self.published_at,
            is_prerelease: self.prerelease,
            asset_url,
            asset_name,
        }
    }
}

/// 根据当前平台从 release assets 中匹配安装包
fn find_platform_asset(assets: &Option<Vec<GitHubAsset>>) -> (Option<String>, Option<String>) {
    let Some(assets) = assets else {
        return (None, None);
    };

    #[cfg(target_os = "windows")]
    {
        let target_arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x64"
        };
        // 优先匹配 NSIS 安装包
        if let Some(a) = assets.iter().find(|a| {
            a.name.contains(target_arch)
                && a.name.ends_with("-setup.exe")
        }) {
            return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
        }
        // 回退：匹配任意 exe
        if let Some(a) = assets.iter().find(|a| a.name.ends_with(".exe")) {
            return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
        }
    }

    #[cfg(target_os = "linux")]
    {
        // 优先匹配 .deb
        if let Some(a) = assets.iter().find(|a| a.name.ends_with(".deb")) {
            return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
        }
        // 回退：AppImage
        if let Some(a) = assets.iter().find(|a| a.name.ends_with(".AppImage")) {
            return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
        }
    }

    (None, None)
}

fn parse_release(body: &str) -> Result<Option<GitHubRelease>, AppError> {
    Ok(Some(serde_json::from_str(body)?))
}

fn parse_latest_pre_release(body: &str) -> Result<Option<GitHubRelease>, AppError> {
    let releases: Vec<GitHubRelease> = serde_json::from_str(body)?;
    let found = releases.iter().find(|r| r.prerelease).cloned();
    Ok(found.or_else(|| releases.first().cloned()))
}

/// 简易语义化版本比较：a > b 返回正数。支持 "1.2.3" / "v1.2.3" / 预发布后缀（忽略后缀）
fn compare_version(a: &str, b: &str) -> i32 {
    let norm = |s: &str| {
        let s = s.trim_start_matches('v');
        let nums: Vec<i64> = s
            .split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .take(3)
            .map(|p| p.parse::<i64>().unwrap_or(0))
            .collect();
        let mut padded = nums.clone();
        while padded.len() < 3 {
            padded.push(0);
        }
        padded
    };
    let (an, bn) = (norm(a), norm(b));
    for i in 0..3 {
        if an[i] != bn[i] {
            return (an[i] - bn[i]).signum() as i32;
        }
    }
    0
}
