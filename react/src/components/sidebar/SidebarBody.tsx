import { Fragment } from 'react'

import { describeError } from '../../api'
import { ROOT_ID } from '../../lib/tree'
import { useCatalogStore } from '../../state/catalogStore'
import { useFilteredData } from '../../state/selectors'
import { useUiStore } from '../../state/uiStore'
import { NoticeBar } from '../overlays/NoticeBar'
import { CategorySection } from './CategorySection'
import { EmptyState } from './EmptyState'
import { RootAppSection } from './RootAppSection'

interface SidebarBodyProps {
  /** 文件拖拽当前悬停的节点 ID。 */
  activeDropId: string | null
  /** 是否正在拖拽文件。 */
  isDragging: boolean
}

/** 侧栏滚动区：提示条 + 顶层应用 + 分类列表 / 空状态 / 错误状态。 */
export function SidebarBody({ activeDropId, isDragging }: SidebarBodyProps) {
  const status = useCatalogStore((state) => state.status)
  const error = useCatalogStore((state) => state.error)
  const load = useCatalogStore((state) => state.load)
  const query = useUiStore((state) => state.query)
  const filtered = useFilteredData()

  // 顶层有应用、或者正在拖拽（需要给出落点）时才渲染顶层区。
  const showRoot = filtered.apps.length > 0 || isDragging
  const hasContent = showRoot || filtered.categories.length > 0

  return (
    <div className="dc-scroll min-h-0 flex-1 pb-[1%]">
      <NoticeBar />

      {status === 'error' ? (
        <div className="px-[5%] py-[5%] text-center text-chip text-danger">
          <p className="break-words">
            {error ? describeError(error) : '读取数据失败'}
          </p>
          <button
            type="button"
            onClick={() => void load()}
            className="mt-[3%] rounded-[6px] bg-[rgb(0_0_0_/_0.06)] px-[4%] py-[2%] text-ink-soft transition-colors hover:bg-[rgb(0_0_0_/_0.1)]"
          >
            重试
          </button>
        </div>
      ) : hasContent ? (
        <>
          {showRoot ? (
            <RootAppSection
              apps={filtered.apps}
              isDropTarget={activeDropId === ROOT_ID}
              isDragging={isDragging}
            />
          ) : null}

          {filtered.categories.map((category, index) => (
            <Fragment key={category.id}>
              {/* 设计稿在每个分类前放一条分隔线，包括第一个分类。 */}
              <div className="mx-[5%] my-[1.5%] h-px bg-hairline" />
              <CategorySection category={category} index={index} />
            </Fragment>
          ))}
        </>
      ) : (
        <EmptyState query={query} loading={status === 'loading'} />
      )}
    </div>
  )
}
