import { useEffect, useRef, useState } from 'react'

import { isTauriRuntime } from '../api/client'

/**
 * 系统文件拖拽。
 *
 * 浏览器出于安全考虑不会把本地文件的真实路径交给网页，所以只有走 Tauri
 * 的原生拖放事件才能拿到路径。这里用 `position` 做一次命中测试，把落点
 * 定位到具体的分类或分组行（行上有 `data-drop-id`）。
 *
 * 浏览器预览模式下该 hook 不注册任何监听，拖拽不可用。
 */
export interface DropTarget {
  /** 命中的分类/分组节点 ID；没有命中任何具体容器时为 `null`。 */
  id: string | null
  /** 命中的插入位置（第几个根节点之前）；没有命中分界线时为 `null`。 */
  index: number | null
}

/**
 * 系统文件拖拽。
 *
 * 命中测试有两种结果，优先级从内到外：
 *
 * 1. `data-drop-index` —— 根节点之间的插入分界线，表示「放到这个位置」；
 * 2. `data-drop-id` —— 具体的分类或分组行，表示「放进这个容器」。
 *
 * 浏览器预览模式下该 hook 不注册任何监听，拖拽不可用。
 */
export function useFileDrop(
  onDropPaths: (paths: string[], target: DropTarget) => void,
) {
  const [isDragging, setIsDragging] = useState(false)
  const [activeDropId, setActiveDropId] = useState<string | null>(null)
  const [activeDropIndex, setActiveDropIndex] = useState<number | null>(null)
  const handlerRef = useRef(onDropPaths)

  // 用 ref 保存回调，避免调用方每次渲染都重新订阅原生事件。
  useEffect(() => {
    handlerRef.current = onDropPaths
  }, [onDropPaths])

  useEffect(() => {
    if (!isTauriRuntime()) {
      return
    }

    let unlisten: (() => void) | undefined
    let disposed = false

    void (async () => {
      const { getCurrentWebview } = await import('@tauri-apps/api/webview')
      const stop = await getCurrentWebview().onDragDropEvent((event) => {
        const payload = event.payload

        if (payload.type === 'enter' || payload.type === 'over') {
          const target = hitTest(payload.position)
          setIsDragging(true)
          setActiveDropId(target.id)
          setActiveDropIndex(target.index)
          return
        }

        if (payload.type === 'drop') {
          const target = hitTest(payload.position)
          setIsDragging(false)
          setActiveDropId(null)
          setActiveDropIndex(null)
          if (payload.paths.length > 0) {
            handlerRef.current(payload.paths, target)
          }
          return
        }

        setIsDragging(false)
        setActiveDropId(null)
        setActiveDropIndex(null)
      })

      if (disposed) {
        stop()
      } else {
        unlisten = stop
      }
    })()

    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  return { isDragging, activeDropId, activeDropIndex }
}

/**
 * 原生事件的坐标是物理像素，`elementFromPoint` 需要 CSS 像素。
 *
 * 先找插入分界线（`data-drop-index`），再找具体容器（`data-drop-id`）——
 * 两者在 DOM 里不重叠，谁命中算谁。
 */
function hitTest(position: { x: number; y: number }): DropTarget {
  const ratio = window.devicePixelRatio || 1
  const element = document.elementFromPoint(position.x / ratio, position.y / ratio)

  const gap = element?.closest<HTMLElement>('[data-drop-index]')
  if (gap?.dataset.dropIndex) {
    const index = Number.parseInt(gap.dataset.dropIndex, 10)
    if (!Number.isNaN(index)) {
      return { id: null, index }
    }
  }

  const host = element?.closest<HTMLElement>('[data-drop-id]')
  return { id: host?.dataset.dropId ?? null, index: null }
}
