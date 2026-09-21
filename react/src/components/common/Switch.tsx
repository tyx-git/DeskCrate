interface SwitchProps {
  /** 当前是否开启。 */
  checked: boolean
  /** 切换时触发；由调用方负责持久化。 */
  onChange: (checked: boolean) => void
  /** 开关左侧的说明文字。 */
  label: string
  /** 说明文字下方的补充；可以省略。 */
  description?: string
}

/**
 * 通用开关。
 *
 * 抽出来是因为设置页里每一项都是「一行说明 + 一个开关」这个形状，之后新增设置
 * 只需要多加一行，不必再复制一遍轨道/圆点的样式。
 */
export function Switch({ checked, onChange, label, description }: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className="flex w-full items-center gap-[2cqw] rounded-[6px] px-[2%] py-[2.2%] text-left transition-colors duration-100 hover:bg-hover"
    >
      <span className="min-w-0 flex-1">
        <span className="block text-ink">{label}</span>
        {description ? (
          <span className="mt-[0.4%] block text-chip leading-[1.35] text-ink-faint">
            {description}
          </span>
        ) : null}
      </span>

      <span
        aria-hidden="true"
        className={`relative inline-flex h-[1.5em] w-[2.9em] shrink-0 items-center rounded-full transition-colors duration-150 ${
          checked ? 'bg-[rgb(58_96_190_/_0.55)]' : 'bg-[rgb(0_0_0_/_0.12)]'
        }`}
      >
        <span
          className="absolute h-[1.1em] w-[1.1em] rounded-full bg-white shadow-[0_1px_2px_rgb(0_0_0_/_0.25)] transition-all duration-150"
          style={{ left: checked ? 'calc(100% - 1.25em)' : '0.2em' }}
        />
      </span>
    </button>
  )
}
