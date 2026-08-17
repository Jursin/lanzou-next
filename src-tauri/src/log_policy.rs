pub(crate) const MAX_LOG_FILE_SIZE: u64 = 20 * 1024 * 1024;
/// 日志插件目标文件名（不含扩展名，插件会自动补 .log）
pub(crate) const LANZOU_LOG_NAME: &str = "lanzou-next";
/// 实际落盘的日志文件名
pub(crate) const LANZOU_LOG_FILE: &str = "lanzou-next.log";

/// 校验可用的日志级别（与 log crate LevelFilter 对应）
pub(crate) fn valid_log_level(level: &str) -> bool {
    matches!(level, "error" | "warn" | "info" | "debug" | "trace")
}

/// 将字符串日志级别转为 LevelFilter，非法值回退为 warn
pub(crate) fn log_level_filter(level: &str) -> log::LevelFilter {
    match level {
        "error" => log::LevelFilter::Error,
        "debug" => log::LevelFilter::Debug,
        "info" => log::LevelFilter::Info,
        "trace" => log::LevelFilter::Trace,
        _ => log::LevelFilter::Warn,
    }
}
