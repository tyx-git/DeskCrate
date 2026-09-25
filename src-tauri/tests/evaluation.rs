//! 性能评估入口。
//!
//! 这些用例故意使用 `#[ignore]`：启动真实 GUI、访问外网和提取 Windows 图标都
//! 会受到机器、网络、杀毒软件与桌面会话影响，不应该成为普通回归测试的门禁。
//! 手动评估时使用：
//!
//! ```text
//! cargo test --test evaluation -- --ignored --nocapture
//! ```
//!
//! 输出中的时间是端到端耗时，内存是目标进程在 GUI 已经进入空闲后的工作集，
//! 不能和任务管理器的“提交大小”混为一谈。多次运行后比较中位数比比较单次值更
//! 可靠。

mod support;

use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use lib::{
    models::{AddAppRequest, AddAppsRequest},
    services::catalog,
};

use support::TestDir;

const STARTUP_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy)]
struct ProcessMemory {
    working_set_bytes: usize,
    peak_working_set_bytes: usize,
}

#[test]
#[ignore = "启动真实 GUI 进程，只用于手动性能评估"]
fn evaluate_startup_time_and_memory() {
    let binary = application_binary().expect(
        "找不到 deskcrate.exe；请先构建应用目标，或在 Cargo 提供 \
         CARGO_BIN_EXE_deskcrate 后再运行评估",
    );

    let mut child = Command::new(&binary)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|error| panic!("无法启动 {}：{error}", binary.display()));

    // 无论等待或读取内存是否失败，都先结束子进程，避免评估失败后残留 DeskCrate。
    let ready = wait_for_native_window(&child, STARTUP_TIMEOUT);
    let startup_ms = ready.as_ref().ok().copied();
    // 窗口创建后留出一个很短的稳定窗口，避免只量到 Tauri 刚分配的最小工作集。
    if ready.is_ok() {
        std::thread::sleep(Duration::from_millis(500));
    }
    let memory = ready.as_ref().ok().and_then(|_| process_memory(&child));
    let _ = child.kill();
    let _ = child.wait();

    let ready_ms = ready.unwrap_or_else(|error| panic!("等待原生窗口创建失败：{error}"));
    let startup_ms = startup_ms.unwrap_or(ready_ms);
    let memory = memory.unwrap_or_else(|| panic!("读取目标进程工作集失败"));

    println!(
        "[evaluation] startup_elapsed_ms={startup_ms} native_window_ms={ready_ms} \
         working_set_bytes={} peak_working_set_bytes={} binary={}",
        memory.working_set_bytes,
        memory.peak_working_set_bytes,
        binary.display()
    );
}

#[test]
#[ignore = "提取 Windows 图标，只用于手动性能评估"]
fn evaluate_local_icon_extraction_time() {
    let directory = TestDir::new("evaluation-local-icon");
    let source = directory.child("icon-source");
    std::fs::create_dir_all(&source).expect("评估目录应当可以创建");
    let state = directory.state();

    let started = Instant::now();
    let app = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: None,
            source_path: source.to_string_lossy().into_owned(),
            name: Some("本地图标评估".to_owned()),
            root_index: None,
        },
    )
    .expect("本地应用应当可以添加");
    let elapsed_ms = started.elapsed().as_millis();
    let icon_path = directory.child(&format!("icons/{}.png", app.id));
    let icon_bytes = std::fs::metadata(&icon_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);

    println!(
        "[evaluation] local_icon elapsed_ms={elapsed_ms} bytes={icon_bytes} \
         icon_path={} icon_type={:?}",
        icon_path.display(),
        app.icon_type
    );
}

#[test]
#[ignore = "访问外网，只用于手动性能评估"]
#[cfg(debug_assertions)]
fn evaluate_url_icon_crawler_time() {
    let url = std::env::var("DESKCRATE_EVALUATION_URL")
        .unwrap_or_else(|_| "https://github.com".to_owned());
    let started = Instant::now();
    let report =
        lib::commands::diagnostics::probe_url_icon(url.clone()).expect("图标探针应该返回报告");
    let elapsed_ms = started.elapsed().as_millis();
    let report_json = serde_json::to_string(&report).expect("探针报告应当可以序列化");

    println!(
        "[evaluation] url_icon_crawler elapsed_ms={elapsed_ms} url={url} report={report_json}"
    );
}

#[test]
#[ignore = "访问外网，只用于手动性能评估"]
fn evaluate_url_add_app_time() {
    let directory = TestDir::new("evaluation-url-add");
    let state = directory.state();
    let url = std::env::var("DESKCRATE_EVALUATION_URL")
        .unwrap_or_else(|_| "https://github.com".to_owned());

    // `catalog::add_app` 只负责快速落库；后台网址图标抓取在 Command 层异步调度。
    // 因此这个指标用于确认“添加应用”不会被网络抓取阻塞，不能当作抓图标耗时。
    let started = Instant::now();
    let app = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: None,
            source_path: url.clone(),
            name: Some("网址添加评估".to_owned()),
            root_index: None,
        },
    )
    .expect("网址应用应当可以快速落库");
    let elapsed_ms = started.elapsed().as_millis();

    println!(
        "[evaluation] url_add elapsed_ms={elapsed_ms} url={url} app_id={} icon_url={}",
        app.id, app.icon_url
    );
}

#[test]
#[ignore = "批量添加性能评估，只用于手动执行"]
fn evaluate_batch_add_apps_time() {
    let directory = TestDir::new("evaluation-batch-add");
    let source_dir = directory.child("batch-source");
    std::fs::create_dir_all(&source_dir).expect("批量评估目录应当可以创建");
    let state = directory.state();
    let source_paths = (0..20)
        .map(|index| source_dir.join(format!("item-{index}")))
        .collect::<Vec<_>>();
    for path in &source_paths {
        std::fs::create_dir_all(path).expect("批量来源目录应当可以创建");
    }

    let started = Instant::now();
    let result = catalog::add_apps(
        &state,
        AddAppsRequest {
            parent_id: None,
            source_paths: source_paths
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            root_index: None,
        },
    )
    .expect("批量应用应当可以添加");
    let elapsed_ms = started.elapsed().as_millis();

    println!(
        "[evaluation] batch_add elapsed_ms={elapsed_ms} added={} failed={}",
        result.added.len(),
        result.failed.len()
    );
}

// `CARGO_BIN_EXE_<name>` 里的 `<name>` 是 bin target 名（即包名），改包名必须
// 同步这里，否则会静默回退到「按目录猜路径」那条分支。
fn application_binary() -> Option<PathBuf> {
    if let Some(path) = option_env!("CARGO_BIN_EXE_deskcrate") {
        return Some(PathBuf::from(path));
    }

    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_deskcrate") {
        return Some(PathBuf::from(path));
    }

    // 兼容手动从 target/debug/deps 下执行测试的情况。Cargo 通常会在同一
    // profile 目录旁边生成 deskcrate.exe。
    let mut profile_dir = std::env::current_exe().ok()?;
    profile_dir.pop();
    if profile_dir.file_name().is_some_and(|name| name == "deps") {
        profile_dir.pop();
    }
    let binary = profile_dir.join(if cfg!(windows) {
        "deskcrate.exe"
    } else {
        "deskcrate"
    });
    binary.is_file().then_some(binary)
}

#[cfg(windows)]
fn wait_for_native_window(child: &Child, timeout: Duration) -> Result<u128, String> {
    use windows::Win32::{
        Foundation::{BOOL, HWND, LPARAM},
        UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId},
    };

    struct Search {
        process_id: u32,
        found: bool,
    }

    unsafe extern "system" fn find_process_window(hwnd: HWND, parameter: LPARAM) -> BOOL {
        let search = unsafe { &mut *(parameter.0 as *mut Search) };
        let mut process_id = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        }
        if process_id == search.process_id {
            search.found = true;
            BOOL(0)
        } else {
            BOOL(1)
        }
    }

    let started = Instant::now();
    let deadline = started + timeout;
    let mut search = Search {
        process_id: child.id(),
        found: false,
    };

    while Instant::now() < deadline {
        search.found = false;
        unsafe {
            EnumWindows(
                Some(find_process_window),
                LPARAM((&mut search as *mut Search) as isize),
            )
            .map_err(|error| format!("EnumWindows 失败：{error}"))?;
        }
        if search.found {
            return Ok(started.elapsed().as_millis());
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    Err(format!(
        "{}ms 内没有找到目标进程的原生窗口",
        timeout.as_millis()
    ))
}

#[cfg(not(windows))]
fn wait_for_native_window(_child: &Child, _timeout: Duration) -> Result<u128, String> {
    Err("该评估需要 Windows GUI 环境".to_owned())
}

#[cfg(windows)]
fn process_memory(child: &Child) -> Option<ProcessMemory> {
    use std::os::windows::io::AsRawHandle;

    use windows::Win32::{
        Foundation::HANDLE,
        System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
    };

    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..PROCESS_MEMORY_COUNTERS::default()
    };
    let handle = HANDLE(child.as_raw_handle() as _);
    unsafe {
        GetProcessMemoryInfo(
            handle,
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
        .ok()?;
    }

    Some(ProcessMemory {
        working_set_bytes: counters.WorkingSetSize,
        peak_working_set_bytes: counters.PeakWorkingSetSize,
    })
}

#[cfg(not(windows))]
fn process_memory(_child: &Child) -> Option<ProcessMemory> {
    None
}
