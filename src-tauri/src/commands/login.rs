use std::collections::HashMap;

use reqwest::redirect::Policy;
use reqwest::{Client, Method};
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::config;
use crate::commands::lanzou::Cookie;
use crate::error::AppError;
use crate::lanzou::client::DEFAULT_USER_AGENT;
use crate::lanzou::matcher;
use crate::state::AppState;

/// 蓝奏云真实登录页（up.woozooo.com 的登录页会 JS 重定向到这里）
pub const LOGIN_URL: &str =
    "https://accounts.woozooo.com/accounts.php?action=login&ref=up.woozooo.com";

/// 账号密码直接登录（Rust 侧完成 acw 挑战 + uselogin + 中转 cookie 链）
#[tauri::command]
pub async fn lanzou_login(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    username: String,
    password: String,
) -> Result<crate::commands::lanzou::Profile, AppError> {
    log::info!("lanzou_login: 用户 {username} 开始登录");
    // 使用独立 client 完成登录流程，避免污染共享 client
    // 禁用自动重定向，手动跟进以收集每一步 Set-Cookie
    let client = Client::builder()
        .redirect(Policy::none())
        .gzip(true)
        .build()
        .map_err(|e| AppError::Config(e.to_string()))?;

    let mut session = LoginSession {
        client: &client,
        cookies: HashMap::new(),
        referer: None,
    };

    // 1. 获取登录页，解决 acw_sc__v2 挑战
    let (html, _) = session.get(LOGIN_URL).await?;
    if html.contains("arg1=")
        && let Some(acw) = matcher::calc_acw_sc_v2(&html)
    {
        session.set_cookie("acw_sc__v2", &acw);
        let _ = session.get(LOGIN_URL).await?;
    }

    // 2. POST uselogin 登录
    let form = HashMap::from([
        ("task".into(), "uselogin".into()),
        ("username".into(), username),
        ("password".into(), password),
        ("ref".into(), "up.woozooo.com".into()),
    ]);
    let login_json = session
        .post_json("https://accounts.woozooo.com/accounts.php", form)
        .await?;
    let zt = login_json["zt"].as_i64().unwrap_or(0);
    if zt != 1 {
        let msg = login_json["msgs"].as_str().unwrap_or("登录失败");
        log::warn!("lanzou_login: 登录失败 - {msg}");
        return Err(AppError::Lanzou(msg.to_string()));
    }
    let redirect = login_json["msgs"].as_str().unwrap_or_default().to_string();

    // 3. 跟随中转鉴权跳转链，收集所有 Set-Cookie（含 httpOnly 的 phpdisk_info）
    let mut next: Option<String> = if redirect.is_empty() {
        None
    } else {
        Some(redirect)
    };
    let mut hops = 0;
    while let Some(url) = next.take() {
        hops += 1;
        if hops > 10 {
            break;
        }
        log::debug!("lanzou_login: 中转跳转 #{hops} -> {url}");
        match session.get(&url).await {
            Ok((body, location)) => {
                if let Some(loc) = location {
                    next = Some(if loc.starts_with("http") {
                        loc
                    } else {
                        resolve_url(&url, &loc)
                    });
                } else {
                    collect_js_redirects(&body, &mut next);
                }
            }
            Err(_) => break,
        }
    }
    log::debug!(
        "lanzou_login: 捕获 cookie: {:?}",
        session.cookies.keys().collect::<Vec<_>>()
    );

    // 4. 将收集到的 cookie 注入共享 client 并持久化
    let collected: Vec<Cookie> = session
        .cookies
        .iter()
        .map(|(k, v)| Cookie::new(k, v))
        .collect();
    {
        let mut client = state.client.lock().await;
        client.set_user_agent(DEFAULT_USER_AGENT);
        for c in &collected {
            client.set_cookie(&c.name, &c.value);
        }
        let _ = client.set_base_url("https://up.woozooo.com");
    }

    config::config_set(
        app.clone(),
        config::AppConfig {
            domain: Some("https://up.woozooo.com".into()),
            user_agent: Some(DEFAULT_USER_AGENT.into()),
            cookies: Some(collected),
            ..Default::default()
        },
    )
    .await?;

    log::info!("lanzou_login: 登录成功");
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.emit("login:success", ());
    }

    // 返回 profile
    let client = state.client.lock().await;
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
    Ok(crate::commands::lanzou::parse_profile(&html))
}

/// 登录会话：维护 cookie + referer，不跟随重定向
struct LoginSession<'a> {
    client: &'a Client,
    cookies: HashMap<String, String>,
    referer: Option<String>,
}

impl LoginSession<'_> {
    fn cookie_header(&self) -> Option<String> {
        if self.cookies.is_empty() {
            return None;
        }
        Some(
            self.cookies
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("; "),
        )
    }

    fn set_cookie(&mut self, name: &str, value: &str) {
        self.cookies.insert(name.to_string(), value.to_string());
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut h = reqwest::header::HeaderMap::new();
        h.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static(DEFAULT_USER_AGENT),
        );
        h.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
            ),
        );
        h.insert(
            reqwest::header::ACCEPT_LANGUAGE,
            reqwest::header::HeaderValue::from_static("zh-CN,zh;q=0.9,zh-TW;q=0.8"),
        );
        h.insert(
            reqwest::header::PRAGMA,
            reqwest::header::HeaderValue::from_static("no-cache"),
        );
        h.insert(
            reqwest::header::CACHE_CONTROL,
            reqwest::header::HeaderValue::from_static("no-cache"),
        );
        h.insert(
            reqwest::header::HeaderName::from_static("upgrade-insecure-requests"),
            reqwest::header::HeaderValue::from_static("1"),
        );
        h.insert(
            reqwest::header::HeaderName::from_static("sec-fetch-dest"),
            reqwest::header::HeaderValue::from_static("document"),
        );
        h.insert(
            reqwest::header::HeaderName::from_static("sec-fetch-mode"),
            reqwest::header::HeaderValue::from_static("navigate"),
        );
        h.insert(
            reqwest::header::HeaderName::from_static("sec-fetch-site"),
            reqwest::header::HeaderValue::from_static("same-origin"),
        );
        // Chromium Client Hints
        Self::apply_chromium_hints(&mut h);
        if let Some(c) = self.cookie_header()
            && let Ok(v) = reqwest::header::HeaderValue::from_str(&c)
        {
            h.insert(reqwest::header::COOKIE, v);
        }
        if let Some(r) = &self.referer
            && let Ok(v) = reqwest::header::HeaderValue::from_str(r)
        {
            h.insert(reqwest::header::REFERER, v);
        }
        h
    }

    fn apply_chromium_hints(headers: &mut reqwest::header::HeaderMap) {
        let insert =
            |headers: &mut reqwest::header::HeaderMap, name: &'static str, value: String| {
                if let Ok(v) = reqwest::header::HeaderValue::from_str(&value) {
                    headers.insert(reqwest::header::HeaderName::from_static(name), v);
                }
            };
        insert(
            headers,
            "sec-ch-ua",
            r#""Chromium";v="147", "Google Chrome";v="147", "Not)A;Brand";v="24""#.into(),
        );
        insert(headers, "sec-ch-ua-mobile", "?0".into());
        insert(headers, "sec-ch-ua-platform", r#""Windows""#.into());
    }

    async fn get(&mut self, url: &str) -> Result<(String, Option<String>), AppError> {
        let resp = self.client.get(url).headers(self.headers()).send().await?;
        self.capture_cookies(&resp);
        let status = resp.status();
        let location = resp
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        if status.is_redirection() && location.is_some() {
            return Ok((String::new(), location));
        }
        self.referer = Some(url.to_string());
        Ok((resp.text().await?, None))
    }

    async fn post_json(
        &mut self,
        url: &str,
        form: HashMap<String, String>,
    ) -> Result<serde_json::Value, AppError> {
        self.referer = Some(url.to_string());
        let resp = self
            .client
            .post(url)
            .headers(self.headers())
            .form(&form)
            .send()
            .await?;
        self.capture_cookies(&resp);
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(AppError::Lanzou(format!("HTTP {status}: {body}")));
        }
        serde_json::from_str(&body)
            .map_err(|e| AppError::Lanzou(format!("非标准 JSON: {e} ({})", truncate(&body))))
    }

    fn capture_cookies(&mut self, resp: &reqwest::Response) {
        for value in resp.headers().get_all(reqwest::header::SET_COOKIE) {
            if let Ok(s) = value.to_str() {
                let (name, rest) = match s.split_once('=') {
                    Some((n, r)) => (n.trim(), r),
                    None => continue,
                };
                let value = rest.split(';').next().unwrap_or("").trim().to_string();
                self.set_cookie(name, &value);
            }
        }
    }
}

fn truncate(s: &str) -> String {
    if s.len() > 200 {
        format!("{}...", &s[..200])
    } else {
        s.to_string()
    }
}

/// 从 HTML 中提取 JS 跳转（document.location / location.href / iframe）
fn collect_js_redirects(html: &str, next: &mut Option<String>) {
    if next.is_some() {
        return;
    }
    let patterns = [
        r"document\.location\.href\s*=\s*'([^']+)'",
        r"location\.href\s*=\s*'([^']+)'",
        r"window\.location\s*=\s*'([^']+)'",
        r#"<iframe[^>]*src=['"]([^'"]+)['"]"#,
    ];
    for pat in patterns {
        if let Ok(re) = regex::Regex::new(pat)
            && let Some(m) = re.captures(html).and_then(|c| c.get(1))
        {
            *next = Some(m.as_str().to_string());
            return;
        }
    }
}

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

/// 退出登录：清除 cookies 并持久化
#[tauri::command]
pub async fn lanzou_logout(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    log::info!("lanzou_logout: 用户退出登录");
    {
        let client = state.client.lock().await;
        client.clear_cookies();
    }
    config::config_set(
        app,
        config::AppConfig {
            cookies: Some(vec![]),
            ..Default::default()
        },
    )
    .await?;
    Ok(())
}
