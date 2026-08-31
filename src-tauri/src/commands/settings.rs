use crate::{error::AppResult, models::Settings, services, state::AppState};

// 获取全部应用配置。
#[tauri::command]
pub fn get_settings(state: tauri::State<'_, AppState>) -> AppResult<Settings> {
    services::settings::get_settings(state.inner())
}

// 更新并持久化全部应用配置，返回保存后的配置。
#[tauri::command]
pub fn update_settings(
    settings: Settings,
    state: tauri::State<'_, AppState>,
) -> AppResult<Settings> {
    services::settings::update_settings(state.inner(), settings)
}
