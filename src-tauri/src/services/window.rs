use crate::{
    error::AppResult,
    models::{WindowPosition, WindowSize},
    state::AppState,
    storage::SettingsRepository,
};

// 主窗口初始化时返回给前端的状态。
//
// `is_initialized` 表示本次调用完成后窗口已经具有可持久化的状态；
// 位置和大小是本次实际应用到窗口的结果。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInitResponse {
    pub is_initialized: bool, // 本次调用完成后是否已经初始化并持久化窗口状态。
    pub position: WindowPosition, // 本次启动采用的窗口左上角位置。
    pub size: WindowSize,     // 本次启动采用的窗口客户区大小。
}

// 首次启动的窗口默认大小。
//
// Tauri 配置中的窗口宽高只是创建窗口时的兜底值；真正的首次尺寸由
// 当前显示器工作区计算。这里保留最小值，避免极小工作区出现零尺寸。
const MIN_WINDOW_DIMENSION: u32 = 1;

// 使用 Tauri 窗口和显示器信息决定本次启动的位置和大小。
//
// 配置文件只负责持久化，窗口对象只负责实际应用。这个 service 不
// 直接持有 `MutexGuard`，因此不会把存储锁跨到 Tauri API 调用中。
pub fn initialize_window<R: tauri::Runtime>(
    state: &AppState,
    window: &tauri::WebviewWindow<R>,
) -> AppResult<WindowInitResponse> {
    let settings = {
        let store = state.get_lock()?;
        store.load_settings()?
    };

    let needs_initialization = !settings.window.is_initialized;
    let (position, size) = if needs_initialization {
        first_launch_geometry(window)?
    } else {
        let position = settings
            .window
            .position
            .ok_or_else(|| invalid_window_state("已初始化配置缺少窗口位置"))?;
        let size = settings
            .window
            .size
            .ok_or_else(|| invalid_window_state("已初始化配置缺少窗口大小"))?;
        validate_window_size(size)?;
        (position, size)
    };

    apply_geometry(window, position, size)?;
    if needs_initialization {
        persist_geometry(state, position, size)?;
    }

    Ok(WindowInitResponse::new(position, size))
}

// 保存窗口退出前的实际客户区位置和大小。
//
// 这是退出事件调用的后端内部函数，不暴露给前端。读取窗口失败时不
// 覆盖已有配置；写入配置失败则把错误交给调用方记录。
pub fn persist_window_state(
    state: &AppState,
    window: &tauri::WebviewWindow<impl tauri::Runtime>,
) -> AppResult<()> {
    let position = window.inner_position().map_err(|error| {
        crate::models::AppError::new("window_position_read_failed", error.to_string())
    })?;
    let size = window.inner_size().map_err(|error| {
        crate::models::AppError::new("window_size_read_failed", error.to_string())
    })?;

    let position = WindowPosition {
        x: position.x,
        y: position.y,
    };
    let size = WindowSize {
        width: size.width.max(MIN_WINDOW_DIMENSION),
        height: size.height.max(MIN_WINDOW_DIMENSION),
    };

    persist_geometry(state, position, size)
}

impl WindowInitResponse {
    fn new(position: WindowPosition, size: WindowSize) -> Self {
        Self {
            is_initialized: true,
            position,
            size,
        }
    }
}

// 统一持久化窗口几何信息，避免首次初始化和退出保存分别维护同一组字段。
fn persist_geometry(state: &AppState, position: WindowPosition, size: WindowSize) -> AppResult<()> {
    let store = state.get_lock()?;
    store.mutate_settings(|settings| {
        settings.window.is_initialized = true;
        settings.window.position = Some(position);
        settings.window.size = Some(size);
        Ok(())
    })
}

fn first_launch_geometry<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
) -> AppResult<(WindowPosition, WindowSize)> {
    let monitor = window
        .current_monitor()
        .map_err(|error| crate::models::AppError::new("monitor_read_failed", error.to_string()))?
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or_else(|| invalid_window_state("无法获取当前显示器信息"))?;

    let work_area = monitor.work_area();
    Ok(centered_geometry(
        work_area.position.x,
        work_area.position.y,
        work_area.size.width,
        work_area.size.height,
    ))
}

// 根据显示器工作区计算首次启动时的窗口几何信息。
//
// 这是一个不依赖 Tauri 窗口对象的纯函数：窗口服务在真实启动时把
// `Monitor::work_area` 的参数传进来，测试层也可以直接验证多显示器和
// 负坐标场景。首次窗口宽高均为工作区的八分之一，位置为工作区居中。
pub fn centered_geometry(
    work_area_x: i32,
    work_area_y: i32,
    work_area_width: u32,
    work_area_height: u32,
) -> (WindowPosition, WindowSize) {
    let width = (work_area_width / 8).max(MIN_WINDOW_DIMENSION);
    let height = (work_area_height / 8).max(MIN_WINDOW_DIMENSION);
    let x = work_area_x + ((work_area_width.saturating_sub(width)) / 2) as i32;
    let y = work_area_y + ((work_area_height.saturating_sub(height)) / 2) as i32;

    (WindowPosition { x, y }, WindowSize { width, height })
}

fn apply_geometry<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    position: WindowPosition,
    size: WindowSize,
) -> AppResult<()> {
    window
        .set_size(tauri::PhysicalSize::new(size.width, size.height))
        .map_err(|error| {
            crate::models::AppError::new("window_size_apply_failed", error.to_string())
        })?;
    window
        .set_position(tauri::PhysicalPosition::new(position.x, position.y))
        .map_err(|error| {
            crate::models::AppError::new("window_position_apply_failed", error.to_string())
        })?;
    Ok(())
}

fn validate_window_size(size: WindowSize) -> AppResult<()> {
    if size.width == 0 || size.height == 0 {
        return Err(invalid_window_state("已保存的窗口宽度和高度必须大于 0"));
    }

    Ok(())
}

fn invalid_window_state(message: impl Into<String>) -> crate::models::AppError {
    crate::models::AppError::new("invalid_window_state", message)
}

// 保留既有服务层入口；实际状态机逻辑集中在 `initialize_window`。
pub fn is_init(
    state: &AppState,
    window: &tauri::WebviewWindow<impl tauri::Runtime>,
) -> AppResult<WindowInitResponse> {
    initialize_window(state, window)
}
