/**
 * 悬浮窗尺寸的上下限。
 *
 * 与 Rust 的 `models::settings::css::OVERLAY_*_MIN/MAX_PX` 一一对应，单位同样
 * 是设计稿 CSS 像素 —— 前端的鼠标坐标和 `innerWidth/innerHeight` 本来就在这个
 * 单位里，先夹再交给后端换算，两边才不会因为一个缩放比而错开。
 */
import { WINDOW_SPEC } from '../spec/windowSpec'

export const OVERLAY_WIDTH_MIN = WINDOW_SPEC.overlayWidthMinPx
export const OVERLAY_WIDTH_MAX = WINDOW_SPEC.overlayWidthMaxPx
export const OVERLAY_HEIGHT_MIN = WINDOW_SPEC.overlayHeightMinPx
export const OVERLAY_HEIGHT_MAX = WINDOW_SPEC.overlayHeightMaxPx

/** 把尺寸夹进设计稿允许的范围。 */
export function clampOverlaySize(width: number, height: number): {
  width: number
  height: number
} {
  const clamp = (value: number, min: number, max: number) =>
    Math.min(Math.max(Math.round(value), min), Math.max(min, max))

  return {
    width: clamp(width, OVERLAY_WIDTH_MIN, OVERLAY_WIDTH_MAX),
    height: clamp(height, OVERLAY_HEIGHT_MIN, OVERLAY_HEIGHT_MAX),
  }
}
