import type { ReactNode } from 'react'

import { Caret } from '../common/Caret'

interface RowProps {
  /** 0 表示顶层分类，1 及以上表示分组，用于匹配设计稿的两套字号。 */
  depth: number
  label: string
  /** 是否折叠。 */
  collapsed: boolean
  /** 名称右侧紧跟的徽标，例如分组的应用数量。 */
  trailing?: ReactNode
  /** 悬停时才显示的右侧操作区。 */
  actions?: ReactNode
  onToggle: () => void
  onContextMenu: (event: React.MouseEvent<HTMLElement>) => void
  /** 拖拽落点标识，供 `useFileDrop` 做命中测试。 */
  dropId: string
  isDropTarget: boolean
}

/**
 * 分类与分组共用的标题行。
 *
 * 两套视觉参数来自设计稿：分类行 `padding: 2% 5%`、字重 600、带下边框；
 * 分组行 `padding: 1.2% 5%`、常规字重、无边框。
 */
export function Row({
  depth,
  label,
  collapsed,
  trailing,
  actions,
  onToggle,
  onContextMenu,
  dropId,
  isDropTarget,
}: RowProps) {
  const isCategory = depth === 0

  const sizing = isCategory
    ? 'gap-[2.2cqw] px-[5%] py-[2%] text-cat font-semibold text-ink border-b border-hairline'
    : 'gap-[1.6cqw] px-[5%] py-[1.2%] text-group text-ink-soft'

  const dropClass = isDropTarget ? 'bg-[rgb(58_96_190_/_0.14)] ring-1 ring-inset ring-focus' : ''

  return (
    <div
      data-drop-id={dropId}
      // 挂在整行而不是内层按钮上：右侧的 `+` 操作区在按钮之外，挂在内层时
      // 在那一小块右键会冒泡到外壳，弹出「新建分类」的空白区菜单。
      onContextMenu={onContextMenu}
      className={`group/row flex items-center transition-colors duration-100 hover:bg-hover ${sizing} ${dropClass}`}
    >
      <button
        type="button"
        onClick={onToggle}
        aria-expanded={!collapsed}
        className="flex min-w-0 flex-1 items-center gap-[inherit] text-left outline-none"
      >
        <Caret collapsed={collapsed} className="w-[1.2em] text-caret" />
        <span className="flex min-w-0 items-center gap-[1.4cqw]">
          <span className="dc-ellipsis">{label}</span>
          {trailing}
        </span>
      </button>

      {actions ? (
        <span className="ml-[1.4cqw] flex shrink-0 items-center gap-[0.6cqw] opacity-0 transition-opacity duration-100 group-hover/row:opacity-100 focus-within:opacity-100">
          {actions}
        </span>
      ) : null}
    </div>
  )
}
