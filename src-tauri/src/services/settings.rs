use crate::{
    error::AppResult,
    models::{
        AppError, Settings, UpdateAutoCollapseRequest, UpdateAutoStartRequest, UpdateThemeRequest,
    },
    state::AppState,
    storage::SettingsRepository,
    system,
};

// 读取 `config.json` 中的全部应用配置。
pub fn get_settings(state: &AppState) -> AppResult<Settings> {
    let store = state.get_lock()?;
    store.load_settings()
}

// 按字段更新自动收起配置，避免提交整个 Settings 快照。
pub fn update_auto_collapse(
    state: &AppState,
    request: UpdateAutoCollapseRequest,
) -> AppResult<Settings> {
    mutate_settings(state, |settings| {
        settings.auto_collapse = request.enabled;
        Ok(())
    })
}

// 按字段更新主题配置，避免提交整个 Settings 快照。
pub fn update_theme(state: &AppState, request: UpdateThemeRequest) -> AppResult<Settings> {
    mutate_settings(state, |settings| {
        settings.theme = request.theme;
        Ok(())
    })
}

// 按字段更新开机自启动配置，并同步 Windows Run 注册表。
pub fn update_auto_start(state: &AppState, request: UpdateAutoStartRequest) -> AppResult<Settings> {
    let store = state.get_lock()?;
    store.mutate_settings(|settings| {
        if settings.auto_start != request.enabled {
            system::registory::set_auto_start_enabled(request.enabled)?;
            settings.auto_start = request.enabled;
        }
        validate_settings(settings)?;
        Ok(())
    })?;
    store.load_settings()
}

// 清除展开态窗口尺寸覆盖，只修改对应字段。
pub fn clear_window_size_override(state: &AppState) -> AppResult<Settings> {
    mutate_settings(state, |settings| {
        settings.window.size_override = None;
        Ok(())
    })
}

// 校验并持久化应用配置。
//
// 当前配置主要由窗口层使用，因此先保证保存的窗口宽高是有效的；
// 位置允许为负数，以兼容多显示器和主显示器左侧的屏幕。
pub fn update_settings(state: &AppState, settings: Settings) -> AppResult<Settings> {
    validate_settings(&settings)?;

    // 兼容接口必须携带当前修订号。否则旧快照无法区分“用户确实想把字段
    // 改回旧值”和“并发更新后仍在提交旧数据”，后者会静默覆盖新配置。
    mutate_settings(state, |current| {
        if current.revision != settings.revision {
            return Err(AppError::new(
                "settings_revision_conflict",
                format!(
                    "配置已被更新，请刷新后重试（当前修订号: {}，请求修订号: {}）",
                    current.revision, settings.revision
                ),
            ));
        }

        if current.auto_start != settings.auto_start {
            system::registory::set_auto_start_enabled(settings.auto_start)?;
        }
        *current = settings.clone();
        Ok(())
    })?;
    // `mutate_settings` 返回的是原子事务提交后的快照，包含递增后的 revision。
    let store = state.get_lock()?;
    store.load_settings()
}

// 对字段级设置操作统一执行校验和原子读改写。
fn mutate_settings<F>(state: &AppState, operation: F) -> AppResult<Settings>
where
    F: FnOnce(&mut Settings) -> AppResult<()>,
{
    let store = state.get_lock()?;
    store.mutate_settings(|settings| {
        operation(settings)?;
        validate_settings(settings)?;
        Ok(())
    })?;
    store.load_settings()
}

// 启动时把开机自启动对一次账。
//
// 配置是应用状态、注册表是系统状态，两者会各说各话：用户手动清掉启动项、
// 或者换了安装目录之后旧路径还挂在 Run 键里。这里以配置为准补一次，
// 保证「设置里开着」和「系统会启动我们」不会长期不一致。
//
// 失败只向上返回错误，由调用方决定要不要惊动用户 —— 自启动是附加功能，
// 不该拦住启动。
pub fn reconcile_auto_start(state: &AppState) -> AppResult<()> {
    let store = state.get_lock()?;
    let configured = store.load_settings()?.auto_start;
    system::registory::reconcile_auto_start(configured);
    Ok(())
}

// 后台清理失效的启动项。
//
// 与上面的对账分开：对账要**立刻**完成（写/删 `DeskCrate` 那一条值），而清理是
// 纯粹的扫尾工作 —— 要枚举整个 Run 键、读「启动」文件夹、解析 `.lnk`（COM），
// 放到后台线程做，既不拖住窗口创建，也不在主线程上初始化 COM。
//
// 顺序上必须在对账**之后**：两者都要动 Run 键，串行执行才不会有竞争。
pub fn sweep_stale_auto_start_entries() {
    std::thread::spawn(system::registory::sweep_stale_auto_start_entries);
}

fn validate_settings(settings: &Settings) -> AppResult<()> {
    // 缓存尺寸和用户尺寸覆盖都会在下次启动时被直接应用到窗口上，
    // 0 宽或 0 高的窗口既看不见也点不到，两者用同一条规则校验。
    //
    // 两个值单位不同（物理像素 / 设计稿 CSS 像素），类型上刻意不能互换，
    // 因此分别取字段再交给同一个校验函数，而不是塞进同一个数组。
    if let Some(size) = settings.window.size {
        validate_size("窗口大小", size.width, size.height)?;
    }
    if let Some(size) = settings.window.size_override {
        validate_size("用户尺寸覆盖", size.width, size.height)?;
    }

    // `is_initialized` 与几何信息必须成套存在。否则下次启动会进入
    // “恢复窗口”分支，却没有可恢复的位置或大小。
    if settings.window.is_initialized {
        if settings.window.position.is_none() {
            return Err(AppError::new(
                "invalid_window_state",
                "已初始化配置必须包含窗口位置",
            ));
        }

        if settings.window.size.is_none() {
            return Err(AppError::new(
                "invalid_window_state",
                "已初始化配置必须包含窗口大小",
            ));
        }
    }

    Ok(())
}

// 尺寸字段的形状校验：宽高都必须大于 0。
//
// 两个尺寸字段的单位不同（物理像素 / 设计稿 CSS 像素），但「不能是 0」这条
// 规则与单位无关，因此只接收分量、不接收具体类型。
fn validate_size(label: &str, width: u32, height: u32) -> AppResult<()> {
    if width == 0 || height == 0 {
        return Err(AppError::new(
            "invalid_window_size",
            format!("{label}的宽度和高度必须大于 0"),
        ));
    }

    Ok(())
}
