import { create } from 'zustand'

import { backend, describeError, toAppError } from '../api'
import { readCollapseMap, writeCollapse } from '../lib/collapseStorage'
import type { AppData, AppEXE, Category, Group, Item } from '../types/catalog'
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

  /** `parentId` 为 `null` 时直接放到顶层。 */
  addApp: (
    parentId: string | null,
    sourcePath: string,
    name?: string,
  ) => Promise<AppEXE | null>
  renameApp: (id: string, name: string) => Promise<boolean>
  deleteApp: (id: string) => Promise<boolean>

  launchApp: (app: AppEXE) => Promise<void>
  toggleCollapse: (id: string, isCollapsed: boolean) => void
}

const EMPTY: AppData = { version: 1, categories: [], apps: [] }

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
        data: { ...state.data, categories: [...state.data.categories, category] },
      }))
      return category
    })
  },

  async renameCategory(id, name) {
    return run(set, get, false, async () => {
      const updated = await backend.renameCategory(id, name)
      set((state) => ({
        data: {
          ...state.data,
          categories: state.data.categories.map((category) =>
            category.id === id ? { ...category, name: updated.name } : category,
          ),
        },
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
          categories: state.data.categories.filter((category) => category.id !== id),
        },
      }))
      return true
    })
  },

  async addGroup(parentId, name) {
    return run(set, get, null, async () => {
      const group = await backend.addGroup(parentId, name)
      set((state) => ({
        data: {
          ...state.data,
          categories: appendToParent(state.data.categories, parentId, { Group: group }),
        },
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
          categories: state.data.categories.map((category) => ({
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
          categories: state.data.categories.map((category) => ({
            ...category,
            groups: removeNode(category.groups, id),
          })),
        },
      }))
      return true
    })
  },

  async addApp(parentId, sourcePath, name) {
    return run(set, get, null, async () => {
      const app = await backend.addApp(parentId, sourcePath, name)
      set((state) => ({
        data:
          parentId === null
            ? { ...state.data, apps: [...state.data.apps, app] }
            : {
                ...state.data,
                categories: appendToParent(state.data.categories, parentId, {
                  App: app,
                }),
              },
      }))
      return app
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
          apps: state.data.apps.map(rename),
          categories: state.data.categories.map((category) => ({
            ...category,
            groups: renameInItems(category.groups, id, updated.name),
          })),
        },
      }))
      return true
    })
  },

  async deleteApp(id) {
    return run(set, get, false, async () => {
      await backend.deleteApp(id)
      set((state) => ({
        data: {
          ...state.data,
          apps: state.data.apps.filter((app) => app.id !== id),
          categories: state.data.categories.map((category) => ({
            ...category,
            groups: removeNode(category.groups, id),
          })),
        },
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

  toggleCollapse(id, isCollapsed) {
    writeCollapse(id, isCollapsed)
    set((state) => ({
      data: {
        ...state.data,
        categories: state.data.categories.map((category) => {
          if (category.id === id) {
            return { ...category, isCollapsed }
          }
          return {
            ...category,
            groups: collapseInItems(category.groups, id, isCollapsed),
          }
        }),
      },
    }))
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

/** 把本地保存的折叠偏好叠加到后端返回的数据树上。 */
function applyCollapseOverrides(data: AppData): AppData {
  const overrides = readCollapseMap()
  if (Object.keys(overrides).length === 0) {
    return data
  }

  return {
    ...data,
    categories: data.categories.map((category) => ({
      ...category,
      isCollapsed: overrides[category.id] ?? category.isCollapsed,
      groups: applyToItems(category.groups, overrides),
    })),
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
function appendToParent(
  categories: Category[],
  parentId: string,
  child: Item,
): Category[] {
  return categories.map((category) =>
    category.id === parentId
      ? { ...category, isCollapsed: false, groups: [...category.groups, child] }
      : { ...category, groups: appendChild(category.groups, parentId, child) },
  )
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
