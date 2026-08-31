use std::{ffi::OsStr, os::windows::ffi::OsStrExt, path::Path};

use windows::{
    core::{Interface, PCWSTR},
    Win32::{
        Storage::FileSystem::WIN32_FIND_DATAW,
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED, STGM_READ,
        },
        UI::Shell::{IShellLinkW, ShellLink, SLGP_RAWPATH},
    },
};

use crate::{
    error::AppResult,
    models::{AppError, IconLocation, ParsedShortcut},
};

// Windows Shell Link API 使用的宽字符缓冲区大小。
//
// `IShellLinkW` 的接口参数是可写切片，使用扩展路径上限可以兼容
// 普通路径和较长路径。实际字符串仍以第一个 NUL 字符为准。
const PATH_BUFFER_LEN: usize = 32_768;

// 解析 Windows `.lnk` 快捷方式。
//
// 这里使用 Windows 原生的 `IShellLinkW`，而不是把快捷方式文件当作
// 普通文本处理。这样可以让系统负责解析目标路径、工作目录、参数和
// 图标位置等 Shell Link 字段。
pub(crate) fn parse_shortcut(path: &Path) -> AppResult<ParsedShortcut> {
    if !path.is_file() {
        return Err(AppError::new(
            "shortcut_not_found",
            format!("快捷方式不存在或不是文件: {}", path.display()),
        ));
    }

    let _com = ComApartment::initialize()?;
    let wide_path = to_wide(path.as_os_str());

    let shell_link: IShellLinkW = unsafe {
        CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| windows_error("shortcut_create_failed", error))?
    };

    let persist_file: IPersistFile = shell_link
        .cast()
        .map_err(|error| windows_error("shortcut_cast_failed", error))?;

    unsafe {
        persist_file
            .Load(PCWSTR(wide_path.as_ptr()), STGM_READ)
            .map_err(|error| windows_error("shortcut_load_failed", error))?;
    }

    let mut target_buffer = vec![0u16; PATH_BUFFER_LEN];
    let mut find_data = WIN32_FIND_DATAW::default();

    unsafe {
        shell_link
            .GetPath(&mut target_buffer, &mut find_data, SLGP_RAWPATH.0 as u32)
            .map_err(|error| windows_error("shortcut_target_failed", error))?;
    }

    let target_path = read_wide_string(&target_buffer).ok_or_else(|| {
        AppError::new(
            "shortcut_target_empty",
            format!("快捷方式没有可用的目标路径: {}", path.display()),
        )
    })?;

    let mut working_dir_buffer = vec![0u16; PATH_BUFFER_LEN];
    let working_dir = unsafe { shell_link.GetWorkingDirectory(&mut working_dir_buffer).ok() }
        .and_then(|_| read_wide_string(&working_dir_buffer));

    let mut arguments_buffer = vec![0u16; PATH_BUFFER_LEN];
    let arguments = unsafe { shell_link.GetArguments(&mut arguments_buffer).ok() }
        .and_then(|_| read_wide_string(&arguments_buffer));

    let mut icon_path_buffer = vec![0u16; PATH_BUFFER_LEN];
    let mut icon_index = 0;
    let icon_location = unsafe {
        shell_link
            .GetIconLocation(&mut icon_path_buffer, &mut icon_index)
            .ok()
    }
    .and_then(|_| {
        read_wide_string(&icon_path_buffer).map(|icon_path| IconLocation {
            path: icon_path,
            index: icon_index,
        })
    });

    Ok(ParsedShortcut {
        source_path: path.to_string_lossy().into_owned(),
        name: shortcut_name(path),
        target_path,
        working_dir,
        arguments,
        icon_location,
    })
}

// 初始化当前线程的 COM Apartment，并在离开作用域时释放初始化计数。
struct ComApartment;

impl ComApartment {
    fn initialize() -> AppResult<Self> {
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

        if result.is_err() {
            return Err(AppError::new(
                "com_initialize_failed",
                format!("无法初始化 COM Apartment: {result:?}"),
            ));
        }

        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

// 把 Rust 的 Windows 路径转换成以 NUL 结尾的 UTF-16 字符串。
fn to_wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

// 读取 Windows API 写入的 NUL 结尾 UTF-16 缓冲区。
fn read_wide_string(buffer: &[u16]) -> Option<String> {
    let end = buffer.iter().position(|character| *character == 0)?;
    if end == 0 {
        return None;
    }

    Some(String::from_utf16_lossy(&buffer[..end]))
}

// 默认使用快捷方式文件名作为前端展示名称。
fn shortcut_name(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "快捷方式".to_owned())
}

// 将 Windows API 错误统一转换成项目错误模型。
fn windows_error(code: &'static str, error: windows::core::Error) -> AppError {
    AppError::new(code, format!("{error:?}"))
}
