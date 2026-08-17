use std::collections::HashMap;

use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::lanzou::client::LanzouClient;
use crate::lanzou::core::ops::OpResult;

/// 回收站条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecycleItem {
    /// 文件/文件夹 id
    pub id: String,
    /// 类型：file | folder
    #[serde(rename = "type")]
    pub file_type: String,
    /// 名称
    pub name: String,
    /// 大小（回收站对文件可能不返回大小，为空字符串）
    pub size: String,
    /// 删除时间
    pub time: String,
}

fn ok() -> OpResult {
    OpResult {
        ok: true,
        message: String::new(),
    }
}

/// 回收站文件夹内的子文件（只读展示：仅名称 + 大小）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecycleFile {
    pub name: String,
    pub size: String,
}

/// 查看回收站文件夹内文件（解析 show_files 页面）
pub async fn recycle_files(
    client: &LanzouClient,
    folder_id: &str,
) -> Result<Vec<RecycleFile>, AppError> {
    let path = format!("mydisk.php?item=recycle&action=show_files&folder_id={folder_id}");
    let resp = client.request(Method::GET, &path, None, None).await?;
    let body = resp.text().await?;
    if body.contains("此文件夹没有包含文件") {
        return Ok(Vec::new());
    }
    Ok(parse_show_files(&body))
}

/// 解析 show_files 页面：<li>图标 NAME (SIZE)</li>（f14 为标题行，跳过）
fn parse_show_files(html: &str) -> Vec<RecycleFile> {
    let mut files = Vec::new();
    let li_re = regex::Regex::new(r"<li[^>]*>([\s\S]*?)</li>").expect("valid regex");
    for cap in li_re.captures_iter(html) {
        // f14 为"文件回收站:"标题行，跳过（判断整个 <li ...> 标签）
        if cap[0].contains("class=\"f14\"") {
            continue;
        }
        let li = &cap[1];
        let text = strip_tags(li);
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        // 形如 "name (size)"
        let (name, size) = if let Some(pos) = text.rfind('(') {
            if text.ends_with(')') {
                (
                    text[..pos].trim().to_string(),
                    text[pos + 1..text.len() - 1].trim().to_string(),
                )
            } else {
                (text.to_string(), String::new())
            }
        } else {
            (text.to_string(), String::new())
        };
        if name.is_empty() {
            continue;
        }
        files.push(RecycleFile { name, size });
    }
    files
}

/// 列出回收站（解析 mydisk.php?item=recycle 页面）
pub async fn recycle_list(client: &LanzouClient) -> Result<Vec<RecycleItem>, AppError> {
    let resp = client
        .request(Method::GET, "mydisk.php?item=recycle", None, None)
        .await?;
    let body = resp.text().await?;
    Ok(parse_recycle_html(&body))
}

fn parse_recycle_html(html: &str) -> Vec<RecycleItem> {
    let mut items = Vec::new();
    for row in html.split("<tr").skip(1) {
        let row = row.split("</tr>").next().unwrap_or("");
        if let Some(id) = extract_checkbox_id(row, "fd_sel_ids[]") {
            items.push(build_item(id, "folder", row));
        } else if let Some(id) = extract_checkbox_id(row, "fl_sel_ids[]") {
            items.push(build_item(id, "file", row));
        }
    }
    items
}

fn build_item(id: String, file_type: &str, row: &str) -> RecycleItem {
    let tds = extract_tds(row);
    RecycleItem {
        id,
        file_type: file_type.into(),
        name: strip_tags(tds.first().map(String::as_str).unwrap_or("")),
        size: strip_tags(tds.get(1).map(String::as_str).unwrap_or("")),
        time: strip_tags(tds.get(2).map(String::as_str).unwrap_or("")),
    }
}

/// 提取复选框 id：<input ... name="fd_sel_ids[]" ... value="123" />
fn extract_checkbox_id(row: &str, name: &str) -> Option<String> {
    let name = name.replace('[', "\\[").replace(']', "\\]");
    let re = regex::Regex::new(&format!(r#"name="{}"[^>]*value="([^"]+)""#, name)).ok()?;
    re.captures(row)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// 提取所有 <td> 的内容（去标签）
fn extract_tds(row: &str) -> Vec<String> {
    let re = regex::Regex::new(r"<\s*td[^>]*>([\s\S]*?)</\s*td>").expect("valid regex");
    re.captures_iter(row)
        .map(|c| c[1].trim().to_string())
        .collect()
}

/// 去除 HTML 标签并解码常见实体
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace('\u{00a0}', " ")
        .trim()
        .to_string()
}

/// 回收站操作（恢复/彻底删除）
/// action: "restore" | "delete"；file_type: "file" | "folder"
pub async fn recycle_action(
    client: &LanzouClient,
    id: &str,
    file_type: &str,
    action: &str,
) -> Result<OpResult, AppError> {
    let (task, id_param) = match (file_type, action) {
        ("file", "restore") => ("file_restore", "file_id"),
        ("file", "delete") => ("file_delete_complete", "file_id"),
        ("folder", "restore") => ("folder_restore", "folder_id"),
        ("folder", "delete") => ("folder_delete_complete", "folder_id"),
        _ => return Err(AppError::Lanzou("无效的回收站操作".into())),
    };

    // 1. GET 确认页，取 formhash（表单 POST 需要）
    let confirm_path = format!("mydisk.php?item=recycle&action={task}&{id_param}={id}");
    let confirm_html = client
        .request(Method::GET, &confirm_path, None, None)
        .await?
        .text()
        .await?;
    let formhash = extract_formhash(&confirm_html)
        .ok_or_else(|| AppError::Lanzou("无法获取回收站操作凭据".into()))?;

    // 2. 提交表单执行操作
    let form = HashMap::from([
        ("action".into(), task.to_string()),
        ("task".into(), task.to_string()),
        (id_param.into(), id.to_string()),
        (
            "ref".into(),
            format!("{}/mydisk.php?item=recycle&action=files", client.base_url()),
        ),
        ("formhash".into(), formhash),
    ]);
    let resp = client
        .request(
            Method::POST,
            "mydisk.php?item=recycle",
            Some(&confirm_path),
            Some(form),
        )
        .await?;
    let body = resp.text().await?;
    if body.contains("成功") {
        Ok(ok())
    } else {
        Err(AppError::Lanzou("回收站操作失败".into()))
    }
}

fn extract_formhash(html: &str) -> Option<String> {
    let re = regex::Regex::new(r#"name="formhash"\s+value="([^"]+)""#).ok()?;
    re.captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 子文件解析（show_files 页面结构）
    #[test]
    fn test_parse_show_files() {
        let html = r##"<div class="file_box"><li class="f14">文件回收站:</li><li><img src='/images/filetype/txt.gif' align='absmiddle' border='0' />&nbsp;recycle_test.txt <font color="#CCCCCC">(11.0 B)</font></li><li>&nbsp;</li></div>"##;
        let files = parse_show_files(html);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "recycle_test.txt");
        assert_eq!(files[0].size, "11.0 B");
    }
}
