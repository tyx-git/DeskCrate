use app_lib::models::{Settings, Theme, ViewMode, WindowPosition, WindowSettings, WindowSize};
use serde_json::json;

#[test]
fn settings_default_matches_the_first_launch_policy() {
    let settings = Settings::default();

    assert_eq!(settings.theme, Theme::Default);
    assert_eq!(settings.view_mode, ViewMode::Expanded);
    assert!(!settings.auto_collapse);
    assert!(!settings.window.is_initialized);
    assert_eq!(settings.window.position, None);
    assert_eq!(settings.window.size, None);
}

#[test]
fn settings_serialize_using_camel_case_and_lowercase_enum_values() {
    let settings = Settings {
        window: WindowSettings {
            is_initialized: true,
            position: Some(WindowPosition { x: -120, y: 80 }),
            size: Some(WindowSize {
                width: 640,
                height: 480,
            }),
        },
        view_mode: ViewMode::Collapsed,
        auto_collapse: true,
        ..Settings::default()
    };

    let json = serde_json::to_value(settings).expect("settings should serialize");

    assert_eq!(json["theme"], "default");
    assert_eq!(json["viewMode"], "collapsed");
    assert_eq!(json["autoCollapse"], true);
    assert_eq!(json["window"]["isInitialized"], true);
    assert_eq!(json["window"]["position"]["x"], -120);
    assert_eq!(json["window"]["size"]["width"], 640);
}

#[test]
fn omitted_nested_settings_use_defaults() {
    let settings: Settings = serde_json::from_value(json!({
        "viewMode": "collapsed"
    }))
    .expect("partial settings should deserialize");

    assert_eq!(settings.view_mode, ViewMode::Collapsed);
    assert_eq!(settings.theme, Theme::Default);
    assert_eq!(settings.window, WindowSettings::default());
    assert!(!settings.auto_collapse);
}
