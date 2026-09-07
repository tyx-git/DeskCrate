use std::path::{Path, PathBuf};

use crate::{
    error::AppResult,
    models::{AppEXE, AppError, IconType},
    path,
    state::AppState,
    storage::DataRepository,
    system::{executor, shortcut},
};

use super::tree;

// 系统操作业务服务。
//
// service 层负责读取应用数据、查找应用节点、整理可选字段，
// 但不直接调用 Windows API，也不构造 `Command`。
pub fn launch_app(state: &AppState, app_id: &str) -> AppResult<()> {
    let app = {
        let store = state.get_lock()?;
        let data = store.load_data()?;
        tree::find_app(&data, app_id)
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

    if path::is_script(Path::new(&app.source_path)) || path::is_script(&launch_spec.target_path) {
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
        matches!(&app.icon_type, &IconType::Shortcut) || path::is_shortcut(source_path);

    if is_shortcut {
        let shortcut = shortcut::parse_shortcut(source_path)?;
        return Ok(LaunchSpec {
            target_path: PathBuf::from(shortcut.target_path),
            working_dir: optional_path(shortcut.working_dir.as_deref()),
            arguments: non_empty_text(shortcut.arguments.as_deref()),
        });
    }

    Ok(LaunchSpec {
        target_path: source_path.to_path_buf(),
        working_dir: optional_path(app.working_dir.as_deref()),
        arguments: non_empty_text(app.arguments.as_deref()),
    })
}

fn optional_path(value: Option<&str>) -> Option<PathBuf> {
    non_empty_text(value).map(PathBuf::from)
}

fn non_empty_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}
