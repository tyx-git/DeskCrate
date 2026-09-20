import { locateNode, type NodeLocation } from '../../lib/tree'
import { useCatalogStore } from '../../state/catalogStore'
import { useUiStore } from '../../state/uiStore'
import type { AppEXE } from '../../types/catalog'
import { SidebarBody } from './SidebarBody'
import { SidebarFooter } from './SidebarFooter'
import { SidebarHeader } from './SidebarHeader'
import { TreeProvider } from './TreeProvider'
import type { TreeActions } from './treeActions'

interface SidebarProps {
  /** 文件拖拽当前悬停的节点 ID，由外壳的 `useFileDrop` 提供。 */
  activeDropId: string | null
  /** 是否正在拖拽文件；顶层区靠它决定是否临时显示落点。 */
  isDragging: boolean
}

/** 展开态侧栏：玻璃面板 + 头部 + 滚动区 + 底栏。 */
export function Sidebar({ activeDropId, isDragging }: SidebarProps) {
  const data = useCatalogStore((state) => state.data)
  const launchApp = useCatalogStore((state) => state.launchApp)
  const toggleCollapse = useCatalogStore((state) => state.toggleCollapse)

  const selectedAppId = useUiStore((state) => state.selectedAppId)
  const setSelectedAppId = useUiStore((state) => state.setSelectedAppId)
  const openContextMenu = useUiStore((state) => state.openContextMenu)
  const openDialog = useUiStore((state) => state.openDialog)

  const actions: TreeActions = {
    selectedAppId,
    activeDropId,

    launch: (app) => void launchApp(app),

    selectApp: (app) => setSelectedAppId(app.id),

    contextMenuForApp: (event, app: AppEXE) => {
      event.preventDefault()
      event.stopPropagation()
      setSelectedAppId(app.id)
      openContextMenu({
        x: event.clientX,
        y: event.clientY,
        target: locateNode(data, app.id),
      })
    },

    contextMenuForNode: (event, location: NodeLocation) => {
      event.preventDefault()
      event.stopPropagation()
      openContextMenu({ x: event.clientX, y: event.clientY, target: location })
    },

    toggleCollapse,

    addAppTo: (parent) =>
      openDialog({ kind: 'add-app', targetId: parent.id, targetName: parent.name }),
  }

  return (
    <TreeProvider actions={actions}>
      <div className="group/panel flex h-full w-full flex-col overflow-hidden rounded-panel bg-panel shadow-panel backdrop-blur-glass">
        <SidebarHeader />
        <SidebarBody activeDropId={activeDropId} isDragging={isDragging} />
        <SidebarFooter />
      </div>
    </TreeProvider>
  )
}
