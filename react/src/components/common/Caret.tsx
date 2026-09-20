interface CaretProps {
  /** 是否处于折叠状态。折叠时箭头指向右侧。 */
  collapsed: boolean
  className?: string
}

/**
 * 展开/折叠箭头。
 *
 * 设计稿用的是 `▾` / `▸` 文本字形，这里换成内联 SVG：旋转动画更平滑，
 * 不同字号下的笔画粗细也更一致。
 */
export function Caret({ collapsed, className }: CaretProps) {
  return (
    <svg
      viewBox="0 0 12 12"
      aria-hidden="true"
      className={`shrink-0 transition-transform duration-150 ease-out ${className ?? ''}`}
      style={{ transform: collapsed ? 'rotate(-90deg)' : 'rotate(0deg)' }}
    >
      <path
        d="M2.2 4.4 L6 8.2 L9.8 4.4"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}
