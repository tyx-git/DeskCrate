import { create } from 'zustand'

import type { NodeLocation } from '../lib/tree'

/** 对话框种类。每种对应一组字段和一个提交动作。 */
export type DialogKind = 'new-category' | 'new-group' | 'add-app' | 'rename' | 'confirm'

export interface DialogState {
  kind: DialogKind
  /** 操作目标节点的 ID；新建分类时为空。 */
  targetId?: string
  /** 操作目标节点的展示名称，用于对话框标题。 */
  targetName?: string
  /** 输入框初始值。 */
  initialValue?: string
  /** `confirm` 类型对话框的正文。 */
  message?: string
  /** `confirm` 类型对话框的确认按钮文案。 */
  confirmLabel?: string
  /** `confirm` 类型对话框点确认后执行的动作。 */
  onConfirm?: () => void
}

export interface ContextMenuState {
  /** 相对于窗口客户区的坐标。 */
  x: number
  y: number
  /** 右键命中的节点；在空白处右键时为 null，表示「新建分类」。 */
  target: NodeLocation | null
}

export interface Notice {
  id: number
  kind: 'error' | 'info'
  message: string
}

/** 拖拽应用时的落点提示：放在哪个图标的前面 / 后面。 */
export interface DropHint {
  appId: string
  position: 'before' | 'after'
}

interface UiState {
  /** 搜索关键词。 */
  query: string
  /** 键盘导航当前选中的应用 ID。 */
  selectedAppId: string | null
  contextMenu: ContextMenuState | null
  dialog: DialogState | null
  notice: Notice | null

  /** 正在被拖拽的应用 ID；没有拖拽时为 null。 */
  draggingAppId: string | null
  /** 当前悬停的落点。 */
  dropHint: DropHint | null
  /**
   * 设置页是否打开。
   *
   * 打开时侧栏主体换成设置页（同一个面板内切换），而不是再叠一层弹窗 —— 侧栏
   * 只有 338px 宽，套一层模态会把内容挤得没法看。
   */
  settingsOpen: boolean

  setQuery: (query: string) => void
  setSelectedAppId: (id: string | null) => void
  openContextMenu: (menu: ContextMenuState) => void
  closeContextMenu: () => void
  openDialog: (dialog: DialogState) => void
  closeDialog: () => void
  notify: (kind: Notice['kind'], message: string) => void
  dismissNotice: () => void
  setDraggingApp: (id: string | null) => void
  setDropHint: (hint: DropHint | null) => void
  setSettingsOpen: (open: boolean) => void
}

let noticeSeq = 0
let noticeTimer: number | undefined

export const useUiStore = create<UiState>((set) => ({
  query: '',
  selectedAppId: null,
  contextMenu: null,
  dialog: null,
  notice: null,
  draggingAppId: null,
  dropHint: null,
  settingsOpen: false,

  setQuery: (query) => set({ query, selectedAppId: null }),
  setSelectedAppId: (selectedAppId) => set({ selectedAppId }),
  openContextMenu: (contextMenu) => set({ contextMenu, dialog: null }),
  closeContextMenu: () => set({ contextMenu: null }),
  openDialog: (dialog) => set({ dialog, contextMenu: null }),

  closeDialog: () => set({ dialog: null }),

  notify: (kind, message) => {
    // 后到的提示覆盖前一条，避免错误堆叠把侧栏顶出可视区。
    window.clearTimeout(noticeTimer)
    const notice: Notice = { id: (noticeSeq += 1), kind, message }
    set({ notice })
    noticeTimer = window.setTimeout(() => set({ notice: null }), 6000)
  },

  dismissNotice: () => {
    window.clearTimeout(noticeTimer)
    set({ notice: null })
  },

  setDraggingApp: (draggingAppId) => set({ draggingAppId, dropHint: null }),
  setDropHint: (dropHint) => set({ dropHint }),
  // 打开设置页时顺手收掉右键菜单和对话框，避免两层浮层叠在一起。
  setSettingsOpen: (settingsOpen) =>
    set(settingsOpen ? { settingsOpen, contextMenu: null, dialog: null } : { settingsOpen }),
}))
