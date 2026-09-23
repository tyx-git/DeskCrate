import { create } from 'zustand'

import { backend, describeError, toAppError } from '../api'
import {
  appendCategory as appendCategoryData,
  appendChild as appendChildData,
  insertApp as insertAppData,
  patchIcon as patchIconData,
  removeApp as removeAppData,
  removeCategory as removeCategoryData,
  removeGroup as removeGroupData,
  renameApp as renameAppData,
  renameCategory as renameCategoryData,
  renameGroup as renameGroupData,
  toggleCollapse as toggleCollapseData,
} from '../lib/catalogMutations'
import type { AppData, AppEXE, Category, Group } from '../types/catalog'
import type { AddAppsResponse } from '../types/catalog'
import type { AppError } from '../types/error'
import { useUiStore } from './uiStore'

interface CatalogState {
  data: AppData
  status: 'loading' | 'ready' | 'error'
  error: AppError | null
  /** 是否有写操作正在进行，用于抑制重复提交。 */
  busy: boolean

  load: () => Promise<void>

  addCategory: (name: string) => Promise<Category | null>
  renameCategory: (id: string, name: string) => Promise<boolean>
  deleteCategory: (id: string) => Promise<boolean>

  addGroup: (parentId: string, name: string) => Promise<Group | null>
  renameGroup: (id: string, name: string) => Promise<boolean>
  deleteGroup: (id: string) => Promise<boolean>

  /**
   * `parentId` 为 `null` 时放到顶层，`rootIndex` 指定插到第几个根节点之前
   * （省略则追加到最下方）；放进分类或分组时忽略 `rootIndex`。
   */
  addApp: (
    parentId: string | null,
    sourcePath: string,
    name?: string,
    rootIndex?: number,
  ) => Promise<AppEXE | null>
  addApps: (
    parentId: string | null,
    sourcePaths: string[],
    rootIndex?: number,
  ) => Promise<AddAppsResponse>
  /**
   * 把一个应用移动到新位置（拖拽换位）。
   *
   * `parentId` 为 `null` 表示移到顶层；`beforeId` 是参照节点（插到它之前），
   * `null` 表示插到目标容器末尾。
   */
  moveApp: (
    id: string,
    parentId: string | null,
    beforeId: string | null,
  ) => Promise<boolean>
  moveGroup: (id: string, parentId: string, beforeId: string | null) => Promise<boolean>
  renameApp: (id: string, name: string) => Promise<boolean>
  deleteApp: (id: string) => Promise<boolean>

  launchApp: (app: AppEXE) => Promise<void>
  toggleCollapse: (id: string, isCollapsed: boolean) => Promise<void>
  /**
   * 把某个应用的图标换掉，用于后台补抓到网址图标之后回填。
   *
   * 只改 `data` 里那一个应用的 `iconUrl`，不重新拉整份数据 —— 图标到达是高频
   * 小更新，整份重拉会让侧栏无谓地重渲染。
   */
  applyIcon: (appId: string, iconUrl: string) => void
}

const EMPTY: AppData = { version: 2, items: [] }
const collapseRequestVersions = new Map<string, number>()
// 后端补图事件可能早于 add_app IPC 返回到达。事件到达时应用还不在
// Zustand 树中就先暂存，待 addApp/addApps 把应用插入后立即回填，避免
// 快速站点（例如 GitHub）被占位地球图标覆盖。
const pendingIconUrls = new Map<string, string>()

function insertAppAndApplyPendingIcon(
  data: AppData,
  app: AppEXE,
  parentId: string | null,
  rootIndex?: number,
): AppData {
  const inserted = insertAppData(data, app, parentId, rootIndex)
  const pendingIconUrl = pendingIconUrls.get(app.id)
  if (pendingIconUrl === undefined) {
    return inserted
  }

  pendingIconUrls.delete(app.id)
  return patchIconData(inserted, app.id, pendingIconUrl)
}

function applyPendingIcons(data: AppData): AppData {
  let next = data
  for (const [appId, iconUrl] of pendingIconUrls) {
    const patched = patchIconData(next, appId, iconUrl)
    if (patched !== next) {
      pendingIconUrls.delete(appId)
      next = patched
    }
  }
  return next
}

// 目录树变换已抽到 `lib/catalogMutations.ts`；这里保留 IPC 编排、状态提交和提示。
export const useCatalogStore = create<CatalogState>((set, get) => ({
  data: EMPTY,
  status: 'loading',
  error: null,
  busy: false,

  async load() {
    set({ status: 'loading', error: null })
    try {
      const data = await backend.getAllData()
      set({ data: applyPendingIcons(data), status: 'ready' })
    } catch (error) {
      set({ status: 'error', error: toAppError(error) })
    }
  },

  async addCategory(name) {
    return run(set, get, null, async () => {
      const category = await backend.addCategory(name)
      set((state) => ({
        data: appendCategoryData(state.data, category),
      }))
      return category
    })
  },

  async renameCategory(id, name) {
    return run(set, get, false, async () => {
      const updated = await backend.renameCategory(id, name)
      set((state) => ({
        data: renameCategoryData(state.data, id, updated.name),
      }))
      return true
    })
  },

  async deleteCategory(id) {
    return run(set, get, false, async () => {
      await backend.deleteCategory(id)
      set((state) => ({
        data: removeCategoryData(state.data, id),
      }))
      return true
    })
  },

  async addGroup(parentId, name) {
    return run(set, get, null, async () => {
      const group = await backend.addGroup(parentId, name)
      set((state) => ({
        data: appendChildData(state.data, parentId, { Group: group }),
      }))
      return group
    })
  },

  async renameGroup(id, name) {
    return run(set, get, false, async () => {
      const updated = await backend.renameGroup(id, name)
      set((state) => ({
        data: renameGroupData(state.data, id, updated.name),
      }))
      return true
    })
  },

  async deleteGroup(id) {
    return run(set, get, false, async () => {
      await backend.deleteGroup(id)
      set((state) => ({
        data: removeGroupData(state.data, id),
      }))
      return true
    })
  },

  async addApp(parentId, sourcePath, name, rootIndex) {
    return run(set, get, null, async () => {
      const app = await backend.addApp(parentId, sourcePath, name, rootIndex)
      set((state) => ({
        data: insertAppAndApplyPendingIcon(state.data, app, parentId, rootIndex),
      }))
      return app
    })
  },

  async addApps(parentId, sourcePaths, rootIndex) {
    return run(set, get, { added: [], failed: [] }, async () => {
      const result = await backend.addApps(parentId, sourcePaths, rootIndex)
      if (result.added.length > 0) {
        set((state) => {
          const data = result.added.reduce(
            (current, app) =>
              insertAppAndApplyPendingIcon(current, app, parentId, rootIndex),
            state.data,
          )
          return { data }
        })
      }
      return result
    })
  },

  async moveApp(id, parentId, beforeId) {
    // 落盘由后端负责（它才知道最终顺序）；这里只负责事后重新拉一次，
    // 免得前端的乐观更新和后端算出来的顺序不一致 —— 排序规则在后端。
    return run(set, get, false, async () => {
      await backend.moveApp(id, parentId, beforeId)
      const fresh = await backend.getAllData()
      set({ data: applyPendingIcons(fresh) })
      return true
    })
  },

  async moveGroup(id, parentId, beforeId) {
    return run(set, get, false, async () => {
      await backend.moveGroup(id, parentId, beforeId)
      const fresh = await backend.getAllData()
      set({ data: applyPendingIcons(fresh) })
      return true
    })
  },

  async renameApp(id, name) {
    return run(set, get, false, async () => {
      const updated = await backend.renameApp(id, name)
      set((state) => ({ data: renameAppData(state.data, id, updated.name) }))
      return true
    })
  },

  async deleteApp(id) {
    return run(set, get, false, async () => {
      await backend.deleteApp(id)
      set((state) => ({
        data: removeAppData(state.data, id),
      }))
      return true
    })
  },

  async launchApp(app) {
    try {
      await backend.launchIcon(app.id)
    } catch (error) {
      useUiStore
        .getState()
        .notify('error', `启动「${app.name}」失败：${describeError(toAppError(error))}`)
    }
  },

  applyIcon(appId, iconUrl) {
    set((state) => {
      const data = patchIconData(state.data, appId, iconUrl)
      if (data === state.data) {
        pendingIconUrls.set(appId, iconUrl)
      } else {
        pendingIconUrls.delete(appId)
      }
      return { data }
    })
  },

  async toggleCollapse(id, isCollapsed) {
    const version = (collapseRequestVersions.get(id) ?? 0) + 1
    collapseRequestVersions.set(id, version)
    set((state) => ({ data: toggleCollapseData(state.data, id, isCollapsed) }))
    try {
      await backend.setNodeCollapsed(id, isCollapsed)
    } catch (error) {
      if (collapseRequestVersions.get(id) === version) {
        set((state) => ({ data: toggleCollapseData(state.data, id, !isCollapsed) }))
      }
      useUiStore.getState().notify('error', `保存折叠状态失败：${describeError(toAppError(error))}`)
    }
  },
}))


/**
 * 统一的写操作外壳：置忙、捕获后端错误并转成提示，失败时返回 `fallback`。
 *
 * 每个 action 都写一遍 try/catch/finally 只会让 store 变长，这里集中一次。
 * 忙碌时直接返回 `fallback`，避免重复提交。
 */
async function run<T>(
  set: (partial: Partial<CatalogState>) => void,
  get: () => CatalogState,
  fallback: T,
  operation: () => Promise<T>,
): Promise<T> {
  if (get().busy) {
    return fallback
  }

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
