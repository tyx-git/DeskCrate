use std::time::{Duration, Instant};

use crate::{
    error::AppResult,
    models::{
        settings::css, CssSize, ResizeWindowRequest, Settings, ViewMode, WindowPosition, WindowSize,
    },
    state::{AppState, OverrideChange},
    storage::SettingsRepository,
};

// 主窗口在 `tauri.conf.json` 里的标签。事件循环和本模块都要用它定位窗口，
// 散落的字面量一旦写错只会表现为「监听器静默失效」，因此集中在这里。
pub const MAIN_WINDOW_LABEL: &str = "main";

// 用户尺寸覆盖的落盘间隔。
//
// 拖拽边框时 Windows 会连续派发 `WM_SIZE`，一次拖拽可能产生上百次
// `Resized`。每次落盘都要走一遍加锁、原子替换和 `fsync`，节流是必须的；
// 被节流掉的那一次不会丢，它留在内存里等下一个确定安静的时刻补写
// （见 `flush_user_resize`）。
const OVERRIDE_WRITE_INTERVAL: Duration = Duration::from_millis(300);

// 主窗口初始化时返回给前端的状态。
//
// `is_initialized` 表示本次调用完成后窗口已经具有可持久化的状态；
// 位置和大小是本次实际应用到窗口的结果。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInitResponse {
    pub is_initialized: bool, // 本次调用完成后是否已经初始化并持久化窗口状态。
    pub position: WindowPosition, // 本次启动采用的窗口左上角位置。
    pub size: WindowSize,     // 本次启动采用的窗口客户区大小。
}

// 首次启动的窗口默认大小。
//
// Tauri 配置中的窗口宽高只是创建窗口时的兜底值；真正的首次尺寸由
// 当前显示器工作区计算。这里保留最小值，避免极小工作区出现零尺寸。
const MIN_WINDOW_DIMENSION: u32 = 1;

// 使用 Tauri 窗口和显示器信息决定本次启动的位置和大小。
//
// 配置文件只负责持久化，窗口对象只负责实际应用。这个 service 不
// 直接持有 `MutexGuard`，因此不会把存储锁跨到 Tauri API 调用中。
pub fn initialize_window<R: tauri::Runtime>(
    state: &AppState,
    window: &tauri::WebviewWindow<R>,
) -> AppResult<WindowInitResponse> {
    let settings = {
        let store = state.get_lock()?;
        store.load_settings()?
    };

    // 事件循环里的尺寸监听器要靠这份登记值判断「当前处于哪个视图模式」，
    // 窗口一建出来就要对齐，否则启动后的第一批 `Resized` 会被按默认的
    // 展开态解读。
    {
        let mut runtime = state.window_runtime()?;
        runtime.view_mode = settings.view_mode;
    }

    let mut needs_persist = !settings.window.is_initialized;
    let (position, size) = if settings.window.is_initialized {
        let position = settings
            .window
            .position
            .ok_or_else(|| invalid_window_state("已初始化配置缺少窗口位置"))?;
        let size = settings
            .window
            .size
            .ok_or_else(|| invalid_window_state("已初始化配置缺少窗口大小"))?;
        validate_window_size(size)?;

        if restored_position_is_reachable(window, position)
            && restored_size_is_usable(window, &settings, size)
        {
            (position, size)
        } else {
            // 上次可能是在已经拔掉的外接显示器上使用，或者配置里存的是旧版本
            // 算出来的坏值。这个应用既不在任务栏也没有窗口边框，窗口一旦跑到
            // 屏幕外、或者小到看不见，用户都无法找回，所以这里按当前工作区重新
            // 推导，并把修正后的几何写回配置。
            log::warn!("已保存的窗口状态不可用（位置越界或尺寸过小），改为按当前工作区重新计算");
            needs_persist = true;
            current_geometry(window, settings.view_mode, settings.window.size_override)?
        }
    } else {
        // 首次启动时按当前视图模式从显示器工作区推导悬浮窗几何。
        current_geometry(window, settings.view_mode, settings.window.size_override)?
    };

    apply_geometry(state, window, position, size)?;
    if needs_persist {
        persist_geometry(state, position, size)?;
    }

    Ok(WindowInitResponse::new(position, size))
}

// 判断恢复出来的窗口左上角是否还落在某个显示器的工作区内。
//
// 只做左上角的粗略判断：目的是拦住「窗口彻底跑到屏幕外」这种不可恢复的
// 情况，而不是精细的可见面积计算。拿不到显示器列表时返回 true，宁可保留
// 用户原来的位置，也不要因为查询失败就把窗口挪走。
fn restored_position_is_reachable<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    position: WindowPosition,
) -> bool {
    let Ok(monitors) = window.available_monitors() else {
        return true;
    };

    if monitors.is_empty() {
        return true;
    }

    monitors.iter().any(|monitor| {
        let area = monitor.work_area();
        position.x >= area.position.x
            && position.y >= area.position.y
            && position.x < area.position.x + area.size.width as i32
            && position.y < area.position.y + area.size.height as i32
    })
}

// 判断恢复出来的窗口尺寸是否仍然够用。
//
// 配置里的尺寸基本由本模块推导，所以这个检查拦的不是「用户改过」，而是
// 「缓存值已经过期」：收起态的内容盒只有 3px 宽，窗口只要比推导值小一点，
// 那圈 16px 的阴影留白就会把它压成 0 —— 指示条既看不见也点不到，而窗口
// 又是无边框、不进任务栏的，用户只能手工去改配置文件。
//
// 判据取 `current_geometry` 而不是设计稿推导值：有用户尺寸覆盖时，覆盖值
// 才是「这次应该多大」，否则用户把窗口拖宽之后，每次启动都会因为旧缓存
// 比覆盖值小而被判为不可用。
//
// 只拦「小到看不见」，不拦「大到不好看」：偏大的窗口仍然可用，收紧它属于
// 显示器尺寸变化后的重算问题，不在这个兜底范围内。
//
// 拿不到显示器信息时返回 true，与 `restored_position_is_reachable` 保持一致：
// 宁可保留已保存的值，也不要因为查询失败就把它丢掉。
fn restored_size_is_usable<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    settings: &Settings,
    size: WindowSize,
) -> bool {
    let Ok((_, required)) =
        current_geometry(window, settings.view_mode, settings.window.size_override)
    else {
        return true;
    };

    size.width >= required.width && size.height >= required.height
}

// 切换侧栏的展开态与收起态。
//
// 收起态需要把窗口真的缩成一条指示条，否则透明窗口仍然会拦截它覆盖
// 区域的鼠标事件。前端只提交目标视图模式，窗口几何由本函数按当前
// 显示器工作区重新推导，这样设计稿里的百分比换算只在 Rust 侧存在一份。
pub fn apply_view_mode<R: tauri::Runtime>(
    state: &AppState,
    window: &tauri::WebviewWindow<R>,
    view_mode: ViewMode,
) -> AppResult<WindowInitResponse> {
    // 拖拽期间被节流掉的覆盖值先补写，再读配置 —— 顺序反了会读到旧的
    // 覆盖值，展开态就会用上一次的尺寸。
    flush_user_resize(state)?;

    let size_override = {
        let store = state.get_lock()?;
        store.load_settings()?.window.size_override
    };

    let (position, size) = current_geometry(window, view_mode, size_override)?;

    // 先登记视图模式、清空尺寸登记值，再动窗口。
    //
    // 两个字段都必须在下发几何之前写好：改 `resizable` 会改窗口样式，
    // Windows 可能顺手重排一次尺寸并派发 `Resized`，而尺寸监听器是靠
    // 「刚下发了什么」和「当前处于哪个视图模式」来判断这次变化是不是
    // 用户拖出来的。`last_applied` 置空表示几何正在下发途中，这期间的
    // 事件一律跳过，由 `apply_geometry` 重新登记。
    {
        let mut runtime = state.window_runtime()?;
        runtime.view_mode = view_mode;
        runtime.last_applied = None;
    }

    apply_geometry(state, window, position, size)?;

    let store = state.get_lock()?;
    store.mutate_settings(|settings| {
        settings.view_mode = view_mode;
        write_geometry(settings, position, size);
        Ok(())
    })?;

    Ok(WindowInitResponse::new(position, size))
}

// 保存窗口退出前的位置和尺寸。
//
// 位置取窗口实际值；尺寸取窗口实际值经 `resolve_size` 归一化后的结果（原因见
// 函数内注释）。这是退出事件调用的后端内部函数，不暴露给前端。读取窗口失败时不
// 覆盖已有配置；写入配置失败则把错误交给调用方记录。
pub fn persist_window_state(
    state: &AppState,
    window: &tauri::WebviewWindow<impl tauri::Runtime>,
) -> AppResult<()> {
    // 用户最后一次拖拽可能正好落在节流窗口里，退出是补写它的最后机会。
    flush_user_resize(state)?;

    let position = window.inner_position().map_err(|error| {
        crate::models::AppError::new("window_position_read_failed", error.to_string())
    })?;
    let size = window.inner_size().map_err(|error| {
        crate::models::AppError::new("window_size_read_failed", error.to_string())
    })?;

    let position = WindowPosition {
        x: position.x,
        y: position.y,
    };
    let size = WindowSize {
        width: size.width.max(MIN_WINDOW_DIMENSION),
        height: size.height.max(MIN_WINDOW_DIMENSION),
    };

    // 按覆盖值归一化后再落盘。
    //
    // Windows 不拦程序化改尺寸，用户却可以把边框拖到设计稿上限之外：那一刻窗口
    // 真的比上限大，而这个「实际尺寸」如果原样存下来，下次启动会先开出一个铺满
    // 大半个屏幕的透明窗口 —— 透明窗口是会拦截鼠标事件的 —— 几百毫秒后才被前端
    // 的 `syncWindowGeometry()` 收回上限。存成「下次会解析成什么」就没有这一下。
    //
    // 没有覆盖值时 `resolve_size` 原样返回，因此「记住用户拖出来的尺寸」这条
    // 主路径不受影响；收起态也原样返回（覆盖值对收起态无效）。
    let (view_mode, size_override) = {
        let store = state.get_lock()?;
        let settings = store.load_settings()?;
        (settings.view_mode, settings.window.size_override)
    };
    let size = resolve_size(view_mode, size, size_override, current_scale_factor(window));

    persist_geometry(state, position, size)
}

// 按前端拖拽把手的实时结果改一次窗口几何。
//
// 窗口是 `resizable: false`（见 `tauri.conf.json`），没有原生缩放边框可用 ——
// 实测下来在这个无边框窗口上根本走不通：tao 自己算的 `WM_NCHITTEST` 会被
// 铺满整个客户区的 WebView2 子窗口截走，父窗口收不到非客户区消息；而直接
// 投递 `WM_NCLBUTTONDOWN` 的 `startResizeDragging` 也不会触发缩放循环。
// 于是缩放由前端把手驱动，每次 `pointermove` 调一次这里。
//
// 刻意**不**登记 `last_applied`：这次变化就是用户拖出来的，交给
// `capture_user_resize` 按正常路径记录并节流落盘，等于复用了「记住用户尺寸」
// 的既有实现，落盘频率也已经被 `OVERRIDE_WRITE_INTERVAL` 限住。
pub fn resize_window<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    request: ResizeWindowRequest,
) -> AppResult<()> {
    let scale_factor = current_scale_factor(window);
    let size = css_to_physical_size(
        CssSize {
            width: request.width,
            height: request.height,
        },
        scale_factor,
    );

    let position = clamp_into_work_area(
        window,
        WindowPosition {
            x: to_physical_i32(request.x, scale_factor),
            y: to_physical_i32(request.y, scale_factor),
        },
        size,
    );

    window
        .set_size(tauri::PhysicalSize::new(size.width, size.height))
        .map_err(|error| {
            crate::models::AppError::new("window_size_apply_failed", error.to_string())
        })?;
    window
        .set_position(tauri::PhysicalPosition::new(position.x, position.y))
        .map_err(|error| {
            crate::models::AppError::new("window_position_apply_failed", error.to_string())
        })?;

    Ok(())
}

// 把窗口几何夹进当前显示器工作区，避免拖上/下边缘时把窗口拖出屏幕。
//
// 拿不到显示器信息时原样返回 —— 宁可让用户把窗口拖到奇怪的位置，也不要因为
// 一次查询失败就把他的操作吞掉。
fn clamp_into_work_area<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    position: WindowPosition,
    size: WindowSize,
) -> WindowPosition {
    let Some(monitor) = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
    else {
        return position;
    };

    let area = monitor.work_area();
    let max_x = area.position.x + area.size.width as i32 - size.width as i32;
    let max_y = area.position.y + area.size.height as i32 - size.height as i32;

    WindowPosition {
        x: position
            .x
            .clamp(area.position.x, max_x.max(area.position.x)),
        y: position
            .y
            .clamp(area.position.y, max_y.max(area.position.y)),
    }
}

// `to_physical` 的带符号版本：窗口坐标允许为负数（多显示器时主屏左侧为负）。
fn to_physical_i32(css_px: i32, scale_factor: f64) -> i32 {
    (css_px as f64 * scale_factor).round() as i32
}

// `RunEvent::WindowEvent` 的 `Resized` 分支入口。
//
// 事件循环的回调没有地方返回 `Result`，所以这里只记日志：丢一次尺寸
// 记录只是让用户少了一次「记住我的尺寸」，不影响窗口可用。
pub fn handle_window_resized<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    use tauri::Manager;

    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return;
    };
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };

    if let Err(error) = capture_user_resize(state.inner(), &window) {
        log::warn!("记录用户拖拽的窗口尺寸失败: {error}");
    }
}

// 把一次 `Resized` 事件解读成「用户拖拽」并记下尺寸。
//
// 事件不携带来源信息，所以这里用三层过滤把程序下发的变化排除掉，任何一层
// 不满足都直接返回：
//
// 1. `last_applied` 为空 —— 几何正在下发途中，或窗口刚创建还没摆过位置，
//    这次的尺寸来自 `tauri.conf.json` 的兜底值或 Windows 的重排；
// 2. 尺寸等于刚下发的值 —— 程序化改尺寸自己触发的回声；
// 3. 当前不是展开态 —— 收起态尺寸永远由设计稿决定，窗口只有一条指示条，
//    把它拖大只会变成一块挡住桌面又看不见内容的透明区域。
//
// 通过过滤的尺寸先落在内存里，再按 `OVERRIDE_WRITE_INTERVAL` 节流落盘。
fn capture_user_resize<R: tauri::Runtime>(
    state: &AppState,
    window: &tauri::WebviewWindow<R>,
) -> AppResult<()> {
    let Ok(inner) = window.inner_size() else {
        return Ok(());
    };
    let size = WindowSize {
        width: inner.width,
        height: inner.height,
    };

    let view_mode = {
        let runtime = state.window_runtime()?;
        if runtime.last_applied.is_none() || runtime.last_applied == Some(size) {
            return Ok(());
        }
        runtime.view_mode
    };

    if view_mode != ViewMode::Expanded {
        return Ok(());
    }

    // 拖回设计尺寸就当成「恢复默认」：留一个等值的覆盖只会让显示器分辨率
    // 或缩放比变化之后无法再按新工作区重算。
    //
    // 物理像素不能直接参与这一步比较和落盘：它只在测量时那块屏幕上成立。
    // 先换算回设计稿的 CSS 像素，比较与记录都在同一单位下进行。
    let scale_factor = current_scale_factor(window);
    let override_size = override_from_physical(size, scale_factor);
    let design = current_geometry(window, view_mode, None)
        .map(|(_, design)| override_from_physical(design, scale_factor));
    let change = match design {
        Ok(design) if design == override_size => OverrideChange::Clear,
        _ => OverrideChange::Set {
            size: override_size,
            applied: css_to_physical_size(override_size, scale_factor),
        },
    };

    let should_write = {
        let mut runtime = state.window_runtime()?;
        runtime.pending_override = Some(change);

        let throttled = runtime
            .override_written_at
            .is_some_and(|written_at| written_at.elapsed() < OVERRIDE_WRITE_INTERVAL);
        if throttled {
            false
        } else {
            runtime.override_written_at = Some(Instant::now());
            true
        }
    };

    if should_write {
        flush_user_resize(state)?;
    }

    Ok(())
}

// 把节流期间攒下的用户尺寸变更补写进配置。
//
// 调用点是所有「窗口即将安静下来」的时刻：切换视图模式前、退出前、以及
// 前端松开缩放把手时。这些时刻之后窗口尺寸不会再变，落盘的值就是最终值。
//
// 松手这一路不能省：`Resized` 只在尺寸变化时派发，而落盘有 300ms 节流，
// 用户停手前的最后一段位移大概率正好落在节流窗口里 —— 不补写的话，记住的
// 是「最后 300ms 之前」的尺寸，下次启动窗口会比用户拖出来的短一截。
pub fn flush_user_resize(state: &AppState) -> AppResult<()> {
    let pending = {
        let mut runtime = state.window_runtime()?;
        runtime.pending_override.take()
    };

    let Some(change) = pending else {
        return Ok(());
    };

    let store = state.get_lock()?;
    store.mutate_settings(|settings| {
        match change {
            OverrideChange::Set { size, applied } => {
                settings.window.size_override = Some(size);
                // 缓存尺寸跟着覆盖值一起更新。`restored_size_is_usable` 是拿
                // 缓存值和 `current_geometry` 的结果比的，不同步的话下次启动
                // 会把用户自己的尺寸判成过期值，多打一条告警并重算一遍。
                settings.window.size = Some(applied);
            }
            OverrideChange::Clear => settings.window.size_override = None,
        }
        Ok(())
    })
}

// 把窗口上报的物理像素尺寸换算成可落盘的设计稿尺寸覆盖。
//
// 夹取在 CSS 空间完成：上下限常量本身就是 CSS 像素，在这里夹一次，
// `resolve_size` 就只需要做一次乘法。夹取后结果永远不小于设计下限，
// `restored_size_is_usable` 的判据因此依然成立。
//
// 与 `css_to_physical_size` 是一对互逆的纯函数，测试层可以直接验证
// 「150% 屏上拖出的尺寸，到 100% 屏上仍是同一个设计稿尺寸」。
pub fn override_from_physical(size: WindowSize, scale_factor: f64) -> CssSize {
    let scale_factor = sanitize_scale_factor(scale_factor);
    CssSize {
        width: clamp_dimension(
            to_css(size.width, scale_factor),
            css::OVERLAY_WIDTH_MIN_PX,
            css::OVERLAY_WIDTH_MAX_PX,
        ),
        height: clamp_dimension(
            to_css(size.height, scale_factor),
            css::OVERLAY_HEIGHT_MIN_PX,
            css::OVERLAY_HEIGHT_MAX_PX,
        ),
    }
}

// 把设计稿尺寸覆盖换算成窗口几何所需的物理尺寸，并夹进设计稿规定的范围。
//
// 上下限是设计稿的 CSS 像素，所以夹取必须在换算之前做：先夹物理像素等于把
// 上下限也乘上了缩放比，虽然数值上等价，但一旦有人改了缩放比的读法（比如
// 用显示器上报值而不是窗口值）就会悄悄错开。
fn css_to_physical_size(size: CssSize, scale_factor: f64) -> WindowSize {
    let scale_factor = sanitize_scale_factor(scale_factor);
    WindowSize {
        width: to_physical(
            clamp_dimension(
                size.width,
                css::OVERLAY_WIDTH_MIN_PX,
                css::OVERLAY_WIDTH_MAX_PX,
            ),
            scale_factor,
        ),
        height: to_physical(
            clamp_dimension(
                size.height,
                css::OVERLAY_HEIGHT_MIN_PX,
                css::OVERLAY_HEIGHT_MAX_PX,
            ),
            scale_factor,
        ),
    }
}

impl WindowInitResponse {
    fn new(position: WindowPosition, size: WindowSize) -> Self {
        Self {
            is_initialized: true,
            position,
            size,
        }
    }
}

// 统一持久化窗口几何信息，避免首次初始化和退出保存分别维护同一组字段。
fn persist_geometry(state: &AppState, position: WindowPosition, size: WindowSize) -> AppResult<()> {
    let store = state.get_lock()?;
    store.mutate_settings(|settings| {
        write_geometry(settings, position, size);
        Ok(())
    })
}

// 把一次窗口几何结果写进配置快照。
//
// `is_initialized` 与位置、大小必须成套出现，因此这里集中处理，
// 避免出现“已初始化但没有可恢复尺寸”的半截状态。
fn write_geometry(settings: &mut Settings, position: WindowPosition, size: WindowSize) {
    settings.window.is_initialized = true;
    settings.window.position = Some(position);
    settings.window.size = Some(size);
}

fn current_geometry<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    view_mode: ViewMode,
    size_override: Option<CssSize>,
) -> AppResult<(WindowPosition, WindowSize)> {
    let monitor = window
        .current_monitor()
        .map_err(|error| crate::models::AppError::new("monitor_read_failed", error.to_string()))?
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or_else(|| invalid_window_state("无法获取当前显示器信息"))?;

    // 设计稿常量是 CSS 像素，窗口几何是物理像素，换算需要窗口所在显示器的缩放比。
    let scale_factor = current_scale_factor(window);

    let work_area = monitor.work_area();
    let (position, size) = overlay_geometry(
        work_area.position.x,
        work_area.position.y,
        work_area.size.width,
        work_area.size.height,
        view_mode,
        scale_factor,
    );

    Ok((
        position,
        resolve_size(view_mode, size, size_override, scale_factor),
    ))
}

// 用用户的尺寸覆盖替换展开态的窗口尺寸。
//
// 窗口可拖拽改尺寸之后，设计稿推导值不再是唯一来源：用户在展开态拖出来的
// 尺寸更贴近他的意图，只要还有效就一直沿用。收起态永远不覆盖 —— 收起态
// 窗口只有一条 3px 指示条，任何更大的尺寸都会变成一块挡住桌面又看不见
// 内容的透明区域，而透明窗口是会拦截鼠标事件的。
//
// 覆盖值以设计稿 CSS 像素存储（`CssSize`），因此与显示器缩放比无关：夹取
// 也在 CSS 空间完成，再统一乘当前缩放比换算成物理像素。夹取后结果永远不小于
// 设计下限，`restored_size_is_usable` 的判据因此依然成立。
//
// 位置不跟着变：面板锚在窗口左上角，窗口变大就是面板向右向下延伸。
//
// 与 `overlay_geometry` 一样是不依赖 Tauri 窗口对象的纯函数，测试层可以
// 直接验证各视图模式、各类越界覆盖值和跨缩放比的一致性。
pub fn resolve_size(
    view_mode: ViewMode,
    size: WindowSize,
    size_override: Option<CssSize>,
    scale_factor: f64,
) -> WindowSize {
    match size_override.filter(|_| view_mode == ViewMode::Expanded) {
        Some(size_override) => css_to_physical_size(size_override, scale_factor),
        None => size,
    }
}

// 根据显示器工作区推导悬浮窗的几何信息。
//
// 主窗口是一个透明、无边框、置顶的悬浮层，但它的尺寸并不等于侧栏尺寸：
// 窗口在侧栏包围盒之外还要多留一圈透明留白（gutter），CSS 的 box-shadow
// 才不会被窗口边界裁掉。
//
// 窗口左上角紧贴工作区左上角，面板在窗口内的偏移完全由 CSS 的 padding
// 表达（`.dc-window-expanded` / `.dc-window-collapsed`）：
//
// 展开态：窗口宽 = 左侧 2px + 场景宽 18.75% + 右侧留白；
//         窗口高 = 场景高 50% + 上下留白；窗口 y 上移一个留白，
//         使面板顶部落在场景高度的 12.5%。
// 收起态：窗口宽 = 3px 指示条 + 右侧留白；窗口高 = 80px + 上下留白。
//
// 单位约定（容易搞错，务必分清）：工作区与返回值都是**物理像素**，而
// `css` 模块里的常量是设计稿的**CSS 像素**，必须先乘 `scale_factor` 才能
// 相加。百分比是例外 —— 比例与缩放比无关，直接按物理工作区换算即可。
//
// 这是一个不依赖 Tauri 窗口对象的纯函数，测试层可以直接验证多显示器、
// 负坐标、极小工作区和高 DPI 等边界场景。
pub fn overlay_geometry(
    work_area_x: i32,
    work_area_y: i32,
    work_area_width: u32,
    work_area_height: u32,
    view_mode: ViewMode,
    scale_factor: f64,
) -> (WindowPosition, WindowSize) {
    let scale_factor = sanitize_scale_factor(scale_factor);
    let gutter = to_physical(css::OVERLAY_SHADOW_GUTTER_PX, scale_factor);
    let (panel_left, panel_top, panel_width, panel_height) = match view_mode {
        ViewMode::Expanded => (
            to_physical(css::SIDEBAR_LEFT_PX, scale_factor),
            scale(work_area_height, css::SIDEBAR_TOP_PERCENT),
            clamp_dimension(
                scale(work_area_width, css::SIDEBAR_WIDTH_PERCENT),
                to_physical(css::OVERLAY_WIDTH_MIN_PX, scale_factor),
                to_physical(css::OVERLAY_WIDTH_MAX_PX, scale_factor),
            ),
            clamp_dimension(
                scale(work_area_height, css::SIDEBAR_HEIGHT_PERCENT),
                to_physical(css::OVERLAY_HEIGHT_MIN_PX, scale_factor),
                to_physical(css::OVERLAY_HEIGHT_MAX_PX, scale_factor),
            ),
        ),
        ViewMode::Collapsed => (
            to_physical(css::COLLAPSED_STRIP_LEFT_PX, scale_factor),
            scale(work_area_height, css::COLLAPSED_STRIP_TOP_PERCENT),
            to_physical(css::COLLAPSED_STRIP_WIDTH_PX, scale_factor),
            to_physical(css::COLLAPSED_STRIP_HEIGHT_PX, scale_factor),
        ),
    };

    // 窗口左边缘紧贴工作区左边缘。面板自身那 2px 左边距由 CSS 的
    // `.dc-window-expanded { padding-left: 2px }` 提供，因此 `panel_left`
    // 只参与窗口宽度计算；如果把 `panel_left` 再加到窗口 x 上，边距会被
    // 算两遍，面板会比设计稿右移 2px。
    let position = WindowPosition {
        x: work_area_x,
        y: work_area_y + panel_top as i32 - gutter as i32,
    };
    let size = WindowSize {
        width: (panel_left + panel_width + gutter).max(MIN_WINDOW_DIMENSION),
        height: (panel_height + gutter * 2).max(MIN_WINDOW_DIMENSION),
    };

    (position, size)
}

// 按百分比换算像素长度。工作区尺寸是 u32，不存在负数或 NaN 输入。
fn scale(total: u32, percent: f32) -> u32 {
    (total as f32 * percent / 100.0).round() as u32
}

// 把设计稿的 CSS 像素换算成窗口几何所需的物理像素。
//
// 这一步不能省。窗口几何（`set_size` / `set_position`）与显示器工作区都是物理
// 像素，而设计稿里的留白、指示条尺寸都是 CSS 像素：在 150% 缩放的屏幕上，16px
// 的阴影留白实际占 24 个物理像素，可收起态窗口总共只有 3 + 16 = 19 个 CSS 像素
// 宽。不换算的话窗口宽度只有 19 个物理像素（≈ 12.7 CSS 像素），连留白本身都
// 装不下，内容盒被压成 0 —— 指示条在屏幕上完全不可见，用户也就无从点它回到
// 展开态。
fn to_physical(css_px: u32, scale_factor: f64) -> u32 {
    (css_px as f64 * scale_factor).round() as u32
}

// `to_physical` 的逆运算：把物理像素换算回设计稿的 CSS 像素。
//
// 用户尺寸覆盖要按 CSS 像素落盘，而窗口上报的是物理像素，记录时就得换回来。
fn to_css(physical_px: u32, scale_factor: f64) -> u32 {
    (physical_px as f64 / scale_factor).round() as u32
}

// 读取窗口所在显示器的缩放比。
//
// 窗口自己读不到时退回显示器上报的值 —— 比退回 1.0 更接近真实情况：高 DPI 屏上
// 退回 1.0 会重新引入「留白吃光内容盒」的缺陷（收起态窗口只剩 19 个物理像素宽，
// 装不下 16px 留白，指示条被压成 0 宽而彻底消失）。两者都拿不到才退回 100%。
//
// 统一走这一个入口，是因为缩放比同时决定「设计稿常量怎么换算成物理像素」和
// 「用户拖出来的物理像素怎么换算回设计稿尺寸」，两条路径必须用同一个值，
// 否则拖一次就会差一个缩放比地存进配置。
fn current_scale_factor<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) -> f64 {
    let reported = window.scale_factor().ok().or_else(|| {
        window
            .current_monitor()
            .ok()
            .flatten()
            .or_else(|| window.primary_monitor().ok().flatten())
            .map(|monitor| monitor.scale_factor())
    });

    sanitize_scale_factor(reported.unwrap_or(1.0))
}

// 缩放比理论上恒为正的有限值。万一读到异常值就退回 100%，
// 避免算出把留白吃光、甚至零尺寸的窗口。
fn sanitize_scale_factor(scale_factor: f64) -> f64 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

fn clamp_dimension(value: u32, min: u32, max: u32) -> u32 {
    value.clamp(min, max.max(min))
}

// 下发几何，并登记「这次尺寸是程序写的」。
//
// 登记和 `set_size` 必须成对出现：`Resized` 事件不带来源信息，尺寸监听器
// 只能靠这份登记值区分程序化改尺寸和用户拖拽。漏登记一次，紧接着的那次
// 程序化变化就会被当成用户拖拽写进配置。
//
// 登记必须在 `set_size` **之前**完成 —— 事件可能在 `set_size` 内部就被
// 派发出来。同理，锁不能跨过 `set_size`：事件处理器也要拿这把锁，持锁
// 期间若被重入就是死锁。
fn apply_geometry<R: tauri::Runtime>(
    state: &AppState,
    window: &tauri::WebviewWindow<R>,
    position: WindowPosition,
    size: WindowSize,
) -> AppResult<()> {
    {
        let mut runtime = state.window_runtime()?;
        runtime.last_applied = Some(size);
    }

    window
        .set_size(tauri::PhysicalSize::new(size.width, size.height))
        .map_err(|error| {
            crate::models::AppError::new("window_size_apply_failed", error.to_string())
        })?;
    window
        .set_position(tauri::PhysicalPosition::new(position.x, position.y))
        .map_err(|error| {
            crate::models::AppError::new("window_position_apply_failed", error.to_string())
        })?;
    Ok(())
}

// 窗口的缩放不在这里开关，而是恒为 `false`（`tauri.conf.json`）。
//
// 曾经展开态会临时打开 `resizable`，让 Windows 给窗口加上 `WS_THICKFRAME`
// 以得到可拖拽的边框。代价是四条边和四个角都能拖，而窗口左边缘是贴着屏幕
// 工作区左边缘的 —— 拖它会把面板挪离设计稿位置，收起态（窗口只有 3px 宽）
// 更是一拖就变成一块挡在桌面上的透明区域。
//
// 现在改由前端在面板的右边缘和上下边缘放把手，按下时调用
// `startResizeDragging` 让系统接管这次缩放：能调哪几条边由前端精确控制，
// 而缩放产生的 `Resized` 事件仍然由 `capture_user_resize` 照常记录。
// 经 tao 源码确认，`drag_resize_window` 只是投递一条 `WM_NCLBUTTONDOWN`，
// 并不检查窗口是否 `resizable`，因此关掉原生边框后依然有效。
fn validate_window_size(size: WindowSize) -> AppResult<()> {
    if size.width == 0 || size.height == 0 {
        return Err(invalid_window_state("已保存的窗口宽度和高度必须大于 0"));
    }

    Ok(())
}

fn invalid_window_state(message: impl Into<String>) -> crate::models::AppError {
    crate::models::AppError::new("invalid_window_state", message)
}

// 保留既有服务层入口；实际状态机逻辑集中在 `initialize_window`。
pub fn is_init(
    state: &AppState,
    window: &tauri::WebviewWindow<impl tauri::Runtime>,
) -> AppResult<WindowInitResponse> {
    initialize_window(state, window)
}
