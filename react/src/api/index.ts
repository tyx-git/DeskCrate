import type { Backend } from './backend'
import { isTauriRuntime } from './client'
import { mockBackend } from './mockBackend'
import { tauriBackend } from './tauriBackend'

/**
 * 全局后端实例。
 *
 * 在 Tauri WebView 中连接真实 Rust 后端；在普通浏览器里自动切换到内存
 * mock，这样设计稿和交互可以在 `npm run dev` 里直接验收。
 */
export const backend: Backend = isTauriRuntime() ? tauriBackend : mockBackend

/** 当前是否处于浏览器预览模式（非 Tauri）。 */
export const isPreviewMode = !backend.isTauri

export { describeError, isTauriRuntime, toAppError } from './client'
export type { Backend } from './backend'
