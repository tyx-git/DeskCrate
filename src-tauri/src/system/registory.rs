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
// - 启动时 `reconcile_auto_start` 反过来对一次账，配置说开但注册表没有就补上。
//
// 读写都走 `windows` crate 的注册表 API，不引入 `winreg` 这类额外依赖。

use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::{
            RegCloseKey, RegCreateKeyW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
            RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ,
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

/// 当前是否已设置开机自启动。
pub(crate) fn is_auto_start_enabled() -> AppResult<bool> {
    let key = open_run_key(KEY_READ)?;
    let value = read_string(key, RUN_VALUE_NAME);
    // 句柄必须显式关掉：注册表句柄是内核对象，泄漏会攒到进程退出。
    unsafe {
        let _ = RegCloseKey(key);
    };
    let value = value?;

    // 只认「存在且指向一个可执行文件」的值。留空串、或指向已被删掉的路径都算
    // 没开 —— 那种情况下系统启动时也只会弹一个找不到文件的错误框。
    //
    // 我们自己写入的值带一对引号（应对 `Program Files` 这种带空格的路径），
    // 先把命令行两端的引号去掉再判断扩展名。
    Ok(value.is_some_and(|value| {
        value
            .trim()
            .trim_matches('"')
            .to_ascii_lowercase()
            .ends_with(".exe")
    }))
}

/// 打开或关闭开机自启动。
pub(crate) fn set_auto_start_enabled(enabled: bool) -> AppResult<()> {
    if !enabled {
        let key = open_run_key(KEY_WRITE)?;
        let result = delete_value(key, RUN_VALUE_NAME);
        unsafe {
            let _ = RegCloseKey(key);
        };
        return result;
    }

    let executable = std::env::current_exe().map_err(|error| {
        AppError::new(
            "auto_start_exe_unavailable",
            format!("取不到当前可执行文件路径: {error}"),
        )
    })?;

    let key = open_run_key(KEY_WRITE)?;
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

/// 按需补齐开机自启动：配置说开、注册表却没有就补上。
///
/// 启动时调用。失败只记日志 —— 自启动是锦上添花，不该让侧栏起不来。
pub(crate) fn reconcile_auto_start(configured: bool) {
    let current = match is_auto_start_enabled() {
        Ok(current) => current,
        Err(error) => {
            log::warn!("读不到开机自启动状态: {error}");
            return;
        }
    };

    if current == configured {
        return;
    }

    if let Err(error) = set_auto_start_enabled(configured) {
        log::warn!("补齐开机自启动失败: {error}");
    }
}

/// 打开（不存在则创建）Run 键。
fn open_run_key(access: windows::Win32::System::Registry::REG_SAM_FLAGS) -> AppResult<HKEY> {
    let subkey = wide::to_wide(RUN_SUBKEY);
    let mut key = HKEY::default();

    // Run 键正常总是存在；即使被优化工具删了，先用 KEY_WRITE 打开会失败，
    // 因此回退到 `RegCreateKeyW` 把它建回来。读路径则只打开，不创造系统状态。
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            access,
            &mut key,
        )
    };
    if status == ERROR_FILE_NOT_FOUND && access == KEY_WRITE {
        let status = unsafe { RegCreateKeyW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()), &mut key) };
        if status != ERROR_SUCCESS {
            return Err(AppError::new(
                "auto_start_key_unavailable",
                format!("打不开注册表项 {RUN_SUBKEY}"),
            ));
        }
        return Ok(key);
    }

    if status != ERROR_SUCCESS {
        return Err(AppError::new(
            "auto_start_key_unavailable",
            format!("打不开注册表项 {RUN_SUBKEY}"),
        ));
    }

    Ok(key)
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
