use std::{os::windows::process::CommandExt, path::Path, process::Command};

use windows::{
    core::PCWSTR,
    Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
};

use crate::{error::AppResult, models::AppError, path, system::wide};

// 统一启动一个用户保存的目标。
//
// 这个模块是系统层的唯一启动入口，只接收启动所必需的三个值：
//
// - `target_path`：目标程序、快捷方式、目录、URL 或关联文件；
// - `working_dir`：可选的工作目录；
// - `arguments`：可选的 Windows 原始参数字符串。
//
// `AppEXE`、分类树和应用 ID 都属于业务层，不在这里出现。
// 普通可执行文件使用 `Command`，Shell 能识别的目标使用
// `ShellExecuteW`。两种实现都只返回简单结果，不向上层暴露
// `Child`、`HICON` 或 Windows 句柄。
pub(crate) fn launch(
    target_path: &Path,
    working_dir: Option<&Path>,
    arguments: Option<&str>,
) -> AppResult<()> {
    validate_target_path(target_path)?;

    if should_use_shell(target_path) {
        shell_open(target_path, working_dir, arguments)
    } else {
        spawn_program(target_path, working_dir, arguments)
    }
}

// 使用 `std::process::Command` 启动可执行文件，并立即返回。
//
// `Child` 是函数内部的临时值。函数返回时子进程仍会继续运行，
// 但进程句柄不会泄漏到 service 或前端。
fn spawn_program(
    target_path: &Path,
    working_dir: Option<&Path>,
    arguments: Option<&str>,
) -> AppResult<()> {
    let mut command = Command::new(target_path);

    if let Some(directory) = non_empty_path(working_dir) {
        command.current_dir(directory);
    }

    // 当前参数被定义为一个完整的 Windows 原始命令行字符串。
    // `raw_arg` 不会重新拆分或转义字符串，可以保留 Windows 参数中
    // 的引号、反斜杠和带空格的值。
    if let Some(raw_arguments) = non_empty_text(arguments) {
        if raw_arguments.contains('\0') {
            return Err(AppError::new(
                "exec_invalid_arguments",
                "启动参数不能包含 NUL 字符",
            ));
        }
        command.raw_arg(raw_arguments);
    }

    // 不调用 `wait()`：普通应用启动属于非阻塞操作。
    command.spawn().map(|_child| ()).map_err(|error| {
        AppError::new(
            "exec_spawn_error",
            format!("无法启动程序 {}: {error}", target_path.display()),
        )
    })
}

// 使用 Windows Shell 打开快捷方式、目录、URL 或文件关联。
//
// `ShellExecuteW` 的返回值大于 32 表示成功；小于等于 32 表示
// Shell 没有接受该请求。这里不使用 `runas` 动词，因此不会主动请求
// 管理员权限。
fn shell_open(
    target_path: &Path,
    working_dir: Option<&Path>,
    arguments: Option<&str>,
) -> AppResult<()> {
    let operation = to_wide_string("open", "exec_invalid_operation")?;
    let target = to_wide_path(target_path)?;
    let parameters = non_empty_text(arguments)
        .map(|value| to_wide_string(value, "exec_invalid_arguments"))
        .transpose()?;
    let directory = non_empty_path(working_dir).map(to_wide_path).transpose()?;

    let parameter_ptr = parameters
        .as_deref()
        .map_or(PCWSTR::null(), |value| PCWSTR(value.as_ptr()));
    let directory_ptr = directory
        .as_deref()
        .map_or(PCWSTR::null(), |value| PCWSTR(value.as_ptr()));

    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(operation.as_ptr()),
            PCWSTR(target.as_ptr()),
            parameter_ptr,
            directory_ptr,
            SW_SHOWNORMAL,
        )
    };

    // `ShellExecuteW` 返回 `HINSTANCE`。Windows 文档规定：数值小于等于
    // 32 表示失败；转换为 `usize` 可以兼容 windows-rs 对句柄底层整数
    // 类型的表示。
    if (result.0 as usize) <= 32 {
        return Err(AppError::new(
            "shell_execute_error",
            format!(
                "Shell 无法打开 {}（返回码 {}）",
                target_path.display(),
                result.0 as usize
            ),
        ));
    }

    Ok(())
}

// 仅把确定是可执行文件的目标交给 `Command`，其他目标交给 Shell。
//
// 这样可以覆盖 `.lnk`、`.url`、目录、`.bat`、`.cmd`、`.msc` 以及
// `http://`、`https://` 等 Shell 目标，同时避免把原始参数交给
// `cmd.exe` 解释。
fn should_use_shell(target_path: &Path) -> bool {
    if path::is_url_target(target_path) || target_path.is_dir() {
        return true;
    }

    !path::is_native_program(target_path)
}

// 防止空目标和带 NUL 的 Windows 字符串进入系统 API。
fn validate_target_path(target_path: &Path) -> AppResult<()> {
    if target_path.as_os_str().is_empty() {
        return Err(AppError::new("exec_target_empty", "启动目标路径不能为空"));
    }

    wide::reject_embedded_nul(
        target_path.as_os_str(),
        "exec_invalid_target",
        "启动目标路径",
    )
}

fn non_empty_path(path: Option<&Path>) -> Option<&Path> {
    path.filter(|path| !path.as_os_str().is_empty())
}

fn non_empty_text(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.trim().is_empty())
}

// 将 Windows 字符串转换成以 NUL 结尾的 UTF-16 字符串。
fn to_wide(value: &std::ffi::OsStr, error_code: &'static str, kind: &str) -> AppResult<Vec<u16>> {
    wide::nul_terminated(value, error_code, kind)
}

// 将 Rust 字符串转换成以 NUL 结尾的 UTF-16 字符串。
fn to_wide_string(value: &str, error_code: &'static str) -> AppResult<Vec<u16>> {
    to_wide(std::ffi::OsStr::new(value), error_code, "字符串")
}

// 将 Windows 路径转换成以 NUL 结尾的 UTF-16 字符串。
fn to_wide_path(path: &Path) -> AppResult<Vec<u16>> {
    to_wide(path.as_os_str(), "exec_invalid_path", "路径")
}
