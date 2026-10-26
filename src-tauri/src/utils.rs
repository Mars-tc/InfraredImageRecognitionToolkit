//! 小工具函数。

/// 缺陷等级名称（0~3）
pub fn level_name(level: u8) -> &'static str {
    match level {
        1 => "一般",
        2 => "重大",
        3 => "紧急",
        _ => "其他",
    }
}
