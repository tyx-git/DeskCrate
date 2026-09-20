import type {
  AppData,
  AppEXE,
  Category,
  Group,
} from '../types/catalog'
import type { Settings, ViewMode, WindowInitResponse } from '../types/settings'

/**
 * 前端用到的全部后端能力。
 *
 * 抽象成接口有两个目的：
 * 1. Tauri 与浏览器 mock 可以共用同一套 UI 代码；
 * 2. 组件只依赖这个接口，不直接散落 `invoke` 字符串。
 */
export interface Backend {
  /** 是否连接到真实的 Tauri 后端。 */
  readonly isTauri: boolean

  // ---- 窗口与配置 ----
  getWindowInit(): Promise<WindowInitResponse>
  applyViewMode(viewMode: ViewMode): Promise<WindowInitResponse>
  getSettings(): Promise<Settings>
  updateSettings(settings: Settings): Promise<Settings>

  // ---- 目录数据 ----
  getAllData(): Promise<AppData>
  addCategory(name: string): Promise<Category>
  renameCategory(id: string, name: string): Promise<Category>
  deleteCategory(id: string): Promise<void>
  addGroup(parentId: string, name: string): Promise<Group>
  renameGroup(id: string, name: string): Promise<Group>
  deleteGroup(id: string): Promise<void>
  /**
   * `parentId` 为 `null` 表示放到顶层，`rootIndex` 指定插到第几个根节点之前
   * （省略则追加到最下方）；放进分类或分组时忽略 `rootIndex`。
   */
  addApp(
    parentId: string | null,
    sourcePath: string,
    name?: string,
    rootIndex?: number,
  ): Promise<AppEXE>
  /**
   * 把一个应用移动到新位置（拖拽换位）。
   *
   * `parentId` 为 `null` 表示移到顶层；`beforeId` 是参照节点（插到它之前），
   * `null` 表示插到目标容器末尾。
   */
  moveApp(
    id: string,
    parentId: string | null,
    beforeId: string | null,
  ): Promise<void>
  renameApp(id: string, name: string): Promise<AppEXE>
  deleteApp(id: string): Promise<void>
  launchIcon(id: string): Promise<void>
}
