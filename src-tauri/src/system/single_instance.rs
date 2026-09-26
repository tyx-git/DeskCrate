// 单实例防护。
//
// 两个进程各弹一个侧栏是本应用最糟糕的失败形态：窗口是透明、无边框、置顶的悬浮
// 层，两份会精确叠在同一位置，用户看不出有两个 —— 直到拖走上面那个，下面那个还
// 留在原地。所以第二次启动必须直接退出。
//
// 检测用命名互斥量而不是锁文件：互斥量由内核随进程销毁自动释放，进程崩溃也不会
// 留下需要判断是否过期的残留文件；锁文件方案则要么依赖 exe 目录可写，要么在异常
// 退出后留下一份分不清死活的文件。
//
// 互斥量名不带前缀时落在**会话命名空间**，正好是想要的粒度：
//
// - 快速用户切换下，两个登录用户各自能跑一个实例；
// - 同一用户下，不管从哪个目录启动都只允许一个。
//
// 代价是开发机上 `target/debug` 与 `target/release` 会互相排斥，跑其中一个之前
// 要先关掉另一个。

use std::fs;
use std::path::{Path, PathBuf};

use windows::Win32::{
    Foundation::{CloseHandle, GetLastError, BOOL, ERROR_ALREADY_EXISTS, HANDLE, HWND, LPARAM},
    UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowThreadProcessId, SetForegroundWindow, ShowWindow,
        SW_SHOW,
    },
};

use crate::system::wide;

// 单实例检测与「取某个进程的可执行文件路径」需要的三个 kernel32 函数本可以走
// `windows` crate 的 `Win32_System_Threading`，但那会连带启用 `Win32_Security`
// （`CreateMutexW` 的签名带 `SECURITY_ATTRIBUTES`）。这两个大模块让 `windows`
// crate 的链接参数显著变长，**越过 rustc 的阈值后会改走 @-file 回退路径，而那条
// 路径上链接器环境是丢的**，构建直接失败。实测两种表现：
//
//   - PATH 上先命中 Git Bash 的 GNU coreutils `link.exe` →
//     `link: missing operand after '\377\376'`
//   - 手动把 MSVC 的 link.exe 放进 PATH → `LNK1181: 无法打开输入文件 kernel32.lib`
//
// 三个函数签名都很短，自己声明比换来一个坏掉的构建划算。窗口相关的函数仍在
// 已启用的 `Win32_UI_WindowsAndMessaging` 里，照常使用。
//
// 参数用裸指针而不是 `HANDLE`：这个块因此只依赖 `core`，不与 `windows` crate
// 的类型纠缠。
#[link(name = "kernel32")]
extern "system" {
    /// 创建或打开命名互斥量。对象已存在时返回有效句柄，并把 `GetLastError`
    /// 置为 `ERROR_ALREADY_EXISTS`。
    fn CreateMutexW(
        lp_mutex_attributes: *const core::ffi::c_void,
        b_initial_owner: i32,
        lp_name: *const u16,
    ) -> *mut core::ffi::c_void;

    /// 打开一个已存在的进程对象。
    fn OpenProcess(
        dw_desired_access: u32,
        b_inherit_handle: i32,
        dw_process_id: u32,
    ) -> *mut core::ffi::c_void;

    /// 取进程可执行文件的完整路径。
    fn QueryFullProcessImageNameW(
        h_process: *mut core::ffi::c_void,
        dw_flags: u32,
        lp_exe_name: *mut u16,
        lpdw_size: *mut u32,
    ) -> i32;
}

/// `PROCESS_QUERY_LIMITED_INFORMATION`。
///
/// 用它而不是 `PROCESS_QUERY_INFORMATION`：前者对更高完整性级别的进程也能拿到
/// 路径，后者会失败 —— 我们的窗口进程和对方可能不在同一个权限层级上。
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

/// `QueryFullProcessImageNameW` 的 `dwFlags`：取 Win32 格式路径。
const PROCESS_NAME_WIN32: u32 = 0;

/// 互斥量名。不带前缀 → 会话命名空间，同一用户下全局唯一。
const MUTEX_NAME: &str = "DeskCrate.SingleInstance";

/// Tauri 2 给主窗口注册的窗口类名。
///
/// 来源：`tauri-runtime-wry` 里的 `builder.window_classname("Tauri Window")`。
/// 它可以用 `tauri.conf.json` 的 `windowClassname` 覆盖，本项目没有设置，所以这里
/// 是常量。真被覆盖了也只是「找不到窗口、不去激活」，不会出错。
const TAURI_WINDOW_CLASS: &str = "Tauri Window";

/// 取窗口类名用的缓冲区长度（字符）。类名上限是 256。
const CLASS_NAME_CAPACITY: usize = 256;

/// 取进程可执行文件路径用的缓冲区长度（字符）。
///
/// 用宽路径上限而不是 `MAX_PATH`：程序完全可能装在超过 260 字符的目录下，
/// 缓冲区不够时 `QueryFullProcessImageNameW` 会失败，我们就匹配不到已有实例。
const PROCESS_PATH_CAPACITY: usize = 32_768;

/// 单实例守卫。
///
/// 必须活到进程结束 —— 一旦被 drop，句柄关闭、互斥量释放，第二个实例就又能启动了。
/// 因此它的生命周期要覆盖 `run()` 的整个函数体（那里会阻塞到应用退出）。
pub(crate) struct SingleInstance {
    /// 只用来持有句柄，从不读取。进程退出时内核会关掉它并释放互斥量。
    _mutex: HANDLE,
}

/// 尝试取得单实例所有权。
///
/// 返回 `None` 表示已有实例在跑 —— 调用方应当**立刻退出，不要进入 `setup`**：
/// 第二个实例跑一遍 `setup` 会重复建目录、并发执行启动项清理、并弹出第二个侧栏。
pub(crate) fn acquire() -> Option<SingleInstance> {
    let name = wide::to_wide(MUTEX_NAME);
    // `bInitialOwner` 传 0：我们只用互斥量当「有没有人在跑」的标志，不需要拥有
    // 它。不拥有也就不会有「线程退出导致互斥量被遗弃」这套语义要处理。
    let mutex = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };

    if mutex.is_null() {
        // 拿不到互斥量本身不该让程序起不来 —— 宁可多开一次，也不能让用户双击
        // 之后什么都没有。
        log::warn!(
            "创建单实例互斥量失败，跳过单实例防护: {}",
            unsafe { GetLastError() }.0
        );
        return Some(SingleInstance {
            _mutex: HANDLE::default(),
        });
    }

    // `CreateMutexW` 对「对象已存在」也返回有效句柄，靠 `GetLastError` 区分。
    let already_running = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    if already_running {
        // 这是「已存在」的那一个，我们不拥有它，句柄也没用了。
        unsafe {
            let _ = CloseHandle(HANDLE(mutex));
        }
        return None;
    }

    Some(SingleInstance {
        _mutex: HANDLE(mutex),
    })
}

/// 把已有实例的窗口调到前台。
///
/// 只在检测到已有实例时调用。窗口是透明置顶的悬浮层，通常已经可见，所以这一步
/// 大多时候是 no-op；它的价值在于兜住「第一个实例的前端启动失败、窗口从没
/// `show()` 出来」这种情况 —— 否则第二次双击会毫无反应，用户不知道为什么。
pub(crate) fn activate_existing_window() {
    let Some(target) = current_executable() else {
        log::debug!("取不到当前可执行文件路径，跳过激活已有实例");
        return;
    };

    let mut search = WindowSearch {
        target: &target,
        found: None,
    };
    // `EnumWindows` 的回调不能捕获环境，上下文只能通过 `LPARAM` 传进去。
    let _ = unsafe {
        EnumWindows(
            Some(find_our_window),
            LPARAM(std::ptr::addr_of_mut!(search) as isize),
        )
    };

    let Some(window) = search.found else {
        log::debug!("已有实例在运行，但没找到它的主窗口");
        return;
    };

    unsafe {
        // 前端没来得及 `show()` 时窗口还是隐藏的，先显出来再抢前台。
        let _ = ShowWindow(window, SW_SHOW);
        let _ = SetForegroundWindow(window);
    }
    log::info!("已有实例在运行，已把它的窗口调到前台");
}

/// `EnumWindows` 的上下文。
struct WindowSearch<'a> {
    target: &'a Path,
    found: Option<HWND>,
}

/// `EnumWindows` 回调：找出属于我们、且类名是 Tauri 主窗口的那个窗口。
///
/// 返回 `TRUE` 表示继续枚举 —— 即使已经找到也要跑完，中途返回 `FALSE` 会让
/// `EnumWindows` 报错，白白多一条告警。
unsafe extern "system" fn find_our_window(window: HWND, parameter: LPARAM) -> BOOL {
    let search = unsafe { &mut *(parameter.0 as *mut WindowSearch<'_>) };
    if search.found.is_some() {
        return BOOL(1);
    }

    // **类名这一关不能省。** 同一个进程的顶层窗口不止一个，实测有：
    //   class "Tao Thread Event Target"  事件目标窗口（`IsWindowVisible` 为真）
    //   class "Tauri Window"             真正的悬浮层
    //   class "MSCTFIME UI" / "IME"      输入法
    // 只按进程匹配会挑到事件目标窗口，`SetForegroundWindow` 打上去毫无反应 ——
    // 表现为「调用成功但窗口没动」。
    if window_class_name(window).as_deref() != Some(TAURI_WINDOW_CLASS) {
        return BOOL(1);
    }

    let mut process_id = 0u32;
    unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    if process_id == 0 {
        return BOOL(1);
    }

    // 类名相同的别的 Tauri 应用也要排除，所以再比对一次可执行文件路径。
    if process_executable(process_id).as_deref() == Some(search.target) {
        search.found = Some(window);
    }
    BOOL(1)
}

/// 当前进程的可执行文件路径（已归一化）。
fn current_executable() -> Option<PathBuf> {
    let path = std::env::current_exe().ok()?;
    Some(fs::canonicalize(&path).unwrap_or(path))
}

/// 某个进程的可执行文件路径（已归一化）。
///
/// 用 `PROCESS_QUERY_LIMITED_INFORMATION` 而不是 `PROCESS_QUERY_INFORMATION`：
/// 前者对更高完整性级别的进程也能拿到路径，而后者会失败 —— 我们的窗口进程和
/// 对方可能不在同一个权限层级上。
fn process_executable(process_id: u32) -> Option<PathBuf> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if process.is_null() {
        return None;
    }

    let mut buffer = vec![0u16; PROCESS_PATH_CAPACITY];
    let mut length = u32::try_from(buffer.len()).ok()?;
    let succeeded = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            buffer.as_mut_ptr(),
            &mut length,
        )
    };
    // 句柄是内核对象，无论成败都要关掉。
    unsafe {
        let _ = CloseHandle(HANDLE(process));
    }
    if succeeded == 0 {
        return None;
    }

    let path = PathBuf::from(String::from_utf16_lossy(&buffer[..length as usize]));
    Some(fs::canonicalize(&path).unwrap_or(path))
}

/// 取窗口类名。取不到（窗口已销毁等）返回 `None`。
fn window_class_name(window: HWND) -> Option<String> {
    let mut buffer = vec![0u16; CLASS_NAME_CAPACITY];
    let length = unsafe { GetClassNameW(window, &mut buffer) };
    if length <= 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..length as usize]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutex_and_window_class_names_are_the_documented_values() {
        // 互斥量名变了会让「旧版本还开着、新版本又启动」这种升级场景短暂失效，
        // 属于要有意识去改的东西。
        assert_eq!(MUTEX_NAME, "DeskCrate.SingleInstance");
        // 类名来自 `tauri-runtime-wry` 的 `builder.window_classname(...)`。
        // Tauri 升级后如果这个字符串变了，激活已有窗口会静默失效 —— 这条断言
        // 就是提醒。
        assert_eq!(TAURI_WINDOW_CLASS, "Tauri Window");
    }

    #[test]
    fn process_executable_of_self_matches_current_executable() {
        // 这两个函数是「按 exe 路径认出自己人」的判定基础，必须对当前进程成立，
        // 否则激活已有窗口永远找不到目标。
        let expected = current_executable().expect("当前进程应当有可执行文件路径");
        let actual = process_executable(std::process::id()).expect("应当能查到自己的可执行文件");
        assert_eq!(actual, expected);
    }

    #[test]
    fn process_executable_rejects_unknown_process() {
        // 不存在的 PID 必须返回 `None` 而不是 panic 或猜一个路径。
        assert!(process_executable(u32::MAX).is_none());
    }
}
