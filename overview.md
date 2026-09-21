# DeskCrate 本轮工作概览

## 已完成

- 设置页新增「开机自启动」开关。
- 使用当前用户注册表 `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run\\DeskCrate` 管理开机启动，不需要管理员权限。
- 启动时按 `config.json` 对注册表状态做校准，注册表失败不阻塞应用启动。
- 网址图标支持位图与 SVG：位图保存为 `.png`，SVG 原文保存为 `.svg`。
- 图标复制、删除、孤儿清理同时支持 `.png` / `.svg`。
- 同源复用继续生效：同 `scheme + host + port` 的网址优先复用已缓存图标，避免重复联网抓取。
- 按要求不接入公共 favicon 服务，抓取失败继续使用默认地球图标。
- 更新网址图标增强设计文档与相关测试。

## 验证结果

- Rust 全量测试通过。
- `cargo fmt --all` 通过。
- `cargo clippy --all-targets -- -D warnings` 通过。
- 前端 `npm run build` 通过（使用托管 Node 运行时）。
- 设置页回归通过，包含开机自启动设置项。
- release 回归通过。
- release 构建命令：`cargo build --release --features custom-protocol -j 2`。

## 交付物

- `src-tauri/target/release/app.exe`
- `dosc/plan/图标抓取增强设计.md`
