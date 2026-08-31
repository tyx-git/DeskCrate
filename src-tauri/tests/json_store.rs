mod support;

use std::fs;

use app_lib::{
    error::AppResult,
    models::{AppData, AppError, Category, Settings},
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
    assert_eq!(store.config_path(), directory.child("config/config.json"));
    assert_eq!(store.data_path(), directory.child("config/data.json"));
    assert!(directory.child("config").is_dir());
    assert!(directory.child("icons").is_dir());
    assert!(store.config_path().is_file());
    assert!(store.data_path().is_file());
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
        version: 1,
        categories: vec![category("category-1", "工具")],
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
    assert!(raw.contains("\"version\": 1"));
    assert!(raw.contains("\"name\": \"工具\""));
}

#[test]
fn mutate_data_and_settings_return_operation_values_and_persist_changes() {
    let directory = TestDir::new("mutate");
    let store = directory.store();

    let added_name = store
        .mutate_data(|data| {
            data.categories.push(category("category-1", "开发"));
            Ok::<_, AppError>(data.categories[0].name.clone())
        })
        .expect("data mutation should succeed");

    let setting_value = store
        .mutate_settings(|settings| {
            settings.auto_collapse = true;
            Ok::<_, AppError>(settings.auto_collapse)
        })
        .expect("settings mutation should succeed");

    assert_eq!(added_name, "开发");
    assert!(setting_value);
    assert_eq!(store.load_data().unwrap().categories.len(), 1);
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
