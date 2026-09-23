use crate::models::AppError;

// Windows API 错误的统一转换入口。
//
// system 子模块都只向上层暴露项目错误模型；集中转换后，各模块不再重复
// 维护相同的错误格式，也能保证前端收到的错误结构始终一致。
pub(crate) fn windows_error(code: &'static str, error: windows::core::Error) -> AppError {
    AppError::new(code, error.to_string())
}
