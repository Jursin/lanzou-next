use crate::error::AppError;

/// 蓝奏云反爬 JS 挑战：计算 acw_sc__v2 cookie 值
/// 参考 https://github.com/zaxtyson/LanZouCloud-API
pub fn calc_acw_sc_v2(html: &str) -> Option<String> {
    let arg1 = regex_arg1(html)?;
    Some(hex_xor(
        &unsbox(&arg1),
        "3000176000856006061501533003690027800375",
    ))
}

fn regex_arg1(html: &str) -> Option<String> {
    let start = html.find("arg1='")? + "arg1='".len();
    let end = html[start..].find('\'')? + start;
    let v: String = html[start..end]
        .chars()
        .filter(char::is_ascii_hexdigit)
        .collect();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

const UNSBOX_ORDER: [usize; 40] = [
    15, 35, 29, 24, 33, 16, 1, 38, 10, 9, 19, 31, 40, 27, 22, 23, 25, 13, 6, 11, 39, 18, 20, 8, 14,
    21, 32, 26, 2, 30, 7, 4, 17, 5, 3, 28, 34, 37, 12, 36,
];

/// 重排字符串字符（unsbox）
fn unsbox(input: &str) -> String {
    let mut out = vec![' '; UNSBOX_ORDER.len()];
    for (idx, ch) in input.chars().enumerate() {
        for (pos, &v) in UNSBOX_ORDER.iter().enumerate() {
            if v == idx + 1 {
                out[pos] = ch;
                break;
            }
        }
    }
    out.iter().collect()
}

/// 十六进制异或
fn hex_xor(input: &str, key: &str) -> String {
    let mut res = String::new();
    let n = input.len().min(key.len());
    let mut i = 0;
    while i + 1 < n {
        let a = u8::from_str_radix(&input[i..i + 2], 16).unwrap_or(0);
        let b = u8::from_str_radix(&key[i..i + 2], 16).unwrap_or(0);
        res.push_str(&format!("{:02x}", a ^ b));
        i += 2;
    }
    res
}

/// 提取分享页 iframe src
pub fn match_iframe(html: &str) -> Result<String, AppError> {
    let pat = r#"<iframe[^>]*src=["']([^"']+)["']"#;
    let re = regex::Regex::new(pat).map_err(|e| AppError::Parse(e.to_string()))?;
    re.captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| AppError::Parse("未找到 iframe".into()))
}

/// 提取下载页 JS 中的 sign 参数
/// 形如: 'sign':'actual_value'  或  'sign':variable,
pub fn match_sign(html: &str) -> Result<String, AppError> {
    // 先移除 /* */ 块注释，避免匹配到被注释掉的旧 ajax 参数
    let re_comment =
        regex::Regex::new(r"/\*[\s\S]*?\*/").map_err(|e| AppError::Parse(e.to_string()))?;
    let clean = re_comment.replace_all(html, "");

    // 优先匹配带引号的直接值
    let quoted_re =
        regex::Regex::new(r"'sign'\s*:\s*'([^']+)'").map_err(|e| AppError::Parse(e.to_string()))?;
    if let Some(m) = quoted_re.captures(&clean).and_then(|c| c.get(1)) {
        return Ok(m.as_str().to_string());
    }

    let sign_re = regex::Regex::new(r"'sign'\s*:\s*([0-9a-zA-Z_]+)")
        .map_err(|e| AppError::Parse(e.to_string()))?;
    let cap = sign_re
        .captures(&clean)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| AppError::Parse("未找到 sign".into()))?;

    // sign 是变量引用，需解析其值
    if cap.len() < 20 {
        let var_re = regex::Regex::new(&format!(r"var\s+{}\s*=\s*'([^']+)'", regex::escape(&cap)))
            .map_err(|e| AppError::Parse(e.to_string()))?;
        if let Some(m) = var_re.captures(&clean).and_then(|c| c.get(1)) {
            return Ok(m.as_str().to_string());
        }
    }
    Ok(cap)
}

/// 提取分享文件夹页参数（filemoreajax.php）
pub struct FolderAjaxParams {
    pub lx: String,
    pub t: String,
    pub k: String,
    pub fid: String,
}

pub fn match_folder_ajax(html: &str) -> Result<FolderAjaxParams, AppError> {
    // 变量名可能带下划线前缀（如 _hbg1i），不能只用 [0-9a-z]
    let lx = match_one(html, r"'lx'\s*:\s*'?(\d)'?")?;
    let t = match_one(html, r"var\s+[0-9a-z_]{6}\s*=\s*'(\d{10})'")?;
    let k = match_one(html, r"var\s+[0-9a-z_]{6}\s*=\s*'([0-9a-z]{15,})'")?;
    let fid = match_one(html, r"'fid'\s*:\s*'?(\d+)'?")?;
    Ok(FolderAjaxParams { lx, t, k, fid })
}

fn match_one(html: &str, pat: &str) -> Result<String, AppError> {
    let re = regex::Regex::new(pat).map_err(|e| AppError::Parse(e.to_string()))?;
    re.captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| AppError::Parse("参数匹配失败".into()))
}

/// 移除 HTML/JS 注释（避免干扰正则）
pub fn remove_notes(html: &str) -> String {
    let re = regex::Regex::new(r"<!--.+?-->|\s+//\s*.+").expect("valid regex");
    re.replace_all(html, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsbox() {
        // 40 个唯一字符，验证重排是双射
        let input = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcd"; // 26+10+4 = 40
        assert_eq!(input.len(), 40);
        let out = unsbox(input);
        assert_eq!(out.len(), 40);
        let mut chars: Vec<char> = out.chars().collect();
        chars.sort();
        let mut input_chars: Vec<char> = input.chars().collect();
        input_chars.sort();
        assert_eq!(chars, input_chars);
    }

    #[test]
    fn test_hex_xor() {
        assert_eq!(hex_xor("ff", "00"), "ff");
        assert_eq!(hex_xor("aa", "0f"), "a5");
    }

    #[test]
    fn test_calc_acw_sc_v2() {
        // 模拟包含 arg1 的挑战页面
        let html =
            r#"<script>var arg1='E4961D5F7A3B2C8D90E1F2A3B4C5D6E7F8091A2B3C4D5E6F7';</script>"#;
        let cookie = calc_acw_sc_v2(html).expect("should calc cookie");
        assert_eq!(cookie.len(), 40);
    }

    #[test]
    fn test_calc_acw_sc_v2_missing_arg1() {
        let html = "<html>no challenge</html>";
        assert!(calc_acw_sc_v2(html).is_none());
    }

    #[test]
    fn test_match_iframe() {
        let html = r#"<html><body><iframe src="//up.woozooo.com/file/xxxx" width="100"></iframe></body></html>"#;
        let src = match_iframe(html).expect("should find iframe");
        assert_eq!(src, "//up.woozooo.com/file/xxxx");
    }

    #[test]
    fn test_match_sign_direct() {
        let html = r#"$.ajax({url:'/ajaxm.php', data:{'sign':'AGZRbwEwU2IEDQU6BDRUaFc8DzxfMlRjCjTPlVkWzFSYFY7ATpWYw_c_c'}})"#;
        let sign = match_sign(html).expect("should find sign");
        assert_eq!(
            sign,
            "AGZRbwEwU2IEDQU6BDRUaFc8DzxfMlRjCjTPlVkWzFSYFY7ATpWYw_c_c"
        );
    }

    #[test]
    fn test_match_sign_variable() {
        let html = r#"data:{'sign':sign}, var sign='AGZRbwEwU2IEDQU6BDRUaFc8DzxfMlRjCjTPlVkWzFSYFY7ATpWYw_c_c';"#;
        let sign = match_sign(html).expect("should resolve variable sign");
        assert_eq!(
            sign,
            "AGZRbwEwU2IEDQU6BDRUaFc8DzxfMlRjCjTPlVkWzFSYFY7ATpWYw_c_c"
        );
    }

    #[test]
    fn test_match_folder_ajax() {
        let html = r#"
            'lx':'1',
            var qwerty = '1234567890';
            var asdfgh = 'abc123def456ghi789jkl012';
            'fid':'12345',
        "#;
        let p = match_folder_ajax(html).expect("should parse folder params");
        assert_eq!(p.lx, "1");
        assert_eq!(p.t, "1234567890");
        assert_eq!(p.k, "abc123def456ghi789jkl012");
        assert_eq!(p.fid, "12345");
    }
}
