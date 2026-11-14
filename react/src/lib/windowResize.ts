// 窗口缩放几何：只处理右/顶/底/右下角，单位设计稿 CSS 像素。
// 顶边先把上沿夹进工作区，再反推高度，保证底边不被后端的位置夹取推走。
import { clampOverlaySize, OVERLAY_HEIGHT_MIN, OVERLAY_WIDTH_MIN } from './overlaySize'

export type ResizeEdge = 'e' | 's' | 'n' | 'se'

export interface ResizeWorkArea {
  left: number
  top: number
  right: number
  bottom: number
}

export interface ResizeSession {
  edge: ResizeEdge
  x: number
  y: number
  width: number
  height: number
  workArea: ResizeWorkArea
}

export interface ResizeGeometry {
  x: number
  y: number
  width: number
  height: number
}

export function nextResizeGeometry(session: ResizeSession, dx: number, dy: number): ResizeGeometry {
  const { edge, workArea } = session
  const bottom = session.y + session.height
  let x = session.x
  let y = session.y
  let width = session.width
  let height = session.height

  if (edge === 'e' || edge === 'se') width = session.width + dx
  if (edge === 's' || edge === 'se') height = session.height + dy
  if (edge === 'n') {
    y = Math.min(Math.max(session.y + dy, workArea.top), bottom - OVERLAY_HEIGHT_MIN)
    height = bottom - y
  }

  const size = clampOverlaySize(width, height)
  width = size.width
  height = size.height

  if (edge === 'n') {
    y = bottom - height
    if (y < workArea.top) {
      y = workArea.top
      height = bottom - y
    }
  }

  if (edge === 'e' || edge === 'se') {
    width = Math.max(OVERLAY_WIDTH_MIN, Math.min(width, workArea.right - x))
  }
  if (edge === 's' || edge === 'se') {
    height = Math.max(OVERLAY_HEIGHT_MIN, Math.min(height, workArea.bottom - y))
  }

  return { x, y, width, height }
}
