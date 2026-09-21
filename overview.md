# DeskCrate 本轮工作概览

## 已完成

- 设置页新增「开机自启动」开关。
- 使用当前用户注册表 `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run\\DeskCrate` 管理开机启动，不需要管理员权限。
- 启动时按 `config.json` 对注册表状态做校准，注册表失败不阻塞应用启动。
- 网址图标支持位图与 SVG：位图保存为 `.png`，SVG 原文保存为 `.svg`。
- 图标复制、删除、孤儿清理同时支持 `.png` / `.svg`。
- 同源复用继续生效：同 `scheme + host + port` 的网址优先复用已缓存图标，避免重复联网抓取。
- 按要求不接入公共 favicon 服务，抓取失败继续使用默认地球图标。
- 修复 GitHub 图标抓取：GitHub 首页及 `/favicon.ico` 在当前网络返回 403 时，优先尝试 GitHub 官方静态资源 `https://github.githubassets.com/favicons/favicon.svg`，不经过公共 favicon 服务。
- 更新网址图标增强设计文档与相关测试。
- 针对 `FetchedIcon` 类型不匹配和 Windows 注册表 API 导入错误复核了当前源码；当前源码已能通过 `cargo check --all-targets`，说明 IDE 中的相关诊断应刷新索引后消失。

## GitHub 修复实测

- 修复前：`github.com` 失败，`0x80072EE2` / `favicon.ico` 403。
- 修复后：成功获取官方 SVG，959 字节，约 1.6 秒。
- 探针清单成功率：19/31（61%）提升到 25/31（81%）。

## 验证结果

- Rust 全量测试通过。
- `cargo fmt --all` 通过。
- `cargo check --all-targets` 通过。
- `cargo clippy --all-targets -- -D warnings` 通过。
- 前端 `npm run build` 通过（使用托管 Node 运行时）。
- 设置页回归通过，包含开机自启动设置项。
- release 回归通过。
- release 构建命令：`cargo build --release --features custom-protocol -j 2`。

## 交付物

- `src-tauri/target/release/app.exe`
- `dosc/plan/图标抓取增强设计.md`
