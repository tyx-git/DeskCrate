import { IconButton } from '../common/IconButton'
import { Row } from './Row'
import { AppGrid } from './AppGrid'
import { useTreeActions } from './treeActions'
import { asApp, asGroup, type AppEXE, type Group, type Item } from '../../types/catalog'
import { countApps } from '../../lib/tree'

interface ItemListProps {
  items: Item[]
  /** 层级深度：分类的直接子节点为 1。 */
  depth: number
  /** 这批节点的父节点 ID。 */
  parentId: string
  parentKind: 'category' | 'group'
}

/** 相邻应用节点会被合并进同一个网格，分组节点则单独成段，整体保持后端返回的顺序。 */
type Block = { kind: 'apps'; apps: AppEXE[] } | { kind: 'group'; group: Group }

function toBlocks(items: Item[]): Block[] {
  const blocks: Block[] = []

  for (const item of items) {
    const app = asApp(item)
    if (app) {
      const last = blocks[blocks.length - 1]
      if (last && last.kind === 'apps') {
        last.apps.push(app)
      } else {
        blocks.push({ kind: 'apps', apps: [app] })
      }
      continue
    }

    const group = asGroup(item)
    if (group) {
      blocks.push({ kind: 'group', group })
    }
  }

  return blocks
}

/** 递归渲染一层节点：应用进网格，分组继续往下递归。 */
export function ItemList({ items, depth, parentId, parentKind }: ItemListProps) {
  const { activeDropId, toggleCollapse, contextMenuForNode, addAppTo } = useTreeActions()

  return (
    <>
      {toBlocks(items).map((block) => {
        if (block.kind === 'apps') {
          // 相邻应用共享一个网格，key 取首个应用的 ID 即可稳定。
          // `parentId` 让拖到网格空白处时知道该追加到哪个容器。
          return (
            <AppGrid key={`apps-${block.apps[0].id}`} apps={block.apps} parentId={parentId} />
          )
        }

        const { group } = block
        const location = {
          id: group.id,
          kind: 'group' as const,
          name: group.name,
          parentId,
          parentKind,
        }

        return (
          <div key={group.id}>
            <Row
              depth={depth}
              label={group.name}
              collapsed={group.isCollapsed}
              dropId={group.id}
              isDropTarget={activeDropId === group.id}
              onToggle={() => toggleCollapse(group.id, !group.isCollapsed)}
              onContextMenu={(event) => contextMenuForNode(event, location)}
              trailing={
                <span className="shrink-0 rounded-chip bg-chip px-[5px] py-[1px] text-chip text-chip-ink">
                  {countApps(group.children)}
                </span>
              }
              actions={
                <IconButton
                  label={`在「${group.name}」中添加应用`}
                  onClick={() => addAppTo(location)}
                  className="text-caret hover:text-ink"
                >
                  +
                </IconButton>
              }
            />

            {group.isCollapsed ? null : (
              <div className="pl-[5%]">
                <ItemList
                  items={group.children}
                  depth={depth + 1}
                  parentId={group.id}
                  parentKind="group"
                />
              </div>
            )}
          </div>
        )
      })}
    </>
  )
}
