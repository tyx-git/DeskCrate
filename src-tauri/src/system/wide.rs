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

// Windows 注册表和其他 Win32 API 都需要 NUL 结尾的 UTF-16；字符串型 API 共用
// 这一条转换，避免每个 system 子模块各写一份。
pub(crate) fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

// `RegSetValueExW` 接收 UTF-16 的原始字节，而不是 `u16` 切片。
pub(crate) fn to_wide_bytes(value: &str) -> Vec<u8> {
    value
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
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
