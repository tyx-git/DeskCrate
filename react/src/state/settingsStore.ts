// 配置与窗口状态：收起/展开的收束变形、显示后几何同步、设置项乐观更新。
import { create } from 'zustand'

import { backend, describeError, toAppError } from '../api'
import { COLLAPSE_MS, prefersReducedMotion } from '../lib/motion'
import type { CssSize, Settings, ViewMode } from '../types/settings'
import { useUiStore } from './uiStore'

interface SettingsStore {
  settings: Settings | null
  viewMode: ViewMode
  switching: boolean
  /** 面板处于「收束」丝状形态：收起先收束再缩窗，展开先挂丝状再放窗。 */
  stageConverged: boolean
  /** 当前面板内容尺寸（CSS 像素），供设置页展示。 */
  panelSize: CssSize | null

  load: () => Promise<void>
  setViewMode: (mode: ViewMode) => Promise<void>
  toggleViewMode: () => Promise<void>
  syncWindowGeometry: () => Promise<void>
  resetWindowSize: () => Promise<void>
  setAutoCollapse: (autoCollapse: boolean) => Promise<void>
  setAutoStart: (autoStart: boolean) => Promise<void>
  setPanelSize: (size: CssSize | null) => void
}

let collapseTimer: number | undefined

export const useSettingsStore = create<SettingsStore>((set, get) => ({
  settings: null,
  viewMode: 'expanded',
  switching: false,
  stageConverged: false,
  panelSize: null,

  async load() {
    try {
      const settings = await backend.getSettings()
      set({ settings, viewMode: settings.viewMode })
    } catch (error) {
      // 保留上一次成功读取的配置；首次失败时 settings 仍为 null，不伪造默认值。
      useUiStore.getState().notify('error', `读取配置失败：${describeError(toAppError(error))}`)
    }
  },

  async setViewMode(mode) {
    const { viewMode } = get()

    if (mode === 'collapsed') {
      if (viewMode === 'collapsed' || collapseTimer !== undefined) return
      set({ stageConverged: true })
      collapseTimer = window.setTimeout(
        () => {
          collapseTimer = undefined
          void commitCollapse(set, get)
        },
        prefersReducedMotion() ? 0 : COLLAPSE_MS,
      )
      return
    }

    if (collapseTimer !== undefined) {
      window.clearTimeout(collapseTimer)
      collapseTimer = undefined
    }
    if (viewMode === 'expanded') {
      set({ stageConverged: false })
      return
    }
    await commitExpand(set, get)
  },

  async toggleViewMode() {
    await get().setViewMode(get().viewMode === 'expanded' ? 'collapsed' : 'expanded')
  },

  async syncWindowGeometry() {
    // 配置尚未读到时不套几何：默认 expanded 会覆盖后端持久化的 collapsed。
    if (!get().settings) return
    // Windows 会在窗口由隐藏转可见时异步把宽度顶到系统最小值，收起态那 29 物理
    // 像素会被撑成白条。show() 之后回读实际尺寸，不一致就再套一次几何。
    const ratio = window.devicePixelRatio || 1
    try {
      let applied = await backend.applyViewMode(get().viewMode)
      for (const wait of [200, 300, 400]) {
        await new Promise((resolve) => window.setTimeout(resolve, wait))
        const expectedWidth = Math.round(applied.size.width / ratio)
        const expectedHeight = Math.round(applied.size.height / ratio)
        if (
          Math.abs(window.innerWidth - expectedWidth) <= 2 &&
          Math.abs(window.innerHeight - expectedHeight) <= 2
        ) {
          break
        }
        applied = await backend.applyViewMode(get().viewMode)
      }
      // apply_view_mode 会推进后端 revision，必须回读，否则后续 updateSettings 冲突。
      await get().load()
    } catch (error) {
      useUiStore.getState().notify('error', `应用窗口尺寸失败：${describeError(toAppError(error))}`)
    }
  },

  async resetWindowSize() {
    try {
      const fresh = await backend.getSettings()
      if (!fresh.window.sizeOverride) {
        await backend.applyViewMode(get().viewMode)
        await get().load()
        useUiStore.getState().notify('info', '窗口尺寸已按设计稿重新计算')
        return
      }
      const saved = await backend.clearWindowSizeOverride()
      set({ settings: saved })
      await backend.applyViewMode(get().viewMode)
      await get().load()
      useUiStore.getState().pushToast('已恢复默认窗口尺寸', { icon: 'undo' })
    } catch (error) {
      useUiStore.getState().notify('error', `重置窗口尺寸失败：${describeError(toAppError(error))}`)
    }
  },

  async setAutoCollapse(autoCollapse) {
    const settings = get().settings
    if (!settings) return
    set({ settings: { ...settings, autoCollapse } })
    try {
      const saved = await backend.updateAutoCollapse(autoCollapse)
      set({ settings: saved })
    } catch (error) {
      set({ settings })
      useUiStore.getState().notify('error', `保存配置失败：${describeError(toAppError(error))}`)
    }
  },

  async setAutoStart(autoStart) {
    const settings = get().settings
    if (!settings) return
    set({ settings: { ...settings, autoStart } })
    try {
      const saved = await backend.updateAutoStart(autoStart)
      set({ settings: saved })
    } catch (error) {
      set({ settings })
      useUiStore.getState().notify('error', `保存配置失败：${describeError(toAppError(error))}`)
    }
  },

  setPanelSize: (panelSize) => set({ panelSize }),
}))

type SetState = (partial: Partial<SettingsStore> | ((state: SettingsStore) => Partial<SettingsStore>)) => void

/** 收束动画播完才真正缩窗：窗口一缩面板只剩 3×80，动画没有地方播。 */
async function commitCollapse(set: SetState, get: () => SettingsStore): Promise<void> {
  set({ switching: true })
  try {
    await backend.applyViewMode('collapsed')
    set({ viewMode: 'collapsed', stageConverged: false, switching: false })
    // apply_view_mode 推进了后端 revision，回读权威配置避免后续保存冲突。
    await get().load()
  } catch (error) {
    set({ stageConverged: false, switching: false })
    useUiStore.getState().notify('error', `切换侧栏状态失败：${describeError(toAppError(error))}`)
  }
}

/** 展开：窗口先变大，面板再从丝状放开，视觉上指示条自己长成侧栏。 */
async function commitExpand(set: SetState, get: () => SettingsStore): Promise<void> {
  set({ switching: true, viewMode: 'expanded', stageConverged: true })
  try {
    await backend.applyViewMode('expanded')
    await get().load()
    await backend.focusWindow()
  } catch (error) {
    set({ viewMode: 'collapsed', stageConverged: false, switching: false })
    useUiStore.getState().notify('error', `切换侧栏状态失败：${describeError(toAppError(error))}`)
    return
  }

  set({ switching: false })
  const release = () => set({ stageConverged: false })
  if (prefersReducedMotion()) {
    release()
    return
  }
  // 等浏览器真的画出「丝状」这一帧再放开，否则同帧加删类过渡不会跑。
  requestAnimationFrame(() => requestAnimationFrame(release))
}