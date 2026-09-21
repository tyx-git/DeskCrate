// 诊断用的命令，只在 debug 构建里存在。
//
// 网址图标抓取的成败高度依赖真实网络环境（UA、重定向、软 404），在单元测试里
// 造不出这些场景，只能拿真实站点量。这些命令就是给 `dosc/图标抓取/probe.mjs`
// 用的取数口子。
//
// 只在 debug 下注册：release 里多一个能对外发请求的命令没有意义，还平白多一份
// 攻击面（虽然 Tauri 的命令只能被应用自己的前端调用）。

use crate::{error::AppResult, services};

/// 探测一个网址的站点图标，返回每一步的细节，不落盘。
#[tauri::command]
pub fn probe_url_icon(url: String) -> AppResult<services::favicon::UrlIconProbe> {
    // `probe` 只发请求、不写数据，失败与否都算探测结果，因此不向上传播错误。
    Ok(services::favicon::probe(&url))
}
