import { create } from 'zustand'

import { backend, describeError, toAppError } from '../api'
import { COLLAPSE_MS, prefersReducedMotion } from '../lib/motion'
import type { Settings, ViewMode } from '../types/settings'
import { focusMainWindow } from '../lib/window'
import { useUiStore } from './uiStore'

interface SettingsStore {
  settings: Settings | null
  viewMode: ViewMode
  /** 收起/展开的窗口几何切换正在进行，避免连点造成状态错乱。 */
  switching: boolean
  /**
   * 面板是否处于「收束」形态。
   *
   * 收起时先把它置为 true 播收束动画，动画结束才真的缩窗口 —— 窗口一缩，
   * 面板就被裁成指示条大小，动画没有地方播。展开时反过来：面板先以丝状
   * 挂上去（此刻窗口还是收起态那么大，视觉上和指示条重合），再放开。
   */
  stageConverged: boolean

  load: () => Promise<void>
  setViewMode: (mode: ViewMode) => Promise<void>
  toggleViewMode: () => Promise<void>
  syncWindowGeometry: () => Promise<void>
  resetWindowSize: () => Promise<void>
  setAutoCollapse: (autoCollapse: boolean) => Promise<void>
  setAutoStart: (autoStart: boolean) => Promise<void>
}

// 收束动画的等待定时器。收束期间鼠标一旦回到面板上就要能打断，所以不能
// 只靠 `switching` 挡住 —— 那会把「取消收起」也一起挡掉。
let collapseTimer: number | undefined

const FALLBACK: Settings = {
  revision: 0,
  theme: 'default',
  window: { isInitialized: false, position: null, size: null, sizeOverride: null },
  viewMode: 'expanded',
  autoCollapse: false,
  autoStart: false,
}

// 设置页使用字段级 Command；兼容的整体更新由后端 revision 冲突保护。
export const useSettingsStore = create<SettingsStore>((set, get) => ({
  settings: null,
  viewMode: 'expanded',
  switching: false,
  stageConverged: false,

  async load() {
    try {
      const settings = await backend.getSettings()
      set({ settings, viewMode: settings.viewMode })
    } catch (error) {
      // 配置读不出来不应该阻塞侧栏渲染，退回默认值并提示一次。
      set({ settings: FALLBACK, viewMode: FALLBACK.viewMode })
      useUiStore.getState().notify('error', `读取配置失败：${describeError(toAppError(error))}`)
    }
  },

  async setViewMode(mode) {
    const { viewMode } = get()

    if (mode === 'collapsed') {
      if (viewMode === 'collapsed' || collapseTimer !== undefined) {
        return
      }
      // 先播收束动画，动画结束才缩窗口。
      set({ stageConverged: true })
      collapseTimer = window.setTimeout(
        () => {
          collapseTimer = undefined
          void commitCollapse(set)
        },
        prefersReducedMotion() ? 0 : COLLAPSE_MS,
      )
      return
    }

    // 展开。正在收束的话先取消 —— 去掉收束类，面板会沿同一条过渡反向展开。
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
    // 后端 `is_init` 是在窗口还隐藏的时候摆好几何的，但 Windows 会在窗口由隐藏
    // 转为可见时把宽度顶到系统的最小窗口宽度（本机 150% 缩放下约 203 物理像素）：
    // 收起态窗口只有 29 物理像素宽，于是被撑成一大块白条。窗口已经可见之后再套
    // 一次几何就能恢复设计尺寸。
    //
    // 关键在于**这个顶宽动作是异步的**：`show()` 返回时窗口还没被顶宽，紧接着
    // 套的几何会被随后到达的那次调整覆盖（实测以收起态启动会稳定停在 135 逻辑
    // 像素宽，直到手工展开再收起才恢复）。所以套完之后要回读实际尺寸，不一致
    // 就再套一次，而不是默认一次就能成。
    //
    // 只在启动流程的 `show()` 之后调用才有意义；用户主动切换走 `setViewMode`，
    // 那条路径下窗口一直是可见的，不存在这个问题。
    const ratio = window.devicePixelRatio || 1

    try {
      let applied = await backend.applyViewMode(get().viewMode)

      for (const delay of [200, 300, 400]) {
        await new Promise((resolve) => {
          window.setTimeout(resolve, delay)
        })

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
    } catch (error) {
      // 尺寸没对上不至于让侧栏不可用，提示一次即可。
      useUiStore
        .getState()
        .notify('error', `应用窗口尺寸失败：${describeError(toAppError(error))}`)
    }
  },

  async resetWindowSize() {
    // 尺寸覆盖是后端在捕获到拖拽时直接写进配置的；清除时只更新这个字段，
    // 不会用旧的完整 Settings 快照覆盖窗口位置和尺寸。
    try {
      const fresh = await backend.getSettings()
      if (!fresh.window.sizeOverride) {
        // 没有覆盖时这次操作退化成「按设计稿重新摆放窗口」，仍然有意义
        // （比如窗口被系统顶宽过），所以照常走一遍几何下发。
        await backend.applyViewMode(get().viewMode)
        useUiStore.getState().notify('info', '窗口尺寸已按设计稿重新计算')
        return
      }

      const saved = await backend.clearWindowSizeOverride()
      set({ settings: saved })
      await backend.applyViewMode(get().viewMode)
    } catch (error) {
      useUiStore.getState().notify('error', `重置窗口尺寸失败：${describeError(toAppError(error))}`)
    }
  },

  async setAutoCollapse(autoCollapse) {
    const settings = get().settings
    if (!settings) {
      return
    }

    const previous = settings
    set({ settings: { ...settings, autoCollapse } })
    try {
      const saved = await backend.updateAutoCollapse(autoCollapse)
      set({ settings: saved })
    } catch (error) {
      set({ settings: previous })
      useUiStore.getState().notify('error', `保存配置失败：${describeError(toAppError(error))}`)
    }
  },

  async setAutoStart(autoStart) {
    const settings = get().settings
    if (!settings) {
      return
    }

    const previous = settings
    set({ settings: { ...settings, autoStart } })
    try {
      const saved = await backend.updateAutoStart(autoStart)
      set({ settings: saved })
    } catch (error) {
      set({ settings: previous })
      useUiStore.getState().notify('error', `保存配置失败：${describeError(toAppError(error))}`)
    }
  },
}))

type SetState = (
  partial: Partial<SettingsStore> | ((state: SettingsStore) => Partial<SettingsStore>),
) => void

/**
 * 收束动画播完之后调用：真正把窗口缩成指示条。
 *
 * 顺序不能反 —— 窗口一缩，面板的可视区域就只剩 3×80，动画没有地方播，
 * 观感就是「啪」地一下消失，正是这次要改掉的手感。
 */
async function commitCollapse(set: SetState): Promise<void> {
  set({ switching: true })
  try {
    await backend.applyViewMode('collapsed')
    set((state) => ({
      viewMode: 'collapsed',
      // 面板已经卸载，清掉收束态，下次展开才会从丝状开始。
      stageConverged: false,
      switching: false,
      settings: state.settings ? { ...state.settings, viewMode: 'collapsed' } : state.settings,
    }))
  } catch (error) {
    set({ stageConverged: false, switching: false })
    useUiStore.getState().notify('error', `切换侧栏状态失败：${describeError(toAppError(error))}`)
  }
}

/**
 * 展开：窗口先变大，面板再从丝状展开。
 *
 * 与收起不同，这里**不能**等动画再改窗口：那会让「鼠标移到指示条上」到
 * 「侧栏出现」之间凭空多出 200ms，反而更迟钝。做法是把面板先按丝状挂上去
 * —— 此刻窗口还是收起态那么大，面板本来就是 3×80，与指示条重合，看不到
 * 切换 —— 等窗口变大后再放开，视觉上就是指示条自己长成了侧栏。
 */
async function commitExpand(set: SetState, get: () => SettingsStore): Promise<void> {
  const { settings } = get()
  set({ switching: true, viewMode: 'expanded', stageConverged: true })

  try {
    await backend.applyViewMode('expanded')
    if (settings) {
      set({ settings: { ...settings, viewMode: 'expanded' } })
    }
    await focusMainWindow()
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

  // 等浏览器真的把「丝状」这一帧画出来再放开：同一帧内加类又删类，浏览器
  // 看不出状态变化，过渡不会跑。两帧是为了躲开 React 提交到绘制之间的间隙。
  requestAnimationFrame(() => requestAnimationFrame(release))
}
