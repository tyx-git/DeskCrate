/** 后端统一错误结构，对应 Rust 的 `models::AppError`。 */
export interface AppError {
  code: string;
  message: string;
}

/**
 * 判断任意值是否为后端返回的 `AppError`。
 *
 * Tauri 的 `invoke` 在 Command 返回 `Err` 时会把序列化后的错误对象作为
 * reject 原因抛出，所以这里做一次形状校验，避免把网络层异常误判成业务错误。
 */
export function isAppError(value: unknown): value is AppError {
  if (typeof value !== 'object' || value === null) {
    return false;
  }

  const candidate = value as Record<string, unknown>;
  return (
    typeof candidate.code === 'string' && typeof candidate.message === 'string'
  );
}
