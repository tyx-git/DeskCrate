import type { CSSProperties } from 'react'

import { ROOT_ID, ROOT_LOCATION } from '../../lib/tree'
import type { AppEXE } from '../../types/catalog'
import { IconButton } from '../common/IconButton'
import { AppGrid } from './AppGrid'
import { useTreeActions } from './treeActions'

interface RootAppSectionProps {
  /** 顶层应用，按保存顺序渲染。 */
  apps: AppEXE[]
  /** 文件拖拽是否正悬停在顶层区。 */
  isDropTarget: boolean
  /** 是否正在拖拽文件；决定没有应用时是否显示落点提示。 */
  isDragging: boolean
}

/**
 * 顶层应用区。
 *
 * 分类只是收纳方式，不是必须的容器：最常用的那几个程序应该能一步放到侧栏
 * 最上面，不必为了放一个应用先建一个分类。这一块就挂在所有分类之前。
 *
 * 它同时是「顶层」这个拖放落点的实体 —— 只有这里有 `data-drop-id`，
 * 拖到分类之间的空白处不会被当成「放到顶层」，语义才清楚。
 */
export function RootAppSection({ apps, isDropTarget, isDragging }: RootAppSectionProps) {
  const { addAppTo } = useTreeActions()
  const showPlaceholder = apps.length === 0

  // `--dc-i` 取 0：收束动画里顶层区最先开始收，读起来是「最上面的先被抽走」。
  return (
    <section
      data-drop-id={ROOT_ID}
      style={{ '--dc-i': 0 } as CSSProperties}
      className={`dc-converge relative pt-[5%] transition-colors duration-100 ${
        isDropTarget ? 'bg-[rgb(58_96_190_/_0.14)]' : ''
      }`}
    >
      <span className="absolute top-[0.5%] right-[5%] flex items-center gap-[1cqw] opacity-0 transition-opacity duration-100 group-hover/panel:opacity-100 focus-within:opacity-100">
        <IconButton
          label="在顶层添加应用"
          onClick={() => addAppTo(ROOT_LOCATION)}
          className="text-caret hover:text-ink"
        >
          +
        </IconButton>
      </span>

      {showPlaceholder ? (
        // 拖拽时给出落点，否则这一块什么都没有 —— 顶层的应用是后来才有的，
        // 不放提示用户永远不知道这里能放东西。
        isDragging ? (
          <p className="dc-grid mx-[5%] mb-[2%] rounded-panel border border-dashed border-focus px-[5%] py-[4%] text-center text-chip text-ink-faint">
            拖到这里就放到顶层
          </p>
        ) : null
      ) : (
        <AppGrid apps={apps} />
      )}
    </section>
  )
}
