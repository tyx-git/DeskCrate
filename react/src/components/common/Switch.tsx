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
      className="dc-setting"
    >
      <span className="dc-setting__text">
        <b>{label}</b>
        {description ? <small>{description}</small> : null}
      </span>

      <span aria-hidden="true" className="dc-switch" data-on={checked}>
        <span className="dc-switch__thumb" />
      </span>
    </button>
  )
}
