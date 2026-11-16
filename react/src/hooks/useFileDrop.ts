// 系统文件拖入：经 api client 订阅原生拖放事件拿真实路径，非 Tauri 环境不注册。
// 落点用 elementFromPoint 命中容器（data-drop-id，空串=顶层）或根级行（data-root-index）。
import { useEffect } from 'react'

import { backend, isTauriRuntime } from '../api'
import { useUiStore } from '../state/uiStore'

interface DropTarget {
  id: string | null
  index: number | null
}

export function useFileDrop(): void {
  useEffect(() => {
    if (!isTauriRuntime()) return

    let unlisten: (() => void) | undefined
    let disposed = false

    void (async () => {
      const stop = await backend.onFileDrop((event) => {
        const ui = useUiStore.getState()
        // HTML5 内部拖拽也会触发原生 enter/over；此时不能显示文件投放遮罩。
        if (ui.drag) {
          ui.setFileDragging(false)
          return
        }
        if (event.type === 'enter' || event.type === 'over') {
          ui.setFileDragging(true)
          return
        }
        if (event.type === 'drop') {
          ui.setFileDragging(false)
          if (event.paths.length === 0) return
          const target = hitTest(event.position)
          ui.openDialog({
            mode: 'add',
            source: event.paths.join('\n'),
            parentId: target.id ?? undefined,
            rootIndex: target.index ?? undefined,
          })
          return
        }
        ui.setFileDragging(false)
      })

      if (disposed) stop()
      else unlisten = stop
    })()

    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])
}

// 原生坐标是物理像素，elementFromPoint 需要 CSS 像素。容器优先：命中容器
// 就只按容器投放；只有落在顶层 Apps 网格（data-drop-id 为空）才带根级序号。
function hitTest(position: { x: number; y: number }): DropTarget {
  const ratio = window.devicePixelRatio || 1
  const element = document.elementFromPoint(position.x / ratio, position.y / ratio)
  const host = element?.closest<HTMLElement>('[data-drop-id]')
  if (!host) return { id: null, index: null }

  const id = host.dataset.dropId || null
  if (id !== null) return { id, index: null }

  const index = Number.parseInt(host.dataset.rootIndex ?? '', 10)
  return { id: null, index: Number.isNaN(index) ? null : index }
}