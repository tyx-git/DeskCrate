import { useEffect } from 'react'

import { useSettingsStore } from '../../state/settingsStore'
import { useUiStore } from '../../state/uiStore'
import { IconButton } from '../common/IconButton'
import { Switch } from '../common/Switch'
import { BrandMark } from './BrandRail'

/**
 * 设置页。
 *
 * **以后所有设置项都放这里** —— 底栏只有一行高，加一项就挤一次。
 * 白瓷稿按「行为 / 窗口 / 快捷键 / 关于」分节：设置项是行，说明在下方，
 * 开关靠右；快捷键与关于是静态信息，也用同一套发丝线。
 */
export function SettingsPanel() {
  const autoCollapse = useSettingsStore((state) => state.settings?.autoCollapse ?? false)
  const autoStart = useSettingsStore((state) => state.settings?.autoStart ?? false)
  const setAutoCollapse = useSettingsStore((state) => state.setAutoCollapse)
  const setAutoStart = useSettingsStore((state) => state.setAutoStart)
  const resetWindowSize = useSettingsStore((state) => state.resetWindowSize)
  const setSettingsOpen = useUiStore((state) => state.setSettingsOpen)

  // Esc 退出设置页，与对话框、右键菜单的行为保持一致。
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation()
        setSettingsOpen(false)
      }
    }
    window.addEventListener('keydown', onKeyDown, true)
    return () => window.removeEventListener('keydown', onKeyDown, true)
  }, [setSettingsOpen])

  return (
    <div className="flex h-full min-h-0 w-full flex-col">
      <header className="dc-brand">
        <IconButton label="返回" onClick={() => setSettingsOpen(false)} className="text-[11px] text-ink-soft">
          <svg viewBox="0 0 12 12" aria-hidden="true" className="h-[1em] w-[1em]">
            <path
              d="M7 2.5 3.5 6 7 9.5"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.4"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </IconButton>
        <span className="dc-brand__word">设置</span>
        <span className="dc-brand__meta">
          <span>CONFIG</span>
        </span>
      </header>

      <div className="dc-scroll dc-settings-scroll">
        <section>
          <h2 className="dc-eyebrow">
            行为<span>BEHAVIOR</span>
          </h2>
          <Switch
            checked={autoCollapse}
            onChange={(next) => void setAutoCollapse(next)}
            label="自动收起"
            description="鼠标离开侧栏后自动收成指示条；关闭则一直保持展开。"
          />
          <Switch
            checked={autoStart}
            onChange={(next) => void setAutoStart(next)}
            label="开机自启动"
            description="登录 Windows 后自动启动 DeskCrate。"
          />
        </section>

        <section>
          <h2 className="dc-eyebrow">
            窗口<span>WINDOW</span>
          </h2>
          <div className="dc-setrow">
            <span className="dc-setting__text">
              <b>窗口尺寸</b>
              <small>拖动侧栏边缘调整尺寸。</small>
            </span>
            <span className="dc-setvalue">342 × 528</span>
            <button type="button" onClick={() => void resetWindowSize()} className="dc-btn-ghost">
              重置
            </button>
          </div>
        </section>

        <section>
          <h2 className="dc-eyebrow">
            快捷键<span>SHORTCUTS</span>
          </h2>
          <dl className="dc-keys">
            <div>
              <dt>搜索侧栏</dt>
              <dd>
                <kbd className="dc-kbd">Ctrl</kbd>
                <kbd className="dc-kbd">K</kbd>
              </dd>
            </div>
            <div>
              <dt>在应用间移动</dt>
              <dd>
                <kbd className="dc-kbd">↑</kbd>
                <kbd className="dc-kbd">↓</kbd>
              </dd>
            </div>
            <div>
              <dt>重命名选中项</dt>
              <dd>
                <kbd className="dc-kbd">F2</kbd>
              </dd>
            </div>
            <div>
              <dt>删除选中项</dt>
              <dd>
                <kbd className="dc-kbd">Del</kbd>
              </dd>
            </div>
            <div>
              <dt>关闭浮层</dt>
              <dd>
                <kbd className="dc-kbd">Esc</kbd>
              </dd>
            </div>
          </dl>
        </section>

        <section>
          <h2 className="dc-eyebrow">
            关于<span>ABOUT</span>
          </h2>
          <div className="dc-about">
            <BrandMark className="dc-about__mark" />
            <div className="dc-about__text">
              <b>DeskCrate 0.1.0</b>
              <small>本地优先 · 数据保存在程序目录</small>
            </div>
          </div>
        </section>
      </div>

      <footer className="dc-settings-foot">ESC 返回侧栏</footer>
    </div>
  )
}
