import {
  asApp,
  asGroup,
  isAppsRoot,
  type AppData,
  type AppEXE,
  type Category,
  type Group,
  type IconType,
  type Item,
  type RootItem,
} from '../types/catalog'

/**
 * 顶层节点的约定 ID。
 *
 * 顶层没有对应节点（它不是分类也不是分组），但对话框、右键菜单和拖放落点
 * 都需要一个能标识它的值。用一个不可能与后端 UUID 重复的前缀命名，并在
 * 发给后端之前翻译成 `null`（后端用 `parentId: null` 表示顶层）。
 */
export const ROOT_ID = '__root__'

/** 按显示顺序取出根节点上的所有顶层应用。 */
export function rootApps(data: AppData): AppEXE[] {
  return data.items.flatMap((item) => (isAppsRoot(item) ? item.Apps : []))
}

/** 按显示顺序取出根节点上的所有分类。 */
export function rootCategories(data: AppData): Category[] {
  return data.items.flatMap((item) => ('Category' in item ? [item.Category] : []))
}

/** 根节点上分类的个数（顶层应用行不算）。 */
export function countRootCategories(data: AppData): number {
  return data.items.filter((item) => 'Category' in item).length
}

/** 判断某个根节点是不是分类。 */
export function asCategoryRoot(item: RootItem): Category | null {
  return 'Category' in item ? item.Category : null
}

/** 判断某个根节点是不是一行顶层应用。 */
export function asAppsRoot(item: RootItem): AppEXE[] | null {
  return isAppsRoot(item) ? item.Apps : null
}

/**
 * 把一个顶层应用插到第 `index` 个根节点之前。
 *
 * 合并规则与后端 `tree::insert_root_app` 完全一致：优先并入落点处那一行
 * （`items[index]`），其次并入紧邻上方那一行（`items[index - 1]`），都没有才
 * 新起一行。否则前端看到的顺序会和后端写进去的不一致，刷新一次就「跳位」。
 *
 * 之所以先并入而不是每次新起一行：拖到某一行的下边缘，直觉上就是加进这一行，
 * 而不是在它下面再造一条新的分界。
 */
export function insertRootApp(items: RootItem[], app: AppEXE, index: number): RootItem[] {
  const at = Math.min(Math.max(index, 0), items.length)
  const next = [...items]

  const current = next[at]
  if (current && isAppsRoot(current)) {
    next[at] = { Apps: [...current.Apps, app] }
    return next
  }

  if (at > 0) {
    const previous = next[at - 1]
    if (previous && isAppsRoot(previous)) {
      next[at - 1] = { Apps: [...previous.Apps, app] }
      return next
    }
  }

  next.splice(at, 0, { Apps: [app] })
  return next
}

/** 删除一个应用（含分类内部的）；删空的那一行会从根节点上摘掉。 */
export function removeRootApp(items: RootItem[], id: string): RootItem[] {
  return items
    .map((item) => {
      if (isAppsRoot(item)) {
        const kept = item.Apps.filter((app) => app.id !== id)
        return kept.length === item.Apps.length ? item : { Apps: kept }
      }
      return {
        Category: {
          ...item.Category,
          groups: removeNodeInItems(item.Category.groups, id),
        },
      }
    })
    .filter((item) => !isAppsRoot(item) || item.Apps.length > 0)
}

/** 从一层节点里删掉指定 ID 的应用或分组。 */
function removeNodeInItems(items: Item[], id: string): Item[] {
  return items.flatMap((item): Item[] => {
    if ('App' in item) {
      return item.App.id === id ? [] : [item]
    }
    if (item.Group.id === id) {
      return []
    }
    return [{ Group: { ...item.Group, children: removeNodeInItems(item.Group.children, id) } }]
  })
}

/** 按显示顺序展开出所有应用节点。 */
export function collectApps(items: readonly Item[]): AppEXE[] {
  const apps: AppEXE[] = []

  for (const item of items) {
    const app = asApp(item)
    if (app) {
      apps.push(app)
      continue
    }

    const group = asGroup(item)
    if (group) {
      apps.push(...collectApps(group.children))
    }
  }

  return apps
}

/** 统计节点集合（含任意层级后代）中的应用数量，用于分组右侧的计数徽标。 */
export function countApps(items: readonly Item[]): number {
  let total = 0

  for (const item of items) {
    const app = asApp(item)
    if (app) {
      total += 1
      continue
    }

    const group = asGroup(item)
    if (group) {
      total += countApps(group.children)
    }
  }

  return total
}

/** 统计整份数据里的应用总数，含顶层应用。 */
export function countAllApps(data: AppData): number {
  return (
    rootApps(data).length +
    rootCategories(data).reduce((sum, category) => sum + countApps(category.groups), 0)
  )
}

/** 在整棵树中按 ID 查找应用节点。 */
export function findApp(data: AppData, id: string): AppEXE | null {
  const root = rootApps(data).find((app) => app.id === id)
  if (root) {
    return root
  }

  for (const category of rootCategories(data)) {
    const found = collectApps(category.groups).find((app) => app.id === id)
    if (found) {
      return found
    }
  }

  return null
}

/** 按名称/来源路径过滤整棵树，并强制展开命中路径上的所有节点。 */
export function filterTree(data: AppData, rawQuery: string): AppData {
  const query = rawQuery.trim().toLowerCase()
  if (query.length === 0) {
    return data
  }

  const matches = (text: string) => text.toLowerCase().includes(query)

  const filterItems = (items: readonly Item[]): Item[] =>
    items.flatMap((item): Item[] => {
      const group = asGroup(item)
      if (group) {
        const children = filterItems(group.children)
        if (children.length > 0) {
          return [
            { Group: { ...group, isCollapsed: false, children } } satisfies Item,
          ]
        }

        // 分组自身命中时保留它的全部子节点，让用户直接看到分组内容。
        if (matches(group.name)) {
          return [{ Group: { ...group, isCollapsed: false } } satisfies Item]
        }

        return []
      }

      const app = asApp(item)
      if (app && (matches(app.name) || matches(app.sourcePath))) {
        return [{ App: app } satisfies Item]
      }

      return []
    })

  // 按根节点的顺序逐个过滤：顶层应用按名称/来源过滤（一行里剩下的合并成
  // 一行），分类按原有规则过滤。顺序保持不变，空的一行会被丢掉。
  const items = data.items.flatMap((item): RootItem[] => {
    const apps = asAppsRoot(item)
    if (apps) {
      const kept = apps.filter((app) => matches(app.name) || matches(app.sourcePath))
      return kept.length > 0 ? [{ Apps: kept }] : []
    }

    const category = asCategoryRoot(item)
    if (!category) {
      return []
    }

    const groups = filterItems(category.groups)
    if (groups.length > 0) {
      return [{ Category: { ...category, isCollapsed: false, groups } }]
    }

    // 分类自身命中时保留它的全部子节点，让用户直接看到分类内容。
    if (matches(category.name)) {
      return [{ Category: { ...category, isCollapsed: false } }]
    }

    return []
  })

  return { ...data, items }
}

/**
 * 按当前折叠状态展开出「屏幕上真正可见」的应用节点。
 *
 * 键盘导航只需要在可见项之间移动，所以折叠的分支必须被跳过。
 */
export function collectVisibleApps(data: AppData): AppEXE[] {
  // 顶层应用永远可见，顺序与屏幕上的顺序一致。
  const apps: AppEXE[] = rootApps(data)

  const walk = (items: readonly Item[]) => {
    for (const item of items) {
      const app = asApp(item)
      if (app) {
        apps.push(app)
        continue
      }

      const group = asGroup(item)
      if (group && !group.isCollapsed) {
        walk(group.children)
      }
    }
  }

  for (const category of rootCategories(data)) {
    if (!category.isCollapsed) {
      walk(category.groups)
    }
  }

  return apps
}

/** 定位一个节点所在的父容器 ID 与容器类型，供右键菜单决定可用的操作。 */
export interface NodeLocation {
  /** 节点 ID；顶层节点用 `ROOT_ID`。 */
  id: string
  /** `root` 不是数据树里的节点，只是「顶层」这个落点的说法。 */
  kind: 'root' | 'category' | 'group' | 'app'
  name: string
  /** 父节点 ID；顶层分类和顶层应用都没有父节点。 */
  parentId: string | null
  /** 父节点是分类还是分组。 */
  parentKind: 'category' | 'group' | null
}

/** 顶层这个落点的固定描述，右键菜单与「添加应用」对话框都用它。 */
export const ROOT_LOCATION: NodeLocation = {
  id: ROOT_ID,
  kind: 'root',
  name: '顶层',
  parentId: null,
  parentKind: null,
}

export function locateNode(data: AppData, id: string): NodeLocation | null {
  // 顶层应用没有父节点，右键菜单据此判断它不能被「移到别处」。
  const rootApp = rootApps(data).find((app) => app.id === id)
  if (rootApp) {
    return {
      id,
      kind: 'app',
      name: rootApp.name,
      parentId: null,
      parentKind: null,
    }
  }

  for (const category of rootCategories(data)) {
    if (category.id === id) {
      return {
        id,
        kind: 'category',
        name: category.name,
        parentId: null,
        parentKind: null,
      }
    }

    const found = locateInItems(category.groups, id, category.id, 'category')
    if (found) {
      return found
    }
  }

  return null
}

function locateInItems(
  items: readonly Item[],
  id: string,
  parentId: string,
  parentKind: 'category' | 'group',
): NodeLocation | null {
  for (const item of items) {
    const app = asApp(item)
    if (app) {
      if (app.id === id) {
        return {
          id,
          kind: 'app',
          name: app.name,
          parentId,
          parentKind,
        }
      }
      continue
    }

    const group = asGroup(item)
    if (!group) {
      continue
    }

    if (group.id === id) {
      return { id, kind: 'group', name: group.name, parentId, parentKind }
    }

    const found = locateInItems(group.children, id, group.id, 'group')
    if (found) {
      return found
    }
  }

  return null
}

/**
 * 判断某个父节点还能不能继续往下加分组。
 *
 * 与后端 `MAX_GROUP_DEPTH = 4` 保持一致：顶层分类下的直接分组算第 1 层。
 * 前端提前拦截可以少一次无效的 IPC 往返。
 */
export const MAX_GROUP_DEPTH = 4

/** 返回节点在分组树中的深度：顶层分类为 0，分类下的直接分组为 1，以此类推。 */
export function depthOf(data: AppData, id: string): number | null {
  for (const category of rootCategories(data)) {
    if (category.id === id) {
      return 0
    }

    const depth = depthInItems(category.groups, id, 1)
    if (depth !== null) {
      return depth
    }
  }

  return null
}

function depthInItems(
  items: readonly Item[],
  id: string,
  depth: number,
): number | null {
  for (const item of items) {
    const group = asGroup(item)
    if (!group) {
      continue
    }

    if (group.id === id) {
      return depth
    }

    const found = depthInItems(group.children, id, depth + 1)
    if (found !== null) {
      return found
    }
  }

  return null
}

/** 图标加载失败时的兜底字形，沿用设计稿的表情符号风格。 */
export const ICON_FALLBACK_GLYPH: Record<IconType, string> = {
  Shortcut: '🔗',
  Executable: '⚙️',
  Folder: '📁',
  File: '📄',
  Url: '🌐',
}

/** 来源类型的中文说明，用于右键菜单和提示。 */
export const ICON_TYPE_LABEL: Record<IconType, string> = {
  Shortcut: '快捷方式',
  Executable: '可执行程序',
  Folder: '文件夹',
  File: '文件',
  Url: '网址',
}

/** 递归查找分组节点。 */
export function findGroup(data: AppData, id: string): Group | null {
  for (const category of rootCategories(data)) {
    const found = findGroupIn(category.groups, id)
    if (found) {
      return found
    }
  }

  return null
}

function findGroupIn(items: readonly Item[], id: string): Group | null {
  for (const item of items) {
    const group = asGroup(item)
    if (!group) {
      continue
    }

    if (group.id === id) {
      return group
    }

    const found = findGroupIn(group.children, id)
    if (found) {
      return found
    }
  }

  return null
}
