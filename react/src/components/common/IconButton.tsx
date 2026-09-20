import type { ReactNode } from 'react'

interface IconButtonProps {
  /** 无障碍名称，同时作为原生 tooltip。 */
  label: string
  onClick: () => void
  children: ReactNode
  tone?: 'default' | 'danger'
  className?: string
}

/** 侧栏头部与菜单里的紧凑图标按钮。 */
export function IconButton({
  label,
  onClick,
  children,
  tone = 'default',
  className,
}: IconButtonProps) {
  const toneClass =
    tone === 'danger'
      ? 'text-danger hover:bg-[rgb(176_56_56_/_0.1)]'
      : 'text-ink-soft hover:bg-hover hover:text-ink'

  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      onClick={onClick}
      className={`inline-flex h-[1.9em] w-[1.9em] shrink-0 items-center justify-center rounded-[6px] text-[1em] leading-none transition-colors duration-100 ${toneClass} ${className ?? ''}`}
    >
      {children}
    </button>
  )
}
