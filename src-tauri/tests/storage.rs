mod support;

use std::fs;

use app_lib::{
    models::{Settings, WindowPosition, WindowSize},
    storage::SettingsRepository,
};

use support::TestDir;

#[test]
fn data_and_config_files_are_separate_persistence_channels() {
    let directory = TestDir::new("separate-files");
    let store = directory.store();

    let settings = Settings {
        auto_collapse: true,
        ..Settings::default()
    };
    store.save_settings(&settings).unwrap();

    let data_raw = fs::read_to_string(store.data_path()).unwrap();
    let config_raw = fs::read_to_string(store.config_path()).unwrap();
    assert!(data_raw.contains("categories"));
    assert!(!data_raw.contains("autoCollapse"));
    assert!(config_raw.contains("autoCollapse"));
    assert!(!config_raw.contains("categories"));
}

#[test]
fn config_round_trip_keeps_negative_multi_monitor_coordinates() {
    let directory = TestDir::new("negative-position");
    let store = directory.store();
    let mut settings = Settings::default();
    settings.window.is_initialized = true;
    settings.window.position = Some(WindowPosition { x: -1920, y: -120 });
    settings.window.size = Some(WindowSize {
        width: 640,
        height: 480,
    });

    store.save_settings(&settings).unwrap();
    let loaded = store.load_settings().unwrap();
    assert_eq!(loaded, settings);
}

#[test]
fn settings_service_rejects_invalid_window_state_without_overwriting_config() {
    let directory = TestDir::new("invalid-settings");
    let state = directory.state();
    let original = app_lib::services::settings::get_settings(&state).unwrap();

    let mut invalid_size = original.clone();
    invalid_size.window.size = Some(WindowSize {
        width: 0,
        height: 480,
    });
    let error = app_lib::services::settings::update_settings(&state, invalid_size)
        .expect_err("zero window width should be rejected");
    assert_eq!(error.code, "invalid_window_size");

    let mut invalid_state = original.clone();
    invalid_state.window.is_initialized = true;
    invalid_state.window.size = Some(WindowSize {
        width: 640,
        height: 480,
    });
    let error = app_lib::services::settings::update_settings(&state, invalid_state)
        .expect_err("initialized settings without position should be rejected");
    assert_eq!(error.code, "invalid_window_state");

    assert_eq!(
        app_lib::services::settings::get_settings(&state).unwrap(),
        original
    );
}
