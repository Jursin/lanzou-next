use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::lanzou::client::LanzouClient;
use crate::lanzou::matcher;

/// 下载直链解析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadUrl {
    /// 文件名
    pub name: String,
    /// 直链
    pub url: String,
}

/// 分享链接类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShareType {
    File,
    Folder,
}

/// 分享解析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareInfo {
    pub r#type: ShareType,
    /// 文件名或文件夹名
    pub name: String,
    /// 提取码
    pub pwd: Option<String>,
}

/// 获取文件分享页并处理反爬 acw_sc__v2 挑战
async fn get_share_page(client: &LanzouClient, url: &str) -> Result<(String, String), AppError> {
    let mut response = client.share_request(url, None).await?;
    let mut final_url = response.url().to_string();
    let mut html = response.text().await?;

    if html.contains("acw_sc__v2") {
        // 页面被反爬 JS 挑战拦截，计算 acw_sc__v2 cookie 后重试
        if let Some(cookie) = matcher::calc_acw_sc_v2(&html) {
            client.set_cookie("acw_sc__v2", &cookie);
            response = client.share_request(url, None).await?;
            final_url = response.url().to_string();
            html = response.text().await?;
        }
    }
    Ok((final_url, html))
}

/// 判断分享链接类型（文件/文件夹）
pub async fn ls_share(
    client: &LanzouClient,
    url: &str,
    pwd: Option<&str>,
) -> Result<ShareInfo, AppError> {
    let (_, html) = get_share_page(client, url).await?;
    let html = matcher::remove_notes(&html);

    let is_file = html.contains("<iframe")
        || html.contains("id=\"passwddiv\"")
        || html.contains("id=\"downajax\"");
    let is_pwd_folder = html.contains("id=\"pwdload\"");
    let is_folder = html.contains("id=\"filemore\"") || html.contains("filemoreajax");

    if is_file {
        // 文件：无密码（iframe）或有密码（passwddiv）
        if html.contains("id=\"passwddiv\"") && pwd.is_none() {
            return Err(AppError::Lanzou("需要提取码".into()));
        }
        let name = extract_file_name(&html);
        return Ok(ShareInfo {
            r#type: ShareType::File,
            name,
            pwd: pwd.map(String::from),
        });
    }
    if is_folder || is_pwd_folder {
        let name = extract_folder_name(&html);
        return Ok(ShareInfo {
            r#type: ShareType::Folder,
            name,
            pwd: pwd.map(String::from),
        });
    }
    // 解析错误
    let cleaned = &html;
    let error_msg = cleaned
        .lines()
        .find(|l| l.contains("该文件已删除") || l.contains("操作失败") || l.contains("页面不存在"))
        .map(String::from)
        .unwrap_or_else(|| "解析失败".to_string());
    Err(AppError::Lanzou(error_msg))
}

fn extract_file_name(html: &str) -> String {
    let patterns = [
        r"<title>([^<]+?)\s*-\s*蓝奏云</title>",
        r#"var filename\s*=\s*'([^']+)'"#,
        r#"<div class="filethetext"[^>]*>([^<]+?)</div>"#,
        r#"id="filenajax">([^<]+?)</div>"#,
    ];
    for pat in patterns {
        if let Ok(re) = regex::Regex::new(pat) {
            if let Some(m) = re.captures(html).and_then(|c| c.get(1)) {
                return m.as_str().trim().to_string();
            }
        }
    }
    "未匹配到文件名".into()
}

fn extract_folder_name(html: &str) -> String {
    let patterns = [
        r#"<title>([^<]+?)\s*-\s*蓝奏云</title>"#,
        r#"<div class="user-title">([^<]+?)</div>"#,
        r#"var\s+.+?\s*=\s*'(.+?)';\n.+document\.title"#,
    ];
    for pat in patterns {
        if let Ok(re) = regex::Regex::new(pat) {
            if let Some(m) = re.captures(html).and_then(|c| c.get(1)) {
                return m.as_str().trim().to_string();
            }
        }
    }
    "未匹配到文件夹名".into()
}

/// 解析文件下载直链
pub async fn file_download_url(
    client: &LanzouClient,
    url: &str,
    pwd: Option<&str>,
) -> Result<DownloadUrl, AppError> {
    let (final_url, html) = get_share_page(client, url).await?;
    let html = matcher::remove_notes(&html);

    // 带提取码的文件：passwddiv，sign 在页面 JS 里，ajaxm url 带 file 参数
    if html.contains("id=\"passwddiv\"") || html.contains("id=\"pwdload\"") {
        let pwd = pwd.ok_or_else(|| AppError::Lanzou("需要提取码".into()))?;
        let sign = matcher::match_sign(&html)?;
        // 提取 ajaxm url（含 file 参数），如 /ajaxm.php?file=277944670
        let ajax_path = extract_ajaxm_path(&html, &final_url);
        let mut form = HashMap::from([
            ("action".into(), "downprocess".into()),
            ("sign".into(), sign),
            ("kd".into(), "1".into()),
            ("p".into(), pwd.to_string()),
        ]);
        // file 参数从 url 提取
        if let Some(fid) = extract_file_param(&ajax_path) {
            form.insert("file".into(), fid);
        }
        return post_ajaxm(client, &final_url, ajax_path, form).await;
    }

    // 无提取码：iframe → 下载页(fn) → sign → ajaxm
    let iframe = matcher::match_iframe(&html)?;
    let iframe_url = resolve_url(&base_domain(&final_url), &iframe);
    let down_html = client
        .share_request(&iframe_url, Some(&final_url))
        .await?
        .text()
        .await?;
    let down_html = matcher::remove_notes(&down_html);
    let sign = matcher::match_sign(&down_html)?;
    let ajax_path = extract_ajaxm_path(&down_html, &final_url);
    let mut form = HashMap::from([
        ("action".into(), "downprocess".into()),
        ("sign".into(), sign),
    ]);
    // 新版下载页(fn)带 ajaxdata → 走 websign 流程；否则回退旧格式(p/file)
    if let Some(ws) = extract_ajaxdata(&down_html) {
        form.insert("websignkey".into(), ws.clone());
        form.insert("signs".into(), ws);
        form.insert("websign".into(), "2".into());
        form.insert("kd".into(), "1".into());
        form.insert("ves".into(), "1".into());
    } else {
        // 文件夹内文件：页面无 passwddiv 但下载可能需要文件夹提取码
        form.insert("p".into(), pwd.unwrap_or("").to_string());
        if let Some(fid) = extract_file_param(&ajax_path) {
            form.insert("file".into(), fid);
        }
    }
    post_ajaxm(client, &final_url, ajax_path, form).await
}

/// 新版下载页的 websignkey（fn 页 var ajaxdata = '...'）
fn extract_ajaxdata(html: &str) -> Option<String> {
    let re = regex::Regex::new(r#"var\s+ajaxdata\s*=\s*'([^']+)'"#).ok()?;
    re.captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// 从页面 JS 提取 ajax 下载接口路径（含 file 参数），如 /ajaxfile.php?file=277944670
/// 蓝奏云接口经历过 ajaxm.php -> ajaxfile.php 变更，两者都兼容匹配
fn extract_ajaxm_path(html: &str, fallback_base: &str) -> String {
    let re = regex::Regex::new(r#"url\s*:\s*'/(?:ajaxm|ajaxfile)\.php\?file=\d+'"#).ok();
    if let Some(re) = re {
        if let Some(m) = re.captures(html).and_then(|c| c.get(0)) {
            let v = m.as_str().to_string();
            // 取单引号内的路径部分
            let path = v
                .trim_start_matches("url")
                .trim()
                .trim_start_matches(':')
                .trim();
            return path.trim_matches('\'').to_string();
        }
    }
    // fallback：域名根下的 ajaxfile.php（不能拼在分享文件路径段下）
    format!(
        "{}/ajaxfile.php",
        base_domain(fallback_base).trim_end_matches('/')
    )
}

/// 从 ajaxm path 提取 file 参数
fn extract_file_param(path: &str) -> Option<String> {
    let re = regex::Regex::new(r"file=(\d+)").ok()?;
    re.captures(path)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// POST ajaxm.php 获取直链
async fn post_ajaxm(
    client: &LanzouClient,
    referer: &str,
    ajax_path: String,
    form: HashMap<String, String>,
) -> Result<DownloadUrl, AppError> {
    let base = base_domain(referer);
    let ajax_url = if ajax_path.starts_with('/') {
        format!("{base}{ajax_path}")
    } else {
        resolve_url(referer, &ajax_path)
    };
    let value = client
        .share_post_json(&ajax_url, Some(referer), form)
        .await?;

    let dom = value["dom"].as_str().unwrap_or_default().to_string();
    let url = value["url"].as_str().unwrap_or_default().to_string();
    let name = value["inf"].as_str().unwrap_or_default().to_string();
    if dom.is_empty() || url.is_empty() {
        let msg = value["info"].as_str().unwrap_or("解析直链失败");
        return Err(AppError::Lanzou(msg.to_string()));
    }
    Ok(DownloadUrl {
        name,
        url: format!("{dom}/file/{url}"),
    })
}

/// 解析相对路径为绝对 URL
fn resolve_url(base: &str, path: &str) -> String {
    if path.starts_with("http") {
        path.to_string()
    } else {
        let base = base.trim_end_matches('/');
        format!(
            "{base}{}",
            if path.starts_with('/') {
                path.to_string()
            } else {
                format!("/{path}")
            }
        )
    }
}

/// 从 URL 提取协议+域名
fn base_domain(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(u) => {
            let scheme = u.scheme();
            let host = u.host_str().unwrap_or_default();
            format!("{scheme}://{host}")
        }
        Err(_) => url.to_string(),
    }
}

/// 分享文件夹里的文件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareFile {
    /// 文件名
    pub name: String,
    /// 文件大小（人类可读）
    pub size: String,
    /// 上传时间
    pub time: String,
    /// 分享 URL（当前页面下的相对链接）
    pub url: String,
}

/// 分享文件夹解析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareFolder {
    /// 文件夹名
    pub name: String,
    /// 总大小（人类可读）
    pub size: String,
    pub list: Vec<ShareFile>,
}

/// 解析分享文件夹中的文件列表（filemoreajax.php 分页）
pub async fn ls_share_folder(
    client: &LanzouClient,
    url: &str,
    pwd: Option<&str>,
) -> Result<ShareFolder, AppError> {
    let (final_url, html) = get_share_page(client, url).await?;
    let html = matcher::remove_notes(&html);

    let name = extract_folder_name(&html);

    // 提取 filemoreajax 参数
    let params = matcher::match_folder_ajax(&html)?;
    let base = base_domain(&final_url);
    let ajax_url = format!("{base}/filemoreajax.php");

    let mut pg = 1;
    let mut list: Vec<ShareFile> = Vec::new();
    loop {
        let mut form = HashMap::from([
            ("lx".into(), params.lx.clone()),
            ("pg".into(), pg.to_string()),
            ("k".into(), params.k.clone()),
            ("t".into(), params.t.clone()),
            ("fid".into(), params.fid.clone()),
        ]);
        if let Some(p) = pwd {
            form.insert("pwd".into(), p.to_string());
        }
        let value = client
            .share_post_json(&ajax_url, Some(&final_url), form)
            .await?;
        let zt = value["zt"].as_i64().unwrap_or(0);
        match zt {
            1 => {
                let text = value["text"].as_array().cloned().unwrap_or_default();
                let page_has_more = text.len() >= 50;
                for f in text {
                    // id 是相对分享链接（如 iPhPB3kgag9a?webpage=...），需按域名根解析为完整 URL；
                    // 不能基于文件夹页 URL 拼接（会带上文件夹路径段，导致访问不到文件页）
                    let rel = f["id"].as_str().unwrap_or("").to_string();
                    list.push(ShareFile {
                        name: f["name_all"].as_str().unwrap_or("").to_string(),
                        size: f["size"].as_str().unwrap_or("").to_string(),
                        time: f["time"].as_str().unwrap_or("").to_string(),
                        url: resolve_url(&base_domain(&final_url), &rel),
                    });
                }
                if page_has_more {
                    pg += 1;
                    continue;
                }
                break;
            }
            3 => return Err(AppError::Lanzou("提取码错误".into())),
            _ => break,
        }
    }

    Ok(ShareFolder {
        name,
        size: String::new(),
        list,
    })
}
