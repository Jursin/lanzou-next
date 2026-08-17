use tauri::{AppHandle, Manager, Runtime};

use crate::error::AppError;
use crate::log_policy::LANZOU_LOG_FILE;

/// 当前日志文件路径（可能尚不存在）
#[tauri::command]
pub fn log_get_file<R: Runtime>(app: AppHandle<R>) -> Result<String, AppError> {
    let log_dir = app.path().app_log_dir()?;
    Ok(log_dir.join(LANZOU_LOG_FILE).to_string_lossy().into_owned())
}

/// 清空日志文件（KeepOne 旋转策略下仅存在单个活动日志文件）
#[tauri::command]
pub fn log_clear<R: Runtime>(app: AppHandle<R>) -> Result<(), AppError> {
    let log_dir = app.path().app_log_dir()?;
    let log_file = log_dir.join(LANZOU_LOG_FILE);
    if log_file.exists() {
        std::fs::remove_file(log_file)?;
    }
    Ok(())
}
