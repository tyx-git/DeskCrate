/**
 * 收起 / 展开过渡的时长。
 *
 * 这两个数字是**唯一的事实来源**：CSS 里所有子动画的时长和延时都由它们
 * 按固定比例算出来，JS 侧的等待也读它们。改这里就能整体调快调慢，不会出现
 * 「动画已经播完、JS 还在等」或者反过来。
 */
export const COLLAPSE_MS = 200
export const EXPAND_MS = 240

/** 是否要求减少动效。开启后跳过等待，直接切窗口几何。 */
export function prefersReducedMotion(): boolean {
  return window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false
}
