use crate::models::AppError;
use std::{error::Error, fmt, io};
pub type AppResult<T> = Result<T, AppError>;
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
        Self::new("JsonSerdeError", error.to_string())
    }
}
