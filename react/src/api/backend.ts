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
  /** `parentId` 为 `null` 表示直接放到顶层。 */
  addApp(
    parentId: string | null,
    sourcePath: string,
    name?: string,
  ): Promise<AppEXE>
  renameApp(id: string, name: string): Promise<AppEXE>
  deleteApp(id: string): Promise<void>
  launchIcon(id: string): Promise<void>
}
