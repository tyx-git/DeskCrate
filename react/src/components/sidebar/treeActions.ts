import { createContext, useContext } from 'react'

import type { NodeLocation } from '../../lib/tree'
import type { AppEXE } from '../../types/catalog'

/** 分类树的交互回调。集中在一处，避免逐层透传十几个 props。 */
export interface TreeActions {
  /** 键盘导航当前选中的应用。 */
  selectedAppId: string | null
  /** 文件拖拽当前悬停的节点 ID。 */
  activeDropId: string | null

  launch: (app: AppEXE) => void
  selectApp: (app: AppEXE) => void
  contextMenuForApp: (event: React.MouseEvent, app: AppEXE) => void
  contextMenuForNode: (event: React.MouseEvent, location: NodeLocation) => void
  toggleCollapse: (id: string, collapsed: boolean) => void
  /** 打开「添加应用」对话框，`parent` 是落点节点。 */
  addAppTo: (parent: NodeLocation) => void
}

export const TreeActionsContext = createContext<TreeActions | null>(null)

export function useTreeActions(): TreeActions {
  const actions = useContext(TreeActionsContext)
  if (!actions) {
    throw new Error('useTreeActions 必须在 TreeProvider 内部使用')
  }
  return actions
}
