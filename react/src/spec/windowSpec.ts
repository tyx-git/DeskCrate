// 窗口规格唯一权威副本：Rust build.rs 也读取这份 JSON 生成常量。
import rawSpec from './window.json'

export const WINDOW_SPEC = rawSpec

/** 把窗口规格注入 CSS 自定义属性，样式不再复制几何数字。 */
export function applyWindowSpecCssVariables(): void {
  const root = document.documentElement
  const values: Record<string, string> = {
    '--dc-sidebar-left': `${WINDOW_SPEC.sidebarLeftPx}px`,
    '--dc-gutter': `${WINDOW_SPEC.overlayShadowGutterPx}px`,
    '--dc-panel-radius': `${WINDOW_SPEC.sidebarBorderRadiusPx}px`,
    '--dc-blur': `${WINDOW_SPEC.sidebarBackdropBlurPx}px`,
    '--dc-scrollbar-width': `${WINDOW_SPEC.sidebarScrollbarWidthPx}px`,
    '--dc-strip-left': `${WINDOW_SPEC.collapsedStripLeftPx}px`,
    '--dc-strip-width': `${WINDOW_SPEC.collapsedStripWidthPx}px`,
    '--dc-strip-height': `${WINDOW_SPEC.collapsedStripHeightPx}px`,
    '--dc-strip-radius': `${WINDOW_SPEC.collapsedStripRadiusPx}px`,
  }
  for (const [name, value] of Object.entries(values)) {
    root.style.setProperty(name, value)
  }
}