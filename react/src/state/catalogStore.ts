// 目录数据 store：结构变更一律走后端命令，成功后回读权威树；删除应用可撤销。
import { create } from 'zustand'

import { backend, describeError, toAppError } from '../api'
import { collectCollapseTargets, locateApp, nextAppId } from '../lib/tree'
import type { AddAppsResponse, AppData, AppEXE, Category, Group } from '../types/catalog'
import type { DropTarget } from '../types/drop'
import type { AppError } from '../types/error'
import { useUiStore } from './uiStore'

interface DeletedAppSnapshot {
  parentId: string | null
  rootIndex?: number
  sourcePath: string
  name: string
  beforeId: string | null
}

interface CatalogState {
  data: AppData
  status: 'loading' | 'ready' | 'error'
  error: AppError | null
  busy: boolean

  load: () => Promise<void>
  addCategory: (name: string) => Promise<Category | null>
  renameCategory: (id: string, name: string) => Promise<boolean>
  deleteCategory: (id: string) => Promise<boolean>
  addGroup: (parentId: string, name: string) => Promise<Group | null>
  renameGroup: (id: string, name: string) => Promise<boolean>
  deleteGroup: (id: string) => Promise<boolean>
  addApp: (parentId: string | null, sourcePath: string, name?: string, rootIndex?: number) => Promise<AppEXE | null>
  addApps: (parentId: string | null, sourcePaths: string[], rootIndex?: number) => Promise<AddAppsResponse>
  moveApp: (id: string, parentId: string | null, beforeId: string | null) => Promise<boolean>
  moveGroup: (id: string, parentId: string, beforeId: string | null) => Promise<boolean>
  dropApp: (id: string, target: DropTarget) => Promise<boolean>
  dropGroup: (id: string, target: DropTarget) => Promise<boolean>
  dropCategory: (id: string, target: DropTarget) => Promise<boolean>
  renameApp: (id: string, name: string) => Promise<boolean>
  deleteApp: (id: string) => Promise<boolean>
  launchApp: (app: AppEXE) => Promise<void>
  toggleCollapse: (id: string, isCollapsed: boolean) => Promise<void>
  setAllCollapsed: (isCollapsed: boolean) => Promise<void>
}

const EMPTY: AppData = { version: 2, items: [] }

export const useCatalogStore = create<CatalogState>((set, get) => ({
  data: EMPTY,
  status: 'loading',
  error: null,
  busy: false,

  async load() {
    set({ status: 'loading', error: null })
    try {
      const data = await backend.getAllData()
      set({ data, status: 'ready' })
    } catch (error) {
      set({ status: 'error', error: toAppError(error) })
    }
  },

  async addCategory(name) {
    return run(set, get, null, async () => {
      const category = await backend.addCategory(name)
      await get().load()
      return category
    })
  },

  async renameCategory(id, name) {
    return run(set, get, false, async () => {
      await backend.renameCategory(id, name)
      await get().load()
      return true
    })
  },

  async deleteCategory(id) {
    return run(set, get, false, async () => {
      await backend.deleteCategory(id)
      await get().load()
      return true
    })
  },

  async addGroup(parentId, name) {
    return run(set, get, null, async () => {
      const group = await backend.addGroup(parentId, name)
      await get().load()
      return group
    })
  },

  async renameGroup(id, name) {
    return run(set, get, false, async () => {
      await backend.renameGroup(id, name)
      await get().load()
      return true
    })
  },

  async deleteGroup(id) {
    return run(set, get, false, async () => {
      await backend.deleteGroup(id)
      await get().load()
      return true
    })
  },

  async addApp(parentId, sourcePath, name, rootIndex) {
    return run(set, get, null, async () => {
      const app = await backend.addApp(parentId, sourcePath, name, rootIndex)
      await get().load()
      return app
    })
  },

  async addApps(parentId, sourcePaths, rootIndex) {
    return run(set, get, { added: [], failed: [] }, async () => {
      const result = await backend.addApps(parentId, sourcePaths, rootIndex)
      await get().load()
      return result
    })
  },

  async moveApp(id, parentId, beforeId) {
    return run(set, get, false, async () => {
      await backend.moveApp(id, parentId, beforeId)
      await get().load()
      return true
    })
  },

  async moveGroup(id, parentId, beforeId) {
    return run(set, get, false, async () => {
      await backend.moveGroup(id, parentId, beforeId)
      await get().load()
      return true
    })
  },

  async dropApp(id, target) {
    return run(set, get, false, async () => {
      await backend.dropApp(id, target)
      await get().load()
      return true
    })
  },

  async dropGroup(id, target) {
    return run(set, get, false, async () => {
      await backend.dropGroup(id, target)
      await get().load()
      return true
    })
  },

  async dropCategory(id, target) {
    return run(set, get, false, async () => {
      await backend.dropCategory(id, target)
      await get().load()
      return true
    })
  },

  async renameApp(id, name) {
    return run(set, get, false, async () => {
      await backend.renameApp(id, name)
      await get().load()
      return true
    })
  },

  async deleteApp(id) {
    const loc = locateApp(get().data, id)
    if (!loc) return false
    const snapshot: DeletedAppSnapshot = {
      parentId: loc.parentId,
      rootIndex: loc.rootIndex,
      sourcePath: loc.app.sourcePath,
      name: loc.app.name,
      beforeId: nextAppId(loc.list, loc.index),
    }
    const ok = await run(set, get, false, async () => {
      await backend.deleteApp(id)
      await get().load()
      return true
    })
    if (ok) {
      useUiStore.getState().pushToast(`已删除「${snapshot.name}」`, {
        icon: 'bin',
        actionLabel: '撤销',
        onAction: () => {
          void restoreApp(get(), snapshot)
        },
      })
    }
    return ok
  },

  async launchApp(app) {
    useUiStore.getState().setLaunching(app.id)
    window.setTimeout(() => {
      if (useUiStore.getState().launchingId === app.id) useUiStore.getState().setLaunching(null)
    }, 520)
    try {
      await backend.launchIcon(app.id)
    } catch (error) {
      useUiStore.getState().notify('error', `启动「${app.name}」失败：${describeError(toAppError(error))}`)
    }
  },

  async toggleCollapse(id, isCollapsed) {
    try {
      await backend.setNodeCollapsed(id, isCollapsed)
      await get().load()
    } catch (error) {
      useUiStore.getState().notify('error', `保存折叠状态失败：${describeError(toAppError(error))}`)
    }
  },

  async setAllCollapsed(isCollapsed) {
    const ids = collectCollapseTargets(get().data, isCollapsed)
    if (ids.length === 0) return
    try {
      await Promise.all(ids.map((id) => backend.setNodeCollapsed(id, isCollapsed)))
    } catch (error) {
      useUiStore.getState().notify('error', `保存折叠状态失败：${describeError(toAppError(error))}`)
    }
    await get().load()
  },
}))

/** 撤销删除：重新添加并尽量回到原位（后端会生成新 ID 并重抓图标）。 */
async function restoreApp(state: CatalogState, snapshot: DeletedAppSnapshot): Promise<void> {
  const app = await state.addApp(snapshot.parentId, snapshot.sourcePath, snapshot.name, snapshot.rootIndex)
  if (!app) return
  if (snapshot.beforeId) {
    await state.moveApp(app.id, snapshot.parentId, snapshot.beforeId)
  } else if (snapshot.parentId === null) {
    await state.moveApp(app.id, null, null)
  }
  useUiStore.getState().pushToast('已恢复删除的应用', { icon: 'undo' })
}

/** 写操作外壳：置忙、捕获后端错误转提示，失败返回 fallback，忙碌时拒绝重复提交。 */
async function run<T>(
  set: (partial: Partial<CatalogState>) => void,
  get: () => CatalogState,
  fallback: T,
  operation: () => Promise<T>,
): Promise<T> {
  if (get().busy) return fallback
  set({ busy: true })
  try {
    return await operation()
  } catch (error) {
    useUiStore.getState().notify('error', describeError(toAppError(error)))
    return fallback
  } finally {
    set({ busy: false })
  }
}
