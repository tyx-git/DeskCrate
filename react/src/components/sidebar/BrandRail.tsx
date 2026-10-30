import { countAllApps } from '../../lib/tree'
import { closeMainWindow } from '../../lib/window'
import { useCatalogStore } from '../../state/catalogStore'
import { useSettingsStore } from '../../state/settingsStore'
import { IconButton } from '../common/IconButton'

/** 品牌方块：两实两空的货箱格，强调色只出现在这里和计数上。 */
const MARK_PATH = (
  <>
    <rect x="0.9" y="0.9" width="5.4" height="5.4" fill="currentColor" />
    <rect
      x="7.7"
      y="0.9"
      width="5.4"
      height="5.4"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
    />
    <rect
      x="0.9"
      y="7.7"
      width="5.4"
      height="5.4"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
    />
    <rect x="7.7" y="7.7" width="5.4" height="5.4" fill="currentColor" />
  </>
)

export function BrandMark({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 14 14" aria-hidden="true" className={className}>
      {MARK_PATH}
    </svg>
  )
}

/**
 * 品牌栏。
 *
 * 窗口是无边框悬浮层，收起与退出没有系统栏按钮可用，统一放在这里：
 * 计数用强调色、操作常驻但压得很轻，鼠标进入时不需要再找按钮。
 */
export function BrandRail() {
  const total = useCatalogStore((state) => countAllApps(state.data))
  const setViewMode = useSettingsStore((state) => state.setViewMode)

  return (
    <header className="dc-brand dc-converge">
      <BrandMark className="dc-brand__mark" />
      <span className="dc-brand__word">DESKCRATE</span>
      <span className="dc-brand__meta">
        <b>{total}</b>
        <span>ITEMS</span>
      </span>
      <span className="dc-brand__actions">
        <IconButton
          label="收起侧栏"
          onClick={() => void setViewMode('collapsed')}
          className="text-[11px] opacity-80"
        >
          <svg viewBox="0 0 12 12" aria-hidden="true" className="h-[1em] w-[1em]">
            <path
              d="M6.6 3 3.6 6l3 3M10.4 3 7.4 6l3 3"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.4"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </IconButton>
        <IconButton
          label="退出 DeskCrate"
          tone="danger"
          onClick={() => void closeMainWindow()}
          className="text-[11px] opacity-80"
        >
          <svg viewBox="0 0 12 12" aria-hidden="true" className="h-[1em] w-[1em]">
            <path
              d="M3 3l6 6M9 3l-6 6"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.4"
              strokeLinecap="round"
            />
          </svg>
        </IconButton>
      </span>
    </header>
  )
}
