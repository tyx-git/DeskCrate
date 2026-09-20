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

// 移动一个应用到新位置时由前端提交的请求数据。
//
// 拖拽改位置是「先摘下来、再插回去」两步，所以请求里只描述**目标**（父容器与
// 下标），源位置由后端自己查 —— 前端不必维护一份位置索引，也就不会和落盘结果
// 对不上。
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MoveAppRequest {
    pub id: String, // 要移动的应用 ID。

    // 目标父容器：分类 ID、分组 ID，`null` 表示顶层。
    //
    // 与 `AddAppRequest.parent_id` 同一套语义：`Option` 而不是哨兵字符串。
    pub parent_id: Option<String>,

    // 参照节点：插到它**之前**。`null` 表示插到目标容器的末尾。
    //
    // 为什么用「参照节点」而不是下标：顶层应用是按**行**存放的，一个下标表达不了
    // 「第几行的第几个」。而且摘掉自己会让后续元素的下标整体左移，前端很容易算
    // 偏一位。以某个具体应用为参照则没有这些歧义。
    pub before_id: Option<String>,
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

    // 放到顶层时插到第几个根节点**之前**（0 起；等于根节点数量表示放到最下方）。
    //
    // 顶层应用和分类可以交错排列，光有 `parent_id: None` 只能表达「放到顶层」，
    // 表达不了「放到哪儿」，所以补一个下标。放在分类或分组里时这个字段被忽略
    // —— 容器内目前一律追加到末尾。
    #[serde(default)]
    pub root_index: Option<usize>,
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

// 根节点上的一段：**要么是一个分类，要么是一行顶层应用**。
//
// 根节点不再「先放所有顶层应用、再放所有分类」，而是两者交错排列，顺序由用户
// 拖放决定。`Apps` 里可以有多个应用，渲染时合并成一行（就像分类下的图标网格），
// 这样顶层既能放一个、也能并排放好几个。
//
// 序列化沿用 Serde 外部标签，前端收到的形状是 `{"Category": {...}}` 或
// `{"Apps": [...]}`，与 `Item` 的判别方式一致。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RootItem {
    Category(Category),
    Apps(Vec<AppEXE>),
}

impl RootItem {
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Category(category) => Some(category.id.as_str()),
            // 一行应用没有自己的 ID：拖放落点用的是「第几个根节点」而不是节点 ID。
            Self::Apps(_) => None,
        }
    }
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
    pub version: i32, // 数据格式版本

    // 根节点的有序列表：顶层应用与分类**交错排列**，顺序就是屏幕上的顺序。
    //
    // 连续的顶层应用会合并渲染成一行，所以一行里可以有任意多个应用。
    //
    // 带 `default`：旧版本没有这个字段，缺了就当空数组，再由 `normalize()`
    // 从下面两个旧字段折出来 —— 因此**不需要破坏性迁移**，老文件照常能读。
    #[serde(default)]
    pub items: Vec<RootItem>,

    // —— 以下两个是旧版字段，只在读取历史文件时出现 ——
    //
    // 旧结构里顶层应用单独一个数组、且固定渲染在所有分类之上。`normalize()`
    // 会把它们折进 `items`（顶层应用行在前，分类依次在后，等价旧观感），此后
    // 这两个字段保持为空、不再写回。保留只是为了兼容，新代码一律只用 `items`。
    #[serde(default)]
    pub categories: Vec<Category>,
    #[serde(default)]
    pub apps: Vec<AppEXE>,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            version: 2,
            items: Vec::new(),
            categories: Vec::new(),
            apps: Vec::new(),
        }
    }
}

impl AppData {
    // 把旧版结构折进 `items`，让新旧文件在内存里只有一种形状。
    //
    // 幂等：已经归一化过的数据再调一次不会有任何变化，所以可以放心在每次
    // 读取后调用。
    pub fn normalize(&mut self) {
        if !self.items.is_empty() || (self.categories.is_empty() && self.apps.is_empty()) {
            return;
        }

        let mut items = Vec::with_capacity(self.categories.len() + 1);
        if !self.apps.is_empty() {
            items.push(RootItem::Apps(std::mem::take(&mut self.apps)));
        }
        for category in self.categories.drain(..) {
            items.push(RootItem::Category(category));
        }

        self.items = items;
        self.version = 2;
    }

    // 按显示顺序展开根节点上的所有顶层应用。
    pub fn root_apps(&self) -> impl Iterator<Item = &AppEXE> {
        self.items.iter().flat_map(|item| match item {
            RootItem::Apps(apps) => apps.as_slice(),
            RootItem::Category(_) => &[],
        })
    }

    // 根节点上的所有分类。
    pub fn root_categories(&self) -> impl Iterator<Item = &Category> {
        self.items.iter().filter_map(|item| match item {
            RootItem::Category(category) => Some(category),
            RootItem::Apps(_) => None,
        })
    }
}
