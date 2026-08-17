use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("网络请求失败: {0}")]
    Http(#[from] reqwest::Error),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),
    #[error("URL 错误: {0}")]
    Url(#[from] url::ParseError),
    #[error("配置错误: {0}")]
    Config(String),
    #[error("未登录")]
    NotLoggedIn,
    #[error("蓝奏云接口返回错误: {0}")]
    Lanzou(String),
    #[error("Tauri 错误: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("解析错误: {0}")]
    Parse(String),
    #[error("更新检查失败: {0}")]
    Update(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
