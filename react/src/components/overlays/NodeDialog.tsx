import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent, type ReactNode } from 'react'

import { isPreviewMode } from '../../api'
import { locateNode, ROOT_ID } from '../../lib/tree'
import { useCatalogStore } from '../../state/catalogStore'
import { useUiStore, type DialogState } from '../../state/uiStore'

/**
 * 新建 / 重命名 / 添加应用 / 删除确认的通用对话框。
 *
 * 这些操作只有一两个字段，用同一套遮罩 + 面板承载可以少写四套弹窗。
 */
export function NodeDialog() {
  const dialog = useUiStore((state) => state.dialog)

  if (!dialog) {
    return null
  }

  // 用 key 让切换目标时表单状态重置，避免残留上一个节点的输入。
  const key = `${dialog.kind}:${dialog.targetId ?? ''}`

  return dialog.kind === 'confirm' ? (
    <ConfirmDialog key={key} dialog={dialog} />
  ) : (
    <DialogForm key={key} dialog={dialog} />
  )
}

/** 同一时刻只会渲染一个对话框，标题 id 用常量即可。 */
const DIALOG_TITLE_ID = 'dc-dialog-title'

/** 对话框遮罩：点空白处关闭，并把 Tab 循环限制在对话框内部。 */
function Scrim({ onDismiss, children }: { onDismiss: () => void; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null)

  // 遮罩之下侧栏仍然挂在 DOM 里，不限制 Tab 的话焦点会走到遮罩背后那些
  // 既看不见也点不到的按钮上。这里把 Tab 首尾相接，做一个最小焦点陷阱。
  const trapTab = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== 'Tab') {
      return
    }

    const focusables = ref.current?.querySelectorAll<HTMLElement>(
      'button:not([disabled]), input:not([disabled]), [href], select, textarea',
    )
    if (!focusables || focusables.length === 0) {
      return
    }

    const first = focusables[0]
    const last = focusables[focusables.length - 1]

    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault()
      first.focus()
    }
  }

  return (
    <div
      ref={ref}
      onKeyDown={trapTab}
      className="absolute inset-0 z-40 flex items-center justify-center bg-[rgb(28_32_36_/_0.3)] px-[5%] backdrop-blur-[3px]"
      onPointerDown={(event) => {
        if (event.target === event.currentTarget) {
          onDismiss()
        }
      }}
    >
      {children}
    </div>
  )
}

/**
 * 破坏性操作的二次确认。
 *
 * 后端没有撤销能力，误删一个分类会连同里面的分组和应用一起消失，
 * 所以删除类操作统一先过这一步。
 */
function ConfirmDialog({ dialog }: { dialog: DialogState }) {
  const close = useUiStore((state) => state.closeDialog)
  const confirmRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    confirmRef.current?.focus()
  }, [])

  return (
    <Scrim onDismiss={close}>
      <div
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={DIALOG_TITLE_ID}
        className="dc-sheet-border w-full rounded-[10px] bg-sheet p-[5%] text-field text-ink shadow-sheet backdrop-blur-sheet"
      >
        <p className="dc-dialog__eyebrow">CONFIRM</p>
        <h2 id={DIALOG_TITLE_ID} className="text-cat font-semibold break-words">
          {dialog.targetName ?? '确认操作'}
        </h2>
        <p className="mt-[3%] text-desc leading-[1.5] break-words text-ink-mute">
          {dialog.message}
        </p>
        <div className="mt-[5%] flex justify-end gap-[6px] text-desc">
          <button
            type="button"
            onClick={close}
            className="dc-btn dc-btn--ghost"
          >
            取消
          </button>
          <button
            ref={confirmRef}
            type="button"
            onClick={() => {
              close()
              dialog.onConfirm?.()
            }}
            className="dc-btn dc-btn--danger"
          >
            {dialog.confirmLabel ?? '确认'}
          </button>
        </div>
      </div>
    </Scrim>
  )
}

interface FieldMeta {
  title: string
  eyebrow: string
  confirmLabel: string
  nameLabel?: string
  namePlaceholder?: string
  withSourcePath?: boolean
}

function describeDialog(dialog: DialogState): FieldMeta {
  switch (dialog.kind) {
    case 'new-category':
      return {
        title: '新建分类',
        eyebrow: 'NEW CATEGORY',
        confirmLabel: '创建',
        nameLabel: '分类名称',
        namePlaceholder: '例如：开发工具',
      }
    case 'new-group':
      return {
        title: `在「${dialog.targetName ?? ''}」下新建分组`,
        eyebrow: 'NEW GROUP',
        confirmLabel: '创建',
        nameLabel: '分组名称',
        namePlaceholder: '例如：编辑器',
      }
    case 'add-app':
      return {
        // 顶层不是分类，标题里不能出现空的分类名。
        title:
          dialog.targetId === ROOT_ID
            ? '添加应用到顶层'
            : `向「${dialog.targetName ?? ''}」添加应用`,
        eyebrow: 'ADD APP',
        confirmLabel: '添加',
        nameLabel: '显示名称（可选）',
        namePlaceholder: '留空则按来源自动命名',
        withSourcePath: true,
      }
    default:
      return { title: '重命名', eyebrow: 'RENAME', confirmLabel: '保存', nameLabel: '新名称' }
  }
}

function DialogForm({ dialog }: { dialog: DialogState }) {
  const close = useUiStore((state) => state.closeDialog)

  const data = useCatalogStore((state) => state.data)
  const busy = useCatalogStore((state) => state.busy)
  const addCategory = useCatalogStore((state) => state.addCategory)
  const addGroup = useCatalogStore((state) => state.addGroup)
  const addApp = useCatalogStore((state) => state.addApp)
  const renameCategory = useCatalogStore((state) => state.renameCategory)
  const renameGroup = useCatalogStore((state) => state.renameGroup)
  const renameApp = useCatalogStore((state) => state.renameApp)

  const [name, setName] = useState(dialog.initialValue ?? '')
  const [sourcePath, setSourcePath] = useState('')
  const firstFieldRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    firstFieldRef.current?.focus()
    firstFieldRef.current?.select()
  }, [])

  const meta = describeDialog(dialog)
  const trimmedName = name.trim()
  const trimmedPath = sourcePath.trim()

  // 添加应用必须给出路径；其余操作必须给出非空名称。
  const canSubmit =
    !busy && (meta.withSourcePath ? trimmedPath.length > 0 : trimmedName.length > 0)

  // 重命名需要按 ID 判断目标类型，进而选择对应的 Command。
  const renameTarget =
    dialog.kind === 'rename' && dialog.targetId
      ? locateNode(data, dialog.targetId)
      : null

  const submit = async (event: FormEvent) => {
    event.preventDefault()

    switch (dialog.kind) {
      case 'new-category': {
        await addCategory(trimmedName)
        break
      }
      case 'new-group': {
        if (dialog.targetId) {
          await addGroup(dialog.targetId, trimmedName)
        }
        break
      }
      case 'add-app': {
        if (dialog.targetId && trimmedPath.length > 0) {
          // `ROOT_ID` 只存在于前端，顶层对后端来说是「没有父节点」。
          const parentId = dialog.targetId === ROOT_ID ? null : dialog.targetId
          await addApp(parentId, trimmedPath, trimmedName || undefined)
        }
        break
      }
      case 'rename': {
        if (!renameTarget) {
          break
        }
        if (renameTarget.kind === 'category') {
          await renameCategory(renameTarget.id, trimmedName)
        } else if (renameTarget.kind === 'group') {
          await renameGroup(renameTarget.id, trimmedName)
        } else {
          await renameApp(renameTarget.id, trimmedName)
        }
        break
      }
      default:
        break
    }

    close()
  }

  const nameRequired = !meta.withSourcePath

  return (
    <Scrim onDismiss={close}>
      <form
        role="dialog"
        aria-modal="true"
        aria-labelledby={DIALOG_TITLE_ID}
        onSubmit={(event) => void submit(event)}
        className="dc-sheet-border w-full rounded-[10px] bg-sheet p-[5%] text-field text-ink shadow-sheet backdrop-blur-sheet"
      >
        <p className="dc-dialog__eyebrow">{meta.eyebrow}</p>
        <h2 id={DIALOG_TITLE_ID} className="text-cat font-semibold break-words">
          {meta.title}
        </h2>

        {meta.withSourcePath ? (
          <label className="mt-[4%] block">
            <span className="mb-[1.5%] block text-desc text-ink-mute">来源路径或网址</span>
            <input
              ref={firstFieldRef}
              type="text"
              value={sourcePath}
              spellCheck={false}
              placeholder="C:\\Tools\\App\\app.exe 或 https://example.com"
              onChange={(event) => setSourcePath(event.target.value)}
              className="dc-input"
            />
          </label>
        ) : null}

        <label className="mt-[4%] block">
          <span className="mb-[1.5%] block text-desc text-ink-mute">
            {meta.nameLabel ?? '名称'}
            {nameRequired ? '' : '（可选）'}
          </span>
          <input
            ref={meta.withSourcePath ? undefined : firstFieldRef}
            type="text"
            value={name}
            spellCheck={false}
            placeholder={meta.namePlaceholder}
            onChange={(event) => setName(event.target.value)}
            className="dc-input"
          />
        </label>

        {meta.withSourcePath ? (
          <p className="mt-[3%] text-desc leading-[1.4] text-ink-faint">
            {isPreviewMode
              ? '浏览器预览模式下不会真的解析图标，只做本地记录。'
              : '也可以把文件、文件夹或快捷方式直接拖到侧栏：拖到分类上就进该分类，拖到两条分界线之间则作为顶层应用插在那个位置。填网址会自动抓取站点图标。'}
          </p>
        ) : null}

        <div className="mt-[5%] flex justify-end gap-[6px] text-desc">
          <button
            type="button"
            onClick={close}
            className="dc-btn dc-btn--ghost"
          >
            取消
          </button>
          <button
            type="submit"
            disabled={!canSubmit}
            className="dc-btn dc-btn--primary"
          >
            {meta.confirmLabel}
          </button>
        </div>
      </form>
    </Scrim>
  )
}
