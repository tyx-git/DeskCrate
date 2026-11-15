// 底部状态栏：数量统计、操作提示与右下角缩放把手（双击恢复默认）。
// 拖拽会话统一在 useWindowResize，这里只负责把 pointerdown 接到 'se'。
import { searchApps } from '../lib/search'
import { countAll } from '../lib/tree'
import { useCatalogStore } from '../state/catalogStore'
import { useSettingsStore } from '../state/settingsStore'
import { useUiStore } from '../state/uiStore'
import type { StartResize } from '../hooks/useWindowResize'

export function PanelFoot({ start }: { start: StartResize }) {
  const data = useCatalogStore((s) => s.data)
  const query = useUiStore((s) => s.query)

  const searching = query.trim().length > 0
  const { apps, cats } = countAll(data)
  const countText = searching ? `${searchApps(data, query).length} 个结果` : `${apps} 个应用 · ${cats} 个分类`
  const hint = searching ? '点击结果启动' : '右键管理 · 拖拽排序'

  return (
    <footer className="panel-foot">
      <span className="foot-count">{countText}</span>
      <span className="foot-hint">{hint}</span>
      <button
        className="resize-grip"
        title="拖拽调整尺寸 · 双击恢复默认"
        aria-label="调整窗口尺寸"
        onPointerDown={start('se')}
        onDoubleClick={() => void useSettingsStore.getState().resetWindowSize()}
      />
    </footer>
  )
}
