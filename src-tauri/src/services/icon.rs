use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use windows::core::PCWSTR;
use windows::Win32::System::Environment::ExpandEnvironmentStringsW;

use crate::{
    error::AppResult,
    models::{IconExtractionResult, ParsedShortcut},
    path,
    services::favicon,
    storage::JsonStore,
    system::{icon_extract::IconExtractor, shortcut},
};

// 图标业务服务。
//
// 该服务只负责业务流程：
//
// - 解析快捷方式；
// - 判断实际图标来源（本地文件、快捷方式、网址站点图标）；
// - 把图标提取器返回的结果交给调用方；
// - 约定图标缓存目录和前端资源 URL。
//
// Windows API、COM 和 GDI 资源都封装在 `crate::system` 中，
// 不会泄漏到 Command 层。
pub(crate) struct IconService {
    extractor: IconExtractor,
}

impl IconService {
    // 使用指定的图标缓存目录创建服务。
    pub(crate) fn new(icons_dir: impl Into<PathBuf>) -> Self {
        Self {
            extractor: IconExtractor::new(icons_dir),
        }
    }

    // 从 `JsonStore` 创建服务，并复制图标目录路径。
    //
    // 只复制路径，不持有 `JsonStore` 借用，
    // 因此调用方可以在释放 `MutexGuard` 后继续执行耗时的图标操作。
    pub(crate) fn from_store(store: &JsonStore) -> Self {
        Self::new(store.icons_path().to_path_buf())
    }

    // 将图标缓存文件路径转换成 WebView 可以直接加载的 Tauri asset URL。
    //
    // `asset.localhost` 由 `tauri.conf.json` 中的 asset protocol 提供；
    // 路径整体按 `encodeURIComponent` 语义编码，避免盘符、反斜杠、空格
    // 或非 ASCII 字符改变 URL 的路径结构。
    pub(crate) fn asset_url(cache_path: &Path) -> String {
        format!(
            "http://asset.localhost/{}",
            encode_uri_component(cache_path)
        )
    }

    // 解析一个 Windows 快捷方式。
    pub(crate) fn parse_shortcut(&self, shortcut_path: &Path) -> AppResult<ParsedShortcut> {
        shortcut::parse_shortcut(shortcut_path)
    }

    // 根据输入路径自动选择提取策略。
    //
    // `.lnk` 会先解析快捷方式；`.exe` 使用显式图标资源提取策略；
    // 普通文件、文件夹和其他 Shell 可识别路径使用关联图标策略。
    pub(crate) fn extract_from_path(
        &self,
        source_path: &Path,
        arguments: Option<&str>,
        app_id: &str,
    ) -> AppResult<IconExtractionResult> {
        // 系统应用（UWP/Store 应用、回收站、控制面板等）的图标不在
        // explorer.exe 的资源里，必须交给 Shell 命名空间解析。
        // 这里只做严格的开头匹配：启动参数中间出现的 shell 字样不算，
        // 大小写也必须完全一致。
        if let Some(target) = arguments.filter(|value| value.starts_with("shell:")) {
            match self.extractor.extract_file(Path::new(target), 0, app_id) {
                Ok(result) => return Ok(result),
                Err(error) => {
                    // 图标失败不该阻止添加/编辑；继续走来源本身的图标，
                    // 与旧数据以及无效 AUMID 的行为保持一致。
                    log::debug!("系统应用图标解析失败，回退来源图标: {}", error.message);
                }
            }
        }

        if path::is_shortcut(source_path) {
            let shortcut = self.parse_shortcut(source_path)?;
            return self.extract_from_shortcut(&shortcut, app_id);
        }

        if path::is_executable(source_path) {
            return self.extractor.extract_file(source_path, 0, app_id);
        }

        self.extractor.extract_associated(source_path, app_id)
    }

    // 抓取网址的站点图标并缓存。
    //
    // 失败不抛给调用方：抓不到图标只是少了张图，网址应用本身照样能加进来，
    // 届时由 `prepare_app` 退回内置图标。这里记一条 debug 日志，方便排查
    // 「为什么这个站点没图标」。
    pub(crate) fn fetch_url_icon(&self, url: &str, app_id: &str) -> Option<IconExtractionResult> {
        // SVG 走另一条存盘路径：不解码，原文字节按 `.svg` 落盘。
        match favicon::fetch_icon(url) {
            Ok(favicon::FetchedIcon::Bitmap(image)) => {
                match self.extractor.save_image(app_id, image) {
                    Ok(result) => Some(result),
                    Err(error) => {
                        log::debug!("保存 {} 的网址图标失败: {}", url, error.message);
                        None
                    }
                }
            }
            Ok(favicon::FetchedIcon::Svg(bytes)) => match self.extractor.save_svg(app_id, &bytes) {
                Ok(result) => Some(result),
                Err(error) => {
                    log::debug!("保存 {} 的 SVG 图标失败: {}", url, error.message);
                    None
                }
            },
            Err(error) => {
                log::debug!("抓取 {} 的网址图标失败: {}", url, error.message);
                None
            }
        }
    }

    // 复用另一个应用的图标缓存。
    //
    // 同源站点第二次添加时走这条路，省掉一次网络抓取。失败返回 `None` 而不是抛错：
    // 复用只是优化，拿不到就退回内置图标、交给后台补抓，不该让「添加应用」失败。
    pub(crate) fn copy_icon(
        &self,
        source_app_id: &str,
        app_id: &str,
    ) -> Option<IconExtractionResult> {
        match self.extractor.copy_icon(source_app_id, app_id) {
            Ok(result) => Some(result),
            Err(error) => {
                log::debug!("复用 {source_app_id} 的图标失败: {}", error.message);
                None
            }
        }
    }

    // 根据已解析的快捷方式提取图标。
    //
    // 优先使用快捷方式显式指定的 `icon_location`；
    // 如果没有指定，则回退到快捷方式的 `target_path`。
    pub(crate) fn extract_from_shortcut(
        &self,
        shortcut: &ParsedShortcut,
        app_id: &str,
    ) -> AppResult<IconExtractionResult> {
        let (icon_source, icon_index) = shortcut_icon_source(shortcut)?;

        if shortcut.icon_location.is_some() {
            self.extractor
                .extract_file(&icon_source, icon_index, app_id)
        } else {
            self.extractor.extract_associated(&icon_source, app_id)
        }
    }
}

fn encode_uri_component(path: &Path) -> String {
    let value = path.to_string_lossy();
    let mut encoded = String::with_capacity(value.len());

    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(
                *byte,
                b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
            )
        {
            encoded.push(*byte as char);
        } else {
            encoded.push('%');
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }

    encoded
}

const HEX: &[u8; 16] = b"0123456789ABCDEF";

// 计算快捷方式实际使用的图标来源。
fn shortcut_icon_source(shortcut: &ParsedShortcut) -> AppResult<(PathBuf, i32)> {
    if let Some(location) = &shortcut.icon_location {
        let icon_path = PathBuf::from(&location.path);
        let resolved_path = resolve_shortcut_relative_path(&shortcut.source_path, icon_path);
        return Ok((resolved_path, location.index));
    }

    if shortcut.target_path.trim().is_empty() {
        return Err(crate::models::AppError::new(
            "shortcut_no_icon_source",
            "快捷方式没有可用的图标来源",
        ));
    }

    Ok((PathBuf::from(&shortcut.target_path), 0))
}

// 将快捷方式中的相对图标路径解析为相对于 `.lnk` 所在目录的路径。
//
// `IconLocation` 常见 `%windir%\...` 这种 Windows 占位符，不能直接当相对路径；
// 先按系统规则展开，再判断绝对/相对。
fn resolve_shortcut_relative_path(shortcut_path: &str, icon_path: PathBuf) -> PathBuf {
    let expanded = {
        let wide: Vec<u16> = icon_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let needed = unsafe { ExpandEnvironmentStringsW(PCWSTR(wide.as_ptr()), None) };

        if needed == 0 {
            None
        } else {
            let mut buffer = vec![0u16; needed as usize];
            let written =
                unsafe { ExpandEnvironmentStringsW(PCWSTR(wide.as_ptr()), Some(&mut buffer)) };

            // 返回值包含结尾 NUL；失败或越界时保留原路径。
            if written == 0 || written > needed {
                None
            } else {
                Some(PathBuf::from(OsString::from_wide(
                    &buffer[..written as usize - 1],
                )))
            }
        }
    };

    let icon_path = expanded.unwrap_or(icon_path);
    if icon_path.is_absolute() {
        return icon_path;
    }

    Path::new(shortcut_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(icon_path)
}

#[cfg(test)]
mod tests {
    use super::resolve_shortcut_relative_path;
    use std::path::{Path, PathBuf};

    #[test]
    fn expands_windows_environment_variables_in_icon_location() {
        let windir = std::env::var("windir").expect("windir 环境变量应存在");
        let resolved = resolve_shortcut_relative_path(
            r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Administrative Tools\Task Scheduler.lnk",
            PathBuf::from(r"%windir%\system32\miguiresource.dll"),
        );

        assert!(resolved.is_absolute());
        assert_eq!(
            resolved,
            Path::new(&windir)
                .join("system32")
                .join("miguiresource.dll")
        );
    }

    #[test]
    fn keeps_relative_icon_paths_relative_to_shortcut() {
        let resolved =
            resolve_shortcut_relative_path(r"C:\Links\app.lnk", PathBuf::from(r"icons\app.ico"));

        assert_eq!(resolved, PathBuf::from(r"C:\Links\icons\app.ico"));
    }

    #[test]
    fn keeps_unknown_placeholders_under_shortcut_directory() {
        let resolved = resolve_shortcut_relative_path(
            r"C:\Links\app.lnk",
            PathBuf::from(r"%DESKCRATE_UNKNOWN_VAR%\icon.ico"),
        );

        assert_eq!(
            resolved,
            PathBuf::from(r"C:\Links\%DESKCRATE_UNKNOWN_VAR%\icon.ico")
        );
    }
}
