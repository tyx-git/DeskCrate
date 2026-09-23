use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::Utc;
use uuid::Uuid;

use crate::{
    error::AppResult,
    models::{
        AddAppRequest, AddAppsRequest, AddAppsResponse, AddCategoryRequest, AddGroupRequest,
        AppData, AppEXE, AppError, BatchAddFailure, Category, DeleteRequest, Group, IconType, Item,
        MoveAppRequest, RenameRequest, RootItem, SetNodeCollapsedRequest,
    },
    path,
    state::AppState,
    storage::DataRepository,
};

use super::{favicon, icon::IconService, tree};

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
        data.items.push(RootItem::Category(category.clone()));
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
    cleanup_icons(&removed.groups, &icons_dir);
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

// 原子更新分类或分组的折叠状态，并持久化到 data.json。
//
// 应用节点没有折叠状态，因此只允许分类和分组 ID；读取、修改和写回由
// DataRepository 的同一事务完成，多个窗口或进程同时更新时不会丢失。
pub fn set_node_collapsed(state: &AppState, request: SetNodeCollapsedRequest) -> AppResult<()> {
    validate_id(&request.id)?;

    let store = state.get_lock()?;
    store.mutate_data(|data| {
        if let Some(category) = tree::find_category_mut(data, &request.id) {
            category.is_collapsed = request.is_collapsed;
            return Ok(());
        }

        if let Some(group) = tree::find_group_mut(data, &request.id) {
            group.is_collapsed = request.is_collapsed;
            return Ok(());
        }

        Err(not_found(
            "collapsible_node_not_found",
            "可折叠节点",
            &request.id,
        ))
    })
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

    // 只在这里持锁读取父节点、找可复用的同源图标和复制图标目录；后续系统操作
    // 不持有锁。
    let (icon_service, reusable) = {
        let store = state.get_lock()?;
        let data = store.load_data()?;
        if let Some(parent_id) = &request.parent_id {
            if !tree::parent_exists(&data, parent_id) {
                return Err(not_found("parent_not_found", "应用的父节点", parent_id));
            }
        }

        // 同源复用：这个站点已经抓过图标的话，`prepare_app` 会直接复制一份，
        // 新应用一落库就带着真实图标，连后台线程都不用起。
        let reusable = match &source {
            Source::Url(url) => reusable_url_icon(&data, url),
            Source::LocalPath(_) => None,
        };

        (IconService::from_store(&store), reusable)
    };

    let prepared = prepare_app(
        &source,
        request.name.as_deref(),
        &app_id,
        &icon_service,
        reusable.as_deref(),
    )?;
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
        // 顶层应用按落点插进根节点的有序列表；放进分类/分组时沿用既有的
        // 「按 parent_id 找到容器再追加」路径。
        match &request.parent_id {
            Some(parent_id) => append_item(data, parent_id, Item::App(app.clone()), "应用的父节点"),
            None => {
                // 没给下标就追加到最末尾（旧行为）。
                tree::insert_root_app(data, app.clone(), request.root_index.unwrap_or(usize::MAX));
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

// 批量新增应用：耗时准备在锁外完成，成功结果一次性提交到 data.json。
//
// 单项来源、快捷方式或图标失败只进入 `failed`，不会阻塞其他项；网址仍使用
// 内置占位图，后续由既有后台补图标流程处理。这样 N 个来源只触发一次 JSON
// 读改写和原子替换，同时不会把图标提取过程放进存储锁。
pub fn add_apps(state: &AppState, request: AddAppsRequest) -> AppResult<AddAppsResponse> {
    if let Some(parent_id) = &request.parent_id {
        validate_id(parent_id)?;
    }
    if request.source_paths.is_empty() {
        return Err(AppError::new("empty_batch", "批量导入来源不能为空"));
    }

    let (icon_service, data_snapshot) = {
        let store = state.get_lock()?;
        (IconService::from_store(&store), store.load_data()?)
    };

    if let Some(parent_id) = &request.parent_id {
        if !tree::parent_exists(&data_snapshot, parent_id) {
            return Err(not_found("parent_not_found", "应用的父节点", parent_id));
        }
    }

    let mut added = Vec::new();
    let mut failed = Vec::new();
    let mut cache_paths = Vec::new();

    for source_path in request.source_paths {
        let original = source_path.clone();
        let result = prepare_batch_app(source_path, &icon_service);
        match result {
            Ok((app, cache_path)) => {
                cache_paths.extend(cache_path);
                added.push(app);
            }
            Err(error) => failed.push(BatchAddFailure {
                source_path: original,
                code: error.code,
                message: error.message,
            }),
        }
    }

    if added.is_empty() {
        for path in cache_paths {
            remove_cached_icon(Some(&path));
        }
        return Ok(AddAppsResponse { added, failed });
    }

    let store = match state.get_lock() {
        Ok(store) => store,
        Err(error) => {
            for path in cache_paths {
                remove_cached_icon(Some(&path));
            }
            return Err(error);
        }
    };
    if let Err(error) = store.mutate_data(|data| {
        for app in &added {
            match &request.parent_id {
                Some(parent_id) => {
                    append_item(data, parent_id, Item::App(app.clone()), "应用的父节点")?;
                }
                None => tree::insert_root_app(
                    data,
                    app.clone(),
                    request.root_index.unwrap_or(usize::MAX),
                ),
            }
        }
        Ok(())
    }) {
        for path in cache_paths {
            remove_cached_icon(Some(&path));
        }
        return Err(error);
    }

    Ok(AddAppsResponse { added, failed })
}

fn prepare_batch_app(
    source_path: String,
    icon_service: &IconService,
) -> AppResult<(AppEXE, Option<PathBuf>)> {
    let request = AddAppRequest {
        parent_id: None,
        source_path,
        name: None,
        root_index: None,
    };
    let source = normalize_source(request.source_path)?;
    if let Source::LocalPath(path) = &source {
        validate_local_source(path)?;
    }

    let app_id = Uuid::new_v4().to_string();
    let prepared = prepare_app(&source, None, &app_id, icon_service, None)?;
    let cache_path = prepared.cache_path.clone();
    let app = AppEXE {
        id: app_id,
        name: prepared.name,
        source_path: source.into_string(),
        arguments: prepared.arguments,
        working_dir: prepared.working_dir,
        icon_url: prepared.icon_url,
        icon_type: prepared.icon_type,
        created_time: Utc::now().to_rfc3339(),
    };
    Ok((app, cache_path))
}

// 补抓一个网址应用的图标，拿到就回填进 `data.json`。
//
// 这是「添加网址」那条路径的后半段：应用已经先用内置图标落库了，这里在后台把
// 真实图标补上并写回去。返回回填后的图标 URL，调用方据此通知前端刷新。
//
// 拿图标有两条路，**同源复用优先于联网**：这个站点之前抓过就直接复制一份缓存，
// 抓不到才发请求。失败是常态（站点慢、不可达、没图标），调用方不该把它当错误
// 处理 —— 只是这个应用继续用内置图标而已。
pub fn refresh_url_icon(state: &AppState, app_id: &str) -> AppResult<Option<String>> {
    // 只在持锁期间读一次网址、图标目录和可复用的同源图标；抓取本身不碰锁。
    let (url, icon_service, reusable) = {
        let store = state.get_lock()?;
        let data = store.load_data()?;
        let app = tree::find_app(&data, app_id)
            .ok_or_else(|| not_found("app_not_found", "应用", app_id))?;

        // 不是网址应用，或者已经有真实图标了（`prepare_app` 复用来的、或补抓过了）。
        if !needs_url_icon(app) {
            return Ok(None);
        }

        (
            app.source_path.clone(),
            IconService::from_store(&store),
            reusable_url_icon(&data, &app.source_path),
        )
    };

    // 同源复用优先于联网。
    //
    // 这条路覆盖「连着加两个同一站点的网址」：第一个还在后台抓，第二个进来时
    // `prepare_app` 还看不到图标，但等到这里通常已经抓完了。慢站因此只会被抓一次。
    let result = reusable
        .and_then(|source_id| icon_service.copy_icon(&source_id, app_id))
        .or_else(|| icon_service.fetch_url_icon(&url, app_id));

    let Some(result) = result else {
        return Ok(None);
    };

    let icon_url = IconService::asset_url(&result.cache_path);

    // 回填要用一次原子事务：读取、改图标、落盘必须在同一把锁里，否则两个后台
    // 任务同时完成时，后写入的会覆盖前一个。
    let store = state.get_lock()?;
    store.mutate_data(|data| {
        let app = tree::find_app_mut(data, app_id)
            .ok_or_else(|| not_found("app_not_found", "应用", app_id))?;
        app.icon_url = icon_url.clone();
        Ok(())
    })?;

    Ok(Some(icon_url))
}

// 网址应用是否还在等后台补图标。
//
// `add_app` 用它决定要不要起后台线程，`refresh_url_icon` 用它判断是否还有活干。
// 两种情况都返回 `false`：不是网址应用；是网址应用但已经有真实图标了
// （同源复用拿到的，或者后台已经补过）。
pub fn needs_url_icon(app: &AppEXE) -> bool {
    app.icon_type == IconType::Url && !has_real_icon(app)
}

// 这个应用是不是已经有一张真实图标，而不是内置的占位图。
//
// 占位图对本地应用不适用（它们的图标一定是从系统里提出来写进缓存的），所以这个
// 判断只对网址应用有意义。
fn has_real_icon(app: &AppEXE) -> bool {
    app.icon_url != DEFAULT_URL_ICON_URL
}

// 找一个**同源**且已经抓到过图标的网址应用，把它当作复用的来源。
//
// 两个收益：连着加 5 个 GitHub 链接只会抓一次；同一站点在侧栏里长得必然一样
// （各抓各的会因为重试时机不同而拿到不同尺寸的图标）。
//
// 只按「源」比对，不按 host 比对：`github.com` 和 `gist.github.com` 的图标本来
// 就不一样，混用会让用户看到错的图。
fn reusable_url_icon(data: &AppData, url: &str) -> Option<String> {
    let origin = url_origin(url)?;
    let mut source_id = None;

    tree::for_each_app(data, &mut |app| {
        // 已经找到就不用再往下走了。
        if source_id.is_some() {
            return;
        }
        // 还挂着内置图标的没有可复用的东西 —— 那说明它自己也没抓到。
        if app.icon_type != IconType::Url || !has_real_icon(app) {
            return;
        }
        if url_origin(&app.source_path).as_deref() == Some(origin.as_str()) {
            source_id = Some(app.id.clone());
        }
    });

    source_id
}

// 网址的「源」：scheme + host + port。拆不出来就返回 `None`（那就没法比对）。
//
// 端口要带上：`localhost:3000` 和 `localhost:5173` 是两个站点。host 统一小写，
// 免得 `GitHub.com` 和 `github.com` 被当成两个源。
fn url_origin(url: &str) -> Option<String> {
    let parts = favicon::split_url(url)?;
    let scheme = if parts.is_https { "https" } else { "http" };

    Some(format!(
        "{scheme}://{}:{}",
        parts.host.to_ascii_lowercase(),
        parts.port
    ))
}

// 把一个应用移动到新位置（拖拽换位）。
//
// 分两步：先从原位置摘下来，再按目标插回去。顺序不能反 —— 先插再摘会让应用在
// 两个地方同时存在，而 `remove_node` 只删它找到的第一个。
//
// 目标用「参照节点 + 插到它之前」表达，因此**先摘后插不会错位**：参照节点在被
// 摘掉自己之后仍然指向同一个元素。
pub fn move_app(state: &AppState, request: MoveAppRequest) -> AppResult<()> {
    let store = state.get_lock()?;
    store.mutate_data(|data| {
        let Some(Item::App(app)) = tree::remove_node(data, &request.id, tree::NodeKind::App) else {
            return Err(not_found("app_not_found", "应用", &request.id));
        };

        let before_id = request.before_id.as_deref();

        match &request.parent_id {
            None => match before_id {
                // 参照的是某个顶层应用 → 插进它所在的那一行的对应位置。
                Some(reference) => {
                    if tree::insert_into_root_row(data, reference, app) {
                        return Ok(());
                    }
                    Err(AppError::new(
                        "reference_not_found",
                        format!("找不到参照节点: {reference}"),
                    ))
                }
                None => {
                    tree::append_to_root(data, app);
                    Ok(())
                }
            },

            Some(parent_id) => match before_id {
                Some(reference) => {
                    if tree::insert_before_in_container(data, reference, app) {
                        Ok(())
                    } else {
                        Err(AppError::new(
                            "reference_not_found",
                            format!("找不到参照节点: {reference}"),
                        ))
                    }
                }
                None => {
                    if tree::append_child(data, parent_id, Item::App(app)) {
                        Ok(())
                    } else {
                        Err(not_found("parent_not_found", "目标容器", parent_id))
                    }
                }
            },
        }
    })
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

    // 被摘掉的节点可能是一个分组（里面还挂着后代应用），所以要递归清理。
    cleanup_icons(std::slice::from_ref(&removed), &icons_dir);
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
    reusable_icon: Option<&str>,
) -> AppResult<PreparedApp> {
    match source {
        // 网址的图标**不在这里联网抓**。
        //
        // 实测平均 6.3 秒、最慢 18.3 秒（见 `dosc/图标抓取/probe-result.md`）：站点
        // 慢或不可达时，每个候选都要耗满一次超时。同步抓会把「添加一个网址」卡成
        // 十几秒，比少一张图标严重得多。
        //
        // 所以先用内置图标把应用落库，抓取放到后台线程，抓到了再回填（见
        // `refresh_url_icon`）。用户看到的是「立刻多了一个应用，图标随后补上」。
        //
        // 唯一的例外是**同源复用**：`reusable_icon` 给了来源应用 ID 时，直接复制
        // 一份本地缓存文件 —— 不联网，只是几 KB 的磁盘拷贝，不用挪到后台。
        Source::Url(url) => {
            let fallback = default_url_name(url);
            let name = choose_app_name(requested_name, &fallback)?;

            if let Some(source_app_id) = reusable_icon {
                if let Some(result) = icon_service.copy_icon(source_app_id, app_id) {
                    let icon_url = IconService::asset_url(&result.cache_path);
                    return Ok(PreparedApp {
                        name,
                        working_dir: None,
                        arguments: None,
                        icon_type: IconType::Url,
                        icon_url,
                        cache_path: Some(result.cache_path),
                    });
                }
                // 复制失败不致命：退回内置图标，后台补抓会兜住。
            }

            Ok(PreparedApp {
                name,
                working_dir: None,
                arguments: None,
                icon_type: IconType::Url,
                icon_url: DEFAULT_URL_ICON_URL.to_owned(),
                cache_path: None,
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
    normalize_name(
        requested_name.unwrap_or(fallback),
        "invalid_app_name",
        "应用",
    )
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

// 关闭应用并清理其图标缓存。
//
// 网址图标现在可能是 png 或 svg，`remove_app_icon` 会把两种扩展名都清掉。
//
// 图标文件名由后端生成，形状固定为 `{app_id}.{png|svg}`，因此只清理这个约定
// 下的文件，不直接信任 JSON 中可能被修改的任意路径。
//
// 两个扩展名都要清：本地应用的图标是位图（`.png`），网址应用解不出位图时会把
// SVG 原文按 `.svg` 落盘。只删一个会留下另一种孤儿。
fn cleanup_icons(items: &[Item], icons_dir: &Path) {
    // 复用树模块那份遍历，嵌套分组交给它去钻。
    tree::for_each_in_items(items, &mut |item| remove_app_icon(item, icons_dir));
}

// 应用可能占用的图标缓存路径。png 和 svg 是互斥的（同一时刻只有一个），
// 但删的时候两个都试一遍最省事 —— 少一次判断，也多一层保险。
fn icon_cache_paths(icons_dir: &Path, app_id: &str) -> [PathBuf; 2] {
    [
        icons_dir.join(format!("{app_id}.png")),
        icons_dir.join(format!("{app_id}.svg")),
    ]
}

fn remove_app_icon(item: &Item, icons_dir: &Path) {
    let Item::App(app) = item else {
        return;
    };

    // `icon_url` 是给 WebView 使用的 URL，可能已经把磁盘路径 percent-encode，
    // 不能把它当作本地 Path 解析。缓存文件名由后端按 app.id 生成，因此这里
    // 直接使用受信任的 ID。
    for path in icon_cache_paths(icons_dir, &app.id) {
        // 删除失败不能静默吞掉：Windows 上文件被其他进程占用时删除会失败，留下的
        // 孤儿只能靠 `sweep_orphan_icons` 兜底，这里至少留下线索。`NotFound` 属于
        // 正常情况（URL 应用本来就没有图标文件），不算失败。
        if let Err(error) = std::fs::remove_file(&path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!("清理图标缓存失败 {}: {error}", path.display());
            }
        }
    }
}

// 启动时对账图标缓存，删除不再被任何应用引用的 `{UUID}.png` / `{UUID}.svg`。
//
// 正常路径下删除应用/分组/分类都会同步清掉图标，所以这个对账只兜异常情况：
// 进程在「写完文件」和「提交 data.json」之间崩溃、或删除时文件被占用而失败。
// 这类孤儿没有任何界面能反映出来，不对账就会一直堆积。
//
// 只处理严格符合 `{UUID}.png` / `{UUID}.svg` 的文件 —— 图标文件名由后端按
// app.id 生成，不符合这个形状的文件不是本程序写的，一律不动，避免误删用户
// 自己的东西。
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

        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase);
        if !matches!(extension.as_deref(), Some("png") | Some("svg")) {
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
    let mut ids = HashSet::new();
    tree::for_each_app(data, &mut |app| {
        ids.insert(app.id.clone());
    });
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
