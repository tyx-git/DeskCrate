// 搜索结果列表：前缀优先排序、命中高亮、分类面包屑，空结果给出添加入口。
import { useEffect, useMemo, useRef } from 'react'

import { looksAddable, searchApps } from '../lib/search'
import { useCatalogStore } from '../state/catalogStore'
import { useUiStore } from '../state/uiStore'
import { PlateInner } from './AppPlate'

function Highlighted({ text, query }: { text: string; query: string }) {
  const q = query.trim().toLowerCase()
  const idx = text.toLowerCase().indexOf(q)
  if (idx < 0 || !q) return <>{text}</>
  return (
    <>
      {text.slice(0, idx)}
      <mark>{text.slice(idx, idx + q.length)}</mark>
      {text.slice(idx + q.length)}
    </>
  )
}

export function SearchResults() {
  const query = useUiStore((s) => s.query)
  const activeResult = useUiStore((s) => s.activeResult)
  const setActiveResult = useUiStore((s) => s.setActiveResult)
  const openDialog = useUiStore((s) => s.openDialog)
  const data = useCatalogStore((s) => s.data)
  const launchApp = useCatalogStore((s) => s.launchApp)
  const activeRef = useRef<HTMLButtonElement>(null)

  const results = useMemo(() => searchApps(data, query), [data, query])

  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: 'nearest' })
  }, [activeResult])

  const q = query.trim()
  if (results.length === 0) {
    const label = q.length > 18 ? `${q.slice(0, 18)}…` : q
    return (
      <div className="no-results">
        没有匹配「{q}」的应用
        <br />
        换个关键词，或把它添加进来
        {looksAddable(q) && (
          <>
            <br />
            <button className="btn btn-accent" onClick={() => openDialog({ mode: 'add', source: q })}>
              添加「{label}」
            </button>
          </>
        )}
      </div>
    )
  }

  return (
    <>
      <div className="results">
        {results.map(({ app, path }, i) => (
          <button
            key={app.id}
            ref={i === activeResult ? activeRef : undefined}
            className={`result-row${i === activeResult ? ' is-active' : ''}`}
            onMouseEnter={() => setActiveResult(i)}
            onClick={() => void launchApp(app)}
          >
            <span className={`result-plate${app.iconUrl === 'icon:pending' ? ' is-pending' : ''}`}>
              <PlateInner app={app} />
            </span>
            <span className="result-name">
              <Highlighted text={app.name} query={q} />
            </span>
            <span className="result-path">{path.length ? path.join(' › ') : '顶层'}</span>
            <span className="result-enter">↵</span>
          </button>
        ))}
      </div>
      <div className="results-hint">
        <span>↑↓ 选择</span>
        <span>↵ 启动</span>
        <span>Esc 清除</span>
        <span style={{ marginLeft: 'auto' }}>{results.length} 个结果</span>
      </div>
    </>
  )
}