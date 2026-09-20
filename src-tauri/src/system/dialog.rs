use std::ffi::OsStr;

use windows::{
    core::PCWSTR,
    Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK, MB_SETFOREGROUND},
};

use crate::system::wide;

// 启动阶段的致命错误提示。
//
// 主窗口是 `visible: false` 且无边框的悬浮层，GUI 进程也没有控制台，所以
// `Builder::setup` 返回 Err 时 Tauri 打出的 panic 信息用户完全看不到——表现
// 为「双击了图标但什么都没发生」。这里用原生消息框把原因直接说清楚。
//
// 只在「没有任何其他渠道能告知用户」的启动失败路径上使用。调用后进程随即
// 退出，因此不需要考虑与主窗口的交互，也不需要异步或返回值。
pub(crate) fn show_startup_error(title: &str, message: &str) {
    // 复用系统层统一的 UTF-16 转换，顺带做嵌入 NUL 的校验。
    let (Ok(title), Ok(message)) = (
        wide::nul_terminated(OsStr::new(title), "startup_dialog_title", "标题"),
        wide::nul_terminated(OsStr::new(message), "startup_dialog_message", "内容"),
    ) else {
        return;
    };

    // SAFETY: 两个缓冲区在调用期间都存活且以 NUL 结尾；`hwnd` 传 None 表示
    // 无属主窗口，消息框会挂到当前线程的活动窗口上（与 `ShellExecuteW` 的
    // 调用方式一致）。返回值只表示用户点了哪个按钮，这里不关心。
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
        );
    }
}
