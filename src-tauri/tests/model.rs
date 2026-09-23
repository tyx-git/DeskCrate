use app_lib::models::{
    CssSize, Settings, Theme, ViewMode, WindowPosition, WindowSettings, WindowSize,
};
use serde_json::json;

#[test]
fn settings_default_matches_the_first_launch_policy() {
    let settings = Settings::default();

    assert_eq!(settings.revision, 0);
    assert_eq!(settings.theme, Theme::Default);
    assert_eq!(settings.view_mode, ViewMode::Expanded);
    assert!(!settings.auto_collapse);
    assert!(!settings.auto_start);
    assert!(!settings.window.is_initialized);
    assert_eq!(settings.window.position, None);
    assert_eq!(settings.window.size, None);
    // 首次启动没有用户拖拽出来的尺寸，展开态按设计稿推导。
    assert_eq!(settings.window.size_override, None);
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
            size_override: Some(CssSize {
                width: 512,
                height: 700,
            }),
        },
        view_mode: ViewMode::Collapsed,
        auto_collapse: true,
        ..Settings::default()
    };

    let json = serde_json::to_value(settings).expect("settings should serialize");

    assert_eq!(json["theme"], "default");
    assert_eq!(json["revision"], 0);
    assert_eq!(json["viewMode"], "collapsed");
    assert_eq!(json["autoCollapse"], true);
    assert_eq!(json["window"]["isInitialized"], true);
    assert_eq!(json["window"]["position"]["x"], -120);
    assert_eq!(json["window"]["size"]["width"], 640);
    assert_eq!(json["window"]["sizeOverride"]["width"], 512);
    assert_eq!(json["window"]["sizeOverride"]["height"], 700);
}

// 旧版本的 `config.json` 里没有 `sizeOverride` 字段，升级后必须还能读出来，
// 且被当作「没有用户尺寸覆盖」，否则用户升级一次就会启动失败。
#[test]
fn settings_without_the_size_override_field_still_deserialize() {
    let settings: Settings = serde_json::from_value(json!({
        "window": {
            "isInitialized": true,
            "position": { "x": 0, "y": 167 },
            "size": { "width": 507, "height": 812 }
        },
        "viewMode": "expanded"
    }))
    .expect("旧版本配置应当可以读出");

    assert_eq!(settings.window.size_override, None);
    assert_eq!(settings.window.size.map(|size| size.width), Some(507));
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
