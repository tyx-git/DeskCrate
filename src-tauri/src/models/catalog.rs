use serde::{Deserialize, Serialize};

// 应用节点的来源/打开方式。
//
// 枚举值同时用于持久化和前端判断图标节点的行为，新增变体时要同步
// 更新前端类型和 service 层的路径识别逻辑。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum IconType {
    Shortcut,   // Windows 快捷方式（.lnk）。
    Executable, // 可执行文件（.exe）。
    Folder,
    File,
    Url,
}

// 分类树中的通用节点。
//
// 采用外部标签序列化，前端收到的节点形状为：
//
// ```json
// { "Group": { "id": "...", "name": "..." } }
// ```
// 或：
//```json
// { "App": { "id": "...", "name": "..." } }
// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Item {
    Group(Group),
    App(AppEXE),
}

// 返回给前端的统一错误结构。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppError {
    pub code: String,
    pub message: String,
}

// 新增顶层分类时由前端提交的请求数据。
//
// 这里只保留前端可以决定的字段。分类 ID、折叠状态、子节点集合和
// 创建时间由 service 层生成，避免前端伪造持久化元数据。
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AddCategoryRequest {
    pub name: String, // 分类名称，必填；具体内容由 service 层校验。
}

// 重命名分类、分组或应用时使用的通用请求。
//
// `id` 始终是后端生成的稳定 UUID，`name` 只负责更新展示名称。
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenameRequest {
    pub id: String,   // 要修改的节点 ID。
    pub name: String, // 修改后的展示名称。
}

// 删除分类、分组或应用时使用的通用请求。
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteRequest {
    pub id: String, // 要删除的节点 ID。
}

// 新增子分组时由前端提交的请求数据。
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AddGroupRequest {
    pub parent_id: String, // 父分类或父分组的 ID。
    pub name: String,      // 分组展示名称。
}

// 新增应用或快捷方式时由前端提交的请求数据。
//
// `source_path` 是用户拖入或选择的原始路径。快捷方式的目标路径、
// 启动参数、工作目录和图标由 service 层解析生成，前端不直接提交。
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AddAppRequest {
    // 要放入的父分类或父分组 ID；`None` 表示直接放在顶层。
    //
    // 用 `Option` 而不是约定的哨兵字符串：顶层没有对应节点，任何哨兵值
    // 都可能和真实 UUID 撞车或被人手写配置时误用，而「没有父节点」本来
    // 就是这件事最准确的说法。
    pub parent_id: Option<String>,
    pub source_path: String, // 原始路径或网址；快捷方式必须保留 .lnk 来源。
    #[serde(default)]
    pub name: Option<String>, // 可选的自定义展示名称；省略时由后端推导。
}

// 快捷方式或应用程序节点。
//
// Rust 使用 snake_case 字段；通过 `rename_all` 序列化后，
// 前端使用 camelCase 字段。
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppEXE {
    pub id: String,          // 稳定的应用节点 ID。
    pub name: String,        // 前端展示名称。
    pub source_path: String, // 用户最初添加的路径或网址；快捷方式保留 .lnk 来源。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>, // 快捷方式或启动目标的原始参数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>, // 可选的启动工作目录。
    pub icon_url: String,    // 前端可使用的图标资源 URL。
    pub icon_type: IconType, // 应用来源类型。
    pub created_time: String, // 创建时间，ISO 8601/RFC 3339。
}

// 顶层分类。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,   // 稳定的分类 ID。
    pub name: String, // 前端展示名称
    #[serde(rename = "isCollapsed")]
    pub is_collapsed: bool, // 分类是否折叠。前端字段固定为isCollapsed
    pub groups: Vec<Item>, // 分类下的分组或应用节点
    pub created_time: String, //创建时间
}

// 子分类。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,   // 稳定的分组 ID。
    pub name: String, // 前端展示名称。
    #[serde(rename = "isCollapsed")]
    pub is_collapsed: bool, // 分组是否折叠。
    pub children: Vec<Item>, // 分组下的应用或继续嵌套的分组。
    pub created_time: String, // 创建时间
}

// 应用保存文件中的根数据。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppData {
    pub version: i32,              // 数据格式版本
    pub categories: Vec<Category>, // 所有顶层分类
    // 直接挂在顶层的应用。
    //
    // 分类只是「收纳方式」，不是必须的容器：常用的那几个程序应该能一步
    // 放到侧栏最上面，不必为了放一个应用先建一个分类。顶层应用渲染在所
    // 有分类之前。
    //
    // 带 `default`：旧版本的 `data.json` 里没有这个字段，缺了就当空数组，
    // 因此不需要写数据迁移，老文件照常能读。
    #[serde(default)]
    pub apps: Vec<AppEXE>,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            version: 1,
            categories: Vec::new(),
            apps: Vec::new(),
        }
    }
}
