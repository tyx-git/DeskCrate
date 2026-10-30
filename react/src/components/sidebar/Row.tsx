import type { ReactNode } from 'react'

import { Caret } from '../common/Caret'

interface RowProps {
  /** 0 表示顶层分类，1 及以上表示分组。 */
  depth: number
  label: string
  /** 是否折叠。 */
  collapsed: boolean
  /** 名称右侧的计数（白瓷稿里是清单末尾的细数字）。 */
  trailing?: ReactNode
  /** 悬停时才显示的右侧操作区。 */
  actions?: ReactNode
  onToggle: () => void
  onContextMenu: (event: React.MouseEvent<HTMLElement>) => void
  /** 同级节点内部拖拽排序；仅分组行启用。 */
  draggable?: boolean
  onDragStart?: (event: React.DragEvent<HTMLElement>) => void
  onDragOver?: (event: React.DragEvent<HTMLElement>) => void
  onDrop?: (event: React.DragEvent<HTMLElement>) => void
  onDragEnd?: () => void
  dropPosition?: 'before' | 'after' | null
  /** 拖拽落点标识，供 `useFileDrop` 做命中测试。 */
  dropId: string
  isDropTarget: boolean
}

/**
 * 分类与分组共用的标题行。
 *
 * 白瓷稿的清单结构：箭头 + 名称 + 一条点状引导线 + 计数，添加按钮只在
 * 悬停时出现。分类行是正文墨色、分组行退一档，靠缩进和字重区分层级。
 */
export function Row({
  depth,
  label,
  collapsed,
  trailing,
  actions,
  onToggle,
  onContextMenu,
  draggable = false,
  onDragStart,
  onDragOver,
  onDrop,
  onDragEnd,
  dropPosition = null,
  dropId,
  isDropTarget,
}: RowProps) {
  const isCategory = depth === 0

  return (
    <div
      data-drop-id={dropId}
      draggable={draggable}
      // 挂在整行而不是内层按钮上：右侧的 `+` 操作区在按钮之外，挂在内层时
      // 在那一小块右键会冒泡到外壳，弹出「新建分类」的空白区菜单。
      onContextMenu={onContextMenu}
      onDragStart={onDragStart}
      onDragOver={onDragOver}
      onDrop={onDrop}
      onDragEnd={onDragEnd}
      className={`dc-row group/row ${isCategory ? 'dc-row--cat' : 'dc-row--group'} ${
        draggable ? 'cursor-grab active:cursor-grabbing' : ''
      } ${dropPosition === 'before' ? 'dc-drop-node-before' : ''} ${
        dropPosition === 'after' ? 'dc-drop-node-after' : ''
      } ${isDropTarget ? 'dc-drop-zone' : ''}`}
    >
      <button type="button" onClick={onToggle} aria-expanded={!collapsed} className="dc-row__main">
        <Caret
          collapsed={collapsed}
          className={isCategory ? 'w-[9px] text-ink-faint' : 'w-[8px] text-ink-faint'}
        />
        <span className="dc-row__label">{label}</span>
        <span className="dc-row__leader" aria-hidden="true" />
        {trailing}
      </button>

      {actions ? <span className="dc-row__actions">{actions}</span> : null}
    </div>
  )
}
