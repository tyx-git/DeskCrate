use crate::{
    error::AppResult,
    models::{ResizeWindowRequest, ViewMode},
    services,
    state::AppState,
};

// 初始化主窗口并返回本次启动实际采用的位置和大小。
//
// 首次启动按当前显示器工作区推导悬浮窗几何，随后把结果写入
// `config.json`；后续启动恢复已保存的窗口状态。
#[tauri::command]
pub fn is_init<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    state: tauri::State<'_, AppState>,
) -> AppResult<services::window::WindowInitResponse> {
    services::window::is_init(state.inner(), &window)
}

// 切换侧栏的展开态与收起态。
//
// 前端只提交目标视图模式（`"expanded"` / `"collapsed"`），窗口几何由
// 后端按当前工作区推导并持久化，前端不需要接触窗口尺寸 API。
#[tauri::command]
pub fn apply_view_mode<R: tauri::Runtime>(
    view_mode: ViewMode,
    window: tauri::WebviewWindow<R>,
    state: tauri::State<'_, AppState>,
) -> AppResult<services::window::WindowInitResponse> {
    services::window::apply_view_mode(state.inner(), &window, view_mode)
}

// 按前端拖拽把手的实时结果改窗口几何。
//
// 前端在 `pointermove` 里连续调用，`x` / `y` / `width` / `height` 都是设计稿
// CSS 像素。缩放结果由尺寸监听器按「用户拖拽」记录并节流落盘，这里不写配置。
#[tauri::command]
pub fn resize_window<R: tauri::Runtime>(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    window: tauri::WebviewWindow<R>,
) -> AppResult<()> {
    services::window::resize_window(
        &window,
        ResizeWindowRequest {
            x,
            y,
            width,
            height,
        },
    )
}

// 缩放把手松手时补写一次尺寸。
//
// 拖拽期间的尺寸变更由 `Resized` 监听器按 300ms 节流落盘，最后一段位移很
// 可能正卡在节流窗口里。松手后不会再有尺寸变化，这里显式补写，保证记住的
// 就是用户最终拖出来的那个尺寸。
#[tauri::command]
pub fn finish_window_resize(state: tauri::State<'_, AppState>) -> AppResult<()> {
    services::window::flush_user_resize(state.inner())
}
