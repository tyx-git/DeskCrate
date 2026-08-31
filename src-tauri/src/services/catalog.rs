use std::path::{Path, PathBuf};

use chrono::Utc;
use uuid::Uuid;

use crate::{
    error::AppResult,
    models::{
        AddAppRequest, AddCategoryRequest, AddGroupRequest, AppData, AppEXE, AppError, Category,
        DeleteRequest, Group, IconType, Item, RenameRequest,
    },
    state::AppState,
    storage::DataRepository,
};

use super::icon::IconService;

// 分组树允许的最大深度；顶层分类下的直接分组算第 1 层。
const MAX_GROUP_DEPTH: usize = 4;

// 获取当前持久化的数据目录。
//
// `service` 层只编排业务流程：从 `AppState` 获取仓储锁，再调用
// `DataRepository` 读取 `data.json`。文件格式和文件系统细节仍然由
// `storage` 层负责。
pub fn get_all_data(state: &AppState) -> AppResult<AppData> {
    let store = state.get_lock()?;
    store.load_data()
}

// 添加一个顶层分类。
//
// 前端只提供分类名称。分类 ID、折叠状态、子节点集合和创建时间
// 都由后端生成，避免客户端伪造持久化元数据。
pub fn add_category(state: &AppState, request: AddCategoryRequest) -> AppResult<Category> {
    let name = normalize_name(request.name, "invalid_category_name", "分类")?;

    let category = Category {
        id: Uuid::new_v4().to_string(),
        name,
        is_collapsed: false,
        groups: Vec::new(),
        created_time: Utc::now().to_rfc3339(),
    };

    let store = state.get_lock()?;
    store.mutate_data(|data| {
        data.categories.push(category.clone());
        Ok(())
    })?;

    Ok(category)
}

// 按 UUID 重命名顶层分类。
pub fn rename_category(state: &AppState, request: RenameRequest) -> AppResult<Category> {
    validate_id(&request.id)?;
    let name = normalize_name(request.name, "invalid_category_name", "分类")?;

    let store = state.get_lock()?;
    store.mutate_data(|data| {
        let category = data
            .categories
            .iter_mut()
            .find(|category| category.id == request.id)
            .ok_or_else(|| not_found("category_not_found", "分类", &request.id))?;

        category.name = name;
        Ok(category.clone())
    })
}

// 按 UUID 删除顶层分类及其全部子节点。
pub fn delete_category(state: &AppState, request: DeleteRequest) -> AppResult<()> {
    validate_id(&request.id)?;

    let store = state.get_lock()?;
    let removed = store.mutate_data(|data| {
        let index = data
            .categories
            .iter()
            .position(|category| category.id == request.id)
            .ok_or_else(|| not_found("category_not_found", "分类", &request.id))?;

        Ok(data.categories.remove(index))
    })?;
    cleanup_category_icons(&removed, store.icons_path());
    Ok(())
}

// 在分类或已有分组下新增一个子分组。
//
// `parent_id` 可以指向顶层分类，也可以指向任意嵌套分组；
// 因此后续即使开放四层以内的分组嵌套，也不需要改变 Command API。
pub fn add_group(state: &AppState, request: AddGroupRequest) -> AppResult<Group> {
    validate_id(&request.parent_id)?;
    let name = normalize_name(request.name, "invalid_group_name", "分组")?;

    let store = state.get_lock()?;

    let group = Group {
        id: Uuid::new_v4().to_string(),
        name,
        is_collapsed: false,
        children: Vec::new(),
        created_time: Utc::now().to_rfc3339(),
    };

    store.mutate_data(|data| {
        let parent_depth = parent_depth(data, &request.parent_id)
            .ok_or_else(|| not_found("parent_not_found", "分组的父节点", &request.parent_id))?;

        if parent_depth >= MAX_GROUP_DEPTH {
            return Err(AppError::new(
                "group_depth_exceeded",
                format!("分组最多只能嵌套 {MAX_GROUP_DEPTH} 层"),
            ));
        }

        if !append_group(data, &request.parent_id, group.clone()) {
            return Err(not_found(
                "parent_not_found",
                "分组的父节点",
                &request.parent_id,
            ));
        }
        Ok(())
    })?;
    Ok(group)
}

// 按 UUID 重命名任意层级的子分组。
pub fn rename_group(state: &AppState, request: RenameRequest) -> AppResult<Group> {
    validate_id(&request.id)?;
    let name = normalize_name(request.name, "invalid_group_name", "分组")?;

    let store = state.get_lock()?;
    store.mutate_data(|data| {
        let group = find_group_mut(data, &request.id)
            .ok_or_else(|| not_found("group_not_found", "分组", &request.id))?;

        group.name = name;
        Ok(group.clone())
    })
}

// 按 UUID 删除任意层级的子分组及其全部后代。
pub fn delete_group(state: &AppState, request: DeleteRequest) -> AppResult<()> {
    validate_id(&request.id)?;

    let store = state.get_lock()?;
    let removed = store.mutate_data(|data| {
        remove_item_from_data(data, &request.id, ItemKind::Group)
            .ok_or_else(|| not_found("group_not_found", "分组", &request.id))
    })?;
    cleanup_item_icons(&removed, store.icons_path());
    Ok(())
}

// 新增应用或快捷方式。
//
// 图标解析属于耗时且可能调用 Windows API 的工作，故刻意分成三段：
//
// 1. 短暂获取锁，复制图标目录并确认父节点存在；
// 2. 释放锁后解析快捷方式、提取图标；
// 3. 重新获取锁，重新读取最新数据并按 ID 写入应用。
//
// 这样图标处理不会长时间阻塞其他目录操作，也不会让 `MutexGuard`
// 跨越未来可能加入的异步边界。
pub fn add_app(state: &AppState, request: AddAppRequest) -> AppResult<AppEXE> {
    validate_id(&request.parent_id)?;
    let source = normalize_source(request.source_path)?;
    let app_id = Uuid::new_v4().to_string();

    // 只在这里持锁读取父节点和复制图标目录；后续系统操作不持有锁。
    let icon_service = {
        let store = state.get_lock()?;
        let data = store.load_data()?;
        if !parent_exists(&data, &request.parent_id) {
            return Err(not_found(
                "parent_not_found",
                "应用的父节点",
                &request.parent_id,
            ));
        }
        IconService::from_store(&store)
    };

    let (name, working_dir, arguments, icon_type, icon_url, cache_path) =
        if let Source::Url(url) = &source {
            let name = choose_app_name(request.name.as_deref(), default_url_name(url))?;

            // URL 没有本地文件图标，使用 service 层提供的内置 SVG data URL；
            // 它本身就是可加载资源，不需要把 URL 伪装成本地路径。
            (
                name,
                None,
                None,
                IconType::Url,
                DEFAULT_URL_ICON_URL.to_owned(),
                None,
            )
        } else {
            let Source::LocalPath(source_path) = &source else {
                unreachable!("来源类型已在上面的 URL 分支处理")
            };
            let is_shortcut = is_shortcut_path(source_path);

            if is_shortcut {
                let shortcut = icon_service.parse_shortcut(source_path)?;
                // 这里必须复制名称：后面还要借用完整的 `shortcut` 提取图标。
                // 直接移动 `shortcut.name` 会使 `shortcut` 部分移动，无法再借用。
                let name = choose_app_name(request.name.as_deref(), shortcut.name.clone())?;
                let icon_result = icon_service.extract_from_shortcut(&shortcut, &app_id)?;
                let icon_url = IconService::asset_url(&icon_result.cache_path);

                (
                    name,
                    shortcut.working_dir,
                    shortcut.arguments,
                    IconType::Shortcut,
                    icon_url,
                    Some(icon_result.cache_path),
                )
            } else {
                let name = choose_app_name(request.name.as_deref(), default_app_name(source_path))?;
                let icon_result = icon_service.extract_from_path(source_path, &app_id)?;
                let icon_type = classify_source(source_path);
                let icon_url = IconService::asset_url(&icon_result.cache_path);

                (
                    name,
                    None,
                    None,
                    icon_type,
                    icon_url,
                    Some(icon_result.cache_path),
                )
            }
        };

    let app = AppEXE {
        id: app_id,
        name,
        source_path: source.as_string(),
        arguments,
        working_dir,
        icon_url,
        icon_type,
        created_time: Utc::now().to_rfc3339(),
    };

    let store = match state.get_lock() {
        Ok(store) => store,
        Err(error) => {
            remove_cached_icon(cache_path.as_deref());
            return Err(error);
        }
    };
    if let Err(error) = store.mutate_data(|data| {
        if append_app(data, &request.parent_id, app.clone()) {
            Ok(())
        } else {
            Err(not_found(
                "parent_not_found",
                "应用的父节点",
                &request.parent_id,
            ))
        }
    }) {
        drop(store);
        remove_cached_icon(cache_path.as_deref());
        return Err(error);
    }

    Ok(app)
}

// 按 UUID 重命名应用。
pub fn rename_app(state: &AppState, request: RenameRequest) -> AppResult<AppEXE> {
    validate_id(&request.id)?;
    let name = normalize_name(request.name, "invalid_app_name", "应用")?;

    let store = state.get_lock()?;
    store.mutate_data(|data| {
        let app = find_app_mut(data, &request.id)
            .ok_or_else(|| not_found("app_not_found", "应用", &request.id))?;

        app.name = name;
        Ok(app.clone())
    })
}

// 按 UUID 删除应用。
pub fn delete_app(state: &AppState, request: DeleteRequest) -> AppResult<()> {
    validate_id(&request.id)?;

    let store = state.get_lock()?;
    let removed = store.mutate_data(|data| {
        remove_item_from_data(data, &request.id, ItemKind::App)
            .ok_or_else(|| not_found("app_not_found", "应用", &request.id))
    })?;
    cleanup_item_icons(&removed, store.icons_path());
    Ok(())
}

// 目录树节点种类，用于删除时避免用错误的接口删除另一种节点。
#[derive(Clone, Copy)]
enum ItemKind {
    Group,
    App,
}

// 在顶层分类或嵌套分组下追加分组。
fn append_group(data: &mut AppData, parent_id: &str, group: Group) -> bool {
    if let Some(category) = data
        .categories
        .iter_mut()
        .find(|category| category.id == parent_id)
    {
        category.groups.push(Item::Group(group));
        return true;
    }

    if let Some(parent) = find_group_mut(data, parent_id) {
        parent.children.push(Item::Group(group));
        return true;
    }

    false
}

// 在顶层分类或嵌套分组下追加应用。
fn append_app(data: &mut AppData, parent_id: &str, app: AppEXE) -> bool {
    if let Some(category) = data
        .categories
        .iter_mut()
        .find(|category| category.id == parent_id)
    {
        category.groups.push(Item::App(app));
        return true;
    }

    if let Some(parent) = find_group_mut(data, parent_id) {
        parent.children.push(Item::App(app));
        return true;
    }

    false
}

// 在所有分类中递归查找分组。
fn find_group_mut<'a>(data: &'a mut AppData, id: &str) -> Option<&'a mut Group> {
    for category in &mut data.categories {
        if let Some(group) = find_group_in_items_mut(&mut category.groups, id) {
            return Some(group);
        }
    }

    None
}

fn find_group_in_items_mut<'a>(items: &'a mut [Item], id: &str) -> Option<&'a mut Group> {
    for item in items {
        if let Item::Group(group) = item {
            // 先在当前节点结束 ID 比较产生的借用，再递归借用 children。
            // 使用 match guard 会让编译器认为同一个枚举字段的可变借用
            // 仍然跨越后续分支，从而触发 E0499。
            if group.id == id {
                return Some(group);
            }

            if let Some(found) = find_group_in_items_mut(&mut group.children, id) {
                return Some(found);
            }
        }
    }
    None
}

// 在所有分类中递归查找应用。
fn find_app_mut<'a>(data: &'a mut AppData, id: &str) -> Option<&'a mut AppEXE> {
    for category in &mut data.categories {
        if let Some(app) = find_app_in_items_mut(&mut category.groups, id) {
            return Some(app);
        }
    }

    None
}

fn find_app_in_items_mut<'a>(items: &'a mut [Item], id: &str) -> Option<&'a mut AppEXE> {
    for item in items {
        if let Item::App(app) = item {
            // 与分组查找保持相同的借用边界：先比较当前应用 ID，
            // 只有当前节点不匹配时才继续处理其他节点。
            if app.id == id {
                return Some(app);
            }
        } else if let Item::Group(group) = item {
            if let Some(found) = find_app_in_items_mut(&mut group.children, id) {
                return Some(found);
            }
        }
    }

    None
}

// 判断一个 ID 是否可以作为分类或分组的父节点。
fn parent_exists(data: &AppData, id: &str) -> bool {
    data.categories.iter().any(|category| category.id == id)
        || data
            .categories
            .iter()
            .any(|category| find_group_in_items(&category.groups, id))
}

// 返回父节点所在的分组深度。
//
// 顶层分类返回 0，分类下的直接分组返回 1，以此类推；新增分组时
// 会把新节点深度加 1，并据此执行最大层数校验。
fn parent_depth(data: &AppData, id: &str) -> Option<usize> {
    if data.categories.iter().any(|category| category.id == id) {
        return Some(0);
    }

    data.categories
        .iter()
        .find_map(|category| find_group_depth(&category.groups, id, 1))
}

fn find_group_depth(items: &[Item], id: &str, depth: usize) -> Option<usize> {
    for item in items {
        match item {
            Item::Group(group) if group.id == id => return Some(depth),
            Item::Group(group) => {
                if let Some(found) = find_group_depth(&group.children, id, depth + 1) {
                    return Some(found);
                }
            }
            Item::App(_) => {}
        }
    }

    None
}

fn find_group_in_items(items: &[Item], id: &str) -> bool {
    items.iter().any(|item| match item {
        Item::Group(group) if group.id == id => true,
        Item::Group(group) => find_group_in_items(&group.children, id),
        Item::App(_) => false,
    })
}

// 从树中递归删除指定种类的节点，并返回被删除的节点。
fn remove_item_from_data(data: &mut AppData, id: &str, kind: ItemKind) -> Option<Item> {
    for category in &mut data.categories {
        if let Some(removed) = remove_item(&mut category.groups, id, kind) {
            return Some(removed);
        }
    }

    None
}

fn remove_item(items: &mut Vec<Item>, id: &str, kind: ItemKind) -> Option<Item> {
    if let Some(index) = items.iter().position(|item| item_matches(item, id, kind)) {
        return Some(items.remove(index));
    }

    for item in items.iter_mut() {
        if let Item::Group(group) = item {
            if let Some(removed) = remove_item(&mut group.children, id, kind) {
                return Some(removed);
            }
        }
    }

    None
}

fn item_matches(item: &Item, id: &str, kind: ItemKind) -> bool {
    match item {
        Item::Group(group) => matches!(kind, ItemKind::Group) && group.id == id,
        Item::App(app) => matches!(kind, ItemKind::App) && app.id == id,
    }
}

fn normalize_name(
    name: String,
    error_code: &'static str,
    entity_name: &'static str,
) -> AppResult<String> {
    let normalized = name.trim();

    if normalized.is_empty() {
        return Err(AppError::new(
            error_code,
            format!("{entity_name}名称不能为空"),
        ));
    }

    Ok(normalized.to_owned())
}

fn choose_app_name(requested_name: Option<&str>, fallback: String) -> AppResult<String> {
    match requested_name {
        Some(name) => normalize_name(name.to_owned(), "invalid_app_name", "应用"),
        None => normalize_name(fallback, "invalid_app_name", "应用"),
    }
}

fn validate_id(id: &str) -> AppResult<()> {
    if id.trim().is_empty() {
        return Err(AppError::new("invalid_id", "节点 ID 不能为空"));
    }

    Ok(())
}

const DEFAULT_URL_ICON_URL: &str =
    "data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22 viewBox=%220 0 24 24%22%3E%3Ccircle cx=%2212%22 cy=%2212%22 r=%229%22 fill=%22none%22 stroke=%22black%22 stroke-width=%222%22/%3E%3Cpath d=%22M3 12h18M12 3c3 2.5 3 15 0 18M12 3c-3 2.5-3 15 0 18%22 fill=%22none%22 stroke=%22black%22 stroke-width=%222%22/%3E%3C/svg%3E";

enum Source {
    LocalPath(PathBuf),
    Url(String),
}

impl Source {
    fn as_string(&self) -> String {
        match self {
            Self::LocalPath(path) => path.to_string_lossy().into_owned(),
            Self::Url(url) => url.clone(),
        }
    }
}

fn normalize_source(source: String) -> AppResult<Source> {
    let normalized = source.trim();

    if normalized.is_empty() {
        return Err(AppError::new(
            "invalid_source_path",
            "应用来源路径或网址不能为空",
        ));
    }

    if normalized.contains('\0') {
        return Err(AppError::new(
            "invalid_source_path",
            "应用来源路径或网址不能包含 NUL 字符",
        ));
    }

    if is_supported_url(normalized) {
        return Ok(Source::Url(normalized.to_owned()));
    }

    Ok(Source::LocalPath(PathBuf::from(normalized)))
}

fn is_supported_url(value: &str) -> bool {
    let Some((scheme, remainder)) = value.split_once("://") else {
        return false;
    };

    !remainder.is_empty()
        && !scheme.is_empty()
        && scheme.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
        && matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https")
}

fn default_url_name(url: &str) -> String {
    url.split_once("://")
        .map(|(_, value)| value)
        .and_then(|value| value.split('/').next())
        .filter(|value| !value.is_empty())
        .unwrap_or("网址")
        .to_owned()
}

fn is_shortcut_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"))
}

fn classify_source(path: &Path) -> IconType {
    if path.is_dir() {
        return IconType::Folder;
    }

    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("exe") => IconType::Executable,
        _ => IconType::File,
    }
}

fn default_app_name(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "应用".to_owned())
}

fn remove_cached_icon(path: Option<&Path>) {
    // 数据写入失败时尽力清理刚刚生成的缓存文件；清理失败不覆盖原始错误。
    if let Some(path) = path {
        let _ = std::fs::remove_file(path);
    }
}

// 删除已经从数据树中移除的节点所对应的图标缓存。
//
// 图标文件名由后端生成并固定为 `{app_id}.png`，因此只清理这个约定
// 下的文件，不直接信任 JSON 中可能被修改的任意路径。
fn cleanup_category_icons(category: &Category, icons_dir: &Path) {
    for item in &category.groups {
        cleanup_item_icons(item, icons_dir);
    }
}

fn cleanup_item_icons(item: &Item, icons_dir: &Path) {
    match item {
        Item::App(app) => {
            let expected_name = format!("{}.png", app.id);
            // `icon_url` 是给 WebView 使用的 URL，可能已经把磁盘路径
            // percent-encode，不能把它当作本地 Path 解析。缓存文件名
            // 由后端按 app.id 生成，因此这里直接使用受信任的 ID。
            let _ = std::fs::remove_file(icons_dir.join(expected_name));
        }
        Item::Group(group) => {
            for child in &group.children {
                cleanup_item_icons(child, icons_dir);
            }
        }
    }
}

fn not_found(code: &'static str, entity_name: &str, id: &str) -> AppError {
    AppError::new(code, format!("找不到{entity_name}: {id}"))
}
