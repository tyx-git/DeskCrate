// 目录树渲染与内部拖拽排序：落点经 dropPlan 映射成后端命令，不可表达时拒绝。
// 分类不支持拖拽（后端无根级重排命令）；分组仅同级重排，应用可跨容器移动。
import { useRef, useState, useEffect, type RefObject } from 'react'

import {
  planAppMove,
  planGroupMove,
  type AppDropTarget,
  type GroupDropTarget,
  type MovePlan,
} from '../lib/dropPlan'
import { countAppsIn, locateNode, nodeList, type ContainerNode } from '../lib/tree'
import { useCatalogStore } from '../state/catalogStore'
import { useUiStore, type DragState } from '../state/uiStore'
import { isAppItem, isAppsRoot, isGroupItem, type AppEXE } from '../types/catalog'
import { AppPlate } from './AppPlate'
import { Glyph } from './Glyph'
import { SearchResults } from './SearchResults'

interface Hit {
  target: AppDropTarget | GroupDropTarget
  el: HTMLElement
}

function rootIndexOf(el: HTMLElement, fallback: number): number {
  return el.dataset.rootIndex !== undefined ? Number(el.dataset.rootIndex) : fallback
}

function computeHit(e: React.DragEvent, drag: DragState, catalog: HTMLElement): Hit | null {
  const t = e.target as HTMLElement
  const data = useCatalogStore.getState().data

  if (drag.kind === 'app') {
    const appEl = t.closest<HTMLElement>('.app')
    const rowEl = t.closest<HTMLElement>('.row')
    const gridEl = t.closest<HTMLElement>('.icon-grid')
    if (appEl?.dataset.app && appEl.dataset.app !== drag.id) {
      const r = appEl.getBoundingClientRect()
      const side = e.clientX < r.left + r.width / 2 ? 'before' : 'after'
      return { target: { type: side, refAppId: appEl.dataset.app }, el: appEl }
    }
    if (rowEl?.dataset.id) {
      return { target: { type: 'into', parentId: rowEl.dataset.id }, el: rowEl }
    }
    if (gridEl) {
      const parentId = gridEl.dataset.container || null
      const rootIndex = gridEl.dataset.rootIndex !== undefined ? Number(gridEl.dataset.rootIndex) : undefined
      return { target: { type: 'into', parentId, rootIndex }, el: gridEl }
    }
    const roots = Array.from(catalog.querySelectorAll<HTMLElement>(':scope > .node'))
    for (let i = 0; i < roots.length; i++) {
      const r = roots[i].getBoundingClientRect()
      if (e.clientY < r.top + r.height / 2) {
        return { target: { type: 'root', rootIndex: rootIndexOf(roots[i], i), side: 'before' }, el: roots[i] }
      }
    }
    if (roots.length > 0) {
      const last = roots[roots.length - 1]
      return { target: { type: 'root', rootIndex: rootIndexOf(last, roots.length - 1), side: 'after' }, el: last }
    }
    return { target: { type: 'root', rootIndex: data.items.length, side: 'before' }, el: catalog }
  }

  const nodeEl = t.closest<HTMLElement>('.node[data-kind="Group"]')
  if (!nodeEl?.dataset.node || nodeEl.dataset.node === drag.id) return null
  const loc = locateNode(data, drag.id)
  const ref = locateNode(data, nodeEl.dataset.node)
  if (!loc || !ref || loc.parentId !== ref.parentId) return null
  const r = nodeEl.getBoundingClientRect()
  const side = e.clientY < r.top + r.height / 2 ? 'before' : 'after'
  return { target: { type: 'sibling', refNodeId: nodeEl.dataset.node, side }, el: nodeEl }
}

function clearMarks(catalog: HTMLElement): void {
  catalog
    .querySelectorAll('.drop-before, .drop-after, .drop-into')
    .forEach((el) => el.classList.remove('drop-before', 'drop-after', 'drop-into'))
}

function markHit(hit: Hit): void {
  const { target, el } = hit
  if (target.type === 'into') {
    el.classList.add('drop-into')
    return
  }
  const side = target.type === 'root' || target.type === 'sibling' ? target.side : target.type
  el.classList.add(side === 'before' ? 'drop-before' : 'drop-after')
}

export function Catalog() {
  const data = useCatalogStore((s) => s.data)
  const query = useUiStore((s) => s.query)
  const catalogRef = useRef<HTMLDivElement>(null)
  const dropRef = useRef<MovePlan | null>(null)

  const onDragStart = (e: React.DragEvent) => {
    const t = e.target as HTMLElement
    const appEl = t.closest<HTMLElement>('.app')
    const rowEl = t.closest<HTMLElement>('.row')
    let drag: DragState
    if (appEl?.dataset.app) {
      drag = { kind: 'app', id: appEl.dataset.app }
    } else if (rowEl?.dataset.id) {
      const nodeEl = rowEl.closest<HTMLElement>('.node')
      if (nodeEl?.dataset.kind !== 'Group' || !nodeEl.dataset.node) {
        e.preventDefault()
        return
      }
      drag = { kind: 'Group', id: nodeEl.dataset.node }
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
    const plan = hit
      ? drag.kind === 'app'
        ? planAppMove(data, drag.id, hit.target as AppDropTarget)
        : planGroupMove(data, drag.id, hit.target as GroupDropTarget)
      : null
    e.dataTransfer.dropEffect = plan ? 'move' : 'none'
    clearMarks(catalog)
    if (plan && hit) markHit(hit)
    dropRef.current = plan
  }

  const onDrop = (e: React.DragEvent) => {
    e.preventDefault()
    const catalog = catalogRef.current
    const drag = useUiStore.getState().drag
    const plan = dropRef.current
    if (catalog) clearMarks(catalog)
    dropRef.current = null
    if (drag && plan) {
      const cat = useCatalogStore.getState()
      if (drag.kind === 'app') void cat.moveApp(drag.id, plan.parentId, plan.beforeId)
      else if (plan.parentId !== null) void cat.moveGroup(drag.id, plan.parentId, plan.beforeId)
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
      role="button"
      tabIndex={0}
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
  const dragging = drag?.kind === 'Group' && drag.id === node.id

  return (
    <div
      className={`node${node.isCollapsed ? ' is-collapsed' : ''}`}
      data-node={node.id}
      data-kind={kind}
      data-root-index={rootIndex}
    >
      <div
        className={`row ${kind === 'Category' ? 'cat-row' : 'grp-row'}${dragging ? ' is-dragging' : ''}`}
        role="button"
        tabIndex={0}
        data-id={node.id}
        data-drop-id={node.id}
        aria-expanded={!node.isCollapsed}
        draggable={kind === 'Group' && !isRenaming}
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