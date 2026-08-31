use std::path::{Path, PathBuf};

use crate::{
    error::AppResult,
    models::{AppData, AppEXE, AppError, IconType, Item},
    state::AppState,
    storage::DataRepository,
    system::{executor, shortcut},
};

// 系统操作业务服务。
//
// service 层负责读取应用数据、查找应用节点、整理可选字段，
// 但不直接调用 Windows API，也不构造 `Command`。
pub fn launch_app(state: &AppState, app_id: &str) -> AppResult<()> {
    let app = {
        let store = state.get_lock()?;
        let data = store.load_data()?;
        find_app(&data, app_id)
            .cloned()
            .ok_or_else(|| AppError::new("app_not_found", "找不到要启动的应用"))?
    };

    // `app` 已经从数据树中克隆出来，离开上面的代码块后 MutexGuard
    // 已释放，系统调用不会占用数据存储锁。
    launch_app_record(&app)
}

// 启动一个已经从数据仓储中取出的应用记录。
fn launch_app_record(app: &AppEXE) -> AppResult<()> {
    let launch_spec = build_launch_spec(app)?;

    if is_script_path(Path::new(&app.source_path)) || is_script_path(&launch_spec.target_path) {
        return Err(AppError::new(
            "script_execution_not_allowed",
            "脚本启动需要经过白名单校验，当前版本暂不允许直接运行",
        ));
    }

    executor::launch(
        &launch_spec.target_path,
        launch_spec.working_dir.as_deref(),
        launch_spec.arguments.as_deref(),
    )
}

// service 层把持久化的应用记录解包成 executor 所需的最小启动参数。
//
// 普通文件、文件夹、可执行文件和 URL 的 `source_path` 就是启动目标。
// `.lnk` 必须在这里重新解析，使用解析后的目标路径；否则把 `.lnk` 和
// 已经解析过的参数一起交给 Shell 可能导致参数重复。解析结果只存在于
// 本次调用，不写回 `AppEXE`，因此 `source_path` 始终保留原始来源。
struct LaunchSpec {
    target_path: PathBuf,
    working_dir: Option<PathBuf>,
    arguments: Option<String>,
}

fn build_launch_spec(app: &AppEXE) -> AppResult<LaunchSpec> {
    let source_path = Path::new(&app.source_path);
    let is_shortcut =
        matches!(&app.icon_type, &IconType::Shortcut) || is_shortcut_path(source_path);

    if is_shortcut {
        let shortcut = shortcut::parse_shortcut(source_path)?;
        return Ok(LaunchSpec {
            target_path: PathBuf::from(shortcut.target_path),
            working_dir: shortcut
                .working_dir
                .filter(|value| !value.trim().is_empty())
                .map(PathBuf::from),
            arguments: shortcut.arguments.filter(|value| !value.trim().is_empty()),
        });
    }

    Ok(LaunchSpec {
        target_path: source_path.to_path_buf(),
        working_dir: app
            .working_dir
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from),
        arguments: app
            .arguments
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned),
    })
}

fn is_shortcut_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"))
}

// 当前版本的安全默认策略：脚本可以作为 `File` 保存，但不能直接启动。
// 后续增加脚本白名单后，应在这里按白名单结果放行，而不是让底层 Shell
// 根据文件关联自动决定是否执行。
fn is_script_path(path: &Path) -> bool {
    // URL 的最后一段可能恰好叫 `file.bat`，但它仍然应该交给浏览器；
    // 只有本地文件路径才参与脚本扩展名判断。
    if is_url_target(path) {
        return false;
    }

    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref(),
        Some("bat") | Some("cmd") | Some("ps1") | Some("vbs") | Some("js")
    )
}

fn is_url_target(path: &Path) -> bool {
    let value = path.to_string_lossy();
    value.contains("://") || value.to_ascii_lowercase().starts_with("mailto:")
}

// 在分类树中递归查找应用节点。
fn find_app<'a>(data: &'a AppData, app_id: &str) -> Option<&'a AppEXE> {
    if app_id.trim().is_empty() {
        return None;
    }

    data.categories
        .iter()
        .flat_map(|category| category.groups.iter())
        .find_map(|item| find_app_in_item(item, app_id))
}

fn find_app_in_item<'a>(item: &'a Item, app_id: &str) -> Option<&'a AppEXE> {
    match item {
        Item::App(app) if app.id == app_id => Some(app),
        Item::App(_) => None,
        Item::Group(group) => group
            .children
            .iter()
            .find_map(|child| find_app_in_item(child, app_id)),
    }
}
