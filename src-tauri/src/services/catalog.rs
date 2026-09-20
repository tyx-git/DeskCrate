use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::Utc;
use uuid::Uuid;

use crate::{
    error::AppResult,
    models::{
        AddAppRequest, AddCategoryRequest, AddGroupRequest, AppData, AppEXE, AppError, Category,
        DeleteRequest, Group, IconType, Item, RenameRequest,
    },
    path,
    state::AppState,
    storage::DataRepository,
};

use super::{icon::IconService, tree};

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
    let name = normalize_name(&request.name, "invalid_category_name", "分类")?;

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
    let name = normalize_name(&request.name, "invalid_category_name", "分类")?;

    let store = state.get_lock()?;
    store.mutate_data(|data| {
        let category = tree::find_category_mut(data, &request.id)
            .ok_or_else(|| not_found("category_not_found", "分类", &request.id))?;

        category.name = name;
        Ok(category.clone())
    })
}

// 按 UUID 删除顶层分类及其全部子节点。
pub fn delete_category(state: &AppState, request: DeleteRequest) -> AppResult<()> {
    validate_id(&request.id)?;

    let (removed, icons_dir) = {
        let store = state.get_lock()?;
        let removed = store.mutate_data(|data| {
            let removed = tree::remove_category(data, &request.id)
                .ok_or_else(|| not_found("category_not_found", "分类", &request.id))?;
            Ok(removed)
        })?;
        (removed, store.icons_path().to_path_buf())
    };
    cleanup_category_icons(&removed, &icons_dir);
    Ok(())
}

// 在分类或已有分组下新增一个子分组。
//
// `parent_id` 可以指向顶层分类，也可以指向任意嵌套分组；
// 因此后续即使开放四层以内的分组嵌套，也不需要改变 Command API。
pub fn add_group(state: &AppState, request: AddGroupRequest) -> AppResult<Group> {
    validate_id(&request.parent_id)?;
    let name = normalize_name(&request.name, "invalid_group_name", "分组")?;

    let group = Group {
        id: Uuid::new_v4().to_string(),
        name,
        is_collapsed: false,
        children: Vec::new(),
        created_time: Utc::now().to_rfc3339(),
    };

    let store = state.get_lock()?;
    store.mutate_data(|data| {
        let parent_depth = tree::parent_depth(data, &request.parent_id)
            .ok_or_else(|| not_found("parent_not_found", "分组的父节点", &request.parent_id))?;

        if parent_depth >= MAX_GROUP_DEPTH {
            return Err(AppError::new(
                "group_depth_exceeded",
                format!("分组最多只能嵌套 {MAX_GROUP_DEPTH} 层"),
            ));
        }

        append_item(
            data,
            &request.parent_id,
            Item::Group(group.clone()),
            "分组的父节点",
        )
    })?;
    Ok(group)
}

// 按 UUID 重命名任意层级的子分组。
pub fn rename_group(state: &AppState, request: RenameRequest) -> AppResult<Group> {
    validate_id(&request.id)?;
    let name = normalize_name(&request.name, "invalid_group_name", "分组")?;

    let store = state.get_lock()?;
    store.mutate_data(|data| {
        let group = tree::find_group_mut(data, &request.id)
            .ok_or_else(|| not_found("group_not_found", "分组", &request.id))?;

        group.name = name;
        Ok(group.clone())
    })
}

// 按 UUID 删除任意层级的子分组及其全部后代。
pub fn delete_group(state: &AppState, request: DeleteRequest) -> AppResult<()> {
    delete_item(
        state,
        request,
        tree::NodeKind::Group,
        "group_not_found",
        "分组",
    )
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
    // `parent_id` 为 `None` 表示放到顶层，此时没有父节点需要校验。
    if let Some(parent_id) = &request.parent_id {
        validate_id(parent_id)?;
    }
    let source = normalize_source(request.source_path)?;
    if let Source::LocalPath(source_path) = &source {
        validate_local_source(source_path)?;
    }
    let app_id = Uuid::new_v4().to_string();

    // 只在这里持锁读取父节点和复制图标目录；后续系统操作不持有锁。
    let icon_service = {
        let store = state.get_lock()?;
        let data = store.load_data()?;
        if let Some(parent_id) = &request.parent_id {
            if !tree::parent_exists(&data, parent_id) {
                return Err(not_found("parent_not_found", "应用的父节点", parent_id));
            }
        }
        IconService::from_store(&store)
    };

    let prepared = prepare_app(&source, request.name.as_deref(), &app_id, &icon_service)?;
    let PreparedApp {
        name,
        working_dir,
        arguments,
        icon_type,
        icon_url,
        cache_path,
    } = prepared;

    let app = AppEXE {
        id: app_id,
        name,
        source_path: source.into_string(),
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
        // 顶层应用没有父容器，直接挂在根节点的 `apps` 上；其余情况沿用
        // 「按 parent_id 找到分类或分组再挂进去」的既有路径。
        match &request.parent_id {
            Some(parent_id) => append_item(data, parent_id, Item::App(app.clone()), "应用的父节点"),
            None => {
                data.apps.push(app.clone());
                Ok(())
            }
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
    let name = normalize_name(&request.name, "invalid_app_name", "应用")?;

    let store = state.get_lock()?;
    store.mutate_data(|data| {
        let app = tree::find_app_mut(data, &request.id)
            .ok_or_else(|| not_found("app_not_found", "应用", &request.id))?;

        app.name = name;
        Ok(app.clone())
    })
}

// 按 UUID 删除应用。
pub fn delete_app(state: &AppState, request: DeleteRequest) -> AppResult<()> {
    delete_item(state, request, tree::NodeKind::App, "app_not_found", "应用")
}

// 在顶层分类或嵌套分组下追加一个节点。
//
// 分类和分组都可以作为父节点，应用与分组只是插入位置不同、节点类型
// 不同；统一接收 `Item` 可以避免维护两份完全相同的查找逻辑。
fn append_item(
    data: &mut AppData,
    parent_id: &str,
    item: Item,
    parent_name: &'static str,
) -> AppResult<()> {
    if tree::append_child(data, parent_id, item) {
        Ok(())
    } else {
        Err(not_found("parent_not_found", parent_name, parent_id))
    }
}

// 删除应用或分组的公共流程：先在原子事务中移除节点，再释放仓储锁，
// 最后清理图标缓存，避免文件系统操作占用内存锁。
fn delete_item(
    state: &AppState,
    request: DeleteRequest,
    kind: tree::NodeKind,
    error_code: &'static str,
    entity_name: &'static str,
) -> AppResult<()> {
    validate_id(&request.id)?;

    let (removed, icons_dir) = {
        let store = state.get_lock()?;
        let removed = store.mutate_data(|data| {
            tree::remove_node(data, &request.id, kind)
                .ok_or_else(|| not_found(error_code, entity_name, &request.id))
        })?;
        (removed, store.icons_path().to_path_buf())
    };

    cleanup_item_icons(&removed, &icons_dir);
    Ok(())
}

// 图标、快捷方式字段和展示名称在写入 AppEXE 前统一整理成这个中间结果，
// 让 `add_app` 只负责仓储事务，不承担多分支字段组装。
struct PreparedApp {
    name: String,
    working_dir: Option<String>,
    arguments: Option<String>,
    icon_type: IconType,
    icon_url: String,
    cache_path: Option<PathBuf>,
}

fn prepare_app(
    source: &Source,
    requested_name: Option<&str>,
    app_id: &str,
    icon_service: &IconService,
) -> AppResult<PreparedApp> {
    match source {
        Source::Url(url) => {
            let fallback = default_url_name(url);
            // 先试着把站点的 favicon 抓下来，抓到就和本地程序一样有个真实图标。
            // 抓不到（站点没图标、离线、超时）不影响添加，退回内置的地球图标。
            let (icon_url, cache_path) = match icon_service.fetch_url_icon(url, app_id) {
                Some(result) => (
                    IconService::asset_url(&result.cache_path),
                    Some(result.cache_path),
                ),
                None => (DEFAULT_URL_ICON_URL.to_owned(), None),
            };

            Ok(PreparedApp {
                name: choose_app_name(requested_name, &fallback)?,
                working_dir: None,
                arguments: None,
                icon_type: IconType::Url,
                icon_url,
                cache_path,
            })
        }
        Source::LocalPath(source_path) if path::is_shortcut(source_path) => {
            let shortcut = icon_service.parse_shortcut(source_path)?;
            let name = choose_app_name(requested_name, &shortcut.name)?;
            let icon_result = icon_service.extract_from_shortcut(&shortcut, app_id)?;
            let icon_url = IconService::asset_url(&icon_result.cache_path);

            Ok(PreparedApp {
                name,
                working_dir: shortcut.working_dir,
                arguments: shortcut.arguments,
                icon_type: IconType::Shortcut,
                icon_url,
                cache_path: Some(icon_result.cache_path),
            })
        }
        Source::LocalPath(source_path) => {
            let fallback = default_app_name(source_path);
            let name = choose_app_name(requested_name, &fallback)?;
            let icon_result = icon_service.extract_from_path(source_path, app_id)?;
            let icon_url = IconService::asset_url(&icon_result.cache_path);

            Ok(PreparedApp {
                name,
                working_dir: None,
                arguments: None,
                icon_type: classify_source(source_path),
                icon_url,
                cache_path: Some(icon_result.cache_path),
            })
        }
    }
}

fn normalize_name(
    name: &str,
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

fn choose_app_name(requested_name: Option<&str>, fallback: &str) -> AppResult<String> {
    match requested_name {
        Some(name) => normalize_name(name, "invalid_app_name", "应用"),
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
    fn into_string(self) -> String {
        match self {
            Self::LocalPath(path) => path.to_string_lossy().into_owned(),
            Self::Url(url) => url,
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

    if path::is_web_url(normalized) {
        return Ok(Source::Url(normalized.to_owned()));
    }

    Ok(Source::LocalPath(PathBuf::from(normalized)))
}

// 本地来源必须在创建节点前可读取；否则 Shell 可能仅凭扩展名返回一个
// 图标，导致失效路径被误保存。URL 不经过这个校验。
fn validate_local_source(source_path: &Path) -> AppResult<()> {
    match std::fs::metadata(source_path) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(AppError::new(
            "source_not_found",
            format!("应用来源不存在: {}", source_path.display()),
        )),
        Err(error) => Err(AppError::new(
            "source_access_failed",
            format!("无法读取应用来源 {}: {error}", source_path.display()),
        )),
    }
}

fn default_url_name(url: &str) -> String {
    url.split_once("://")
        .map(|(_, value)| value)
        .and_then(|value| value.split('/').next())
        .filter(|value| !value.is_empty())
        .unwrap_or("网址")
        .to_owned()
}

fn classify_source(path: &Path) -> IconType {
    if path.is_dir() {
        return IconType::Folder;
    }

    if path::is_executable(path) {
        IconType::Executable
    } else {
        IconType::File
    }
}

fn default_app_name(path: &Path) -> String {
    path::display_name(path, "应用")
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
            let path = icons_dir.join(expected_name);

            // 删除失败不能静默吞掉：Windows 上文件被其他进程占用时删除会失败，
            // 留下的孤儿只能靠 `sweep_orphan_icons` 兜底，这里至少留下线索。
            // `NotFound` 属于正常情况（URL 应用本来就没有图标文件），不算失败。
            if let Err(error) = std::fs::remove_file(&path) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("清理图标缓存失败 {}: {error}", path.display());
                }
            }
        }
        Item::Group(group) => {
            for child in &group.children {
                cleanup_item_icons(child, icons_dir);
            }
        }
    }
}

// 启动时对账图标缓存，删除不再被任何应用引用的 `{UUID}.png`。
//
// 正常路径下删除应用/分组/分类都会同步清掉图标，所以这个对账只兜异常情况：
// 进程在「写完 PNG」和「提交 data.json」之间崩溃、或删除时文件被占用而失败。
// 这类孤儿没有任何界面能反映出来，不对账就会一直堆积。
//
// 只处理严格符合 `{UUID}.png` 的文件 —— 图标文件名由后端按 app.id 生成，
// 不符合这个形状的文件不是本程序写的，一律不动，避免误删用户自己的东西。
//
// 返回被删除的文件名（如 `{uuid}.png`），供调用方记录日志。
pub fn sweep_orphan_icons(state: &AppState) -> AppResult<Vec<String>> {
    let (referenced, icons_dir) = {
        let store = state.get_lock()?;
        let data = store.load_data()?;
        (collect_app_ids(&data), store.icons_path().to_path_buf())
    };

    let entries = match std::fs::read_dir(&icons_dir) {
        Ok(entries) => entries,
        // 图标目录尚未建立时不可能有孤儿。
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(AppError::new(
                "icon_dir_unreadable",
                format!("无法读取图标目录 {}: {error}", icons_dir.display()),
            ))
        }
    };

    let mut removed = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();

        if path.extension().and_then(|extension| extension.to_str()) != Some("png") {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };

        if !is_uuid(stem) || referenced.contains(stem) {
            continue;
        }

        match std::fs::remove_file(&path) {
            Ok(()) => removed.push(file_name.to_owned()),
            Err(error) => log::warn!("删除孤儿图标失败 {}: {error}", path.display()),
        }
    }

    removed.sort();
    Ok(removed)
}

// 收集数据树中所有应用的 ID，作为图标缓存的引用集合。
//
// 顶层应用也要算进去：漏了它们，孤儿对账会把这些图标当成没人引用的垃圾删掉。
fn collect_app_ids(data: &AppData) -> HashSet<String> {
    fn walk(items: &[Item], ids: &mut HashSet<String>) {
        for item in items {
            match item {
                Item::App(app) => {
                    ids.insert(app.id.clone());
                }
                Item::Group(group) => walk(&group.children, ids),
            }
        }
    }

    let mut ids: HashSet<String> = data.apps.iter().map(|app| app.id.clone()).collect();
    for category in &data.categories {
        walk(&category.groups, &mut ids);
    }
    ids
}

// 判断文件名主干是否是标准连字符形式的 UUID（8-4-4-4-12，共 36 字符）。
//
// `Uuid::parse_str` 还接受 32 位简单形式和带花括号形式，所以额外卡住长度，
// 确保只命中后端自己生成的那种命名。
fn is_uuid(stem: &str) -> bool {
    stem.len() == 36 && Uuid::parse_str(stem).is_ok()
}

fn not_found(code: &'static str, entity_name: &str, id: &str) -> AppError {
    AppError::new(code, format!("找不到{entity_name}: {id}"))
}
