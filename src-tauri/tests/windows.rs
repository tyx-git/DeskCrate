mod support;

use std::fs;

use app_lib::{
    models::{AppData, AppEXE, Category, IconType, Item},
    services::{self, system},
    storage::DataRepository,
};

use support::TestDir;

fn app_record(source_path: &str, icon_type: IconType) -> AppEXE {
    AppEXE {
        id: "app-1".to_owned(),
        name: "测试应用".to_owned(),
        source_path: source_path.to_owned(),
        arguments: None,
        working_dir: None,
        icon_url: "data:image/svg+xml,test".to_owned(),
        icon_type,
        created_time: "2026-08-31T00:00:00Z".to_owned(),
    }
}

fn state_with_app(directory: &TestDir, app: AppEXE) -> app_lib::state::AppState {
    let store = directory.store();
    let data = AppData {
        version: 1,
        categories: vec![Category {
            id: "category-1".to_owned(),
            name: "系统测试".to_owned(),
            is_collapsed: false,
            groups: vec![Item::App(app)],
            created_time: "2026-08-31T00:00:00Z".to_owned(),
        }],
    };
    store.save_data(&data).expect("fixture data should save");
    app_lib::state::AppState::new(store)
}

#[test]
fn missing_app_id_is_rejected_before_any_system_operation() {
    let directory = TestDir::new("system-missing");
    let state = directory.state();

    let error = system::launch_app(&state, "missing").expect_err("missing ID should fail");
    assert_eq!(error.code, "app_not_found");
}

#[test]
fn scripts_are_saved_as_files_but_blocked_by_the_launch_service() {
    let directory = TestDir::new("system-script");
    let script = directory.child("danger.ps1");
    fs::write(&script, "Write-Output blocked").expect("script fixture should be written");
    let state = state_with_app(
        &directory,
        app_record(&script.to_string_lossy(), IconType::File),
    );

    let error = system::launch_app(&state, "app-1").expect_err("scripts must be blocked");
    assert_eq!(error.code, "script_execution_not_allowed");
}

#[test]
fn executor_rejects_empty_and_nul_targets_without_launching_anything() {
    let empty_dir = TestDir::new("system-empty");
    let empty_state = state_with_app(&empty_dir, app_record("", IconType::File));
    let error = system::launch_app(&empty_state, "app-1").expect_err("empty target should fail");
    assert_eq!(error.code, "exec_target_empty");

    let nul_dir = TestDir::new("system-nul");
    let nul_source = "C:\\invalid\0path";
    let nul_state = state_with_app(&nul_dir, app_record(nul_source, IconType::File));
    let error = system::launch_app(&nul_state, "app-1").expect_err("NUL target should fail");
    assert_eq!(error.code, "exec_invalid_target");
}

#[test]
fn malformed_shortcut_is_reported_by_the_shortcut_parser_boundary() {
    let directory = TestDir::new("system-shortcut");
    let shortcut = directory.child("broken.lnk");
    fs::write(&shortcut, b"not a shell link").expect("shortcut fixture should be written");
    let state = state_with_app(
        &directory,
        app_record(&shortcut.to_string_lossy(), IconType::Shortcut),
    );

    let error = system::launch_app(&state, "app-1").expect_err("malformed shortcut should fail");
    assert!(matches!(
        error.code.as_str(),
        "shortcut_load_failed" | "shortcut_target_failed"
    ));
}

#[test]
fn public_window_geometry_helper_centers_on_negative_monitor_coordinates() {
    let (position, size) = services::window::centered_geometry(-1920, -100, 1920, 1080);

    assert_eq!(size.width, 240);
    assert_eq!(size.height, 135);
    assert_eq!(position.x, -1080);
    assert_eq!(position.y, 372);
}

#[test]
fn public_window_geometry_helper_never_returns_zero_dimensions() {
    let (position, size) = services::window::centered_geometry(10, 20, 0, 0);

    assert_eq!(position.x, 10);
    assert_eq!(position.y, 20);
    assert_eq!(size.width, 1);
    assert_eq!(size.height, 1);
}
