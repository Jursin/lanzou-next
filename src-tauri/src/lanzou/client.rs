use std::collections::HashMap;
use std::sync::Arc;

use reqwest::cookie::Jar;
use reqwest::{Client, ClientBuilder, Method, Response, Url};

use crate::error::AppError;

pub const DEFAULT_BASE_URL: &str = "https://up.woozooo.com";
/// 默认 User-Agent
pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36";

/// 蓝奏云响应包装。zt: 1/2 成功, 9 登录失效
#[derive(Debug, Clone, serde::Deserialize)]
pub struct LanzouResponse<T> {
    pub zt: u32,
    /// info 可能是字符串或数组，用 Value 兼容
    pub info: Option<serde_json::Value>,
    pub text: Option<T>,
}

#[derive(Debug, Clone)]
pub struct LanzouClient {
    client: Client,
    /// 账号 cookie
    cookie_map: Arc<std::sync::Mutex<HashMap<String, String>>>,
    base_url: Url,
    user_agent: String,
}

impl LanzouClient {
    pub fn new() -> Self {
        Self::with_config(DEFAULT_BASE_URL, DEFAULT_USER_AGENT)
    }

    pub fn with_config(base_url: &str, user_agent: &str) -> Self {
        let cookie_map = Arc::new(std::sync::Mutex::new(HashMap::new()));
        let client = ClientBuilder::new()
            .cookie_provider(Arc::new(Jar::default()))
            .gzip(true)
            .build()
            .expect("failed to build http client");
        Self {
            client,
            cookie_map,
            base_url: Url::parse(base_url).expect("invalid base url"),
            user_agent: user_agent.to_string(),
        }
    }

    pub fn set_base_url(&mut self, url: &str) -> Result<(), AppError> {
        self.base_url = Url::parse(url)?;
        Ok(())
    }

    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    pub fn set_user_agent(&mut self, ua: &str) {
        self.user_agent = ua.to_string();
    }

    /// 注入 cookie
    pub fn set_cookie(&self, name: &str, value: &str) {
        if let Ok(mut map) = self.cookie_map.lock() {
            map.insert(name.to_string(), value.to_string());
        }
    }

    /// 移除所有 cookie
    pub fn clear_cookies(&self) {
        if let Ok(mut map) = self.cookie_map.lock() {
            map.clear();
        }
    }

    pub fn cookie_header(&self) -> Option<String> {
        let map = self.cookie_map.lock().ok()?;
        if map.is_empty() {
            return None;
        }
        Some(
            map.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("; "),
        )
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        // 浏览器特征头
        headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
            ),
        );
        headers.insert(
            reqwest::header::ACCEPT_LANGUAGE,
            reqwest::header::HeaderValue::from_static("zh-CN,zh;q=0.9,zh-TW;q=0.8"),
        );
        headers.insert(
            reqwest::header::PRAGMA,
            reqwest::header::HeaderValue::from_static("no-cache"),
        );
        headers.insert(
            reqwest::header::CACHE_CONTROL,
            reqwest::header::HeaderValue::from_static("no-cache"),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("upgrade-insecure-requests"),
            reqwest::header::HeaderValue::from_static("1"),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("sec-fetch-dest"),
            reqwest::header::HeaderValue::from_static("empty"),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("sec-fetch-mode"),
            reqwest::header::HeaderValue::from_static("cors"),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("sec-fetch-site"),
            reqwest::header::HeaderValue::from_static("same-origin"),
        );
        // Chromium 系 UA 附带一致的 Client Hints，指纹更接近真实浏览器（Firefox/Safari 不发送）
        Self::apply_chromium_hints(&self.user_agent, &mut headers);
        if let Ok(value) = reqwest::header::HeaderValue::from_str(&self.user_agent) {
            headers.insert(reqwest::header::USER_AGENT, value);
        }
        if let Some(cookie) = self.cookie_header() {
            if let Ok(value) = reqwest::header::HeaderValue::from_str(&cookie) {
                headers.insert(reqwest::header::COOKIE, value);
            }
        }
        headers
    }

    /// 根据 UA 中的 Chrome 主版本生成匹配的 Sec-CH-UA 客户端提示头，避免版本号对不上被识别
    /// 仅 Chromium 系 UA 发送；Firefox/Safari 不发送这些头
    fn apply_chromium_hints(user_agent: &str, headers: &mut reqwest::header::HeaderMap) {
        let Some(major) = user_agent
            .split_once("Chrome/")
            .and_then(|(_, rest)| rest.split('.').next())
            .filter(|m| !m.is_empty() && m.chars().all(|c| c.is_ascii_digit()))
        else {
            return;
        };
        let insert =
            |headers: &mut reqwest::header::HeaderMap, name: &'static str, value: String| {
                if let Ok(v) = reqwest::header::HeaderValue::from_str(&value) {
                    headers.insert(reqwest::header::HeaderName::from_static(name), v);
                }
            };
        insert(
            headers,
            "sec-ch-ua",
            format!(r#""Chromium";v="{major}", "Google Chrome";v="{major}", "Not)A;Brand";v="24""#),
        );
        insert(headers, "sec-ch-ua-mobile", "?0".into());
        let platform = match std::env::consts::OS {
            "macos" => "macOS",
            "linux" => "Linux",
            _ => "Windows",
        };
        insert(headers, "sec-ch-ua-platform", format!(r#""{platform}""#));
    }

    fn url(&self, path: &str) -> Result<Url, AppError> {
        Ok(self.base_url.join(path)?)
    }

    pub async fn request(
        &self,
        method: Method,
        path: &str,
        referer: Option<&str>,
        form: Option<HashMap<String, String>>,
    ) -> Result<Response, AppError> {
        let url = self.url(path)?;
        let mut builder = self.client.request(method, url).headers(self.headers());
        if let Some(referer) = referer {
            builder = builder.header(reqwest::header::REFERER, referer);
        }
        if let Some(form) = form {
            builder = builder.form(&form);
        }
        let response = builder.send().await?;
        Ok(response)
    }

    /// 发起 multipart POST（上传文件用）
    pub async fn multipart_post(
        &self,
        path: &str,
        referer: Option<&str>,
        form: reqwest::multipart::Form,
    ) -> Result<Response, AppError> {
        let url = self.url(path)?;
        let mut builder = self
            .client
            .request(Method::POST, url)
            .headers(self.headers());
        if let Some(referer) = referer {
            builder = builder.header(reqwest::header::REFERER, referer);
        }
        let response = builder.multipart(form).send().await?;
        Ok(response)
    }

    /// 对分享域名（绝对 URL）发起请求，用于下载页解析等跨域场景
    pub async fn share_request(
        &self,
        url: &str,
        referer: Option<&str>,
    ) -> Result<Response, AppError> {
        let url = Url::parse(url)?;
        let mut builder = self
            .client
            .request(Method::GET, url)
            .headers(self.headers());
        if let Some(referer) = referer {
            builder = builder.header(reqwest::header::REFERER, referer);
        }
        let response = builder.send().await?;
        Ok(response)
    }

    /// 返回可继续定制（如 Range 头）的 GET 请求构造器
    pub fn share_request_builder(
        &self,
        url: &str,
        referer: Option<&str>,
    ) -> Result<reqwest::RequestBuilder, AppError> {
        let url = Url::parse(url)?;
        let mut builder = self
            .client
            .request(Method::GET, url)
            .headers(self.headers());
        if let Some(referer) = referer {
            builder = builder.header(reqwest::header::REFERER, referer);
        }
        Ok(builder)
    }

    /// 对分享域名发起 POST 表单请求（用于 ajaxm.php 等）
    pub async fn share_post(
        &self,
        url: &str,
        referer: Option<&str>,
        form: HashMap<String, String>,
    ) -> Result<Response, AppError> {
        let url = Url::parse(url)?;
        let mut builder = self
            .client
            .request(Method::POST, url)
            .headers(self.headers());
        if let Some(referer) = referer {
            builder = builder.header(reqwest::header::REFERER, referer);
        }
        let response = builder.form(&form).send().await?;
        Ok(response)
    }

    /// 发起分享域名 POST 请求并解析 JSON 返回原始 JSON 值（不校验 zt）
    pub async fn share_post_json(
        &self,
        url: &str,
        referer: Option<&str>,
        form: HashMap<String, String>,
    ) -> Result<serde_json::Value, AppError> {
        let response = self.share_post(url, referer, form).await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(AppError::Lanzou(format!("HTTP {}: {}", status, body)));
        }
        serde_json::from_str(&body)
            .map_err(|e| AppError::Lanzou(format!("非标准 JSON: {} ({})", e, truncate(&body))))
    }

    /// 发送请求并解析为蓝奏云标准 JSON 包装（zt 字段校验）
    pub async fn json<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        referer: Option<&str>,
        form: Option<HashMap<String, String>>,
    ) -> Result<T, AppError> {
        let response = self.request(method, path, referer, form).await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(AppError::Lanzou(format!("HTTP {}: {}", status, body)));
        }
        // 蓝奏云接口 content-type 为 text/json
        let wrapper: LanzouResponse<T> = serde_json::from_str(&body).map_err(|e| {
            AppError::Lanzou(format!("非标准 JSON 响应: {} ({})", e, truncate(&body)))
        })?;
        match wrapper.zt {
            1 | 2 => wrapper
                .text
                .ok_or_else(|| AppError::Lanzou("响应缺少 text 字段".into())),
            9 => Err(AppError::NotLoggedIn),
            _ => {
                let msg = wrapper
                    .info
                    .map(|v| {
                        v.as_str()
                            .map(String::from)
                            .unwrap_or_else(|| v.to_string())
                    })
                    .unwrap_or_else(|| "未知错误".into());
                Err(AppError::Lanzou(msg))
            }
        }
    }

    /// 发送请求并返回 info 字段（zt 校验通过后），兼容 info 承载数据的接口（如 detail/mkdir）
    pub async fn json_info(
        &self,
        method: Method,
        path: &str,
        referer: Option<&str>,
        form: Option<HashMap<String, String>>,
    ) -> Result<serde_json::Value, AppError> {
        let response = self.request(method, path, referer, form).await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(AppError::Lanzou(format!("HTTP {}: {}", status, body)));
        }
        let wrapper: LanzouResponse<serde_json::Value> =
            serde_json::from_str(&body).map_err(|e| {
                AppError::Lanzou(format!("非标准 JSON 响应: {} ({})", e, truncate(&body)))
            })?;
        match wrapper.zt {
            1 | 2 => wrapper
                .info
                .ok_or_else(|| AppError::Lanzou("响应缺少 info 字段".into())),
            9 => Err(AppError::NotLoggedIn),
            _ => {
                let msg = wrapper
                    .info
                    .map(|v| {
                        v.as_str()
                            .map(String::from)
                            .unwrap_or_else(|| v.to_string())
                    })
                    .unwrap_or_else(|| "未知错误".into());
                Err(AppError::Lanzou(msg))
            }
        }
    }

    /// 仅校验 zt，忽略响应体内容（用于删除/重命名等操作接口）
    pub async fn json_ok(
        &self,
        method: Method,
        path: &str,
        referer: Option<&str>,
        form: Option<HashMap<String, String>>,
    ) -> Result<(), AppError> {
        let response = self.request(method, path, referer, form).await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(AppError::Lanzou(format!("HTTP {}: {}", status, body)));
        }
        let wrapper: LanzouResponse<serde_json::Value> =
            serde_json::from_str(&body).map_err(|e| {
                AppError::Lanzou(format!("非标准 JSON 响应: {} ({})", e, truncate(&body)))
            })?;
        match wrapper.zt {
            1 | 2 => Ok(()),
            9 => Err(AppError::NotLoggedIn),
            _ => {
                let msg = wrapper
                    .info
                    .map(|v| {
                        v.as_str()
                            .map(String::from)
                            .unwrap_or_else(|| v.to_string())
                    })
                    .unwrap_or_else(|| "未知错误".into());
                Err(AppError::Lanzou(msg))
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

impl Default for LanzouClient {
    fn default() -> Self {
        Self::new()
    }
}
