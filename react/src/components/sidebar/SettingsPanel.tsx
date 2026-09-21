import { useEffect } from 'react'

import { useSettingsStore } from '../../state/settingsStore'
import { useUiStore } from '../../state/uiStore'
import { IconButton } from '../common/IconButton'
import { Switch } from '../common/Switch'

/**
 * 设置页。
 *
 * **以后所有设置项都放这里** —— 不要把开关再塞进底栏：底栏只有一行高，加一项就
 * 挤一次，而且侧栏宽 338px，横向也放不下几个。
 *
 * 每一项都是「一行说明 + 一个控件」，新增设置时往下加一行即可。
 */
export function SettingsPanel() {
  const autoCollapse = useSettingsStore((state) => state.settings?.autoCollapse ?? false)
  const setAutoCollapse = useSettingsStore((state) => state.setAutoCollapse)
  const setSettingsOpen = useUiStore((state) => state.setSettingsOpen)

  // Esc 退出设置页，与对话框、右键菜单的行为保持一致。
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation()
        setSettingsOpen(false)
      }
    }
    window.addEventListener('keydown', onKeyDown, true)
    return () => window.removeEventListener('keydown', onKeyDown, true)
  }, [setSettingsOpen])

  return (
    <div className="flex h-full min-h-0 w-full flex-col">
      <div className="flex items-center gap-[1.4cqw] border-b border-hairline px-[4%] py-[2%]">
        <IconButton label="返回" onClick={() => setSettingsOpen(false)}>
          <svg viewBox="0 0 12 12" aria-hidden="true" className="h-[1em] w-[1em]">
            <path
              d="M7 2.5 L3.5 6 L7 9.5"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.6"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </IconButton>
        <h2 className="text-cat font-semibold text-ink">设置</h2>
      </div>

      <div className="dc-scroll min-h-0 flex-1 px-[3%] py-[2%]">
        <section>
          <h3 className="px-[2%] pb-[1%] text-chip text-ink-faint">行为</h3>
          <Switch
            checked={autoCollapse}
            onChange={(next) => void setAutoCollapse(next)}
            label="自动收起"
            description="鼠标离开侧栏后自动收成指示条；关闭则一直保持展开。"
          />
        </section>
      </div>
    </div>
  )
}
