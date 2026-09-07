use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

use crate::{error::AppResult, models::AppError};

// Windows API 使用以 NUL 结尾的 UTF-16 字符串。所有进入系统 API 的
// Rust 路径和文本都经由这里转换，避免嵌入的 NUL 被 Windows 截断。
pub(crate) fn nul_terminated(
    value: &OsStr,
    error_code: &'static str,
    value_name: &str,
) -> AppResult<Vec<u16>> {
    validate_no_nul(value, error_code, value_name)?;

    let mut wide = value.encode_wide().collect::<Vec<_>>();
    wide.push(0);
    Ok(wide)
}

// 在不需要实际 UTF-16 缓冲区的路径上复用相同的 NUL 校验。
pub(crate) fn reject_embedded_nul(
    value: &OsStr,
    error_code: &'static str,
    value_name: &str,
) -> AppResult<()> {
    validate_no_nul(value, error_code, value_name)
}

fn validate_no_nul(value: &OsStr, error_code: &'static str, value_name: &str) -> AppResult<()> {
    if value.encode_wide().any(|character| character == 0) {
        return Err(AppError::new(
            error_code,
            format!("Windows {value_name}不能包含 NUL 字符"),
        ));
    }

    Ok(())
}
