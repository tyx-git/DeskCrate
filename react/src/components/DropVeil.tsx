// 文件拖入时的全屏提示遮罩，由原生拖放事件或预览的 HTML5 拖放驱动。
import { useUiStore } from '../state/uiStore'
import { Glyph } from './Glyph'

export function DropVeil() {
  const fileDragging = useUiStore((s) => s.fileDragging)
  if (!fileDragging) return null
  return (
    <div className="drop-veil">
      <Glyph name="plus" />
      <p>松开以添加到侧栏</p>
    </div>
  )
}