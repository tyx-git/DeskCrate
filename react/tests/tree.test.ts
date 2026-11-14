// tree.ts 的只读查询与后端 services/tree.rs 的语义对齐测试。
import { describe, expect, it } from 'vitest'

import type { AppData, AppEXE, Category, Group, Item, RootItem } from '../src/types/catalog'
import {
  MAX_GROUP_DEPTH,
  collectCollapseTargets,
  countAll,
  locateApp,
  locateNode,
  nextAppId,
  nodeDepth,
} from '../src/lib/tree'

const TIME = '2026-01-01T00:00:00Z'
const app = (id: string): AppEXE => ({
  id,
  name: id,
  sourcePath: `C:\\Apps\\${id}.exe`,
  iconUrl: 'data:image/png;base64,x',
  iconType: 'Executable',
  createdTime: TIME,
})
const grp = (id: string, children: Item[] = []): Group => ({ id, name: id, isCollapsed: false, children, createdTime: TIME })
const cat = (id: string, groups: Item[] = []): Category => ({ id, name: id, isCollapsed: false, groups, createdTime: TIME })
const data = (...items: RootItem[]): AppData => ({ version: 1, items })

/** 顶层行 [a,b] + 分类 c（内含分组 g1[x, g2[y]] 与散应用 z）+ 末行 [d]。 */
function fixture(): AppData {
  return data(
    { Apps: [app('a'), app('b')] },
    { Category: cat('c', [{ Group: grp('g1', [{ App: app('x') }, { Group: grp('g2', [{ App: app('y') }]) }]) }, { App: app('z') }]) },
    { Apps: [app('d')] },
  )
}

describe('查询', () => {
  it('locateApp 区分顶层行与容器内应用', () => {
    const d = fixture()
    const top = locateApp(d, 'a')
    expect(top?.parentId).toBeNull()
    expect(top?.rootIndex).toBe(0)
    const nested = locateApp(d, 'y')
    expect(nested?.parentId).toBe('g2')
    expect(locateApp(d, 'missing')).toBeNull()
  })

  it('locateNode 给出深度与父容器', () => {
    const d = fixture()
    expect(locateNode(d, 'c')).toMatchObject({ kind: 'Category', parentId: null, depth: 0 })
    expect(locateNode(d, 'g2')).toMatchObject({ kind: 'Group', parentId: 'g1', depth: 2 })
  })

  it('countAll 统计跨行与嵌套应用', () => {
    expect(countAll(fixture())).toEqual({ apps: 6, cats: 1 })
  })

  it('nextAppId 跳过分组找下一个应用', () => {
    const list: Item[] = [{ App: app('x') }, { Group: grp('g') }, { App: app('y') }]
    expect(nextAppId(list, 0)).toBe('y')
    expect(nextAppId(list, 2)).toBeNull()
  })

  it('nodeDepth 与 MAX_GROUP_DEPTH 与后端一致', () => {
    expect(MAX_GROUP_DEPTH).toBe(4)
    expect(nodeDepth(fixture(), 'c')).toBe(0)
    expect(nodeDepth(fixture(), 'g1')).toBe(1)
    expect(nodeDepth(fixture(), 'g2')).toBe(2)
  })
})

describe('collectCollapseTargets', () => {
  it('覆盖分类与分组，且只返回状态不一致的节点', () => {
    const targets = collectCollapseTargets(fixture(), true)
    expect(targets.sort()).toEqual(['c', 'g1', 'g2'])

    const collapsed = fixture()
    collapsed.items[1] = { Category: { ...cat('c'), isCollapsed: true } }
    expect(collectCollapseTargets(collapsed, true)).toEqual([])
  })
})
