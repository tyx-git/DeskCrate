// 空态：邀请拖入，并提供添加应用与新建分类两个入口。
import { useUiStore } from '../state/uiStore'

export function EmptyState() {
  const openDialog = useUiStore((s) => s.openDialog)
  return (
    <div className="empty">
      <svg className="empty-mark" viewBox="0 0 14 14" aria-hidden="true">
        <rect x="0.9" y="0.9" width="5.4" height="5.4" fill="none" stroke="currentColor" strokeWidth="1.1" />
        <rect x="7.7" y="0.9" width="5.4" height="5.4" fill="none" stroke="currentColor" strokeWidth="1.1" />
        <rect x="0.9" y="7.7" width="5.4" height="5.4" fill="none" stroke="currentColor" strokeWidth="1.1" />
        <rect x="7.7" y="7.7" width="5.4" height="5.4" fill="currentColor" />
      </svg>
      <p className="empty-title">把应用拖进箱子</p>
      <p className="empty-desc">
        支持快捷方式、程序、文件夹与网址
        <br />
        也可以从下方开始
      </p>
      <div className="empty-actions">
        <button className="btn btn-accent" onClick={() => openDialog({ mode: 'add' })}>
          添加应用
        </button>
        <button className="btn" onClick={() => openDialog({ mode: 'new-category' })}>
          新建分类
        </button>
      </div>
    </div>
  )
}