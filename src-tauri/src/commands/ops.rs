use tauri::State;

use crate::error::AppError;
use crate::lanzou::core::ops::{
    file_description, file_detail, folder_detail, mkdir, move_items, rename_file, rename_folder,
    rm_file, rm_folder, set_file_access, set_file_description, set_folder_access, FileDesc,
    MoveTarget, OpResult, ShareDetail,
};
use crate::lanzou::core::recycle::{
    recycle_action, recycle_files, recycle_list, RecycleFile, RecycleItem,
};
use crate::state::AppState;

/// 新建文件夹
#[tauri::command]
pub async fn lanzou_mkdir(
    state: State<'_, AppState>,
    parent_id: i64,
    name: String,
    description: Option<String>,
) -> Result<String, AppError> {
    let client = state.client.lock().await;
    let id = mkdir(&client, parent_id, &name, description.as_deref()).await?;
    log::info!("lanzou_mkdir: parent_id={parent_id}, name={name}, new_id={id}");
    Ok(id)
}

/// 删除文件
#[tauri::command]
pub async fn lanzou_rm_file(
    state: State<'_, AppState>,
    file_id: String,
) -> Result<OpResult, AppError> {
    let client = state.client.lock().await;
    let result = rm_file(&client, &file_id).await?;
    log::info!("lanzou_rm_file: file_id={file_id}, ok={}", result.ok);
    Ok(result)
}

/// 删除文件夹
#[tauri::command]
pub async fn lanzou_rm_folder(
    state: State<'_, AppState>,
    folder_id: String,
) -> Result<OpResult, AppError> {
    let client = state.client.lock().await;
    let result = rm_folder(&client, &folder_id).await?;
    log::info!("lanzou_rm_folder: folder_id={folder_id}, ok={}", result.ok);
    Ok(result)
}

/// 重命名文件
#[tauri::command]
pub async fn lanzou_rename_file(
    state: State<'_, AppState>,
    file_id: String,
    name: String,
) -> Result<OpResult, AppError> {
    let client = state.client.lock().await;
    let result = rename_file(&client, &file_id, &name).await?;
    log::info!(
        "lanzou_rename_file: file_id={file_id}, name={name}, ok={}",
        result.ok
    );
    Ok(result)
}

/// 重命名文件夹
#[tauri::command]
pub async fn lanzou_rename_folder(
    state: State<'_, AppState>,
    folder_id: String,
    name: String,
    description: Option<String>,
) -> Result<OpResult, AppError> {
    let client = state.client.lock().await;
    let result = rename_folder(&client, &folder_id, &name, description.as_deref()).await?;
    log::info!(
        "lanzou_rename_folder: folder_id={folder_id}, name={name}, ok={}",
        result.ok
    );
    Ok(result)
}

/// 批量移动文件/文件夹（文件夹为模拟移动）
#[tauri::command]
pub async fn lanzou_move(
    state: State<'_, AppState>,
    items: Vec<MoveTarget>,
    target_id: i64,
) -> Result<OpResult, AppError> {
    let client = state.client.lock().await;
    let result = move_items(&client, items.clone(), target_id).await?;
    log::info!(
        "lanzou_move: count={}, target_id={target_id}, ok={}",
        items.len(),
        result.ok
    );
    Ok(result)
}

/// 设置文件访问权限
#[tauri::command]
pub async fn lanzou_set_file_access(
    state: State<'_, AppState>,
    file_id: String,
    shows: u32,
    shownames: String,
) -> Result<OpResult, AppError> {
    let client = state.client.lock().await;
    set_file_access(&client, &file_id, shows, &shownames).await
}

/// 设置文件夹访问权限
#[tauri::command]
pub async fn lanzou_set_folder_access(
    state: State<'_, AppState>,
    folder_id: String,
    shows: u32,
    shownames: String,
) -> Result<OpResult, AppError> {
    let client = state.client.lock().await;
    set_folder_access(&client, &folder_id, shows, &shownames).await
}

/// 文件描述详情
#[tauri::command]
pub async fn lanzou_file_description(
    state: State<'_, AppState>,
    file_id: String,
) -> Result<FileDesc, AppError> {
    let client = state.client.lock().await;
    file_description(&client, &file_id).await
}

/// 修改文件描述
#[tauri::command]
pub async fn lanzou_set_file_description(
    state: State<'_, AppState>,
    file_id: String,
    desc: String,
) -> Result<OpResult, AppError> {
    let client = state.client.lock().await;
    set_file_description(&client, &file_id, &desc).await
}

/// 文件分享信息
#[tauri::command]
pub async fn lanzou_file_detail(
    state: State<'_, AppState>,
    file_id: String,
) -> Result<ShareDetail, AppError> {
    let client = state.client.lock().await;
    file_detail(&client, &file_id).await
}

/// 文件夹分享信息
#[tauri::command]
pub async fn lanzou_folder_detail(
    state: State<'_, AppState>,
    folder_id: String,
) -> Result<ShareDetail, AppError> {
    let client = state.client.lock().await;
    folder_detail(&client, &folder_id).await
}

/// 列出回收站
#[tauri::command]
pub async fn lanzou_recycle_list(state: State<'_, AppState>) -> Result<Vec<RecycleItem>, AppError> {
    let client = state.client.lock().await;
    let items = recycle_list(&client).await?;
    log::info!("lanzou_recycle_list: items={}", items.len());
    Ok(items)
}

/// 查看回收站文件夹内的子文件
#[tauri::command]
pub async fn lanzou_recycle_files(
    state: State<'_, AppState>,
    folder_id: String,
) -> Result<Vec<RecycleFile>, AppError> {
    let client = state.client.lock().await;
    recycle_files(&client, &folder_id).await
}

/// 回收站操作（恢复/彻底删除）
#[tauri::command]
pub async fn lanzou_recycle_action(
    state: State<'_, AppState>,
    id: String,
    file_type: String,
    action: String,
) -> Result<OpResult, AppError> {
    let client = state.client.lock().await;
    recycle_action(&client, &id, &file_type, &action).await
}
