import { useRef } from 'react'

import { collectVisibleApps } from '../../lib/tree'
import { SEARCH_INPUT_ID } from '../../hooks/useHotkeys'
import { useCatalogStore } from '../../state/catalogStore'
import { useFilteredData } from '../../state/selectors'
import { useUiStore } from '../../state/uiStore'

/**
 * 搜索行。
 *
 * 白瓷稿里它是独立的一行：上面是品牌栏，下面是清单。输入槽做成浅浅的
 * 下沉面，聚焦时才出现靛蓝描边；未输入时右侧显示 Ctrl K 键帽，提醒可以
 * 随时把焦点叫回来。
 */
export function SidebarHeader() {
  const query = useUiStore((state) => state.query)
  const setQuery = useUiStore((state) => state.setQuery)
  const launchApp = useCatalogStore((state) => state.launchApp)
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
    <div className="dc-search dc-converge">
      <div className="dc-search__slot">
        <svg viewBox="0 0 12 12" aria-hidden="true" className="dc-search__icon">
          <circle
            cx="5.1"
            cy="5.1"
            r="3.1"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.3"
          />
          <path
            d="m7.5 7.5 2.6 2.6"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.3"
            strokeLinecap="round"
          />
        </svg>

        <input
          ref={inputRef}
          id={SEARCH_INPUT_ID}
          type="text"
          value={query}
          spellCheck={false}
          autoComplete="off"
          placeholder="搜索应用或网址…"
          aria-label="搜索应用或网址"
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
          className="dc-search__input"
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
            className="dc-search__clear"
          >
            ×
          </button>
        ) : (
          <kbd className="dc-search__key">Ctrl K</kbd>
        )}
      </div>
    </div>
  )
}
