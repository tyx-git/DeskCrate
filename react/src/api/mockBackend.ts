import type {
  AppData,
  AppEXE,
  Category,
  Group,
  IconType,
  Item,
} from '../types/catalog'
import type { CssSize, Settings, ViewMode, WindowInitResponse, WindowSize } from '../types/settings'
import type { Backend } from './backend'

/**
 * 浏览器预览用的内存后端。
 *
 * 存在的意义是让 UI 可以在没有 Rust 工具链、没有 Tauri 运行时的机器上
 * 直接 `npm run dev` 验收。它刻意复刻了后端的关键语义：UUID 由「后端」
 * 生成、名称去空格后不能为空、分组嵌套不超过 4 层、删除返回 void。
 * 数据写入 localStorage，刷新页面不会丢。
 */

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
  ): Category => ({ id, name, isCollapsed, groups, createdTime: now })

  return {
    version: 1,
    // 顶层应用：最常用的那几个，不必为了放它们先建一个分类。
    apps: [
      appEXE('app-root-explorer', '此电脑', 'C:\\Windows\\explorer.exe', 'Folder', '🖥️'),
      appEXE('app-root-github', 'GitHub', 'https://github.com', 'Url', '🐙'),
      appEXE('app-root-terminal', '终端', 'C:\\Windows\\System32\\cmd.exe', 'Executable', '⌨️'),
    ],
    categories: [
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

function defaultSettings(): Settings {
  return {
    theme: 'default',
    window: { isInitialized: false, position: null, size: null, sizeOverride: null },
    viewMode: 'expanded',
    autoCollapse: false,
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
 * 补上顶层应用数组。
 *
 * 预览数据存在 localStorage 里，旧版本的存档没有 `apps` 字段；后端靠
 * `serde(default)` 兜住了同样的问题，这里必须照做，否则预览会直接崩。
 */
function withTopLevelApps(data: AppData): AppData {
  return Array.isArray(data.apps) ? data : { ...data, apps: [] }
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
    this.data = withTopLevelApps(readJson(STORAGE_KEY, seedData()))
    this.settings = readJson(SETTINGS_KEY, defaultSettings())
  }

  private persistData(): void {
    writeJson(STORAGE_KEY, this.data)
  }

  private persistSettings(): void {
    writeJson(SETTINGS_KEY, this.settings)
  }

  /** 在整棵树里就地替换某个节点，返回是否命中。 */
  private replaceItem(
    items: Item[],
    id: string,
    produce: (app: AppEXE) => AppEXE | null,
  ): Item[] {
    return items.flatMap((item): Item[] => {
      if ('App' in item) {
        if (item.App.id !== id) {
          return [item]
        }
        const next = produce(item.App)
        return next ? [{ App: next }] : []
      }

      return [
        {
          Group: {
            ...item.Group,
            children: this.replaceItem(item.Group.children, id, produce),
          },
        },
      ]
    })
  }

  private mapGroups(
    items: Item[],
    id: string,
    produce: (group: Group) => Group | null,
  ): Item[] {
    return items.flatMap((item): Item[] => {
      if ('App' in item) {
        return [item]
      }

      if (item.Group.id === id) {
        const next = produce(item.Group)
        return next ? [{ Group: next }] : []
      }

      return [
        {
          Group: {
            ...item.Group,
            children: this.mapGroups(item.Group.children, id, produce),
          },
        },
      ]
    })
  }

  async getWindowInit(): Promise<WindowInitResponse> {
    await delay()
    const position = this.settings.window.position ?? { x: 0, y: 0 }
    const size = this.settings.window.size ?? { width: 378, height: 572 }
    this.settings.window = { ...this.settings.window, isInitialized: true, position, size }
    this.persistSettings()
    return { isInitialized: true, position, size }
  }

  async applyViewMode(viewMode: ViewMode): Promise<WindowInitResponse> {
    await delay()
    this.settings.viewMode = viewMode
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
    this.settings = structuredClone(settings)
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
    this.data.categories.push(category)
    this.persistData()
    return structuredClone(category)
  }

  async renameCategory(id: string, name: string): Promise<Category> {
    await delay()
    const category = this.data.categories.find((item) => item.id === id)
    if (!category) {
      throw { code: 'category_not_found', message: `找不到分类: ${id}` }
    }
    category.name = normalizeName(name, '分类')
    this.persistData()
    return structuredClone(category)
  }

  async deleteCategory(id: string): Promise<void> {
    await delay()
    const index = this.data.categories.findIndex((item) => item.id === id)
    if (index < 0) {
      throw { code: 'category_not_found', message: `找不到分类: ${id}` }
    }
    this.data.categories.splice(index, 1)
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

    const category = this.data.categories.find((item) => item.id === parentId)
    if (category) {
      category.groups.push({ Group: group })
      this.persistData()
      return structuredClone(group)
    }

    let attached = false
    for (const item of this.data.categories) {
      const next = this.mapGroups(item.groups, parentId, (parent) => {
        attached = true
        return { ...parent, children: [...parent.children, { Group: group }] }
      })
      item.groups = next
      if (attached) {
        break
      }
    }

    if (!attached) {
      throw { code: 'parent_not_found', message: `找不到分组的父节点: ${parentId}` }
    }

    this.persistData()
    return structuredClone(group)
  }

  async renameGroup(id: string, name: string): Promise<Group> {
    await delay()
    const renamed = normalizeName(name, '分组')
    let result: Group | null = null

    for (const category of this.data.categories) {
      category.groups = this.mapGroups(category.groups, id, (group) => {
        result = { ...group, name: renamed }
        return result
      })
    }

    if (!result) {
      throw { code: 'group_not_found', message: `找不到分组: ${id}` }
    }

    this.persistData()
    return structuredClone(result)
  }

  async deleteGroup(id: string): Promise<void> {
    await delay()
    let removed = false

    for (const category of this.data.categories) {
      category.groups = this.mapGroups(category.groups, id, () => {
        removed = true
        return null
      })
    }

    if (!removed) {
      throw { code: 'group_not_found', message: `找不到分组: ${id}` }
    }

    this.persistData()
  }

  async addApp(
    parentId: string | null,
    sourcePath: string,
    name?: string,
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

    // 与后端一致：`null` 表示放到顶层，不进任何分类。
    if (parentId === null) {
      this.data.apps.push(app)
      this.persistData()
      return structuredClone(app)
    }

    const category = this.data.categories.find((item) => item.id === parentId)
    if (category) {
      category.groups.push({ App: app })
      this.persistData()
      return structuredClone(app)
    }

    let attached = false
    for (const item of this.data.categories) {
      item.groups = this.mapGroups(item.groups, parentId, (group) => {
        attached = true
        return { ...group, children: [...group.children, { App: app }] }
      })
      if (attached) {
        break
      }
    }

    if (!attached) {
      throw { code: 'parent_not_found', message: `找不到父节点: ${parentId}` }
    }

    this.persistData()
    return structuredClone(app)
  }

  async renameApp(id: string, name: string): Promise<AppEXE> {
    await delay()
    const renamed = normalizeName(name, '应用')
    let result: AppEXE | null = null

    const root = this.data.apps.find((app) => app.id === id)
    if (root) {
      root.name = renamed
      result = root
    }

    for (const category of this.data.categories) {
      category.groups = this.replaceItem(category.groups, id, (app) => {
        result = { ...app, name: renamed }
        return result
      })
    }

    if (!result) {
      throw { code: 'app_not_found', message: `找不到应用: ${id}` }
    }

    this.persistData()
    return structuredClone(result)
  }

  async deleteApp(id: string): Promise<void> {
    await delay()
    let removed = false

    const index = this.data.apps.findIndex((app) => app.id === id)
    if (index >= 0) {
      this.data.apps.splice(index, 1)
      removed = true
    }

    for (const category of this.data.categories) {
      category.groups = this.replaceItem(category.groups, id, () => {
        removed = true
        return null
      })
    }

    if (!removed) {
      throw { code: 'app_not_found', message: `找不到应用: ${id}` }
    }

    this.persistData()
  }

  async launchIcon(id: string): Promise<void> {
    await delay()
    if (!findAppInTree(this.data, id)) {
      throw { code: 'app_not_found', message: `找不到应用: ${id}` }
    }
    // 浏览器里无法真的启动程序，留一条日志方便确认调用链通了。
    console.info('[mock] launch_icon', id)
  }
}

function findAppInTree(data: AppData, id: string): AppEXE | null {
  const root = data.apps.find((app) => app.id === id)
  if (root) {
    return root
  }

  const walk = (items: readonly Item[]): AppEXE | null => {
    for (const item of items) {
      if ('App' in item) {
        if (item.App.id === id) {
          return item.App
        }
        continue
      }
      const found = walk(item.Group.children)
      if (found) {
        return found
      }
    }
    return null
  }

  for (const category of data.categories) {
    const found = walk(category.groups)
    if (found) {
      return found
    }
  }

  return null
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
