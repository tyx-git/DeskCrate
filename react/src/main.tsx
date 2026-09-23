import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

import './index.css'
import App from './App'
import { backend } from './api'
import { runBootstrap } from './lib/bootstrapController'
import { showMainWindow } from './lib/window'
import { applyWindowSpecCssVariables } from './spec/windowSpec'
import { useCatalogStore } from './state/catalogStore'
import { useSettingsStore } from './state/settingsStore'
import { useUiStore } from './state/uiStore'

/**
 * 启动顺序：
 * 1. `is_init` —— 让后端按当前显示器工作区摆好悬浮窗（首次启动按设计稿
 *    百分比计算并持久化，之后恢复已保存的位置与尺寸）；
 * 2. 并行读取目录数据与配置；
 * 3. 显示窗口；
 * 4. 显示之后再套一次几何。
 *
 * 窗口在 `tauri.conf.json` 里配置为 `visible: false`，必须由前端显式显示，
 * 否则启动瞬间会看到窗口从默认尺寸跳到侧栏尺寸的闪烁。
 *
 * 第 4 步不是冗余：窗口隐藏期间改的尺寸会被 Windows 在显示时顶到系统最小窗口
 * 宽度，收起态那 29 物理像素宽的窗口会被撑成一大块白条。详见
 * `settingsStore.syncWindowGeometry`。
 */
const container = document.getElementById('root')
if (!container) {
  throw new Error('找不到 #root 挂载点')
}

applyWindowSpecCssVariables()

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
)

void runBootstrap({
  subscribeIconReady: () =>
    backend.onIconReady(({ appId, iconUrl }) => {
      useCatalogStore.getState().applyIcon(appId, iconUrl)
    }),
  initializeWindow: () => backend.getWindowInit(),
  loadCatalog: () => useCatalogStore.getState().load(),
  loadSettings: () => useSettingsStore.getState().load(),
  showWindow: showMainWindow,
  syncWindowGeometry: () => useSettingsStore.getState().syncWindowGeometry(),
  notify: (message) => useUiStore.getState().notify('error', message),
})
