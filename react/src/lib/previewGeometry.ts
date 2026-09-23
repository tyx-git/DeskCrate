import { OVERLAY_HEIGHT_MAX, OVERLAY_HEIGHT_MIN, OVERLAY_WIDTH_MAX, OVERLAY_WIDTH_MIN } from './overlaySize'
import type { ViewMode } from '../types/settings'
import { WINDOW_SPEC } from '../spec/windowSpec'

/**
 * 浏览器预览用的窗口几何计算。
 *
 * 这段逻辑是 `src-tauri/src/services/window.rs::overlay_geometry` 的镜像，
 * 只为在没有 Tauri 运行时的情况下把侧栏摆到设计稿的位置上。真实运行时
 * 窗口尺寸完全由 Rust 侧决定，前端不参与计算。
 *
 * 常量需要与 Rust 的 `models::settings::css` 保持一致。**单位是这里唯一
 * 容易出错的地方**：浏览器里 CSS 像素就是最终单位，所以下面直接用设计稿
 * 的数值即可；而 Rust 侧 `set_size` / `set_position` 收的是物理像素，必须
 * 先把这些常量乘上窗口缩放比（见 `overlay_geometry` 的 `scale_factor`）。
 * 别为了「对齐两边」而在 Rust 侧去掉那次换算 —— 在 150% 缩放的屏幕上，
 * 收起态窗口会因此只剩 19 个物理像素宽，装不下 16px 留白，指示条被压成
 * 0 宽而彻底消失。
 */
export interface PreviewBox {
  left: number
  top: number
  width: number
  height: number
}

export function previewWindowBox(
  sceneWidth: number,
  sceneHeight: number,
  viewMode: ViewMode,
): PreviewBox {
  const clamp = (value: number, min: number, max: number) =>
    Math.min(Math.max(value, min), Math.max(min, max))

  const [panelLeft, panelTop, panelWidth, panelHeight] =
    viewMode === 'expanded'
      ? [
          WINDOW_SPEC.sidebarLeftPx,
          (sceneHeight * WINDOW_SPEC.sidebarTopPercent) / 100,
          clamp((sceneWidth * WINDOW_SPEC.sidebarWidthPercent) / 100, OVERLAY_WIDTH_MIN, OVERLAY_WIDTH_MAX),
          clamp((sceneHeight * WINDOW_SPEC.sidebarHeightPercent) / 100, OVERLAY_HEIGHT_MIN, OVERLAY_HEIGHT_MAX),
        ]
      : [
          0,
          (sceneHeight * WINDOW_SPEC.collapsedStripTopPercent) / 100,
          WINDOW_SPEC.collapsedStripWidthPx,
          WINDOW_SPEC.collapsedStripHeightPx,
        ]

  return {
    // 窗口左边缘贴场景左边缘：面板自身的 2px 左边距由 `.dc-window-expanded`
    // 的 padding-left 提供，`panelLeft` 只参与窗口宽度计算。
    left: 0,
    top: panelTop - WINDOW_SPEC.overlayShadowGutterPx,
    width: panelLeft + panelWidth + WINDOW_SPEC.overlayShadowGutterPx,
    height: panelHeight + WINDOW_SPEC.overlayShadowGutterPx * 2,
  }
}
