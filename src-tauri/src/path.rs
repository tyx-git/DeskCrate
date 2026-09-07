use std::path::Path;

// 跨服务共享的纯路径工具。
//
// 这些函数只依赖标准库，不包含业务状态或 Windows API，因而可以同时
// 被 service 层和 system 层使用，不会造成模块方向反转。

pub(crate) fn is_shortcut(path: &Path) -> bool {
    has_extension(path, "lnk")
}

pub(crate) fn is_executable(path: &Path) -> bool {
    has_extension(path, "exe")
}

pub(crate) fn is_native_program(path: &Path) -> bool {
    is_executable(path) || has_extension(path, "com")
}

// 判断本地脚本路径；URL 即使最后一段带脚本扩展名，也不属于脚本文件。
pub(crate) fn is_script(path: &Path) -> bool {
    if is_url_target(path) {
        return false;
    }

    ["bat", "cmd", "ps1", "vbs", "js"]
        .iter()
        .any(|extension| has_extension(path, extension))
}

// 判断是否为当前版本支持的网页地址。
//
// 应用来源和系统启动目标必须使用同一套规则，否则添加阶段可能把地址
// 当成本地文件，而启动阶段又把它交给 Shell。当前只允许 http/https，
// 与 `IconType::Url` 的语义保持一致。
pub(crate) fn is_web_url(value: &str) -> bool {
    let value = value.trim();
    let Some((scheme, remainder)) = value.split_once("://") else {
        return false;
    };

    !remainder.is_empty()
        && (scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
}

pub(crate) fn is_url_target(path: &Path) -> bool {
    is_web_url(&path.to_string_lossy())
}

// 从路径推导展示名称，并统一处理没有文件名的路径。
pub(crate) fn display_name(path: &Path, fallback: &str) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

fn has_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}
