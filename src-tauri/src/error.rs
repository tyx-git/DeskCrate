use crate::models::AppError;
use std::{error::Error, fmt, io};
pub type AppResult<T> = Result<T, AppError>;

// JSON 解析失败的稳定错误码。
//
// `JsonStore` 的损坏恢复逻辑靠它区分两类失败：**文件内容不是合法 JSON** 时，
// 备份或复制用户的文件是安全的（内容已经坏了）；而 **文件读不出来**（被其他
// 进程占用、权限不足）时绝不能动它 —— 那种情况下文件本身可能是好的，改名或
// 覆盖只会把好数据弄丢。
//
// 前端也会拿到这个 code，所以它是契约的一部分，不能随手改。
pub const JSON_PARSE_ERROR_CODE: &str = "JsonSerdeError";

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt<'a>(&self, formatter: &mut fmt::Formatter<'a>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl Error for AppError {}

impl From<io::Error> for AppError {
    fn from(error: io::Error) -> Self {
        Self::new("IoError", error.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        Self::new(JSON_PARSE_ERROR_CODE, error.to_string())
    }
}
