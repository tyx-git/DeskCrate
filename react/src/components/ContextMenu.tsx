// 面板内右键菜单：按目标类型给出动作，应用可钻入「移动到…」容器子页。
import { useLayoutEffect, useRef, type RefObject } from 'react'

import { flattenContainers, locateApp, MAX_GROUP_DEPTH, nodeDepth } from '../lib/tree'
import { useCatalogStore } from '../state/catalogStore'
import { useUiStore, type CtxTargetType } from '../state/uiStore'
import { Glyph, type GlyphName } from './Glyph'

interface MenuItem {
  key: string
  label: string
  glyph: GlyphName
  kbd?: string
  arrow?: boolean
  danger?: boolean
  onClick: () => void
}

const clamp = (v: number, min: number, max: number) => Math.min(Math.max(v, min), Math.max(min, max))

export function ContextMenu({ panelRef }: { panelRef: RefObject<HTMLElement | null> }) {
  const ctx = useUiStore((s) => s.ctx)
  const menuRef = useRef<HTMLDivElement>(null)

  useLayoutEffect(() => {
    const menu = menuRef.current
    const panel = panelRef.current
    if (!menu || !panel || !ctx) return
    const pr = panel.getBoundingClientRect()
    menu.style.maxHeight = `${pr.height - 16}px`
    const mw = menu.offsetWidth
    const mh = menu.offsetHeight
    const x = clamp(ctx.clientX - pr.left, 6, pr.width - mw - 6)
    const y = clamp(ctx.clientY - pr.top, 6, pr.height - mh - 6)
    menu.style.left = `${x}px`
    menu.style.top = `${y}px`
    menu.querySelector<HTMLElement>('.ctx-item')?.focus({ preventScroll: true })
  }, [ctx, panelRef])

  if (!ctx) return null

  return (
    <div className="ctx" role="menu" ref={menuRef} onClick={(e) => e.stopPropagation()}>
      {ctx.moveAppId ? <MoveMenu appId={ctx.moveAppId} /> : <TargetMenu type={ctx.type} id={ctx.id} />}
    </div>
  )
}

function TargetMenu({ type, id }: { type: CtxTargetType; id: string }) {
  const data = useCatalogStore((s) => s.data)
  const catalog = useCatalogStore.getState()
  const ui = useUiStore.getState()

  const items: MenuItem[] = []
  if (type === 'app') {
    items.push(
      {
        key: 'launch',
        label: '启动',
        glyph: 'launch',
        kbd: '↵',
        onClick: () => {
          const loc = locateApp(data, id)
          ui.closeCtx()
          if (loc) void catalog.launchApp(loc.app)
        },
      },
      { key: 'rename', label: '重命名', glyph: 'pencil', kbd: 'F2', onClick: () => ui.openDialog({ mode: 'rename-app', id }) },
      { key: 'move', label: '移动到…', glyph: 'move', arrow: true, onClick: () => ui.openMoveMenu(id) },
      { key: 'sep', label: '', glyph: 'x', onClick: () => {} },
      {
        key: 'delete',
        label: '删除',
        glyph: 'bin',
        danger: true,
        kbd: 'Del',
        onClick: () => {
          ui.closeCtx()
          void catalog.deleteApp(id)
        },
      },
    )
  } else if (type === 'Category' || type === 'Group') {
    const canNest = type === 'Category' || nodeDepth(data, id) < MAX_GROUP_DEPTH
    items.push(
      { key: 'add', label: '添加应用', glyph: 'plus', onClick: () => ui.openDialog({ mode: 'add', parentId: id }) },
    )
    if (canNest) {
      items.push({
        key: 'group',
        label: type === 'Category' ? '新建分组' : '新建子分组',
        glyph: 'layers',
        onClick: () => ui.openDialog({ mode: 'new-group', parentId: id }),
      })
    }
    items.push(
      { key: 'rename', label: '重命名', glyph: 'pencil', kbd: 'F2', onClick: () => ui.startRename(id) },
      { key: 'sep', label: '', glyph: 'x', onClick: () => {} },
      {
        key: 'delete',
        label: type === 'Category' ? '删除分类' : '删除分组',
        glyph: 'bin',
        danger: true,
        onClick: () => ui.openDialog({ mode: 'delete-node', id }),
      },
    )
  } else {
    items.push(
      { key: 'add', label: '添加应用', glyph: 'plus', onClick: () => ui.openDialog({ mode: 'add' }) },
      { key: 'category', label: '新建分类', glyph: 'grid', onClick: () => ui.openDialog({ mode: 'new-category' }) },
      { key: 'sep', label: '', glyph: 'x', onClick: () => {} },
      { key: 'expand', label: '全部展开', glyph: 'expand', onClick: () => { ui.closeCtx(); void catalog.setAllCollapsed(false) } },
      { key: 'collapse', label: '全部折叠', glyph: 'collapseAll', onClick: () => { ui.closeCtx(); void catalog.setAllCollapsed(true) } },
    )
  }

  return (
    <>
      {items.map((item) =>
        item.key === 'sep' ? (
          <div className="ctx-sep" key={item.key} />
        ) : (
          <button className={`ctx-item${item.danger ? ' danger' : ''}`} key={item.key} onClick={item.onClick}>
            <Glyph name={item.glyph} />
            <span>{item.label}</span>
            {item.kbd && <span className="ctx-kbd">{item.kbd}</span>}
            {item.arrow && (
              <span className="ctx-arrow">
                <Glyph name="chev-right" />
              </span>
            )}
          </button>
        ),
      )}
    </>
  )
}

function MoveMenu({ appId }: { appId: string }) {
  const data = useCatalogStore((s) => s.data)
  const containers = flattenContainers(data)
  const ui = useUiStore.getState()
  const catalog = useCatalogStore.getState()
  const ctx = useUiStore((s) => s.ctx)

  return (
    <>
      <div className="ctx-title">
        <button aria-label="返回" onClick={() => ctx && ui.openCtx(ctx.type, ctx.id, ctx.clientX, ctx.clientY)}>
          <Glyph name="chev-left" />
        </button>
        <span>移动到…</span>
      </div>
      {containers.map((c) => (
        <button
          className="ctx-item"
          key={c.id || 'root'}
          onClick={() => {
            ui.closeCtx()
            void catalog.moveApp(appId, c.id || null, null).then((ok) => {
              if (ok) ui.pushToast('已移动', { icon: 'move' })
            })
          }}
        >
          <Glyph name={c.id ? (c.depth ? 'layers' : 'folder') : 'grid'} />
          <span>{c.label}</span>
        </button>
      ))}
    </>
  )
}