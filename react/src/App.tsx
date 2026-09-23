import { isPreviewMode } from './api'
import { DevScene } from './components/DevScene'
import { SidebarShell } from './components/sidebar/SidebarShell'

/**
 * 应用根组件。
 *
 * Tauri 运行时下窗口本身就是侧栏，直接渲染外壳；浏览器里则套一层模拟
 * 桌面场景，方便在没有 Rust 工具链的机器上验收设计稿。
 */
export default function App() {
  if (isPreviewMode) {
    return (
      <DevScene>
        <SidebarShell />
      </DevScene>
    )
  }

  return <SidebarShell />
}
