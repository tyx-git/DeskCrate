use crate::{error::AppResult, services, state::AppState};

// 初始化主窗口并返回本次启动实际采用的位置和大小。
//
// 首次启动使用当前显示器工作区的八分之一尺寸并居中，随后把结果
// 写入 `config.json`；后续启动恢复已保存的窗口状态。
#[tauri::command]
pub fn is_init<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    state: tauri::State<'_, AppState>,
) -> AppResult<services::window::WindowInitResponse> {
    services::window::is_init(state.inner(), &window)
}
