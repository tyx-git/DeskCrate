pub(crate) mod dialog;
pub(crate) mod executor;
pub(crate) mod http;
pub(crate) mod icon_extract;
// 开机自启动：往 HKCU\...\CurrentVersion\Run 里读写一条启动项。
//
// 文件名 `registory` 是历史拼写（正确应为 `registry`），暂不改名，以免动到
// 既有的引用与文档。
pub(crate) mod registory;
pub(crate) mod shortcut;
// 单实例防护：命名互斥量 + 激活已有实例的窗口。
pub(crate) mod single_instance;
pub(crate) mod support;
pub(crate) mod wide;
