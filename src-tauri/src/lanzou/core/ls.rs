use std::collections::HashMap;

use reqwest::Method;

use crate::error::AppError;
use crate::lanzou::client::LanzouClient;
use crate::lanzou::core::{CrumbsInfo, LsFile, LsResult};

/// 列出文件夹下的所有文件 + 目录
pub async fn ls(
    client: &LanzouClient,
    folder_id: i64,
    folder_first: bool,
) -> Result<LsResult, AppError> {
    let (dirs, files) = tokio::join!(ls_dir(client, folder_id), ls_file(client, folder_id));
    let dirs = dirs?;
    let files = files?;

    let mut folders: Vec<LsFile> = dirs
        .items
        .iter()
        .map(|v| LsFile {
            name: v.name.clone(),
            id: v.fol_id.to_string(),
            r#type: "folder".into(),
            icon: None,
            size: None,
            time: None,
            downs: None,
        })
        .collect();

    let mut files: Vec<LsFile> = files
        .iter()
        .map(|v| LsFile {
            name: v.name_all.clone(),
            id: v.id.to_string(),
            r#type: "file".into(),
            icon: v.icon.clone(),
            size: v.size.clone(),
            time: v.time.clone(),
            downs: v.downs.clone(),
        })
        .collect();

    if folder_first {
        folders.append(&mut files);
        files = folders;
    } else {
        files.append(&mut folders);
    }

    // 面包屑：根目录显示"根目录"，子目录用 API 返回的 info 路径
    let mut info = vec![CrumbsInfo {
        id: "-1".into(),
        name: "根目录".into(),
    }];
    info.extend(dirs.crumbs);

    Ok(LsResult { info, files })
}

#[derive(Debug, serde::Deserialize)]
struct LsDirItem {
    name: String,
    fol_id: String,
}

struct LsDirResult {
    items: Vec<LsDirItem>,
    crumbs: Vec<CrumbsInfo>,
}

/// 列出该文件夹下的所有文件夹 + 面包屑
async fn ls_dir(client: &LanzouClient, folder_id: i64) -> Result<LsDirResult, AppError> {
    let form = HashMap::from([
        ("task".into(), "47".into()),
        ("folder_id".into(), folder_id.to_string()),
    ]);
    // info 是面包屑路径（子目录时非空），text 是文件夹列表
    let body = client
        .request(Method::POST, "doupload.php", None, Some(form))
        .await?
        .text()
        .await?;
    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| AppError::Lanzou(format!("非标准 JSON: {e} ({})", truncate(&body))))?;
    Ok(LsDirResult {
        items: serde_json::from_value(value["text"].clone()).unwrap_or_default(),
        crumbs: value["info"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        // folderid 可能为数字也可能为字符串，两种都要兼容，否则整段路径会被丢弃
                        let id = v["folderid"]
                            .as_i64()
                            .map(|n| n.to_string())
                            .or_else(|| v["folderid"].as_str().map(String::from))?;
                        let name = v["name"].as_str()?.to_string();
                        Some(CrumbsInfo { id, name })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn truncate(s: &str) -> String {
    if s.len() > 200 {
        format!("{}...", &s[..200])
    } else {
        s.to_string()
    }
}

#[derive(Debug, serde::Deserialize)]
struct LsFileItem {
    id: String,
    name_all: String,
    icon: Option<String>,
    size: Option<String>,
    time: Option<String>,
    downs: Option<String>,
}

/// 列出文件夹下所有文件（自动分页）
async fn ls_file(client: &LanzouClient, folder_id: i64) -> Result<Vec<LsFileItem>, AppError> {
    let mut pg = 1;
    let mut file_list: Vec<LsFileItem> = vec![];
    loop {
        let form = HashMap::from([
            ("task".into(), "5".into()),
            ("folder_id".into(), folder_id.to_string()),
            ("pg".into(), pg.to_string()),
            ("vei".into(), String::new()),
        ]);
        let res: Option<Vec<LsFileItem>> = client
            .json(Method::POST, "doupload.php", None, Some(form))
            .await?;
        let has_more = res.as_ref().is_some_and(|t| t.len() >= 18);
        if let Some(text) = res {
            file_list.extend(text);
        }
        if !has_more {
            break;
        }
        pg += 1;
    }
    Ok(file_list)
}
