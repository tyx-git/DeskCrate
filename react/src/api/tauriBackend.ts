import { invoke } from '@tauri-apps/api/core'

import { COMMANDS } from './commands'

/** 与后端 `commands::catalog::ICON_READY_EVENT` 保持一致。 */
const ICON_READY_EVENT = 'dc://icon-ready'

import type {
  AppData,
  AppEXE,
  Category,
  Group,
  AddAppsResponse,
} from '../types/catalog'
import type { Settings, Theme, ViewMode, WindowInitResponse } from '../types/settings'
import type { Backend } from './backend'

/**
 * 真实 Tauri 后端实现。
 *
 * Tauri 会自动把 JS 的 camelCase 参数名映射到 Rust 的 snake_case 形参，
 * 因此这里提交 `parentId` / `sourcePath` / `viewMode` 即可。
 */
// Command 名称集中在 `api/commands.ts`；参数和返回类型继续由 TypeScript
// 与 Rust 模型共同维护，后续可接入契约生成。
export const tauriBackend: Backend = {
  isTauri: true,

  getWindowInit: () => invoke<WindowInitResponse>(COMMANDS.getWindowInit),

  applyViewMode: (viewMode: ViewMode) =>
    invoke<WindowInitResponse>(COMMANDS.applyViewMode, { viewMode }),

  getSettings: () => invoke<Settings>(COMMANDS.getSettings),

  updateSettings: (settings: Settings) =>
    invoke<Settings>(COMMANDS.updateSettings, { settings }),

  updateAutoCollapse: (enabled: boolean) =>
    invoke<Settings>(COMMANDS.updateAutoCollapse, { request: { enabled } }),

  updateAutoStart: (enabled: boolean) =>
    invoke<Settings>(COMMANDS.updateAutoStart, { request: { enabled } }),

  updateTheme: (theme: Theme) =>
    invoke<Settings>(COMMANDS.updateTheme, { request: { theme } }),

  clearWindowSizeOverride: () =>
    invoke<Settings>(COMMANDS.clearWindowSizeOverride),

  getAllData: () => invoke<AppData>(COMMANDS.getAllData),

  addCategory: (name: string) => invoke<Category>(COMMANDS.addCategory, { name }),

  renameCategory: (id: string, name: string) =>
    invoke<Category>(COMMANDS.renameCategory, { id, name }),

  deleteCategory: (id: string) => invoke<void>(COMMANDS.deleteCategory, { id }),

  addGroup: (parentId: string, name: string) =>
    invoke<Group>(COMMANDS.addGroup, { parentId, name }),

  renameGroup: (id: string, name: string) =>
    invoke<Group>(COMMANDS.renameGroup, { id, name }),

  deleteGroup: (id: string) => invoke<void>(COMMANDS.deleteGroup, { id }),

  setNodeCollapsed: (id: string, isCollapsed: boolean) =>
    invoke<void>(COMMANDS.setNodeCollapsed, { id, isCollapsed }),

  // `parentId` 传 `null`：Rust 侧的形参是 `Option<String>`，null 即顶层。
  addApp: (
    parentId: string | null,
    sourcePath: string,
    name?: string,
    rootIndex?: number,
  ) => invoke<AppEXE>(COMMANDS.addApp, { parentId, sourcePath, name, rootIndex }),

  addApps: (parentId: string | null, sourcePaths: string[], rootIndex?: number) =>
    invoke<AddAppsResponse>(COMMANDS.addApps, { parentId, sourcePaths, rootIndex }),

  moveApp: (id: string, parentId: string | null, beforeId: string | null) =>
    invoke<void>(COMMANDS.moveApp, { id, parentId, beforeId }),

  moveGroup: (id: string, parentId: string, beforeId: string | null) =>
    invoke<void>(COMMANDS.moveGroup, { id, parentId, beforeId }),

  renameApp: (id: string, name: string) =>
    invoke<AppEXE>(COMMANDS.renameApp, { id, name }),

  deleteApp: (id: string) => invoke<void>(COMMANDS.deleteApp, { id }),

  launchIcon: (id: string) => invoke<void>(COMMANDS.launchIcon, { id }),

  // 后台补抓到网址图标后后端会发这个事件；返回一个取消监听的函数。
  async onIconReady(listener) {
    const { listen } = await import('@tauri-apps/api/event')
    const unlisten = await listen<{ appId: string; iconUrl: string }>(
      ICON_READY_EVENT,
      (event) => listener(event.payload),
    )
    return unlisten
  },
}
