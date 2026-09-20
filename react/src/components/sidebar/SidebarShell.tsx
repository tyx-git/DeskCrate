import { useCallback, useRef, type CSSProperties } from 'react'

import { useFileDrop, type DropTarget } from '../../hooks/useFileDrop'
import { useAutoCollapse } from '../../hooks/useAutoCollapse'
import { useHotkeys } from '../../hooks/useHotkeys'
import { ROOT_ID } from '../../lib/tree'
import { COLLAPSE_MS, EXPAND_MS } from '../../lib/motion'
import { useCatalogStore } from '../../state/catalogStore'
import { useSettingsStore } from '../../state/settingsStore'
import { useUiStore } from '../../state/uiStore'
import { ContextMenu } from '../overlays/ContextMenu'
import { NodeDialog } from '../overlays/NodeDialog'
import { CollapsedStrip } from './CollapsedStrip'
import { ResizeHandles } from './ResizeHandles'
import { Sidebar } from './Sidebar'

/**
 * 悬浮层外壳。
 *
 * 窗口本身是透明、无边框、置顶的，这里负责三件事：
 * 1. 用 `.dc-window-*` 留出阴影空间，让玻璃面板的 box-shadow 不被裁掉；
 * 2. 用 `.dc-container` 建立容器查询上下文，侧栏内部所有 cqw 字号都以
 *    侧栏自身宽度为基准；
 * 3. 承载右键菜单、对话框和文件拖放这些窗口级交互。
 */
export function SidebarShell() {
  const shellRef = useRef<HTMLDivElement>(null)
  const panelRef = useRef<HTMLDivElement>(null)
  const viewMode = useSettingsStore((state) => state.viewMode)
  const stageConverged = useSettingsStore((state) => state.stageConverged)
  const collapsed = viewMode === 'collapsed'

  const addApp = useCatalogStore((state) => state.addApp)
  const notify = useUiStore((state) => state.notify)
  const openContextMenu = useUiStore((state) => state.openContextMenu)

  useAutoCollapse(shellRef, panelRef)
  useHotkeys()

  const handleDropPaths = useCallback(
    (paths: string[], target: DropTarget) => {
      // 命中分界线 → 作为顶层应用插到那个位置；命中分类/分组行 → 放进该容器。
      // 顶层没有对应的后端节点，`parentId` 用 `null` 表达，位置交给 `rootIndex`。
      const parentId = target.index !== null
        ? null
        : target.id === null || target.id === ROOT_ID
          ? null
          : target.id
      const rootIndex = target.index

      void (async () => {
        let added = 0
        for (const path of paths) {
          const created = await addApp(parentId, path, undefined, rootIndex ?? undefined)
          if (created) {
            added += 1
          }
        }
        if (added > 0) {
          notify('info', `已添加 ${added} 个条目`)
        }
      })()
    },
    [addApp, notify],
  )

  const { isDragging, activeDropId, activeDropIndex } = useFileDrop(handleDropPaths)

  return (
    <div
      ref={shellRef}
      // 动画时长从这里下发到 CSS，保证 JS 等待与 CSS 过渡读的是同一个数字。
      style={
        {
          '--dc-collapse-ms': `${COLLAPSE_MS}ms`,
          '--dc-expand-ms': `${EXPAND_MS}ms`,
        } as CSSProperties
      }
      className={`relative h-full ${collapsed ? 'dc-window-collapsed' : 'dc-window-expanded'}`}
      onContextMenu={(event) => {
        event.preventDefault()
        if (collapsed) {
          return
        }
        // 行与图标内部已经 stopPropagation，能走到这里的都是空白区。
        openContextMenu({ x: event.clientX, y: event.clientY, target: null })
      }}
    >
      <div className="dc-container">
        {collapsed ? (
          <CollapsedStrip />
        ) : (
          // 收束动画只有一层：宽高一起往左上角收成指示条同大的一条。
          // 曾经拆成两层做「先横向后纵向」，实测观感是别扭的两段式。
          <div
            ref={panelRef}
            className={`dc-stage ${stageConverged ? 'dc-stage--converged' : ''}`}
          >
            <Sidebar
              activeDropId={activeDropId}
              activeDropIndex={activeDropIndex}
              isDragging={isDragging}
            />
            {/* 收束时从面板顶部扫到底部的光带，被一起夹进细丝里。 */}
            <span aria-hidden className="dc-sweep" />
          </div>
        )}

        {isDragging && !collapsed ? (
          <div className="pointer-events-none absolute inset-0 rounded-panel ring-2 ring-inset ring-focus" />
        ) : null}
      </div>

      {/* 收起态窗口只有一条 3px 指示条；收束动画期间也不要露出把手。 */}
      {collapsed || stageConverged ? null : <ResizeHandles />}

      <ContextMenu />
      <NodeDialog />
    </div>
  )
}
