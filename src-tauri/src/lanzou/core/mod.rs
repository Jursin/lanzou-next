pub mod download;
pub mod files;
pub mod ls;
pub mod merge;
pub mod ops;
pub mod profile;
pub mod recycle;
pub mod share;
pub mod upload;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LsFile {
    /// 文件名（展示名）
    pub name: String,
    /// 文件 id
    pub id: String,
    /// 文件类型：file | folder
    pub r#type: String,
    pub icon: Option<String>,
    pub size: Option<String>,
    pub time: Option<String>,
    pub downs: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrumbsInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LsResult {
    pub info: Vec<CrumbsInfo>,
    pub files: Vec<LsFile>,
}
