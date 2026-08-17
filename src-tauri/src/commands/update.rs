use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::error::AppError;
use crate::state::AppState;

pub const UPDATE_API_URL: &str = "https://api.github.com/repos/Jursin/lanzou-next/releases";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub name: String,
    pub url: String,
    pub published_at: Option<String>,
    pub is_prerelease: bool,
    pub asset_url: Option<String>,
    pub asset_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDownloadProgress {
    pub downloaded: u64,
    pub total: u64,
}

#[tauri::command]
pub async fn cancel_download(state: State<'_, AppState>) -> Result<(), AppError> {
    state
        .update_cancel
        .store(true, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn check_for_update(
    _state: State<'_, AppState>,
    beta: Option<bool>,
) -> Result<Option<UpdateInfo>, AppError> {
    let url = if beta.unwrap_or(false) {
        format!("{UPDATE_API_URL}?per_page=20")
    } else {
        format!("{UPDATE_API_URL}/latest")
    };
    let http = reqwest::Client::builder()
        .user_agent(crate::lanzou::client::DEFAULT_USER_AGENT)
        .build()
        .map_err(|e| AppError::Update(format!("创建 HTTP 客户端失败: {e}")))?;
    let resp = http.get(&url).send().await?;
    if !resp.status().is_success() {
        return Err(AppError::Update(format!(
            "更新接口返回 HTTP {}",
            resp.status()
        )));
    }
    let body = resp.text().await?;
    let release = if beta.unwrap_or(false) {
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

#[tauri::command]
pub async fn download_and_install(
    app: AppHandle,
    state: State<'_, AppState>,
    info: UpdateInfo,
) -> Result<(), AppError> {
    let asset_url = info
        .asset_url
        .ok_or_else(|| AppError::Update("无可用安装包".into()))?;

    let http = reqwest::Client::builder()
        .user_agent(crate::lanzou::client::DEFAULT_USER_AGENT)
        .build()
        .map_err(|e| AppError::Update(format!("创建 HTTP 客户端失败: {e}")))?;
    let resp = http.get(&asset_url).send().await?;
    if !resp.status().is_success() {
        return Err(AppError::Update(format!(
            "下载失败: HTTP {}",
            resp.status()
        )));
    }
    let total = resp.content_length().unwrap_or(0);

    state
        .update_cancel
        .store(false, std::sync::atomic::Ordering::SeqCst);

    use futures_util::StreamExt;
    use std::io::Write;
    let file_name = asset_url.rsplit('/').next().unwrap_or("update.exe");
    let installer_path = std::env::temp_dir().join(file_name);
    let mut file = std::fs::File::create(&installer_path)?;
    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_emit = std::time::Instant::now();
    let cancel_flag = state.update_cancel.clone();

    while let Some(chunk) = stream.next().await {
        if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
            drop(file);
            let _ = std::fs::remove_file(&installer_path);
            state
                .update_cancel
                .store(false, std::sync::atomic::Ordering::SeqCst);
            return Err(AppError::Update("下载已取消".into()));
        }
        let chunk = chunk?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        if last_emit.elapsed() >= std::time::Duration::from_millis(300) {
            let _ = app.emit(
                "update:download-progress",
                UpdateDownloadProgress { downloaded, total },
            );
            last_emit = std::time::Instant::now();
        }
    }
    file.flush()?;
    drop(file);
    let _ = app.emit(
        "update:download-progress",
        UpdateDownloadProgress { downloaded, total },
    );

    // ShellExecuteW("open") + NSIS /P(PassiveMode) /R(安装后重启)
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW;

        let file: Vec<u16> = OsStr::new(installer_path.to_str().unwrap_or(""))
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let args: Vec<u16> = OsStr::new("/P /R")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let verb: Vec<u16> = OsStr::new("open")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                file.as_ptr(),
                args.as_ptr(),
                std::ptr::null(),
                SW_SHOW,
            );
        };
    }

    std::process::exit(0);
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

fn find_platform_asset(assets: &Option<Vec<GitHubAsset>>) -> (Option<String>, Option<String>) {
    let Some(assets) = assets else {
        return (None, None);
    };
    #[cfg(target_os = "windows")]
    {
        let arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x64"
        };
        if let Some(a) = assets
            .iter()
            .find(|a| a.name.contains(arch) && a.name.ends_with("-setup.exe"))
        {
            return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
        }
        if let Some(a) = assets.iter().find(|a| a.name.ends_with(".exe")) {
            return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(a) = assets.iter().find(|a| a.name.ends_with(".deb")) {
            return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
        }
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

/// 版本比较：a > b 返回正数
fn compare_version(a: &str, b: &str) -> i32 {
    let norm = |s: &str| {
        let s = s.trim_start_matches('v');
        let mut nums: Vec<i64> = s
            .split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .take(3)
            .map(|p| p.parse::<i64>().unwrap_or(0))
            .collect();
        nums.resize(3, 0);
        nums
    };
    let (an, bn) = (norm(a), norm(b));
    for i in 0..3 {
        if an[i] != bn[i] {
            return (an[i] - bn[i]).signum() as i32;
        }
    }
    0
}
