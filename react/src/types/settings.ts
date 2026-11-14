// 配置与窗口模型：与 src-tauri/src/models/settings.rs 一一对应。

/** 应用主题，当前只有默认一种。 */
export type Theme = 'default'

/** 侧栏显示状态。 */
export type ViewMode = 'expanded' | 'collapsed'

/** 窗口客户区尺寸，单位物理像素。 */
export interface WindowSize {
  width: number
  height: number
}

/** 设计稿坐标下的尺寸，单位 CSS 像素，与显示器缩放比无关。 */
export interface CssSize {
  width: number
  height: number
}

/** 窗口左上角位置，单位物理像素；多显示器下可为负数。 */
export interface WindowPosition {
  x: number
  y: number
}

/** 窗口几何，单位设计稿 CSS 像素。 */
export interface WindowGeometry {
  x: number
  y: number
  width: number
  height: number
}

export interface WindowSettings {
  isInitialized: boolean
  position: WindowPosition | null
  /** 物理像素。 */
  size: WindowSize | null
  /** 用户拖拽出的尺寸覆盖，CSS 像素；收起态永远忽略。 */
  sizeOverride: CssSize | null
}

export interface Settings {
  revision: number
  theme: Theme
  window: WindowSettings
  viewMode: ViewMode
  autoCollapse: boolean
  autoStart: boolean
}

/** is_init 与 apply_view_mode 的返回结构。 */
export interface WindowInitResponse {
  isInitialized: boolean
  position: WindowPosition
  /** 物理像素。 */
  size: WindowSize
}