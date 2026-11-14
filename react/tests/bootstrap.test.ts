// 启动状态机测试：顺序、兜底显示与降级路径，全部用假依赖驱动。
import { describe, expect, it, vi } from 'vitest'

import { runBootstrap, type BootstrapDependencies } from '../src/lib/bootstrap'

function deps(overrides: Partial<BootstrapDependencies> = {}): BootstrapDependencies {
  return {
    subscribeIconReady: vi.fn(async () => () => {}),
    initializeWindow: vi.fn(async () => ({})),
    loadCatalog: vi.fn(async () => {}),
    loadSettings: vi.fn(async () => {}),
    showWindow: vi.fn(async () => {}),
    syncWindowGeometry: vi.fn(async () => {}),
    notify: vi.fn(),
    fallbackDelayMs: 3000,
    schedule: vi.fn(() => 1),
    cancel: vi.fn(),
    ...overrides,
  }
}

describe('runBootstrap', () => {
  it('正常路径：先装监听再初始化，显示后才同步几何', async () => {
    const order: string[] = []
    const d = deps({
      subscribeIconReady: vi.fn(async () => {
        order.push('subscribe')
        return () => {}
      }),
      initializeWindow: vi.fn(async () => {
        order.push('init')
        return {}
      }),
      loadCatalog: vi.fn(async () => {
        order.push('catalog')
      }),
      loadSettings: vi.fn(async () => {
        order.push('settings')
      }),
      showWindow: vi.fn(async () => {
        order.push('show')
      }),
      syncWindowGeometry: vi.fn(async () => {
        order.push('sync')
      }),
    })
    const result = await runBootstrap(d)
    expect(order[0]).toBe('subscribe')
    expect(order[1]).toBe('init')
    expect(order.indexOf('show')).toBeGreaterThan(order.indexOf('catalog'))
    expect(order.indexOf('show')).toBeGreaterThan(order.indexOf('settings'))
    expect(order.indexOf('sync')).toBeGreaterThan(order.indexOf('show'))
    expect(result).toEqual({ phase: 'ready', windowShown: true, geometrySynced: true })
    expect(d.notify).not.toHaveBeenCalled()
    // 显示成功后兜底定时器被取消
    expect(d.cancel).toHaveBeenCalledWith(1)
  })

  it('初始化失败仍加载并显示窗口，进入降级', async () => {
    const d = deps({ initializeWindow: vi.fn(async () => Promise.reject(new Error('boom'))) })
    const result = await runBootstrap(d)
    expect(d.loadCatalog).toHaveBeenCalled()
    expect(d.showWindow).toHaveBeenCalled()
    expect(result.phase).toBe('degraded')
    expect(result.windowShown).toBe(true)
    expect(d.notify).toHaveBeenCalledWith(expect.stringContaining('窗口初始化失败'))
  })

  it('几何同步失败：窗口已显示但标记降级', async () => {
    const d = deps({ syncWindowGeometry: vi.fn(async () => Promise.reject(new Error('sync'))) })
    const result = await runBootstrap(d)
    expect(result.windowShown).toBe(true)
    expect(result.geometrySynced).toBe(false)
    expect(result.phase).toBe('degraded')
  })

  it('主流程卡住时兜底定时器把窗口显示出来', async () => {
    let fallback: (() => void) | undefined
    const d = deps({
      initializeWindow: vi.fn(() => new Promise(() => {})),
      schedule: vi.fn((callback: () => void) => {
        fallback = callback
        return 7
      }),
    })
    void runBootstrap(d)
    expect(d.showWindow).not.toHaveBeenCalled()
    fallback!()
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(d.showWindow).toHaveBeenCalledTimes(1)
  })

  it('显示失败进入降级，兜底触发时再试一次', async () => {
    let fallback: (() => void) | undefined
    const showWindow = vi.fn(async () => {
      throw new Error('show failed')
    })
    const d = deps({
      showWindow,
      schedule: vi.fn((callback: () => void) => {
        fallback = callback
        return 7
      }),
    })
    const result = await runBootstrap(d)
    expect(result.windowShown).toBe(false)
    expect(result.phase).toBe('degraded')
    fallback!()
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(showWindow).toHaveBeenCalledTimes(2)
    expect(d.notify).toHaveBeenCalledWith(expect.stringContaining('显示窗口失败'))
  })
})