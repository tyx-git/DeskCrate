// 面板内交互状态：搜索、菜单、对话框、设置、通知与拖拽标记。
import { create } from 'zustand'

import type { GlyphName } from '../components/Glyph'

export type DialogState =
  | { mode: 'add'; source?: string; name?: string; parentId?: string; rootIndex?: number }
  | { mode: 'new-category' }
  | { mode: 'new-group'; parentId: string }
  | { mode: 'rename-app'; id: string }
  | { mode: 'delete-node'; id: string }

export type CtxTargetType = 'app' | 'Category' | 'Group' | 'blank'

export interface CtxState {
  type: CtxTargetType
  id: string
  clientX: number
  clientY: number
  /** 非空表示菜单已钻入「移动到…」子页。 */
  moveAppId?: string
}

export interface Toast {
  id: number
  text: string
  icon: GlyphName
  danger: boolean
  actionLabel?: string
  onAction?: () => void
  duration: number
  leaving: boolean
}

export interface ToastOptions {
  icon?: GlyphName
  danger?: boolean
  actionLabel?: string
  onAction?: () => void
  duration?: number
}

/** 拖拽中的节点：应用、分组、顶层分类。 */
export interface DragState {
  kind: 'app' | 'Group' | 'Category'
  id: string
}

interface UiState {
  query: string
  dialog: DialogState | null
  ctx: CtxState | null
  settingsOpen: boolean
  toasts: Toast[]
  drag: DragState | null
  renamingId: string | null
  launchingId: string | null
  sizeBadge: string | null
  fileDragging: boolean

  setQuery: (query: string) => void
  openDialog: (dialog: DialogState) => void
  closeDialog: () => void
  openCtx: (type: CtxTargetType, id: string, clientX: number, clientY: number) => void
  openMoveMenu: (appId: string) => void
  closeCtx: () => void
  openSettings: () => void
  closeSettings: () => void
  pushToast: (text: string, options?: ToastOptions) => number
  notify: (kind: 'error' | 'info' | 'success', text: string) => void
  dismissToast: (id: number) => void
  startDrag: (drag: DragState) => void
  endDrag: () => void
  startRename: (id: string) => void
  endRename: () => void
  setLaunching: (id: string | null) => void
  setSizeBadge: (text: string | null) => void
  setFileDragging: (value: boolean) => void
}

let toastSeq = 0
const toastTimers = new Map<number, number>()
const MAX_TOASTS = 3

export const useUiStore = create<UiState>((set, get) => ({
  query: '',
  dialog: null,
  ctx: null,
  settingsOpen: false,
  toasts: [],
  drag: null,
  renamingId: null,
  launchingId: null,
  sizeBadge: null,
  fileDragging: false,

  setQuery: (query) => set({ query }),

  openDialog: (dialog) => set({ dialog, ctx: null }),
  closeDialog: () => set({ dialog: null }),

  openCtx: (type, id, clientX, clientY) => set({ ctx: { type, id, clientX, clientY } }),
  openMoveMenu: (appId) => {
    const ctx = get().ctx
    if (ctx) set({ ctx: { ...ctx, moveAppId: appId } })
  },
  closeCtx: () => set({ ctx: null }),

  openSettings: () => set({ settingsOpen: true, ctx: null }),
  closeSettings: () => set({ settingsOpen: false }),

  pushToast: (text, options = {}) => {
    const id = ++toastSeq
    const toast: Toast = {
      id,
      text,
      icon: options.icon ?? 'check',
      danger: options.danger ?? false,
      actionLabel: options.actionLabel,
      onAction: options.onAction,
      duration: options.duration ?? (options.actionLabel ? 6000 : 3600),
      leaving: false,
    }
    set((state) => {
      const kept = state.toasts.filter((t) => !t.leaving)
      const overflow = kept.slice(0, Math.max(0, kept.length - (MAX_TOASTS - 1)))
      for (const old of overflow) {
        const timer = toastTimers.get(old.id)
        if (timer) window.clearTimeout(timer)
        toastTimers.delete(old.id)
      }
      const next = [...kept, toast].slice(-MAX_TOASTS)
      return { toasts: next }
    })
    const timer = window.setTimeout(() => get().dismissToast(id), toast.duration)
    toastTimers.set(id, timer)
    return id
  },

  notify: (kind, text) => {
    get().pushToast(text, {
      icon: kind === 'error' ? 'alert' : kind === 'success' ? 'check' : 'check',
      danger: kind === 'error',
    })
  },

  dismissToast: (id) => {
    const timer = toastTimers.get(id)
    if (timer) window.clearTimeout(timer)
    toastTimers.delete(id)
    set((state) => ({
      toasts: state.toasts.map((t) => (t.id === id ? { ...t, leaving: true } : t)),
    }))
    window.setTimeout(() => {
      set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) }))
    }, 180)
  },

  startDrag: (drag) => set({ drag, ctx: null, fileDragging: false }),
  endDrag: () => set({ drag: null, fileDragging: false }),

  startRename: (renamingId) => set({ renamingId, ctx: null }),
  endRename: () => set({ renamingId: null }),

  setLaunching: (launchingId) => set({ launchingId }),
  setSizeBadge: (sizeBadge) => set({ sizeBadge }),
  setFileDragging: (fileDragging) => set({ fileDragging }),
}))