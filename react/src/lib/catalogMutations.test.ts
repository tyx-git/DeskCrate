import { describe, expect, it } from 'vitest'

import { moveApp, moveGroup } from './catalogMutations'
import type { AppData, AppEXE, Category, Group, RootItem } from '../types/catalog'

const app = (id: string, name = id): AppEXE => ({
  id,
  name,
  sourcePath: `https://example.com/${id}`,
  iconUrl: '',
  iconType: 'Url',
  createdTime: '2026-09-23T00:00:00Z',
})

const group = (id: string, children: Group['children'] = []): Group => ({
  id,
  name: id,
  isCollapsed: false,
  children,
  createdTime: '2026-09-23T00:00:00Z',
})

const category = (id: string, groups: Category['groups']): Category => ({
  id,
  name: id,
  isCollapsed: false,
  groups,
  createdTime: '2026-09-23T00:00:00Z',
})

const data = (...items: RootItem[]): AppData => ({ version: 2, items })

function childIds(tree: AppData, categoryId: string): string[] {
  const root = tree.items.find((item) => 'Category' in item && item.Category.id === categoryId)
  if (!root || !('Category' in root)) return []
  return root.Category.groups.map((item) => ('App' in item ? item.App.id : item.Group.id))
}

describe('moveApp', () => {
  it('reorders apps before a mixed app/group sibling in the same category', () => {
    const input = data({
      Category: category('category', [
        { App: app('a') },
        { Group: group('group') },
        { App: app('b') },
      ]),
    })

    const result = moveApp(input, 'b', 'category', 'group')

    expect(result.ok).toBe(true)
    if (result.ok) expect(childIds(result.data, 'category')).toEqual(['a', 'b', 'group'])
  })
})

describe('moveGroup', () => {
  it('reorders mixed siblings and preserves the moved subtree', () => {
    const input = data({
      Category: category('category', [
        { Group: group('first', [{ App: app('nested-app') }]) },
        { App: app('middle') },
        { Group: group('second') },
      ]),
    })

    const result = moveGroup(input, 'second', 'category', 'middle')

    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(childIds(result.data, 'category')).toEqual(['first', 'second', 'middle'])
      const moved = result.data.items[0]
      expect(moved && 'Category' in moved && moved.Category.groups[0]).toEqual(
        expect.objectContaining({ Group: expect.objectContaining({ id: 'first' }) }),
      )
    }
  })

  it('rejects moving a group into its own descendant without changing the source tree', () => {
    const input = data({
      Category: category('category', [
        { Group: group('parent', [{ Group: group('child') }]) },
      ]),
    })

    const result = moveGroup(input, 'parent', 'category', 'child')

    expect(result).toEqual({ ok: false, code: 'invalid_group_position' })
    expect(childIds(input, 'category')).toEqual(['parent'])
  })

  it('rejects cross-parent movement', () => {
    const input = data(
      { Category: category('one', [{ Group: group('group') }]) },
      { Category: category('two', []) },
    )

    expect(moveGroup(input, 'group', 'two', null)).toEqual({
      ok: false,
      code: 'invalid_group_parent',
    })
  })
})
