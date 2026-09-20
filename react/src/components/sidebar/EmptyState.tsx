import { useUiStore } from '../../state/uiStore'

interface EmptyStateProps {
  /** 当前搜索词，用于区分「还没有分类」和「搜索无结果」。 */
  query: string
  /** 数据是否仍在加载。 */
  loading: boolean
}

export function EmptyState({ query, loading }: EmptyStateProps) {
  const openDialog = useUiStore((state) => state.openDialog)
  const setQuery = useUiStore((state) => state.setQuery)

  if (loading) {
    return (
      <p className="px-[5%] py-[6%] text-center text-chip text-ink-faint">
        正在读取数据…
      </p>
    )
  }

  if (query.trim().length > 0) {
    return (
      <div className="px-[5%] py-[6%] text-center text-chip text-ink-faint">
        <p className="break-words">没有匹配「{query.trim()}」的应用</p>
        <button
          type="button"
          onClick={() => setQuery('')}
          className="mt-[3%] text-caret underline underline-offset-2 transition-colors hover:text-ink"
        >
          清空搜索
        </button>
      </div>
    )
  }

  return (
    <div className="px-[6%] py-[8%] text-center text-chip text-ink-faint">
      <p className="text-cat-icon">📦</p>
      <p className="mt-[4%] leading-[1.5]">
        还没有任何内容。
        <br />
        直接把常用的程序和网址拖进来就会放到顶层，
        <br />
        也可以新建一个分类再往里拖。
      </p>
      <button
        type="button"
        onClick={() => openDialog({ kind: 'new-category' })}
        className="mt-[5%] rounded-[6px] bg-[rgb(0_0_0_/_0.06)] px-[4%] py-[2%] text-ink-soft transition-colors hover:bg-[rgb(0_0_0_/_0.1)]"
      >
        新建分类
      </button>
    </div>
  )
}
