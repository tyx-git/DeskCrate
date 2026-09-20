mod support;

use std::{env, fs, path::PathBuf, process::Command};

use app_lib::{
    error::AppResult,
    models::{AppData, AppError, Category, RootItem, Settings, ViewMode},
    storage::{DataRepository, JsonStore, SettingsRepository},
};

use support::TestDir;

fn category(id: &str, name: &str) -> Category {
    Category {
        id: id.to_owned(),
        name: name.to_owned(),
        is_collapsed: false,
        groups: Vec::new(),
        created_time: "2026-08-31T00:00:00Z".to_owned(),
    }
}

#[test]
fn store_exposes_the_expected_paths_and_creates_layout() {
    let directory = TestDir::new("paths");
    let store = directory.store();

    assert_eq!(store.path(), directory.path());
    // 用逐段 join 表达期望值：路径必须是平台分隔符，不能混着正斜杠 ——
    // 它会出现在用户可见的报错信息里。
    let config_dir = directory.child("config");
    assert_eq!(store.config_path(), config_dir.join("config.json"));
    assert_eq!(store.data_path(), config_dir.join("data.json"));
    assert!(config_dir.is_dir());
    assert!(directory.child("icons").is_dir());
    assert!(store.config_path().is_file());
    assert!(store.data_path().is_file());
    assert!(config_dir.join("data.json.lock").is_file());
    assert!(config_dir.join("config.json.lock").is_file());
}

#[test]
fn ensure_layout_is_idempotent_and_preserves_existing_files() {
    let directory = TestDir::new("layout");
    let store = directory.store();

    let data = AppData {
        version: 9,
        ..AppData::default()
    };
    store.save_data(&data).expect("data should be saved");

    let settings = Settings {
        auto_collapse: true,
        ..Settings::default()
    };
    store
        .save_settings(&settings)
        .expect("settings should be saved");

    store
        .ensure_layout()
        .expect("second layout setup should succeed");

    assert_eq!(store.load_data().expect("data should load"), data);
    assert_eq!(
        store.load_settings().expect("settings should load"),
        settings
    );
}

#[test]
fn data_and_settings_round_trip_through_their_repository_traits() {
    let directory = TestDir::new("round-trip");
    let store = directory.store();

    let data = AppData {
        version: 2,
        items: vec![RootItem::Category(category("category-1", "工具"))],
        ..AppData::default()
    };
    let settings = Settings {
        auto_collapse: true,
        ..Settings::default()
    };

    store.save_data(&data).expect("data should save");
    store
        .save_settings(&settings)
        .expect("settings should save");

    assert_eq!(store.load_data().expect("data should load"), data);
    assert_eq!(
        store.load_settings().expect("settings should load"),
        settings
    );

    let raw = fs::read_to_string(store.data_path()).expect("data JSON should be readable");
    assert!(raw.contains("\"version\": 2"));
    assert!(raw.contains("\"name\": \"工具\""));
}

#[test]
fn mutate_data_and_settings_return_operation_values_and_persist_changes() {
    let directory = TestDir::new("mutate");
    let store = directory.store();

    let added_name = store
        .mutate_data(|data| {
            data.items
                .push(RootItem::Category(category("category-1", "开发")));
            Ok::<_, AppError>(data.items[0].id().unwrap_or_default().to_owned())
        })
        .expect("data mutation should succeed");

    let setting_value = store
        .mutate_settings(|settings| {
            settings.auto_collapse = true;
            Ok::<_, AppError>(settings.auto_collapse)
        })
        .expect("settings mutation should succeed");

    // `items[0].id()` 是分类的 UUID，不是空串；这里只验证「改动被持久化了」。
    assert!(!added_name.is_empty());
    assert!(setting_value);
    assert_eq!(store.load_data().unwrap().items.len(), 1);
    assert!(store.load_settings().unwrap().auto_collapse);
}

#[test]
fn failed_mutations_do_not_write_the_modified_snapshot() {
    let directory = TestDir::new("failed-mutation");
    let store = directory.store();

    let original_data = store.load_data().expect("default data should load");
    let data_result: AppResult<()> = store.mutate_data(|data| {
        data.version = 99;
        Err(AppError::new("expected_failure", "do not persist"))
    });

    let original_settings = store.load_settings().expect("default settings should load");
    let settings_result: AppResult<()> = store.mutate_settings(|settings| {
        settings.auto_collapse = true;
        Err(AppError::new("expected_failure", "do not persist"))
    });

    assert_eq!(data_result.unwrap_err().code, "expected_failure");
    assert_eq!(settings_result.unwrap_err().code, "expected_failure");
    assert_eq!(store.load_data().unwrap(), original_data);
    assert_eq!(store.load_settings().unwrap(), original_settings);
}

#[test]
fn writes_are_atomic_and_leave_no_temporary_files_after_success() {
    let directory = TestDir::new("atomic");
    let store = directory.store();

    let first = AppData {
        version: 1,
        ..AppData::default()
    };
    store.save_data(&first).expect("first write should succeed");

    let second = AppData {
        version: 2,
        ..AppData::default()
    };
    store
        .save_data(&second)
        .expect("replacement write should succeed");

    assert_eq!(store.load_data().unwrap(), second);
    let temporary_files: Vec<_> = fs::read_dir(directory.child("config"))
        .expect("config directory should be readable")
        .map(|entry| entry.expect("directory entry should be valid").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "tmp"))
        .collect();
    assert!(
        temporary_files.is_empty(),
        "temporary files: {temporary_files:?}"
    );
}

#[test]
fn malformed_json_is_reported_as_a_json_error() {
    let directory = TestDir::new("malformed");
    let store = directory.store();

    fs::write(store.data_path(), "{ not valid json").expect("fixture should be written");
    let error = store.load_data().expect_err("malformed data must fail");

    assert_eq!(error.code, "JsonSerdeError");
}

// 找出某个文件的全部损坏备份。`file_name_prefix` 只匹配文件名，不匹配目录。
fn corrupt_backups(directory: &TestDir, file_name_prefix: &str) -> Vec<PathBuf> {
    fs::read_dir(directory.child("config"))
        .expect("config directory should be readable")
        .map(|entry| entry.expect("directory entry should be valid").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(file_name_prefix))
        })
        .collect()
}

// 配置文件损坏时必须自愈：备份原文件 + 用默认配置重建。
//
// 这条路径很关键 —— 文档里把「手工改 config.json」写成窗口跑到屏幕外时的自救
// 方式，而手工改正是最容易改坏的场景。实测过不自愈的后果：应用会永久卡在
// 「窗口几何退回 tauri.conf.json 默认值、且收起态完全失效」的半坏状态。
#[test]
fn corrupt_config_is_backed_up_and_rebuilt() {
    let directory = TestDir::new("corrupt-config");
    let store = directory.store();

    let broken = "{ \"window\": { \"isInitialized\": true, ";
    fs::write(store.config_path(), broken).expect("fixture should be written");

    assert_eq!(
        store
            .load_settings()
            .expect("损坏的配置应当被重建，而不是让应用起不来"),
        Settings::default()
    );

    // 重建后的文件必须是合法 JSON，否则每次启动都要再重建一遍。
    let rebuilt: Settings = serde_json::from_str(
        &fs::read_to_string(store.config_path()).expect("重建后的配置应当可读"),
    )
    .expect("重建后的配置应当是合法 JSON");
    assert_eq!(rebuilt, Settings::default());

    // 原始内容要留一份：用户可能想看看自己到底改错了什么。
    let backups = corrupt_backups(&directory, "config.json.corrupt");
    assert_eq!(backups.len(), 1, "应当恰好留一份备份: {backups:?}");
    assert_eq!(
        fs::read_to_string(&backups[0]).expect("备份应当可读"),
        broken
    );

    // 第二次读不应再产生备份 —— 文件已经合法了。
    store.load_settings().expect("重建后应当可以正常读取");
    assert_eq!(corrupt_backups(&directory, "config.json.corrupt").len(), 1);
}

// 收起/展开走的是 `mutate_settings` 而不是 `load_settings`，所以恢复逻辑必须覆盖
// 「读改写」这条路径。只修 `load_settings` 的话，配置损坏时窗口照样收不起来 ——
// 实测现象是点收起只弹一条失败提示、窗口纹丝不动。
#[test]
fn corrupt_config_does_not_break_settings_mutations() {
    let directory = TestDir::new("corrupt-config-mutate");
    let store = directory.store();

    fs::write(store.config_path(), "not json at all").expect("fixture should be written");

    store
        .mutate_settings(|settings| {
            settings.view_mode = ViewMode::Collapsed;
            Ok(())
        })
        .expect("配置损坏时读改写也应当自愈");

    assert_eq!(
        store
            .load_settings()
            .expect("重建后的配置应当可读")
            .view_mode,
        ViewMode::Collapsed
    );
}

// 数据文件损坏时**不重建**，只复制一份副本并把错误说清楚。
//
// 与配置相反：data.json 是用户自己攒的分类和应用，静默换一份空的会看起来像
// 「数据全没了」。副本用复制而不是改名 —— 原文件必须留在原位，是删掉还是修好
// 由用户决定。
#[test]
fn corrupt_data_is_copied_aside_and_reported_with_the_path() {
    let directory = TestDir::new("corrupt-data");
    let store = directory.store();

    let broken = "[ broken json";
    fs::write(store.data_path(), broken).expect("fixture should be written");

    let error = store.load_data().expect_err("损坏的数据必须报错");
    assert_eq!(error.code, "JsonSerdeError");

    // 错误必须能指导用户行动：说清哪个文件坏了、副本在哪。
    // serde 的原始报错（"expected value at line 1 column 3"）不含文件名，
    // 用户看到之后完全不知道该去修哪个文件。
    let data_path = store.data_path().display().to_string();
    assert!(
        error.message.contains(&data_path),
        "错误信息里应当有原文件路径 {data_path}: {}",
        error.message
    );
    assert!(
        error.message.contains("data.json.corrupt"),
        "错误信息里应当有副本路径: {}",
        error.message
    );

    // 原文件保持不动，副本内容与之一致。
    assert_eq!(
        fs::read_to_string(store.data_path()).expect("原文件应当还在"),
        broken
    );
    let backups = corrupt_backups(&directory, "data.json.corrupt");
    assert_eq!(backups.len(), 1, "应当恰好一份副本: {backups:?}");
    assert_eq!(
        fs::read_to_string(&backups[0]).expect("副本应当可读"),
        broken
    );

    // 反复读不会堆出更多副本：原文件还在，每次启动都会再走一遍这条路径，
    // 所以副本名是固定的、内容覆盖式写入。
    let _ = store.load_data();
    let _ = store.load_data();
    assert_eq!(corrupt_backups(&directory, "data.json.corrupt").len(), 1);
}

// IO 错误绝不能触发「备份重建」：文件读不出来（被占用、权限不足）时文件本身可能
// 是好的，改名或覆盖只会把好数据弄丢。
#[test]
fn io_failures_do_not_trigger_config_recovery() {
    let directory = TestDir::new("config-io-error");
    let store = JsonStore::new(directory.path());
    store.ensure_layout().expect("layout should be created");

    // 把 config.json 换成一个同名目录：`read_to_string` 会以 IO 错误失败，
    // 而不是 JSON 解析错误。
    fs::remove_file(store.config_path()).expect("fixture file should be removable");
    fs::create_dir(store.config_path()).expect("fixture directory should be created");

    let error = store.load_settings().expect_err("读不出来时必须报错");
    assert_ne!(
        error.code, "JsonSerdeError",
        "IO 错误不该被当成内容损坏，否则会去动一个可能完好的文件"
    );
    assert!(store.config_path().is_dir(), "IO 错误时不能动用户的文件");
    assert!(
        corrupt_backups(&directory, "config.json.corrupt").is_empty(),
        "IO 错误不该产生备份"
    );
}

#[test]
fn repository_traits_are_implemented_by_json_store() {
    fn use_data_repository(repository: &impl DataRepository) -> AppResult<AppData> {
        repository.load_data()
    }

    fn use_settings_repository(repository: &impl SettingsRepository) -> AppResult<Settings> {
        repository.load_settings()
    }

    let directory = TestDir::new("traits");
    let store = JsonStore::new(directory.path());
    store.ensure_layout().expect("layout should be created");

    assert_eq!(use_data_repository(&store).unwrap(), AppData::default());
    assert_eq!(
        use_settings_repository(&store).unwrap(),
        Settings::default()
    );
}

#[test]
fn concurrent_process_mutations_do_not_lose_updates() {
    const WORKER_COUNT: usize = 4;
    const ITERATIONS: usize = 20;
    const PATH_ENV: &str = "DESKCRATE_JSON_STORE_WORKER_PATH";

    let directory = TestDir::new("cross-process-lock");
    let store = directory.store();
    let test_binary = env::current_exe().expect("test executable path should be available");

    let mut children = Vec::with_capacity(WORKER_COUNT);
    for _ in 0..WORKER_COUNT {
        let child = Command::new(&test_binary)
            .args(["--exact", "cross_process_mutation_worker", "--nocapture"])
            .env(PATH_ENV, directory.path())
            .env(
                "DESKCRATE_JSON_STORE_WORKER_ITERATIONS",
                ITERATIONS.to_string(),
            )
            .spawn()
            .expect("worker process should start");
        children.push(child);
    }

    for mut child in children {
        let status = child.wait().expect("worker process should finish");
        assert!(status.success(), "worker process failed: {status}");
    }

    let data = store.load_data().expect("data should remain readable");
    assert_eq!(data.items.len(), WORKER_COUNT * ITERATIONS);
    assert_eq!(data.version, AppData::default().version);
}

#[test]
fn cross_process_mutation_worker() {
    const PATH_ENV: &str = "DESKCRATE_JSON_STORE_WORKER_PATH";

    let Some(path) = env::var_os(PATH_ENV) else {
        return;
    };
    let iterations = env::var("DESKCRATE_JSON_STORE_WORKER_ITERATIONS")
        .expect("worker iteration count should be set")
        .parse::<usize>()
        .expect("worker iteration count should be valid");

    let store = JsonStore::new(path);
    for iteration in 0..iterations {
        let id = format!("worker-{}-{iteration}", std::process::id());
        store
            .mutate_data(|data| {
                data.items
                    .push(RootItem::Category(category(&id, "并发写入")));
                Ok::<_, AppError>(())
            })
            .expect("worker mutation should succeed");
    }
}
