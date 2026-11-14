// 悬浮窗尺寸上下限：与后端 css::OVERLAY_*_MIN/MAX_PX 一致，单位设计稿 CSS 像素。
import { WINDOW_SPEC } from '../spec/windowSpec'

export const OVERLAY_WIDTH_MIN = WINDOW_SPEC.overlayWidthMinPx
export const OVERLAY_WIDTH_MAX = WINDOW_SPEC.overlayWidthMaxPx
export const OVERLAY_HEIGHT_MIN = WINDOW_SPEC.overlayHeightMinPx
export const OVERLAY_HEIGHT_MAX = WINDOW_SPEC.overlayHeightMaxPx

export function clampOverlaySize(width: number, height: number): { width: number; height: number } {
  const clamp = (value: number, min: number, max: number) =>
    Math.min(Math.max(Math.round(value), min), Math.max(min, max))
  return {
    width: clamp(width, OVERLAY_WIDTH_MIN, OVERLAY_WIDTH_MAX),
    height: clamp(height, OVERLAY_HEIGHT_MIN, OVERLAY_HEIGHT_MAX),
  }
}