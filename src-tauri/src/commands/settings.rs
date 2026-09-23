use crate::{
    error::AppResult,
    models::{Settings, UpdateAutoCollapseRequest, UpdateAutoStartRequest, UpdateThemeRequest},
    services,
    state::AppState,
};

// 获取全部应用配置。
#[tauri::command]
pub fn get_settings(state: tauri::State<'_, AppState>) -> AppResult<Settings> {
    services::settings::get_settings(state.inner())
}

// 按字段更新自动收起配置。
#[tauri::command]
pub fn update_auto_collapse(
    request: UpdateAutoCollapseRequest,
    state: tauri::State<'_, AppState>,
) -> AppResult<Settings> {
    services::settings::update_auto_collapse(state.inner(), request)
}

// 按字段更新开机自启动配置。
#[tauri::command]
pub fn update_auto_start(
    request: UpdateAutoStartRequest,
    state: tauri::State<'_, AppState>,
) -> AppResult<Settings> {
    services::settings::update_auto_start(state.inner(), request)
}

// 按字段更新主题配置。
#[tauri::command]
pub fn update_theme(
    request: UpdateThemeRequest,
    state: tauri::State<'_, AppState>,
) -> AppResult<Settings> {
    services::settings::update_theme(state.inner(), request)
}

// 清除展开态窗口尺寸覆盖。
#[tauri::command]
pub fn clear_window_size_override(state: tauri::State<'_, AppState>) -> AppResult<Settings> {
    services::settings::clear_window_size_override(state.inner())
}

// 更新并持久化全部应用配置，返回保存后的配置。
#[tauri::command]
pub fn update_settings(
    settings: Settings,
    state: tauri::State<'_, AppState>,
) -> AppResult<Settings> {
    services::settings::update_settings(state.inner(), settings)
}
