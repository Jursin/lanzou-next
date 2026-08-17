use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_store::StoreExt;

use crate::commands::lanzou::Cookie;
use crate::error::AppError;
use crate::lanzou::client::DEFAULT_USER_AGENT;
use crate::state::AppState;

pub const CONFIG_STORE_FILE: &str = "config.json";

/// 系统默认下载目录（未手动设置下载位置时的默认值）
pub fn system_download_dir<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    app.path()
        .download_dir()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    /// 蓝奏云接口地址
    pub lanzou_url: Option<String>,
    /// 登录后使用的域名（up.woozooo.com）
    pub domain: Option<String>,
    /// 请求使用的 user-agent（与登录窗口一致）
    pub user_agent: Option<String>,
    /// 登录 cookies
    pub cookies: Option<Vec<Cookie>>,
    /// 下载目录
    pub download_dir: Option<String>,
    /// 是否默认此地址为下载路径
    pub set_default_download_dir: Option<bool>,
    /// 主题: light | dark | auto
    pub theme_source: Option<String>,
    /// 配色方案 id
    pub color_scheme: Option<String>,
    /// 同时上传数量
    pub upload_max: Option<u32>,
    /// 同时下载数量
    pub download_max: Option<u32>,
    /// 上传流量警戒线（单位 G），设置了值即开启警戒
    pub upload_warning_size: Option<u32>,
    /// 文件分片大小（单位 MB），分片上传时单片大小
    pub split_size: Option<u32>,
    /// 关闭时最小化到托盘
    pub minimize_to_tray_on_close: Option<bool>,
    /// 轻量模式：最小化到托盘时销毁渲染进程降低内存
    pub lightweight_mode: Option<bool>,
    /// 开发者模式：右键显示检查菜单
    pub developer_mode: Option<bool>,
    /// 日志级别: error | warn | info | debug | trace
    pub log_level: Option<String>,
    /// 启动时自动检查更新
    pub auto_check_update: Option<bool>,
    /// 接收测试版更新
    pub beta_update: Option<bool>,
    /// 上次检查更新时间（Unix 毫秒时间戳）
    pub last_check_update_time: Option<u64>,
}

fn store<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<std::sync::Arc<tauri_plugin_store::Store<R>>, AppError> {
    app.store(CONFIG_STORE_FILE)
        .map_err(|e| AppError::Config(e.to_string()))
}

#[tauri::command]
pub fn config_get(app: AppHandle) -> Result<AppConfig, AppError> {
    let store = store(&app)?;
    let get = |key: &str| store.get(key);
    Ok(AppConfig {
        lanzou_url: get("lanzou_url").and_then(|v| v.as_str().map(String::from)),
        domain: get("domain").and_then(|v| v.as_str().map(String::from)),
        // 未持久化过 UA 时回落到系统默认值，保证前端始终有值可展示/重置
        user_agent: get("user_agent")
            .and_then(|v| v.as_str().map(String::from))
            .or_else(|| Some(DEFAULT_USER_AGENT.to_string())),
        cookies: get("cookies").and_then(|v| serde_json::from_value::<Vec<Cookie>>(v.clone()).ok()),
        download_dir: match get("download_dir").and_then(|v| v.as_str().map(String::from)) {
            Some(d) => Some(d),
            None => system_download_dir(&app),
        },
        set_default_download_dir: get("set_default_download_dir").and_then(|v| v.as_bool()),
        theme_source: get("theme_source").and_then(|v| v.as_str().map(String::from)),
        color_scheme: get("color_scheme").and_then(|v| v.as_str().map(String::from)),
        upload_max: get("upload_max").and_then(|v| v.as_u64()).map(|v| v as u32),
        download_max: get("download_max")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        upload_warning_size: get("upload_warning_size")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        split_size: get("split_size").and_then(|v| v.as_u64()).map(|v| v as u32),
        minimize_to_tray_on_close: get("minimize_to_tray_on_close").and_then(|v| v.as_bool()),
        lightweight_mode: get("lightweight_mode").and_then(|v| v.as_bool()),
        developer_mode: get("developer_mode").and_then(|v| v.as_bool()),
        log_level: get("log_level").and_then(|v| v.as_str().map(String::from)),
        auto_check_update: get("auto_check_update").and_then(|v| v.as_bool()),
        beta_update: get("beta_update").and_then(|v| v.as_bool()),
        last_check_update_time: get("last_check_update_time").and_then(|v| v.as_u64()),
    })
}

#[tauri::command]
pub async fn config_set(app: AppHandle, cfg: AppConfig) -> Result<(), AppError> {
    let store = store(&app)?;
    if let Some(v) = cfg.lanzou_url {
        store.set("lanzou_url", serde_json::json!(v));
    }
    if let Some(v) = cfg.domain {
        store.set("domain", serde_json::json!(v));
    }
    if let Some(v) = cfg.user_agent {
        let v = v.trim().to_string();
        if v.is_empty() {
            return Err(AppError::Config("User-Agent 不能为空".into()));
        }
        store.set("user_agent", serde_json::json!(v));
        // 立即应用到运行中的请求客户端（此后所有请求使用新 UA）
        app.state::<AppState>()
            .client
            .lock()
            .await
            .set_user_agent(&v);
    }
    if let Some(v) = cfg.cookies {
        store.set("cookies", serde_json::json!(v));
    }
    if let Some(v) = cfg.download_dir {
        store.set("download_dir", serde_json::json!(v));
    }
    if let Some(v) = cfg.set_default_download_dir {
        store.set("set_default_download_dir", serde_json::json!(v));
    }
    if let Some(v) = cfg.theme_source {
        store.set("theme_source", serde_json::json!(v));
    }
    if let Some(v) = cfg.color_scheme {
        store.set("color_scheme", serde_json::json!(v));
    }
    if let Some(v) = cfg.upload_max {
        store.set("upload_max", serde_json::json!(v));
    }
    if let Some(v) = cfg.download_max {
        store.set("download_max", serde_json::json!(v));
    }
    if let Some(v) = cfg.upload_warning_size {
        store.set("upload_warning_size", serde_json::json!(v));
    }
    if let Some(v) = cfg.split_size {
        store.set("split_size", serde_json::json!(v));
    }
    if let Some(v) = cfg.minimize_to_tray_on_close {
        store.set("minimize_to_tray_on_close", serde_json::json!(v));
        // 同步托盘图标显示/隐藏
        crate::sync_tray_visibility(&app);
    }
    if let Some(v) = cfg.lightweight_mode {
        store.set("lightweight_mode", serde_json::json!(v));
    }
    if let Some(v) = cfg.developer_mode {
        store.set("developer_mode", serde_json::json!(v));
    }
    if let Some(v) = cfg.log_level {
        if !crate::log_policy::valid_log_level(&v) {
            return Err(AppError::Config(format!("无效的日志级别: {v}")));
        }
        store.set("log_level", serde_json::json!(v));
    }
    if let Some(v) = cfg.auto_check_update {
        store.set("auto_check_update", serde_json::json!(v));
    }
    if let Some(v) = cfg.beta_update {
        store.set("beta_update", serde_json::json!(v));
    }
    if let Some(v) = cfg.last_check_update_time {
        store.set("last_check_update_time", serde_json::json!(v));
    }
    store.save().map_err(|e| AppError::Config(e.to_string()))?;
    Ok(())
}

/// 删除指定配置键，键名为存储键（snake_case）
#[tauri::command]
pub fn config_clear(app: AppHandle, keys: Vec<String>) -> Result<(), AppError> {
    let store = store(&app)?;
    for key in &keys {
        store.delete(key);
    }
    store.save().map_err(|e| AppError::Config(e.to_string()))?;
    if keys.iter().any(|k| k == "minimize_to_tray_on_close") {
        crate::sync_tray_visibility(&app);
    }
    Ok(())
}

/// 默认配置项（不含登录态），写回存储使其持久化，供恢复默认与首次运行使用
fn write_defaults<R: Runtime>(
    app: &AppHandle<R>,
    store: &std::sync::Arc<tauri_plugin_store::Store<R>>,
) -> AppConfig {
    let download_dir = system_download_dir(app).unwrap_or_default();
    let default_cfg = AppConfig {
        lanzou_url: None,
        domain: None,
        user_agent: Some(DEFAULT_USER_AGENT.to_string()),
        cookies: None,
        download_dir: Some(download_dir.clone()),
        set_default_download_dir: Some(false),
        theme_source: Some("auto".into()),
        color_scheme: Some("glacier".into()),
        upload_max: Some(1),
        download_max: Some(2),
        upload_warning_size: Some(7),
        split_size: Some(100),
        minimize_to_tray_on_close: Some(true),
        lightweight_mode: Some(true),
        developer_mode: Some(false),
        log_level: Some("warn".into()),
        auto_check_update: Some(true),
        beta_update: Some(false),
        last_check_update_time: None,
    };
    store.set("download_dir", serde_json::json!(download_dir));
    store.set("set_default_download_dir", serde_json::json!(false));
    store.set("user_agent", serde_json::json!(DEFAULT_USER_AGENT));
    store.set("theme_source", serde_json::json!("auto"));
    store.set("color_scheme", serde_json::json!("glacier"));
    store.set("upload_max", serde_json::json!(1));
    store.set("download_max", serde_json::json!(2));
    store.set("upload_warning_size", serde_json::json!(7));
    store.set("split_size", serde_json::json!(100));
    store.set("minimize_to_tray_on_close", serde_json::json!(true));
    store.set("lightweight_mode", serde_json::json!(true));
    store.set("developer_mode", serde_json::json!(false));
    store.set("log_level", serde_json::json!("warn"));
    store.set("auto_check_update", serde_json::json!(true));
    store.set("beta_update", serde_json::json!(false));
    default_cfg
}

/// 首次运行时初始化默认配置（存储为空时写入默认值）
pub fn ensure_defaults<R: Runtime>(app: &AppHandle<R>) {
    let Ok(store) = store(app) else { return };
    if !store.is_empty() {
        return;
    }
    write_defaults(app, &store);
    let _ = store.save();
}

/// 恢复默认设置：保留登录态（接口地址/域名/UA/cookies），其余配置写回默认值
#[tauri::command]
pub fn config_reset(app: AppHandle) -> Result<AppConfig, AppError> {
    let store = store(&app)?;
    let lanzou_url = store
        .get("lanzou_url")
        .and_then(|v| v.as_str().map(String::from));
    let domain = store
        .get("domain")
        .and_then(|v| v.as_str().map(String::from));
    let user_agent = store
        .get("user_agent")
        .and_then(|v| v.as_str().map(String::from));
    let cookies = store
        .get("cookies")
        .and_then(|v| serde_json::from_value::<Vec<Cookie>>(v.clone()).ok());
    // 清理旧遗留键
    store.delete("upload_warning_enabled");
    let mut default_cfg = write_defaults(&app, &store);
    store.save().map_err(|e| AppError::Config(e.to_string()))?;
    // 托盘开关被重置，需同步托盘图标状态
    crate::sync_tray_visibility(&app);
    default_cfg.lanzou_url = lanzou_url;
    default_cfg.domain = domain;
    default_cfg.user_agent = user_agent;
    default_cfg.cookies = cookies;
    Ok(default_cfg)
}

/// 读取启动时用于恢复登录的配置项（仅网络相关）
pub fn load_session_config<R: Runtime>(
    app: &AppHandle<R>,
) -> (Option<String>, Option<String>, Option<Vec<Cookie>>) {
    let Ok(store) = store(app) else {
        return (None, None, None);
    };
    let domain = store
        .get("domain")
        .and_then(|v| v.as_str().map(String::from));
    let ua = store
        .get("user_agent")
        .and_then(|v| v.as_str().map(String::from));
    let cookies = store
        .get("cookies")
        .and_then(|v| serde_json::from_value::<Vec<Cookie>>(v.clone()).ok());
    (domain, ua, cookies)
}

/// 读取布尔配置（不存在时返回默认值），供托盘/窗口逻辑使用
pub fn read_bool<R: Runtime>(app: &AppHandle<R>, key: &str, default: bool) -> bool {
    let Ok(store) = store(app) else {
        return default;
    };
    store.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}
