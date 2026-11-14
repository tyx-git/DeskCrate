// 应用入口：启动状态机只跑一次，webview 即窗口，面板按阴影留白偏移。
import { useEffect } from 'react'

import { backend } from './api'
import { Panel } from './components/Panel'
import { useWindowSize } from './hooks/useWindowSize'
import { runBootstrap } from './lib/bootstrap'
import { useCatalogStore } from './state/catalogStore'
import { useSettingsStore } from './state/settingsStore'
import { useUiStore } from './state/uiStore'

// StrictMode 开发态会双跑 effect，启动流程（监听/显示窗口）只允许一次。
let bootstrapStarted = false

export default function App() {
  const size = useWindowSize()

  useEffect(() => {
    if (bootstrapStarted) return
    bootstrapStarted = true
    void runBootstrap({
      subscribeIconReady: () => backend.onIconReady(() => useCatalogStore.getState().load()),
      initializeWindow: () => backend.getWindowInit(),
      loadCatalog: () => useCatalogStore.getState().load(),
      loadSettings: () => useSettingsStore.getState().load(),
      showWindow: () => backend.showWindow(),
      syncWindowGeometry: () => useSettingsStore.getState().syncWindowGeometry(),
      notify: (message) => useUiStore.getState().notify('error', message),
    })
  }, [])

  return (
    <div className="dc-window">
      <Panel windowSize={size} />
    </div>
  )
}