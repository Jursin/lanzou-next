use std::path::PathBuf;

use crate::error::AppError;

/// 检查文件路径是否存在
#[tauri::command]
pub fn lanzou_check_path(path: String) -> Result<bool, AppError> {
    Ok(PathBuf::from(&path).is_file())
}

/// 删除本地文件，同时清理断点续传的 .download 临时文件
/// path 为已完成文件的完整路径；dir+name 用于删除暂停/未完成下载留下的临时文件
#[tauri::command]
pub fn lanzou_delete_local(
    path: String,
    dir: Option<String>,
    name: Option<String>,
) -> Result<(), AppError> {
    if !path.is_empty() {
        let p = PathBuf::from(&path);
        remove_file_retry(&p)?;
        remove_file_retry(&PathBuf::from(format!("{path}.download")))?;
    }
    if let (Some(d), Some(n)) = (dir, name) {
        if !n.trim().is_empty() {
            let base = if d.is_empty() {
                dirs::download_dir().unwrap_or_else(std::env::temp_dir)
            } else {
                PathBuf::from(d)
            };
            remove_file_retry(&base.join(format!("{}.download", sanitize_filename::sanitize(&n))))?;
        }
    }
    Ok(())
}

/// 删除合并下载留下的本地分片文件夹（`<下载目录>/<名>.parts`），dir 为空时用系统下载目录
#[tauri::command]
pub fn lanzou_delete_local_dir(dir: String, name: String) -> Result<(), AppError> {
    let base = if dir.is_empty() {
        dirs::download_dir().unwrap_or_else(std::env::temp_dir)
    } else {
        PathBuf::from(&dir)
    };
    let parts_dir = base.join(format!("{}.parts", sanitize_filename::sanitize(&name)));
    remove_dir_retry(&parts_dir)
}

/// 删除文件，失败（如 Windows 下文件被占用）时短暂重试
fn remove_file_retry(p: &std::path::Path) -> Result<(), AppError> {
    for _ in 0..5 {
        match std::fs::remove_file(p) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// 删除目录树，失败（如 Windows 下目录被占用）时短暂重试
fn remove_dir_retry(p: &std::path::Path) -> Result<(), AppError> {
    for _ in 0..5 {
        match std::fs::remove_dir_all(p) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                std::thread::sleep(std::time::Duration::from_millis(80));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_path() {
        let tmp = std::env::temp_dir().join("lanzou_check_test.txt");
        std::fs::write(&tmp, b"x").expect("write");
        assert!(lanzou_check_path(tmp.to_string_lossy().into_owned()).expect("check"));
        let missing = std::env::temp_dir().join("lanzou_missing_xyz.txt");
        assert!(!lanzou_check_path(missing.to_string_lossy().into_owned()).expect("check missing"));
        let _ = std::fs::remove_file(&tmp);
    }
}
