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
    // 所有窗口规格由 `react/src/spec/window.json` 生成，React、CSS 和 Rust
    // 不再各自维护一份会漂移的窗口几何常量。
    include!(concat!(env!("OUT_DIR"), "/window_spec.rs"));
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

// 更新自动收起配置时由前端提交的字段级请求。
//
// 这里只接收目标字段，避免旧的 Settings 快照覆盖窗口状态或其他配置。
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAutoCollapseRequest {
    pub enabled: bool, // 是否允许鼠标离开侧栏后自动收起。
}

// 更新开机自启动配置时由前端提交的字段级请求。
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAutoStartRequest {
    pub enabled: bool, // 是否随 Windows 登录自动启动。
}

// 更新主题配置时由前端提交的字段级请求。
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateThemeRequest {
    pub theme: Theme, // 要保存的主题。
}

// DeskCrate 的持久化应用配置，对应 `config.json`。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub revision: u64,          // 配置修订号；整体更新接口用它拒绝过期快照。
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
