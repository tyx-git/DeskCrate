// 落点映射测试：可表达的落点给出命令参数，不可表达的必须拒绝（返回 null）。
import { describe, expect, it } from 'vitest'

import type { AppData, AppEXE, Category, Group, Item, RootItem } from '../src/types/catalog'
import { planAppMove, planGroupMove } from '../src/lib/dropPlan'

const TIME = '2026-01-01T00:00:00Z'
const app = (id: string): AppEXE => ({
  id,
  name: id,
  sourcePath: `C:\\${id}.exe`,
  iconUrl: 'data:x',
  iconType: 'Executable',
  createdTime: TIME,
})
const grp = (id: string, children: Item[] = []): Group => ({ id, name: id, isCollapsed: false, children, createdTime: TIME })
const cat = (id: string, groups: Item[] = []): Category => ({ id, name: id, isCollapsed: false, groups, createdTime: TIME })
const data = (...items: RootItem[]): AppData => ({ version: 1, items })

/** 行[a,b] + 分类 c[g1[x], z] + 行[d]。 */
function fixture(): AppData {
  return data(
    { Apps: [app('a'), app('b')] },
    { Category: cat('c', [{ Group: grp('g1', [{ App: app('x') }]) }, { App: app('z') }]) },
    { Apps: [app('d')] },
  )
}

describe('planAppMove', () => {
  it('应用之前/之后映射为同容器 beforeId', () => {
    expect(planAppMove(fixture(), 'd', { type: 'before', refAppId: 'a' })).toEqual({ parentId: null, beforeId: 'a' })
    expect(planAppMove(fixture(), 'd', { type: 'after', refAppId: 'a' })).toEqual({ parentId: null, beforeId: 'b' })
    expect(planAppMove(fixture(), 'z', { type: 'before', refAppId: 'x' })).toEqual({ parentId: 'g1', beforeId: 'x' })
  })

  it('容器末尾应用之后：追加到容器末尾', () => {
    expect(planAppMove(fixture(), 'z', { type: 'after', refAppId: 'x' })).toEqual({ parentId: 'g1', beforeId: null })
  })

  it('顶层行尾追加仅当该行是最后一个根节点', () => {
    expect(planAppMove(fixture(), 'z', { type: 'after', refAppId: 'd' })).toEqual({ parentId: null, beforeId: null })
    // b 所在行后面还有分类与行[d]，「行尾」无法用 append 表达
    expect(planAppMove(fixture(), 'z', { type: 'after', refAppId: 'b' })).toBeNull()
  })

  it('拖到自身上拒绝', () => {
    expect(planAppMove(fixture(), 'a', { type: 'before', refAppId: 'a' })).toBeNull()
  })

  it('放入容器 / 顶层网格', () => {
    expect(planAppMove(fixture(), 'a', { type: 'into', parentId: 'g1' })).toEqual({ parentId: 'g1', beforeId: null })
    expect(planAppMove(fixture(), 'a', { type: 'into', parentId: null, rootIndex: 0 })).toEqual({ parentId: null, beforeId: 'a' })
    expect(planAppMove(fixture(), 'a', { type: 'into', parentId: null, rootIndex: 2 })).toEqual({ parentId: null, beforeId: 'd' })
  })

  it('根级分界：紧邻应用行才可表达', () => {
    expect(planAppMove(fixture(), 'x', { type: 'root', rootIndex: 0, side: 'before' })).toEqual({ parentId: null, beforeId: 'a' })
    expect(planAppMove(fixture(), 'x', { type: 'root', rootIndex: 2, side: 'after' })).toEqual({ parentId: null, beforeId: null })
    // 分类两侧的分界没有对应命令
    expect(planAppMove(fixture(), 'x', { type: 'root', rootIndex: 1, side: 'before' })).toBeNull()
    expect(planAppMove(fixture(), 'x', { type: 'root', rootIndex: 1, side: 'after' })).toBeNull()
    // 非末行的 after 分界无法表达
    expect(planAppMove(fixture(), 'x', { type: 'root', rootIndex: 0, side: 'after' })).toBeNull()
  })
})

describe('planGroupMove', () => {
  const d = data({ Category: cat('c', [{ Group: grp('g1') }, { App: app('x') }, { Group: grp('g2') }]) })

  it('同级之前/之后', () => {
    expect(planGroupMove(d, 'g2', { type: 'sibling', refNodeId: 'g1', side: 'before' })).toEqual({ parentId: 'c', beforeId: 'g1' })
    expect(planGroupMove(d, 'g1', { type: 'sibling', refNodeId: 'g2', side: 'after' })).toEqual({ parentId: 'c', beforeId: null })
  })

  it('插到末项分组之前', () => {
    expect(planGroupMove(d, 'g1', { type: 'sibling', refNodeId: 'g2', side: 'before' })).toEqual({ parentId: 'c', beforeId: 'g2' })
  })

  it('跨父容器与自身拒绝', () => {
    const nested = data({ Category: cat('c', [{ Group: grp('g1', [{ Group: grp('g2') }]) }]) })
    expect(planGroupMove(nested, 'g2', { type: 'sibling', refNodeId: 'g1', side: 'before' })).toBeNull()
    expect(planGroupMove(d, 'g1', { type: 'sibling', refNodeId: 'g1', side: 'before' })).toBeNull()
  })
})