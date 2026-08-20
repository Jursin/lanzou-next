use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_store::StoreExt;

use crate::error::AppError;
use crate::state::AppState;

pub const UPDATE_API_URL: &str = "https://api.github.com/repos/Jursin/lanzou-next/releases";

/// 如果配置了 GitHub 加速地址，将加速地址拼接到下载 URL 前面
fn apply_github_proxy<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    url: &str,
) -> Result<String, AppError> {
    let store = app
        .store(crate::commands::config::CONFIG_STORE_FILE)
        .map_err(|e| AppError::Update(e.to_string()))?;
    let proxy = store
        .get("github_proxy_url")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    if proxy.is_empty() || !url.starts_with("https://github.com") {
        return Ok(url.to_string());
    }
    let proxy = proxy.trim_end_matches('/');
    Ok(format!("{proxy}/{url}"))
}

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
    app: AppHandle,
    _state: State<'_, AppState>,
    beta: Option<bool>,
) -> Result<Option<UpdateInfo>, AppError> {
    let include_prerelease = beta.unwrap_or(false);
    let url = apply_github_proxy(&app, &format!("{UPDATE_API_URL}?per_page=100"))?;
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
    let releases: Vec<GitHubRelease> = serde_json::from_str(&body)?;
    let best = releases
        .iter()
        .filter(|r| include_prerelease || !r.prerelease)
        .max_by(|a, b| compare_version(&a.tag_name, &b.tag_name).cmp(&0));
    let Some(release) = best else {
        return Ok(None);
    };
    let current = env!("CARGO_PKG_VERSION");
    if compare_version(&release.tag_name, current) <= 0 {
        return Ok(None);
    }
    Ok(Some(release.clone().into_info()))
}

#[tauri::command]
pub async fn download_and_install(
    app: AppHandle,
    state: State<'_, AppState>,
    info: UpdateInfo,
) -> Result<(), AppError> {
    // 重置取消标志必须在发起请求之前，否则会覆盖用户刚发出的取消请求
    state
        .update_cancel
        .store(false, std::sync::atomic::Ordering::SeqCst);

    let asset_url = info
        .asset_url
        .ok_or_else(|| AppError::Update("无可用安装包".into()))?;
    let asset_url = apply_github_proxy(&app, &asset_url)?;

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
    let cancel_flag = state.update_cancel.clone();

    // 请求期间用户可能已取消，这里再检查一次
    if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
        state
            .update_cancel
            .store(false, std::sync::atomic::Ordering::SeqCst);
        return Err(AppError::Update("下载已取消".into()));
    }

    use futures_util::StreamExt;
    use std::io::Write;
    let file_name = asset_url.rsplit('/').next().unwrap_or("update.exe");
    let installer_path = std::env::temp_dir().join(file_name);
    let mut file = std::fs::File::create(&installer_path)?;
    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_emit = std::time::Instant::now();

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

    // 下载完成后、启动安装前，若用户已取消则放弃安装
    if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
        let _ = std::fs::remove_file(&installer_path);
        state
            .update_cancel
            .store(false, std::sync::atomic::Ordering::SeqCst);
        return Err(AppError::Update("下载已取消".into()));
    }

    let _ = app.emit(
        "update:download-progress",
        UpdateDownloadProgress { downloaded, total },
    );

    // ShellExecuteW("open") + NSIS /P(静默模式) /R(安装后重启)
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

        std::process::exit(0);
    }

    #[cfg(target_os = "macos")]
    {
        let app_name = "Lanzou-Next";
        let app_bundle = format!("{app_name}.app");
        let applications = std::path::PathBuf::from("/Applications");

        // 挂载 DMG
        let mount_output = std::process::Command::new("hdiutil")
            .args(["attach", "-nobrowse", "-quiet"])
            .arg(&installer_path)
            .output()
            .map_err(|e| AppError::Update(format!("挂载 DMG 失败: {e}")))?;

        if !mount_output.status.success() {
            return Err(AppError::Update("挂载 DMG 失败".into()));
        }

        // 查找挂载点
        let volumes = std::fs::read_dir("/Volumes")
            .map_err(|e| AppError::Update(format!("读取 /Volumes 失败: {e}")))?;

        let mut mount_point = None;
        for entry in volumes.flatten() {
            let path = entry.path();
            if path.join(&app_bundle).exists() {
                mount_point = Some(path);
                break;
            }
        }

        let mount_point = match mount_point {
            Some(p) => p,
            None => {
                return Err(AppError::Update("DMG 中未找到应用".into()));
            }
        };

        let src_app = mount_point.join(&app_bundle);
        let dst_app = applications.join(&app_bundle);

        // 优先直接复制，失败则通过 osascript 提权
        let copy_ok = std::process::Command::new("ditto")
            .args(["--replace", "--keepParent"])
            .arg(&src_app)
            .arg(&dst_app)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if !copy_ok {
            let script = format!(
                r#"do shell script "ditto --replace --keepParent '{}' '/Applications/{}'" with administrator privileges"#,
                src_app.display(),
                app_bundle,
            );
            let status = std::process::Command::new("osascript")
                .args(["-e", &script])
                .status()
                .map_err(|e| AppError::Update(format!("需要管理员权限: {e}")))?;

            if !status.success() {
                let _ = std::process::Command::new("hdiutil")
                    .args(["detach", "-quiet", "-nobrowse"])
                    .arg(&mount_point)
                    .output();
                return Err(AppError::Update("安装失败，请手动安装".into()));
            }
        }

        // 卸载 DMG
        let _ = std::process::Command::new("hdiutil")
            .args(["detach", "-quiet", "-nobrowse"])
            .arg(&mount_point)
            .output();

        // 清理下载的 DMG
        let _ = std::fs::remove_file(&installer_path);

        app.restart();
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(appimage) = std::env::var("APPIMAGE") {
            let appimage = std::path::PathBuf::from(appimage);
            // 先把正在运行的 AppImage 改名移走（旧 inode 继续运行），释放原路径后再写入新文件
            let backup = appimage.with_extension("AppImage.bak");
            let _ = std::fs::remove_file(&backup);
            std::fs::rename(&appimage, &backup)?;
            let perms = backup.metadata()?.permissions();
            // 新文件优先 rename（同设备原子），跨设备则回退复制
            if std::fs::rename(&installer_path, &appimage).is_err() {
                std::fs::copy(&installer_path, &appimage)?;
                let _ = std::fs::remove_file(&installer_path);
            }
            // 恢复原始可执行权限
            std::fs::set_permissions(&appimage, perms)?;
            let _ = std::fs::remove_file(&backup);
            app.restart();
        } else {
            let ext = installer_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            let install_ok = if ext == "deb" {
                let status = std::process::Command::new("pkexec")
                    .args(["dpkg", "-i"])
                    .arg(&installer_path)
                    .status();
                if status.as_ref().map(|s| !s.success()).unwrap_or(false) {
                    let _ = std::process::Command::new("sudo")
                        .args(["apt-get", "install", "-f", "-y"])
                        .status();
                }
                status.map(|s| s.success()).unwrap_or(false)
            } else if ext == "rpm" {
                std::process::Command::new("pkexec")
                    .args(["rpm", "-U"])
                    .arg(&installer_path)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            } else {
                // .pkg.tar.zst
                std::process::Command::new("pkexec")
                    .args(["pacman", "-U", "--noconfirm"])
                    .arg(&installer_path)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            };

            if install_ok {
                let _ = std::fs::remove_file(&installer_path);
                app.restart();
            }
            Err(AppError::Update("安装失败，请手动安装下载的包".into()))
        }
    }
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
        let is_appimage = std::env::var("APPIMAGE").is_ok();
        if is_appimage {
            let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "amd64" };
            if let Some(a) = assets.iter().find(|a| a.name.contains(arch) && a.name.ends_with(".AppImage")) {
                return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
            }
            if let Some(a) = assets.iter().find(|a| a.name.ends_with(".AppImage")) {
                return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
            }
        }
        // 检测包管理器以选择对应资产
        let has_pacman = which("pacman");
        let has_dpkg = which("dpkg");
        if has_pacman {
            let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
            if let Some(a) = assets.iter().find(|a| a.name.contains(arch) && a.name.ends_with(".pkg.tar.zst")) {
                return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
            }
            if let Some(a) = assets.iter().find(|a| a.name.ends_with(".pkg.tar.zst")) {
                return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
            }
        } else if has_dpkg {
            let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "amd64" };
            if let Some(a) = assets.iter().find(|a| a.name.contains(arch) && a.name.ends_with(".deb")) {
                return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
            }
            if let Some(a) = assets.iter().find(|a| a.name.ends_with(".deb")) {
                return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
            }
        } else {
            // 基于 rpm 的发行版
            let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
            if let Some(a) = assets.iter().find(|a| a.name.contains(arch) && a.name.ends_with(".rpm")) {
                return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
            }
            if let Some(a) = assets.iter().find(|a| a.name.ends_with(".rpm")) {
                return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
            }
        }
        // 回退：尝试任意可用资产
        if let Some(a) = assets.iter().find(|a| a.name.ends_with(".AppImage")) {
            return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
        }
    }
    #[cfg(target_os = "macos")]
    {
        let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
        if let Some(a) = assets.iter().find(|a| a.name.contains(arch) && a.name.ends_with(".dmg")) {
            return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
        }
        if let Some(a) = assets.iter().find(|a| a.name.ends_with(".dmg")) {
            return (Some(a.browser_download_url.clone()), Some(a.name.clone()));
        }
    }
    (None, None)
}

/// 检查指定名称的二进制文件是否在 PATH 中
#[cfg(target_os = "linux")]
fn which(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
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
