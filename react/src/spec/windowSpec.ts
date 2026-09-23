import rawSpec from './window.json'

export const WINDOW_SPEC = rawSpec

/** 把窗口规格注入 CSS 自定义属性，CSS 不再复制窗口几何数字。 */
export function applyWindowSpecCssVariables(): void {
  const root = document.documentElement
  const cssValues: Record<string, string> = {
    '--dc-spec-sidebar-left': `${WINDOW_SPEC.sidebarLeftPx}px`,
    '--dc-spec-sidebar-top': `${WINDOW_SPEC.sidebarTopPercent}%`,
    '--dc-spec-sidebar-width': `${WINDOW_SPEC.sidebarWidthPercent}%`,
    '--dc-spec-sidebar-height': `${WINDOW_SPEC.sidebarHeightPercent}%`,
    '--dc-spec-sidebar-padding-y': `${WINDOW_SPEC.sidebarVerticalPaddingPx}px`,
    '--dc-spec-sidebar-radius': `${WINDOW_SPEC.sidebarBorderRadiusPx}px`,
    '--dc-spec-scrollbar-width': `${WINDOW_SPEC.sidebarScrollbarWidthPx}px`,
    '--dc-spec-glass-blur': `${WINDOW_SPEC.sidebarBackdropBlurPx}px`,
    '--dc-spec-strip-left': `${WINDOW_SPEC.collapsedStripLeftPx}px`,
    '--dc-spec-strip-top': `${WINDOW_SPEC.collapsedStripTopPercent}%`,
    '--dc-spec-strip-width': `${WINDOW_SPEC.collapsedStripWidthPx}px`,
    '--dc-spec-strip-height': `${WINDOW_SPEC.collapsedStripHeightPx}px`,
    '--dc-spec-strip-radius': `${WINDOW_SPEC.collapsedStripRadiusPx}px`,
    '--dc-spec-gutter': `${WINDOW_SPEC.overlayShadowGutterPx}px`,
    '--dc-spec-panel-radius': `${WINDOW_SPEC.sidebarBorderRadiusPx}px`,
  }

  for (const [name, value] of Object.entries(cssValues)) {
    root.style.setProperty(name, value)
  }
}
