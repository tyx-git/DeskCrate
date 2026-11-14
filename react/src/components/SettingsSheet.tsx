// 设置抽屉：集中通用开关、外观、数据位置与关于，所有配置都在这里。
import { useSettingsStore } from '../state/settingsStore'
import { useUiStore } from '../state/uiStore'
import { Glyph } from './Glyph'

export function SettingsSheet() {
  const open = useUiStore((s) => s.settingsOpen)
  const closeSettings = useUiStore((s) => s.closeSettings)
  const settings = useSettingsStore((s) => s.settings)
  const panelSize = useSettingsStore((s) => s.panelSize)
  const setAutoCollapse = useSettingsStore((s) => s.setAutoCollapse)
  const setAutoStart = useSettingsStore((s) => s.setAutoStart)
  const resetWindowSize = useSettingsStore((s) => s.resetWindowSize)

  const sizeText = panelSize ? `${panelSize.width} × ${panelSize.height}` : '—'

  return (
    <aside className={`settings-sheet${open ? ' is-open' : ''}`} aria-label="设置" aria-hidden={!open}>
      <header className="sheet-head">
        <h2>设置</h2>
        <button className="icon-btn" aria-label="关闭设置" onClick={closeSettings}>
          <Glyph name="x" />
        </button>
      </header>
      <div className="sheet-body dc-scroll">
        <section className="set-group">
          <h3 className="set-eyebrow">通用</h3>
          <label className="set-row">
            <span className="set-text">
              <span className="set-name">自动收起</span>
              <span className="set-desc">鼠标离开侧栏后收为指示条</span>
            </span>
            <input
              type="checkbox"
              className="switch"
              checked={settings?.autoCollapse ?? false}
              onChange={(e) => void setAutoCollapse(e.target.checked)}
            />
          </label>
          <label className="set-row">
            <span className="set-text">
              <span className="set-name">开机自启动</span>
              <span className="set-desc">随 Windows 登录启动 · 写入 HKCU Run</span>
            </span>
            <input
              type="checkbox"
              className="switch"
              checked={settings?.autoStart ?? false}
              onChange={(e) => void setAutoStart(e.target.checked)}
            />
          </label>
        </section>

        <section className="set-group">
          <h3 className="set-eyebrow">外观</h3>
          <div className="set-row">
            <span className="set-text">
              <span className="set-name">主题</span>
              <span className="set-desc">当前版本仅提供默认主题</span>
            </span>
            <span className="theme-chip">
              <i className="theme-swatch" />
              柔白 · 青瓷
            </span>
          </div>
          <div className="set-row">
            <span className="set-text">
              <span className="set-name">窗口尺寸</span>
              <span className="set-desc mono">{sizeText}</span>
            </span>
            <button className="btn btn-sm" onClick={() => void resetWindowSize()}>
              恢复默认
            </button>
          </div>
        </section>

        <section className="set-group">
          <h3 className="set-eyebrow">数据</h3>
          <div className="set-row set-row-stack">
            <span className="set-name">存储位置</span>
            <span className="set-desc mono">&lt;程序目录&gt;\config\data.json</span>
          </div>
          <div className="set-row set-row-stack">
            <span className="set-name">图标缓存</span>
            <span className="set-desc mono">&lt;程序目录&gt;\icons\</span>
          </div>
        </section>

        <section className="set-group">
          <h3 className="set-eyebrow">关于</h3>
          <div className="set-row">
            <span className="set-text">
              <span className="set-name">DeskCrate</span>
              <span className="set-desc mono">v0.1.0 · 青瓷</span>
            </span>
            <span className="about-dot">
              <i className="status-dot" />
              运行中
            </span>
          </div>
        </section>
      </div>
    </aside>
  )
}