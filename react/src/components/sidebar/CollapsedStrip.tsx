import { useSettingsStore } from '../../state/settingsStore'

/**
 * 收起态指示条。
 *
 * 设计稿：3×80px、柔白 60%、圆角 1.5px、外发光 6px。窗口本身已经缩到
 * 指示条大小，所以这里只需要填满容器并给出可点击的反馈。
 *
 * 注意：收起态窗口的内容盒只有 3px 宽，而展开侧栏的唯一途径就是点这条
 * 指示条（没有展开快捷键），3px 的命中区域实际上很难点中。窗口右侧本来
 * 就留着 16px 的透明阴影留白（透明窗口同样接收点击），这里借它把命中
 * 区域一路铺到窗口右边缘 —— 视觉尺寸不变，鼠标悬停到留白上时指示条会
 * 高亮，反过来提示这里可以点。这层留白是整个窗口最后一块「点了也没别的
 * 用处」的区域，铺满它等于「点窗口任意位置都能展开」，是收起态唯一的退路。
 */
export function CollapsedStrip() {
  const setViewMode = useSettingsStore((state) => state.setViewMode)

  return (
    <button
      type="button"
      aria-label="展开 DeskCrate 侧栏"
      title="展开 DeskCrate 侧栏"
      onClick={() => void setViewMode('expanded')}
      className="relative h-full w-full rounded-strip bg-strip shadow-strip transition-colors duration-150 hover:bg-[rgb(255_255_255_/_0.9)] active:bg-white"
    >
      <span aria-hidden className="absolute inset-y-0 left-0 -right-gutter" />
    </button>
  )
}
