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
    error::AppResult,
    models::{
        AddAppRequest, AddAppsRequest, AddAppsResponse, AppEXE, Settings, WindowPosition,
        WindowSize,
    },
    services,
    state::AppState,
    storage::SettingsRepository,
};

use support::TestDir;

/**
 * `add_app` 的测试替身。
 *
 * 真正的 `add_app` 还要一个 `AppHandle` —— 用来在后台补抓完网址图标之后发事件。
 * 但这套测试跑在 `MockRuntime` 上，而 `AppHandle` 在那里不是合法的 `CommandArg`，
 * 注册不进去。
 *
 * 所以这里注册一个**同签名**的替身，只做前半段（落库）。参数名、camelCase 转换
 * 和返回值都与真实命令一致，因此这个测试该验的 IPC 形状仍然验得到；后台补图标
 * 那条路径由 `services_catalog` 的用例覆盖。
 */
#[tauri::command]
fn add_app(
    parent_id: Option<String>,
    source_path: String,
    name: Option<String>,
    root_index: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> AppResult<AppEXE> {
    services::catalog::add_app(
        state.inner(),
        AddAppRequest {
            parent_id,
            source_path,
            name,
            root_index,
        },
    )
}

// `add_apps` 的 MockRuntime 替身不接收 AppHandle；批量落库和失败明细由
// service 层覆盖，生产 Command 额外负责网址图标事件调度。
#[tauri::command]
fn add_apps(
    parent_id: Option<String>,
    source_paths: Vec<String>,
    root_index: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> AppResult<AddAppsResponse> {
    services::catalog::add_apps(
        state.inner(),
        AddAppsRequest {
            parent_id,
            source_paths,
            root_index,
        },
    )
}

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
            commands::catalog::set_node_collapsed,
            add_app,
            add_apps,
            commands::catalog::rename_app,
            commands::catalog::delete_app,
            commands::settings::get_settings,
            commands::settings::update_auto_collapse,
            commands::settings::update_auto_start,
            commands::settings::update_theme,
            commands::settings::clear_window_size_override,
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
    // 根节点是有序的 `items` 数组：分类与顶层应用混排。
    assert_eq!(all_data["items"][0]["Category"]["id"], category_id);

    let group = invoke(
        &window,
        "add_group",
        json!({"parentId": category_id, "name": "编辑器"}),
    );
    let group_id = group["id"].as_str().unwrap().to_owned();

    invoke(
        &window,
        "set_node_collapsed",
        json!({"id": category_id, "isCollapsed": true}),
    );
    let collapsed = invoke(&window, "get_all_data", json!({}));
    assert_eq!(collapsed["items"][0]["Category"]["isCollapsed"], true);

    invoke(
        &window,
        "set_node_collapsed",
        json!({"id": group_id, "isCollapsed": true}),
    );
    let collapsed = invoke(&window, "get_all_data", json!({}));
    assert_eq!(
        collapsed["items"][0]["Category"]["groups"][0]["Group"]["isCollapsed"],
        true
    );

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
    let updated_response = invoke(
        &window,
        "update_settings",
        json!({"settings": updated_json}),
    );
    assert_eq!(updated_response["autoCollapse"], true);
    assert_eq!(updated_response["revision"], 1);

    invoke(&window, "delete_category", json!({"id": category_id}));
    assert!(invoke(&window, "get_all_data", json!({}))["categories"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn field_level_settings_commands_preserve_unrelated_configuration() {
    let directory = TestDir::new("api-field-settings");
    let store = directory.store();
    let mut initial = Settings::default();
    initial.window.is_initialized = true;
    initial.window.position = Some(WindowPosition { x: -300, y: 120 });
    initial.window.size = Some(WindowSize {
        width: 640,
        height: 480,
    });
    initial.window.size_override = Some(app_lib::models::CssSize {
        width: 420,
        height: 600,
    });
    store
        .save_settings(&initial)
        .expect("settings fixture should save");

    let app = create_test_app(&directory);
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock window should build");

    let updated = invoke(
        &window,
        "update_auto_collapse",
        json!({"request": {"enabled": true}}),
    );
    assert_eq!(updated["autoCollapse"], true);
    assert_eq!(updated["window"]["position"]["x"], -300);
    assert_eq!(updated["window"]["sizeOverride"]["width"], 420);

    let updated = invoke(
        &window,
        "update_theme",
        json!({"request": {"theme": "default"}}),
    );
    assert_eq!(updated["theme"], "default");
    assert_eq!(updated["autoCollapse"], true);
    assert_eq!(updated["window"]["size"]["height"], 480);

    let cleared = invoke(&window, "clear_window_size_override", json!({}));
    assert!(cleared["window"]["sizeOverride"].is_null());
    assert_eq!(cleared["window"]["position"]["y"], 120);
    assert_eq!(cleared["autoCollapse"], true);

    let stale = get_ipc_response(
        &window,
        request(
            "update_settings",
            json!({"settings": serde_json::to_value(initial).unwrap()}),
        ),
    )
    .expect_err("a stale whole-settings snapshot should be rejected");
    assert_eq!(stale["code"], "settings_revision_conflict");
}

#[test]
fn batch_add_command_returns_successes_and_failures() {
    let directory = TestDir::new("api-batch-add");
    let app = create_test_app(&directory);
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock window should build");

    let result = invoke(
        &window,
        "add_apps",
        json!({
            "parentId": null,
            "sourcePaths": ["https://example.com", "", "https://rust-lang.org"]
        }),
    );

    assert_eq!(result["added"].as_array().unwrap().len(), 2);
    assert_eq!(result["failed"].as_array().unwrap().len(), 1);
    assert_eq!(result["failed"][0]["code"], "invalid_source_path");

    let data = invoke(&window, "get_all_data", json!({}));
    assert_eq!(data["items"][0]["Apps"].as_array().unwrap().len(), 2);
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
