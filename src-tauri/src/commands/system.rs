use crate::{error::AppResult, services, state::AppState};

// 启动数据树中指定 ID 的应用或快捷方式。
//
// 前端只传递稳定的应用 ID；路径、参数和工作目录由后端从
// `data.json` 读取，`Child` 和 Windows 句柄不会暴露给前端。
#[tauri::command]
pub fn launch_icon(id: String, state: tauri::State<'_, AppState>) -> AppResult<()> {
    services::system::launch_app(state.inner(), &id)
}
