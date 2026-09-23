import { describeError, toAppError } from '../api/client'

/** 启动阶段，按窗口初始化、数据加载、显示和几何同步的顺序推进。 */
export type BootstrapPhase =
  | 'created'
  | 'initialized'
  | 'loaded'
  | 'shown'
  | 'geometry-synced'
  | 'ready'
  | 'degraded'

export interface BootstrapResult {
  phase: BootstrapPhase
  windowShown: boolean
  geometrySynced: boolean
}

export interface BootstrapDependencies {
  /** 在窗口显示前安装后台图标就绪事件，避免添加网址过快时丢事件。 */
  subscribeIconReady?: () => Promise<() => void>
  initializeWindow: () => Promise<unknown>
  loadCatalog: () => Promise<void>
  loadSettings: () => Promise<void>
  showWindow: () => Promise<void>
  syncWindowGeometry: () => Promise<void>
  notify: (message: string) => void
  fallbackDelayMs?: number
  schedule?: (callback: () => void, delayMs: number) => number
  cancel?: (handle: number) => void
}

const DEFAULT_FALLBACK_DELAY_MS = 3_000

/**
 * 执行应用启动状态机。
 *
 * 依赖通过参数注入，浏览器 API、Zustand 和 Tauri 都不直接写进状态机，
 * 因而可以用假的依赖测试初始化失败、显示重试和几何同步顺序。
 */
export async function runBootstrap(
  dependencies: BootstrapDependencies,
): Promise<BootstrapResult> {
  const schedule =
    dependencies.schedule ?? ((callback: () => void, delayMs: number) => window.setTimeout(callback, delayMs))
  const cancel = dependencies.cancel ?? ((handle: number) => window.clearTimeout(handle))
  const fallbackDelay = dependencies.fallbackDelayMs ?? DEFAULT_FALLBACK_DELAY_MS

  let phase: BootstrapPhase = 'created'
  let degraded = false
  let windowShown = false
  let geometrySynced = false
  let showInFlight: Promise<boolean> | null = null
  let fallbackHandle: number | undefined

  const reportFailure = (label: string, error: unknown) => {
    degraded = true
    dependencies.notify(`${label}：${describeError(toAppError(error))}`)
  }

  const cancelFallback = () => {
    if (fallbackHandle === undefined) {
      return
    }
    cancel(fallbackHandle)
    fallbackHandle = undefined
  }

  const showWindow = (): Promise<boolean> => {
    if (windowShown) {
      return Promise.resolve(true)
    }
    if (showInFlight) {
      return showInFlight
    }

    showInFlight = (async () => {
      try {
        await dependencies.showWindow()
        windowShown = true
        cancelFallback()
        return true
      } catch (error) {
        reportFailure('显示窗口失败', error)
        return false
      } finally {
        showInFlight = null
      }
    })()

    return showInFlight
  }

  // 先安装兜底，再等待动态导入事件模块；即使监听模块加载失败，窗口也不会永久隐藏。
  fallbackHandle = schedule(() => {
    fallbackHandle = undefined
    void showWindow()
  }, fallbackDelay)

  // 监听必须先于窗口显示和用户交互建立。旧实现放在 App 的 useEffect 中，
  // 动态 import 与 React 副作用执行都可能晚于第一个网址应用的后台补图事件。
  if (dependencies.subscribeIconReady) {
    try {
      await dependencies.subscribeIconReady()
    } catch (error) {
      reportFailure('安装图标更新监听失败', error)
    }
  }

  try {
    await dependencies.initializeWindow()
    phase = 'initialized'
  } catch (error) {
    reportFailure('窗口初始化失败', error)
  }

  // 读取一次阶段，既保留状态机语义，也明确初始化失败后继续加载数据的策略。
  if (phase !== 'initialized') {
    degraded = true
  }

  // 两个 store 自己处理数据/配置读取错误，启动控制器只负责等待它们完成。
  await Promise.all([dependencies.loadCatalog(), dependencies.loadSettings()])
  phase = 'loaded'

  const shown = await showWindow()
  if (shown) {
    phase = 'shown'
  }

  try {
    await dependencies.syncWindowGeometry()
    geometrySynced = true
    if (shown) {
      phase = 'geometry-synced'
    }
  } catch (error) {
    reportFailure('同步窗口几何失败', error)
  }

  if (!degraded && windowShown && geometrySynced) {
    phase = 'ready'
  } else if (degraded) {
    phase = 'degraded'
  }

  return { phase, windowShown, geometrySynced }
}
