import { useCallback } from 'react'

import { asAppsRoot, asCategoryRoot } from '../lib/tree'
import type { AppData, AppEXE, Group, Item, RootItem } from '../types/catalog'
import type { DraggedNode, DropHint } from '../state/uiStore'

interface Placement {
  parentId: string | null
  siblingIds: string[]
}

function findPlacement(items: RootItem[], node: DraggedNode): Placement | null {
  if (node.kind === 'app') {
    for (const root of items) {
      const apps = asAppsRoot(root)
      if (apps?.some((app) => app.id === node.id)) {
        return { parentId: null, siblingIds: apps.map((app) => app.id) }
      }
    }
  }

  const walk = (entries: readonly Item[], parentId: string): Placement | null => {
    for (const entry of entries) {
      if (node.kind === 'app' && 'App' in entry && entry.App.id === node.id) {
        return { parentId, siblingIds: siblingIdsOf(entries) }
      }
      if ('Group' in entry) {
        if (node.kind === 'group' && entry.Group.id === node.id) {
          return { parentId, siblingIds: siblingIdsOf(entries) }
        }
        const found = walk(entry.Group.children, entry.Group.id)
        if (found) return found
      }
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

function siblingIdsOf(entries: readonly Item[]): string[] {
  return entries.map((entry) => ('App' in entry ? entry.App.id : entry.Group.id))
}

function groupContains(group: Group, id: string): boolean {
  return group.children.some(
    (entry) =>
      ('App' in entry ? entry.App.id : entry.Group.id) === id ||
      ('Group' in entry && groupContains(entry.Group, id)),
  )
}

export function useAppDrag(deps: {
  data: AppData
  moveApp: (id: string, parentId: string | null, beforeId: string | null) => Promise<unknown>
  moveGroup: (id: string, parentId: string, beforeId: string | null) => Promise<unknown>
  setDraggingNode: (node: DraggedNode | null) => void
  setDropHint: (hint: DropHint | null) => void
  draggingNode: DraggedNode | null
}) {
  const { data, moveApp, moveGroup, setDraggingNode, setDropHint, draggingNode } = deps

  const onDragStartApp = useCallback(
    (event: React.DragEvent<HTMLElement>, app: AppEXE) => {
      event.dataTransfer.effectAllowed = 'move'
      event.dataTransfer.setData('text/plain', app.id)
      setDraggingNode({ id: app.id, kind: 'app' })
    },
    [setDraggingNode],
  )

  const onDragStartGroup = useCallback(
    (event: React.DragEvent<HTMLElement>, group: Group) => {
      event.dataTransfer.effectAllowed = 'move'
      event.dataTransfer.setData('text/plain', group.id)
      setDraggingNode({ id: group.id, kind: 'group' })
    },
    [setDraggingNode],
  )

  const onDragOverNode = useCallback(
    (event: React.DragEvent<HTMLElement>, target: DraggedNode, axis: 'x' | 'y' = 'x') => {
      if (!draggingNode || (draggingNode.id === target.id && draggingNode.kind === target.kind)) return
      event.preventDefault()
      event.stopPropagation()
      event.dataTransfer.dropEffect = 'move'
      const box = event.currentTarget.getBoundingClientRect()
      const after = axis === 'x'
        ? event.clientX > box.left + box.width / 2
        : event.clientY > box.top + box.height / 2
      setDropHint({ nodeId: target.id, position: after ? 'after' : 'before' })
    },
    [draggingNode, setDropHint],
  )

  const onDropOnNode = useCallback(
    (event: React.DragEvent<HTMLElement>, target: DraggedNode, axis: 'x' | 'y' = 'x') => {
      event.preventDefault()
      event.stopPropagation()
      const dragged = draggingNode
      setDropHint(null)
      setDraggingNode(null)
      if (!dragged || (dragged.id === target.id && dragged.kind === target.kind)) return

      const targetPlacement = findPlacement(data.items, target)
      const sourcePlacement = findPlacement(data.items, dragged)
      if (!targetPlacement || !sourcePlacement || sourcePlacement.parentId !== targetPlacement.parentId) return

      if (dragged.kind === 'group') {
        const source = findGroup(data.items, dragged.id)
        if (!source || groupContains(source, target.id)) return
      }

      const box = event.currentTarget.getBoundingClientRect()
      const after = axis === 'x'
        ? event.clientX > box.left + box.width / 2
        : event.clientY > box.top + box.height / 2
      const at = targetPlacement.siblingIds.indexOf(target.id)
      const beforeId = after ? (targetPlacement.siblingIds[at + 1] ?? null) : target.id
      if (beforeId === dragged.id) return

      if (dragged.kind === 'app') {
        void moveApp(dragged.id, targetPlacement.parentId, beforeId)
      } else if (targetPlacement.parentId !== null) {
        void moveGroup(dragged.id, targetPlacement.parentId, beforeId)
      }
    },
    [data.items, draggingNode, moveApp, moveGroup, setDraggingNode, setDropHint],
  )

  const onDropOnContainer = useCallback(
    (event: React.DragEvent<HTMLElement>, parentId: string | null) => {
      event.preventDefault()
      event.stopPropagation()
      const dragged = draggingNode
      setDropHint(null)
      setDraggingNode(null)
      if (!dragged || dragged.kind !== 'app') return
      void moveApp(dragged.id, parentId, null)
    },
    [draggingNode, moveApp, setDraggingNode, setDropHint],
  )

  const onDragEnd = useCallback(() => {
    setDropHint(null)
    setDraggingNode(null)
  }, [setDraggingNode, setDropHint])

  return { onDragStartApp, onDragStartGroup, onDragOverNode, onDropOnNode, onDropOnContainer, onDragEnd }
}

function findGroup(items: RootItem[], id: string): Group | null {
  const walk = (entries: readonly Item[]): Group | null => {
    for (const entry of entries) {
      if ('Group' in entry) {
        if (entry.Group.id === id) return entry.Group
        const nested = walk(entry.Group.children)
        if (nested) return nested
      }
    }
    return null
  }
  for (const root of items) {
    const category = asCategoryRoot(root)
    if (category) {
      const found = walk(category.groups)
      if (found) return found
    }
  }
  return null
}
