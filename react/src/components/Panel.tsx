// 侧栏面板本体：收束变形几何 + 头部/搜索/目录/底部 + 全部面板内覆盖层。
// 面板几何永远由窗口尺寸推导：展开态四周留阴影空白，收束态只剩指示条。
import { useEffect, useRef } from 'react'

import { describeError, isTauriRuntime } from '../api/client'
import { useAutoCollapse } from '../hooks/useAutoCollapse'
import { useFileDrop } from '../hooks/useFileDrop'
import { useWindowResize } from '../hooks/useWindowResize'
import { WINDOW_SPEC } from '../spec/windowSpec'
import { useCatalogStore } from '../state/catalogStore'
import { useSettingsStore } from '../state/settingsStore'
import { useUiStore } from '../state/uiStore'
import { Catalog } from './Catalog'
import { ContextMenu } from './ContextMenu'
import { DropVeil } from './DropVeil'
import { EmptyState } from './EmptyState'
import { NodeDialog } from './NodeDialog'
import { PanelFoot } from './PanelFoot'
import { PanelHead } from './PanelHead'
import { SearchBox } from './SearchBox'
import { SettingsSheet } from './SettingsSheet'
import { Toasts } from './Toasts'

const GUTTER_X = WINDOW_SPEC.sidebarLeftPx + WINDOW_SPEC.overlayShadowGutterPx
const GUTTER_Y = WINDOW_SPEC.overlayShadowGutterPx * 2

export function Panel({ windowSize }: { windowSize: { width: number; height: number } }) {
  const panelRef = useRef<HTMLElement>(null)
  const searchRef = useRef<HTMLInputElement>(null)
  const { start, resizing } = useWindowResize()

  const viewMode = useSettingsStore((s) => s.viewMode)
  const stageConverged = useSettingsStore((s) => s.stageConverged)
  const status = useCatalogStore((s) => s.status)
  const error = useCatalogStore((s) => s.error)
  const itemCount = useCatalogStore((s) => s.data.items.length)
  const sizeBadge = useUiStore((s) => s.sizeBadge)
  const ctx = useUiStore((s) => s.ctx)

  useAutoCollapse(panelRef)
  useFileDrop()

  // 面板内容尺寸供设置页展示；收束态的 3×80 不是用户关心的「窗口尺寸」。
  useEffect(() => {
    const el = panelRef.current
    if (!el) return
    const observer = new ResizeObserver(() => {
      const settings = useSettingsStore.getState()
      if (settings.viewMode === 'collapsed' || settings.stageConverged) return
      const size = { width: el.clientWidth, height: el.clientHeight }
      if (settings.panelSize?.width !== size.width || settings.panelSize?.height !== size.height) {
        settings.setPanelSize(size)
      }
    })
    observer.observe(el)
    return () => observer.disconnect()
  }, [])

  // 点击菜单以外任意处关闭菜单（菜单自身已阻止冒泡）。
  useEffect(() => {
    if (!ctx) return
    const onPointerDown = (e: PointerEvent) => {
      if (!(e.target as HTMLElement).closest('.ctx')) useUiStore.getState().closeCtx()
    }
    document.addEventListener('pointerdown', onPointerDown)
    return () => document.removeEventListener('pointerdown', onPointerDown)
  }, [ctx])

  const strip = viewMode === 'collapsed' || stageConverged
  const geometry = strip
    ? {
        left: WINDOW_SPEC.collapsedStripLeftPx,
        top: WINDOW_SPEC.overlayShadowGutterPx,
        width: WINDOW_SPEC.collapsedStripWidthPx,
        height: WINDOW_SPEC.collapsedStripHeightPx,
      }
    : {
        left: WINDOW_SPEC.sidebarLeftPx,
        top: WINDOW_SPEC.overlayShadowGutterPx,
        width: Math.max(0, windowSize.width - GUTTER_X),
        height: Math.max(0, windowSize.height - GUTTER_Y),
      }

  const onClick = (e: React.MouseEvent) => {
    if (viewMode === 'collapsed') {
      e.stopPropagation()
      void useSettingsStore.getState().setViewMode('expanded')
    }
  }

  const onContextMenu = (e: React.MouseEvent) => {
    e.preventDefault()
    if (viewMode === 'collapsed') return
    const t = e.target as HTMLElement
    if (t.closest('.ctx, .dialog, .settings-sheet, .toasts, .scrim')) return
    const ui = useUiStore.getState()
    const appEl = t.closest<HTMLElement>('[data-app]')
    if (appEl?.dataset.app) {
      ui.openCtx('app', appEl.dataset.app, e.clientX, e.clientY)
      return
    }
    const rowEl = t.closest<HTMLElement>('.row')
    if (rowEl?.dataset.id) {
      const kind = rowEl.closest<HTMLElement>('.node')?.dataset.kind === 'Group' ? 'Group' : 'Category'
      ui.openCtx(kind, rowEl.dataset.id, e.clientX, e.clientY)
      return
    }
    ui.openCtx('blank', '', e.clientX, e.clientY)
  }

  // 系统文件拖入由 useFileDrop 走原生拖放事件拿真实路径。
  const showEmpty = status === 'ready' && itemCount === 0

  return (
    <>
      <section
        ref={panelRef}
        className={`panel${strip ? ' is-collapsed' : ''}${resizing ? ' is-resizing' : ''}`}
        style={geometry}
        aria-label="DeskCrate 侧栏"
        onClick={onClick}
        onContextMenu={onContextMenu}
      >
        <div className="panel-body">
          <PanelHead />
          <SearchBox searchRef={searchRef} />
          {status === 'error' ? (
            <div className="empty">
              <p className="empty-title">目录加载失败</p>
              <p className="empty-desc">{error ? describeError(error) : '读取数据时出错'}</p>
              {!isTauriRuntime() && (
                <p className="state-note">
                  当前不在 DeskCrate 窗口内：请用 debug exe 或发布版启动；浏览器预览只能看到静态界面。
                </p>
              )}
              <div className="empty-actions">
                <button className="btn btn-accent" onClick={() => void useCatalogStore.getState().load()}>
                  重试
                </button>
              </div>
            </div>
          ) : showEmpty ? (
            <EmptyState />
          ) : (
            <Catalog />
          )}
          <PanelFoot start={start} />

          <NodeDialog />
          <SettingsSheet />
          <Toasts />
          <ContextMenu panelRef={panelRef} />
          {sizeBadge && <div className="size-badge mono">{sizeBadge}</div>}
          <DropVeil />
        </div>
      </section>
      {!strip && (
        <>
          <div className="edge-handle edge-top" aria-hidden="true" onPointerDown={start('n')} />
          <div className="edge-handle edge-right" aria-hidden="true" onPointerDown={start('e')} />
          <div className="edge-handle edge-bottom" aria-hidden="true" onPointerDown={start('s')} />
        </>
      )}
    </>
  )
}
