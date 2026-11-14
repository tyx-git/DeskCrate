// 后端接入层：唯一实现是真实 Tauri 命令。
import type { Backend } from './backend'
import { tauriBackend } from './tauriBackend'

export const backend: Backend = tauriBackend

export { describeError, isTauriRuntime, toAppError } from './client'
export type { Backend, FileDropEvent, UrlIconProbe, UrlIconProbeStep } from './backend'