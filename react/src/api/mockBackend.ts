import { findApp, findGroup, rootCategories } from '../lib/tree'
import {
  type AppData,
  type AppEXE,
  type AddAppsResponse,
  type Category,
  type Group,
  type IconType,
  type Item,
  type RootItem,
} from '../types/catalog'
import type { CssSize, Settings, ViewMode, WindowInitResponse, WindowSize } from '../types/settings'
import type { Backend } from './backend'
import {
  appendCategory as appendCategoryData,
  appendChild as appendChildData,
  removeCategory as removeCategoryData,
  insertApp as insertAppData,
  moveApp as moveAppData,
  moveGroup as moveGroupData,
  removeApp as removeAppData,
  removeGroup as removeGroupData,
  renameApp as renameAppData,
  renameCategory as renameCategoryData,
  renameGroup as renameGroupData,
  toggleCollapse as toggleCollapseData,
} from '../lib/catalogMutations'

/**
 * 浏览器预览用的内存后端。
 *
 * 存在的意义是让 UI 可以在没有 Rust 工具链、没有 Tauri 运行时的机器上
 * 直接 `npm run dev` 验收。它刻意复刻了后端的关键语义：UUID 由「后端」
 * 生成、名称去空格后不能为空、分组嵌套不超过 4 层、删除返回 void。
 * 数据写入 localStorage，刷新页面不会丢。
 */
// 目录树增删改和移动规则复用 `lib/catalogMutations.ts`；这里仅保留浏览器
// 预览所需的示例数据、localStorage、延迟和图标占位行为。

const STORAGE_KEY = 'deskcrate.mock.data'
const SETTINGS_KEY = 'deskcrate.mock.settings'

/** 设计稿里的示例数据，用来直观对照 `dosc/plan/展示页.html`。 */
function seedData(): AppData {
  const now = new Date().toISOString()

  const appEXE = (
    id: string,
    name: string,
    sourcePath: string,
    iconType: IconType,
    glyph: string,
  ): AppEXE => ({
    id,
    name,
    sourcePath,
    iconUrl: emojiIcon(glyph),
    iconType,
    createdTime: now,
  })

  const app = (
    id: string,
    name: string,
    sourcePath: string,
    iconType: IconType,
    glyph: string,
  ): Item => ({ App: appEXE(id, name, sourcePath, iconType, glyph) })

  const group = (
    id: string,
    name: string,
    children: Item[],
    isCollapsed = false,
  ): Item => ({
    Group: { id, name, isCollapsed, children, createdTime: now },
  })

  const category = (
    id: string,
    name: string,
    groups: Item[],
    isCollapsed = false,
  ): RootItem => ({
    Category: { id, name, isCollapsed, groups, createdTime: now },
  })

  return {
    version: 2,
    items: [
      // 顶层应用与分类交错排列，顺序就是屏幕上的顺序。
      {
        Apps: [
          appEXE('app-root-explorer', '此电脑', 'C:\\Windows\\explorer.exe', 'Folder', '🖥️'),
          appEXE('app-root-github', 'GitHub', 'https://github.com', 'Url', '🐙'),
          appEXE('app-root-terminal', '终端', 'C:\\Windows\\System32\\cmd.exe', 'Executable', '⌨️'),
        ],
      },
      category('cat-common', '常用', [
        app('app-chrome', 'Chrome', 'https://www.google.com', 'Url', '🌐'),
        app('app-firefox', 'Firefox', 'https://www.mozilla.org', 'Url', '🦊'),
        app('app-explorer', '资源管理器', 'C:\\Windows\\explorer.exe', 'Folder', '📁'),
        app('app-wechat', 'WeChat', 'C:\\Program Files\\WeChat\\WeChat.exe', 'Executable', '💬'),
        app('app-vscode', 'VS Code', 'C:\\Tools\\Code\\Code.exe', 'Executable', '📝'),
        app('app-steam', 'Steam', 'C:\\Program Files\\Steam\\steam.exe', 'Executable', '🎮'),
      ]),
      category('cat-dev', '开发工具', [
        group('grp-frontend', '前端', [
          app('app-vite', 'Vite', 'C:\\Tools\\vite.cmd', 'File', '⚡'),
          app('app-react', 'React', 'https://react.dev', 'Url', '⚛️'),
          app('app-vue', 'Vue', 'https://cn.vuejs.org', 'Url', '🟢'),
          app('app-npm', 'npm', 'C:\\Program Files\\nodejs\\npm.cmd', 'File', '📦'),
          app('app-figma', 'Figma', 'https://www.figma.com', 'Url', '🎨'),
        ]),
        group('grp-backend', '后端', [
          app('app-rust', 'RustRover', 'C:\\Tools\\RustRover\\bin\\rustrover64.exe', 'Executable', '🦀'),
          app('app-cargo', 'Cargo', 'C:\\Users\\Public\\cargo.exe', 'Executable', '📦'),
          app('app-postman', 'Postman', 'C:\\Tools\\Postman\\Postman.exe', 'Executable', '🚀'),
        ], true),
        group('grp-database', '数据库', [
          app('app-dbeaver', 'DBeaver', 'C:\\Tools\\DBeaver\\dbeaver.exe', 'Executable', '🗄️'),
          app('app-redis', 'RedisInsight', 'C:\\Tools\\Redis\\redisinsight.exe', 'Executable', '🔴'),
        ], true),
      ]),
      category('cat-game', '游戏', [
        app('app-genshin', '原神', 'C:\\Games\\Genshin\\GenshinImpact.exe', 'Executable', '🎯'),
        app('app-starrail', '崩铁', 'C:\\Games\\StarRail\\StarRail.exe', 'Executable', '⚔️'),
        app('app-mc', 'Minecraft', 'C:\\Games\\MC\\minecraft.exe', 'Executable', '🏗️'),
      ]),
      category('cat-media', '影音', [], true),
      category('cat-study', '学习', [], true),
    ],
  }
}

/** 用内联 SVG 承载表情符号，既模拟后端返回的资源 URL，又无需外部图片。 */
function emojiIcon(glyph: string): string {
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48">` +
    `<text x="24" y="35" font-size="34" text-anchor="middle">${glyph}</text></svg>`
  return `data:image/svg+xml,${encodeURIComponent(svg)}`
}

function normalizeSettings(settings: Settings): Settings {
  // 旧版预览存档没有 `autoStart`，按关闭迁移；这样新增设置项不会让预览
  // 读到 undefined，也和 Rust 的 `#[serde(default)]` 保持一致。
  return {
    ...defaultSettings(),
    ...settings,
    window: { ...defaultSettings().window, ...settings.window },
    autoStart: settings.autoStart ?? false,
    revision: settings.revision ?? 0,
  }
}

function defaultSettings(): Settings {
  return {
    revision: 0,
    theme: 'default',
    window: { isInitialized: false, position: null, size: null, sizeOverride: null },
    viewMode: 'expanded',
    autoCollapse: false,
    autoStart: false,
  }
}

/**
 * 预览用的窗口尺寸，是 Rust 侧 `resolve_size` 的镜像。
 *
 * 展开态优先采用用户拖出来的尺寸，收起态忽略它；单位同样要对齐 —— 覆盖值
 * 按设计稿 CSS 像素存储，而窗口尺寸是物理像素，所以这里用 `devicePixelRatio`
 * 换算一次。设计稿兜底值没有真实工作区可依据，直接给出一个固定值。
 */
function previewWindowSize(viewMode: ViewMode, override: CssSize | null): WindowSize {
  if (viewMode === 'collapsed') {
    return { width: 19, height: 112 }
  }
  if (!override) {
    return { width: 378, height: 572 }
  }

  const ratio = window.devicePixelRatio || 1
  return {
    width: Math.round(override.width * ratio),
    height: Math.round(override.height * ratio),
  }
}

/**
 * 把旧版预览存档折成当前的根节点形状。
 *
 * 浏览器预览的数据存在 localStorage 里，旧存档是「`apps` + `categories`」两个
 * 数组；后端靠 `AppData::normalize()` 兜同样的兼容性，这里必须照做，否则预览
 * 会直接崩。
 */
function normalizeData(data: AppData): AppData {
  if (Array.isArray(data.items)) {
    return data
  }

  const legacy = data as unknown as { categories?: Category[]; apps?: AppEXE[] }
  const items: RootItem[] = []
  if (legacy.apps && legacy.apps.length > 0) {
    items.push({ Apps: legacy.apps })
  }
  for (const category of legacy.categories ?? []) {
    items.push({ Category: category })
  }

  return { version: 2, items }
}

function readJson<T>(key: string, fallback: T): T {
  try {
    const raw = window.localStorage.getItem(key)
    return raw ? (JSON.parse(raw) as T) : fallback
  } catch {
    return fallback
  }
}

function writeJson(key: string, value: unknown): void {
  try {
    window.localStorage.setItem(key, JSON.stringify(value))
  } catch {
    // 隐私模式下 localStorage 可能不可用，预览场景静默降级即可。
  }
}

function createId(): string {
  return crypto.randomUUID()
}

function normalizeName(raw: string, kind: string): string {
  const name = raw.trim()
  if (name.length === 0) {
    throw { code: `invalid_${kind}_name`, message: `${kind}名称不能为空` }
  }
  return name
}

function delay(): Promise<void> {
  // 保留一点延迟，让加载态和乐观更新在预览时也能被观察到。
  return new Promise((resolve) => window.setTimeout(resolve, 90))
}

class MockBackend implements Backend {
  readonly isTauri = false

  private data: AppData
  private settings: Settings

  constructor() {
    this.data = normalizeData(readJson(STORAGE_KEY, seedData()))
    this.settings = normalizeSettings(readJson(SETTINGS_KEY, defaultSettings()))
  }

  private persistData(): void {
    writeJson(STORAGE_KEY, this.data)
  }

  private persistSettings(): void {
    writeJson(SETTINGS_KEY, this.settings)
  }

  /** 根节点上的所有分类。 */
  private categories(): Category[] {
    return rootCategories(this.data)
  }

  async getWindowInit(): Promise<WindowInitResponse> {
    await delay()
    const position = this.settings.window.position ?? { x: 0, y: 0 }
    const size = this.settings.window.size ?? { width: 378, height: 572 }
    this.settings.window = { ...this.settings.window, isInitialized: true, position, size }
    this.settings.revision += 1
    this.persistSettings()
    return { isInitialized: true, position, size }
  }

  async applyViewMode(viewMode: ViewMode): Promise<WindowInitResponse> {
    await delay()
    this.settings.viewMode = viewMode
    this.settings.revision += 1
    this.persistSettings()
    return {
      isInitialized: true,
      position: { x: 0, y: 100 },
      size: previewWindowSize(viewMode, this.settings.window.sizeOverride),
    }
  }

  async getSettings(): Promise<Settings> {
    await delay()
    return structuredClone(this.settings)
  }

  async updateSettings(settings: Settings): Promise<Settings> {
    await delay()
    if (settings.revision !== this.settings.revision) {
      throw {
        code: 'settings_revision_conflict',
        message: '配置已被更新，请刷新后重试',
      }
    }
    this.settings = { ...structuredClone(settings), revision: settings.revision + 1 }
    this.persistSettings()
    return structuredClone(this.settings)
  }

  async updateAutoCollapse(enabled: boolean): Promise<Settings> {
    await delay()
    this.settings.autoCollapse = enabled
    this.settings.revision += 1
    this.persistSettings()
    return structuredClone(this.settings)
  }

  async updateAutoStart(enabled: boolean): Promise<Settings> {
    await delay()
    this.settings.autoStart = enabled
    this.settings.revision += 1
    this.persistSettings()
    return structuredClone(this.settings)
  }

  async updateTheme(theme: Settings['theme']): Promise<Settings> {
    await delay()
    this.settings.theme = theme
    this.settings.revision += 1
    this.persistSettings()
    return structuredClone(this.settings)
  }

  async clearWindowSizeOverride(): Promise<Settings> {
    await delay()
    this.settings.window = { ...this.settings.window, sizeOverride: null }
    this.settings.revision += 1
    this.persistSettings()
    return structuredClone(this.settings)
  }

  async getAllData(): Promise<AppData> {
    await delay()
    return structuredClone(this.data)
  }

  async addCategory(name: string): Promise<Category> {
    await delay()
    const category: Category = {
      id: createId(),
      name: normalizeName(name, '分类'),
      isCollapsed: false,
      groups: [],
      createdTime: new Date().toISOString(),
    }
    this.data = appendCategoryData(this.data, category)
    this.persistData()
    return structuredClone(category)
  }

  async renameCategory(id: string, name: string): Promise<Category> {
    await delay()
    const category = this.categories().find((item) => item.id === id)
    if (!category) {
      throw { code: 'category_not_found', message: `找不到分类: ${id}` }
    }
    const renamed = normalizeName(name, '分类')
    this.data = renameCategoryData(this.data, id, renamed)
    const result = this.categories().find((item) => item.id === id)!
    this.persistData()
    return structuredClone(result)
  }

  async deleteCategory(id: string): Promise<void> {
    await delay()
    if (!this.categories().some((item) => item.id === id)) {
      throw { code: 'category_not_found', message: `找不到分类: ${id}` }
    }
    this.data = removeCategoryData(this.data, id)
    this.persistData()
  }

  async addGroup(parentId: string, name: string): Promise<Group> {
    await delay()
    const group: Group = {
      id: createId(),
      name: normalizeName(name, '分组'),
      isCollapsed: false,
      children: [],
      createdTime: new Date().toISOString(),
    }

    const next = appendChildData(this.data, parentId, { Group: group })
    if (next === this.data) {
      throw { code: 'parent_not_found', message: `找不到分组的父节点: ${parentId}` }
    }

    this.data = next
    this.persistData()
    return structuredClone(group)
  }

  async renameGroup(id: string, name: string): Promise<Group> {
    await delay()
    const renamed = normalizeName(name, '分组')
    const group = findGroup(this.data, id)
    if (!group) {
      throw { code: 'group_not_found', message: `找不到分组: ${id}` }
    }

    this.data = renameGroupData(this.data, id, renamed)
    this.persistData()
    return structuredClone({ ...group, name: renamed })
  }

  async deleteGroup(id: string): Promise<void> {
    await delay()
    if (!findGroup(this.data, id)) {
      throw { code: 'group_not_found', message: `找不到分组: ${id}` }
    }

    this.data = removeGroupData(this.data, id)
    this.persistData()
  }

  async setNodeCollapsed(id: string, isCollapsed: boolean): Promise<void> {
    await delay()
    const category = this.categories().find((item) => item.id === id)
    if (category) {
      this.data = toggleCollapseData(this.data, id, isCollapsed)
      this.persistData()
      return
    }

    if (!findGroup(this.data, id)) {
      throw { code: 'collapsible_node_not_found', message: `找不到可折叠节点: ${id}` }
    }

    this.data = toggleCollapseData(this.data, id, isCollapsed)
    this.persistData()
  }

  async addApp(
    parentId: string | null,
    sourcePath: string,
    name?: string,
    rootIndex?: number,
  ): Promise<AppEXE> {
    await delay()
    const path = sourcePath.trim()
    if (path.length === 0) {
      throw { code: 'invalid_source_path', message: '来源路径不能为空' }
    }

    const app: AppEXE = {
      id: createId(),
      name: name?.trim() || deriveName(path),
      sourcePath: path,
      iconUrl: emojiIcon(guessGlyph(path)),
      iconType: guessIconType(path),
      createdTime: new Date().toISOString(),
    }

    const next = insertAppData(this.data, app, parentId, rootIndex)
    if (next === this.data) {
      throw { code: 'parent_not_found', message: `找不到父节点: ${parentId}` }
    }

    this.data = next
    this.persistData()
    return structuredClone(app)
  }

  async addApps(
    parentId: string | null,
    sourcePaths: string[],
    rootIndex?: number,
  ): Promise<AddAppsResponse> {
    await delay()
    const added: AppEXE[] = []
    const failed: AddAppsResponse['failed'] = []

    for (const sourcePath of sourcePaths) {
      try {
        const app = await this.addApp(parentId, sourcePath, undefined, rootIndex)
        added.push(app)
      } catch (error) {
        const value = error as { code?: string; message?: string }
        failed.push({
          sourcePath,
          code: value.code ?? 'mock_import_failed',
          message: value.message ?? '批量导入失败',
        })
      }
    }

    return { added: structuredClone(added), failed }
  }

  async moveApp(id: string, parentId: string | null, beforeId: string | null): Promise<void> {
    await delay()
    const result = moveAppData(this.data, id, parentId, beforeId)
    if (!result.ok) {
      const messages = {
        app_not_found: `找不到应用: ${id}`,
        parent_not_found: `找不到目标容器: ${parentId}`,
        reference_not_found: `找不到参照节点: ${beforeId}`,
      }
      throw { code: result.code, message: messages[result.code] }
    }
    this.data = result.data
    this.persistData()
  }

  async moveGroup(id: string, parentId: string, beforeId: string | null): Promise<void> {
    await delay()
    const result = moveGroupData(this.data, id, parentId, beforeId)
    if (!result.ok) {
      const messages = {
        group_not_found: `找不到分组: ${id}`,
        parent_not_found: `找不到分组父容器: ${parentId}`,
        reference_not_found: `找不到参照节点: ${beforeId}`,
        invalid_group_parent: '分组只能在当前父容器内重排',
        invalid_group_position: '分组只能在同级中移动，且不能放入自身或后代',
      }
      throw { code: result.code, message: messages[result.code] }
    }
    this.data = result.data
    this.persistData()
  }

  // 预览模式没有后台抓图标这回事：mock 的网址图标本来就是现成的表情。
  async onIconReady(): Promise<() => void> {
    return () => {}
  }

  async renameApp(id: string, name: string): Promise<AppEXE> {
    await delay()
    const renamed = normalizeName(name, '应用')
    const existing = findApp(this.data, id)
    if (!existing) {
      throw { code: 'app_not_found', message: `找不到应用: ${id}` }
    }

    this.data = renameAppData(this.data, id, renamed)
    this.persistData()
    return structuredClone({ ...existing, name: renamed })
  }

  async deleteApp(id: string): Promise<void> {
    await delay()
    if (!findApp(this.data, id)) {
      throw { code: 'app_not_found', message: `找不到应用: ${id}` }
    }

    this.data = removeAppData(this.data, id)
    this.persistData()
  }

  async launchIcon(id: string): Promise<void> {
    await delay()
    if (!findApp(this.data, id)) {
      throw { code: 'app_not_found', message: `找不到应用: ${id}` }
    }
    // 浏览器里无法真的启动程序，留一条日志方便确认调用链通了。
    console.info('[mock] launch_icon', id)
  }
}

function deriveName(path: string): string {
  if (/^https?:\/\//i.test(path)) {
    try {
      return new URL(path).hostname.replace(/^www\./i, '')
    } catch {
      return path
    }
  }

  const segment = path.split(/[\\/]/).pop() ?? path
  return segment.replace(/\.[^.]+$/, '') || segment
}

function guessIconType(path: string): IconType {
  if (/^https?:\/\//i.test(path)) {
    return 'Url'
  }
  if (/\.lnk$/i.test(path)) {
    return 'Shortcut'
  }
  if (/\.exe$/i.test(path)) {
    return 'Executable'
  }
  if (!/\.[^.\\/]+$/.test(path)) {
    return 'Folder'
  }
  return 'File'
}

function guessGlyph(path: string): string {
  switch (guessIconType(path)) {
    case 'Url':
      return '🌐'
    case 'Folder':
      return '📁'
    case 'Shortcut':
      return '🔗'
    case 'Executable':
      return '⚙️'
    default:
      return '📄'
  }
}

/** 预览模式的单例。 */
export const mockBackend: Backend = new MockBackend()
