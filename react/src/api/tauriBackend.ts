import { invoke } from '@tauri-apps/api/core'

/** 与后端 `commands::catalog::ICON_READY_EVENT` 保持一致。 */
const ICON_READY_EVENT = 'dc://icon-ready'

import type {
  AppData,
  AppEXE,
  Category,
  Group,
} from '../types/catalog'
import type { Settings, ViewMode, WindowInitResponse } from '../types/settings'
import type { Backend } from './backend'

/**
 * 真实 Tauri 后端实现。
 *
 * Tauri 会自动把 JS 的 camelCase 参数名映射到 Rust 的 snake_case 形参，
 * 因此这里提交 `parentId` / `sourcePath` / `viewMode` 即可。
 */
export const tauriBackend: Backend = {
  isTauri: true,

  getWindowInit: () => invoke<WindowInitResponse>('is_init'),

  applyViewMode: (viewMode: ViewMode) =>
    invoke<WindowInitResponse>('apply_view_mode', { viewMode }),

  getSettings: () => invoke<Settings>('get_settings'),

  updateSettings: (settings: Settings) =>
    invoke<Settings>('update_settings', { settings }),

  getAllData: () => invoke<AppData>('get_all_data'),

  addCategory: (name: string) => invoke<Category>('add_category', { name }),

  renameCategory: (id: string, name: string) =>
    invoke<Category>('rename_category', { id, name }),

  deleteCategory: (id: string) => invoke<void>('delete_category', { id }),

  addGroup: (parentId: string, name: string) =>
    invoke<Group>('add_group', { parentId, name }),

  renameGroup: (id: string, name: string) =>
    invoke<Group>('rename_group', { id, name }),

  deleteGroup: (id: string) => invoke<void>('delete_group', { id }),

  // `parentId` 传 `null`：Rust 侧的形参是 `Option<String>`，null 即顶层。
  addApp: (
    parentId: string | null,
    sourcePath: string,
    name?: string,
    rootIndex?: number,
  ) => invoke<AppEXE>('add_app', { parentId, sourcePath, name, rootIndex }),

  moveApp: (id: string, parentId: string | null, beforeId: string | null) =>
    invoke<void>('move_app', { id, parentId, beforeId }),

  renameApp: (id: string, name: string) =>
    invoke<AppEXE>('rename_app', { id, name }),

  deleteApp: (id: string) => invoke<void>('delete_app', { id }),

  launchIcon: (id: string) => invoke<void>('launch_icon', { id }),

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
