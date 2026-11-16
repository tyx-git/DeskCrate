// 面板内对话框：添加/编辑应用、新建分类/分组与删除确认。
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
  if (d.mode === 'edit-app' || d.mode === 'delete-node') return `${d.mode}:${d.id}`
  return d.mode
}

function DialogForm({ dialog }: { dialog: DialogState }) {
  const data = useCatalogStore((s) => s.data)
  const closeDialog = useUiStore((s) => s.closeDialog)
  const ui = useUiStore.getState()
  const catalog = useCatalogStore.getState()

  const containers = flattenContainers(data)
  const app = dialog.mode === 'edit-app' ? locateApp(data, dialog.id)?.app : undefined
  const [source, setSource] = useState(
    dialog.mode === 'add' ? (dialog.source ?? '') : (app?.sourcePath ?? ''),
  )
  const [name, setName] = useState(
    dialog.mode === 'add' ? (dialog.name ?? '') : (app?.name ?? ''),
  )
  const [argumentsText, setArgumentsText] = useState(app?.arguments ?? '')
  const [workingDir, setWorkingDir] = useState(app?.workingDir ?? '')
  const [parent, setParent] = useState(dialog.mode === 'add' ? (dialog.parentId ?? '') : '')
  const [error, setError] = useState('')
  const firstRef = useRef<HTMLInputElement | HTMLTextAreaElement>(null)

  useEffect(() => {
    firstRef.current?.focus()
    if (dialog.mode !== 'add' && firstRef.current instanceof HTMLInputElement) firstRef.current.select()
  }, [dialog.mode])

  const fail = (msg: string) => setError(msg)
  const optional = (value: string) => value.trim() || undefined

  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError('')

    if (dialog.mode === 'add') {
      const lines = source.split('\n').map((s) => s.trim()).filter(Boolean)
      if (lines.length === 0) return fail('来源路径或网址不能为空')
      const trimmedName = name.trim()
      if (trimmedName && lines.length > 1) return fail('批量导入时不能指定统一名称，请留空')
      const parentId = parent || null
      const args = optional(argumentsText)
      const directory = optional(workingDir)

      if (lines.length === 1) {
        const created = await catalog.addApp(
          parentId,
          lines[0],
          trimmedName || undefined,
          args,
          directory,
          dialog.rootIndex,
        )
        if (!created) return
        closeDialog()
        ui.pushToast(`已添加「${created.name}」`, { icon: 'plus' })
        return
      }

      const res = await catalog.addApps(parentId, lines, args, directory, dialog.rootIndex)
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

    if (dialog.mode === 'edit-app') {
      const trimmed = name.trim()
      if (!trimmed) return fail('应用名称不能为空')
      if (!source.trim()) return fail('来源路径或网址不能为空')
      const ok = await catalog.updateApp({
        id: dialog.id,
        name: trimmed,
        sourcePath: source.trim(),
        arguments: optional(argumentsText),
        workingDir: optional(workingDir),
      })
      if (!ok) return
      closeDialog()
      ui.pushToast(`已保存「${trimmed}」`, { icon: 'pencil' })
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
  const editing = dialog.mode === 'edit-app'

  return (
    <form onSubmit={submit}>
      <h2 className="dialog-title">{title}</h2>
      <div className="dialog-fields">
        {(dialog.mode === 'add' || editing) && (
          <>
            <label className="field">
              <span className="field-label">显示名称</span>
              <input
                ref={dialog.mode === 'edit-app' ? (firstRef as React.RefObject<HTMLInputElement>) : undefined}
                className="field-input"
                type="text"
                placeholder="留空时自动推导"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </label>
            <label className="field">
              <span className="field-label">来源路径或网址</span>
              {editing ? (
                <input
                  className="field-input"
                  type="text"
                  placeholder="C:\\Program Files\\App\\app.exe"
                  value={source}
                  onChange={(e) => setSource(e.target.value)}
                />
              ) : (
                <textarea
                  ref={firstRef as React.RefObject<HTMLTextAreaElement>}
                  className="field-input"
                  rows={3}
                  placeholder={'C:\\Program Files\\App\\app.exe\nhttps://example.com'}
                  value={source}
                  onChange={(e) => setSource(e.target.value)}
                />
              )}
              {!editing && <span className="field-hint">每行一个，可批量导入；网址图标会在后台补齐</span>}
            </label>
            <div className="field">
              <span className="field-label">启动参数（可选）</span>
              <input
                className="field-input"
                type="text"
                placeholder={'例如：-NoProfile -File "D:\\scripts\\start.ps1"'}
                value={argumentsText}
                onChange={(e) => setArgumentsText(e.target.value)}
              />
              <label className="field-sub">
                <span>工作目录</span>
                <input
                  className="field-input"
                  type="text"
                  placeholder="例如：D:\\Projects"
                  value={workingDir}
                  onChange={(e) => setWorkingDir(e.target.value)}
                />
              </label>
              <span className="field-hint">
                {editing
                  ? '留空表示不使用启动参数或工作目录；保存后运行只读取当前配置'
                  : '添加时解析快捷方式作为默认值；保存后运行只读取当前配置'}
              </span>
            </div>
            {!editing && (
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
            )}
          </>
        )}

        {dialog.mode === 'new-category' && (
          <label className="field">
            <span className="field-label">分类名称</span>
            <input
              ref={firstRef as React.RefObject<HTMLInputElement>}
              className="field-input"
              type="text"
              placeholder="例如：开发工具"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </label>
        )}

        {dialog.mode === 'new-group' && (
          <label className="field">
            <span className="field-label">分组名称</span>
            <input
              ref={firstRef as React.RefObject<HTMLInputElement>}
              className="field-input"
              type="text"
              placeholder="例如：前端"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <span className="field-hint">父级：{locateNode(data, dialog.parentId)?.node.name ?? '顶层'}</span>
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

function dialogMeta(dialog: DialogState, data: ReturnType<typeof useCatalogStore.getState>['data']): {
  title: string
  submitLabel: string
} {
  switch (dialog.mode) {
    case 'add':
      return { title: '添加应用', submitLabel: '保存' }
    case 'new-category':
      return { title: '新建分类', submitLabel: '创建' }
    case 'new-group':
      return { title: '新建分组', submitLabel: '创建' }
    case 'edit-app':
      return { title: '编辑应用', submitLabel: '保存' }
    case 'delete-node': {
      const kind = locateNode(data, dialog.id)?.kind
      return { title: kind === 'Category' ? '删除分类' : '删除分组', submitLabel: '删除' }
    }
  }
}
