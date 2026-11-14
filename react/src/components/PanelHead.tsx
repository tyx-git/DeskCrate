// 面板头部：品牌标识、运行状态点与添加/设置/收起三个入口。
import { useSettingsStore } from '../state/settingsStore'
import { useUiStore } from '../state/uiStore'
import { Glyph } from './Glyph'

export function PanelHead() {
  const openDialog = useUiStore((s) => s.openDialog)
  const openSettings = useUiStore((s) => s.openSettings)
  const setViewMode = useSettingsStore((s) => s.setViewMode)

  return (
    <header className="panel-head">
      <div className="brand">
        <svg className="brand-mark" viewBox="0 0 14 14" aria-hidden="true">
          <rect x="0.9" y="0.9" width="5.4" height="5.4" fill="currentColor" />
          <rect x="7.7" y="0.9" width="5.4" height="5.4" fill="none" stroke="currentColor" strokeWidth="1.6" />
          <rect x="0.9" y="7.7" width="5.4" height="5.4" fill="none" stroke="currentColor" strokeWidth="1.6" />
          <rect x="7.7" y="7.7" width="5.4" height="5.4" fill="currentColor" />
        </svg>
        <span className="brand-name">DeskCrate</span>
      </div>
      <div className="head-actions">
        <span className="status-dot" title="运行中 · 单实例防护" aria-label="运行中" />
        <button className="icon-btn" title="添加应用或分类" aria-label="添加" onClick={() => openDialog({ mode: 'add' })}>
          <Glyph name="plus" />
        </button>
        <button className="icon-btn" title="设置" aria-label="设置" onClick={openSettings}>
          <Glyph name="gear" />
        </button>
        <button className="icon-btn" title="收起侧栏" aria-label="收起侧栏" onClick={() => void setViewMode('collapsed')}>
          <Glyph name="chev-left" />
        </button>
      </div>
    </header>
  )
}