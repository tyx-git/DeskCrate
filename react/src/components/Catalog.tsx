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

function rootIndexOf(el: HTMLElement, fallback: number): number {
  return el.dataset.rootIndex !== undefined ? Number(el.dataset.rootIndex) : fallback
}

function appSide(e: React.DragEvent, el: HTMLElement): DropSide {
  const rect = el.getBoundingClientRect()
  return e.clientX < rect.left + rect.width / 2 ? 'before' : 'after'
}

function nodeSide(e: React.DragEvent, el: HTMLElement): DropSide {
  const rect = el.getBoundingClientRect()
  return e.clientY < rect.top + rect.height / 2 ? 'before' : 'after'
}

function computeHit(e: React.DragEvent, drag: DragState, catalog: HTMLElement): Hit | null {
  const target = e.target as HTMLElement
  const data = useCatalogStore.getState().data

  if (drag.kind === 'app') {
    const appEl = target.closest<HTMLElement>('.app')
    if (appEl?.dataset.app) {
      if (appEl.dataset.app === drag.id) return null
      return {
        target: { type: 'app', id: appEl.dataset.app, side: appSide(e, appEl) },
        el: appEl,
      }
    }

    const gridEl = target.closest<HTMLElement>('.icon-grid')
    if (gridEl) {
      const containerId = gridEl.dataset.container
      if (containerId) {
        return { target: { type: 'container', id: containerId }, el: gridEl }
      }
      return {
        target: { type: 'root', index: rootIndexOf(gridEl, data.items.length), side: 'before' },
        el: gridEl,
      }
    }

    const rowEl = target.closest<HTMLElement>('.row')
    if (rowEl?.dataset.id) {
      return { target: { type: 'container', id: rowEl.dataset.id }, el: rowEl }
    }

    return rootHit(e, catalog)
  }

  if (drag.kind === 'Group') {
    const nodeEl = target.closest<HTMLElement>('.node[data-kind="Group"]')
    if (!nodeEl?.dataset.node || nodeEl.dataset.node === drag.id) return null
    return {
      target: { type: 'container', id: nodeEl.dataset.node, side: nodeSide(e, nodeEl) },
      el: nodeEl,
    }
  }

  if (drag.kind === 'Category') {
    return rootHit(e, catalog)
  }

  return null
}

function rootHit(e: React.DragEvent, catalog: HTMLElement): Hit | null {
  const roots = Array.from(catalog.querySelectorAll<HTMLElement>(':scope > .node'))
  for (let i = 0; i < roots.length; i++) {
    const target = roots[i]
    const rect = target.getBoundingClientRect()
    if (e.clientY < rect.top + rect.height / 2) {
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
    if (target.side) {
      el.classList.add(target.side === 'before' ? 'drop-before' : 'drop-after')
    } else {
      el.classList.add('drop-into')
    }
    return
  }

  if (target.type === 'rootEnd') {
    el.classList.add('drop-into')
    return
  }

  el.classList.add(target.side === 'before' ? 'drop-before' : 'drop-after')
}

export function Catalog() {
  const data = useCatalogStore((s) => s.data)
  const query = useUiStore((s) => s.query)
  const catalogRef = useRef<HTMLDivElement>(null)
  const dropRef = useRef<DropTarget | null>(null)

  const onDragStart = (e: React.DragEvent) => {
    const target = e.target as HTMLElement
    const appEl = target.closest<HTMLElement>('.app')
    const nodeEl = target.closest<HTMLElement>('.node')

    let drag: DragState
    if (appEl?.dataset.app) {
      drag = { kind: 'app', id: appEl.dataset.app }
    } else if (nodeEl?.dataset.node && (nodeEl.dataset.kind === 'Group' || nodeEl.dataset.kind === 'Category')) {
      drag = { kind: nodeEl.dataset.kind, id: nodeEl.dataset.node }
    } else {
      e.preventDefault()
      return
    }

    e.dataTransfer.effectAllowed = 'move'
    try {
      e.dataTransfer.setData('text/plain', drag.id)
    } catch {
      /* 某些环境不允许 setData，忽略 */
    }
    useUiStore.getState().startDrag(drag)
  }

  const onDragOver = (e: React.DragEvent) => {
    const drag = useUiStore.getState().drag
    const catalog = catalogRef.current
    if (!drag || !catalog) return
    e.preventDefault()

    const hit = computeHit(e, drag, catalog)
    clearMarks(catalog)
    if (hit) {
      e.dataTransfer.dropEffect = 'move'
      markHit(hit)
    } else {
      e.dataTransfer.dropEffect = 'none'
    }
    dropRef.current = hit?.target ?? null
  }

  const onDrop = (e: React.DragEvent) => {
    e.preventDefault()
    const catalog = catalogRef.current
    const drag = useUiStore.getState().drag
    const target = dropRef.current
    if (catalog) clearMarks(catalog)
    dropRef.current = null

    if (drag && target) {
      const catalogStore = useCatalogStore.getState()
      if (drag.kind === 'app') {
        void catalogStore.dropApp(drag.id, target)
      } else if (drag.kind === 'Group' && target.type === 'container' && target.side) {
        void catalogStore.dropGroup(drag.id, target)
      } else if (drag.kind === 'Category' && (target.type === 'root' || target.type === 'rootEnd')) {
        void catalogStore.dropCategory(drag.id, target)
      }
    }

    useUiStore.getState().endDrag()
  }

  const onDragEnd = () => {
    const catalog = catalogRef.current
    if (catalog) clearMarks(catalog)
    dropRef.current = null
    useUiStore.getState().endDrag()
  }

  return (
    <div
      className="catalog dc-scroll"
      ref={catalogRef}
      tabIndex={-1}
      onDragStart={onDragStart}
      onDragOver={onDragOver}
      onDrop={onDrop}
      onDragEnd={onDragEnd}
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
      draggable
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
        draggable={!isRenaming}
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
