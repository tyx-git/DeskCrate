import { useState } from 'react'

import { ICON_FALLBACK_GLYPH, ICON_TYPE_LABEL } from '../../lib/tree'
import { useUiStore } from '../../state/uiStore'
import type { AppEXE } from '../../types/catalog'

interface AppIconProps {
  app: AppEXE
  selected: boolean
  onLaunch: () => void
  onSelect: () => void
  onContextMenu: (event: React.MouseEvent<HTMLElement>) => void
  onDragStart: (event: React.DragEvent<HTMLElement>) => void
  dragging?: boolean
  onDragOver: (event: React.DragEvent<HTMLElement>) => void
  onDrop: (event: React.DragEvent<HTMLElement>) => void
  onDragEnd: () => void
}

/**
 * 单个应用图标。
 *
 * 图标地址由后端给出（asset 协议 URL 或内置 data URL）。加载失败时退回
 * 按来源类型取的表情符号，避免侧栏里出现破图。
 *
 * 图标本身可拖拽：拖到另一个图标的左半 / 右半，表示插到它前面 / 后面，被瞄准的
 * 那一侧会画出一条插入线（`.dc-drop-*`）。拖拽期间不触发启动，避免「想拖却打开
 * 了程序」。
 */
export function AppIcon({
  app,
  selected,
  onLaunch,
  onSelect,
  onContextMenu,
  onDragStart,
  dragging = false,
  onDragOver,
  onDrop,
  onDragEnd,
}: AppIconProps) {
  // 记录失败的是哪一个 URL，而不是一个永久布尔值。后台补图替换 URL 后，
  // 新资源自然会重新尝试加载，不需要在 effect 里同步 setState。
  const [failedUrl, setFailedUrl] = useState<string | null>(null)
  const showImage = app.iconUrl.length > 0 && failedUrl !== app.iconUrl

  const dropHint = useUiStore((state) => state.dropHint)
  const hint = dropHint?.nodeId === app.id ? dropHint.position : null

  const surface = selected
    ? 'bg-active ring-1 ring-inset ring-focus'
    : 'hover:bg-hover active:bg-active'

  return (
    <button
      type="button"
      data-app-id={app.id}
      draggable
      title={`${app.name}\n${ICON_TYPE_LABEL[app.iconType]}\n${app.sourcePath}`}
      onClick={onLaunch}
      onFocus={onSelect}
      onContextMenu={onContextMenu}
      onDragStart={onDragStart}
      onDragOver={onDragOver}
      onDrop={onDrop}
      onDragEnd={onDragEnd}
      className={`dc-app-icon relative flex w-icon-item shrink-0 flex-col items-center rounded-[6px] py-[0.6cqw] outline-none transition-colors duration-100 ${surface} ${
        dragging ? 'opacity-40' : ''
      } ${hint === 'before' ? 'dc-drop-before' : ''} ${
        hint === 'after' ? 'dc-drop-after' : ''
      }`}
    >
      <span className="flex h-icon-glyph w-icon-glyph items-center justify-center">
        {showImage ? (
          <img
            src={app.iconUrl}
            alt=""
            draggable={false}
            onError={() => setFailedUrl(app.iconUrl)}
            className="max-h-full max-w-full object-contain"
          />
        ) : (
          <span className="text-glyph leading-none">
            {ICON_FALLBACK_GLYPH[app.iconType]}
          </span>
        )}
      </span>
      <span className="dc-ellipsis mt-[3px] w-full text-center text-app leading-[1.2] text-ink-mute">
        {app.name}
      </span>
    </button>
  )
}
