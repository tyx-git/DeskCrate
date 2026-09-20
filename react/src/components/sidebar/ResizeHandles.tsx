import { useCallback, useRef } from 'react'

import { clampOverlaySize } from '../../lib/overlaySize'
import { applyWindowGeometry, finishWindowResize, type WindowGeometry } from '../../lib/window'

/** 允许手动调整的窗口边（取值与后端 `ResizeWindowRequest` 的方向一致）。 */
type Edge = 'East' | 'North' | 'NorthEast' | 'South' | 'SouthEast'

interface Handle {
  direction: Edge
  /** 位置：贴着面板边缘、落在窗口的透明留白里，因此不会挡住面板内容。 */
  className: string
  label: string
}

/**
 * 缩放把手。
 *
 * 窗口在 `tauri.conf.json` 里是 `resizable: false`，没有原生缩放边框 —— 原生
 * 边框会连左侧一起开放，而左边缘贴着屏幕工作区，拖它是错的操作。这里只给
 * 右边缘和上下边缘（含右侧两个角）放把手。
 *
 * 把手落在面板外围那圈透明留白里（面板右/上/下各留 16px、左留 2px），平时
 * 看不见，悬停时才显出一条细高亮，既提示可拖又不占用面板。
 */
const HANDLES: readonly Handle[] = [
  {
    direction: 'East',
    className: 'right-[10px] top-[10px] bottom-[10px] w-[6px] cursor-ew-resize',
    label: '拖动调整侧栏宽度',
  },
  {
    direction: 'North',
    className: 'left-[2px] right-[16px] top-[10px] h-[6px] cursor-ns-resize',
    label: '拖动调整侧栏高度',
  },
  {
    direction: 'South',
    className: 'left-[2px] right-[16px] bottom-[10px] h-[6px] cursor-ns-resize',
    label: '拖动调整侧栏高度',
  },
  {
    direction: 'NorthEast',
    className: 'right-[10px] top-[10px] h-[6px] w-[6px] cursor-nesw-resize',
    label: '拖动同时调整侧栏宽度和高度',
  },
  {
    direction: 'SouthEast',
    className: 'right-[10px] bottom-[10px] h-[6px] w-[6px] cursor-nwse-resize',
    label: '拖动同时调整侧栏宽度和高度',
  },
]

/** 一次拖拽的起点快照。 */
interface DragOrigin {
  pointerId: number
  direction: Edge
  /** 指针起点，视口坐标（CSS 像素）。 */
  clientX: number
  clientY: number
  /** 窗口起点，屏幕坐标（CSS 像素）。`window.screenX/Y` 在 WebView2 里可用。 */
  screenX: number
  screenY: number
  width: number
  height: number
}

// 按方向把「指针位移」翻译成新的窗口几何。
//
// 拖上边缘时高度变大、窗口顶部要跟着上移，所以按下边缘那两条要先算出
// **夹取之后**的高度，再反推 y —— 否则撞到设计稿上限时窗口会一边被夹住
// 一边继续往屏幕外挪。
function nextGeometry(origin: DragOrigin, clientX: number, clientY: number): WindowGeometry {
  const dx = clientX - origin.clientX
  const dy = clientY - origin.clientY

  const growsUp = origin.direction === 'North' || origin.direction === 'NorthEast'
  const growsRight =
    origin.direction === 'East' || origin.direction.endsWith('East')

  const { width, height } = clampOverlaySize(
    origin.width + (growsRight ? dx : 0),
    origin.height + (growsUp ? -dy : dy),
  )

  return {
    x: origin.screenX,
    // 下边缘固定：顶部随高度变化上移或下移。
    y: growsUp ? origin.screenY + origin.height - height : origin.screenY,
    width,
    height,
  }
}

/**
 * 展开态的窗口缩放把手。
 *
 * 纯鼠标操作，键盘用户走右键菜单里的「重置窗口尺寸」，因此对无障碍树隐藏，
 * 避免读屏念出一组无法用键盘触发的控件。
 */
export function ResizeHandles() {
  const originRef = useRef<DragOrigin | null>(null)
  const pendingRef = useRef<WindowGeometry | null>(null)
  const frameRef = useRef<number | null>(null)

  // 指针移动可能一帧来好几次，逐条下发只会白白多跑几十次 IPC 加一次
  // `set_size`。攒到下一帧合并成一次下发，肉眼看不出延迟。
  const schedule = useCallback((geometry: WindowGeometry) => {
    pendingRef.current = geometry
    if (frameRef.current !== null) {
      return
    }

    frameRef.current = window.requestAnimationFrame(() => {
      frameRef.current = null
      const next = pendingRef.current
      pendingRef.current = null
      if (next) {
        void applyWindowGeometry(next)
      }
    })
  }, [])

  const stop = useCallback(() => {
    const origin = originRef.current
    originRef.current = null
    document.body.classList.remove('dc-resizing')

    if (frameRef.current !== null) {
      window.cancelAnimationFrame(frameRef.current)
      frameRef.current = null
    }
    // 松手时把最后一帧补下发，避免停在节流掉的那一个中间尺寸上。
    const last = pendingRef.current
    pendingRef.current = null
    if (last) {
      void applyWindowGeometry(last).then(finishWindowResize)
    } else {
      void finishWindowResize()
    }
    return origin
  }, [])

  return (
    <div aria-hidden="true">
      {HANDLES.map((handle) => (
        <div
          key={handle.direction}
          title={handle.label}
          onPointerDown={(event) => {
            if (event.button !== 0) {
              return
            }
            event.preventDefault()
            event.stopPropagation()

            const target = event.currentTarget
            target.setPointerCapture(event.pointerId)
            originRef.current = {
              pointerId: event.pointerId,
              direction: handle.direction,
              clientX: event.clientX,
              clientY: event.clientY,
              screenX: window.screenX,
              screenY: window.screenY,
              width: window.innerWidth,
              height: window.innerHeight,
            }
            document.body.classList.add('dc-resizing')
          }}
          onPointerMove={(event) => {
            const origin = originRef.current
            if (!origin || origin.pointerId !== event.pointerId) {
              return
            }
            schedule(nextGeometry(origin, event.clientX, event.clientY))
          }}
          onPointerUp={(event) => {
            const origin = stop()
            if (!origin) {
              return
            }
            if (event.currentTarget.hasPointerCapture(event.pointerId)) {
              event.currentTarget.releasePointerCapture(event.pointerId)
            }
          }}
          onPointerCancel={() => void stop()}
          onContextMenu={(event) => {
            // 把手上右键不该弹出侧栏菜单。
            event.preventDefault()
            event.stopPropagation()
          }}
          className={`absolute z-20 transition-colors duration-100 hover:bg-[rgb(58_96_190_/_0.45)] ${handle.className}`}
        />
      ))}
    </div>
  )
}
