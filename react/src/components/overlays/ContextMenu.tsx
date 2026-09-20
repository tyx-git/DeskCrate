import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'

import {
  countApps,
  depthOf,
  findApp,
  findGroup,
  MAX_GROUP_DEPTH,
  ROOT_ID,
  ROOT_LOCATION,
  type NodeLocation,
} from '../../lib/tree'
import { useCatalogStore } from '../../state/catalogStore'
import { useSettingsStore } from '../../state/settingsStore'
import { useUiStore, type DialogState } from '../../state/uiStore'
import type { AppData, AppEXE } from '../../types/catalog'

interface MenuEntry {
  label: string
  onSelect: () => void
  tone?: 'danger'
  disabled?: boolean
}

/** 菜单里的分隔线。 */
const SEPARATOR = 'separator' as const
type MenuNode = MenuEntry | typeof SEPARATOR

interface BuildContext {
  data: AppData
  openDialog: (dialog: DialogState) => void
  launchApp: (app: AppEXE) => Promise<void>
  deleteCategory: (id: string) => Promise<boolean>
  deleteGroup: (id: string) => Promise<boolean>
  deleteApp: (id: string) => Promise<boolean>
}

/**
 * 右键菜单。
 *
 * 分类、分组、应用和空白处各有一套操作。菜单渲染在窗口内部，位置在绘制
 * 前按实际尺寸夹取，避免在侧栏右下角右键时被窗口边界裁掉。
 */
export function ContextMenu() {
  const menu = useUiStore((state) => state.contextMenu)
  const close = useUiStore((state) => state.closeContextMenu)
  const openDialog = useUiStore((state) => state.openDialog)

  const data = useCatalogStore((state) => state.data)
  const launchApp = useCatalogStore((state) => state.launchApp)
  const load = useCatalogStore((state) => state.load)
  const deleteCategory = useCatalogStore((state) => state.deleteCategory)
  const deleteGroup = useCatalogStore((state) => state.deleteGroup)
  const deleteApp = useCatalogStore((state) => state.deleteApp)
  const resetWindowSize = useSettingsStore((state) => state.resetWindowSize)

  const ref = useRef<HTMLDivElement>(null)
  const [position, setPosition] = useState({ left: 0, top: 0 })

  const entries = useMemo<MenuNode[]>(() => {
    const target = menu?.target ?? null

    if (!target) {
      return [
        // 顶层不是分类，但同样能放应用：常用的那几个不必先建一个分类。
        {
          label: '添加应用到顶层…',
          onSelect: () =>
            openDialog({
              kind: 'add-app',
              targetId: ROOT_ID,
              targetName: ROOT_LOCATION.name,
            }),
        },
        { label: '新建分类…', onSelect: () => openDialog({ kind: 'new-category' }) },
        { label: '刷新数据', onSelect: () => void load() },
        SEPARATOR,
        // 窗口尺寸可以被拖拽改掉，拖歪了要有个入口回到设计稿的尺寸。
        // 这个菜单只会在展开态弹出（收起态的外壳直接不处理右键），
        // 所以不需要额外判断当前视图模式。
        { label: '重置窗口尺寸', onSelect: () => void resetWindowSize() },
      ]
    }

    return buildEntries(target, {
      data,
      openDialog,
      launchApp,
      deleteCategory,
      deleteGroup,
      deleteApp,
    })
  }, [
    menu,
    data,
    openDialog,
    launchApp,
    load,
    deleteCategory,
    deleteGroup,
    deleteApp,
    resetWindowSize,
  ])

  // 先按鼠标位置渲染，再在浏览器绘制前按实际尺寸夹取。
  useLayoutEffect(() => {
    const element = ref.current
    if (!menu || !element) {
      return
    }

    const { width, height } = element.getBoundingClientRect()
    const margin = 4
    setPosition({
      left: Math.max(margin, Math.min(menu.x, window.innerWidth - width - margin)),
      top: Math.max(margin, Math.min(menu.y, window.innerHeight - height - margin)),
    })
  }, [menu, entries.length])

  // 菜单是被「用户自己把注意力挪走」关掉的（点到别处、按 Tab 走出去）还是被
  // 「程序化关闭」（Esc、选中菜单项）关掉的，决定了要不要把焦点还给触发元素：
  // 前者不能抢回来，后者必须还，否则键盘用户按 Esc 之后焦点会掉到 `body`，
  // 在侧栏里的位置就彻底丢了。
  const keepFocusAway = useRef(false)

  const handlePointerDown = useCallback(
    (event: PointerEvent) => {
      if (!ref.current?.contains(event.target as Node)) {
        keepFocusAway.current = true
        close()
      }
    },
    [close],
  )

  useEffect(() => {
    if (!menu) {
      return
    }

    window.addEventListener('pointerdown', handlePointerDown, true)
    window.addEventListener('blur', close)
    window.addEventListener('resize', close)
    return () => {
      window.removeEventListener('pointerdown', handlePointerDown, true)
      window.removeEventListener('blur', close)
      window.removeEventListener('resize', close)
    }
  }, [menu, handlePointerDown, close])

  // 打开时把焦点移进菜单。菜单是触发图标的**兄弟节点**而不是祖先，键盘事件
  // 只会沿着焦点元素的祖先链冒泡，所以不移焦点的话方向键根本到不了下面的
  // onKeyDown，反而会被全局快捷键当成「在图标之间移动」，菜单变得无法用键盘操作。
  useEffect(() => {
    if (!menu) {
      return
    }

    keepFocusAway.current = false
    const trigger = document.activeElement as HTMLElement | null
    ref.current?.querySelector<HTMLButtonElement>('[role="menuitem"]:not([disabled])')?.focus()

    return () => {
      // 触发元素可能已经不在了（比如从菜单里删掉了那一行），这时无处可还。
      if (keepFocusAway.current || !trigger?.isConnected) {
        return
      }

      // 不能依赖 `document.activeElement` 判断焦点是否还在菜单里：passive effect 的
      // 清理函数在 React 摘除 DOM 之后才跑，那时获得焦点的菜单项已经被移出文档，
      // 浏览器把焦点丢回 `body`，`activeElement` 里读不到任何线索。
      trigger.focus()
    }
  }, [menu])

  if (!menu) {
    return null
  }

  const moveFocus = (delta: number) => {
    const buttons = Array.from(
      ref.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not([disabled])') ?? [],
    )
    if (buttons.length === 0) {
      return
    }

    const current = buttons.indexOf(document.activeElement as HTMLButtonElement)
    const next = buttons[(current + delta + buttons.length) % buttons.length]
    next?.focus()
  }

  return (
    <div
      ref={ref}
      role="menu"
      aria-orientation="vertical"
      aria-label={menu.target ? `${menu.target.name} 的操作` : '侧栏操作'}
      style={{ left: position.left, top: position.top }}
      className="absolute z-30 min-w-[8.5em] overflow-hidden rounded-sheet bg-sheet py-[0.5em] text-chip text-ink shadow-sheet backdrop-blur-sheet"
      onBlur={(event) => {
        // 焦点离开菜单就收起：按 Tab 走出去以后菜单还悬在那里会一直挡住下面的内容。
        if (!ref.current?.contains(event.relatedTarget)) {
          keepFocusAway.current = true
          close()
        }
      }}
      onContextMenu={(event) => {
        // 阻止冒泡，否则会被外壳的空白区右键处理再打开一个菜单。
        event.preventDefault()
        event.stopPropagation()
      }}
      onKeyDown={(event) => {
        if (event.key === 'ArrowDown') {
          event.preventDefault()
          moveFocus(1)
        } else if (event.key === 'ArrowUp') {
          event.preventDefault()
          moveFocus(-1)
        }
      }}
    >
      {entries.map((entry, index) =>
        entry === SEPARATOR ? (
          <div key={`sep-${index}`} role="separator" className="my-[0.4em] h-px bg-hairline" />
        ) : (
          <button
            key={entry.label}
            type="button"
            role="menuitem"
            disabled={entry.disabled}
            title={entry.disabled ? '分组最多嵌套 4 层' : undefined}
            onClick={() => {
              close()
              entry.onSelect()
            }}
            className={`block w-full px-[1.1em] py-[0.45em] text-left transition-colors duration-100 disabled:cursor-not-allowed disabled:opacity-40 ${
              entry.tone === 'danger'
                ? 'text-danger hover:bg-[rgb(176_56_56_/_0.12)]'
                : 'hover:bg-active'
            }`}
          >
            {entry.label}
          </button>
        ),
      )}
    </div>
  )
}

function buildEntries(target: NodeLocation, context: BuildContext): MenuNode[] {
  const { data, openDialog } = context
  const renameEntry: MenuEntry = {
    label: '重命名…',
    onSelect: () =>
      openDialog({
        kind: 'rename',
        targetId: target.id,
        targetName: target.name,
        initialValue: target.name,
      }),
  }

  if (target.kind === 'category') {
    const category = data.items
      .flatMap((item) => ('Category' in item ? [item.Category] : []))
      .find((item) => item.id === target.id)
    const descendants = category ? countApps(category.groups) : 0

    return [
      {
        label: '添加应用…',
        onSelect: () => openDialog({ kind: 'add-app', targetId: target.id, targetName: target.name }),
      },
      {
        label: '新建分组…',
        onSelect: () => openDialog({ kind: 'new-group', targetId: target.id, targetName: target.name }),
      },
      renameEntry,
      SEPARATOR,
      {
        label: '删除分类',
        tone: 'danger',
        // 后端没有撤销，删除分类会连里面的分组和应用一起消失，必须先确认。
        onSelect: () =>
          openDialog({
            kind: 'confirm',
            targetName: `删除分类「${target.name}」？`,
            message:
              descendants > 0
                ? `分类下的 ${descendants} 个应用和全部分组会一起删除，此操作无法撤销。`
                : '该分类下暂无内容，删除后无法撤销。',
            confirmLabel: '删除',
            onConfirm: () => void context.deleteCategory(target.id),
          }),
      },
    ]
  }

  if (target.kind === 'group') {
    const depth = depthOf(data, target.id) ?? MAX_GROUP_DEPTH
    const group = findGroup(data, target.id)
    const descendants = group ? countApps(group.children) : 0

    return [
      {
        label: '添加应用…',
        onSelect: () => openDialog({ kind: 'add-app', targetId: target.id, targetName: target.name }),
      },
      {
        label: '新建子分组…',
        // 与后端 `MAX_GROUP_DEPTH` 对齐，提前拦截超深嵌套。
        disabled: depth >= MAX_GROUP_DEPTH,
        onSelect: () => openDialog({ kind: 'new-group', targetId: target.id, targetName: target.name }),
      },
      renameEntry,
      SEPARATOR,
      {
        label: '删除分组',
        tone: 'danger',
        onSelect: () =>
          openDialog({
            kind: 'confirm',
            targetName: `删除分组「${target.name}」？`,
            message:
              descendants > 0
                ? `分组下的 ${descendants} 个应用和子分组会一起删除，此操作无法撤销。`
                : '该分组下暂无内容，删除后无法撤销。',
            confirmLabel: '删除',
            onConfirm: () => void context.deleteGroup(target.id),
          }),
      },
    ]
  }

  const app = findApp(data, target.id)
  return [
    {
      label: '启动',
      disabled: !app,
      onSelect: () => {
        if (app) {
          void context.launchApp(app)
        }
      },
    },
    renameEntry,
    SEPARATOR,
    {
      label: '删除',
      tone: 'danger',
      onSelect: () =>
        openDialog({
          kind: 'confirm',
          targetName: `删除「${target.name}」？`,
          message: '应用会从侧栏移除，此操作无法撤销。',
          confirmLabel: '删除',
          onConfirm: () => void context.deleteApp(target.id),
        }),
    },
  ]
}
