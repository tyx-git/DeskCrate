import { useState } from 'react'

import { ICON_FALLBACK_GLYPH, ICON_TYPE_LABEL } from '../../lib/tree'
import type { AppEXE } from '../../types/catalog'

interface AppIconProps {
  app: AppEXE
  selected: boolean
  onLaunch: () => void
  onSelect: () => void
  onContextMenu: (event: React.MouseEvent<HTMLElement>) => void
}

/**
 * 单个应用图标。
 *
 * 图标地址由后端给出（asset 协议 URL 或内置 data URL）。加载失败时退回
 * 按来源类型取的表情符号，避免侧栏里出现破图。
 */
export function AppIcon({
  app,
  selected,
  onLaunch,
  onSelect,
  onContextMenu,
}: AppIconProps) {
  const [failed, setFailed] = useState(false)
  const showImage = app.iconUrl.length > 0 && !failed

  const surface = selected
    ? 'bg-active ring-1 ring-inset ring-focus'
    : 'hover:bg-hover active:bg-active'

  return (
    <button
      type="button"
      data-app-id={app.id}
      title={`${app.name}\n${ICON_TYPE_LABEL[app.iconType]}\n${app.sourcePath}`}
      onClick={onLaunch}
      onFocus={onSelect}
      onContextMenu={onContextMenu}
      className={`flex w-icon-item shrink-0 flex-col items-center rounded-[6px] py-[0.6cqw] transition-colors duration-100 outline-none ${surface}`}
    >
      <span className="flex h-icon-glyph w-icon-glyph items-center justify-center">
        {showImage ? (
          <img
            src={app.iconUrl}
            alt=""
            draggable={false}
            onError={() => setFailed(true)}
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
