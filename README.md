<div align="center">
  <h1>DeskCrate</h1>
  <p><strong>把散落的应用、快捷方式、文件夹和网址，收进一个轻量、便携的 Windows 桌面工具箱。</strong></p>
  <p>面向高频桌面工作流的分类启动器，由 Rust、Tauri 与 React 驱动。</p>
  <p>
    <a href="https://github.com/tyx-git/DeskCrate"><img alt="Version" src="https://img.shields.io/badge/version-v0.1.0-2563eb?style=flat-square"></a>
    <img alt="Platform" src="https://img.shields.io/badge/platform-Windows-0078d4?style=flat-square">
    <img alt="Rust" src="https://img.shields.io/badge/backend-Rust-ce422b?style=flat-square">
    <img alt="Tauri" src="https://img.shields.io/badge/framework-Tauri%202-24c8db?style=flat-square">
    <a href="https://github.com/tyx-git/DeskCrate/stargazers"><img alt="GitHub stars" src="https://img.shields.io/github/stars/tyx-git/DeskCrate?style=flat-square"></a>
  </p>
  <p>
    <a href="https://github.com/tyx-git/DeskCrate">项目主页</a> ·
    <a href="https://github.com/tyx-git/DeskCrate/issues">问题反馈</a> ·
    <a href="https://github.com/tyx-git/DeskCrate/releases">版本发布</a>
  </p>
</div>

> [!IMPORTANT]
> DeskCrate 当前版本为 `v0.1.0`。后端分层、Command API 与测试层已经完成，React 侧栏界面也已接入并联调通过（展开态 / 收起态、右键管理、快捷键、文件拖放）。当前仓库适合开发、审阅和真机验收，尚未在多种分辨率和 DPI 环境下做完整测试，还不是面向普通用户的正式发行版。

## 为什么是 DeskCrate

常用工具通常散落在桌面、开始菜单、深层目录和浏览器收藏夹中。DeskCrate 希望用一条安静、可折叠的桌面侧栏统一整理这些入口，同时保持本地优先、启动迅速和数据可控。

- **便携运行**：不依赖安装到 `Program Files`，配置、数据和图标缓存随程序目录保存。
- **统一入口**：支持 Windows 快捷方式、可执行程序、文件夹、普通文件和网址。
- **层级整理**：使用 UUID 管理分类、分组和应用，支持最多四层分组嵌套。
- **原生图标**：解析 `.lnk`，从程序或 Shell 关联目标提取图标并缓存为 PNG。
- **可靠持久化**：通过内存 mutation 和同目录原子替换，避免留下半写入 JSON。
- **悬浮侧栏**：透明、无边框、置顶、不进任务栏，窗口尺寸贴合侧栏包围盒；收起态真的缩成 3×80 指示条，而不是仅隐藏 DOM。
- **窗口记忆**：按设计稿比例推导首次几何，退出时保存位置与尺寸；恢复位置落在所有显示器之外时自动重算。
- **安全边界**：前端不接触 `HICON`、`Child` 或 `MutexGuard`，脚本默认禁止直接执行。

## 当前能力

| 模块 | 已实现能力 |
|---|---|
| 目录管理 | 获取完整数据树，新增、重命名和删除分类、分组及应用 |
| 来源识别 | 识别 `Shortcut`、`Executable`、`Folder`、`File`、`Url` |
| 快捷方式 | 通过 Windows COM 解析目标、参数、工作目录和图标位置 |
| 图标服务 | 提取 Windows 图标、转换 RGBA、保存 PNG、生成 Tauri asset URL |
| 应用启动 | 非阻塞启动程序，通过系统关联打开文件、目录和网址 |
| 配置管理 | 持久化主题、展开状态、自动收起和窗口状态 |
| 数据安全 | 分离 `data.json` 与 `config.json`，校验失败不落盘，写入采用原子替换 |
| 损坏恢复 | `config.json` 内容损坏时备份为 `config.json.corrupt-<时间戳>` 并用默认值重建（几何可重新推导，无损）；`data.json` 损坏时不重建，只留一份 `data.json.corrupt` 副本并把原文件路径写进错误信息 |
| API 边界 | 使用 Tauri Command 返回可序列化模型和统一 `AppError` |
| 侧栏界面 | 展开态分类/分组/应用树与收起态指示条，按设计稿还原视觉细节 |
| 窗口形态 | 透明无边框悬浮层，几何由后端按工作区推导，收起态真缩窗 |
| 顶层应用 | 应用可以直接放在顶层、渲染在所有分类之前，不必先建一个分类（拖到顶层提示区，或右键「添加应用到顶层…」） |
| 网址图标 | 添加网址时抓取站点 favicon（`<link rel=icon>` 候选 → 回退 `/favicon.ico`）并缓存到 `icons/`，失败则退回内置地球图标、不影响添加 |
| 窗口尺寸 | 展开态可拖拽**右侧宽度与上下高度**（左边缘不可拖）并跨重启记住（按设计稿 CSS 像素存储，换屏不失真），右键「重置窗口尺寸」回到设计稿尺寸 |
| 收起过渡 | 收起时面板的宽高**同时**收束成指示条同大的细丝再缩窗，展开时反向长回来；200ms / 240ms，尊重 `prefers-reduced-motion` |
| 收起态启动 | 以收起态启动时自动把被 Windows 顶宽的指示条收回到设计宽度（顶宽是异步的，需回读实际尺寸重试，见 9.4.1） |
| 交互能力 | 右键菜单、新建/重命名/删除对话框、搜索过滤、快捷键、自动收起、文件拖放 |

## 技术栈

| 领域 | 技术 |
|---|---|
| 桌面框架 | Tauri 2.11 |
| 后端 | Rust 2021、Serde、Chrono、UUID |
| Windows 集成 | `windows` 0.58、COM、Shell、GDI |
| 前端框架 | React 19、TypeScript 6、Vite 8 |
| 前端样式 | Tailwind CSS v4（`@theme` 设计令牌 + 容器查询单位 `cqw`） |
| 前端状态 | Zustand v5 |
| 图像处理 | `image` 0.25 |
| 持久化 | Pretty JSON、同目录临时文件、原子替换 |
| 测试与校验 | Rust 集成测试、Tauri MockRuntime、`tsc`、ESLint |

## 快速开始

### 环境要求

- Windows 10 或 Windows 11 x64
- Rust stable，目标工具链为 `stable-x86_64-pc-windows-msvc`
- Visual Studio Build Tools，包含 MSVC 与 Windows SDK
- Node.js `20.19+`、`22.12+` 或 `24+`，以及 npm
- Microsoft Edge WebView2 Runtime

### 获取源码

```powershell
git clone https://github.com/tyx-git/DeskCrate.git
Set-Location DeskCrate
```

### 安装依赖

```powershell
rustup toolchain install stable-x86_64-pc-windows-msvc
rustup override set stable-x86_64-pc-windows-msvc
npm install --prefix react
cargo install tauri-cli --version "^2.0" --locked
```

已经安装 Tauri CLI 时，无需重复执行最后一条命令。

### 启动开发环境

完整桌面应用（Rust 后端 + 真实悬浮窗）：

```powershell
cargo tauri dev
```

Tauri 会根据 `src-tauri/tauri.conf.json` 自动启动 Vite 开发服务器，并在前端初始化完成后
显示主窗口。窗口以 `visible: false` 启动，由前端在数据加载完成后显式 `show()`，避免
启动瞬间窗口从默认尺寸跳到目标位置的闪烁。

只调前端（不需要 Rust 工具链）：

```powershell
npm --prefix react run dev
```

在浏览器中打开 `http://localhost:5173`。此时前端会自动切换到内存 mock 后端，预置一份
与设计稿同构的演示数据（5 分类 / 3 分组 / 14 应用），增删改会写入 `localStorage`，
刷新后保留。`DevScene` 会铺一层模拟桌面背景，并按 1920×1080 场景摆放侧栏，方便比对
设计稿比例。

### 构建应用

> [!IMPORTANT]
> 生产构建**必须**启用 `custom-protocol` feature。缺少它时 `tauri::generate_context!()`
> 不会把 `react/dist` 嵌进二进制，窗口会转而从 `devUrl`（`http://localhost:5173/`）
> 加载前端 —— 在没有开发服务器的机器上打开就是一片空白。

```powershell
npm --prefix react run build                       # 先产出 react/dist
Set-Location src-tauri
cargo build --release --features custom-protocol   # 生产等价产物
```

产物是 `src-tauri/target/release/app.exe`，前端资源已嵌入，可脱离开发服务器运行。

仓库当前**没有安装 Tauri CLI**：`react/package.json` 只依赖 `@tauri-apps/api`，
`.cargo/bin` 和全局 npm 里都没有 `cargo-tauri`，所以 `cargo tauri dev` /
`cargo tauri build` 暂时不可用。需要打包安装器时先安装：

```powershell
cargo install tauri-cli --version "^2.0" --locked
```

CLI 的 `tauri build` 会自行注入 `tauri/custom-protocol`，装了 CLI 之后
`cargo tauri build` 无需再手写 `--features`。

## 质量检查

后端：

```powershell
Set-Location src-tauri
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test -- --test-threads=1
cargo build          # 快速链接检查（debug；此配置不嵌入前端资源）
cargo build --release --features custom-protocol   # 生产等价构建，发布前跑
```

前端：

```powershell
npm --prefix react run build   # tsc -b && vite build
npm --prefix react run lint    # eslint .
```

最近一次验证结果：后端 72 项测试全部通过，覆盖模型契约、JSON 仓储、业务服务、
Tauri Command、窗口几何、顶层应用、网址图标的纯函数、损坏文件恢复与 Windows 系统
边界；`cargo fmt`、`clippy -D warnings`、`cargo build` 均无告警。前端 `tsc -b`、
`eslint .`、`vite build` 均通过。

真机断言另有：窗口尺寸 12 项（改尺寸 → 按 CSS 像素落盘 → 收起/展开 → 重置 →
夹取上限 → 重启恢复）、退出归一化 7 项（拖到上限之外后优雅关闭，落盘的
`window.size` 不是窗口实际尺寸）、损坏恢复 12 项（把两个文件分别写坏再启动）、
边缘缩放 8 项（只有右/上/下能拖，只改对应维度）、收起过渡 11 项（收束先于缩窗、
展开先变大、鼠标移出面板即收起）、顶层应用 5 项（渲染顺序、顶层落点、分类无图标）。
旧的 3 项窗口样式断言（展开态有 `WS_THICKFRAME`）已随边缘缩放改造失效——窗口现在
恒为 `resizable: false`。以上均在真实 `app.exe` 上通过，做法见
`dosc/plan/plan.md` 第 7.3、9.5、9.6 节。

注意：`cargo build`（debug）走的是 `devUrl` 分支，**不会**读取 `frontendDist`，
所以它只验证链接，不验证前端资源接线。要确认资源真的被嵌入，用
`cargo build --release --features custom-protocol` 构建后检查二进制里是否出现
`assets/` 字符串，或干脆**关掉 Vite 开发服务器**再运行产物。

## 数据目录

DeskCrate 将用户数据放在可执行文件同级目录，便于整体移动和备份：

```text
DeskCrate.exe
config/
  data.json       分类、分组、应用和快捷方式
  config.json     主题、视图模式和窗口状态
icons/
  <app-id>.png    后端提取的图标缓存
```

`data.json` 和 `config.json` 分开持久化。对 JSON 的“局部修改”发生在内存模型中，提交时仍会序列化完整文档并执行原子替换。

`icons/` 里的文件名固定为 `{app_id}.png`。删除应用、分组或分类都会同步清掉对应图标；
启动时还会做一次对账，回收异常路径留下的孤儿文件 —— 只删严格符合 `{UUID}.png` 命名的
文件，你自己放进去的其他文件不会被碰。详见 `dosc/plan/plan.md` 第 8.3 节。

> [!NOTE]
> `v0.1.0` 的 `iconUrl` 仍包含程序目录的绝对路径。整体移动程序目录后可能需要重新生成图标资源 URL；后续版本会改为按当前 `icons/` 目录动态构造。

> [!IMPORTANT]
> `icons/` 目录是**运行时**注册进 Tauri asset protocol 作用域的（`lib.rs` 的 `setup`
> 里调用 `asset_protocol_scope().allow_directory(...)`）。**不要把它写回
> `tauri.conf.json` 的 `assetProtocol.scope`**：配置里唯一能表达「程序目录」的变量是
> `$EXE`，而它解析时走 `dirs::executable_dir()`，该函数在 Windows 上恒返回 `None`，
> 于是这条配置会被**静默丢弃**、作用域变成空集，所有图标请求被 403 拒绝，日志里
> 只留下一句 `asset protocol not configured to allow the path`。详见
> `dosc/plan/plan.md` 第 8.2 节。

## 后端接口

共 15 个 Command：

| 接口组 | Command |
|---|---|
| 数据读取 | `get_all_data` |
| 分类 | `add_category`、`rename_category`、`delete_category` |
| 分组 | `add_group`、`rename_group`、`delete_group` |
| 应用 | `add_app`、`rename_app`、`delete_app`、`launch_icon` |
| 配置 | `get_settings`、`update_settings` |
| 窗口 | `is_init`、`apply_view_mode` |

新增节点的 UUID 由后端生成。前端应先读取接口返回的 `id`，后续重命名、删除和启动均按 ID 定位，不能使用名称作为主键。

窗口几何由后端独占：前端只提交 `apply_view_mode("expanded" | "collapsed")`，尺寸和位置由
`services::window::overlay_geometry` 按当前工作区推导，前端不调用 `setSize` / `setPosition`。
这样设计稿的百分比换算在 Rust 侧只存在一份，也保证收起态是真的缩窗而非隐藏 DOM。

> [!IMPORTANT]
> 启动流程里 `show()` 之后还会再套一次几何（`settingsStore.syncWindowGeometry`），
> 这不是冗余：窗口隐藏期间改的尺寸会被 Windows 在显示时顶到系统最小窗口宽度，
> 收起态那 29 物理像素宽的窗口会被撑成一大块白条（实测 30 → 202.5 物理像素）。
> 详细对照实验见 `dosc/plan/plan.md` 9.4。

前端不直接调用 `invoke`，而是通过 `react/src/api/backend.ts` 定义的 `Backend` 接口访问。
在 Tauri 环境下走 `tauriBackend`，在浏览器中自动降级为 `mockBackend`。

## 项目结构

```text
DeskCrate/
  react/                        React + TypeScript 前端
    src/
      api/                      Backend 抽象与 Tauri/mock 双实现
      types/                    Rust 模型的 TS 镜像
      lib/                      纯函数工具（树操作、几何、存储）
      state/                    Zustand store（catalog / settings / ui）
      hooks/                    自动收起、快捷键、文件拖放
      components/
        sidebar/                侧栏主体与收起指示条
        overlays/               右键菜单、对话框、提示条
        common/                 通用小控件
      index.css                 @theme 设计令牌 + 容器查询单位
      main.tsx                  启动编排
  src-tauri/
    src/
      commands/                 Tauri API 边界
      services/                 业务规则与流程编排
      storage/                  JSON 仓储与原子写入
      system/                   Windows 底层实现
      models/                   持久化模型与请求模型
      state.rs                  共享应用状态
      lib.rs                    Tauri 应用组装入口
      main.rs                   最小二进制入口
    tests/                      集成测试
```

## 设计还原说明

设计稿（`dosc/plan/展示页.html`、`收起态.html`）以 1920×1080 场景为画布，用 `vw` 书写
字号与间距。真实窗口宽度只有 220–480px，直接沿用 `vw` 会让侧栏内文字被放大到不可用。
因此前端改用**容器查询单位**：

```text
1 场景 vw = 1 / 18.75% = 5.3333 cqw
```

`.dc-container` 声明 `container-type: inline-size`，所有尺寸令牌用 `clamp()` 给出上下限，
例如 `--text-cat: clamp(10px, 5.867cqw, 16px)`（对应设计稿 1.1vw）。

窗口在侧栏包围盒外额外预留 16px 透明留白（`OVERLAY_SHADOW_GUTTER_PX`），否则 CSS 的
`box-shadow` 会被窗口边界裁掉。面板自身的 2px 左边距由 CSS padding 提供，不写进窗口
x 坐标，避免偏移被计算两次。

> [!IMPORTANT]
> 留白、指示条这些常量是**设计稿的 CSS 像素**，而 `set_size` / `set_position` 与显示器
> 工作区都是**物理像素**。`overlay_geometry()` 因此接收 `scale_factor`，把所有 `css`
> 常量经 `to_physical()` 换算后再相加（百分比不用换算）。漏掉换算不是「差几个像素」：
> 150% 缩放的屏幕上 16px 留白占 24 个物理像素，而收起态窗口总共只有 19 个 CSS 像素宽，
> 内容盒会被压成 0，指示条宽 0 —— 看不见也点不到，用户只能改 `config.json` 才能回到
> 展开态。`react/src/lib/previewGeometry.ts` 是浏览器预览的镜像，那边 CSS 像素就是最终
> 单位，**不要**加换算。

## 已知限制

| 项 | 现状 | 原因 |
|---|---|---|
| 收起态持久化 | 存放在前端 `localStorage` | 后端 `Settings` 没有对应的 `isCollapsed` Command |
| 分类 / 分组图标 | 没有图标 | `Category`、`Group` 模型本就没有图标字段；此前按名称推断 emoji 的做法已移除 |
| 网址图标格式 | 不支持 SVG | `image` 无 SVG 解码器，为此引 resvg 不划算；这类站点退回内置地球图标 |
| 添加应用 | 手工输入路径 | 未引入 `tauri-plugin-dialog` 文件选择器 |
| 排序 / 拖拽重排 | 未实现 | 后端没有排序 API |
| 窗口位置 | 无法拖动 | 没有标题栏就没有 `HTCAPTION`；窗口也不再带 `WS_THICKFRAME` |
| 收束过渡观感 | 只用断言验了时序 | 细丝与指示条的衔接、光带观感需要肉眼核对 |
| 显示器变化 | 不整体重算几何 | 仅兜底「窗口完全落在所有显示器之外」与「窗口小到装不下当前视图模式」两种不可恢复的情况 |
| 应用标识符 | 仍是模板默认值 `com.tauri.dev` | 发布前需替换为自己的反向域名 |
| 应用图标 | 仍是 Tauri 官方默认图标 | 发布前需替换品牌图标并重新生成整套尺寸 |
| 真机验收 | 未完成 | 尚未用肉眼核对悬浮层观感与高 DPI 表现 |

> [!WARNING]
> **不要把 DeskCrate 放到 `Program Files` 等受保护目录下运行。** 它需要在程序所在目录
> 创建 `config/` 与 `icons/`；目录不可写时启动会失败。虽然已经会用原生对话框提示原因，
> 但根因（`bundle.targets: "all"` 产出的 MSI 默认装到 `Program Files`）尚未处理，详见
> `dosc/plan/plan.md` 第 15.3 节。

## 开发路线

- [x] 完成后端分层结构与稳定 Command API
- [x] 完成目录树 CRUD、配置持久化和窗口状态机
- [x] 完成快捷方式解析、图标提取和基础启动能力
- [x] 完成模型、仓储、服务、API 与 Windows 边界测试
- [x] 改造主窗口为透明无边框悬浮层，实现展开态 / 收起态几何推导
- [x] 接入 React 侧栏界面并完成前后端联调
- [x] 在真实二进制上验证窗口状态机三条分支、生产 CSP 与 capabilities 权限
- [x] 实现展开态拖拽改尺寸并跨重启记住，含收起态禁用缩放与「重置窗口尺寸」
- [ ] 在真实桌面环境验收透明悬浮层观感与多分辨率 / DPI 表现
- [ ] 替换应用标识符与品牌图标，并处理 MSI 默认装到 `Program Files` 带来的写入权限问题
- [x] 增加损坏配置恢复策略（顶层应用的 `apps` 字段带 `serde(default)`，不需要迁移）
- [x] 顶层可直接放应用、网址自动抓站点图标、只留右/上/下三个缩放边、收起收束过渡
- [ ] 为收起状态增加持久化 Command，替换前端 `localStorage`
- [ ] 让窗口位置可拖动，并把新位置写回 `window.position`
- [ ] 增加节点排序、移动、导入和导出
- [ ] 设计脚本执行白名单
- [ ] 扩展系统工具、全局快捷键与桌面边缘交互

## 参与开发

提交 Issue 前请提供复现步骤、预期行为、实际行为、Windows 版本以及相关日志。Pull Request 应保持单一职责，并在涉及模型或 API 变化时同步补充测试。

- 问题反馈：[GitHub Issues](https://github.com/tyx-git/DeskCrate/issues)
- 源码仓库：[tyx-git/DeskCrate](https://github.com/tyx-git/DeskCrate)

## 作者与版本

- 作者：TTxyz
- 当前版本：v0.1.0
- 仓库：<https://github.com/tyx-git/DeskCrate.git>

## 许可证

当前仓库尚未声明开源许可证。在许可证文件正式加入前，默认保留所有权利。
