import { useEffect } from 'react'

import { locateNode } from '../lib/tree'
import { useCatalogStore } from '../state/catalogStore'
import { useUiStore } from '../state/uiStore'

const SEARCH_INPUT_ID = 'deskcrate-search'

/** 让搜索框获得焦点并全选，方便直接覆盖输入。 */
export function focusSearch(): void {
  const input = document.getElementById(SEARCH_INPUT_ID)
  if (input instanceof HTMLInputElement) {
    input.focus()
    input.select()
  }
}

/**
 * 全局快捷键。
 *
 * 悬浮侧栏没有菜单栏和窗口按钮，键盘是主要的可达性通道：
 * `Ctrl/⌘+K` 搜索、`↑/↓` 在可见应用之间移动、`F2` 重命名、`Delete` 删除、
 * `Esc` 逐层关闭（对话框 → 右键菜单 → 搜索词）。
 */
export function useHotkeys() {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      // 已经由输入框等更内层的处理器消费掉的事件不再重复处理。
      if (event.defaultPrevented) {
        return
      }

      const ui = useUiStore.getState()

      if (event.key === 'Escape') {
        if (ui.dialog) {
          event.preventDefault()
          ui.closeDialog()
          return
        }
        if (ui.contextMenu) {
          event.preventDefault()
          ui.closeContextMenu()
          return
        }
        if (ui.query.length > 0) {
          event.preventDefault()
          ui.setQuery('')
        }
        return
      }

      if ((event.ctrlKey || event.metaKey) && ['k', 'f'].includes(event.key.toLowerCase())) {
        event.preventDefault()
        focusSearch()
        return
      }

      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        const nodes = Array.from(document.querySelectorAll<HTMLElement>('[data-app-id]'))
        if (nodes.length === 0) {
          return
        }

        event.preventDefault()
        const current = nodes.indexOf(document.activeElement as HTMLElement)
        const target =
          event.key === 'ArrowDown'
            ? (nodes[Math.min(current + 1, nodes.length - 1)] ?? nodes[0])
            : (nodes[Math.max(current - 1, 0)] ?? nodes[nodes.length - 1])
        target?.focus()
        return
      }

      // F2 / Delete 只作用于当前获得焦点的应用图标；分类与分组的管理
      // 操作通过右键菜单完成。
      const selectedId = ui.selectedAppId
      if (!selectedId) {
        return
      }

      if (event.key === 'F2') {
        event.preventDefault()
        const location = locateNode(useCatalogStore.getState().data, selectedId)
        if (location) {
          ui.openDialog({ kind: 'rename', targetId: location.id, targetName: location.name, initialValue: location.name })
        }
        return
      }

      if (event.key === 'Delete') {
        event.preventDefault()
        const location = locateNode(useCatalogStore.getState().data, selectedId)
        if (location) {
          // 与右键菜单保持一致：删除前先确认。
          ui.openDialog({
            kind: 'confirm',
            targetName: `删除「${location.name}」？`,
            message: '应用会从侧栏移除，此操作无法撤销。',
            confirmLabel: '删除',
            onConfirm: () => void useCatalogStore.getState().deleteApp(location.id),
          })
        }
      }
    }

    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [])
}

export { SEARCH_INPUT_ID }
