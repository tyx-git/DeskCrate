import { insertRootApp, removeRootApp } from './tree'
import type { AppData, AppEXE, Category, Group, Item, RootItem } from '../types/catalog'

/**
 * 目录树的纯不可变变换。
 *
 * 这里不访问 Zustand、IPC 或 localStorage；每个函数只接收数据并返回新数据，
 * 便于独立测试，也让 catalogStore 只负责请求生命周期和状态提交。
 */

export function appendCategory(data: AppData, category: Category): AppData {
  return { ...data, items: [...data.items, { Category: category }] }
}

export function renameCategory(data: AppData, id: string, name: string): AppData {
  return {
    ...data,
    items: data.items.map((item) =>
      'Category' in item && item.Category.id === id
        ? { Category: { ...item.Category, name } }
        : item,
    ),
  }
}

export function removeCategory(data: AppData, id: string): AppData {
  return {
    ...data,
    items: data.items.filter((item) => !('Category' in item && item.Category.id === id)),
  }
}

export function appendChild(data: AppData, parentId: string, child: Item): AppData {
  let changed = false
  const items = data.items.map((item) => {
    if (changed || !('Category' in item)) {
      return item
    }

    if (item.Category.id === parentId) {
      changed = true
      return {
        Category: {
          ...item.Category,
          isCollapsed: false,
          groups: [...item.Category.groups, child],
        },
      }
    }

    const result = appendToGroupItems(item.Category.groups, parentId, child)
    if (!result.changed) {
      return item
    }

    changed = true
    return { Category: { ...item.Category, groups: result.items } }
  })

  return changed ? { ...data, items } : data
}

export function insertApp(
  data: AppData,
  app: AppEXE,
  parentId: string | null,
  rootIndex?: number,
): AppData {
  if (parentId === null) {
    return {
      ...data,
      items: insertRootApp(data.items, app, rootIndex ?? Number.MAX_SAFE_INTEGER),
    }
  }

  return appendChild(data, parentId, { App: app })
}

export function renameGroup(data: AppData, id: string, name: string): AppData {
  return mapCategoryGroups(data, (items) =>
    mapNestedItems(items, (item) => {
      if ('Group' in item && item.Group.id === id) {
        return { Group: { ...item.Group, name } }
      }
      return item
    }),
  )
}

export function removeGroup(data: AppData, id: string): AppData {
  return mapCategoryGroups(data, (items) =>
    mapNestedItems(items, (item) => {
      if ('Group' in item && item.Group.id === id) {
        return null
      }
      return item
    }),
  )
}

export function renameApp(data: AppData, id: string, name: string): AppData {
  let changed = false
  const items = data.items.map((item): RootItem => {
    if ('Apps' in item) {
      const apps = item.Apps.map((app) => {
        if (app.id !== id) {
          return app
        }
        changed = true
        return { ...app, name }
      })
      return { Apps: apps }
    }

    const groups = mapNestedItems(item.Category.groups, (entry) => {
      if ('App' in entry && entry.App.id === id) {
        changed = true
        return { App: { ...entry.App, name } }
      }
      return entry
    })
    return { Category: { ...item.Category, groups } }
  })

  return changed ? { ...data, items } : data
}

export function removeApp(data: AppData, id: string): AppData {
  return { ...data, items: removeRootApp(data.items, id) }
}

export type MoveAppFailure = 'app_not_found' | 'parent_not_found' | 'reference_not_found'

export type MoveAppResult =
  | { ok: true; data: AppData }
  | { ok: false; code: MoveAppFailure }

export type MoveGroupFailure =
  | 'group_not_found'
  | 'parent_not_found'
  | 'reference_not_found'
  | 'invalid_group_parent'
  | 'invalid_group_position'

export type MoveGroupResult =
  | { ok: true; data: AppData }
  | { ok: false; code: MoveGroupFailure }

/** 纯函数实现同级分组移动；失败时不会返回被摘除一半的数据。 */
export function moveGroup(
  data: AppData,
  id: string,
  parentId: string,
  beforeId: string | null,
): MoveGroupResult {
  const source = findGroupParent(data.items, id)
  if (!source) return { ok: false, code: 'group_not_found' }
  if (source.parentId !== parentId) return { ok: false, code: 'invalid_group_parent' }

  const detached = detachGroup(data, id)
  if (!detached.group) return { ok: false, code: 'group_not_found' }
  if (beforeId === id || (beforeId && groupContainsId(detached.group, beforeId))) {
    return { ok: false, code: 'invalid_group_position' }
  }

  const targetNodeParent = beforeId ? findItemParent(detached.data.items, beforeId) : parentId
  if (beforeId && targetNodeParent !== parentId) {
    return { ok: false, code: 'invalid_group_position' }
  }

  if (beforeId === null) {
    const next = appendChild(detached.data, parentId, { Group: detached.group })
    return next === detached.data
      ? { ok: false, code: 'parent_not_found' }
      : { ok: true, data: next }
  }

  const items = insertBeforeNodeInContainer(
    detached.data.items,
    parentId,
    beforeId,
    { Group: detached.group },
  )
  return items
    ? { ok: true, data: { ...detached.data, items } }
    : { ok: false, code: 'reference_not_found' }
}

/** 纯函数实现应用移动；失败时不会返回被摘除一半的数据。 */
export function moveApp(
  data: AppData,
  id: string,
  parentId: string | null,
  beforeId: string | null,
): MoveAppResult {
  const detached = detachApp(data, id)
  if (!detached.app) {
    return { ok: false, code: 'app_not_found' }
  }

  const app = detached.app
  if (parentId === null) {
    if (beforeId === null) {
      return {
        ok: true,
        data: { ...detached.data, items: insertRootApp(detached.data.items, app, Number.MAX_SAFE_INTEGER) },
      }
    }

    const items = insertBeforeRootApp(detached.data.items, beforeId, app)
    return items ? { ok: true, data: { ...detached.data, items } } : { ok: false, code: 'reference_not_found' }
  }

  if (beforeId === null) {
    const next = appendChild(detached.data, parentId, { App: app })
    return next === detached.data
      ? { ok: false, code: 'parent_not_found' }
      : { ok: true, data: next }
  }

  const targetParent = findItemParent(detached.data.items, beforeId)
  if (targetParent !== parentId) {
    return { ok: false, code: 'reference_not_found' }
  }
  const items = insertBeforeNestedNode(
    detached.data.items,
    beforeId,
    { App: app },
  )
  return items ? { ok: true, data: { ...detached.data, items } } : { ok: false, code: 'reference_not_found' }
}

export function patchIcon(data: AppData, appId: string, iconUrl: string): AppData {
  let changed = false
  const items = data.items.map((item): RootItem => {
    if ('Apps' in item) {
      const apps = item.Apps.map((app) => {
        if (app.id !== appId) {
          return app
        }
        changed = true
        return { ...app, iconUrl }
      })
      return { Apps: apps }
    }

    const groups = mapNestedItems(item.Category.groups, (entry) => {
      if ('App' in entry && entry.App.id === appId) {
        changed = true
        return { App: { ...entry.App, iconUrl } }
      }
      return entry
    })
    return { Category: { ...item.Category, groups } }
  })

  return changed ? { ...data, items } : data
}

export function toggleCollapse(data: AppData, id: string, isCollapsed: boolean): AppData {
  let changed = false
  const items = data.items.map((item): RootItem => {
    if (!('Category' in item)) {
      return item
    }

    if (item.Category.id === id) {
      changed = true
      return { Category: { ...item.Category, isCollapsed } }
    }

    const groups = mapNestedItems(item.Category.groups, (entry) => {
      if ('Group' in entry && entry.Group.id === id) {
        changed = true
        return { Group: { ...entry.Group, isCollapsed } }
      }
      return entry
    })
    return { Category: { ...item.Category, groups } }
  })

  return changed ? { ...data, items } : data
}

export function applyCollapseOverrides(
  data: AppData,
  overrides: Record<string, boolean>,
): AppData {
  if (Object.keys(overrides).length === 0) {
    return data
  }

  return {
    ...data,
    items: data.items.map((item) => {
      if (!('Category' in item)) {
        return item
      }

      return {
        Category: {
          ...item.Category,
          isCollapsed: overrides[item.Category.id] ?? item.Category.isCollapsed,
          groups: applyOverridesToItems(item.Category.groups, overrides),
        },
      }
    }),
  }
}

function mapCategoryGroups(data: AppData, update: (items: Item[]) => Item[]): AppData {
  return {
    ...data,
    items: data.items.map((item) =>
      'Category' in item
        ? { Category: { ...item.Category, groups: update(item.Category.groups) } }
        : item,
    ),
  }
}

function appendToGroupItems(
  items: Item[],
  parentId: string,
  child: Item,
): { items: Item[]; changed: boolean } {
  for (let index = 0; index < items.length; index += 1) {
    const item = items[index]
    if (!('Group' in item)) {
      continue
    }

    if (item.Group.id === parentId) {
      const next = [...items]
      next[index] = {
        Group: {
          ...item.Group,
          isCollapsed: false,
          children: [...item.Group.children, child],
        },
      }
      return { items: next, changed: true }
    }

    const result = appendToGroupItems(item.Group.children, parentId, child)
    if (result.changed) {
      const next = [...items]
      next[index] = { Group: { ...item.Group, children: result.items } }
      return { items: next, changed: true }
    }
  }

  return { items, changed: false }
}

function mapNestedItems(
  items: Item[],
  transform: (item: Item) => Item | null,
): Item[] {
  return items.flatMap((item): Item[] => {
    const next = transform(item)
    if (next === null) {
      return []
    }

    if ('Group' in next) {
      return [{ Group: { ...next.Group, children: mapNestedItems(next.Group.children, transform) } }]
    }

    return [next]
  })
}

function applyOverridesToItems(
  items: Item[],
  overrides: Record<string, boolean>,
): Item[] {
  return items.map((item) => {
    if ('App' in item) {
      return item
    }

    return {
      Group: {
        ...item.Group,
        isCollapsed: overrides[item.Group.id] ?? item.Group.isCollapsed,
        children: applyOverridesToItems(item.Group.children, overrides),
      },
    }
  })
}

function detachApp(data: AppData, id: string): { data: AppData; app: AppEXE | null } {
  let app: AppEXE | null = null
  const items = data.items.flatMap((item): RootItem[] => {
    if (app) {
      return [item]
    }

    if ('Apps' in item) {
      const kept = item.Apps.filter((candidate) => {
        if (candidate.id !== id) {
          return true
        }
        app = candidate
        return false
      })
      return kept.length > 0 ? [{ Apps: kept }] : []
    }

    const result = detachFromItems(item.Category.groups, id)
    if (!result.app) {
      return [item]
    }

    app = result.app
    return [{ Category: { ...item.Category, groups: result.items } }]
  })

  return { data: app ? { ...data, items } : data, app }
}

function detachFromItems(items: Item[], id: string): { items: Item[]; app: AppEXE | null } {
  let app: AppEXE | null = null
  const next: Item[] = []

  for (const item of items) {
    if (app) {
      next.push(item)
      continue
    }

    if ('App' in item) {
      if (item.App.id === id) {
        app = item.App
      } else {
        next.push(item)
      }
      continue
    }

    const result = detachFromItems(item.Group.children, id)
    if (result.app) {
      app = result.app
      next.push({ Group: { ...item.Group, children: result.items } })
    } else {
      next.push(item)
    }
  }

  return { items: next, app }
}

function groupContainsId(group: Group, id: string): boolean {
  return group.children.some(
    (item) =>
      ('App' in item ? item.App.id : item.Group.id) === id ||
      ('Group' in item && groupContainsId(item.Group, id)),
  )
}

function findGroupParent(items: RootItem[], id: string): { parentId: string; group: Group } | null {
  const walk = (entries: Item[], parentId: string): { parentId: string; group: Group } | null => {
    for (const item of entries) {
      if ('Group' in item) {
        if (item.Group.id === id) return { parentId, group: item.Group }
        const nested = walk(item.Group.children, item.Group.id)
        if (nested) return nested
      }
    }
    return null
  }
  for (const root of items) {
    if ('Category' in root) {
      const result = walk(root.Category.groups, root.Category.id)
      if (result) return result
    }
  }
  return null
}

function detachGroup(data: AppData, id: string): { data: AppData; group: Group | null } {
  let group: Group | null = null
  const detach = (items: Item[]): Item[] => {
    const result: Item[] = []
    for (const item of items) {
      if ('Group' in item && item.Group.id === id) {
        group = item.Group
      } else if ('Group' in item) {
        result.push({ Group: { ...item.Group, children: detach(item.Group.children) } })
      } else {
        result.push(item)
      }
    }
    return result
  }
  const items = data.items.map((root): RootItem =>
    'Category' in root ? { Category: { ...root.Category, groups: detach(root.Category.groups) } } : root,
  )
  return { data: group ? { ...data, items } : data, group }
}

function insertBeforeNestedNode(items: RootItem[], beforeId: string, node: Item): RootItem[] | null {
  for (let index = 0; index < items.length; index += 1) {
    const root = items[index]
    if (!('Category' in root)) continue
    const groups = insertBeforeNodeInItems(root.Category.groups, beforeId, node)
    if (groups) {
      const next = [...items]
      next[index] = { Category: { ...root.Category, groups } }
      return next
    }
  }
  return null
}

function insertBeforeNodeInContainer(
  items: RootItem[],
  parentId: string,
  beforeId: string,
  node: Item,
): RootItem[] | null {
  for (let index = 0; index < items.length; index += 1) {
    const root = items[index]
    if (!('Category' in root)) continue
    if (root.Category.id === parentId) {
      const groups = insertBeforeNodeInItems(root.Category.groups, beforeId, node)
      if (!groups) return null
      const next = [...items]
      next[index] = { Category: { ...root.Category, groups } }
      return next
    }
    const groups = insertBeforeNodeInGroup(root.Category.groups, parentId, beforeId, node)
    if (groups) {
      const next = [...items]
      next[index] = { Category: { ...root.Category, groups } }
      return next
    }
  }
  return null
}

function insertBeforeNodeInGroup(
  items: Item[],
  parentId: string,
  beforeId: string,
  node: Item,
): Item[] | null {
  for (let index = 0; index < items.length; index += 1) {
    const item = items[index]
    if (!('Group' in item)) continue
    if (item.Group.id === parentId) {
      const children = insertBeforeNodeInItems(item.Group.children, beforeId, node)
      if (!children) return null
      const next = [...items]
      next[index] = { Group: { ...item.Group, children } }
      return next
    }
    const children = insertBeforeNodeInGroup(item.Group.children, parentId, beforeId, node)
    if (children) {
      const next = [...items]
      next[index] = { Group: { ...item.Group, children } }
      return next
    }
  }
  return null
}

function insertBeforeNodeInItems(items: Item[], beforeId: string, node: Item): Item[] | null {
  for (let index = 0; index < items.length; index += 1) {
    const item = items[index]
    if (('App' in item ? item.App.id : item.Group.id) === beforeId) {
      return [...items.slice(0, index), node, ...items.slice(index)]
    }
    if ('Group' in item) {
      const nested = insertBeforeNodeInItems(item.Group.children, beforeId, node)
      if (nested) {
        const next = [...items]
        next[index] = { Group: { ...item.Group, children: nested } }
        return next
      }
    }
  }
  return null
}

function findItemParent(items: RootItem[], id: string): string | null {
  const walk = (entries: Item[], parentId: string): string | null => {
    for (const item of entries) {
      const itemId = 'App' in item ? item.App.id : item.Group.id
      if (itemId === id) return parentId
      if ('Group' in item) {
        const nested = walk(item.Group.children, item.Group.id)
        if (nested !== null) return nested
      }
    }
    return null
  }
  for (const root of items) {
    if ('Category' in root) {
      const parent = walk(root.Category.groups, root.Category.id)
      if (parent !== null) return parent
    }
  }
  return null
}

function insertBeforeRootApp(items: RootItem[], beforeId: string, app: AppEXE): RootItem[] | null {
  const next = items.map((item) => {
    if (!('Apps' in item)) {
      return item
    }

    const index = item.Apps.findIndex((candidate) => candidate.id === beforeId)
    if (index < 0) {
      return item
    }

    return {
      Apps: [...item.Apps.slice(0, index), app, ...item.Apps.slice(index)],
    }
  })

  return next.some((item, index) => item !== items[index]) ? next : null
}
