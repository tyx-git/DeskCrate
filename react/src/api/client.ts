// 运行时判定与错误归一化。
import { isAppError, type AppError } from '../types/error'

/** 是否运行在 Tauri WebView 中（而不是普通浏览器预览）。 */
export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

/** 把任意异常收敛成后端同形状的 { code, message }。 */
export function toAppError(error: unknown): AppError {
  if (isAppError(error)) {
    return error
  }
  if (error instanceof Error) {
    return { code: 'frontend_error', message: error.message }
  }
  return { code: 'unknown_error', message: String(error) }
}

/** 转成可直接展示的中文文案。 */
export function describeError(error: AppError): string {
  return error.message.trim().length > 0 ? error.message : error.code
}