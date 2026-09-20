use crate::{
    error::AppResult,
    models::{
        AddAppRequest, AddCategoryRequest, AddGroupRequest, AppData, AppEXE, Category,
        DeleteRequest, Group, RenameRequest,
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

// 添加应用或快捷方式；图标和快捷方式字段由服务层自动解析。
//
// `parent_id` 传 `null` 表示直接放到顶层，不进任何分类。
#[tauri::command]
pub fn add_app(
    parent_id: Option<String>,
    source_path: String,
    name: Option<String>,
    state: tauri::State<'_, AppState>,
) -> AppResult<AppEXE> {
    services::catalog::add_app(
        state.inner(),
        AddAppRequest {
            parent_id,
            source_path,
            name,
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
