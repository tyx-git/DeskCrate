// 目录树渲染与拖拽：前端只负责命中 DOM 落点，移动逻辑由后端 drop_* 命令完成。
// 应用可跨容器移动；分组仅同父级重排；顶层分类只在根列表内重排。
import { useRef, useState, useEffect, type RefObject } from 'react'

import { countAppsIn, nodeList, type ContainerNode } from '../lib/tree'
import { useCatalogStore } from '../state/catalogStore'
import { useUiStore, type DragState } from '../state/uiStore'
import { isAppItem, isAppsRoot, isGroupItem, type AppEXE } from '../types/catalog'
import type { DropSide, DropTarget } from '../types/drop'
import { AppPlate } from './AppPlate'
import { Glyph } from './Glyph'
import { SearchResults } from './SearchResults'

interface Hit {
  target: DropTarget
  el: HTMLElement
}

interface PointerDrag {
  drag: DragState
  pointerId: number
  startX: number
  startY: number
  source: HTMLElement
  captureTarget: HTMLElement
  ghost: HTMLElement | null
  active: boolean
}

function createDragGhost(source: HTMLElement): HTMLElement {
  const isNode = source.classList.contains('node')
  const row = isNode ? source.querySelector<HTMLElement>('.row') : null
  const rect = (row ?? source).getBoundingClientRect()
  const ghost = source.cloneNode(true) as HTMLElement
  ghost.classList.add('drag-ghost')
  ghost.querySelector('.node-body')?.remove()
  ghost.style.width = `${rect.width}px`
  ghost.style.height = `${rect.height}px`
  document.body.appendChild(ghost)
  return ghost
}

function rootIndexOf(el: HTMLElement, fallback: number): number {
  return el.dataset.rootIndex !== undefined ? Number(el.dataset.rootIndex) : fallback
}

function appSide(clientX: number, el: HTMLElement): DropSide {
  const rect = el.getBoundingClientRect()
  return clientX < rect.left + rect.width / 2 ? 'before' : 'after'
}

function nodeSide(clientY: number, el: HTMLElement): DropSide {
  const rect = el.getBoundingClientRect()
  return clientY < rect.top + rect.height / 2 ? 'before' : 'after'
}

function nearestAppInGrid(grid: HTMLElement, clientX: number, clientY: number): HTMLElement | null {
  let nearest: HTMLElement | null = null
  let distance = Infinity
  for (const app of grid.querySelectorAll<HTMLElement>('.app[data-app]')) {
    const rect = app.getBoundingClientRect()
    const dx = clientX - (rect.left + rect.width / 2)
    const dy = clientY - (rect.top + rect.height / 2)
    const next = dx * dx + dy * dy
    if (next < distance) {
      distance = next
      nearest = app
    }
  }
  return nearest
}

function computeHit(clientX: number, clientY: number, drag: DragState, catalog: HTMLElement): Hit | null {
  const target = document.elementFromPoint(clientX, clientY)
  if (!(target instanceof HTMLElement) || !catalog.contains(target)) return null

  if (drag.kind === 'Group') {
    const rowEl = target.closest<HTMLElement>('.row[data-id]')
    const nodeEl = rowEl?.closest<HTMLElement>('.node[data-kind="Group"]')
    if (!rowEl || !nodeEl?.dataset.node || nodeEl.dataset.node === drag.id) return null
    return {
      target: { type: 'item', id: nodeEl.dataset.node, side: nodeSide(clientY, rowEl) },
      el: nodeEl,
    }
  }

  if (drag.kind === 'Category') {
    return rootHit(clientY, catalog)
  }

  const appEl = target.closest<HTMLElement>('.app[data-app]')
  if (appEl?.dataset.app) {
    if (appEl.dataset.app === drag.id) return null
    return {
      target: { type: 'item', id: appEl.dataset.app, side: appSide(clientX, appEl) },
      el: appEl,
    }
  }

  const gridEl = target.closest<HTMLElement>('.icon-grid')
  if (gridEl) {
    const nearest = nearestAppInGrid(gridEl, clientX, clientY)
    if (nearest?.dataset.app && nearest.dataset.app !== drag.id) {
      return {
        target: { type: 'item', id: nearest.dataset.app, side: appSide(clientX, nearest) },
        el: nearest,
      }
    }
    const containerId = gridEl.dataset.container
    if (containerId) {
      return { target: { type: 'container', id: containerId }, el: gridEl }
    }
    return {
      target: { type: 'root', index: rootIndexOf(gridEl, 0), side: 'before' },
      el: gridEl,
    }
  }

  const rowEl = target.closest<HTMLElement>('.row[data-id]')
  if (rowEl?.dataset.id) {
    return { target: { type: 'container', id: rowEl.dataset.id }, el: rowEl }
  }

  return rootHit(clientY, catalog)
}

function rootHit(clientY: number, catalog: HTMLElement): Hit | null {
  const roots = Array.from(catalog.querySelectorAll<HTMLElement>(':scope > .node'))
  for (let i = 0; i < roots.length; i++) {
    const target = roots[i]
    const rect = target.getBoundingClientRect()
    if (clientY < rect.top + rect.height / 2) {
      return {
        target: { type: 'root', index: rootIndexOf(target, i), side: 'before' },
        el: target,
      }
    }
  }

  if (roots.length > 0) {
    const last = roots[roots.length - 1]
    return {
      target: { type: 'root', index: rootIndexOf(last, roots.length - 1), side: 'after' },
      el: last,
    }
  }

  return { target: { type: 'rootEnd' }, el: catalog }
}

function clearMarks(catalog: HTMLElement): void {
  catalog
    .querySelectorAll('.drop-before, .drop-after, .drop-into')
    .forEach((el) => el.classList.remove('drop-before', 'drop-after', 'drop-into'))
}

function markHit(hit: Hit): void {
  const { target, el } = hit

  if (target.type === 'container') {
    el.classList.add('drop-into')
    return
  }

  if (target.type === 'rootEnd') {
    if (el !== el.ownerDocument.body) el.classList.add('drop-into')
    return
  }

  el.classList.add(target.side === 'before' ? 'drop-before' : 'drop-after')
}

export function Catalog() {
  const data = useCatalogStore((s) => s.data)
  const query = useUiStore((s) => s.query)
  const catalogRef = useRef<HTMLDivElement>(null)
  const dropRef = useRef<DropTarget | null>(null)
  const pointerRef = useRef<PointerDrag | null>(null)
  const suppressClickRef = useRef(false)

  useEffect(() => {
    const finish = (event: PointerEvent, dropped: boolean) => {
      const pointer = pointerRef.current
      if (!pointer || event.pointerId !== pointer.pointerId) return
      pointerRef.current = null

      const catalog = catalogRef.current
      if (catalog) clearMarks(catalog)
      pointer.ghost?.remove()

      try {
        if (pointer.captureTarget.hasPointerCapture(pointer.pointerId)) {
          pointer.captureTarget.releasePointerCapture(pointer.pointerId)
        }
      } catch {
        // capture may already be released when the source unmounts
      }

      if (pointer.active) {
        if (dropped) {
          const target = dropRef.current
          if (target) void useCatalogStore.getState().dropItem(pointer.drag.id, target)
        }
        useUiStore.getState().endDrag()
        globalThis.setTimeout(() => {
          suppressClickRef.current = false
        }, 0)
      }
      dropRef.current = null
    }

    const onPointerMove = (event: PointerEvent) => {
      const pointer = pointerRef.current
      if (!pointer || event.pointerId !== pointer.pointerId) return

      if (!pointer.active) {
        const dx = event.clientX - pointer.startX
        const dy = event.clientY - pointer.startY
        if (Math.hypot(dx, dy) < 4) return
        pointer.active = true
        suppressClickRef.current = true
        useUiStore.getState().startDrag(pointer.drag)
        pointer.ghost = createDragGhost(pointer.source)
      }

      event.preventDefault()
      if (pointer.ghost) {
        pointer.ghost.style.left = `${event.clientX + 12}px`
        pointer.ghost.style.top = `${event.clientY + 12}px`
      }

      const catalog = catalogRef.current
      if (!catalog) return
      const hit = computeHit(event.clientX, event.clientY, pointer.drag, catalog)
      clearMarks(catalog)
      if (hit) markHit(hit)
      dropRef.current = hit?.target ?? null

      const box = catalog.getBoundingClientRect()
      if (event.clientY < box.top + 24) catalog.scrollTop -= 10
      else if (event.clientY > box.bottom - 24) catalog.scrollTop += 10
    }

    const onPointerUp = (event: PointerEvent) => finish(event, true)
    const onPointerCancel = (event: PointerEvent) => finish(event, false)

    window.addEventListener('pointermove', onPointerMove, { passive: false })
    window.addEventListener('pointerup', onPointerUp)
    window.addEventListener('pointercancel', onPointerCancel)

    return () => {
      window.removeEventListener('pointermove', onPointerMove)
      window.removeEventListener('pointerup', onPointerUp)
      window.removeEventListener('pointercancel', onPointerCancel)
      pointerRef.current?.ghost?.remove()
      if (pointerRef.current?.active) useUiStore.getState().endDrag()
      pointerRef.current = null
    }
  }, [])

  const onPointerDown = (event: React.PointerEvent) => {
    if (event.button !== 0) return
    const target = event.target as HTMLElement
    if (target.closest('button, input, textarea, select, .rename-input, [data-no-drag]')) return

    const appEl = target.closest<HTMLElement>('.app[data-app]')
    const rowEl = appEl ? null : target.closest<HTMLElement>('.row[data-id]')
    const nodeEl = rowEl?.closest<HTMLElement>('.node[data-kind]')

    let drag: DragState | null = null
    if (appEl?.dataset.app) {
      drag = { kind: 'app', id: appEl.dataset.app }
    } else {
      const kind = nodeEl?.dataset.kind
      if (nodeEl?.dataset.node && (kind === 'Group' || kind === 'Category')) {
        drag = { kind, id: nodeEl.dataset.node }
      }
    }
    if (!drag) return

    dropRef.current = null
    suppressClickRef.current = false
    const source = (appEl ?? nodeEl) as HTMLElement
    const captureTarget = (appEl ?? rowEl) as HTMLElement
    pointerRef.current = {
      drag,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      source,
      captureTarget,
      ghost: null,
      active: false,
    }
    try {
      captureTarget.setPointerCapture(event.pointerId)
    } catch {
      // pointer capture is best-effort; window listeners still handle the drag
    }
  }

  const onClickCapture = (event: React.MouseEvent) => {
    if (!suppressClickRef.current) return
    suppressClickRef.current = false
    event.preventDefault()
    event.stopPropagation()
  }

  return (
    <div
      className="catalog dc-scroll"
      ref={catalogRef}
      tabIndex={-1}
      onPointerDown={onPointerDown}
      onClickCapture={onClickCapture}
      onScroll={() => useUiStore.getState().closeCtx()}
    >
      {query.trim() ? (
        <SearchResults />
      ) : (
        data.items.map((root, ri) =>
          isAppsRoot(root) ? (
            <div className="node root-apps" data-root-index={ri} key={`apps-${ri}`}>
              <AppGrid apps={root.Apps} parentId="" rootIndex={ri} />
            </div>
          ) : (
            <Node key={root.Category.id} node={root.Category} kind="Category" depth={0} rootIndex={ri} />
          ),
        )
      )}
    </div>
  )
}

function AppGrid({
  apps,
  parentId,
  indent = 0,
  rootIndex,
}: {
  apps: AppEXE[]
  parentId: string
  indent?: number
  rootIndex?: number
}) {
  return (
    <div
      className="icon-grid"
      data-container={parentId}
      data-drop-id={parentId}
      data-root-index={rootIndex}
      style={indent ? { paddingLeft: `calc(var(--pad-x) + ${indent}px)` } : undefined}
    >
      {apps.map((app) => (
        <AppCell key={app.id} app={app} />
      ))}
    </div>
  )
}

function AppCell({ app }: { app: AppEXE }) {
  const drag = useUiStore((s) => s.drag)
  const launchingId = useUiStore((s) => s.launchingId)
  const launchApp = useCatalogStore((s) => s.launchApp)
  const dragging = drag?.kind === 'app' && drag.id === app.id
  const launching = launchingId === app.id

  return (
    <div
      className={`app${dragging ? ' is-dragging' : ''}${launching ? ' is-launching' : ''}`}
      data-app={app.id}
      title={`${app.name}\n${app.sourcePath}`}
      onClick={() => void launchApp(app)}
    >
      <AppPlate app={app} />
      <span className="app-name">{app.name}</span>
    </div>
  )
}

function Node({
  node,
  kind,
  depth,
  rootIndex,
}: {
  node: ContainerNode
  kind: 'Category' | 'Group'
  depth: number
  rootIndex?: number
}) {
  const drag = useUiStore((s) => s.drag)
  const renamingId = useUiStore((s) => s.renamingId)
  const startRename = useUiStore((s) => s.startRename)
  const openCtx = useUiStore((s) => s.openCtx)
  const toggleCollapse = useCatalogStore((s) => s.toggleCollapse)

  const list = nodeList(node)
  const count = countAppsIn(list)
  const indent = kind === 'Group' ? 14 + (depth - 1) * 12 : 0
  const looseApps = list.filter(isAppItem).map((it) => it.App)
  const subGroups = list.filter(isGroupItem).map((it) => it.Group)
  const isRenaming = renamingId === node.id
  const dragging = drag?.id === node.id && drag.kind === kind

  return (
    <div
      className={`node${node.isCollapsed ? ' is-collapsed' : ''}`}
      data-node={node.id}
      data-kind={kind}
      data-root-index={rootIndex}
    >
      <div
        className={`row ${kind === 'Category' ? 'cat-row' : 'grp-row'}${dragging ? ' is-dragging' : ''}`}
        data-id={node.id}
        data-drop-id={node.id}
        style={indent ? { paddingLeft: `calc(var(--pad-x) + ${indent}px)` } : undefined}
        onClick={() => void toggleCollapse(node.id, !node.isCollapsed)}
      >
        <span className="chev">
          <Glyph name="chev-down" />
        </span>
        {isRenaming ? (
          <RenameInput node={node} kind={kind} />
        ) : (
          <span className="row-name" onDoubleClick={() => startRename(node.id)}>
            {node.name}
          </span>
        )}
        <span className="count">{count}</span>
        <button
          className="icon-btn row-more"
          aria-label={`管理${node.name}`}
          onClick={(e) => {
            e.stopPropagation()
            const r = e.currentTarget.getBoundingClientRect()
            openCtx(kind, node.id, r.left, r.bottom + 4)
          }}
        >
          <Glyph name="dots" />
        </button>
      </div>
      <div className="node-body">
        <div className="node-inner">
          {looseApps.length > 0 && (
            <AppGrid apps={looseApps} parentId={node.id} indent={kind === 'Group' ? indent : 0} />
          )}
          {subGroups.map((g) => (
            <Node key={g.id} node={g} kind="Group" depth={depth + 1} />
          ))}
        </div>
      </div>
    </div>
  )
}

function RenameInput({ node, kind }: { node: ContainerNode; kind: 'Category' | 'Group' }) {
  const endRename = useUiStore((s) => s.endRename)
  const notify = useUiStore((s) => s.notify)
  const renameCategory = useCatalogStore((s) => s.renameCategory)
  const renameGroup = useCatalogStore((s) => s.renameGroup)
  const [value, setValue] = useState(node.name)
  const inputRef = useRef<HTMLInputElement>(null)
  const doneRef = useRef(false)

  useEffect(() => {
    inputRef.current?.focus()
    inputRef.current?.select()
  }, [])

  const commit = async () => {
    if (doneRef.current) return
    const name = value.trim()
    if (!name) {
      notify('error', '名称不能为空')
      inputRef.current?.focus()
      return
    }
    doneRef.current = true
    endRename()
    if (kind === 'Category') await renameCategory(node.id, name)
    else await renameGroup(node.id, name)
  }
  const cancel = () => {
    if (doneRef.current) return
    doneRef.current = true
    endRename()
  }

  return (
    <input
      ref={inputRef as RefObject<HTMLInputElement>}
      className="rename-input"
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        e.stopPropagation()
        if (e.key === 'Enter') void commit()
        else if (e.key === 'Escape') cancel()
      }}
      onBlur={() => void commit()}
    />
  )
}
