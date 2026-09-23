import type { CSSProperties } from 'react'

import { useAppDrag } from '../../hooks/useAppDrag'
import { countApps } from '../../lib/tree'
import { useCatalogStore } from '../../state/catalogStore'
import { useUiStore } from '../../state/uiStore'
import type { Category } from '../../types/catalog'
import { IconButton } from '../common/IconButton'
import { ItemList } from './ItemList'
import { Row } from './Row'
import { useTreeActions } from './treeActions'

interface CategorySectionProps {
  category: Category
  /** 分类在列表中的序号，仅用于收束动画的错峰延时。 */
  index: number
}

/** 一个顶层分类：标题行 + 折叠内容。 */
export function CategorySection({ category, index }: CategorySectionProps) {
  const { activeDropId, toggleCollapse, contextMenuForNode, addAppTo } = useTreeActions()
  const data = useCatalogStore((state) => state.data)
  const moveApp = useCatalogStore((state) => state.moveApp)
  const moveGroup = useCatalogStore((state) => state.moveGroup)
  const draggingNode = useUiStore((state) => state.draggingNode)
  const setDraggingNode = useUiStore((state) => state.setDraggingNode)
  const setDropHint = useUiStore((state) => state.setDropHint)
  const drag = useAppDrag({ data, moveApp, moveGroup, setDraggingNode, setDropHint, draggingNode })

  const location = {
    id: category.id,
    kind: 'category' as const,
    name: category.name,
    parentId: null,
    parentKind: null,
  }

  return (
    // `--dc-i` 供收束动画做错峰：分类按顺序依次左收，形成「被抽走」的层次感。
    // 顶层区占用 0，分类从 1 起。
    <section
      className={`dc-converge ${draggingNode?.kind === 'app' ? 'dc-drop-zone' : ''}`}
      style={{ '--dc-i': index + 1 } as CSSProperties}
    >
      <Row
        depth={0}
        label={category.name}
        collapsed={category.isCollapsed}
        dropId={category.id}
        isDropTarget={activeDropId === category.id}
        onDragOver={(event) => {
          if (draggingNode?.kind !== 'app') return
          event.preventDefault()
          event.dataTransfer.dropEffect = 'move'
        }}
        onDrop={(event) => drag.onDropOnContainer(event, category.id)}
        onToggle={() => toggleCollapse(category.id, !category.isCollapsed)}
        onContextMenu={(event) => contextMenuForNode(event, location)}
        trailing={
          category.isCollapsed && countApps(category.groups) > 0 ? (
            <span className="shrink-0 rounded-chip bg-chip px-[5px] py-[1px] text-chip text-chip-ink">
              {countApps(category.groups)}
            </span>
          ) : null
        }
        actions={
          <IconButton
            label={`在「${category.name}」中添加应用`}
            onClick={() => addAppTo(location)}
            className="text-caret hover:text-ink"
          >
            +
          </IconButton>
        }
      />

      {category.isCollapsed ? null : (
        <div className="pl-[5%]">
          <ItemList
            items={category.groups}
            depth={1}
            parentId={category.id}
            parentKind="category"
          />
        </div>
      )}
    </section>
  )
}
