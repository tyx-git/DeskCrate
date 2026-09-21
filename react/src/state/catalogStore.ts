import { create } from 'zustand'

import { backend, describeError, toAppError } from '../api'
import { readCollapseMap, writeCollapse } from '../lib/collapseStorage'
import { insertRootApp, removeRootApp } from '../lib/tree'
import { isAppsRoot } from '../types/catalog'
import type { AppData, AppEXE, Category, Group, Item, RootItem } from '../types/catalog'
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
  renameApp: (id: string, name: string) => Promise<boolean>
  deleteApp: (id: string) => Promise<boolean>

  launchApp: (app: AppEXE) => Promise<void>
  toggleCollapse: (id: string, isCollapsed: boolean) => void
  /**
   * 把某个应用的图标换掉，用于后台补抓到网址图标之后回填。
   *
   * 只改 `data` 里那一个应用的 `iconUrl`，不重新拉整份数据 —— 图标到达是高频
   * 小更新，整份重拉会让侧栏无谓地重渲染。
   */
  applyIcon: (appId: string, iconUrl: string) => void
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
      set({ data: applyCollapseOverrides(data), status: 'ready' })
    } catch (error) {
      set({ status: 'error', error: toAppError(error) })
    }
  },

  async addCategory(name) {
    return run(set, get, null, async () => {
      const category = await backend.addCategory(name)
      set((state) => ({
        data: { ...state.data, items: [...state.data.items, { Category: category }] },
      }))
      return category
    })
  },

  async renameCategory(id, name) {
    return run(set, get, false, async () => {
      const updated = await backend.renameCategory(id, name)
      set((state) => ({
        data: { ...state.data, items: mapCategories(state.data.items, id, (category) => ({ ...category, name: updated.name })) },
      }))
      return true
    })
  },

  async deleteCategory(id) {
    return run(set, get, false, async () => {
      await backend.deleteCategory(id)
      set((state) => ({
        data: {
          ...state.data,
          items: state.data.items.filter(
            (item) => !('Category' in item && item.Category.id === id),
          ),
        },
      }))
      return true
    })
  },

  async addGroup(parentId, name) {
    return run(set, get, null, async () => {
      const group = await backend.addGroup(parentId, name)
      set((state) => ({
        data: { ...state.data, items: appendToParent(state.data.items, parentId, { Group: group }) },
      }))
      return group
    })
  },

  async renameGroup(id, name) {
    return run(set, get, false, async () => {
      const updated = await backend.renameGroup(id, name)
      set((state) => ({
        data: {
          ...state.data,
          items: mapCategoryChildren(state.data.items, (category) => ({
            ...category,
            groups: renameInItems(category.groups, id, updated.name),
          })),
        },
      }))
      return true
    })
  },

  async deleteGroup(id) {
    return run(set, get, false, async () => {
      await backend.deleteGroup(id)
      set((state) => ({
        data: {
          ...state.data,
          items: mapCategoryChildren(state.data.items, (category) => ({
            ...category,
            groups: removeNode(category.groups, id),
          })),
        },
      }))
      return true
    })
  },

  async addApp(parentId, sourcePath, name, rootIndex) {
    return run(set, get, null, async () => {
      const app = await backend.addApp(parentId, sourcePath, name, rootIndex)
      set((state) => ({
        data: {
          ...state.data,
          items:
            parentId === null
              ? insertRootApp(state.data.items, app, rootIndex ?? Number.MAX_SAFE_INTEGER)
              : appendToParent(state.data.items, parentId, { App: app }),
        },
      }))
      return app
    })
  },

  async moveApp(id, parentId, beforeId) {
    // 落盘由后端负责（它才知道最终顺序）；这里只负责事后重新拉一次，
    // 免得前端的乐观更新和后端算出来的顺序不一致 —— 排序规则在后端。
    return run(set, get, false, async () => {
      await backend.moveApp(id, parentId, beforeId)
      const fresh = await backend.getAllData()
      set({ data: applyCollapseOverrides(fresh) })
      return true
    })
  },

  async renameApp(id, name) {
    return run(set, get, false, async () => {
      const updated = await backend.renameApp(id, name)
      const rename = (app: AppEXE): AppEXE =>
        app.id === id ? { ...app, name: updated.name } : app
      set((state) => ({
        data: {
          ...state.data,
          items: state.data.items.map((item) => {
            if (isAppsRoot(item)) {
              return { Apps: item.Apps.map(rename) }
            }
            return {
              Category: { ...item.Category, groups: renameInItems(item.Category.groups, id, updated.name) },
            }
          }),
        },
      }))
      return true
    })
  },

  async deleteApp(id) {
    return run(set, get, false, async () => {
      await backend.deleteApp(id)
      set((state) => ({
        data: { ...state.data, items: removeRootApp(state.data.items, id) },
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
      let changed = false
      const items = state.data.items.map((item) => {
        if (isAppsRoot(item)) {
          return {
            Apps: item.Apps.map((app) =>
              app.id === appId ? { ...app, iconUrl } : app,
            ),
          }
        }
        const category = item.Category
        return {
          Category: {
            ...category,
            groups: patchIconUrl(category.groups, appId, iconUrl, () => {
              changed = true
            }),
          },
        }
      })
      return changed ? { data: { ...state.data, items } } : {}
    })
  },

  toggleCollapse(id, isCollapsed) {
    writeCollapse(id, isCollapsed)
    set((state) => ({
      data: {
        ...state.data,
        items: state.data.items.map((item) => {
          if (!('Category' in item)) {
            return item
          }
          const category = item.Category
          if (category.id === id) {
            return { Category: { ...category, isCollapsed } }
          }
          return {
            Category: {
              ...category,
              groups: collapseInItems(category.groups, id, isCollapsed),
            },
          }
        }),
      },
    }))
  },
}))

/** 在一层节点里替换某个应用的图标地址；命中时调用 `onHit`。 */
function patchIconUrl(
  items: Item[],
  appId: string,
  iconUrl: string,
  onHit: () => void,
): Item[] {
  return items.map((entry) => {
    if ('App' in entry) {
      if (entry.App.id !== appId) {
        return entry
      }
      onHit()
      return { App: { ...entry.App, iconUrl } }
    }
    return {
      Group: {
        ...entry.Group,
        children: patchIconUrl(entry.Group.children, appId, iconUrl, onHit),
      },
    }
  })
}

/** 只改动根节点上 id 命中的那一个分类。 */
function mapCategories(
  items: RootItem[],
  id: string,
  update: (category: Category) => Category,
): RootItem[] {
  return items.map((item) =>
    'Category' in item && item.Category.id === id
      ? { Category: update(item.Category) }
      : item,
  )
}

/** 对根节点上每一个分类的 groups 做同样的变换。 */
function mapCategoryChildren(
  items: RootItem[],
  update: (category: Category) => Category,
): RootItem[] {
  return items.map((item) => ('Category' in item ? { Category: update(item.Category) } : item))
}


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

/** 把本地保存的折叠偏好叠加到后端返回的数据树上。 */
function applyCollapseOverrides(data: AppData): AppData {
  const overrides = readCollapseMap()
  if (Object.keys(overrides).length === 0) {
    return data
  }

  return {
    ...data,
    items: data.items.map((item) =>
      'Category' in item
        ? {
            Category: {
              ...item.Category,
              isCollapsed: overrides[item.Category.id] ?? item.Category.isCollapsed,
              groups: applyToItems(item.Category.groups, overrides),
            },
          }
        : item,
    ),
  }
}

function applyToItems(items: Item[], overrides: Record<string, boolean>): Item[] {
  return items.map((item) => {
    if ('App' in item) {
      return item
    }

    return {
      Group: {
        ...item.Group,
        isCollapsed: overrides[item.Group.id] ?? item.Group.isCollapsed,
        children: applyToItems(item.Group.children, overrides),
      },
    }
  })
}

/** 把目标节点的展示名称替换为新名称，分组与应用共用。 */
function renameInItems(items: Item[], id: string, name: string): Item[] {
  return items.map((item): Item => {
    if ('App' in item) {
      return item.App.id === id ? { App: { ...item.App, name } } : item
    }

    if (item.Group.id === id) {
      return { Group: { ...item.Group, name } }
    }

    return {
      Group: { ...item.Group, children: renameInItems(item.Group.children, id, name) },
    }
  })
}

/** 只翻转目标节点的折叠标记。 */
function collapseInItems(items: Item[], id: string, isCollapsed: boolean): Item[] {
  return items.map((item): Item => {
    if ('App' in item) {
      return item
    }

    if (item.Group.id === id) {
      return { Group: { ...item.Group, isCollapsed } }
    }

    return {
      Group: {
        ...item.Group,
        children: collapseInItems(item.Group.children, id, isCollapsed),
      },
    }
  })
}

/**
 * 把新节点追加到目标父节点下。父节点既可能是分类本身，也可能是某个分组。
 *
 * `appendChild` 只在 `category.groups` 里递归匹配分组，匹配不到「分类自己」，
 * 所以这里必须先判断分类这一层，再落到分组层。
 *
 * 父节点若处于折叠状态就一并展开，否则新增的条目会被折叠起来，看起来像没加上。
 */
function appendToParent(roots: RootItem[], parentId: string, child: Item): RootItem[] {
  return roots.map((item) => {
    // 顶层应用那一行不能当容器，原样放过。
    if (!('Category' in item)) {
      return item
    }

    const category = item.Category
    return category.id === parentId
      ? {
          Category: { ...category, isCollapsed: false, groups: [...category.groups, child] },
        }
      : { Category: { ...category, groups: appendChild(category.groups, parentId, child) } }
  })
}

/** 把新节点追加到指定分组的子节点下；父分组不存在时保持原样。 */
function appendChild(items: Item[], parentId: string, child: Item): Item[] {  return items.map((item): Item => {
    if ('App' in item) {
      return item
    }

    if (item.Group.id === parentId) {
      return {
        Group: {
          ...item.Group,
          isCollapsed: false,
          children: [...item.Group.children, child],
        },
      }
    }

    return {
      Group: {
        ...item.Group,
        children: appendChild(item.Group.children, parentId, child),
      },
    }
  })
}

/** 从树中删除指定 ID 的节点。 */
function removeNode(items: Item[], id: string): Item[] {
  return items.flatMap((item): Item[] => {
    if ('App' in item) {
      return item.App.id === id ? [] : [item]
    }

    if (item.Group.id === id) {
      return []
    }

    return [{ Group: { ...item.Group, children: removeNode(item.Group.children, id) } }]
  })
}
