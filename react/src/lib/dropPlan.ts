// 拖拽落点 → 后端命令参数的纯映射。
// 后端 move_app 只支持「插到参照应用之前」与「追加到末尾」，没有根级下标；
// move_group 只支持同级重排；分类根级重排没有对应命令。因此落点只在可表达
// 时返回计划，不可表达返回 null（UI 不显示插入标记，拒绝这次拖放）。
import { isAppsRoot, type AppData, type Item } from '../types/catalog'
import { locateApp, locateNode, nextAppId } from './tree'

export type AppDropTarget =
  | { type: 'before' | 'after'; refAppId: string }
  | { type: 'into'; parentId: string | null; rootIndex?: number }
  | { type: 'root'; rootIndex: number; side: 'before' | 'after' }

export interface MovePlan {
  parentId: string | null
  beforeId: string | null
}

export function planAppMove(data: AppData, dragId: string, target: AppDropTarget): MovePlan | null {
  if (target.type === 'before' || target.type === 'after') {
    const ref = locateApp(data, target.refAppId)
    if (!ref || ref.app.id === dragId) return null

    if (ref.parentId !== null) {
      const beforeId = target.type === 'before' ? target.refAppId : nextAppId(ref.list, ref.index)
      return { parentId: ref.parentId, beforeId }
    }

    if (target.type === 'before') return { parentId: null, beforeId: target.refAppId }

    const next = nextAppId(ref.list, ref.index)
    if (next) return { parentId: null, beforeId: next }
    // 行尾追加只有当这行就是最后一个根节点时才等于「 appendToRoot」。
    if (ref.rootIndex === data.items.length - 1) return { parentId: null, beforeId: null }
    return null
  }

  if (target.type === 'into') {
    if (target.parentId !== null) return { parentId: target.parentId, beforeId: null }
    const row = target.rootIndex !== undefined ? data.items[target.rootIndex] : undefined
    if (row && isAppsRoot(row) && row.Apps.length > 0) {
      return { parentId: null, beforeId: row.Apps[0].id }
    }
    return { parentId: null, beforeId: null }
  }

  // 根级分界：只有紧邻应用行且可表达时才接受。
  if (target.type !== 'root') return null
  const at = data.items[target.rootIndex]
  if (target.side === 'before' && at && isAppsRoot(at) && at.Apps.length > 0) {
    return { parentId: null, beforeId: at.Apps[0].id }
  }
  if (
    target.side === 'after' &&
    at &&
    isAppsRoot(at) &&
    target.rootIndex === data.items.length - 1
  ) {
    return { parentId: null, beforeId: null }
  }
  return null
}

export type GroupDropTarget = { type: 'sibling'; refNodeId: string; side: 'before' | 'after' }

export function planGroupMove(data: AppData, dragId: string, target: GroupDropTarget): MovePlan | null {
  const loc = locateNode(data, dragId)
  const ref = locateNode(data, target.refNodeId)
  if (!loc || !ref || loc.kind !== 'Group' || ref.kind !== 'Group') return null
  if (loc.parentId === null || loc.parentId !== ref.parentId) return null
  if (ref.node.id === dragId) return null

  if (target.side === 'before') return { parentId: loc.parentId, beforeId: ref.node.id }
  const siblings = ref.list as Item[]
  const next = siblings[ref.index + 1]
  const beforeId = next ? ('App' in next ? next.App.id : next.Group.id) : null
  return { parentId: loc.parentId, beforeId }
}