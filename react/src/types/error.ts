// 后端统一错误结构，对应 Rust models::AppError。

export interface AppError {
  code: string
  message: string
}

export function isAppError(value: unknown): value is AppError {
  if (typeof value !== 'object' || value === null) {
    return false
  }
  const candidate = value as Record<string, unknown>
  return typeof candidate.code === 'string' && typeof candidate.message === 'string'
}