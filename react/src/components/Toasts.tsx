// 面板内通知：最多三条，删除应用带「撤销」动作，自动淡出。
import { useUiStore } from '../state/uiStore'
import { Glyph } from './Glyph'

export function Toasts() {
  const toasts = useUiStore((s) => s.toasts)
  const dismissToast = useUiStore((s) => s.dismissToast)

  return (
    <div className="toasts" aria-live="polite">
      {toasts.map((toast) => (
        <div className={`toast${toast.danger ? ' is-danger' : ''}${toast.leaving ? ' is-leaving' : ''}`} key={toast.id}>
          <span className="toast-icon">
            <Glyph name={toast.icon} />
          </span>
          <span className="toast-text">{toast.text}</span>
          {toast.actionLabel && (
            <button
              className="toast-action"
              onClick={() => {
                dismissToast(toast.id)
                toast.onAction?.()
              }}
            >
              {toast.actionLabel}
            </button>
          )}
          <button className="icon-btn toast-close" aria-label="关闭通知" onClick={() => dismissToast(toast.id)}>
            <Glyph name="x" />
          </button>
        </div>
      ))}
    </div>
  )
}