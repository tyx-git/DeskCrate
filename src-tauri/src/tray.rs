// 托盘入口：窗口隐藏任务栏按钮，只靠托盘图标提供「展开 / 退出」。
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

use crate::{services, state::AppState};

const EXPAND_ID: &str = "expand";
const QUIT_ID: &str = "quit";

// 创建并托管托盘图标。
//
// `TrayIcon` 是引用计数的，最后一个实例 drop 时图标会从托盘移除；因此必须
// `manage` 到 App 里，不能只留一个局部变量。
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let expand = MenuItem::with_id(app, EXPAND_ID, "展开", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, QUIT_ID, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&expand, &quit_item])?;

    let builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("DeskCrate")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            EXPAND_ID => show_main_window(app),
            QUIT_ID => quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    let icon = Image::from_bytes(include_bytes!("../icons/32x32.png"))?;
    let tray = builder.icon(icon).build(app)?;
    app.manage(tray);
    Ok(())
}

fn show_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(services::window::MAIN_WINDOW_LABEL) else {
        return;
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

fn quit(app: &AppHandle) {
    // 先保存几何，再退出；ExitRequested/Exit 触发时窗口可能已经销毁，
    // 这里还在事件处理中，能读到窗口。
    if let Some(window) = app.get_webview_window(services::window::MAIN_WINDOW_LABEL) {
        if let Some(state) = app.try_state::<AppState>() {
            if let Err(error) = services::window::persist_window_state(state.inner(), &window) {
                log::warn!("托盘退出前保存窗口状态失败: {error}");
            }
        }
    }
    app.exit(0);
}
