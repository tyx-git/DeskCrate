import { countAllApps } from '../../lib/tree'
import { useCatalogStore } from '../../state/catalogStore'
import { useSettingsStore } from '../../state/settingsStore'
import { useUiStore } from '../../state/uiStore'
import { IconButton } from '../common/IconButton'

/** 侧栏底栏：新建分类入口、应用总数、自动收起开关。 */
export function SidebarFooter() {
  const total = useCatalogStore((state) => countAllApps(state.data))
  const autoCollapse = useSettingsStore((state) => state.settings?.autoCollapse ?? false)
  const setAutoCollapse = useSettingsStore((state) => state.setAutoCollapse)
  const openDialog = useUiStore((state) => state.openDialog)

  return (
    <footer className="dc-converge flex items-center gap-[1.4cqw] border-t border-hairline px-[5%] py-[1.6%] text-chip text-ink-faint">
      <IconButton
        label="新建分类"
        onClick={() => openDialog({ kind: 'new-category' })}
        className="text-caret hover:text-ink"
      >
        +
      </IconButton>

      <span className="shrink-0">{total} 个应用</span>

      <button
        type="button"
        role="switch"
        aria-checked={autoCollapse}
        onClick={() => void setAutoCollapse(!autoCollapse)}
        title={autoCollapse ? '关闭鼠标离开自动收起' : '开启鼠标离开自动收起'}
        className="ml-auto flex shrink-0 items-center gap-[1.2cqw] transition-colors duration-100 hover:text-ink"
      >
        <span>自动收起</span>
        <span
          className={`relative inline-flex h-[1.5em] w-[2.9em] items-center rounded-full transition-colors duration-150 ${
            autoCollapse ? 'bg-[rgb(58_96_190_/_0.55)]' : 'bg-[rgb(0_0_0_/_0.12)]'
          }`}
        >
          <span
            className="absolute h-[1.1em] w-[1.1em] rounded-full bg-white shadow-[0_1px_2px_rgb(0_0_0_/_0.25)] transition-all duration-150"
            style={{ left: autoCollapse ? 'calc(100% - 1.25em)' : '0.2em' }}
          />
        </span>
      </button>
    </footer>
  )
}
