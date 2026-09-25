use crate::{
    error::AppResult,
    models::{
        AddAppRequest, AddAppsRequest, AddAppsResponse, AddCategoryRequest, AddGroupRequest,
        AppData, AppEXE, Category, DeleteRequest, Group, MoveAppRequest, MoveGroupRequest,
        RenameRequest, SetNodeCollapsedRequest,
    },
    services,
    state::AppState,
};

// 获取所有分类、分组和应用数据。
// 前端启动后可以先调用这个命令，从返回的数据中读取后端生成的UUID；之后重命名、删除和启动操作都使用对应节点的 id。
#[tauri::command]
pub fn get_all_data(state: tauri::State<'_, AppState>) -> AppResult<AppData> {
    services::catalog::get_all_data(state.inner())
}

// 添加一个顶层分类，并返回后端创建的完整分类对象。
#[tauri::command]
pub fn add_category(name: String, state: tauri::State<'_, AppState>) -> AppResult<Category> {
    services::catalog::add_category(state.inner(), AddCategoryRequest { name })
}

// 按ID重命名顶层分类，并返回更新后的分类对象。
#[tauri::command]
pub fn rename_category(
    id: String,
    name: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<Category> {
    services::catalog::rename_category(state.inner(), RenameRequest { id, name })
}

// 按ID删除顶层分类及其全部内容。
#[tauri::command]
pub fn delete_category(id: String, state: tauri::State<'_, AppState>) -> AppResult<()> {
    services::catalog::delete_category(state.inner(), DeleteRequest { id })
}

// 在分类或分组下创建子分组。
#[tauri::command]
pub fn add_group(
    parent_id: String,
    name: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<Group> {
    services::catalog::add_group(state.inner(), AddGroupRequest { parent_id, name })
}

// 按ID重命名子分组，并返回更新后的分组对象。
#[tauri::command]
pub fn rename_group(
    id: String,
    name: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<Group> {
    services::catalog::rename_group(state.inner(), RenameRequest { id, name })
}

// 按ID删除子分组及其全部后代。
#[tauri::command]
pub fn delete_group(id: String, state: tauri::State<'_, AppState>) -> AppResult<()> {
    services::catalog::delete_group(state.inner(), DeleteRequest { id })
}

// 持久化分类或分组的折叠状态。
#[tauri::command]
pub fn set_node_collapsed(
    id: String,
    is_collapsed: bool,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    services::catalog::set_node_collapsed(
        state.inner(),
        SetNodeCollapsedRequest { id, is_collapsed },
    )
}

// 后台抓取 worker 发出的事件名。前端监听它，把占位图标换成真实图标。
pub const ICON_READY_EVENT: &str = "dc://icon-ready";

// 添加应用或快捷方式；图标和快捷方式字段由服务层自动解析。
//
// `parent_id` 传 `null` 表示直接放到顶层（此时 `rootIndex` 指定插到第几个
// 根节点之前，省略则追加到最下方）；放进分类或分组时忽略 `rootIndex`。

#[tauri::command]
pub fn add_app(
    parent_id: Option<String>,
    source_path: String,
    name: Option<String>,
    root_index: Option<usize>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> AppResult<AppEXE> {
    let created = services::catalog::add_app(
        state.inner(),
        AddAppRequest {
            parent_id,
            source_path,
            name,
            root_index,
        },
    )?;

    // 网址应用先带着图标返回，缺图标的话在后台补。
    //
    // 抓图标是几秒量级的网络操作，实测最慢 18 秒；如果等它完成再返回，「添加一个
    // 网址」就会卡住那么久。任务进入固定 4 个 worker 的有界队列；网络请求不占目录锁。
    //
    // 同源复用命中的应用返回时就已经带着真实图标了，`needs_url_icon` 会返回
    // `false`，连队列任务都不创建。
    if services::catalog::needs_url_icon(&created) {
        let app_id = created.id.clone();
        services::url_icon_queue::enqueue(state.inner(), &app, app_id);
    }

    Ok(created)
}

// 批量添加应用或快捷方式；成功项一次性写入，失败项返回明细。
#[tauri::command]
pub fn add_apps(
    parent_id: Option<String>,
    source_paths: Vec<String>,
    root_index: Option<usize>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> AppResult<AddAppsResponse> {
    let result = services::catalog::add_apps(
        state.inner(),
        AddAppsRequest {
            parent_id,
            source_paths,
            root_index,
        },
    )?;

    for created in &result.added {
        if services::catalog::needs_url_icon(created) {
            services::url_icon_queue::enqueue(state.inner(), &app, created.id.clone());
        }
    }

    Ok(result)
}

// 把一个应用移动到新位置（拖拽换位）。
//
// `parentId` 传 `null` 表示移到顶层；`beforeId` 是参照节点（插到它之前），
// 传 `null` 表示插到目标容器的末尾。
#[tauri::command]
pub fn move_app(
    id: String,
    parent_id: Option<String>,
    before_id: Option<String>,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    services::catalog::move_app(
        state.inner(),
        MoveAppRequest {
            id,
            parent_id,
            before_id,
        },
    )
}

#[tauri::command]
pub fn move_group(
    id: String,
    parent_id: String,
    before_id: Option<String>,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    services::catalog::move_group(
        state.inner(),
        MoveGroupRequest {
            id,
            parent_id,
            before_id,
        },
    )
}

// 按ID重命名应用，并返回更新后的应用对象。
#[tauri::command]
pub fn rename_app(
    id: String,
    name: String,
    state: tauri::State<'_, AppState>,
) -> AppResult<AppEXE> {
    services::catalog::rename_app(state.inner(), RenameRequest { id, name })
}

// 按ID删除应用。
#[tauri::command]
pub fn delete_app(id: String, state: tauri::State<'_, AppState>) -> AppResult<()> {
    services::catalog::delete_app(state.inner(), DeleteRequest { id })
}
