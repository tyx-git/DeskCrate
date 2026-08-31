// Tauri Builder、插件、Command 注册和后端初始化

pub mod commands;
pub mod error;
pub mod models;
pub mod services;
pub mod state;
pub mod storage;
pub(crate) mod system;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::Manager;

    tauri::Builder::default()
        .setup(|app| {
            let exe_path = std::env::current_exe()?;
            let exe_dir = exe_path
                .parent()
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::NotFound, "无法获取程序所在目录")
                })?
                .to_path_buf();
            let store = storage::JsonStore::new(exe_dir);
            store
                .ensure_layout()
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
            app.manage(state::AppState::new(store));

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::catalog::get_all_data,
            commands::catalog::add_category,
            commands::catalog::rename_category,
            commands::catalog::delete_category,
            commands::catalog::add_group,
            commands::catalog::rename_group,
            commands::catalog::delete_group,
            commands::catalog::add_app,
            commands::catalog::rename_app,
            commands::catalog::delete_app,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::window::is_init,
            commands::system::launch_icon
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // `is_init` 由前端在窗口准备好后调用；这里仅处理进程退出时
            // 的持久化，避免 setup 阶段和前端状态机重复初始化窗口。
            //
            // `ExitRequested` 早于窗口销毁，优先在这里保存；保留 `Exit`
            // 作为极端退出路径的重试机会。
            static WINDOW_STATE_SAVED: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);

            let should_save = matches!(
                event,
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
            ) && !WINDOW_STATE_SAVED.load(std::sync::atomic::Ordering::Acquire);

            if !should_save {
                return;
            }

            let Some(window) = app_handle.get_webview_window("main") else {
                log::warn!("退出时找不到主窗口，无法保存窗口状态");
                return;
            };

            let Some(state) = app_handle.try_state::<state::AppState>() else {
                log::warn!("退出时找不到 AppState，无法保存窗口状态");
                return;
            };

            if let Err(error) = services::window::persist_window_state(state.inner(), &window) {
                log::error!("退出时保存窗口状态失败: {error}");
            } else {
                WINDOW_STATE_SAVED.store(true, std::sync::atomic::Ordering::Release);
            }
        });
}
