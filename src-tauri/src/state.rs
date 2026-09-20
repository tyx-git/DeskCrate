use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use crate::{
    error::AppResult,
    models::{AppError, CssSize, ViewMode, WindowSize},
    storage::JsonStore,
};

// 待落盘的用户尺寸覆盖变更。
//
// 用枚举而不是 `Option<Option<CssSize>>`：外层表示「有没有待落盘的变更」，
// 内层表示「设成某个尺寸」还是「清掉覆盖」。拖回设计稿尺寸时必须能表达
// 「清掉」，否则覆盖值会一直留在配置里，窗口再也回不到设计稿尺寸。
pub enum OverrideChange {
    // 用户拖出来的尺寸。`size` 是落盘的覆盖值，按设计稿 CSS 像素记录，
    // 因此与缩放比无关，换到缩放比不同的显示器上仍然成立。
    //
    // 另外带上它换算出的物理尺寸 `applied`，用于同步 `window.size` 这份缓存：
    // 补写发生在切换视图模式、退出这些时刻，调用方手里没有窗口对象，读不到
    // 当前缩放比，换算必须在捕获拖拽时就做掉。
    Set { size: CssSize, applied: WindowSize },
    Clear,
}

// 窗口几何的运行时状态，只存在于内存，不写进 `config.json`。
//
// 存在的唯一理由是区分「这次尺寸变化是程序下发的还是用户拖出来的」：
// `WindowEvent::Resized` 不携带来源信息，只能靠这里记下的「刚下发了什么」
// 反推。窗口创建、`is_init` 摆位、切换视图模式、`show()` 被系统顶宽都会
// 触发同一个事件，没有这份记录就无法把它们和用户拖拽分开，用户尺寸覆盖
// 会被启动流程里的瞬时尺寸污染。
#[derive(Default)]
pub struct WindowRuntime {
    // 最近一次由程序下发的尺寸。None 表示几何正在下发途中 —— 这段时间里
    // 到达的 `Resized` 一律不可信（改 `resizable` 会改窗口样式，Windows
    // 可能顺手重排尺寸并派发事件）。
    pub last_applied: Option<WindowSize>,
    pub view_mode: ViewMode, // 最近一次由程序下发的视图模式。
    pub pending_override: Option<OverrideChange>, // 已确认是用户拖出来的变更，还没落盘。
    pub override_written_at: Option<Instant>, // 最近一次把覆盖值写进配置的时刻，用于节流。
}

pub struct AppState {
    store: Mutex<JsonStore>,      // 私有成员，只能通过get_lock函数获取锁
    window: Mutex<WindowRuntime>, // 私有成员，只能通过window_runtime函数获取锁
}

impl AppState {
    pub fn new(store: JsonStore) -> Self {
        Self {
            store: Mutex::new(store),
            window: Mutex::new(WindowRuntime::default()),
        }
    }
    pub(crate) fn get_lock(&self) -> AppResult<MutexGuard<'_, JsonStore>> {
        self.store.lock().map_err(|_| {
            log::error!("AppState 的共享存储锁已中毒");
            AppError::new("get_lock_failed", "无法获取内存锁")
        })
    }

    pub(crate) fn window_runtime(&self) -> AppResult<MutexGuard<'_, WindowRuntime>> {
        self.window.lock().map_err(|_| {
            log::error!("AppState 的窗口运行时锁已中毒");
            AppError::new("window_runtime_lock_failed", "无法获取内存锁")
        })
    }
}
