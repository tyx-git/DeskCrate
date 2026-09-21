import { countAllApps } from '../../lib/tree'
import { useCatalogStore } from '../../state/catalogStore'
import { useUiStore } from '../../state/uiStore'
import { IconButton } from '../common/IconButton'

/** 设置图标：内圈 + 八根辐条，比画完整齿轮的齿更省事也更好认。 */
function GearIcon() {
  return (
    <svg viewBox="0 0 12 12" aria-hidden="true" className="h-[1em] w-[1em]">
      <circle cx="6" cy="6" r="2" fill="none" stroke="currentColor" strokeWidth="1.3" />
      <path
        d="M6 0.8v1.5M6 9.7v1.5M0.8 6h1.5M9.7 6h1.5M2.3 2.3l1 1M8.7 8.7l1 1M9.7 2.3l-1 1M3.3 8.7l-1 1"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.3"
        strokeLinecap="round"
      />
    </svg>
  )
}

/** 侧栏底栏：新建分类入口、应用总数、设置入口。 */
export function SidebarFooter() {
  const total = useCatalogStore((state) => countAllApps(state.data))
  const openDialog = useUiStore((state) => state.openDialog)
  const setSettingsOpen = useUiStore((state) => state.setSettingsOpen)

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

      <IconButton
        label="设置"
        onClick={() => setSettingsOpen(true)}
        className="ml-auto text-caret hover:text-ink"
      >
        <GearIcon />
      </IconButton>
    </footer>
  )
}
