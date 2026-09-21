import { countAllApps } from '../../lib/tree'
import { useCatalogStore } from '../../state/catalogStore'
import { useUiStore } from '../../state/uiStore'
import { IconButton } from '../common/IconButton'

/**
 * 设置图标：八齿齿轮，单路径轮廓，比「圆 + 八条辐条」更像那么回事。
 * 颜色用 `currentColor`，由 IconButton 的 `text-caret hover:text-ink` 决定。
 *
 * 尺寸是 `1em + 2px`：底栏字号是 `text-chip`（设计稿尺寸下约 10px），加 2px 让它
 * 比旁边的「+」和文字更显眼一点。用 `calc` 而不是写死 12px，是为了在字号被
 * 容器查询压小时图标也跟着退回去，不至于撑破 1.9em 的按钮。
 */
function GearIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" className="h-[calc(1em+2px)] w-[calc(1em+2px)]">
      <path
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0 1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a6.759 6.759 0 0 1 0 .255c-.008.378.137.75.43.991l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 0 1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 0 1-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94h-2.594c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125 1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.429-.992a6.932 6.932 0 0 1 0-.255c.008-.378-.137-.75-.43-.991l-1.004-.828a1.125 1.125 0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.281zM15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0z"
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
