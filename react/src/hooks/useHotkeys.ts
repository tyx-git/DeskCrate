// 全局键盘：Ctrl+K 聚焦搜索，Esc 逐层关闭浮层，焦点在应用/分类行时的快捷键。
import { useEffect, type RefObject } from 'react'

import { useCatalogStore } from '../state/catalogStore'
import { useSettingsStore } from '../state/settingsStore'
import { useUiStore } from '../state/uiStore'
import { locateApp, locateNode } from '../lib/tree'

export function useHotkeys(searchRef: RefObject<HTMLInputElement | null>): void {
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const ui = useUiStore.getState()
      const settings = useSettingsStore.getState()
      const catalog = useCatalogStore.getState()

      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault()
        if (settings.viewMode === 'collapsed') void settings.setViewMode('expanded')
        searchRef.current?.focus()
        searchRef.current?.select()
        return
      }

      if (e.key === 'Escape') {
        if (ui.ctx) return ui.closeCtx()
        if (ui.dialog) return ui.closeDialog()
        if (ui.settingsOpen) return ui.closeSettings()
        if (ui.query) {
          ui.setQuery('')
          searchRef.current?.focus()
        }
        return
      }

      const target = e.target as HTMLElement | null
      if (target?.closest('input, textarea, select')) return

      const appEl = target?.closest<HTMLElement>('.app')
      const rowEl = target?.closest<HTMLElement>('.row')

      if (appEl?.dataset.app) {
        const id = appEl.dataset.app
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          const loc = locateApp(catalog.data, id)
          if (loc) void catalog.launchApp(loc.app)
        } else if (e.key === 'Delete') {
          e.preventDefault()
          void catalog.deleteApp(id)
        } else if (e.key === 'F2') {
          e.preventDefault()
          ui.openDialog({ mode: 'rename-app', id })
        }
        return
      }

      if (rowEl?.dataset.id) {
        const id = rowEl.dataset.id
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          const loc = locateNode(catalog.data, id)
          if (loc) void catalog.toggleCollapse(id, !loc.node.isCollapsed)
        } else if (e.key === 'F2') {
          e.preventDefault()
          ui.startRename(id)
        }
      }
    }

    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [searchRef])
}