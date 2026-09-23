/**
 * 目录数据模型。
 *
 * 与 `src-tauri/src/models/catalog.rs` 一一对应。后端使用 `rename_all =
 * "camelCase"`，所以这里全部是 camelCase 字段名；改 Rust 模型时必须同步这里。
 */

/** 应用节点的来源与打开方式。序列化值与 Rust 的 `IconType` 保持一致。 */
export type IconType = 'Shortcut' | 'Executable' | 'Folder' | 'File' | 'Url';

/** 应用或快捷方式节点。 */
export interface AppEXE {
  /** 后端生成的稳定 UUID，重命名、删除、启动都必须用它定位。 */
  id: string;
  /** 展示名称，可由用户覆盖。 */
  name: string;
  /** 用户最初添加的路径或网址；快捷方式保留 `.lnk` 原路径。 */
  sourcePath: string;
  /** 快捷方式或启动目标的原始参数。 */
  arguments?: string;
  /** 可选的启动工作目录。 */
  workingDir?: string;
  /** 可直接用于 `<img src>` 的资源 URL。 */
  iconUrl: string;
  iconType: IconType;
  /** 创建时间，RFC 3339 字符串。 */
  createdTime: string;
}

/** 子分组，可以继续嵌套分组或应用。 */
export interface Group {
  id: string;
  name: string;
  isCollapsed: boolean;
  children: Item[];
  createdTime: string;
}

/** 顶层分类。 */
export interface Category {
  id: string;
  name: string;
  isCollapsed: boolean;
  /** 分类下的分组或应用节点，允许应用直接挂在分类上。 */
  groups: Item[];
  createdTime: string;
}

/**
 * 分类树中的通用节点。
 *
 * Rust 使用 Serde 外部标签，所以 JSON 形状是 `{ "Group": {...} }` 或
 * `{ "App": {...} }`。判别时请用 `isGroupItem` / `isAppItem`。
 */
export type Item = { Group: Group } | { App: AppEXE };

/**
 * 根节点上的一段：**要么是一个分类，要么是一行顶层应用**。
 *
 * 一行里可以有任意多个顶层应用，渲染时合并成一个网格。
 */
export type RootItem = { Category: Category } | { Apps: AppEXE[] };

/** 判断根节点是不是分类。 */
export function isCategoryRoot(item: RootItem): item is { Category: Category } {
  return 'Category' in item;
}

/** 判断根节点是不是一行顶层应用。 */
export function isAppsRoot(item: RootItem): item is { Apps: AppEXE[] } {
  return 'Apps' in item;
}

/** `data.json` 的根结构。 */
export interface AppData {
  version: number;
  /**
   * 根节点的有序列表。顶层应用与分类**交错排列**，顺序即屏幕上的顺序。
   *
   * 后端字段带 `serde(default)`，旧数据（只有 `categories` / `apps`）会在读取时
   * 被折成这个形状，前端只会看到 `items` 一种形态。
   */
  items: RootItem[];
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

/** 判断节点是否为分组。 */
export function isGroupItem(item: Item): item is { Group: Group } {
  return 'Group' in item;
}

/** 判断节点是否为应用。 */
export function isAppItem(item: Item): item is { App: AppEXE } {
  return 'App' in item;
}

/**
 * 取出节点携带的实际数据。
 *
 * 把外部标签的拆包集中在一处，避免每个组件各写一遍 `'Group' in item`。
 */
export function unwrapItem(item: Item): Group | AppEXE {
  return isGroupItem(item) ? item.Group : item.App;
}

/** 判断节点是否为分组，并同时取出分组数据。 */
export function asGroup(item: Item): Group | null {
  return isGroupItem(item) ? item.Group : null;
}

/** 判断节点是否为应用，并同时取出应用数据。 */
export function asApp(item: Item): AppEXE | null {
  return isAppItem(item) ? item.App : null;
}
