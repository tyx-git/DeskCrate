use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// 快捷方式中记录的图标来源。
//
// `path` 可以指向 exe、dll 或 ico 文件，`index` 是 Windows 图标资源索引。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IconLocation {
    pub path: String, // 图标来源文件路径。
    pub index: i32,   // 图标资源索引，Windows 允许使用负数资源 ID。
}

// 解析 `.lnk` 文件后得到的临时结果。
//
// 这个模型用于系统层和 Service 之间传递解析结果，
// 不会直接写入 `data.json`。创建 `AppEXE` 时，
// 服务层会保留原始 `source_path`，并从这里提取目标路径、参数和工作目录。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ParsedShortcut {
    pub source_path: String, // 原始快捷方式路径。
    pub name: String,        // 默认展示名称，通常取快捷方式文件名。
    pub target_path: String, // 快捷方式最终指向的目标路径。

    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>, // 启动工作目录。

    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>, // 启动参数。

    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_location: Option<IconLocation>, // 快捷方式指定的图标来源。
}

// 图标提取并缓存后的内部结果。
//
// `cache_path` 是后端实际写入文件系统的路径。输入源路径已经由调用方
// 持有，结果不再重复返回它；系统层也不关心 Tauri 的资源协议，service
// 层可以按项目 API 约定把这个路径转换为前端可加载的 URL。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IconExtractionResult {
    pub cache_path: PathBuf, // 程序目录/icons 下的实际 PNG 路径。
}
