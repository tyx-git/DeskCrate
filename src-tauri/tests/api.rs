mod support;

use serde_json::{json, Value};
use tauri::{
    ipc::{CallbackFn, InvokeBody},
    test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY},
    webview::InvokeRequest,
    WebviewWindowBuilder,
};

use app_lib::{
    commands,
    models::{Settings, WindowPosition, WindowSize},
    storage::SettingsRepository,
};

use support::TestDir;

fn request(command: &str, body: Value) -> InvokeRequest {
    InvokeRequest {
        cmd: command.to_owned(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "http://tauri.localhost"
            .parse()
            .expect("test URL should parse"),
        body: InvokeBody::Json(body),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_owned(),
    }
}

fn create_test_app(directory: &TestDir) -> tauri::App<tauri::test::MockRuntime> {
    let state = directory.state();

    mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::catalog::get_all_data,
            commands::catalog::add_category,
            commands::catalog::rename_category,
            commands::catalog::delete_category,
            commands::catalog::add_group,
            commands::catalog::rename_group,
            commands::catalog::delete_group,
            commands::catalog::add_app,
            commands::catalog::rename_app,
            commands::catalog::delete_app,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::system::launch_icon,
            commands::window::is_init,
            commands::window::apply_view_mode
        ])
        .build(mock_context(noop_assets()))
        .expect("mock Tauri app should build")
}

fn invoke(
    window: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    command: &str,
    body: Value,
) -> Value {
    get_ipc_response(window, request(command, body))
        .expect("command should return success")
        .deserialize::<Value>()
        .expect("command response should be JSON")
}

#[test]
fn catalog_and_settings_commands_use_the_documented_ipc_shape() {
    let directory = TestDir::new("api");
    let app = create_test_app(&directory);
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock window should build");

    let category = invoke(&window, "add_category", json!({"name": "工具"}));
    let category_id = category["id"].as_str().unwrap().to_owned();
    assert_eq!(category["name"], "工具");
    assert_eq!(category["isCollapsed"], false);
    assert!(category["groups"].as_array().unwrap().is_empty());

    let all_data = invoke(&window, "get_all_data", json!({}));
    assert_eq!(all_data["categories"][0]["id"], category_id);

    let group = invoke(
        &window,
        "add_group",
        json!({"parentId": category_id, "name": "编辑器"}),
    );
    let group_id = group["id"].as_str().unwrap().to_owned();

    let renamed_category = invoke(
        &window,
        "rename_category",
        json!({"id": category_id, "name": "开发工具"}),
    );
    assert_eq!(renamed_category["name"], "开发工具");

    let app_node = invoke(
        &window,
        "add_app",
        json!({
            "parentId": group_id,
            "sourcePath": "https://www.rust-lang.org",
            "name": "Rust"
        }),
    );
    let app_id = app_node["id"].as_str().unwrap().to_owned();
    assert_eq!(app_node["iconType"], "Url");
    assert_eq!(app_node["sourcePath"], "https://www.rust-lang.org");

    let renamed_app = invoke(
        &window,
        "rename_app",
        json!({"id": app_id, "name": "Rust 官网"}),
    );
    assert_eq!(renamed_app["name"], "Rust 官网");
    invoke(&window, "delete_app", json!({"id": app_id}));
    invoke(
        &window,
        "rename_group",
        json!({"id": group_id, "name": "工具组"}),
    );
    invoke(&window, "delete_group", json!({"id": group_id}));

    let settings = invoke(&window, "get_settings", json!({}));
    let expected: Value = serde_json::to_value(Settings::default()).unwrap();
    assert_eq!(settings, expected);

    let updated = Settings {
        auto_collapse: true,
        ..Settings::default()
    };
    let updated_json = serde_json::to_value(&updated).unwrap();
    assert_eq!(
        invoke(
            &window,
            "update_settings",
            json!({"settings": updated_json})
        ),
        serde_json::to_value(updated).unwrap()
    );

    invoke(&window, "delete_category", json!({"id": category_id}));
    assert!(invoke(&window, "get_all_data", json!({}))["categories"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn commands_return_frontend_safe_errors_for_invalid_input() {
    let directory = TestDir::new("api-errors");
    let app = create_test_app(&directory);
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock window should build");

    let error = get_ipc_response(&window, request("add_category", json!({"name": "   "})))
        .expect_err("invalid category should be rejected");
    assert_eq!(error["code"], "invalid_category_name");
    assert!(error["message"].is_string());

    let error = get_ipc_response(
        &window,
        request("delete_category", json!({"id": "missing"})),
    )
    .expect_err("missing category should be rejected");
    assert_eq!(error["code"], "category_not_found");
    assert!(error["message"].is_string());

    let error = get_ipc_response(&window, request("launch_icon", json!({"id": "missing"})))
        .expect_err("missing app should be rejected");
    assert_eq!(error["code"], "app_not_found");
}

#[test]
fn window_command_restores_and_persists_window_state() {
    let directory = TestDir::new("api-window");
    let store = directory.store();
    let mut settings = Settings::default();
    settings.window.is_initialized = true;
    settings.window.position = Some(WindowPosition { x: -240, y: 120 });
    settings.window.size = Some(WindowSize {
        width: 640,
        height: 480,
    });
    store
        .save_settings(&settings)
        .expect("window fixture settings should save");

    let app = create_test_app(&directory);
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock window should build");

    let restored = invoke(&window, "is_init", json!({}));
    assert_eq!(restored["isInitialized"], true);
    assert_eq!(restored["position"], json!({"x": -240, "y": 120}));
    assert_eq!(restored["size"], json!({"width": 640, "height": 480}));

    let state = directory.state();
    app_lib::services::window::persist_window_state(&state, &window)
        .expect("mock window state should persist");

    let persisted = directory
        .store()
        .load_settings()
        .expect("persisted window settings should load");
    assert!(persisted.window.is_initialized);
    assert_eq!(
        persisted.window.position,
        Some(WindowPosition { x: 0, y: 0 })
    );
    assert_eq!(
        persisted.window.size,
        Some(WindowSize {
            width: 1,
            height: 1,
        })
    );
}
