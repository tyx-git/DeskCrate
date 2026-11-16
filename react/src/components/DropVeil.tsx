// 文件拖入时的全屏提示遮罩，由原生拖放事件驱动；内部 HTML5 拖拽不显示。
import { useUiStore } from '../state/uiStore'
import { Glyph } from './Glyph'

export function DropVeil() {
  const fileDragging = useUiStore((s) => s.fileDragging)
  const dragging = useUiStore((s) => s.drag)
  if (!fileDragging || dragging) return null
  return (
    <div className="drop-veil">
      <Glyph name="plus" />
      <p>松开以添加到侧栏</p>
    </div>
  )
}
