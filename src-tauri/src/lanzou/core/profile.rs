use reqwest::Method;

use crate::error::AppError;
use crate::lanzou::client::LanzouClient;

/// 从 mypower 页面提取“单个文件大小”文本（如 "100M"、"1G"、"500M"）
pub fn parse_max_size(html: &str) -> Option<String> {
    let re = regex::Regex::new(
        r#"单个文件大小:\s*</div>\s*<div class="mf2">\s*<font[^>]*>([^<]*)</font>"#,
    )
    .ok()?;
    re.captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .filter(|s| !s.is_empty())
}

/// 解析大小字符串为字节数：支持 "100M"、"1G"、"500"、"1.5G"（无单位时视为 MB）
pub fn size_to_bytes(s: &str) -> Option<u64> {
    let s = s.trim().to_ascii_lowercase();
    let (num, unit) = s.split_at(s.len().saturating_sub(1));
    let (num, mult) = match unit {
        "k" => (num, 1024u64),
        "m" => (num, 1024u64 * 1024),
        "g" => (num, 1024u64 * 1024 * 1024),
        "t" => (num, 1024u64 * 1024 * 1024 * 1024),
        _ => (s.as_str(), 1024u64 * 1024),
    };
    let v: f64 = num.trim().parse().ok()?;
    Some((v * mult as f64) as u64)
}

/// 获取账号单个文件大小限制（字节）。未登录/请求失败时返回 None
pub async fn account_max_size(client: &LanzouClient) -> Result<Option<u64>, AppError> {
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
    Ok(parse_max_size(&html).and_then(|s| size_to_bytes(&s)))
}
