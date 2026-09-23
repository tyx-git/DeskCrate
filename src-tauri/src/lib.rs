// Tauri Builder、插件、Command 注册和后端初始化

pub mod commands;
pub mod error;
pub mod models;
pub(crate) mod path;
pub mod services;
pub mod state;
pub mod storage;
pub(crate) mod system;
pub(crate) mod text;

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
            if let Err(error) = store.ensure_layout() {
                // 数据目录不可写时（例如被安装到 `Program Files` 下却没有提权），
                // 直接返回 Err 只会让 Tauri 在无控制台的 GUI 进程里静默 panic，
                // 用户看到的是「双击了图标但什么都没发生」。先弹原生对话框说明原因。
                system::dialog::show_startup_error(
                    "DeskCrate 无法启动",
                    &format!(
                        "无法在程序目录下创建数据目录：\n{}\n\n{error}\n\n\
                         DeskCrate 需要在程序所在目录下创建 config/ 与 icons/。\n\
                         请把它放到有写入权限的位置（例如用户目录），\n\
                         不要放在 Program Files 等受保护目录中。",
                        store.path().display()
                    ),
                );
                return Err(Box::new(error) as Box<dyn std::error::Error>);
            }
            // 把图标缓存目录注册进 asset protocol 作用域。
            //
            // `tauri.conf.json` 里写的 `$EXE/icons/**` 在 Windows 上**永远不会生效**：
            // Tauri 把 `BaseDirectory::Executable` 解析为 `dirs::executable_dir()`，
            // 而该函数在 Windows 恒返回 `None`（见 dirs 6.0 文档的表格），于是
            // `PathResolver::parse` 返回 `Err`，`Scope::new` 里的 `if let Ok(path)`
            // 会**静默丢弃**这条配置 —— 作用域变成空集，所有图标请求都被以
            // 「asset protocol not configured to allow the path」拒绝，日志里
            // 没有任何「你的 scope 配错了」的线索。
            //
            // 程序目录只有运行时才知道，配置里也没有在 Windows 上等价的变量
            // （`$RESOURCE` 指向资源目录、`$APPDATA` 指向用户目录，都不是程序目录），
            // 所以只能在这里用运行时 API 补上。
            //
            // `Scope` 内部是 `Arc<ScopeInner>`，此处取到的克隆与协议处理器持有的是
            // 同一份，注册后立即生效，无需重启窗口。
            let icons_dir = store.icons_path().to_path_buf();
            app.asset_protocol_scope()
                .allow_directory(&icons_dir, true)?;

            app.manage(state::AppState::new(store));

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // 启动时对账图标缓存，清掉异常路径留下的孤儿 PNG。
            //
            // 正常删除应用/分组/分类都会同步清理图标，这里只兜住进程崩溃、
            // 文件被占用导致删除失败等情况 —— 这些孤儿在界面上完全看不出来。
            // 对账失败不该拖住启动，因此只记日志、不向上返回错误。
            match services::catalog::sweep_orphan_icons(app.state::<state::AppState>().inner()) {
                Ok(removed) if !removed.is_empty() => {
                    log::info!("启动对账清理了 {} 个孤儿图标: {removed:?}", removed.len());
                }
                Ok(_) => {}
                Err(error) => log::warn!("图标缓存对账失败: {error}"),
            }

            // 开机自启动对账：配置说开就确保 Run 键里有我们，说关就确保没有。
            // 与图标对账同理，失败只记日志 —— 不该因为注册表不可写就起不来。
            if let Err(error) =
                services::settings::reconcile_auto_start(app.state::<state::AppState>().inner())
            {
                log::warn!("开机自启动对账失败: {error}");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 诊断命令只在 debug 构建里注册，见 `commands::diagnostics`。
            #[cfg(debug_assertions)]
            commands::diagnostics::probe_url_icon,
            commands::catalog::get_all_data,
            commands::catalog::add_category,
            commands::catalog::rename_category,
            commands::catalog::delete_category,
            commands::catalog::add_group,
            commands::catalog::rename_group,
            commands::catalog::delete_group,
            commands::catalog::set_node_collapsed,
            commands::catalog::add_app,
            commands::catalog::add_apps,
            commands::catalog::move_app,
            commands::catalog::move_group,
            commands::catalog::rename_app,
            commands::catalog::delete_app,
            commands::settings::get_settings,
            commands::settings::update_auto_collapse,
            commands::settings::update_auto_start,
            commands::settings::update_theme,
            commands::settings::clear_window_size_override,
            commands::settings::update_settings,
            commands::window::is_init,
            commands::window::apply_view_mode,
            commands::window::resize_window,
            commands::window::finish_window_resize,
            commands::system::launch_icon
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::WindowEvent { label, event, .. } = &event {
                // 尺寸变化要单独处理：用户拖拽边框改尺寸时不经过任何 Command，
                // 只能在这里接住。`capture_user_resize` 负责把它和程序下发的
                // 尺寸区分开，详见 `services::window`。
                if label == services::window::MAIN_WINDOW_LABEL
                    && matches!(event, tauri::WindowEvent::Resized(_))
                {
                    services::window::handle_window_resized(app_handle);
                    return;
                }
            }

            // 窗口几何在 `is_init` 与 `apply_view_mode` 里已经持久化过，这里的退出
            // 保存是兜底：用户拖拽改出来的尺寸不会经过那两个入口，最终几何只能在
            // 关闭前这一刻拿到。
            //
            // 必须挂在 `CloseRequested` 上。实测 `ExitRequested` / `Exit` 触发时窗口
            // 已经销毁，`get_webview_window("main")` 返回 None，读不到
            // inner_position / inner_size；这两个事件只作为兜底再试一次。
            let should_save = match &event {
                tauri::RunEvent::WindowEvent { label, event, .. } => {
                    label == services::window::MAIN_WINDOW_LABEL
                        && matches!(event, tauri::WindowEvent::CloseRequested { .. })
                }
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => true,
                _ => false,
            };
            if !should_save {
                return;
            }

            let Some(window) = app_handle.get_webview_window(services::window::MAIN_WINDOW_LABEL)
            else {
                // 走 `CloseRequested` 正常关闭时这里已经保存过了，兜底事件重复触发时
                // 窗口不在属于预期情况，不告警。
                log::debug!("退出时主窗口已销毁，跳过窗口状态保存");
                return;
            };

            let Some(state) = app_handle.try_state::<state::AppState>() else {
                log::warn!("退出时找不到 AppState，无法保存窗口状态");
                return;
            };

            if let Err(error) = services::window::persist_window_state(state.inner(), &window) {
                log::error!("退出时保存窗口状态失败: {error}");
            }
        });
}
