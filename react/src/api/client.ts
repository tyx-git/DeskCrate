import { isAppError, type AppError } from '../types/error'

/** 判断当前是否运行在 Tauri WebView 中（而不是普通浏览器）。 */
export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

/**
 * 把任意异常归一化成后端同形状的 `AppError`。
 *
 * Tauri 在 Command 返回 `Err` 时会把序列化后的错误对象直接作为 reject
 * 原因抛出；但网络层异常、插件缺失等也可能冒泡到这里，因此统一收敛成
 * 一种形状，UI 只需要处理 `{ code, message }`。
 */
export function toAppError(error: unknown): AppError {
  if (isAppError(error)) {
    return error
  }

  if (error instanceof Error) {
    return { code: 'frontend_error', message: error.message }
  }

  return { code: 'unknown_error', message: String(error) }
}

/** 把 `AppError` 转成可直接展示的中文文案。 */
export function describeError(error: AppError): string {
  return error.message.trim().length > 0 ? error.message : error.code
}
