// 目录数据模型：与 src-tauri/src/models/catalog.rs 的 serde 形状一一对应。

/** 应用节点的来源与打开方式，序列化值与 Rust IconType 一致（PascalCase）。 */
export type IconType = 'Shortcut' | 'Executable' | 'Folder' | 'File' | 'Url'

/** 应用或快捷方式节点。 */
export interface AppEXE {
  id: string
  name: string
  /** 用户最初添加的路径或网址；快捷方式保留 .lnk 原路径。 */
  sourcePath: string
  arguments?: string
  workingDir?: string
  /** 可直接用于 <img src> 的资源 URL（asset 协议或内置 data: 图标）。 */
  iconUrl: string
  iconType: IconType
  createdTime: string
}

/** 编辑应用时提交的完整可编辑字段。 */
export interface UpdateAppInput {
  id: string
  name: string
  sourcePath: string
  arguments?: string
  workingDir?: string
}

/** 子分组，可继续嵌套分组或应用。 */
export interface Group {
  id: string
  name: string
  isCollapsed: boolean
  children: Item[]
  createdTime: string
}

/** 顶层分类。 */
export interface Category {
  id: string
  name: string
  isCollapsed: boolean
  groups: Item[]
  createdTime: string
}

/** Serde 外部标签：{ Group: {...} } 或 { App: {...} }。 */
export type Item = { Group: Group } | { App: AppEXE }

/** 根节点上的一段：一个分类，或一行顶层应用（两者交错排列）。 */
export type RootItem = { Category: Category } | { Apps: AppEXE[] }

export function isCategoryRoot(item: RootItem): item is { Category: Category } {
  return 'Category' in item
}

export function isAppsRoot(item: RootItem): item is { Apps: AppEXE[] } {
  return 'Apps' in item
}

export function isGroupItem(item: Item): item is { Group: Group } {
  return 'Group' in item
}

export function isAppItem(item: Item): item is { App: AppEXE } {
  return 'App' in item
}

/** data.json 的根结构；旧字段已在后端读取时归一化进 items。 */
export interface AppData {
  version: number
  items: RootItem[]
}

/** 批量导入中未能写入的一项。 */
export interface BatchAddFailure {
  sourcePath: string
  code: string
  message: string
}

/** 批量导入结果。 */
export interface AddAppsResponse {
  added: AppEXE[]
  failed: BatchAddFailure[]
}