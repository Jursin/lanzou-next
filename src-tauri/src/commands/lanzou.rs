use reqwest::Method;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::AppError;
use crate::lanzou::core::ls::ls;
use crate::lanzou::core::share::{ls_share, ls_share_folder, ShareFolder, ShareInfo};
use crate::lanzou::core::LsResult;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub secure: Option<bool>,
}

impl Cookie {
    pub fn new(name: &str, value: &str) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            domain: None,
            path: None,
            secure: None,
        }
    }
}

/// 账号信息（profile 解析结果）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub is_login: bool,
    /// 个性域名
    pub domain: Option<String>,
    /// 最近登录时间
    pub last_login: Option<String>,
    /// 允许上传类型
    pub support_list: Vec<String>,
    /// 单个文件大小限制
    pub max_size: Option<String>,
    /// 安全验证（手机号）
    pub verification: Option<String>,
}

/// 列出文件夹下的文件 + 目录
#[tauri::command]
pub async fn lanzou_ls(
    state: State<'_, AppState>,
    folder_id: Option<i64>,
    folder_first: Option<bool>,
) -> Result<LsResult, AppError> {
    let client = state.client.lock().await;
    let result = ls(
        &client,
        folder_id.unwrap_or(-1),
        folder_first.unwrap_or(true),
    )
    .await?;
    let file_count = result.files.iter().filter(|f| f.r#type == "file").count();
    log::info!(
        "lanzou_ls: folder_id={}, files={}, folders={}",
        folder_id.unwrap_or(-1),
        file_count,
        result.files.len() - file_count
    );
    Ok(result)
}

/// 账号信息：解析 mypower 页面
#[tauri::command]
pub async fn lanzou_profile(state: State<'_, AppState>) -> Result<Profile, AppError> {
    let client = state.client.lock().await;
    // 登录态判定：phpdisk_info / ylogin / lanzou_ifo 任一有效即视为已登录
    let has_auth_cookie = client.cookie_header().is_some_and(|h| {
        h.split(';').any(|pair| {
            let pair = pair.trim();
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            let name = name.trim();
            let value = value.trim();
            !value.is_empty()
                && (name == "phpdisk_info" || name == "ylogin" || name == "lanzou_ifo")
        })
    });
    if !has_auth_cookie {
        return Ok(Profile {
            is_login: false,
            domain: None,
            last_login: None,
            support_list: vec![],
            max_size: None,
            verification: None,
        });
    }
    let html = client
        .request(
            Method::GET,
            "mydisk.php?item=profile&action=mypower",
            None,
            None,
        )
        .await?
        .text()
        .await?;
    log::debug!("lanzou_profile: html len={}", html.len());
    let profile = parse_profile(&html);
    log::info!("lanzou_profile: is_login={}", profile.is_login);
    Ok(profile)
}

/// 从 mypower 页面解析账号信息（基于真实页面结构的正则提取）
pub fn parse_profile(html: &str) -> Profile {
    // 提取指定 id/class 元素内的文本（匹配第一个实际元素，而非 script 中的引用）
    fn element_text(html: &str, pattern: &str) -> Option<String> {
        let re = regex::Regex::new(pattern).ok()?;
        re.captures(html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
    }

    let domain =
        element_text(html, r#"<div id="domaindiynow">([^<]*)</div>"#).filter(|s| !s.is_empty());
    let last_login = element_text(
        html,
        r#"最近登录时间:\s*</div>\s*<div class="mf2">([^<]*)</div>"#,
    )
    .filter(|s| !s.is_empty());
    let verification =
        element_text(html, r#"<span id="phone_id">([^<]*)</span>"#).filter(|s| !s.is_empty());
    let max_size = crate::lanzou::core::profile::parse_max_size(html);

    // 允许上传类型
    let support_list = element_text(
        html,
        r#"允许上传类型:\s*</div>\s*<div class="mf2">([\s\S]*?)</div>"#,
    )
    .map(|s| {
        s.replace("<br>", ",")
            .split(',')
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();

    Profile {
        is_login: true,
        domain,
        last_login,
        support_list,
        max_size,
        verification,
    }
}

/// 解析分享链接（文件/文件夹）
#[tauri::command]
pub async fn lanzou_share_info(
    state: State<'_, AppState>,
    url: String,
    pwd: Option<String>,
) -> Result<ShareInfo, AppError> {
    let client = state.client.lock().await;
    ls_share(&client, &url, pwd.as_deref()).await
}

/// 解析分享文件夹中的文件列表
#[tauri::command]
pub async fn lanzou_share_folder(
    state: State<'_, AppState>,
    url: String,
    pwd: Option<String>,
) -> Result<ShareFolder, AppError> {
    let client = state.client.lock().await;
    ls_share_folder(&client, &url, pwd.as_deref()).await
}

/// 上传预检：扫描所选路径，返回超出账号单文件限制的文件清单
#[tauri::command]
pub async fn lanzou_upload_precheck(
    state: State<'_, AppState>,
    path: String,
) -> Result<crate::lanzou::core::upload::PrecheckResult, AppError> {
    let client = state.client.lock().await;
    let account_max = crate::lanzou::core::profile::account_max_size(&client)
        .await
        .ok()
        .flatten();
    let oversized =
        crate::lanzou::core::upload::scan_oversized(&std::path::PathBuf::from(&path), account_max);
    Ok(crate::lanzou::core::upload::PrecheckResult {
        max_size: account_max,
        oversized,
    })
}

/// 合并下载：多选分片文件后本地合并
#[tauri::command]
pub async fn lanzou_merge_download(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    task: crate::lanzou::core::merge::MergeDownloadTask,
) -> Result<(), AppError> {
    let client = {
        let guard = state.client.lock().await;
        guard.clone()
    };
    let cancel_flag = state.register_cancel(&task.id).await;
    let result = crate::lanzou::core::merge::merge_download(&app, &client, &task, cancel_flag)
        .await
        .map(|_| ());
    state.finish_cancel(&task.id).await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL_LIKE_HTML: &str = r##"
<html><body>
<div id="info1">
<div class="mf"><div class="mf1">个性域名: </div><div class="mf2">
<div id="domaindiynow">https://wwazk.lanzn.com</div>
<span id="domaindiyedit"><span onclick="domainedit()" class="btn">修改</span></span>
</div></div>
<div class="mf"><div class="mf1">最近登录时间: </div><div class="mf2">2026-08-13 20:42:18</div></div>
<div class="mf"><div class="mf1">手机号: </div><div class="mf2"><span id="phone_id">173****5372</span></div></div>
<div class="mf"><div class="mf1">允许上传类型: </div><div class="mf2">doc,docx,zip,rar,apk,txt,exe,7z,e,z,ct,ke,cetrainer,db,tar,pdf,w3x<br>epub,mobi,azw,azw3,osk,osz,xpa,cpk,lua,jar,dmg,ppt,pptx,xls,xlsx,mp3<br>ipa,iso,img,gho,ttf,ttc,txf,dwg,bat,imazingapp,dll,crx,xapk,conf<br>deb,rp,rpm,rplib,mobileconfig,appimage,lolgezi,flac<br>cad,hwt,accdb,ce,xmind,enc,bds,bdi,ssf,it<br>pkg,cfg,mp4,avi,png,jpeg,jpg,gif,webp,brushset</div></div>
<div class="mf"><div class="mf1">单个文件大小: </div><div class="mf2"><font color="#FF8800" size="5">
100M
</font>  <span class="txtgray"><a href="/mydisk.php?item=profile&action=huiyuan">升级</a></span></div></div>
</div>
<script>
function phonesub(){ $("#phone_id").html(phonenew); w_info(msg.info); }
</script>
</body></html>
"##;

    #[test]
    fn test_parse_profile() {
        let p = parse_profile(REAL_LIKE_HTML);
        assert!(p.is_login);
        assert_eq!(p.domain.as_deref(), Some("https://wwazk.lanzn.com"));
        assert_eq!(p.last_login.as_deref(), Some("2026-08-13 20:42:18"));
        assert_eq!(p.verification.as_deref(), Some("173****5372"));
        assert_eq!(p.max_size.as_deref(), Some("100M"));
        assert!(p.support_list.contains(&"doc".to_string()));
        assert!(p.support_list.contains(&"mp4".to_string()));
        assert!(p.support_list.contains(&"brushset".to_string()));
        // JS 里的 $("#phone_id").html() 不应污染手机号
        assert_ne!(p.verification.as_deref(), Some("phonenew"));
    }
}
