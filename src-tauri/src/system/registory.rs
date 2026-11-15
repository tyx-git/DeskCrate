// 开机自启动。
//
// Windows 的自启动入口有两种：当前用户的 Run 键，以及「启动」文件夹里的
// 快捷方式。这里用后者：
//
//     %APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\DeskCrate.lnk
//
// 原因是联想电脑管家等启动优化器会把第三方写入的 Run 值移到
// `Run\LenovoDisabled` 并加上 `rem|` 前缀，导致应用看似写成功、实际不会
// 开机启动；启动文件夹快捷方式不会被这类优化器搬走。
//
// 任务管理器的启用/禁用标记仍然写在注册表：
//
//     HKCU\...\Explorer\StartupApproved\StartupFolder\DeskCrate.lnk
//
// 「启动」文件夹是**系统状态**，`config.json` 是**应用状态**，两者会各说各话
// （用户手动删掉快捷方式、换了安装目录、或在任务管理器里禁用）。因此：
//
// - `update_settings` 里以配置为准，值变了就同步快捷方式；
// - 启动时 `reconcile_auto_start` 反过来对一次账。
//
// **对账不能只看「快捷方式存在与否」**：程序被挪到别的目录之后，旧快捷方式
// 可能仍然存在，但开机启动的不是用户正在用的那个。所以必须解析 `.lnk` 的
// target 和 `current_exe()` 比对，见 `AutoStartState`。
//
// 写快捷方式/读快捷方式都走 `windows` crate 的 Shell Link API；注册表标记
// 直接走注册表 API，不引入 `winreg` 这类额外依赖。

use std::fs;
use std::path::{Path, PathBuf};

use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::{
            ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_PATH_NOT_FOUND,
            ERROR_SUCCESS, HANDLE,
        },
        System::{
            Com::CoTaskMemFree,
            Registry::{
                RegCloseKey, RegCreateKeyW, RegDeleteValueW, RegEnumValueW, RegOpenKeyExW,
                RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE,
                REG_BINARY, REG_SAM_FLAGS, REG_SZ, REG_VALUE_TYPE,
            },
        },
        UI::Shell::{FOLDERID_Startup, SHGetKnownFolderPath, KNOWN_FOLDER_FLAG},
    },
};

use crate::{
    error::AppResult,
    models::AppError,
    system::{shortcut, wide},
};

// 枚举 Run 键时放值名的缓冲区长度（字符）。Run 的值名都是短标识符，1024 足够；
// 真遇到更长的名字会返回 `ERROR_MORE_DATA`，跳过即可。
const VALUE_NAME_CAPACITY: usize = 1024;

// 自启动项在注册表里的名字。
//
// 用固定名字而不是随机 GUID：用户想手动清启动项时，能在任务管理器或系统优化
// 工具里看出这是谁，删起来也有据可依。
const RUN_VALUE_NAME: &str = "DeskCrate";

const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

// 自启动项在「启动」文件夹里的文件名。Lenovo PC Manager 会把 Run 值搬进
// `Run\LenovoDisabled`，所以 Runs 值不再作为唯一入口；启动文件夹快捷方式
// 不会被这类优化器搬走，任务管理器也仍然能识别和禁用。
const STARTUP_SHORTCUT_NAME: &str = "DeskCrate.lnk";

// 任务管理器/系统设置里的“已禁用”标记。它不删 Run 值，只在旁边记一条
// `REG_BINARY`；应用必须读它，否则会把用户手动禁用的启动项又当成“已开启”。
const STARTUP_APPROVED_RUN_SUBKEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

const STARTUP_APPROVED_STARTUP_SUBKEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder";

// 本机实测的启用标记（12 字节）。首字节 0x02 表示启用，0x03 表示禁用；
// 时间戳部分对“启用”没有意义，全部填 0 即可。
const STARTUP_APPROVED_ENABLED: [u8; 12] = [0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

/// 「启动」文件夹里的自启动快捷方式与**当前进程**的关系。
///
/// 这里刻意不用布尔：快捷方式存在，不等于它指向正在运行的这个副本。程序被
/// 挪到别的目录后，旧快捷方式可能仍指向旧副本 —— 必须和 `current_exe()`
/// 比对才能发现这种情况。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AutoStartState {
    /// 快捷方式不存在。
    Absent,
    /// 指向正在运行的这个副本。
    MatchesCurrent,
    /// 存在但指向别处，含已被删除的路径。
    Elsewhere(PathBuf),
}

/// 任务管理器写下的启用/禁用状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartupApproval {
    /// 没有标记；Windows 视为启用。
    Missing,
    /// 用户明确启用。
    Enabled,
    /// 用户在任务管理器/系统设置里禁用。
    Disabled,
    /// 值存在但格式不认识；不据此自动改配置。
    Unknown,
}

/// 对账时要执行的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoStartAction {
    /// 系统状态已符合配置，不动。
    LeaveAlone,
    /// 创建/修复「启动」文件夹快捷方式，并写启用标记。
    Write,
    /// 删掉快捷方式和 StartupApproved 标记。
    Delete,
    /// 任务管理器已启用，配置还是关的：只把配置改成开启。
    TurnConfigOn,
    /// 任务管理器已禁用，保留被禁用的快捷方式，只把配置改成关闭。
    TurnConfigOffKeepEntry,
    /// 任务管理器已禁用且快捷方式已失效：删掉条目并把配置改成关闭。
    TurnConfigOffDeleteEntries,
}

/// 对账后应用配置需要同步的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AutoStartOutcome {
    NoChange,
    TurnOn,
    TurnOff,
}

/// 由「配置」和「注册表实际状态」推导该做什么。
///
/// 抽成纯函数是为了能直接测 —— 真去读写 Run 键会污染开发机的系统状态。
fn auto_start_action(
    configured: bool,
    state: &AutoStartState,
    approval: StartupApproval,
) -> AutoStartAction {
    match (approval, configured, state) {
        // 任务管理器禁用了当前副本：配置跟随为关闭，但保留那条禁用项，
        // 用户以后还能在任务管理器里重新启用。
        (StartupApproval::Disabled, true, AutoStartState::MatchesCurrent) => {
            AutoStartAction::TurnConfigOffKeepEntry
        }
        (StartupApproval::Disabled, false, AutoStartState::MatchesCurrent) => {
            AutoStartAction::LeaveAlone
        }
        // 禁用标记还在，但 Run 值已经缺失/指向别处：这是残留，清掉。
        (StartupApproval::Disabled, true, _) => AutoStartAction::TurnConfigOffDeleteEntries,
        (StartupApproval::Disabled, false, _) => AutoStartAction::Delete,
        // 启用/无标记/未知：沿用原来的配置优先逻辑。
        (_, true, AutoStartState::MatchesCurrent) => AutoStartAction::LeaveAlone,
        (_, false, AutoStartState::MatchesCurrent) => AutoStartAction::TurnConfigOn,
        (_, true, _) => AutoStartAction::Write,
        (_, false, _) => AutoStartAction::Delete,
    }
}

/// 打开或关闭开机自启动。
pub(crate) fn set_auto_start_enabled(enabled: bool) -> AppResult<()> {
    if !enabled {
        return disable_auto_start();
    }

    let executable = std::env::current_exe().map_err(|error| {
        AppError::new(
            "auto_start_exe_unavailable",
            format!("取不到当前可执行文件路径: {error}"),
        )
    })?;
    write_auto_start(&executable)
}

/// 按需对齐开机自启动。
///
/// 启动时调用。失败只记日志 —— 自启动是锦上添花，不该让侧栏起不来。
pub(crate) fn reconcile_auto_start(configured: bool) -> AutoStartOutcome {
    // 取不到当前路径就没法判断「指向别处」，此时**放弃本次对账**而不是凭猜测
    // 去改系统状态。
    let current = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            log::warn!("取不到当前可执行文件路径，跳过自启动对账: {error}");
            return AutoStartOutcome::NoChange;
        }
    };

    let state = match auto_start_state(&current) {
        Ok(state) => state,
        Err(error) => {
            log::warn!("读不到开机自启动状态: {error}");
            return AutoStartOutcome::NoChange;
        }
    };

    let approval = match read_startup_approval() {
        Ok(approval) => approval,
        Err(error) => {
            log::warn!("读不到 StartupApproved 状态，保留配置不动: {error}");
            return AutoStartOutcome::NoChange;
        }
    };

    match auto_start_action(configured, &state, approval) {
        AutoStartAction::LeaveAlone => AutoStartOutcome::NoChange,
        AutoStartAction::Write => {
            match write_auto_start(&current) {
                Ok(()) => {
                    if let AutoStartState::Elsewhere(previous) = &state {
                        log::info!(
                            "开机自启动原本指向 {}，已改指当前副本 {}",
                            previous.display(),
                            current.display()
                        );
                    }
                }
                Err(error) => log::warn!("补写开机自启动失败: {error}"),
            }
            AutoStartOutcome::NoChange
        }
        AutoStartAction::Delete => {
            if let Err(error) = delete_auto_start_entries() {
                log::warn!("清理开机自启动失败: {error}");
            }
            AutoStartOutcome::NoChange
        }
        AutoStartAction::TurnConfigOn => AutoStartOutcome::TurnOn,
        AutoStartAction::TurnConfigOffKeepEntry => AutoStartOutcome::TurnOff,
        AutoStartAction::TurnConfigOffDeleteEntries => match delete_auto_start_entries() {
            Ok(()) => AutoStartOutcome::TurnOff,
            Err(error) => {
                log::warn!("任务管理器已禁用，但清理残留启动项失败: {error}");
                AutoStartOutcome::NoChange
            }
        },
    }
}

/// 读出「启动」文件夹快捷方式并判断它和 `current` 的关系。
fn auto_start_state(current: &Path) -> AppResult<AutoStartState> {
    // 取不到「启动」文件夹路径时按“没有启动项”处理；对账的写路径会再报错。
    let Some(path) = startup_shortcut_path() else {
        return Ok(AutoStartState::Absent);
    };
    if !path.is_file() {
        return Ok(AutoStartState::Absent);
    }

    let parsed = match shortcut::parse_shortcut(&path) {
        Ok(parsed) => parsed,
        Err(error) => {
            // 快捷方式损坏/无法解析：当成“指向别处”，让对账重写它。
            log::warn!("解析启动快捷方式失败，将重建: {error}");
            return Ok(AutoStartState::Elsewhere(path));
        }
    };

    let recorded = PathBuf::from(parsed.target_path);
    Ok(if points_at_same_file(&recorded, current) {
        AutoStartState::MatchesCurrent
    } else {
        AutoStartState::Elsewhere(recorded)
    })
}

/// 读任务管理器为启动文件夹快捷方式写下的启用/禁用标记。
///
/// 键或值不存在都按 `Missing` 处理（Windows 视为启用）；类型不是
/// `REG_BINARY` 时按 `Unknown` 处理，绝不据此把配置改成关闭。
fn read_startup_approval() -> AppResult<StartupApproval> {
    let Some(key) = open_key(STARTUP_APPROVED_STARTUP_SUBKEY, KEY_READ, false)? else {
        return Ok(StartupApproval::Missing);
    };
    let value = read_binary(key, STARTUP_SHORTCUT_NAME);
    unsafe {
        let _ = RegCloseKey(key);
    };

    let Some((kind, bytes)) = value? else {
        return Ok(StartupApproval::Missing);
    };
    if kind != REG_BINARY {
        return Ok(StartupApproval::Unknown);
    }

    Ok(startup_approval_from_bytes(&bytes))
}

/// 从 `StartupApproved` 的二进制值判断状态。
///
/// 本机实测（Windows 11）：首字节 `0x03` 是禁用，`0x02` 是启用；后半段
/// 是 FILETIME/时间戳，跟启用状态无关。空值或未知首字节都不自动改配置。
fn startup_approval_from_bytes(bytes: &[u8]) -> StartupApproval {
    match bytes.first() {
        Some(0x03) => StartupApproval::Disabled,
        Some(0x02) => StartupApproval::Enabled,
        _ => StartupApproval::Unknown,
    }
}

/// 写“已启用”标记；任务管理器据此显示为开启。
fn write_startup_approval_enabled() -> AppResult<()> {
    let Some(key) = open_key(STARTUP_APPROVED_STARTUP_SUBKEY, KEY_SET_VALUE, true)? else {
        return Err(AppError::new(
            "auto_start_approval_key_unavailable",
            format!("打不开注册表项 {STARTUP_APPROVED_STARTUP_SUBKEY}"),
        ));
    };
    let result = write_binary(key, STARTUP_SHORTCUT_NAME, &STARTUP_APPROVED_ENABLED);
    unsafe {
        let _ = RegCloseKey(key);
    };
    result
}

/// 删除一个 StartupApproved 标记。键/值不存在都算成功。
fn delete_startup_approval(subkey: &str, value_name: &str) -> AppResult<()> {
    let Some(key) = open_key(subkey, KEY_SET_VALUE, false)? else {
        return Ok(());
    };
    let result = delete_value(key, value_name);
    unsafe {
        let _ = RegCloseKey(key);
    };
    result
}

/// 两个路径是否指向同一个文件。
///
/// 必须走 `canonicalize`：Windows 上同一个文件可以有多种写法（大小写、
/// `\\?\` 扩展前缀、`PROGRA~1` 这种 8.3 短名、目录符号链接与联结），直接比
/// 字符串会把这些误判成「指向别处」，于是每次启动都白重写一遍注册表。
///
/// 路径已被删除时 `canonicalize` 失败，正好落到「不是同一个文件」——
/// 那正是需要修正的情况。
fn points_at_same_file(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

/// 创建「启动」文件夹快捷方式，并写启用标记。
fn write_auto_start(executable: &Path) -> AppResult<()> {
    let shortcut_path = startup_shortcut_path().ok_or_else(|| {
        AppError::new(
            "auto_start_startup_folder_unavailable",
            "找不到当前用户的「启动」文件夹",
        )
    })?;

    if let Some(parent) = shortcut_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AppError::new(
                "auto_start_startup_folder_unavailable",
                format!("创建「启动」文件夹失败 {}: {error}", parent.display()),
            )
        })?;
    }

    shortcut::create_shortcut(
        &shortcut_path,
        executable,
        executable.parent(),
        Some("DeskCrate"),
    )?;
    write_startup_approval_enabled()?;

    // 旧版本/手工测试可能在 Run 里留下 DeskCrate；删除它，避免与启动文件夹
    // 快捷方式同时拉起两个实例。LenovoDisabled 是第三方优化器的存储，不碰。
    if let Err(error) = delete_run_value() {
        log::warn!("清理遗留 Run 启动项失败（不影响启动文件夹快捷方式）: {error}");
    }

    Ok(())
}

/// 删掉 Run 值，不碰启动文件夹。
fn delete_run_value() -> AppResult<()> {
    // 不创建键：键不存在则值必然不存在，关闭自启动本来就是幂等的。
    // 用 `KEY_SET_VALUE` + `create = false` 是为了不制造系统状态 —— 否则「关掉
    // 自启动」这个动作反而会凭空建出一个 Run 键。
    let Some(key) = open_run_key(KEY_SET_VALUE, false)? else {
        return Ok(());
    };
    let result = delete_value(key, RUN_VALUE_NAME);
    unsafe {
        let _ = RegCloseKey(key);
    };
    result
}

/// 删除「启动」文件夹里的自启动快捷方式。文件不存在算成功。
fn delete_startup_shortcut() -> AppResult<()> {
    let Some(path) = startup_shortcut_path() else {
        return Ok(());
    };

    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::new(
            "auto_start_shortcut_delete_failed",
            format!("删不掉启动快捷方式 {}: {error}", path.display()),
        )),
    }
}

/// 删除启动文件夹快捷方式、对应 StartupApproved 标记，以及旧版 Run 残留。
fn delete_auto_start_entries() -> AppResult<()> {
    delete_startup_shortcut()?;
    delete_startup_approval(STARTUP_APPROVED_STARTUP_SUBKEY, STARTUP_SHORTCUT_NAME)?;

    if let Err(error) = delete_run_value() {
        log::warn!("清理遗留 Run 启动项失败（不影响关闭自启动）: {error}");
    }
    if let Err(error) = delete_startup_approval(STARTUP_APPROVED_RUN_SUBKEY, RUN_VALUE_NAME) {
        log::warn!("清理 Run 的 StartupApproved 标记失败: {error}");
    }

    Ok(())
}

/// 应用内关闭自启动：删快捷方式和标记，并**立即**清理其他重复快捷方式。
fn disable_auto_start() -> AppResult<()> {
    delete_auto_start_entries()?;
    cleanup_startup_folder_shortcuts();
    Ok(())
}

/// 在当前线程解析并清理「启动」文件夹里的 DeskCrate 快捷方式。
///
/// 应用内关闭自启动时调用；启动时的 sweep 在后台线程执行，避免在窗口创建
/// 前初始化 COM。这里失败只记日志，sweep 下次启动仍会兜底。
fn cleanup_startup_folder_shortcuts() {
    let Some(exe_name) = std::env::current_exe().ok().and_then(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }) else {
        log::warn!("取不到当前可执行文件名，跳过「启动」文件夹清理");
        return;
    };

    let _com = match shortcut::ComApartment::initialize() {
        Ok(com) => com,
        Err(error) => {
            log::warn!("初始化 COM 失败，跳过「启动」文件夹清理: {error}");
            return;
        }
    };
    if let Err(error) = sweep_startup_folder(&exe_name) {
        log::warn!("清理「启动」文件夹失败: {error}");
    }
}

/// 扫描并清理失效的启动项。
///
/// 两个来源：
///
/// 1. **Run 键里任意值名**下、指向已不存在的 `deskcrate.exe` 的条目。我们自己的
///    值名固定是 `DeskCrate`（由 `reconcile_auto_start` 处理），但用户可能手工
///    添加过别的名字；程序被挪走或删掉之后，那些条目就是每次开机弹一个「找不到
///    文件」的垃圾。
/// 2. **「启动」文件夹**里指向 `deskcrate.exe` 的快捷方式。
///
/// 判定只看**目标文件名是否与我们当前的可执行文件同名**（不区分大小写），
/// 因此不会误删别的程序的启动项。
///
/// 由 `lib.rs` 在启动时丢到后台线程执行：这里要枚举注册表、读「启动」文件夹并
/// 解析快捷方式（COM），不该拖住窗口创建，也不该在主线程上初始化 COM。
pub(crate) fn sweep_stale_auto_start_entries() {
    let Some(exe_name) = std::env::current_exe().ok().and_then(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }) else {
        log::warn!("取不到当前可执行文件名，跳过失效启动项清理");
        return;
    };

    if let Err(error) = sweep_run_key(&exe_name) {
        log::warn!("清理注册表启动项失败: {error}");
    }

    // 解析快捷方式和取「启动」文件夹路径都要 COM，在这个后台线程上初始化一次。
    // 失败就只跳过「启动」文件夹这一半 —— 注册表那半已经做完了。
    let _com = match shortcut::ComApartment::initialize() {
        Ok(com) => com,
        Err(error) => {
            log::warn!("初始化 COM 失败，跳过「启动」文件夹清理: {error}");
            return;
        }
    };
    if let Err(error) = sweep_startup_folder(&exe_name) {
        log::warn!("清理「启动」文件夹失败: {error}");
    }
}

/// 清掉 Run 键里指向已失效 `deskcrate.exe` 的条目（不区分值名）。
fn sweep_run_key(exe_name: &str) -> AppResult<()> {
    // 枚举要 `KEY_READ`，删值只需 `KEY_SET_VALUE`；键不存在就没什么可清理的。
    let Some(key) = open_run_key(KEY_READ | KEY_SET_VALUE, false)? else {
        return Ok(());
    };
    let result = sweep_run_key_values(key, exe_name);
    unsafe {
        let _ = RegCloseKey(key);
    };
    result
}

fn sweep_run_key_values(key: HKEY, exe_name: &str) -> AppResult<()> {
    let mut index = 0u32;
    let mut buffer = vec![0u16; VALUE_NAME_CAPACITY];

    loop {
        let mut name_length = u32::try_from(buffer.len()).unwrap_or(0);
        let status = unsafe {
            RegEnumValueW(
                key,
                index,
                PWSTR(buffer.as_mut_ptr()),
                &mut name_length,
                None,
                None,
                None,
                None,
            )
        };

        if status == ERROR_NO_MORE_ITEMS {
            return Ok(());
        }
        if status == ERROR_MORE_DATA {
            // 值名长过缓冲区，跳过后继续 —— 不是我们写得出来的名字。
            index += 1;
            continue;
        }
        if status != ERROR_SUCCESS {
            return Err(AppError::new(
                "auto_start_enum_failed",
                "无法枚举注册表启动项",
            ));
        }

        let value_name = String::from_utf16_lossy(&buffer[..name_length as usize]);
        let command_line = read_string(key, &value_name)?.unwrap_or_default();

        if !should_sweep(&command_line, exe_name) {
            index += 1;
            continue;
        }

        match delete_value(key, &value_name) {
            Ok(()) => {
                log::info!("已清理失效的启动项 {value_name}（目标 {command_line} 已不存在）");
                // 删掉之后同一个下标会落到下一个值上，不能再自增。
            }
            Err(error) => {
                log::warn!("清不掉失效的启动项 {value_name}: {error}");
                index += 1;
            }
        }
    }
}

/// 清掉「启动」文件夹里指向 `deskcrate.exe` 的快捷方式。
fn sweep_startup_folder(exe_name: &str) -> AppResult<()> {
    let Some(folder) = startup_folder() else {
        return Ok(());
    };

    let entries = match fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(AppError::new(
                "startup_folder_unreadable",
                format!("读不到「启动」文件夹 {}: {error}", folder.display()),
            ));
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let is_shortcut = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"));
        if !is_shortcut {
            continue;
        }

        // 受管的 `DeskCrate.lnk` 由对账逻辑维护，不能在这里当成“重复项”删掉。
        if path.file_name().is_some_and(|name| {
            name.to_string_lossy()
                .eq_ignore_ascii_case(STARTUP_SHORTCUT_NAME)
        }) {
            continue;
        }

        let parsed = match shortcut::parse_shortcut(&path) {
            Ok(parsed) => parsed,
            // 解析不了就无法判断它是不是我们的，留着不动。
            Err(error) => {
                log::debug!(
                    "跳过无法解析的启动快捷方式 {}: {}",
                    path.display(),
                    error.message
                );
                continue;
            }
        };
        if !is_our_executable(Path::new(&parsed.target_path), exe_name) {
            continue;
        }

        // 目标存在与否都删：
        // - 存在 → 会和我们受管的快捷方式一起把程序拉起两次，屏幕上出现两个侧栏；
        // - 不存在 → 每次开机弹一个「找不到文件」。
        match fs::remove_file(&path) {
            Ok(()) => {
                log::info!(
                    "已清理「启动」文件夹里的 {}（目标 {}）",
                    path.display(),
                    parsed.target_path
                );
                if let Some(file_name) = path.file_name() {
                    let file_name = file_name.to_string_lossy();
                    if let Err(error) =
                        delete_startup_approval(STARTUP_APPROVED_STARTUP_SUBKEY, &file_name)
                    {
                        log::debug!(
                            "清理启动快捷方式的 StartupApproved 标记失败 {}: {error}",
                            file_name
                        );
                    }
                }
            }
            Err(error) => log::warn!("删不掉启动快捷方式 {}: {error}", path.display()),
        }
    }

    Ok(())
}

/// 取「启动」文件夹路径。
///
/// 用 `FOLDERID_Startup` 而不是拼 `%APPDATA%\...\Startup`：后者在用户通过策略
/// 挪过 Start Menu 的机器上会指向一个不存在的目录，于是清理静默失效。
fn startup_folder() -> Option<PathBuf> {
    let raw =
        unsafe { SHGetKnownFolderPath(&FOLDERID_Startup, KNOWN_FOLDER_FLAG(0), HANDLE::default()) }
            .ok()?;

    let path = unsafe { raw.to_string() }.ok().map(PathBuf::from);
    // `SHGetKnownFolderPath` 用 `CoTaskMemAlloc` 分配，必须配 `CoTaskMemFree`，
    // 否则每次启动漏一小块内存。
    unsafe { CoTaskMemFree(Some(raw.0 as *const std::ffi::c_void)) };

    path
}

/// 受管的自启动快捷方式路径。
fn startup_shortcut_path() -> Option<PathBuf> {
    startup_folder().map(|folder| folder.join(STARTUP_SHORTCUT_NAME))
}

/// 一条 Run 值该不该被清理。
///
/// 两个条件同时成立才清理：目标文件名与我们同名（不误删别的程序），且目标已经
/// 不存在（清掉才是有意义的）。抽成纯函数是为了能用真实的 Run 键内容做回归 ——
/// 这个函数返回 true 就意味着真的会删注册表值。
fn should_sweep(command_line: &str, exe_name: &str) -> bool {
    let target = Path::new(executable_path_of(command_line));
    is_our_executable(target, exe_name) && !target.exists()
}

/// 从 Run 值里取出可执行文件路径。
///
/// 按 Windows 的命令行规则切：带引号就取引号内的部分（应对 `Program Files`
/// 这类带空格的路径），否则取到第一个空白为止。
fn executable_path_of(command_line: &str) -> &str {
    let trimmed = command_line.trim();
    match trimmed.strip_prefix('"') {
        Some(rest) => rest.split('"').next().unwrap_or(rest),
        None => trimmed.split_whitespace().next().unwrap_or(""),
    }
}

/// 路径的文件名是否与我们当前的可执行文件同名（不区分大小写）。
fn is_our_executable(candidate: &Path, exe_name: &str) -> bool {
    candidate
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(exe_name))
}

/// 打开 Run 键。
///
/// `create = false` 时，键不存在返回 `Ok(None)` —— 调用方据此判断「没有启动项」
/// 或「没什么可删的」。这一点很要紧：删除路径若回退到 `RegCreateKeyW`，一次
/// 「关掉自启动」反而会凭空建出一个 Run 键。
///
/// 只有写路径需要 `create = true`：Run 键本身可能被优化工具删掉了。
///
/// 写/删值只用 `KEY_SET_VALUE`，**不要**用 `KEY_WRITE`：`KEY_WRITE` 还包含
/// `KEY_CREATE_SUB_KEY`，而 Run 键的 ACL 可能拒绝建子键，导致整个
/// `RegOpenKeyExW` 返回 ACCESS_DENIED，连已经存在的值都改不了。
fn open_run_key(access: REG_SAM_FLAGS, create: bool) -> AppResult<Option<HKEY>> {
    open_key(RUN_SUBKEY, access, create)
}

fn open_key(subkey: &str, access: REG_SAM_FLAGS, create: bool) -> AppResult<Option<HKEY>> {
    let wide_subkey = wide::to_wide(subkey);
    let mut key = HKEY::default();

    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(wide_subkey.as_ptr()),
            0,
            access,
            &mut key,
        )
    };
    if status == ERROR_SUCCESS {
        return Ok(Some(key));
    }

    if status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND {
        if !create {
            return Ok(None);
        }
        let status =
            unsafe { RegCreateKeyW(HKEY_CURRENT_USER, PCWSTR(wide_subkey.as_ptr()), &mut key) };
        if status == ERROR_SUCCESS {
            return Ok(Some(key));
        }
    }

    Err(AppError::new(
        "auto_start_key_unavailable",
        format!("打不开注册表项 {subkey}"),
    ))
}

/// 读一个字符串值。值不存在时返回 `Ok(None)`，类型不匹配时报错。
fn read_string(key: HKEY, name: &str) -> AppResult<Option<String>> {
    let wide_name = wide::to_wide(name);
    let mut kind = REG_SZ;
    let mut size = 0u32;

    // 先问长度，第二次调用才真的把字节读出来。
    let status = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(wide_name.as_ptr()),
            None,
            Some(&mut kind),
            None,
            Some(&mut size),
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if status != ERROR_SUCCESS {
        return Err(AppError::new(
            "auto_start_read_failed",
            "读不到开机自启动的注册表值",
        ));
    }

    // 值存在但不是 `REG_SZ`：说明这条 Run 值不是我们写的，或者被别的程序改成了
    // 别的类型。当成没有值处理 —— 否则 `REG_DWORD` 的 4 字节会被当两个 UTF-16
    // 单元读成乱码。
    if kind != REG_SZ {
        return Ok(None);
    }

    // 长度不是偶数说明不是 UTF-16 的整数个单位，当成没有值处理。
    if size == 0 || size % 2 != 0 {
        return Ok(None);
    }

    let mut buffer = vec![0u8; size as usize];
    let status = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(wide_name.as_ptr()),
            None,
            Some(&mut kind),
            Some(buffer.as_mut_ptr()),
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(AppError::new(
            "auto_start_read_failed",
            "读不到开机自启动的注册表值",
        ));
    }

    let units: Vec<u16> = buffer
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    // 末尾的终止 null 丢掉。
    let end = units
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(units.len());

    Ok(Some(String::from_utf16_lossy(&units[..end])))
}

/// 读一个二进制值。返回类型和内容；值不存在为 `None`。
///
/// 类型不符时返回空内容，由调用方按 `Unknown` 处理，避免把别的类型当二进制。
fn read_binary(key: HKEY, name: &str) -> AppResult<Option<(REG_VALUE_TYPE, Vec<u8>)>> {
    let wide_name = wide::to_wide(name);
    let mut kind = REG_BINARY;
    let mut size = 0u32;

    let status = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(wide_name.as_ptr()),
            None,
            Some(&mut kind),
            None,
            Some(&mut size),
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if status != ERROR_SUCCESS {
        return Err(AppError::new(
            "auto_start_read_failed",
            "读不到 StartupApproved 的注册表值",
        ));
    }

    if kind != REG_BINARY {
        return Ok(Some((kind, Vec::new())));
    }
    if size == 0 {
        return Ok(Some((kind, Vec::new())));
    }

    let mut buffer = vec![0u8; size as usize];
    let status = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(wide_name.as_ptr()),
            None,
            Some(&mut kind),
            Some(buffer.as_mut_ptr()),
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(AppError::new(
            "auto_start_read_failed",
            "读不到 StartupApproved 的注册表值",
        ));
    }

    Ok(Some((kind, buffer)))
}

/// 写一个二进制值。
fn write_binary(key: HKEY, name: &str, value: &[u8]) -> AppResult<()> {
    let wide_name = wide::to_wide(name);
    let status =
        unsafe { RegSetValueExW(key, PCWSTR(wide_name.as_ptr()), 0, REG_BINARY, Some(value)) };

    if status != ERROR_SUCCESS {
        return Err(AppError::new(
            "auto_start_write_failed",
            "写不进 StartupApproved 的注册表值",
        ));
    }

    Ok(())
}

/// 删掉一个值。值本来就不存在算成功 —— 关自启动是幂等的。
fn delete_value(key: HKEY, name: &str) -> AppResult<()> {
    let wide_name = wide::to_wide(name);

    let status = unsafe { RegDeleteValueW(key, PCWSTR(wide_name.as_ptr())) };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    if status != ERROR_SUCCESS {
        return Err(AppError::new(
            "auto_start_delete_failed",
            "删不掉开机自启动的注册表值",
        ));
    }

    Ok(())
}

/// 给路径套上引号（仅测试用；生产不再写 HKCU Run）。
#[cfg(test)]
fn quote_path(path: &str) -> String {
    format!("\"{path}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    // 这里只测纯逻辑。真去读写 Run 键会改开发机的系统状态（凭空多出 / 少掉一条
    // 开机启动项），不该出现在自动化的测试套件里。

    #[test]
    fn auto_start_decision_follows_config_when_approval_is_not_disabled() {
        let elsewhere = AutoStartState::Elsewhere(PathBuf::from(r"C:\old\deskcrate.exe"));

        // 任务管理器没有禁用：沿用原来的配置优先逻辑。
        for approval in [
            StartupApproval::Missing,
            StartupApproval::Enabled,
            StartupApproval::Unknown,
        ] {
            assert_eq!(
                auto_start_action(true, &AutoStartState::MatchesCurrent, approval),
                AutoStartAction::LeaveAlone
            );
            assert_eq!(
                auto_start_action(false, &AutoStartState::MatchesCurrent, approval),
                AutoStartAction::TurnConfigOn
            );
            assert_eq!(
                auto_start_action(true, &AutoStartState::Absent, approval),
                AutoStartAction::Write
            );
            assert_eq!(
                auto_start_action(true, &elsewhere, approval),
                AutoStartAction::Write
            );
            assert_eq!(
                auto_start_action(false, &AutoStartState::Absent, approval),
                AutoStartAction::Delete
            );
            assert_eq!(
                auto_start_action(false, &elsewhere, approval),
                AutoStartAction::Delete
            );
        }
    }

    #[test]
    fn auto_start_decision_handles_task_manager_disabled() {
        let elsewhere = AutoStartState::Elsewhere(PathBuf::from(r"C:\old\deskcrate.exe"));

        // 系统禁用但 Run 值仍指向当前副本：配置跟随为关闭，保留禁用项。
        assert_eq!(
            auto_start_action(
                true,
                &AutoStartState::MatchesCurrent,
                StartupApproval::Disabled
            ),
            AutoStartAction::TurnConfigOffKeepEntry
        );
        assert_eq!(
            auto_start_action(
                false,
                &AutoStartState::MatchesCurrent,
                StartupApproval::Disabled
            ),
            AutoStartAction::LeaveAlone
        );
        // 系统禁用且 Run 值已缺失/指向别处：清掉残留并关配置。
        assert_eq!(
            auto_start_action(true, &AutoStartState::Absent, StartupApproval::Disabled),
            AutoStartAction::TurnConfigOffDeleteEntries
        );
        assert_eq!(
            auto_start_action(true, &elsewhere, StartupApproval::Disabled),
            AutoStartAction::TurnConfigOffDeleteEntries
        );
        assert_eq!(
            auto_start_action(false, &AutoStartState::Absent, StartupApproval::Disabled),
            AutoStartAction::Delete
        );
        assert_eq!(
            auto_start_action(false, &elsewhere, StartupApproval::Disabled),
            AutoStartAction::Delete
        );
    }

    #[test]
    fn startup_approval_bytes_are_parsed_defensively() {
        let enabled = [0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let disabled = [0x03, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        assert_eq!(
            startup_approval_from_bytes(&enabled),
            StartupApproval::Enabled
        );
        assert_eq!(
            startup_approval_from_bytes(&disabled),
            StartupApproval::Disabled
        );
        // 空值、未知首字节、短数据：不能据此关闭配置。
        assert_eq!(startup_approval_from_bytes(&[]), StartupApproval::Unknown);
        assert_eq!(
            startup_approval_from_bytes(&[0x01]),
            StartupApproval::Unknown
        );
    }

    #[test]
    fn same_file_check_tolerates_case_and_missing_paths() {
        let current = std::env::current_exe().expect("测试进程应当有可执行文件路径");

        // 同一个文件，大小写不同 —— Windows 路径不区分大小写。
        let upper = PathBuf::from(current.to_string_lossy().to_uppercase());
        assert!(
            points_at_same_file(&upper, &current),
            "大小写不同不该被当成指向别处，否则每次启动都会白重写注册表"
        );

        // 已删除的路径必须判为「不是同一个文件」—— 那正是需要修正的情况。
        let missing = current.with_file_name("deskcrate-does-not-exist-9f3a.exe");
        assert!(!points_at_same_file(&missing, &current));

        // 目录不是文件，`canonicalize` 能成功但两边不相等。
        let directory = current.parent().expect("可执行文件应当有父目录");
        assert!(!points_at_same_file(directory, &current));
    }

    #[test]
    fn quoted_paths_round_trip_through_the_quote_helper() {
        // 写入时套引号、读取时去掉，两端必须对称，否则比对永远不相等。
        let quoted = quote_path(r"C:\Program Files\DeskCrate\deskcrate.exe");
        assert_eq!(
            quoted.trim().trim_matches('"'),
            r"C:\Program Files\DeskCrate\deskcrate.exe"
        );
    }

    #[test]
    fn command_line_parsing_handles_quoted_unquoted_and_argument_forms() {
        // 我们自己写入的形式（带引号）。
        assert_eq!(
            executable_path_of(r#""C:\Program Files\DeskCrate\deskcrate.exe""#),
            r"C:\Program Files\DeskCrate\deskcrate.exe"
        );
        // 手工添加的常见形式：带引号 + 参数。
        assert_eq!(
            executable_path_of(r#""C:\A\deskcrate.exe" --minimized"#),
            r"C:\A\deskcrate.exe"
        );
        // 无引号、无空格。
        assert_eq!(
            executable_path_of(r"C:\A\deskcrate.exe"),
            r"C:\A\deskcrate.exe"
        );
        // 无引号但带参数：按 Windows 规则切到第一个空白。
        assert_eq!(
            executable_path_of(r"C:\A\deskcrate.exe --minimized"),
            r"C:\A\deskcrate.exe"
        );
        // 两端空白要忽略；空串不能 panic。
        assert_eq!(executable_path_of("   "), "");
        assert_eq!(executable_path_of(""), "");
    }

    #[test]
    fn executable_name_matching_is_case_insensitive_and_strict() {
        assert!(is_our_executable(
            Path::new(r"C:\A\deskcrate.exe"),
            "deskcrate.exe"
        ));
        assert!(is_our_executable(
            Path::new(r"C:\A\DeskCrate.EXE"),
            "deskcrate.exe"
        ));

        // 别的程序绝不能命中 —— 这个扫描会真的删注册表值。
        assert!(!is_our_executable(
            Path::new(r"C:\A\deskcrate-other.exe"),
            "deskcrate.exe"
        ));
        assert!(!is_our_executable(
            Path::new(r"C:\A\app.exe"),
            "deskcrate.exe"
        ));
        // 目录没有文件名，不能命中。
        assert!(!is_our_executable(Path::new(r"C:\A\"), "deskcrate.exe"));
        // 空命令行的解析结果不能命中。
        assert!(!is_our_executable(Path::new(""), "deskcrate.exe"));
    }

    #[test]
    fn sweep_decision_never_fires_on_foreign_startup_entries() {
        // 这些是真实开发机上 Run 键里的内容（2026-09-25 实测）。这个函数返回
        // true 就会真的删掉注册表值，所以拿真实数据当回归夹具：一个都不能命中。
        //
        // 注意其中 `E:\MuMu Player 12\...` 是**未加引号且带空格**的写法，
        // 按命令行规则会被切成 `E:\MuMu`。解析结果确实不对，但方向是安全的：
        // 切出来的文件名不是 `deskcrate.exe`，所以不会误删。这条用例把「解析
        // 不完美也不误删」这个性质钉住。
        let foreign = [
            r#""C:\Program Files\Microsoft OneDrive\OneDrive.exe" /background"#,
            r"c4732a97-536e-417d-88c1-61e45e7e33c6",
            r"0",
            r"E:\MuMu Player 12\shell\MuMuPlayerService.exe",
            r"C:\Users\lenovo\AppData\Local\Programs\utools\uTools.exe",
            r#""C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --win-session-start"#,
        ];
        for command_line in foreign {
            assert!(
                !should_sweep(command_line, "deskcrate.exe"),
                "不能命中别的程序: {command_line}"
            );
        }
    }

    #[test]
    fn sweep_decision_fires_only_for_missing_our_own_copy() {
        // 目标已不存在 → 该清。
        assert!(should_sweep(r#""C:\gone\deskcrate.exe""#, "deskcrate.exe"));
        // 目标存在（用当前测试进程自己当样本）→ 不清。
        let existing = std::env::current_exe().expect("测试进程应当有可执行文件路径");
        let exe_name = existing
            .file_name()
            .expect("可执行文件应当有文件名")
            .to_string_lossy()
            .into_owned();
        let command_line = quote_path(&existing.to_string_lossy());
        assert!(!should_sweep(&command_line, &exe_name));
    }
}
