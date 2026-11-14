// 搜索框：Ctrl+K 聚焦，↑↓ 在结果间移动，↵ 启动当前结果，清除按钮回到目录。
import { useMemo, type RefObject } from 'react'

import { searchApps } from '../lib/search'
import { useCatalogStore } from '../state/catalogStore'
import { useUiStore } from '../state/uiStore'
import { Glyph } from './Glyph'

export function SearchBox({ searchRef }: { searchRef: RefObject<HTMLInputElement | null> }) {
  const query = useUiStore((s) => s.query)
  const activeResult = useUiStore((s) => s.activeResult)
  const setQuery = useUiStore((s) => s.setQuery)
  const setActiveResult = useUiStore((s) => s.setActiveResult)
  const data = useCatalogStore((s) => s.data)
  const launchApp = useCatalogStore((s) => s.launchApp)

  const results = useMemo(() => searchApps(data, query), [data, query])

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      if (results.length === 0) return
      e.preventDefault()
      const delta = e.key === 'ArrowDown' ? 1 : -1
      setActiveResult((activeResult + delta + results.length) % results.length)
    } else if (e.key === 'Enter') {
      const hit = results[activeResult]
      if (hit) void launchApp(hit.app)
    }
  }

  return (
    <div className="search">
      <span className="search-glyph">
        <Glyph name="search" />
      </span>
      <input
        ref={searchRef}
        type="text"
        placeholder="搜索应用"
        autoComplete="off"
        spellCheck={false}
        aria-label="搜索应用"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={onKeyDown}
      />
      {query ? (
        <button
          className="search-clear"
          aria-label="清除搜索"
          onClick={() => {
            setQuery('')
            searchRef.current?.focus()
          }}
        >
          <Glyph name="x" />
        </button>
      ) : (
        <kbd className="search-kbd">Ctrl K</kbd>
      )}
    </div>
  )
}