import {
  asApp,
  asGroup,
  type AppData,
  type AppEXE,
  type Category,
  type Group,
  type IconType,
  type Item,
} from '../types/catalog'

/**
 * 顶层节点的约定 ID。
 *
 * 顶层没有对应节点（它不是分类也不是分组），但对话框、右键菜单和拖放落点
 * 都需要一个能标识它的值。用一个不可能与后端 UUID 重复的前缀命名，并在
 * 发给后端之前翻译成 `null`（后端用 `parentId: null` 表示顶层）。
 */
export const ROOT_ID = '__root__'

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
    data.apps.length +
    data.categories.reduce((sum, category) => sum + countApps(category.groups), 0)
  )
}

/** 在整棵树中按 ID 查找应用节点。顶层应用优先，它们在分类之前。 */
export function findApp(data: AppData, id: string): AppEXE | null {
  const root = data.apps.find((app) => app.id === id)
  if (root) {
    return root
  }

  for (const category of data.categories) {
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

  const categories = data.categories.flatMap((category): Category[] => {
    const groups = filterItems(category.groups)
    if (groups.length > 0) {
      return [{ ...category, isCollapsed: false, groups }]
    }

    if (matches(category.name)) {
      return [{ ...category, isCollapsed: false }]
    }

    return []
  })

  // 顶层应用按名称/来源过滤，命中规则与分类树里的应用保持一致。
  const apps = data.apps.filter(
    (app) => matches(app.name) || matches(app.sourcePath),
  )

  return { ...data, apps, categories }
}

/**
 * 按当前折叠状态展开出「屏幕上真正可见」的应用节点。
 *
 * 键盘导航只需要在可见项之间移动，所以折叠的分支必须被跳过。
 */
export function collectVisibleApps(data: AppData): AppEXE[] {
  // 顶层应用永远可见，且排在分类之前，与屏幕上的顺序一致。
  const apps: AppEXE[] = [...data.apps]

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

  for (const category of data.categories) {
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
  const rootApp = data.apps.find((app) => app.id === id)
  if (rootApp) {
    return {
      id,
      kind: 'app',
      name: rootApp.name,
      parentId: null,
      parentKind: null,
    }
  }

  for (const category of data.categories) {
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
  for (const category of data.categories) {
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
  for (const category of data.categories) {
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
