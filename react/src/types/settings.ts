/**
 * 配置与窗口模型。
 *
 * 与 `src-tauri/src/models/settings.rs` 一一对应，对应 `config.json`。
 */

/** 应用主题。当前只有中性柔白一种。 */
export type Theme = 'default';

/** 侧栏显示状态。 */
export type ViewMode = 'expanded' | 'collapsed';

/** 窗口客户区尺寸，单位物理像素。 */
export interface WindowSize {
  width: number;
  height: number;
}

/**
 * 设计稿坐标下的尺寸，单位 CSS 像素。
 *
 * 与 `WindowSize` 形状相同、单位不同，刻意分开两个类型而不是共用一个：
 * CSS 像素与显示器缩放比无关，物理像素只在测量它的那块屏幕上成立。
 * 设计稿常量（`src-tauri/src/models/settings.rs::css`）用的也是这个单位。
 */
export interface CssSize {
  width: number;
  height: number;
}

/** 窗口左上角位置，单位物理像素；多显示器下 x/y 可以为负数。 */
export interface WindowPosition {
  x: number;
  y: number;
}

export interface WindowSettings {
  /** 是否已经完成首次窗口初始化。 */
  isInitialized: boolean;
  /** 已保存的位置；`null` 表示首次启动。 */
  position: WindowPosition | null;
  /** 已保存的尺寸，单位物理像素；`null` 表示首次启动。 */
  size: WindowSize | null;
  /**
   * 用户在展开态拖拽边缘改出来的尺寸，单位 **CSS 像素**（设计稿坐标）。
   *
   * `null` 表示没有覆盖，展开态按设计稿的百分比推导；收起态永远忽略它。
   * 由后端在捕获到拖拽时写入，前端只负责清空（见 `resetWindowSize`）。
   *
   * 存 CSS 像素而不是物理像素，是为了让这份值跨显示器仍然成立：物理像素
   * 只在拖拽时那块屏幕上成立，换到缩放比不同的屏幕会被按新缩放比重新解释。
   */
  sizeOverride: CssSize | null;
}

export interface Settings {
  theme: Theme;
  window: WindowSettings;
  viewMode: ViewMode;
  /** 鼠标离开侧栏后是否自动收起。 */
  autoCollapse: boolean;
  /** 是否随 Windows 登录自动启动。 */
  autoStart: boolean;
}

/** `is_init` 与 `apply_view_mode` 的返回结构。 */
export interface WindowInitResponse {
  isInitialized: boolean;
  position: WindowPosition;
  size: WindowSize;
}
