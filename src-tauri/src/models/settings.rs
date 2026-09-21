use serde::{Deserialize, Serialize};

// 展示页和收起态 HTML 中使用的固定视觉参数。
//
// 这是**设计规格在代码里的副本**：数值来自 `dosc/plan/展示页.html` 与
// `dosc/plan/收起态.html`。其中窗口几何那几个（位置百分比、尺寸上下限、留白）
// 会被 `services::window` 真正读取，其余纯视觉细节目前只是规格留档 —— **即使没有
// Rust 代码引用也不要删**，它们的价值是「改设计时三处需要同步」的其中一处锚点。
// 真正生效的样式在 `react/src/index.css` 的 `@theme` 里。
//
// 这些常量不会被序列化到 `config.json`；用户可修改的配置才放入 `Settings`。
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

    // 以下常量描述主窗口自身的形状。侧栏是一个浮在桌面上的玻璃面板，
    // 窗口尺寸并不等于侧栏尺寸：窗口需要在侧栏包围盒之外留出一圈透明
    // 留白，CSS 的 box-shadow 才不会被窗口边界裁掉。
    pub const OVERLAY_SHADOW_GUTTER_PX: u32 = 16; //窗口在侧栏右侧和上下两侧多留的透明像素。
    pub const COLLAPSED_STRIP_LEFT_PX: u32 = 0; //收起指示条距离场景左侧的距离。

    // 悬浮窗的尺寸夹取范围。工作区极小时保证侧栏仍然可用，工作区极大时
    // 避免侧栏铺满半个屏幕。
    pub const OVERLAY_WIDTH_MIN_PX: u32 = 220; //展开态窗口宽度下限。
    pub const OVERLAY_WIDTH_MAX_PX: u32 = 480; //展开态窗口宽度上限。
    pub const OVERLAY_HEIGHT_MIN_PX: u32 = 320; //展开态窗口高度下限。
    pub const OVERLAY_HEIGHT_MAX_PX: u32 = 900; //展开态窗口高度上限。
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

// 应用窗口的实际尺寸，单位**物理像素**。
//
// 与 `CssSize` 刻意分成两个类型而不是共用一个：两者形状相同、单位不同，
// 混用不会报错，只会让尺寸在高 DPI 屏上莫名其妙地差一个缩放比。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WindowSize {
    pub width: u32,  // 窗口宽度，单位为物理像素。
    pub height: u32, // 窗口高度，单位为物理像素。
}

// 设计稿坐标下的尺寸，单位 **CSS 像素**。
//
// 与 `css` 模块里的常量同单位，因此与显示器缩放比无关：用户尺寸覆盖必须
// 按这个单位存储，否则换到缩放比不同的显示器上会被按新缩放比重新解释 ——
// 150% 屏上拖出来的 657 物理像素，到 100% 屏上就变成 657 CSS 像素，直接
// 顶到宽度上限。存 CSS 像素则换屏后仍是用户当初想要的那个大小。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CssSize {
    pub width: u32,  // 设计稿坐标下的宽度。
    pub height: u32, // 设计稿坐标下的高度。
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
    pub size_override: Option<CssSize>, // 用户在展开态拖拽出的窗口尺寸；None 表示按设计稿推导。
}

// 前端拖拽缩放把手时提交的窗口几何。
//
// 四个值都用设计稿 CSS 像素：前端手里的鼠标坐标、`innerWidth/innerHeight`
// 本来就都是 CSS 像素，让它在自己的单位里把几何算完再交给后端换算，比让
// 它先猜一次当前缩放比更不容易错。
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResizeWindowRequest {
    pub x: i32,     // 窗口左上角横坐标（屏幕坐标）。
    pub y: i32,     // 窗口左上角纵坐标（屏幕坐标）。
    pub width: u32, // 客户区宽度。
    pub height: u32,
}

// DeskCrate 的持久化应用配置，对应 `config.json`。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub theme: Theme,           // 当前应用主题。
    pub window: WindowSettings, // 主窗口的位置和尺寸配置。
    pub view_mode: ViewMode,    // 侧栏当前是展开态还是收起态。
    pub auto_collapse: bool,    // 是否允许鼠标离开侧栏后自动切换为收起态。
    /// 是否开机自启动。
    ///
    /// 这个字段**不是**配置说了算就完了：它同时要写进 Windows 的
    /// `HKCU\...\CurrentVersion\Run`，否则系统不会启动我们。写入注册表的动作
    /// 在 `services::settings::update_settings` 里完成，启动时再由
    /// `reconcile_auto_start` 对一次账。
    pub auto_start: bool,
}
