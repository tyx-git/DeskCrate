import { useUiStore } from '../../state/uiStore'

/** 内联提示条。侧栏窗口很小，用内联条而不是浮动 toast，避免遮挡图标。 */
export function NoticeBar() {
  const notice = useUiStore((state) => state.notice)
  const dismiss = useUiStore((state) => state.dismissNotice)

  if (!notice) {
    return null
  }

  const tone = notice.kind === 'error' ? 'dc-notice--error' : ''

  return (
    <div
      // 错误要立刻播报，用 alert（assertive）；普通提示用 status（polite）即可。
      role={notice.kind === 'error' ? 'alert' : 'status'}
      className={`dc-notice ${tone}`}
    >
      <span className="min-w-0 flex-1 break-words">{notice.message}</span>
      <button
        type="button"
        onClick={dismiss}
        aria-label="关闭提示"
        title="关闭提示"
        className="dc-notice__close shrink-0 leading-none opacity-70 transition-opacity hover:opacity-100"
      >
        ×
      </button>
    </div>
  )
}
