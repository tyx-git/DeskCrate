import { useCallback } from 'react'

import { asAppsRoot, asCategoryRoot } from '../lib/tree'
import type { AppData, AppEXE, Item, RootItem } from '../types/catalog'

/**
 * 一个应用在树里的位置：父容器（`null` = 顶层）与同一层的兄弟顺序。
 *
 * 兄弟顺序用来把「插到 X 之后」翻译成「插到 X 的下一个兄弟之前」—— 后端只认
 * 参照节点，不认下标。
 */
interface Placement {
  parentId: string | null
  siblingIds: string[]
}

function findPlacement(items: RootItem[], appId: string): Placement | null {
  for (const root of items) {
    const apps = asAppsRoot(root)
    if (apps?.some((app) => app.id === appId)) {
      return { parentId: null, siblingIds: apps.map((app) => app.id) }
    }
  }

  const walk = (entries: readonly Item[], parentId: string): Placement | null => {
    for (const entry of entries) {
      if ('App' in entry) {
        if (entry.App.id === appId) {
          return { parentId, siblingIds: siblingIdsOf(entries) }
        }
        continue
      }
      const found = walk(entry.Group.children, entry.Group.id)
      if (found) return found
    }
    return null
  }

  for (const root of items) {
    const category = asCategoryRoot(root)
    if (!category) continue
    const found = walk(category.groups, category.id)
    if (found) return found
  }

  return null
}

/** 取一层节点里所有兄弟的 ID（应用与分组混排时按显示顺序）。 */
function siblingIdsOf(entries: readonly Item[]): string[] {
  return entries.map((entry) => ('App' in entry ? entry.App.id : entry.Group.id))
}

/**
 * 应用图标的拖拽换位。
 *
 * 规则：**拖到某个图标的左半 → 插到它前面；右半 → 插到它后面。**
 *
 * 目标位置一律用「参照节点」表达（插到参照之前），有两个原因：顶层应用是按**行**
 * 存放的，一个下标表达不了「第几行的第几个」；而且先摘后插会让下标整体左移，用
 * 下标很容易算偏一位。
 *
 * 「插到 X 后面」在 X 是最后一个时退化成「追加到末尾」（`beforeId = null`）。
 */
export function useAppDrag(deps: {
  data: AppData
  // store 的 action 会顺带返回成功与否，这里不关心返回值。
  moveApp: (id: string, parentId: string | null, beforeId: string | null) => Promise<unknown>
  setDraggingApp: (id: string | null) => void
  setDropHint: (hint: { appId: string; position: 'before' | 'after' } | null) => void
  draggingAppId: string | null
}) {
  const { data, moveApp, setDraggingApp, setDropHint, draggingAppId } = deps

  const onDragStart = useCallback(
    (event: React.DragEvent<HTMLElement>, app: AppEXE) => {
      // 只用于内部换位，别让系统文件拖放掺和进来。
      event.dataTransfer.effectAllowed = 'move'
      event.dataTransfer.setData('text/plain', app.id)
      setDraggingApp(app.id)
    },
    [setDraggingApp],
  )

  const onDragOverIcon = useCallback(
    (event: React.DragEvent<HTMLElement>, target: AppEXE) => {
      if (!draggingAppId || draggingAppId === target.id) return
      // 必须阻止默认行为，否则浏览器不会派发 drop。
      event.preventDefault()
      event.dataTransfer.dropEffect = 'move'

      const box = event.currentTarget.getBoundingClientRect()
      const after = event.clientX > box.left + box.width / 2
      setDropHint({ appId: target.id, position: after ? 'after' : 'before' })
    },
    [draggingAppId, setDropHint],
  )

  const onDropOnIcon = useCallback(
    (event: React.DragEvent<HTMLElement>, target: AppEXE) => {
      event.preventDefault()
      setDropHint(null)
      setDraggingApp(null)
      if (!draggingAppId || draggingAppId === target.id) return

      const box = event.currentTarget.getBoundingClientRect()
      const after = event.clientX > box.left + box.width / 2

      const placement = findPlacement(data.items, target.id)
      if (!placement) return

      const at = placement.siblingIds.indexOf(target.id)
      const beforeId = after ? (placement.siblingIds[at + 1] ?? null) : target.id
      void moveApp(draggingAppId, placement.parentId, beforeId)
    },
    [data.items, draggingAppId, moveApp, setDraggingApp, setDropHint],
  )

  /** 松手在容器空白处（不在任何图标上）→ 追加到该容器末尾。 */
  const onDropOnContainer = useCallback(
    (event: React.DragEvent<HTMLElement>, parentId: string | null) => {
      event.preventDefault()
      setDropHint(null)
      setDraggingApp(null)
      if (!draggingAppId) return
      void moveApp(draggingAppId, parentId, null)
    },
    [draggingAppId, moveApp, setDraggingApp, setDropHint],
  )

  const onDragEnd = useCallback(() => {
    setDropHint(null)
    setDraggingApp(null)
  }, [setDraggingApp, setDropHint])

  return { onDragStart, onDragOverIcon, onDropOnIcon, onDropOnContainer, onDragEnd }
}
