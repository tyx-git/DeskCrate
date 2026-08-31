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
> DeskCrate 当前版本为 `v0.1.0`。后端基础能力和测试层已经完成，React 前端仍是工程脚手架，尚未接入最终侧栏界面；当前仓库适合开发、审阅和联调，还不是面向普通用户的正式发行版。

## 为什么是 DeskCrate

常用工具通常散落在桌面、开始菜单、深层目录和浏览器收藏夹中。DeskCrate 希望用一条安静、可折叠的桌面侧栏统一整理这些入口，同时保持本地优先、启动迅速和数据可控。

- **便携运行**：不依赖安装到 `Program Files`，配置、数据和图标缓存随程序目录保存。
- **统一入口**：支持 Windows 快捷方式、可执行程序、文件夹、普通文件和网址。
- **层级整理**：使用 UUID 管理分类、分组和应用，支持最多四层分组嵌套。
- **原生图标**：解析 `.lnk`，从程序或 Shell 关联目标提取图标并缓存为 PNG。
- **可靠持久化**：通过内存 mutation 和同目录原子替换，避免留下半写入 JSON。
- **窗口记忆**：首次启动按显示器工作区居中，退出时保存窗口位置与尺寸。
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
| API 边界 | 使用 Tauri Command 返回可序列化模型和统一 `AppError` |

## 技术栈

| 领域 | 技术 |
|---|---|
| 桌面框架 | Tauri 2.11 |
| 后端 | Rust 2021、Serde、Chrono、UUID |
| Windows 集成 | `windows` 0.58、COM、Shell、GDI |
| 前端 | React 19、TypeScript 6、Vite 8 |
| 图像处理 | `image` 0.25 |
| 持久化 | Pretty JSON、同目录临时文件、原子替换 |
| 测试 | Rust 集成测试、Tauri MockRuntime |

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

```powershell
cargo tauri dev
```

Tauri 会根据 `src-tauri/tauri.conf.json` 自动启动 Vite 开发服务器。当前打开的是前端脚手架页面，后端 Command、仓储和系统层可用于继续联调。

### 构建应用

```powershell
cargo tauri build
```

## 质量检查

在仓库根目录执行：

```powershell
Set-Location src-tauri
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test -- --test-threads=1
```

最近一次后端验证结果为 40 项测试通过，覆盖模型契约、JSON 仓储、业务服务、Tauri Command、窗口状态和 Windows 系统边界。

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

> [!NOTE]
> `v0.1.0` 的 `iconUrl` 仍包含程序目录的绝对路径。整体移动程序目录后可能需要重新生成图标资源 URL；后续版本会改为按当前 `icons/` 目录动态构造。

## 后端接口

| 接口组 | Command |
|---|---|
| 数据读取 | `get_all_data` |
| 分类 | `add_category`、`rename_category`、`delete_category` |
| 分组 | `add_group`、`rename_group`、`delete_group` |
| 应用 | `add_app`、`rename_app`、`delete_app`、`launch_icon` |
| 配置 | `get_settings`、`update_settings` |
| 窗口 | `is_init` |

新增节点的 UUID 由后端生成。前端应先读取接口返回的 `id`，后续重命名、删除和启动均按 ID 定位，不能使用名称作为主键。

## 项目结构

```text
DeskCrate/
  react/                  React + TypeScript 前端
  src-tauri/
    src/
      commands/           Tauri API 边界
      services/           业务规则与流程编排
      storage/            JSON 仓储与原子写入
      system/             Windows 底层实现
      models/             持久化模型与请求模型
      state.rs            共享应用状态
      lib.rs              Tauri 应用组装入口
      main.rs             最小二进制入口
    tests/                独立集成测试
```

## 开发路线

- [x] 完成后端分层结构与稳定 Command API
- [x] 完成目录树 CRUD、配置持久化和窗口状态机
- [x] 完成快捷方式解析、图标提取和基础启动能力
- [x] 完成模型、仓储、服务、API 与 Windows 边界测试
- [ ] 接入正式 React 侧栏界面并完成前后端联调
- [ ] 增加数据迁移与损坏配置恢复策略
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
