import { useRef } from 'react'

import { collectVisibleApps } from '../../lib/tree'
import { closeMainWindow } from '../../lib/window'
import { SEARCH_INPUT_ID } from '../../hooks/useHotkeys'
import { useCatalogStore } from '../../state/catalogStore'
import { useFilteredData } from '../../state/selectors'
import { useSettingsStore } from '../../state/settingsStore'
import { useUiStore } from '../../state/uiStore'
import { IconButton } from '../common/IconButton'

/**
 * 侧栏头部：搜索框 + 窗口操作。
 *
 * 设计稿里只有一个通栏搜索框。因为窗口是无边框悬浮层、又不进任务栏，
 * 必须提供退出入口，所以这里在搜索框右侧补了两个按钮，默认透明，鼠标
 * 进入侧栏时才淡入，静止观感与设计稿一致。
 */
export function SidebarHeader() {
  const query = useUiStore((state) => state.query)
  const setQuery = useUiStore((state) => state.setQuery)
  const launchApp = useCatalogStore((state) => state.launchApp)
  const setViewMode = useSettingsStore((state) => state.setViewMode)
  const filtered = useFilteredData()
  const inputRef = useRef<HTMLInputElement>(null)

  /** 回车启动当前搜索结果里的第一个应用。 */
  const launchFirstMatch = () => {
    const [first] = collectVisibleApps(filtered)
    if (first) {
      void launchApp(first)
    }
  }

  return (
    <div className="dc-converge flex items-center gap-[1.4cqw] px-[4%] pt-[2%] pb-[1.5%]">
      <div className="flex min-w-0 flex-1 items-center gap-[1.4cqw] rounded-field border border-field-line bg-field px-field-x py-field-y text-field text-ink-faint shadow-field">
        <span aria-hidden="true" className="shrink-0 leading-none opacity-80">
          🔍
        </span>
        <input
          ref={inputRef}
          id={SEARCH_INPUT_ID}
          type="text"
          value={query}
          spellCheck={false}
          autoComplete="off"
          placeholder="搜索应用..."
          aria-label="搜索应用"
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault()
              launchFirstMatch()
            }
            if (event.key === 'Escape') {
              event.preventDefault()
              if (query.length > 0) {
                setQuery('')
              } else {
                inputRef.current?.blur()
              }
            }
          }}
          className="min-w-0 flex-1 bg-transparent text-ink outline-none placeholder:text-ink-faint"
        />
        {query.length > 0 ? (
          <button
            type="button"
            aria-label="清空搜索"
            title="清空搜索"
            onClick={() => {
              setQuery('')
              inputRef.current?.focus()
            }}
            className="shrink-0 leading-none opacity-70 transition-opacity hover:opacity-100"
          >
            ×
          </button>
        ) : null}
      </div>

      <div className="flex shrink-0 items-center gap-[0.5cqw] text-field opacity-0 transition-opacity duration-150 group-hover/panel:opacity-100 focus-within:opacity-100">
        <IconButton label="收起侧栏" onClick={() => void setViewMode('collapsed')}>
          <svg viewBox="0 0 12 12" aria-hidden="true" className="h-[1em] w-[1em]">
            <path
              d="M7.5 2.5 L4 6 L7.5 9.5"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.6"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </IconButton>
        <IconButton label="退出 DeskCrate" tone="danger" onClick={() => void closeMainWindow()}>
          <svg viewBox="0 0 12 12" aria-hidden="true" className="h-[1em] w-[1em]">
            <path
              d="M3 3 L9 9 M9 3 L3 9"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.6"
              strokeLinecap="round"
            />
          </svg>
        </IconButton>
      </div>
    </div>
  )
}
