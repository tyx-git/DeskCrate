import { AppIcon } from './AppIcon'
import type { AppEXE } from '../../types/catalog'
import { useTreeActions } from './treeActions'

interface AppGridProps {
  apps: AppEXE[]
}

/** 应用图标网格。间距与内边距来自设计稿的 `.icon-grid`。 */
export function AppGrid({ apps }: AppGridProps) {
  const { selectedAppId, launch, selectApp, contextMenuForApp } = useTreeActions()

  return (
    <div className="dc-grid px-[5%] pt-[2%] pb-[1%]">
      {apps.map((app) => (
        <AppIcon
          key={app.id}
          app={app}
          selected={selectedAppId === app.id}
          onLaunch={() => launch(app)}
          onSelect={() => selectApp(app)}
          onContextMenu={(event) => contextMenuForApp(event, app)}
        />
      ))}
    </div>
  )
}
