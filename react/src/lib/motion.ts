// 收起/展开过渡时长：CSS 与 JS 等待共用这一份，避免两边漂移。
export const COLLAPSE_MS = 240
export const EXPAND_MS = 240

export function prefersReducedMotion(): boolean {
  return window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false
}