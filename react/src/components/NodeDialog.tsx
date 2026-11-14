// 面板内对话框：添加应用、新建分类/分组、重命名应用与删除确认。
import { useEffect, useRef, useState } from 'react'

import {
  countAppsIn,
  flattenContainers,
  locateApp,
  locateNode,
  MAX_GROUP_DEPTH,
  nodeDepth,
  nodeList,
} from '../lib/tree'
import { useCatalogStore } from '../state/catalogStore'
import { useUiStore, type DialogState } from '../state/uiStore'

export function NodeDialog() {
  const dialog = useUiStore((s) => s.dialog)
  const closeDialog = useUiStore((s) => s.closeDialog)
  if (!dialog) return null
  return (
    <>
      <div className="scrim" onClick={closeDialog} />
      <div className="dialog" role="dialog" aria-modal="true">
        <DialogForm key={dialogKey(dialog)} dialog={dialog} />
      </div>
    </>
  )
}

function dialogKey(d: DialogState): string {
  if (d.mode === 'new-group') return `new-group:${d.parentId}`
  if (d.mode === 'rename-app' || d.mode === 'delete-node') return `${d.mode}:${d.id}`
  return d.mode
}

function DialogForm({ dialog }: { dialog: DialogState }) {
  const data = useCatalogStore((s) => s.data)
  const closeDialog = useUiStore((s) => s.closeDialog)
  const ui = useUiStore.getState()
  const catalog = useCatalogStore.getState()

  const containers = flattenContainers(data)
  const [source, setSource] = useState(dialog.mode === 'add' ? (dialog.source ?? '') : '')
  const [name, setName] = useState(initialName(dialog, data))
  const [parent, setParent] = useState(dialog.mode === 'add' ? (dialog.parentId ?? '') : '')
  const [error, setError] = useState('')
  const firstRef = useRef<HTMLTextAreaElement | HTMLInputElement>(null)

  useEffect(() => {
    firstRef.current?.focus()
    if (dialog.mode !== 'add' && firstRef.current instanceof HTMLInputElement) firstRef.current.select()
  }, [dialog.mode])

  const fail = (msg: string) => setError(msg)

  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError('')
    if (dialog.mode === 'add') {
      const lines = source.split('\n').map((s) => s.trim()).filter(Boolean)
      if (lines.length === 0) return fail('来源路径或网址不能为空')
      const trimmedName = name.trim()
      if (trimmedName && lines.length > 1) return fail('批量导入时不能指定统一名称，请留空')
      const parentId = parent || null
      const res = await catalog.addApps(parentId, lines, dialog.rootIndex)
      if (trimmedName && lines.length === 1 && res.added.length === 1) {
        await catalog.renameApp(res.added[0].id, trimmedName)
      }
      if (res.failed.length > 0) {
        setSource(res.failed.map((f) => f.sourcePath).join('\n'))
        fail(
          `已添加 ${res.added.length} 个，${res.failed.length} 个失败：` +
            res.failed.map((f) => `${f.sourcePath}（${f.message}）`).join('；'),
        )
        if (res.added.length === 0) return
      } else {
        closeDialog()
      }
      if (res.added.length > 0) ui.pushToast(`已添加 ${res.added.length} 个应用`, { icon: 'plus' })
      return
    }

    if (dialog.mode === 'new-category') {
      const trimmed = name.trim()
      if (!trimmed) return fail('分类名称不能为空')
      const created = await catalog.addCategory(trimmed)
      if (!created) return
      closeDialog()
      ui.pushToast(`已创建分类「${trimmed}」`, { icon: 'grid' })
      return
    }

    if (dialog.mode === 'new-group') {
      const trimmed = name.trim()
      if (!trimmed) return fail('分组名称不能为空')
      if (nodeDepth(data, dialog.parentId) >= MAX_GROUP_DEPTH) return fail(`分组最多嵌套 ${MAX_GROUP_DEPTH} 层`)
      const created = await catalog.addGroup(dialog.parentId, trimmed)
      if (!created) return
      closeDialog()
      ui.pushToast(`已创建分组「${trimmed}」`, { icon: 'layers' })
      return
    }

    if (dialog.mode === 'rename-app') {
      const trimmed = name.trim()
      if (!trimmed) return fail('应用名称不能为空')
      const ok = await catalog.renameApp(dialog.id, trimmed)
      if (ok) closeDialog()
      return
    }

    if (dialog.mode === 'delete-node') {
      const loc = locateNode(data, dialog.id)
      if (!loc) return closeDialog()
      const ok =
        loc.kind === 'Category'
          ? await catalog.deleteCategory(dialog.id)
          : await catalog.deleteGroup(dialog.id)
      if (!ok) return
      closeDialog()
      ui.pushToast(`已删除「${loc.node.name}」及 ${countAppsIn(nodeList(loc.node))} 个项目`, { icon: 'bin' })
      return
    }
  }

  const { title, submitLabel } = dialogMeta(dialog, data)

  return (
    <form onSubmit={submit}>
      <h2 className="dialog-title">{title}</h2>
      <div className="dialog-fields">
        {dialog.mode === 'add' && (
          <>
            <label className="field">
              <span className="field-label">来源路径或网址</span>
              <textarea
                ref={firstRef as React.RefObject<HTMLTextAreaElement>}
                className="field-input"
                rows={3}
                placeholder={'C:\\Program Files\\App\\app.exe\nhttps://example.com'}
                value={source}
                onChange={(e) => setSource(e.target.value)}
              />
              <span className="field-hint">每行一个，可批量导入；网址图标会在后台补齐</span>
            </label>
            <label className="field">
              <span className="field-label">显示名称（可选）</span>
              <input
                className="field-input"
                type="text"
                placeholder="留空时自动推导"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </label>
            <label className="field">
              <span className="field-label">放入</span>
              <select className="field-input" value={parent} onChange={(e) => setParent(e.target.value)}>
                {containers.map((c) => (
                  <option key={c.id || 'root'} value={c.id}>
                    {' '.repeat(c.depth)}
                    {c.label}
                  </option>
                ))}
              </select>
            </label>
          </>
        )}

        {(dialog.mode === 'new-category' || dialog.mode === 'new-group' || dialog.mode === 'rename-app') && (
          <label className="field">
            <span className="field-label">
              {dialog.mode === 'new-category' ? '分类名称' : dialog.mode === 'new-group' ? '分组名称' : '显示名称'}
            </span>
            <input
              ref={firstRef as React.RefObject<HTMLInputElement>}
              className="field-input"
              type="text"
              placeholder={dialog.mode === 'new-category' ? '例如：开发工具' : dialog.mode === 'new-group' ? '例如：前端' : undefined}
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            {dialog.mode === 'new-group' && (
              <span className="field-hint">父级：{locateNode(data, dialog.parentId)?.node.name ?? '顶层'}</span>
            )}
          </label>
        )}

        {dialog.mode === 'delete-node' && <DeleteHint id={dialog.id} />}
      </div>
      {error && <p className="dialog-error">{error}</p>}
      <div className="dialog-actions">
        <button type="button" className="btn" onClick={closeDialog}>
          取消
        </button>
        <button type="submit" className={`btn${dialog.mode === 'delete-node' ? '' : ' btn-accent'}`}>
          {submitLabel}
        </button>
      </div>
    </form>
  )
}

function DeleteHint({ id }: { id: string }) {
  const data = useCatalogStore((s) => s.data)
  const loc = locateNode(data, id)
  if (!loc) return null
  const count = countAppsIn(nodeList(loc.node))
  return (
    <p className="field-hint" style={{ fontSize: 'var(--t-desc)' }}>
      将删除{loc.kind === 'Category' ? '分类' : '分组'}「{loc.node.name}」及其中的 {count} 个应用，此操作无法撤销。
    </p>
  )
}

function initialName(dialog: DialogState, data: ReturnType<typeof useCatalogStore.getState>['data']): string {
  if (dialog.mode === 'add') return dialog.name ?? ''
  if (dialog.mode === 'rename-app') return locateApp(data, dialog.id)?.app.name ?? ''
  return ''
}

function dialogMeta(dialog: DialogState, data: ReturnType<typeof useCatalogStore.getState>['data']): {
  title: string
  submitLabel: string
} {
  switch (dialog.mode) {
    case 'add':
      return { title: '添加应用', submitLabel: '添加' }
    case 'new-category':
      return { title: '新建分类', submitLabel: '创建' }
    case 'new-group':
      return { title: '新建分组', submitLabel: '创建' }
    case 'rename-app':
      return { title: '重命名应用', submitLabel: '保存' }
    case 'delete-node': {
      const kind = locateNode(data, dialog.id)?.kind
      return { title: kind === 'Category' ? '删除分类' : '删除分组', submitLabel: '删除' }
    }
  }
}