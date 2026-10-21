use std::collections::{HashMap, HashSet};

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
    /// 密码
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
            return Err(AppError::Lanzou("需要密码".into()));
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
    let error_msg = html
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
        if let Ok(re) = regex::Regex::new(pat)
            && let Some(m) = re.captures(html).and_then(|c| c.get(1))
        {
            return m.as_str().trim().to_string();
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
        if let Ok(re) = regex::Regex::new(pat)
            && let Some(m) = re.captures(html).and_then(|c| c.get(1))
        {
            return m.as_str().trim().to_string();
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

    // 带密码：sign/kdns 在页面 JS 中，ajax 地址带 file 参数
    if html.contains("id=\"passwddiv\"") || html.contains("id=\"pwdload\"") {
        let pwd = pwd.ok_or_else(|| AppError::Lanzou("需要密码".into()))?;
        let sign = matcher::match_sign(&html)?;
        let ajax_path = extract_ajaxm_path(&html, &final_url);
        let kd = matcher::match_js_var_raw(&html, "kdns").unwrap_or_else(|| "1".into());
        let mut form = HashMap::from([
            ("action".into(), "downprocess".into()),
            ("sign".into(), sign),
            ("kd".into(), kd),
            ("p".into(), pwd.to_string()),
        ]);
        // file 参数从 url 提取
        if let Some(fid) = extract_file_param(&ajax_path) {
            form.insert("file".into(), fid);
        }
        return post_ajaxm(client, &final_url, ajax_path, form).await;
    }

    // 无密码：下载参数可能在同源 iframe 页，也可能直接在分享页
    let mut down_html = html;
    let mut down_base = final_url.clone();
    if let Ok(iframe) = matcher::match_iframe(&down_html) {
        down_base = resolve_url(&base_domain(&final_url), &iframe);
        down_html = client
            .share_request(&down_base, Some(&final_url))
            .await?
            .text()
            .await?;
        down_html = matcher::remove_notes(&down_html);
    }
    let sign = matcher::match_sign(&down_html)?;
    let ajax_path = extract_ajaxm_path(&down_html, &down_base);
    let mut form = HashMap::from([
        ("action".into(), "downprocess".into()),
        ("sign".into(), sign),
    ]);
    // 页面带 ajaxdata 时走新版 websign 协议
    if let Some(key) = extract_ajaxdata(&down_html) {
        let signs = matcher::match_js_var(&down_html, "signs").unwrap_or_else(|| key.clone());
        let websign = matcher::match_js_var(&down_html, "websign")
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "2".into());
        let kd = matcher::match_js_var_raw(&down_html, "kdns").unwrap_or_else(|| "1".into());
        form.insert("websignkey".into(), key);
        form.insert("signs".into(), signs);
        form.insert("websign".into(), websign);
        form.insert("kd".into(), kd);
        form.insert("ves".into(), "1".into());
    } else {
        // 文件夹内文件：页面无 passwddiv 但下载可能需要文件夹密码
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

/// 从页面 JS 提取 ajax 下载接口（含 file 参数）
fn extract_ajaxm_path(html: &str, fallback_base: &str) -> String {
    // 绝对地址；分享页旧版为同源相对路径，不能只匹配 url : '/...'
    let abs_re =
        regex::Regex::new(r#"'((?:https?:)?//[^'\s]*(?:ajaxm|ajaxfile)\.php\?file=\d+)'"#).ok();
    if let Some(re) = abs_re
        && let Some(m) = re.captures(html).and_then(|c| c.get(1))
    {
        let v = m.as_str();
        return if v.starts_with("//") {
            format!("https:{v}")
        } else {
            v.to_string()
        };
    }
    // 同源相对路径：url : '/ajaxm.php?file='
    let re = regex::Regex::new(r#"url\s*:\s*'(/(?:ajaxm|ajaxfile)\.php\?file=\d+)'"#).ok();
    if let Some(re) = re
        && let Some(m) = re.captures(html).and_then(|c| c.get(1))
    {
        return m.as_str().to_string();
    }
    // 新版页面用 domain1/domain2 变量给出完整接口地址
    for name in ["domain1", "domain2"] {
        if let Some(v) = matcher::match_js_var(html, name)
            && v.contains("/ajax")
        {
            return v;
        }
    }
    // fallback：域名根下的 ajaxfile.php（不能拼在分享文件路径段下）
    let file = extract_file_param(html)
        .map(|f| format!("?file={f}"))
        .unwrap_or_default();
    format!(
        "{}/ajaxfile.php{file}",
        base_domain(fallback_base).trim_end_matches('/')
    )
}

/// 从 ajax 地址提取 file 参数
fn extract_file_param(path: &str) -> Option<String> {
    let re = regex::Regex::new(r"file=(\d+)").ok()?;
    re.captures(path)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// POST 下载接口（ajaxm/ajaxfile）获取直链
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
    // 下载节点还会下发反爬校验页/「验证并下载」确认页，需解开才是文件地址
    let final_url = resolve_download_node(client, &format!("{dom}/file/{url}"), referer).await?;
    Ok(DownloadUrl {
        name,
        url: final_url,
    })
}

/// 下载节点先返回 acw 校验页、再返回「验证并下载」确认页，逐级解开后得到文件地址
async fn resolve_download_node(
    client: &LanzouClient,
    jump_url: &str,
    referer: &str,
) -> Result<String, AppError> {
    // 下载节点要求携带 down_ip=1
    client.set_cookie("down_ip", "1");
    let mut url = jump_url.to_string();
    let mut challenged: HashSet<String> = HashSet::new();
    let mut confirmed: HashSet<String> = HashSet::new();
    for _ in 0..8 {
        // Range GET 探测：文件会返回二进制，校验页/确认页返回 text/html
        let probe = client
            .share_request_builder(&url, Some(referer))?
            .header(reqwest::header::RANGE, "bytes=0-0")
            .header(reqwest::header::ACCEPT_ENCODING, "identity")
            .send()
            .await?;
        let status = probe.status();
        // 跟随重定向后的地址
        let current = probe.url().to_string();
        let content_type = probe
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        if matches!(status.as_u16(), 403 | 404 | 410) {
            return Err(AppError::Lanzou("下载直链已失效".into()));
        }
        if status.is_success() && !content_type.contains("text/html") {
            // 丢弃探测用的 1 字节，保持连接可复用
            if status == reqwest::StatusCode::PARTIAL_CONTENT {
                let _ = probe.bytes().await;
            }
            return Ok(current);
        }
        drop(probe);
        let body = client
            .share_request(&current, Some(referer))
            .await?
            .text()
            .await?;
        let host = base_domain(&current);
        if let Some(cookie) = matcher::calc_acw_sc_v2(&body) {
            if !challenged.insert(host) {
                return Err(AppError::Lanzou("下载节点校验失败".into()));
            }
            client.set_cookie("acw_sc__v2", &cookie);
            url = current;
            continue;
        }
        if body.contains("验证并下载") {
            if !confirmed.insert(host) {
                return Err(AppError::Lanzou("下载节点需要人工验证".into()));
            }
            url = submit_download_verify(client, &current, &body).await?;
            continue;
        }
        return Err(AppError::Lanzou("下载节点返回未知页面".into()));
    }
    Err(AppError::Lanzou("下载节点解析失败".into()))
}

/// 提交「验证并下载」确认页，返回新的直链
async fn submit_download_verify(
    client: &LanzouClient,
    page_url: &str,
    html: &str,
) -> Result<String, AppError> {
    let (file, sign) = match_verify_params(html)
        .ok_or_else(|| AppError::Lanzou("下载验证页缺少参数".into()))?;
    // 服务端要求页面加载约 2 秒后才接受提交，过早提交返回 ?SignError
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
    let ajax_url = format!("{}/ajax.php", base_domain(page_url));
    let value = client
        .share_post_json(
            &ajax_url,
            Some(page_url),
            HashMap::from([
                ("file".into(), file),
                ("el".into(), "2".into()),
                ("sign".into(), sign),
            ]),
        )
        .await?;
    let url = value["url"].as_str().unwrap_or_default();
    if !url.starts_with("http") {
        return Err(AppError::Lanzou("下载验证失败，请稍后重试".into()));
    }
    Ok(url.to_string())
}

/// 验证页 ajax data 中的 file 与 sign，形如 data : { 'file':'...','el':el,'sign':'...' }
fn match_verify_params(html: &str) -> Option<(String, String)> {
    let block_re = regex::Regex::new(r"data\s*:\s*\{([^{}]*)\}").ok()?;
    block_re.captures_iter(html).find_map(|c| {
        let block = c.get(1)?.as_str();
        Some((match_data_key(block, "file")?, match_data_key(block, "sign")?))
    })
}

/// 读取 data 块中的字符串参数
fn match_data_key(block: &str, key: &str) -> Option<String> {
    let re = regex::Regex::new(&format!(
        r#"['"]?{}['"]?\s*:\s*['"]([^'"]+)['"]"#,
        regex::escape(key)
    ))
    .ok()?;
    re.captures(block)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// 解析相对路径为绝对 URL
pub(crate) fn resolve_url(base: &str, path: &str) -> String {
    if path.starts_with("//") {
        format!("https:{path}")
    } else if path.starts_with("http") {
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
                    // id 是相对分享链接，需按域名根解析（不能拼上文件夹路径段）
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
            3 => return Err(AppError::Lanzou("密码错误".into())),
            _ => break,
        }
    }

    Ok(ShareFolder {
        name,
        size: String::new(),
        list,
    })
}
