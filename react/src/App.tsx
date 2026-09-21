import { useEffect } from 'react'

import { backend, isPreviewMode } from './api'
import { DevScene } from './components/DevScene'
import { SidebarShell } from './components/sidebar/SidebarShell'
import { useCatalogStore } from './state/catalogStore'

/**
 * 应用根组件。
 *
 * Tauri 运行时下窗口本身就是侧栏，直接渲染外壳；浏览器里则套一层模拟
 * 桌面场景，方便在没有 Rust 工具链的机器上验收设计稿。
 */
export default function App() {
  // 网址图标是后补的：添加网址时先用内置图标落库，后台抓到真实图标后发事件，
  // 这里监听并把占位图标换掉。监听挂在根组件上，保证整个生命周期只挂一次。
  useEffect(() => {
    let cancelled = false
    let unlisten: (() => void) | undefined

    void backend
      .onIconReady(({ appId, iconUrl }) => {
        if (cancelled) return
        useCatalogStore.getState().applyIcon(appId, iconUrl)
      })
      .then((dispose) => {
        // 组件可能在监听建立完成前就卸载了（热更新时很常见）。
        if (cancelled) {
          dispose()
          return
        }
        unlisten = dispose
      })

    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [])

  if (isPreviewMode) {
    return (
      <DevScene>
        <SidebarShell />
      </DevScene>
    )
  }

  return <SidebarShell />
}
