// 开机自启动。
//
// Windows 的自启动不靠「配置里写个开关」，而是往当前用户的 Run 键里塞一条：
//
//     HKCU\Software\Microsoft\Windows\CurrentVersion\Run
//         DeskCrate = "C:\...\deskcrate.exe"
//
// 选 `HKCU` 而不是 `HKLM`：后者需要管理员权限，而侧栏这类常驻小程序没有必要
// 为了开机启动去要 UAC。代价是自启动只对当前用户生效 —— 这正好是期望行为。
//
// 注册表是**系统状态**，`config.json` 是**应用状态**，两者会各说各话（用户手动
// 删掉启动项、或者换了安装目录）。因此：
//
// - `update_settings` 里以配置为准，值变了就同步注册表；
// - 启动时 `reconcile_auto_start` 反过来对一次账。
//
// **对账不能只看「Run 值像不像一个可执行文件路径」**：程序被挪到别的目录之后，
// 旧路径可能仍然存在（用户留着旧副本），扩展名也对，但开机启动的不是用户正在
// 用的那个。所以必须拿 Run 值和 `current_exe()` 比对，见 `AutoStartState`。
//
// 读写都走 `windows` crate 的注册表 API，不引入 `winreg` 这类额外依赖。

use std::fs;
use std::path::{Path, PathBuf};

use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::{
            RegCloseKey, RegCreateKeyW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
            RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SAM_FLAGS, REG_SZ,
        },
    },
};

use crate::{error::AppResult, models::AppError, system::wide};

// 自启动项在注册表里的名字。
//
// 用固定名字而不是随机 GUID：用户想手动清启动项时，能在任务管理器或系统优化
// 工具里看出这是谁，删起来也有据可依。
const RUN_VALUE_NAME: &str = "DeskCrate";

const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Run 键里记录的启动项与**当前进程**的关系。
///
/// 这里刻意不用布尔：`value.ends_with(".exe")` 只能回答「值像不像一个可执行
/// 文件路径」，而程序被挪到别的目录后，旧路径往往仍然存在（用户留着旧副本），
/// 扩展名也对 —— 开机启动的却是另一个副本。必须和 `current_exe()` 比对才能
/// 发现这种情况。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AutoStartState {
    /// 没有值，或值是空串。
    Absent,
    /// 指向正在运行的这个副本。
    MatchesCurrent,
    /// 值存在但指向别处，含已被删除的路径。`.lnk` 快捷方式也落在这里 ——
    /// 统一改写成真实 exe 路径，比在启动路径里拉起 COM 去解析快捷方式划算。
    Elsewhere(PathBuf),
}

/// 对账时要执行的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoStartAction {
    /// 系统状态已符合配置，不动。
    LeaveAlone,
    /// 把 Run 值写成当前 exe 路径。
    Write,
    /// 删掉 Run 值。
    Delete,
}

/// 由「配置」和「注册表实际状态」推导该做什么。
///
/// 抽成纯函数是为了能直接测 —— 真去读写 Run 键会污染开发机的系统状态。
fn auto_start_action(configured: bool, state: &AutoStartState) -> AutoStartAction {
    match (configured, state) {
        // 已经一致，不制造多余的注册表写入。
        (true, AutoStartState::MatchesCurrent) => AutoStartAction::LeaveAlone,
        (false, AutoStartState::Absent) => AutoStartAction::LeaveAlone,
        // 配置说开：无论是没有值、还是指向别处（失效路径 / 另一个副本 / 快捷
        // 方式），都改写成当前副本 —— 这正是「换了安装目录后自启动还指着旧路径」
        // 的自愈路径。
        (true, _) => AutoStartAction::Write,
        // 配置说关：设置页显示关闭时，系统里就不该有我们的启动项。指向别处的
        // 条目一并清掉，否则「UI 说关、开机却仍会启动」会长期不一致。
        (false, _) => AutoStartAction::Delete,
    }
}

/// 打开或关闭开机自启动。
pub(crate) fn set_auto_start_enabled(enabled: bool) -> AppResult<()> {
    if !enabled {
        return delete_auto_start();
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
pub(crate) fn reconcile_auto_start(configured: bool) {
    // 取不到当前路径就没法判断「指向别处」，此时**放弃本次对账**而不是凭猜测
    // 去改系统状态。
    let current = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            log::warn!("取不到当前可执行文件路径，跳过自启动对账: {error}");
            return;
        }
    };

    let state = match auto_start_state(&current) {
        Ok(state) => state,
        Err(error) => {
            log::warn!("读不到开机自启动状态: {error}");
            return;
        }
    };

    match auto_start_action(configured, &state) {
        AutoStartAction::LeaveAlone => {}
        AutoStartAction::Write => match write_auto_start(&current) {
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
        },
        AutoStartAction::Delete => {
            if let Err(error) = delete_auto_start() {
                log::warn!("清理开机自启动失败: {error}");
            }
        }
    }
}

/// 读出 Run 值并判断它和 `current` 的关系。
fn auto_start_state(current: &Path) -> AppResult<AutoStartState> {
    // 读路径不创建键：Run 键被优化工具删掉时，「没有启动项」本身就是答案。
    let Some(key) = open_run_key(KEY_READ, false)? else {
        return Ok(AutoStartState::Absent);
    };
    let value = read_string(key, RUN_VALUE_NAME);
    // 句柄必须显式关掉：注册表句柄是内核对象，泄漏会攒到进程退出。
    unsafe {
        let _ = RegCloseKey(key);
    };

    let Some(value) = value? else {
        return Ok(AutoStartState::Absent);
    };
    // 我们自己写入的值带一对引号（应对 `Program Files` 这种带空格的路径）。
    let recorded = value.trim().trim_matches('"');
    if recorded.is_empty() {
        return Ok(AutoStartState::Absent);
    }

    let recorded = PathBuf::from(recorded);
    Ok(if points_at_same_file(&recorded, current) {
        AutoStartState::MatchesCurrent
    } else {
        AutoStartState::Elsewhere(recorded)
    })
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

/// 把 Run 值写成给定的可执行文件路径。
fn write_auto_start(executable: &Path) -> AppResult<()> {
    // 写路径允许创建键：Run 键本身可能被优化工具删掉了。
    let Some(key) = open_run_key(KEY_WRITE, true)? else {
        // `create = true` 时不会返回 `None`；真到了这里也不要 panic ——
        // GUI 进程没有控制台，崩了现场没有任何可诊断信息。
        return Err(AppError::new(
            "auto_start_key_unavailable",
            format!("打不开注册表项 {RUN_SUBKEY}"),
        ));
    };
    let result = write_string(
        key,
        RUN_VALUE_NAME,
        &quote_path(&executable.to_string_lossy()),
    );
    unsafe {
        let _ = RegCloseKey(key);
    };
    result
}

/// 删掉 Run 值。
fn delete_auto_start() -> AppResult<()> {
    // 不创建键：键不存在则值必然不存在，关闭自启动本来就是幂等的。
    // 用 `KEY_WRITE` + `create = false` 是为了不制造系统状态 —— 否则「关掉
    // 自启动」这个动作反而会凭空建出一个 Run 键。
    let Some(key) = open_run_key(KEY_WRITE, false)? else {
        return Ok(());
    };
    let result = delete_value(key, RUN_VALUE_NAME);
    unsafe {
        let _ = RegCloseKey(key);
    };
    result
}

/// 打开 Run 键。
///
/// `create = false` 时，键不存在返回 `Ok(None)` —— 调用方据此判断「没有启动项」
/// 或「没什么可删的」。这一点很要紧：删除路径若回退到 `RegCreateKeyW`，一次
/// 「关掉自启动」反而会凭空建出一个 Run 键。
///
/// 只有写路径需要 `create = true`：Run 键本身可能被优化工具删掉了。
fn open_run_key(access: REG_SAM_FLAGS, create: bool) -> AppResult<Option<HKEY>> {
    let subkey = wide::to_wide(RUN_SUBKEY);
    let mut key = HKEY::default();

    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            access,
            &mut key,
        )
    };
    if status == ERROR_SUCCESS {
        return Ok(Some(key));
    }

    if status == ERROR_FILE_NOT_FOUND {
        if !create {
            return Ok(None);
        }
        let status = unsafe { RegCreateKeyW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()), &mut key) };
        if status == ERROR_SUCCESS {
            return Ok(Some(key));
        }
    }

    Err(AppError::new(
        "auto_start_key_unavailable",
        format!("打不开注册表项 {RUN_SUBKEY}"),
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

/// 写一个字符串值。
fn write_string(key: HKEY, name: &str, value: &str) -> AppResult<()> {
    let wide_name = wide::to_wide(name);
    let bytes = wide::to_wide_bytes(value);

    let status =
        unsafe { RegSetValueExW(key, PCWSTR(wide_name.as_ptr()), 0, REG_SZ, Some(&bytes)) };

    if status != ERROR_SUCCESS {
        return Err(AppError::new(
            "auto_start_write_failed",
            "写不开机自启动的注册表值",
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

/// 给路径套上引号。
///
/// Run 键里的命令行是按空格切的，路径带空格（`C:\Program Files\...`）不加引号
/// 会被切成好几段，开机时系统根本找不到程序。没空格也套上，省一次判断。
fn quote_path(path: &str) -> String {
    format!("\"{path}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    // 这里只测纯逻辑。真去读写 Run 键会改开发机的系统状态（凭空多出 / 少掉一条
    // 开机启动项），不该出现在自动化的测试套件里。

    #[test]
    fn action_table_covers_every_config_and_registry_combination() {
        let elsewhere = AutoStartState::Elsewhere(PathBuf::from(r"C:\old\deskcrate.exe"));

        // 配置说开：已指向当前副本就不动，其余一律改写成当前副本。
        assert_eq!(
            auto_start_action(true, &AutoStartState::MatchesCurrent),
            AutoStartAction::LeaveAlone
        );
        assert_eq!(
            auto_start_action(true, &AutoStartState::Absent),
            AutoStartAction::Write
        );
        // 这一格是「换了目录后自启动还指着旧路径」的自愈路径。
        assert_eq!(auto_start_action(true, &elsewhere), AutoStartAction::Write);

        // 配置说关：已经没有条目就不动，其余一律清掉 ——
        // 包括指向别处的条目，避免「UI 说关、开机却仍会启动」。
        assert_eq!(
            auto_start_action(false, &AutoStartState::Absent),
            AutoStartAction::LeaveAlone
        );
        assert_eq!(
            auto_start_action(false, &AutoStartState::MatchesCurrent),
            AutoStartAction::Delete
        );
        assert_eq!(
            auto_start_action(false, &elsewhere),
            AutoStartAction::Delete
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
}
