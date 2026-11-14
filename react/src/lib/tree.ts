// 目录树的只读查询：与后端 services/tree.rs 的查询语义一致。
// 结构变更一律走后端命令，前端不复制树的写操作。
import {
  isAppItem,
  isAppsRoot,
  isCategoryRoot,
  isGroupItem,
  type AppData,
  type AppEXE,
  type Category,
  type Group,
  type Item,
  type RootItem,
} from '../types/catalog'

export type ContainerNode = Category | Group

/** 分组最大嵌套层数，与后端 services/catalog.rs::MAX_GROUP_DEPTH 一致。 */
export const MAX_GROUP_DEPTH = 4

export function nodeList(node: ContainerNode): Item[] {
  return 'groups' in node ? node.groups : node.children
}

export function countAppsIn(list: Item[]): number {
  let n = 0
  for (const item of list) {
    if (isAppItem(item)) n += 1
    else n += countAppsIn(item.Group.children)
  }
  return n
}

export function countAll(data: AppData): { apps: number; cats: number } {
  let apps = 0
  let cats = 0
  for (const root of data.items) {
    if (isAppsRoot(root)) apps += root.Apps.length
    else {
      cats += 1
      apps += countAppsIn(root.Category.groups)
    }
  }
  return { apps, cats }
}

/** 应用定位结果：app 始终是裸 AppEXE，list 是它所在的数组。 */
export interface AppLocation {
  app: AppEXE
  /** null 表示顶层应用行。 */
  parentId: string | null
  /** 仅顶层时有值：所在根节点下标。 */
  rootIndex?: number
  /** 在 list 中的下标（顶层是 AppEXE[]，容器内是 Item[]）。 */
  index: number
  list: AppEXE[] | Item[]
}

export function locateApp(data: AppData, id: string): AppLocation | null {
  for (let ri = 0; ri < data.items.length; ri++) {
    const root = data.items[ri]
    if (isAppsRoot(root)) {
      const index = root.Apps.findIndex((app) => app.id === id)
      if (index >= 0) {
        return { app: root.Apps[index], parentId: null, rootIndex: ri, index, list: root.Apps }
      }
    } else {
      const found = locateIn(root.Category.groups, id, root.Category.id)
      if (found) return found
    }
  }
  return null
}

function locateIn(list: Item[], id: string, parentId: string): AppLocation | null {
  for (let i = 0; i < list.length; i++) {
    const item = list[i]
    if (isAppItem(item)) {
      if (item.App.id === id) return { app: item.App, parentId, index: i, list }
    } else {
      const found = locateIn(item.Group.children, id, item.Group.id)
      if (found) return found
    }
  }
  return null
}

export interface NodeLocation {
  node: ContainerNode
  kind: 'Category' | 'Group'
  list: RootItem[] | Item[]
  index: number
  parentId: string | null
  depth: number
}

export function locateNode(data: AppData, id: string): NodeLocation | null {
  for (let i = 0; i < data.items.length; i++) {
    const root = data.items[i]
    if (!isCategoryRoot(root)) continue
    if (root.Category.id === id) {
      return { node: root.Category, kind: 'Category', list: data.items, index: i, parentId: null, depth: 0 }
    }
    const found = locateNodeIn(root.Category.groups, id, root.Category.id, 1)
    if (found) return found
  }
  return null
}

function locateNodeIn(list: Item[], id: string, parentId: string, depth: number): NodeLocation | null {
  for (let i = 0; i < list.length; i++) {
    const item = list[i]
    if (!isGroupItem(item)) continue
    if (item.Group.id === id) {
      return { node: item.Group, kind: 'Group', list, index: i, parentId, depth }
    }
    const found = locateNodeIn(item.Group.children, id, item.Group.id, depth + 1)
    if (found) return found
  }
  return null
}

export interface ContainerOption {
  id: string
  label: string
  depth: number
}

/** 「移动到…」与添加对话框的容器列表：顶层 + 所有分类/分组（带面包屑）。 */
export function flattenContainers(data: AppData): ContainerOption[] {
  const out: ContainerOption[] = [{ id: '', label: '顶层（与应用行交错）', depth: 0 }]
  const walk = (list: Item[], depth: number, prefix: string) => {
    for (const item of list) {
      if (!isGroupItem(item)) continue
      out.push({ id: item.Group.id, label: prefix + item.Group.name, depth })
      walk(item.Group.children, depth + 1, `${prefix + item.Group.name} › `)
    }
  }
  for (const root of data.items) {
    if (!isCategoryRoot(root)) continue
    out.push({ id: root.Category.id, label: root.Category.name, depth: 0 })
    walk(root.Category.groups, 1, `${root.Category.name} › `)
  }
  return out
}

export interface AppWithPath {
  app: AppEXE
  path: string[]
}

export function allApps(data: AppData): AppWithPath[] {
  const out: AppWithPath[] = []
  const walk = (list: Item[], path: string[]) => {
    for (const item of list) {
      if (isAppItem(item)) out.push({ app: item.App, path })
      else walk(item.Group.children, [...path, item.Group.name])
    }
  }
  for (const root of data.items) {
    if (isAppsRoot(root)) {
      for (const app of root.Apps) out.push({ app, path: [] })
    } else {
      walk(root.Category.groups, [root.Category.name])
    }
  }
  return out
}

export function nodeDepth(data: AppData, id: string): number {
  return locateNode(data, id)?.depth ?? 0
}

/** 同一数组里参照应用之后的下一个应用 ID（跳过分组），没有则 null。 */
export function nextAppId(list: AppEXE[] | Item[], index: number): string | null {
  for (let i = index + 1; i < list.length; i++) {
    const entry = list[i]
    if ('id' in entry) return entry.id
    if (isAppItem(entry)) return entry.App.id
  }
  return null
}

/** 全部展开/折叠：返回需要变更的节点 ID 与目标状态。 */
export function collectCollapseTargets(data: AppData, isCollapsed: boolean): string[] {
  const ids: string[] = []
  const walk = (list: Item[]) => {
    for (const item of list) {
      if (!isGroupItem(item)) continue
      if (item.Group.isCollapsed !== isCollapsed) ids.push(item.Group.id)
      walk(item.Group.children)
    }
  }
  for (const root of data.items) {
    if (!isCategoryRoot(root)) continue
    if (root.Category.isCollapsed !== isCollapsed) ids.push(root.Category.id)
    walk(root.Category.groups)
  }
  return ids
}
