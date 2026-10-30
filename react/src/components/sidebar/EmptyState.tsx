import { useUiStore } from '../../state/uiStore'

interface EmptyStateProps {
  /** 当前搜索词，用于区分「还没有分类」和「搜索无结果」。 */
  query: string
  /** 数据是否仍在加载。 */
  loading: boolean
}

/**
 * 空态。
 *
 * 白瓷稿里没有插画，只有一枚很轻的货箱标记、一句说明和一个墨色主按钮；
 * 搜索无结果时同一套结构，按钮换成「清空搜索」。
 */
export function EmptyState({ query, loading }: EmptyStateProps) {
  const openDialog = useUiStore((state) => state.openDialog)
  const setQuery = useUiStore((state) => state.setQuery)

  if (loading) {
    return <p className="dc-empty__text py-[8%] text-center">正在读取数据…</p>
  }

  if (query.trim().length > 0) {
    return (
      <div className="dc-empty">
        <p className="dc-empty__title">没有找到</p>
        <p className="dc-empty__text">没有匹配「{query.trim()}」的应用或网址。</p>
        <button type="button" onClick={() => setQuery('')} className="dc-btn-ink">
          清空搜索
        </button>
      </div>
    )
  }

  return (
    <div className="dc-empty">
      <svg viewBox="0 0 14 14" aria-hidden="true" className="dc-empty__mark">
        <rect
          x="0.9"
          y="0.9"
          width="5.4"
          height="5.4"
          fill="none"
          stroke="currentColor"
          strokeWidth="0.9"
        />
        <rect
          x="7.7"
          y="0.9"
          width="5.4"
          height="5.4"
          fill="none"
          stroke="currentColor"
          strokeWidth="0.9"
        />
        <rect
          x="0.9"
          y="7.7"
          width="5.4"
          height="5.4"
          fill="none"
          stroke="currentColor"
          strokeWidth="0.9"
        />
        <rect
          x="7.7"
          y="7.7"
          width="5.4"
          height="5.4"
          fill="none"
          stroke="currentColor"
          strokeWidth="0.9"
        />
      </svg>
      <p className="dc-empty__title">箱子是空的</p>
      <p className="dc-empty__text">把程序、文件夹或网址拖进来，或先建一个分类。</p>
      <button
        type="button"
        onClick={() => openDialog({ kind: 'new-category' })}
        className="dc-btn-ink"
      >
        新建分类
      </button>
    </div>
  )
}
