// 搜索框：输入筛选，清除按钮回到目录。
import { type RefObject } from 'react'

import { useUiStore } from '../state/uiStore'
import { Glyph } from './Glyph'

export function SearchBox({ searchRef }: { searchRef: RefObject<HTMLInputElement | null> }) {
  const query = useUiStore((s) => s.query)
  const setQuery = useUiStore((s) => s.setQuery)

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
      ) : null}
    </div>
  )
}
