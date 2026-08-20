mod commands;
mod error;
mod lanzou;
mod log_policy;
mod state;

use commands::config::{config_clear, config_get, config_reset, config_set};
use commands::lanzou::{
    lanzou_ls, lanzou_merge_download, lanzou_profile, lanzou_share_folder, lanzou_share_info,
    lanzou_upload_precheck,
};
use commands::log::{log_clear, log_get_file};
use commands::login::{lanzou_login, lanzou_logout};
use commands::ops::{
    lanzou_file_description, lanzou_file_detail, lanzou_folder_detail, lanzou_mkdir, lanzou_move,
    lanzou_recycle_action, lanzou_recycle_files, lanzou_recycle_list, lanzou_rename_file,
    lanzou_rename_folder, lanzou_rm_file, lanzou_rm_folder, lanzou_set_file_access,
    lanzou_set_file_description, lanzou_set_folder_access,
};
use commands::update::{cancel_download, check_for_update, download_and_install};
use lanzou::core::download::{lanzou_cancel_transfer, lanzou_download, lanzou_download_by_id};
use lanzou::core::files::{lanzou_check_path, lanzou_delete_local, lanzou_delete_local_dir};
use lanzou::core::upload::lanzou_upload;
use state::AppState;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

fn show_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    } else {
        let win = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
            .title("蓝奏云盘")
            .inner_size(1260.0, 800.0)
            .min_inner_size(960.0, 640.0)
            .resizable(true)
            .decorations(false)
            .visible(false)
            .build()
            .expect("创建主窗口失败");
        let _ = win.set_focus();
    }
}

// 轻量模式下销毁窗口以释放内存，否则隐藏窗口。
fn minimize_to_tray<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let lightweight = commands::config::read_bool(app, "lightweight_mode", true);
    if let Some(win) = app.get_webview_window("main") {
        if lightweight {
            let _ = win.destroy();
        } else {
            let _ = win.hide();
        }
    }
}

// 托盘入口：左键恢复窗口，菜单中提供退出动作。
fn setup_tray<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示主界面", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    TrayIconBuilder::with_id("main-tray")
        .icon(tauri::image::Image::from_bytes(include_bytes!(
            "../icons/32x32.png"
        ))?)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

// 根据配置同步托盘显示状态：开启时创建，关闭时移除。
pub(crate) fn sync_tray_visibility<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let on = commands::config::read_bool(app, "minimize_to_tray_on_close", true);
    let has = app.tray_by_id("main-tray").is_some();
    if on && !has {
        let _ = setup_tray(app);
    } else if !on && has {
        let _ = app.remove_tray_by_id("main-tray");
    }
}

/// 启动前直读配置文件中用户设置的日志级别（store 插件此时尚未初始化）。
/// 读取失败或未设置时回退为默认 warn。
fn read_log_level() -> log::LevelFilter {
    (|| -> Option<log::LevelFilter> {
        let data_dir = dirs::data_dir()?.join("com.lanzou.next");
        let store_path = data_dir.join("config.json");
        let content = std::fs::read_to_string(store_path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;
        let level = json.get("log_level")?.as_str()?;
        Some(log_policy::log_level_filter(level))
    })()
    .unwrap_or(log::LevelFilter::Warn)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let log_level = read_log_level();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some(log_policy::LANZOU_LOG_NAME.into()),
                    }),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                ])
                .format(|out, message, record| {
                    let now = chrono::Local::now();
                    out.finish(format_args!(
                        "{} [{:<5}] {}",
                        now.format("%Y-%m-%dT%H:%M:%S%.3f"),
                        record.level(),
                        message
                    ))
                })
                .max_file_size(log_policy::MAX_LOG_FILE_SIZE.into())
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
                .level(log_level)
                .level_for("reqwest", log::LevelFilter::Warn)
                .level_for("hyper_util", log::LevelFilter::Warn)
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .setup(|app| {
            // 首次运行写入默认配置
            commands::config::ensure_defaults(app.handle());
            // 启动时恢复登录会话（domain + user-agent + cookies）
            let (domain, ua, cookies) = commands::config::load_session_config(app.handle());
            let state = app.state::<AppState>();
            let mut client = tauri::async_runtime::block_on(state.client.lock());
            if let Some(ua) = ua {
                client.set_user_agent(&ua);
            }
            if let Some(domain) = domain
                && let Ok(url) = url::Url::parse(&domain)
            {
                let _ = client.set_base_url(url.as_ref());
            }
            if let Some(cookies) = cookies {
                for c in cookies {
                    client.set_cookie(&c.name, &c.value);
                }
            }
            // 系统托盘（仅开启"关闭时最小化到托盘"时显示）
            setup_tray(app.handle())?;
            sync_tray_visibility(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let to_tray = commands::config::read_bool(app, "minimize_to_tray_on_close", true);
                log::info!("close requested: minimize_to_tray_on_close={to_tray}");
                if to_tray {
                    api.prevent_close();
                    minimize_to_tray(app);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            config_get,
            config_set,
            config_reset,
            config_clear,
            log_get_file,
            log_clear,
            lanzou_ls,
            lanzou_profile,
            lanzou_share_info,
            lanzou_share_folder,
            lanzou_download,
            lanzou_download_by_id,
            lanzou_cancel_transfer,
            lanzou_upload,
            lanzou_upload_precheck,
            lanzou_merge_download,
            lanzou_check_path,
            lanzou_delete_local,
            lanzou_delete_local_dir,
            lanzou_login,
            lanzou_logout,
            lanzou_mkdir,
            lanzou_rm_file,
            lanzou_rm_folder,
            lanzou_rename_file,
            lanzou_rename_folder,
            lanzou_move,
            lanzou_recycle_list,
            lanzou_recycle_files,
            lanzou_recycle_action,
            lanzou_set_file_access,
            lanzou_set_folder_access,
            lanzou_file_description,
            lanzou_set_file_description,
            lanzou_file_detail,
            lanzou_folder_detail,
            check_for_update,
            cancel_download,
            download_and_install,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // 关闭时最小化到托盘：仅拦截"窗口关闭触发"的退出（code=None，非显式 app.exit）；
            // 显式退出（托盘菜单"退出"等，code=Some）直接放行
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event
                && code.is_none()
                && commands::config::read_bool(app, "minimize_to_tray_on_close", true)
            {
                api.prevent_exit();
            }
        });
}
