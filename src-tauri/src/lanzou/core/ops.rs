use std::collections::HashMap;

use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::lanzou::client::LanzouClient;

/// 通用响应（zt 校验由 client.json 处理）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpResult {
    pub ok: bool,
    pub message: String,
}

fn ok() -> OpResult {
    OpResult {
        ok: true,
        message: String::new(),
    }
}

/// 新建文件夹，返回新文件夹 id
pub async fn mkdir(
    client: &LanzouClient,
    parent_id: i64,
    name: &str,
    description: Option<&str>,
) -> Result<String, AppError> {
    let folder_name = name.replace([' ', '(', ')'], "_");
    let form = HashMap::from([
        ("task".into(), "2".into()),
        ("parent_id".into(), parent_id.to_string()),
        ("folder_name".into(), folder_name),
        (
            "folder_description".into(),
            description.unwrap_or("").to_string(),
        ),
    ]);
    let res: serde_json::Value = client
        .json(Method::POST, "doupload.php", None, Some(form))
        .await?;
    res.as_str()
        .map(String::from)
        .ok_or_else(|| AppError::Lanzou("创建文件夹失败".into()))
}

/// 删除文件
pub async fn rm_file(client: &LanzouClient, file_id: &str) -> Result<OpResult, AppError> {
    let form = HashMap::from([
        ("task".into(), "6".into()),
        ("file_id".into(), file_id.to_string()),
    ]);
    client
        .json_ok(Method::POST, "doupload.php", None, Some(form))
        .await?;
    Ok(ok())
}

/// 删除文件夹（蓝奏云不允许直接删除含子文件夹的目录，需递归清空子内容）
pub async fn rm_folder(client: &LanzouClient, folder_id: &str) -> Result<OpResult, AppError> {
    // 递归删除子文件与子文件夹
    let children =
        crate::lanzou::core::ls::ls(client, folder_id.parse().unwrap_or(-1), false).await?;
    for f in children.files {
        if f.r#type == "folder" {
            Box::pin(rm_folder(client, &f.id)).await?;
        } else {
            rm_file(client, &f.id).await?;
        }
    }
    // 删除文件夹本身
    let form = HashMap::from([
        ("task".into(), "3".into()),
        ("folder_id".into(), folder_id.to_string()),
    ]);
    client
        .json_ok(Method::POST, "doupload.php", None, Some(form))
        .await?;
    Ok(ok())
}

/// 重命名文件
pub async fn rename_file(
    client: &LanzouClient,
    file_id: &str,
    name: &str,
) -> Result<OpResult, AppError> {
    let form = HashMap::from([
        ("task".into(), "46".into()),
        ("type".into(), "2".into()),
        ("file_id".into(), file_id.to_string()),
        ("file_name".into(), name.to_string()),
    ]);
    client
        .json_ok(Method::POST, "doupload.php", None, Some(form))
        .await?;
    Ok(ok())
}

/// 重命名文件夹 + 描述
pub async fn rename_folder(
    client: &LanzouClient,
    folder_id: &str,
    name: &str,
    description: Option<&str>,
) -> Result<OpResult, AppError> {
    let form = HashMap::from([
        ("task".into(), "4".into()),
        ("folder_id".into(), folder_id.to_string()),
        ("folder_name".into(), name.to_string()),
        (
            "folder_description".into(),
            description.unwrap_or("").to_string(),
        ),
    ]);
    client
        .json_ok(Method::POST, "doupload.php", None, Some(form))
        .await?;
    Ok(ok())
}

/// 移动文件到文件夹
pub async fn mv_file(
    client: &LanzouClient,
    file_id: &str,
    folder_id: i64,
) -> Result<OpResult, AppError> {
    let form = HashMap::from([
        ("task".into(), "20".into()),
        ("file_id".into(), file_id.to_string()),
        ("folder_id".into(), folder_id.to_string()),
    ]);
    client
        .json_ok(Method::POST, "doupload.php", None, Some(form))
        .await?;
    Ok(ok())
}

/// 待移动项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveTarget {
    /// 文件/文件夹 id
    pub id: String,
    /// 文件/文件夹名称（文件夹模拟移动时需要）
    pub name: String,
    /// 类型：file | folder
    #[serde(rename = "type")]
    pub file_type: String,
}

/// 最大递归层级（蓝奏云目录层级限制）
const MOVE_MAX_DEPTH: u32 = 8;

/// 移动节点（预构建的被移动文件夹子树）
struct MoveNode {
    id: String,
    name: String,
    is_folder: bool,
    children: Vec<MoveNode>,
}

/// 移动文件与文件夹。
/// 蓝奏云网页端只支持移动单个文件（task=20），不支持直接移动文件夹；
/// 文件夹采用"模拟移动"：在目标下重建同名文件夹 → 递归移动子内容 → 删除原文件夹
/// 蓝奏云无原子移动：先移内容后删源，中途失败/中断会留下部分移动的中间状态，
/// 失败时错误信息会附带"部分内容可能已移动"提示（moved 计数）。
pub async fn move_items(
    client: &LanzouClient,
    items: Vec<MoveTarget>,
    target_id: i64,
) -> Result<OpResult, AppError> {
    let target_str = target_id.to_string();
    // 已成功移动的文件数（用于失败时提示部分移动）
    let moved = std::sync::atomic::AtomicU32::new(0);
    for item in items {
        if item.file_type == "folder" {
            // 预构建子树，并校验目标不在被移动文件夹自身或其子孙内（防循环）
            let tree = collect_move_tree(client, &item.id, &item.name, 0)
                .await
                .map_err(|e| {
                    partial_move_err(e, moved.load(std::sync::atomic::Ordering::SeqCst))
                })?;
            if contains_id(&tree, &target_str) {
                return Err(AppError::Lanzou("不能移动到自身或其子文件夹".into()));
            }
            move_node(client, &tree, target_id, 0, &moved)
                .await
                .map_err(|e| {
                    partial_move_err(e, moved.load(std::sync::atomic::Ordering::SeqCst))
                })?;
        } else {
            mv_file(client, &item.id, target_id).await.map_err(|e| {
                partial_move_err(e, moved.load(std::sync::atomic::Ordering::SeqCst))
            })?;
            moved.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    Ok(ok())
}

/// 若已移动过部分内容，在错误上附加提示
fn partial_move_err(e: AppError, moved: u32) -> AppError {
    if moved == 0 {
        return e;
    }
    match e {
        AppError::Http(msg) => AppError::Lanzou(format!(
            "移动中断：{msg}（部分内容可能已移动，请检查目标与源文件夹）"
        )),
        AppError::Lanzou(msg) => {
            AppError::Lanzou(format!("{msg}（部分内容可能已移动，请检查目标与源文件夹）"))
        }
        other => other,
    }
}

/// 递归收集某文件夹下的完整子树（文件为叶子节点）
async fn collect_move_tree(
    client: &LanzouClient,
    id: &str,
    name: &str,
    depth: u32,
) -> Result<MoveNode, AppError> {
    if depth >= MOVE_MAX_DEPTH {
        return Err(AppError::Lanzou("文件夹层级过深，无法移动".into()));
    }
    let list = crate::lanzou::core::ls::ls(client, id.parse().unwrap_or(-1), false)
        .await?
        .files;
    let mut children = Vec::with_capacity(list.len());
    for f in list {
        if f.r#type == "folder" {
            children.push(Box::pin(collect_move_tree(client, &f.id, &f.name, depth + 1)).await?);
        } else {
            children.push(MoveNode {
                id: f.id,
                name: f.name,
                is_folder: false,
                children: vec![],
            });
        }
    }
    Ok(MoveNode {
        id: id.to_string(),
        name: name.to_string(),
        is_folder: true,
        children,
    })
}

/// 节点或其子孙是否含指定 id
fn contains_id(node: &MoveNode, id: &str) -> bool {
    node.id == id || node.children.iter().any(|c| contains_id(c, id))
}

/// 模拟移动单个文件夹：目标下确保同名文件夹存在 → 递归移动子内容 → 删除原文件夹
async fn move_node(
    client: &LanzouClient,
    node: &MoveNode,
    target_id: i64,
    depth: u32,
    moved: &std::sync::atomic::AtomicU32,
) -> Result<(), AppError> {
    if depth >= MOVE_MAX_DEPTH {
        return Err(AppError::Lanzou("文件夹层级过深，无法移动".into()));
    }
    // 1. 在目标下确保存在同名文件夹（已存在则合并，否则新建）
    let sub_id = ensure_folder(client, target_id, &node.name).await?;
    // 2. 递归移动子内容（每移动一个文件就计数，失败时据此提示部分移动）
    for child in &node.children {
        if child.is_folder {
            Box::pin(move_node(client, child, sub_id, depth + 1, moved)).await?;
        } else {
            mv_file(client, &child.id, sub_id).await?;
            moved.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    // 3. 删除源文件夹（内容已移走）
    rm_folder(client, &node.id).await?;
    Ok(())
}

/// 在 parent_id 下确保存在名为 name 的文件夹，返回其 id
async fn ensure_folder(client: &LanzouClient, parent_id: i64, name: &str) -> Result<i64, AppError> {
    let children = crate::lanzou::core::ls::ls(client, parent_id, false).await?;
    for f in children.files {
        if f.r#type == "folder" && f.name == name {
            return f
                .id
                .parse()
                .map_err(|_| AppError::Lanzou("文件夹 id 解析失败".into()));
        }
    }
    let new_id = mkdir(client, parent_id, name, None).await?;
    new_id
        .parse()
        .map_err(|_| AppError::Lanzou("新建文件夹 id 解析失败".into()))
}

/// 设置文件访问权限（shows: 0 关闭访问密码 / 1 开启访问密码；shownames 为访问密码）
pub async fn set_file_access(
    client: &LanzouClient,
    file_id: &str,
    shows: u32,
    shownames: &str,
) -> Result<OpResult, AppError> {
    let form = HashMap::from([
        ("task".into(), "23".into()),
        ("file_id".into(), file_id.to_string()),
        ("shows".into(), shows.to_string()),
        ("shownames".into(), shownames.to_string()),
    ]);
    client
        .json_ok(Method::POST, "doupload.php", None, Some(form))
        .await?;
    Ok(ok())
}

/// 设置文件夹访问权限
pub async fn set_folder_access(
    client: &LanzouClient,
    folder_id: &str,
    shows: u32,
    shownames: &str,
) -> Result<OpResult, AppError> {
    let form = HashMap::from([
        ("task".into(), "16".into()),
        ("folder_id".into(), folder_id.to_string()),
        ("shows".into(), shows.to_string()),
        ("shownames".into(), shownames.to_string()),
    ]);
    client
        .json_ok(Method::POST, "doupload.php", None, Some(form))
        .await?;
    Ok(ok())
}

/// 文件描述详情
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDesc {
    /// 文件名
    pub name: Option<String>,
    /// 描述
    pub desc: Option<String>,
}

pub async fn file_description(client: &LanzouClient, file_id: &str) -> Result<FileDesc, AppError> {
    let form = HashMap::from([
        ("task".into(), "12".into()),
        ("file_id".into(), file_id.to_string()),
    ]);
    let res: serde_json::Value = client
        .json_info(Method::POST, "doupload.php", None, Some(form))
        .await?;
    Ok(FileDesc {
        name: res["text"].as_str().map(String::from),
        desc: res.as_str().map(String::from),
    })
}

/// 修改文件描述
pub async fn set_file_description(
    client: &LanzouClient,
    file_id: &str,
    desc: &str,
) -> Result<OpResult, AppError> {
    let form = HashMap::from([
        ("task".into(), "11".into()),
        ("file_id".into(), file_id.to_string()),
        ("desc".into(), desc.to_string()),
    ]);
    client
        .json_ok(Method::POST, "doupload.php", None, Some(form))
        .await?;
    Ok(ok())
}

/// 文件/文件夹分享信息（详情）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareDetail {
    /// 访问密码是否开启
    pub has_pwd: bool,
    /// 访问密码
    pub pwd: Option<String>,
    /// 分享链接
    pub url: Option<String>,
    /// 文件夹名
    pub name: Option<String>,
}

/// 获取文件分享信息（task=22）
pub async fn file_detail(client: &LanzouClient, file_id: &str) -> Result<ShareDetail, AppError> {
    let form = HashMap::from([
        ("task".into(), "22".into()),
        ("file_id".into(), file_id.to_string()),
    ]);
    let info = client
        .json_info(Method::POST, "doupload.php", None, Some(form))
        .await?;
    let onof = info["onof"].as_str().unwrap_or("0");
    Ok(ShareDetail {
        has_pwd: onof == "1",
        pwd: if onof == "1" {
            info["pwd"].as_str().map(String::from)
        } else {
            None
        },
        url: info["is_newd"]
            .as_str()
            .zip(info["f_id"].as_str())
            .map(|(a, b)| format!("{a}/{b}"))
            .or_else(|| info["new_url"].as_str().map(String::from)),
        name: None,
    })
}

/// 获取文件夹分享信息（task=18）
pub async fn folder_detail(
    client: &LanzouClient,
    folder_id: &str,
) -> Result<ShareDetail, AppError> {
    let form = HashMap::from([
        ("task".into(), "18".into()),
        ("folder_id".into(), folder_id.to_string()),
    ]);
    let info = client
        .json_info(Method::POST, "doupload.php", None, Some(form))
        .await?;
    let onof = info["onof"].as_str().unwrap_or("0");
    Ok(ShareDetail {
        has_pwd: onof == "1",
        pwd: if onof == "1" {
            info["pwd"].as_str().map(String::from)
        } else {
            None
        },
        url: info["new_url"].as_str().map(String::from),
        name: info["name"].as_str().map(String::from),
    })
}
