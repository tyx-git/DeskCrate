import { useAppDrag } from '../../hooks/useAppDrag'
import { useCatalogStore } from '../../state/catalogStore'
import { useUiStore } from '../../state/uiStore'
import type { AppEXE } from '../../types/catalog'
import { AppIcon } from './AppIcon'
import { useTreeActions } from './treeActions'

interface AppGridProps {
  apps: AppEXE[]
  /**
   * 这一行应用所在的容器：顶层为 `null`，否则是分类或分组的 ID。
   * 松手在网格空白处时按「追加到该容器」处理。
   */
  parentId?: string | null
}

/** 应用图标网格。间距与内边距来自设计稿的 `.icon-grid`。 */
export function AppGrid({ apps, parentId = null }: AppGridProps) {
  const { selectedAppId, launch, selectApp, contextMenuForApp } = useTreeActions()
  const data = useCatalogStore((state) => state.data)
  const moveApp = useCatalogStore((state) => state.moveApp)
  const moveGroup = useCatalogStore((state) => state.moveGroup)
  const draggingNode = useUiStore((state) => state.draggingNode)
  const setDraggingNode = useUiStore((state) => state.setDraggingNode)
  const setDropHint = useUiStore((state) => state.setDropHint)

  const drag = useAppDrag({ data, moveApp, moveGroup, setDraggingNode, setDropHint, draggingNode })

  return (
    <div
      className="dc-grid px-[5%] pt-[2%] pb-[1%]"
      onDragOver={(event) => {
        // 只有正在拖动应用时，空白处才表示追加到该容器末尾。
        if (draggingNode?.kind !== 'app') return
        event.preventDefault()
        event.dataTransfer.dropEffect = 'move'
      }}
      onDrop={(event) => {
        if (event.target !== event.currentTarget) return
        drag.onDropOnContainer(event, parentId)
      }}
    >
      {apps.map((app) => (
        <AppIcon
          key={app.id}
          app={app}
          selected={selectedAppId === app.id}
          dragging={draggingNode?.kind === 'app' && draggingNode.id === app.id}
          onLaunch={() => launch(app)}
          onSelect={() => selectApp(app)}
          onContextMenu={(event) => contextMenuForApp(event, app)}
          onDragStart={(event) => drag.onDragStartApp(event, app)}
          onDragOver={(event) => drag.onDragOverNode(event, { id: app.id, kind: 'app' })}
          onDrop={(event) => drag.onDropOnNode(event, { id: app.id, kind: 'app' })}
          onDragEnd={drag.onDragEnd}
        />
      ))}
    </div>
  )
}
