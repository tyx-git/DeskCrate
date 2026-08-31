use app_lib::models::{
    AddAppRequest, AddCategoryRequest, AddGroupRequest, AppData, AppEXE, AppError, Category,
    DeleteRequest, Group, IconType, Item, RenameRequest,
};
use serde_json::json;

fn sample_app() -> AppEXE {
    AppEXE {
        id: "app-1".to_owned(),
        name: "RustRover".to_owned(),
        source_path: r"C:\Tools\RustRover\RustRover.exe".to_owned(),
        arguments: None,
        working_dir: None,
        icon_url: "http://asset.localhost/icon.png".to_owned(),
        icon_type: IconType::Executable,
        created_time: "2026-08-31T00:00:00Z".to_owned(),
    }
}

#[test]
fn catalog_serializes_with_frontend_names_and_without_legacy_fields() {
    let data = AppData {
        version: 1,
        categories: vec![Category {
            id: "category-1".to_owned(),
            name: "开发工具".to_owned(),
            is_collapsed: false,
            groups: vec![Item::Group(Group {
                id: "group-1".to_owned(),
                name: "编辑器".to_owned(),
                is_collapsed: true,
                children: vec![Item::App(sample_app())],
                created_time: "2026-08-31T00:00:00Z".to_owned(),
            })],
            created_time: "2026-08-31T00:00:00Z".to_owned(),
        }],
    };

    let json = serde_json::to_value(&data).expect("catalog should serialize");
    let category = &json["categories"][0];
    let group = &category["groups"][0]["Group"];
    let app = &group["children"][0]["App"];

    assert_eq!(app["sourcePath"], r"C:\Tools\RustRover\RustRover.exe");
    assert_eq!(app["iconUrl"], "http://asset.localhost/icon.png");
    assert_eq!(app["iconType"], "Executable");
    assert!(app.get("targetPath").is_none());
    assert!(app.get("iconPath").is_none());
    assert!(category.get("icon").is_none());
    assert!(group.get("icon").is_none());
    assert_eq!(category["isCollapsed"], false);
    assert_eq!(group["isCollapsed"], true);
    assert!(app.get("arguments").is_none());
    assert!(app.get("workingDir").is_none());
}

#[test]
fn all_icon_types_use_the_persisted_pascal_case_values() {
    let values = [
        (IconType::Shortcut, "Shortcut"),
        (IconType::Executable, "Executable"),
        (IconType::Folder, "Folder"),
        (IconType::File, "File"),
        (IconType::Url, "Url"),
    ];

    for (icon_type, expected) in values {
        assert_eq!(
            serde_json::to_string(&icon_type).unwrap(),
            format!("\"{expected}\"")
        );
        let decoded: IconType = serde_json::from_str(&format!("\"{expected}\"")).unwrap();
        assert_eq!(decoded, icon_type);
    }
}

#[test]
fn request_models_use_camel_case_input_names() {
    let category: AddCategoryRequest = serde_json::from_value(json!({"name": "工具"})).unwrap();
    let group: AddGroupRequest =
        serde_json::from_value(json!({"parentId": "category-1", "name": "编辑器"})).unwrap();
    let app: AddAppRequest = serde_json::from_value(json!({
        "parentId": "group-1",
        "sourcePath": "https://www.rust-lang.org",
        "name": "Rust"
    }))
    .unwrap();
    let rename: RenameRequest =
        serde_json::from_value(json!({"id": "node-1", "name": "新名称"})).unwrap();
    let delete: DeleteRequest = serde_json::from_value(json!({"id": "node-1"})).unwrap();

    assert_eq!(category.name, "工具");
    assert_eq!(group.parent_id, "category-1");
    assert_eq!(app.source_path, "https://www.rust-lang.org");
    assert_eq!(app.name.as_deref(), Some("Rust"));
    assert_eq!(rename.id, "node-1");
    assert_eq!(delete.id, "node-1");
}

#[test]
fn optional_app_fields_are_omitted_when_they_are_none() {
    let json = serde_json::to_value(sample_app()).expect("app should serialize");

    assert!(json.get("arguments").is_none());
    assert!(json.get("workingDir").is_none());
}

#[test]
fn default_data_has_schema_version_one_and_no_categories() {
    let data = AppData::default();

    assert_eq!(data.version, 1);
    assert!(data.categories.is_empty());
}

#[test]
fn app_error_is_a_frontend_safe_serializable_object() {
    let error = AppError::new("app_not_found", "找不到应用");
    let json = serde_json::to_value(error).expect("error should serialize");

    assert_eq!(
        json,
        json!({"code": "app_not_found", "message": "找不到应用"})
    );
}
