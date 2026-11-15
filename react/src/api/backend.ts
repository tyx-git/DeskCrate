// 前端用到的全部后端能力，与 src-tauri 的命令一一对应。
import type { AddAppsResponse, AppData, AppEXE, Category, Group } from '../types/catalog'
import type { DropTarget } from '../types/drop'
import type { Settings, Theme, ViewMode, WindowGeometry, WindowInitResponse } from '../types/settings'

/** 系统文件拖入事件；position 为原生物理像素。 */
export interface FileDropEvent {
  type: 'enter' | 'over' | 'drop' | 'leave'
  paths: string[]
  position: { x: number; y: number }
}

/** 网址图标抓取诊断的一步结果（仅 debug 构建可用）。 */
export interface UrlIconProbeStep {
  stage: string
  url: string
  status: number | null
  bytes: number
  kind: string | null
  note: string
}

/** 网址图标抓取诊断报告（仅 debug 构建可用）。 */
export interface UrlIconProbe {
  url: string
  ok: boolean
  stage: string
  totalMs: number
  steps: UrlIconProbeStep[]
}

export interface Backend {
  // ---- 窗口与配置 ----
  getWindowInit(): Promise<WindowInitResponse>
  applyViewMode(viewMode: ViewMode): Promise<WindowInitResponse>
  /** 实时改窗口几何（CSS 像素），拖拽期间连续调用。 */
  resizeWindow(geometry: WindowGeometry): Promise<void>
  /** 松手时补写被节流的最后一次尺寸。 */
  finishWindowResize(): Promise<void>
  /** 显示主窗口（tauri.conf.json 里初始 hidden）。 */
  showWindow(): Promise<void>
  /** 把焦点交给主窗口。 */
  focusWindow(): Promise<void>
  getSettings(): Promise<Settings>
  /** 全量快照兼容接口，带 revision 守卫；UI 默认走字段级命令。 */
  updateSettings(settings: Settings): Promise<Settings>
  updateAutoCollapse(enabled: boolean): Promise<Settings>
  updateAutoStart(enabled: boolean): Promise<Settings>
  /** 主题切换；当前只有默认主题，UI 暂不调用。 */
  updateTheme(theme: Theme): Promise<Settings>
  clearWindowSizeOverride(): Promise<Settings>

  // ---- 目录数据 ----
  getAllData(): Promise<AppData>
  addCategory(name: string): Promise<Category>
  renameCategory(id: string, name: string): Promise<Category>
  deleteCategory(id: string): Promise<void>
  addGroup(parentId: string, name: string): Promise<Group>
  renameGroup(id: string, name: string): Promise<Group>
  deleteGroup(id: string): Promise<void>
  setNodeCollapsed(id: string, isCollapsed: boolean): Promise<void>
  /** parentId 为 null 表示顶层；rootIndex 指定插到第几个根节点之前。 */
  addApp(parentId: string | null, sourcePath: string, name?: string, rootIndex?: number): Promise<AppEXE>
  addApps(parentId: string | null, sourcePaths: string[], rootIndex?: number): Promise<AddAppsResponse>
  /** beforeId 是参照节点（插到它之前），null 表示目标容器末尾。 */
  moveApp(id: string, parentId: string | null, beforeId: string | null): Promise<void>
  /** 分组只允许在原父容器内同级重排。 */
  moveGroup(id: string, parentId: string, beforeId: string | null): Promise<void>
  /** 拖拽应用；后端根据落点完成删/插与根下标校正。 */
  dropApp(id: string, target: DropTarget): Promise<void>
  /** 拖拽分组；后端只允许同父级重排。 */
  dropGroup(id: string, target: DropTarget): Promise<void>
  /** 拖拽分类；后端完成根列表重排。 */
  dropCategory(id: string, target: DropTarget): Promise<void>
  renameApp(id: string, name: string): Promise<AppEXE>
  deleteApp(id: string): Promise<void>
  launchIcon(id: string): Promise<void>

  /** 网址图标后台补齐事件（dc://icon-ready），返回取消监听函数。 */
  onIconReady(listener: (payload: { appId: string; iconUrl: string }) => void): Promise<() => void>
  /** 系统文件拖入（原生拖放事件），返回取消监听函数。 */
  onFileDrop(listener: (event: FileDropEvent) => void): Promise<() => void>
  /** 网址图标抓取诊断，仅 debug 构建注册。 */
  probeUrlIcon(url: string): Promise<UrlIconProbe>
}