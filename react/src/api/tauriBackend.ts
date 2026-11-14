// 真实 Tauri 后端：invoke 参数用 camelCase，Tauri 自动映射到 Rust snake_case。
// 平台 API（窗口显示/聚焦/原生拖放）也只在这里 import，src 其他位置不直接碰 @tauri-apps。
import { invoke } from '@tauri-apps/api/core'

import type { AddAppsResponse, AppData, AppEXE, Category, Group } from '../types/catalog'
import type { Settings, Theme, ViewMode, WindowInitResponse } from '../types/settings'
import type { Backend, UrlIconProbe } from './backend'
import { COMMANDS } from './commands'

/** 与后端 commands::catalog::ICON_READY_EVENT 保持一致。 */
const ICON_READY_EVENT = 'dc://icon-ready'

export const tauriBackend: Backend = {
  getWindowInit: () => invoke<WindowInitResponse>(COMMANDS.getWindowInit),
  applyViewMode: (viewMode: ViewMode) => invoke<WindowInitResponse>(COMMANDS.applyViewMode, { viewMode }),
  resizeWindow: (geometry) => invoke<void>(COMMANDS.resizeWindow, { ...geometry }),
  finishWindowResize: () => invoke<void>(COMMANDS.finishWindowResize),
  showWindow: async () => {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    await getCurrentWindow().show()
  },
  focusWindow: async () => {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    await getCurrentWindow().setFocus()
  },

  getSettings: () => invoke<Settings>(COMMANDS.getSettings),
  updateSettings: (settings: Settings) => invoke<Settings>(COMMANDS.updateSettings, { settings }),
  updateAutoCollapse: (enabled: boolean) => invoke<Settings>(COMMANDS.updateAutoCollapse, { request: { enabled } }),
  updateAutoStart: (enabled: boolean) => invoke<Settings>(COMMANDS.updateAutoStart, { request: { enabled } }),
  updateTheme: (theme: Theme) => invoke<Settings>(COMMANDS.updateTheme, { request: { theme } }),
  clearWindowSizeOverride: () => invoke<Settings>(COMMANDS.clearWindowSizeOverride),

  getAllData: () => invoke<AppData>(COMMANDS.getAllData),
  addCategory: (name: string) => invoke<Category>(COMMANDS.addCategory, { name }),
  renameCategory: (id: string, name: string) => invoke<Category>(COMMANDS.renameCategory, { id, name }),
  deleteCategory: (id: string) => invoke<void>(COMMANDS.deleteCategory, { id }),
  addGroup: (parentId: string, name: string) => invoke<Group>(COMMANDS.addGroup, { parentId, name }),
  renameGroup: (id: string, name: string) => invoke<Group>(COMMANDS.renameGroup, { id, name }),
  deleteGroup: (id: string) => invoke<void>(COMMANDS.deleteGroup, { id }),
  setNodeCollapsed: (id: string, isCollapsed: boolean) =>
    invoke<void>(COMMANDS.setNodeCollapsed, { id, isCollapsed }),
  addApp: (parentId: string | null, sourcePath: string, name?: string, rootIndex?: number) =>
    invoke<AppEXE>(COMMANDS.addApp, { parentId, sourcePath, name, rootIndex }),
  addApps: (parentId: string | null, sourcePaths: string[], rootIndex?: number) =>
    invoke<AddAppsResponse>(COMMANDS.addApps, { parentId, sourcePaths, rootIndex }),
  moveApp: (id: string, parentId: string | null, beforeId: string | null) =>
    invoke<void>(COMMANDS.moveApp, { id, parentId, beforeId }),
  moveGroup: (id: string, parentId: string, beforeId: string | null) =>
    invoke<void>(COMMANDS.moveGroup, { id, parentId, beforeId }),
  renameApp: (id: string, name: string) => invoke<AppEXE>(COMMANDS.renameApp, { id, name }),
  deleteApp: (id: string) => invoke<void>(COMMANDS.deleteApp, { id }),
  launchIcon: (id: string) => invoke<void>(COMMANDS.launchIcon, { id }),

  async onIconReady(listener) {
    const { listen } = await import('@tauri-apps/api/event')
    const unlisten = await listen<{ appId: string; iconUrl: string }>(ICON_READY_EVENT, (event) =>
      listener(event.payload),
    )
    return unlisten
  },

  async onFileDrop(listener) {
    const { getCurrentWebview } = await import('@tauri-apps/api/webview')
    const unlisten = await getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload
      if (payload.type === 'leave') {
        listener({ type: 'leave', paths: [], position: { x: 0, y: 0 } })
        return
      }
      listener({
        type: payload.type,
        paths: payload.type === 'drop' ? payload.paths : [],
        position: payload.position,
      })
    })
    return unlisten
  },

  probeUrlIcon: (url: string) => invoke<UrlIconProbe>(COMMANDS.probeUrlIcon, { url }),
}