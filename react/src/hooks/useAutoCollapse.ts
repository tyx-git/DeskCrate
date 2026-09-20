import { useEffect, type RefObject } from 'react'

import { useSettingsStore } from '../state/settingsStore'
import { useUiStore } from '../state/uiStore'

/** 鼠标离开后延迟收起的时间。太短会手一抖就缩回去，太长就显得迟钝。 */
const COLLAPSE_DELAY_MS = 260

/**
 * 「自动收起」行为。
 *
 * 只有用户开启 `autoCollapse` 时才生效：
 * - 展开态鼠标离开**面板** → 延时收起；
 * - 收起态鼠标移入窗口 → 立即展开（面板从指示条长出来，见 `settingsStore`）。
 *
 * 关闭该选项时，收起态只能通过点击指示条展开，避免用户主动收起后又被
 * 鼠标经过意外弹开。
 */
export function useAutoCollapse(
  shellRef: RefObject<HTMLElement | null>,
  panelRef: RefObject<HTMLElement | null>,
) {
  const viewMode = useSettingsStore((state) => state.viewMode)
  const autoCollapse = useSettingsStore((state) => state.settings?.autoCollapse ?? false)
  const setViewMode = useSettingsStore((state) => state.setViewMode)

  useEffect(() => {
    const shell = shellRef.current
    if (!shell || !autoCollapse) {
      return
    }

    if (viewMode === 'collapsed') {
      const expand = () => void setViewMode('expanded')
      shell.addEventListener('mouseenter', expand)
      return () => shell.removeEventListener('mouseenter', expand)
    }

    // 以**面板**而不是整个窗口作为悬停区。
    //
    // 窗口比面板大一圈（右侧和上下各留 16px 给阴影），从面板移到这圈透明
    // 留白上原本算「还在窗口里」，鼠标明明已经离开侧栏了却不收起 —— 这是
    // 「不灵敏」最主要的一处。换成面板之后，离开侧栏就算离开。
    const panel = panelRef.current ?? shell

    let timer: number | undefined

    const scheduleCollapse = () => {
      // 对话框 / 右键菜单打开时不收起：遮罩和菜单都盖在面板上，鼠标移上去
      // 会被当成「离开了面板」，侧栏就在用户操作到一半时缩回去了。
      const ui = useUiStore.getState()
      if (ui.dialog || ui.contextMenu) {
        return
      }

      // 收束动画期间面板被 `clip-path` 裁小，命中区也跟着变小，鼠标停在原处
      // 会不断触发 `mouseleave`。此时已经在收了，再排一次只会多跑一遍
      // `apply_view_mode`。
      if (useSettingsStore.getState().stageConverged) {
        return
      }

      window.clearTimeout(timer)
      timer = window.setTimeout(() => void setViewMode('collapsed'), COLLAPSE_DELAY_MS)
    }
    const cancelCollapse = () => window.clearTimeout(timer)

    panel.addEventListener('mouseleave', scheduleCollapse)
    panel.addEventListener('mouseenter', cancelCollapse)

    return () => {
      window.clearTimeout(timer)
      panel.removeEventListener('mouseleave', scheduleCollapse)
      panel.removeEventListener('mouseenter', cancelCollapse)
    }
  }, [viewMode, autoCollapse, setViewMode, shellRef, panelRef])
}
