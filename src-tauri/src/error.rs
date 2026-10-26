//! 统一错误类型：命令层统一返回 Result<T, String>，错误信息面向最终用户可读。

pub type AppResult<T> = Result<T, String>;
