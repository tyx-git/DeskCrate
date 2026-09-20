import { Fragment } from 'react'

import { describeError } from '../../api'
import { asAppsRoot, asCategoryRoot } from '../../lib/tree'
import { useCatalogStore } from '../../state/catalogStore'
import { useFilteredData } from '../../state/selectors'
import { useUiStore } from '../../state/uiStore'
import { NoticeBar } from '../overlays/NoticeBar'
import { AppGrid } from './AppGrid'
import { CategorySection } from './CategorySection'
import { EmptyState } from './EmptyState'

interface SidebarBodyProps {
  /**
   * 文件拖拽悬停的插入位置（第几个根节点之前）。
   *
   * 分类/分组行自己的高亮由 `CategorySection` 通过 `useTreeActions` 读
   * `activeDropId`，这里只关心「插到两个根节点之间」这一种落点。
   */
  activeDropIndex: number | null
  /** 是否正在拖拽文件。 */
  isDragging: boolean
}

/**
 * 侧栏滚动区。
 *
 * 根节点是一个**有序列表**：顶层应用与分类交错排列，顺序即屏幕上的顺序。
 * 每两个根节点之间（含最上方与最下方）都有一条落点分界线，拖放文件时命中的
 * 那条会变色，指示「会插到这里」—— 与 Edge 收藏夹的拖放指示是同一套语言。
 */
export function SidebarBody({ activeDropIndex, isDragging }: SidebarBodyProps) {
  const status = useCatalogStore((state) => state.status)
  const error = useCatalogStore((state) => state.error)
  const load = useCatalogStore((state) => state.load)
  const query = useUiStore((state) => state.query)
  const filtered = useFilteredData()

  const total = filtered.items.length

  const renderDrop = (index: number) => {
    const active = activeDropIndex === index
    // 最后一条（「放到最下方」）平时藏起来，免得列表末尾凭空多出一条线；
    // 其余保持设计稿的分隔线观感，拖拽时全部显形，方便看清能插在哪。
    const visible = active || index < total || isDragging

    return (
      <div
        key={`drop-${index}`}
        data-drop-index={index}
        className="mx-[5%] flex h-[10px] items-center"
      >
        <span
          className={`h-px w-full transition-colors duration-100 ${
            active ? 'bg-drop-active' : visible ? 'bg-hairline' : 'bg-transparent'
          }`}
        />
      </div>
    )
  }

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
      ) : filtered.items.length > 0 ? (
        <>
          {renderDrop(0)}
          {filtered.items.map((item, index) => {
            const apps = asAppsRoot(item)
            const category = asCategoryRoot(item)

            return (
              <Fragment key={apps ? `apps-${apps[0]?.id ?? index}` : `cat-${category?.id ?? index}`}>
                {apps ? (
                  <AppGrid apps={apps} />
                ) : category ? (
                  <CategorySection category={category} index={index} />
                ) : null}
                {renderDrop(index + 1)}
              </Fragment>
            )
          })}
        </>
      ) : (
        <>
          {renderDrop(0)}
          <EmptyState query={query} loading={status === 'loading'} />
        </>
      )}
    </div>
  )
}
