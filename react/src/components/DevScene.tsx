import { useEffect, useState, type ReactNode } from 'react'

import { previewWindowBox, type PreviewBox } from '../lib/previewGeometry'
import { useSettingsStore } from '../state/settingsStore'

/**
 * 浏览器预览场景。
 *
 * 只在非 Tauri 环境使用：铺一层桌面壁纸，把侧栏按设计稿的百分比位置和
 * 尺寸摆上去，右上角标注当前状态与窗口尺寸。这样 `npm run dev` 里看到的
 * 效果和真实悬浮窗一致，可以直接和设计稿对照。
 */
export function DevScene({ children }: { children: ReactNode }) {
  const viewMode = useSettingsStore((state) => state.viewMode)
  const box = usePreviewBox(viewMode)

  return (
    <div className="dc-devscene fixed inset-0 overflow-hidden">
      <div
        className="absolute"
        style={{ left: box.left, top: box.top, width: box.width, height: box.height }}
      >
        {children}
      </div>

      <div className="dc-devscene__badge pointer-events-none absolute right-[3%] top-[3%] rounded-[6px] px-[14px] py-[6px] text-[12px]">
        <strong className="font-semibold text-ink">
          {viewMode === 'collapsed' ? '收起态' : '展开态'}
        </strong>
        <span>
          {' '}
          — {Math.round(box.width)}×{Math.round(box.height)} · 浏览器预览
        </span>
      </div>
    </div>
  )
}

/** 跟随视口尺寸重算预览盒子，验证不同分辨率下的等比缩放。 */
function usePreviewBox(viewMode: 'expanded' | 'collapsed'): PreviewBox {
  const [box, setBox] = useState(() =>
    previewWindowBox(window.innerWidth, window.innerHeight, viewMode),
  )

  useEffect(() => {
    const update = () =>
      setBox(previewWindowBox(window.innerWidth, window.innerHeight, viewMode))

    update()
    window.addEventListener('resize', update)
    return () => window.removeEventListener('resize', update)
  }, [viewMode])

  return box
}
