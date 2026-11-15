// 自动收起：鼠标离开面板 300ms 后收为指示条，悬停指示条 100ms 后展开。
// 监听挂在面板而非窗口上——窗口四周有 16px 透明阴影留白，不该触发悬停。
import { useEffect, type RefObject } from 'react'

import { useSettingsStore } from '../state/settingsStore'
import { useUiStore } from '../state/uiStore'

const LEAVE_MS = 400
const HOVER_MS = 100

export function useAutoCollapse(panelRef: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    const el = panelRef.current
    if (!el) return

    let leaveTimer: number | undefined
    let hoverTimer: number | undefined

    const onEnter = () => {
      window.clearTimeout(leaveTimer)
      const settings = useSettingsStore.getState()
      if (!settings.settings?.autoCollapse) return
      if (settings.viewMode !== 'collapsed') return
      window.clearTimeout(hoverTimer)
      hoverTimer = window.setTimeout(() => void settings.setViewMode('expanded'), HOVER_MS)
    }

    const onLeave = () => {
      window.clearTimeout(hoverTimer)
      const settings = useSettingsStore.getState()
      const ui = useUiStore.getState()
      if (!settings.settings?.autoCollapse || settings.viewMode !== 'expanded') return
      // 有浮层或正在收束变形时不收起，避免把对话框/菜单一起收没。
      if (ui.dialog || ui.ctx || ui.settingsOpen || settings.stageConverged) return
      window.clearTimeout(leaveTimer)
      leaveTimer = window.setTimeout(() => void settings.setViewMode('collapsed'), LEAVE_MS)
    }

    el.addEventListener('mouseenter', onEnter)
    el.addEventListener('mouseleave', onLeave)
    return () => {
      window.clearTimeout(leaveTimer)
      window.clearTimeout(hoverTimer)
      el.removeEventListener('mouseenter', onEnter)
      el.removeEventListener('mouseleave', onLeave)
    }
  }, [panelRef])
}