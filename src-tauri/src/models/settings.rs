use serde::{Deserialize, Serialize};

// 展示页和收起态 HTML 中使用的固定视觉参数。
// 这些常量属于前端 CSS 设计规范，不会被序列化到 `config.json`。
// 前端实现 CSS 时应以这些值为准，用户可修改的配置才放入 `Settings`。
pub mod css {
    pub const SIDEBAR_LEFT_PX: u32 = 2; //展开侧栏距离左侧的距离。
    pub const SIDEBAR_TOP_PERCENT: f32 = 12.5; //展开侧栏距离顶部的百分比。
    pub const SIDEBAR_WIDTH_PERCENT: f32 = 18.75; //展开侧栏占宽度的百分比。
    pub const SIDEBAR_HEIGHT_PERCENT: f32 = 50.0; //展开侧栏占高度的百分比。
    pub const SIDEBAR_VERTICAL_PADDING_PX: u32 = 10; //展开侧栏上下内边距。
    pub const SIDEBAR_BORDER_RADIUS_PX: u32 = 10; //展开侧栏圆角半径。
    pub const SIDEBAR_SCROLLBAR_WIDTH_PX: u32 = 4; //展开侧栏滚动条宽度。
    pub const SIDEBAR_BACKDROP_BLUR_PX: u32 = 28; //展开侧栏背景模糊半径。

    pub const COLLAPSED_STRIP_TOP_PERCENT: f32 = 12.5; //收起指示条距离场景顶部的百分比。
    pub const COLLAPSED_STRIP_WIDTH_PX: u32 = 3; //收起指示条宽度。
    pub const COLLAPSED_STRIP_HEIGHT_PX: u32 = 80; //收起指示条高度。

    pub const SEARCH_MARGIN_TOP_PERCENT: f32 = 2.0; //搜索框上外边距百分比。
    pub const SEARCH_MARGIN_HORIZONTAL_PERCENT: f32 = 4.0; //搜索框左右外边距百分比。
    pub const SEARCH_MARGIN_BOTTOM_PERCENT: f32 = 1.5; //搜索框下外边距百分比。
    pub const DIVIDER_HEIGHT_PX: u32 = 1; //分隔线高度。

    pub const ICON_GRID_GAP_MIN_PX: u32 = 4; //图标网格间距的最小值。
    pub const ICON_GRID_GAP_VIEWPORT_PERCENT: f32 = 0.8; //图标网格间距的视口宽度比例。
    pub const ICON_GRID_GAP_MAX_PX: u32 = 12; //图标网格间距的最大值。
    pub const ICON_ITEM_WIDTH_MIN_PX: u32 = 36; //单个图标项宽度的最小值。
    pub const ICON_ITEM_WIDTH_VIEWPORT_PERCENT: f32 = 4.0; //单个图标项宽度的视口宽度比例。
    pub const ICON_ITEM_WIDTH_MAX_PX: u32 = 64; //单个图标项宽度的最大值。
    pub const ICON_EMOJI_SIZE_MIN_PX: u32 = 24; //图标容器尺寸的最小值。
    pub const ICON_EMOJI_SIZE_VIEWPORT_PERCENT: f32 = 3.0; //图标容器尺寸的视口宽度比例。
    pub const ICON_EMOJI_SIZE_MAX_PX: u32 = 44; //图标容器尺寸的最大值。
}

// 应用主题。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    Default, // 默认主题，中性柔白主题。
}

// 侧栏当前的显示状态。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ViewMode {
    #[default]
    Expanded, // 展开态：显示完整前端、分类和应用图标。
    Collapsed, // 收起态：只显示收起指示条。
}

// 应用窗口的实际尺寸。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WindowSize {
    pub width: u32,  // 窗口宽度，单位为像素。
    pub height: u32, // 窗口高度，单位为像素。
}

// 应用窗口在桌面上的实际位置。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WindowPosition {
    pub x: i32, // 窗口左上角的水平坐标，允许为负数。
    pub y: i32, // 窗口左上角的垂直坐标，允许为负数。
}

// 应用窗口的统一配置。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct WindowSettings {
    pub is_initialized: bool, // 是否已经完成首次窗口初始化；用于决定首次居中还是恢复已保存参数。
    pub position: Option<WindowPosition>, // 已保存的窗口位置；None 表示首次启动时居中。
    pub size: Option<WindowSize>, // 已保存的窗口宽高；None 表示首次启动，由窗口层计算并居中。
}

// DeskCrate 的持久化应用配置，对应 `config.json`。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub theme: Theme,           // 当前应用主题。
    pub window: WindowSettings, // 主窗口的位置和尺寸配置。
    pub view_mode: ViewMode,    // 侧栏当前是展开态还是收起态。
    pub auto_collapse: bool,    // 是否允许鼠标离开侧栏后自动切换为收起态。
}
