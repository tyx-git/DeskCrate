import type { ReactNode } from 'react'

import { TreeActionsContext, type TreeActions } from './treeActions'

export function TreeProvider({
  actions,
  children,
}: {
  actions: TreeActions
  children: ReactNode
}) {
  return (
    <TreeActionsContext.Provider value={actions}>{children}</TreeActionsContext.Provider>
  )
}
