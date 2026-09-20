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
export function useFileDrop(onDropPaths: (paths: string[], targetId: string | null) => void) {
  const [isDragging, setIsDragging] = useState(false)
  const [activeDropId, setActiveDropId] = useState<string | null>(null)
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
          setIsDragging(true)
          setActiveDropId(hitTest(payload.position))
          return
        }

        if (payload.type === 'drop') {
          const targetId = hitTest(payload.position)
          setIsDragging(false)
          setActiveDropId(null)
          if (payload.paths.length > 0) {
            handlerRef.current(payload.paths, targetId)
          }
          return
        }

        setIsDragging(false)
        setActiveDropId(null)
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

  return { isDragging, activeDropId }
}

/** 原生事件的坐标是物理像素，`elementFromPoint` 需要 CSS 像素。 */
function hitTest(position: { x: number; y: number }): string | null {
  const ratio = window.devicePixelRatio || 1
  const element = document.elementFromPoint(position.x / ratio, position.y / ratio)
  const host = element?.closest<HTMLElement>('[data-drop-id]')
  return host?.dataset.dropId ?? null
}
