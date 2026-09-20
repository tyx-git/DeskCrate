mod support;

use std::fs;

use app_lib::{
    models::{
        AddAppRequest, AddCategoryRequest, AddGroupRequest, DeleteRequest, IconType, Item,
        RenameRequest,
    },
    services::catalog,
    storage::{DataRepository, JsonStore},
};

use support::{count_apps, find_app, find_category, find_group, TestDir};

#[test]
fn category_crud_uses_generated_uuid_and_persists_changes() {
    let directory = TestDir::new("category-crud");
    let state = directory.state();

    let first = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "  开发工具  ".to_owned(),
        },
    )
    .expect("category should be added");
    let second = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "媒体".to_owned(),
        },
    )
    .expect("second category should be added");

    assert_ne!(first.id, second.id);
    assert_eq!(
        uuid::Uuid::parse_str(&first.id).unwrap().get_version_num(),
        4
    );
    assert_eq!(first.name, "开发工具");
    assert!(!first.is_collapsed);
    assert!(first.groups.is_empty());

    let renamed = catalog::rename_category(
        &state,
        RenameRequest {
            id: first.id.clone(),
            name: "工具".to_owned(),
        },
    )
    .expect("category should be renamed");
    assert_eq!(renamed.name, "工具");

    catalog::delete_category(
        &state,
        DeleteRequest {
            id: second.id.clone(),
        },
    )
    .expect("category should be deleted");

    let data = catalog::get_all_data(&state).expect("data should load");
    assert_eq!(data.categories.len(), 1);
    assert_eq!(data.categories[0].id, first.id);
    assert_eq!(data.categories[0].name, "工具");
}

#[test]
fn category_and_node_name_validation_does_not_change_data() {
    let directory = TestDir::new("name-validation");
    let state = directory.state();

    let error = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: " \n\t ".to_owned(),
        },
    )
    .expect_err("blank category name should fail");
    assert_eq!(error.code, "invalid_category_name");

    let error = catalog::add_group(
        &state,
        AddGroupRequest {
            parent_id: "missing".to_owned(),
            name: "分组".to_owned(),
        },
    )
    .expect_err("missing parent should fail");
    assert_eq!(error.code, "parent_not_found");

    assert!(catalog::get_all_data(&state).unwrap().categories.is_empty());
}

#[test]
fn group_crud_supports_nesting_and_enforces_the_maximum_depth() {
    let directory = TestDir::new("group-crud");
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "分类".to_owned(),
        },
    )
    .unwrap();

    let group_1 = catalog::add_group(
        &state,
        AddGroupRequest {
            parent_id: category.id.clone(),
            name: "一级".to_owned(),
        },
    )
    .unwrap();
    let group_2 = catalog::add_group(
        &state,
        AddGroupRequest {
            parent_id: group_1.id.clone(),
            name: "二级".to_owned(),
        },
    )
    .unwrap();
    let group_3 = catalog::add_group(
        &state,
        AddGroupRequest {
            parent_id: group_2.id.clone(),
            name: "三级".to_owned(),
        },
    )
    .unwrap();
    let group_4 = catalog::add_group(
        &state,
        AddGroupRequest {
            parent_id: group_3.id.clone(),
            name: "四级".to_owned(),
        },
    )
    .unwrap();

    let error = catalog::add_group(
        &state,
        AddGroupRequest {
            parent_id: group_4.id.clone(),
            name: "五级".to_owned(),
        },
    )
    .expect_err("fifth group should exceed the depth limit");
    assert_eq!(error.code, "group_depth_exceeded");

    let renamed = catalog::rename_group(
        &state,
        RenameRequest {
            id: group_2.id.clone(),
            name: "二级重命名".to_owned(),
        },
    )
    .expect("nested group should be renamed");
    assert_eq!(renamed.name, "二级重命名");

    catalog::delete_group(
        &state,
        DeleteRequest {
            id: group_2.id.clone(),
        },
    )
    .unwrap();
    let data = catalog::get_all_data(&state).unwrap();
    assert!(find_group(&data, &group_2.id).is_none());
    assert!(find_group(&data, &group_3.id).is_none());
    assert!(find_group(&data, &group_4.id).is_none());
}

#[test]
fn app_crud_supports_urls_and_uses_id_lookup() {
    let directory = TestDir::new("app-url");
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "网址".to_owned(),
        },
    )
    .unwrap();

    let app = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: Some(category.id.clone()),
            source_path: "  https://www.rust-lang.org/learn  ".to_owned(),
            name: None,
        },
    )
    .unwrap();

    assert_eq!(app.icon_type, IconType::Url);
    assert_eq!(app.source_path, "https://www.rust-lang.org/learn");
    assert_eq!(app.name, "www.rust-lang.org");
    // 联网时这里会是抓下来的站点图标（asset 协议地址），离线时退回内置的
    // 地球图标 —— 两种都算通过，抓取行为本身由 `tests/url_icon.rs` 覆盖。
    assert!(
        app.icon_url.starts_with("data:image/svg+xml,")
            || app.icon_url.starts_with("http://asset.localhost/"),
        "网址图标要么抓到并缓存，要么退回内置图标，实际为 {}",
        app.icon_url
    );
    assert!(app.arguments.is_none());
    assert!(app.working_dir.is_none());

    let renamed = catalog::rename_app(
        &state,
        RenameRequest {
            id: app.id.clone(),
            name: "Rust 官网".to_owned(),
        },
    )
    .unwrap();
    assert_eq!(renamed.name, "Rust 官网");
    catalog::delete_app(&state, DeleteRequest { id: app.id.clone() }).unwrap();

    let data = catalog::get_all_data(&state).unwrap();
    assert_eq!(count_apps(&data), 0);
    assert_eq!(
        catalog::delete_app(&state, DeleteRequest { id: app.id })
            .unwrap_err()
            .code,
        "app_not_found"
    );
}

#[test]
fn app_crud_classifies_regular_files_and_executables() {
    let directory = TestDir::new("app-file-types");
    let regular_file = directory.child("notes.txt");
    fs::write(&regular_file, "DeskCrate test file").expect("regular file should be written");
    let executable = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .expect("Windows test environment should define WINDIR")
        .join("explorer.exe");
    assert!(
        executable.is_file(),
        "system executable should exist: {executable:?}"
    );
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "文件类型".to_owned(),
        },
    )
    .unwrap();

    let file_app = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: Some(category.id.clone()),
            source_path: regular_file.to_string_lossy().into_owned(),
            name: None,
        },
    )
    .expect("regular file should be accepted");
    assert_eq!(file_app.icon_type, IconType::File);
    assert!(directory
        .child(&format!("icons/{}.png", file_app.id))
        .is_file());

    let executable_app = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: Some(category.id),
            source_path: executable.to_string_lossy().into_owned(),
            name: None,
        },
    )
    .expect("executable should be accepted");
    assert_eq!(executable_app.icon_type, IconType::Executable);
    assert!(directory
        .child(&format!("icons/{}.png", executable_app.id))
        .is_file());
}

#[test]
fn local_folder_app_extracts_an_icon_and_delete_cleans_the_cache() {
    let directory = TestDir::new("app-folder");
    let source_dir = directory.child("图标 folder");
    fs::create_dir_all(&source_dir).expect("source folder should be created");
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "文件夹".to_owned(),
        },
    )
    .unwrap();

    let app = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: Some(category.id),
            source_path: source_dir.to_string_lossy().into_owned(),
            name: Some("我的文件夹".to_owned()),
        },
    )
    .expect("folder app should be added");

    assert_eq!(app.icon_type, IconType::Folder);
    assert!(app.icon_url.starts_with("http://asset.localhost/"));
    assert!(app.icon_url.contains("icons"));
    let icon_path = directory.child(&format!("icons/{}.png", app.id));
    assert!(
        icon_path.is_file(),
        "icon should be cached at {icon_path:?}"
    );

    catalog::delete_app(&state, DeleteRequest { id: app.id }).unwrap();
    assert!(!icon_path.exists(), "deleted app icon should be cleaned");
}

#[test]
fn deleting_a_category_removes_descendants_and_their_icon_cache() {
    let directory = TestDir::new("category-cleanup");
    let source_dir = directory.child("cleanup-folder");
    fs::create_dir_all(&source_dir).expect("source folder should be created");
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "待删除".to_owned(),
        },
    )
    .unwrap();
    let group = catalog::add_group(
        &state,
        AddGroupRequest {
            parent_id: category.id.clone(),
            name: "子分组".to_owned(),
        },
    )
    .unwrap();
    let app = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: Some(group.id),
            source_path: source_dir.to_string_lossy().into_owned(),
            name: None,
        },
    )
    .unwrap();
    let icon_path = directory.child(&format!("icons/{}.png", app.id));
    assert!(icon_path.exists());

    catalog::delete_category(&state, DeleteRequest { id: category.id }).unwrap();
    assert!(!icon_path.exists());
    assert!(catalog::get_all_data(&state).unwrap().categories.is_empty());
}

#[test]
fn orphan_icon_sweep_removes_only_unreferenced_uuid_files() {
    let directory = TestDir::new("icon-sweep");
    let source_dir = directory.child("sweep-folder");
    fs::create_dir_all(&source_dir).expect("source folder should be created");
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "分类".to_owned(),
        },
    )
    .unwrap();
    let app = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: Some(category.id),
            source_path: source_dir.to_string_lossy().into_owned(),
            name: None,
        },
    )
    .unwrap();

    let referenced = directory.child(&format!("icons/{}.png", app.id));
    assert!(referenced.is_file(), "应用图标应已缓存");

    // 伪造三种文件：真正的孤儿、非 UUID 命名、以及非 PNG 扩展名。
    // 后两种不是本程序写的，对账必须放过它们。
    let orphan_name = "11111111-2222-3333-4444-555555555555.png";
    let orphan = directory.child(&format!("icons/{orphan_name}"));
    let foreign = directory.child("icons/readme.png");
    let other = directory.child("icons/notes.txt");
    for path in [&orphan, &foreign, &other] {
        fs::write(path, b"fixture").expect("fixture should be written");
    }

    let removed = catalog::sweep_orphan_icons(&state).unwrap();

    assert_eq!(removed, vec![orphan_name.to_owned()]);
    assert!(!orphan.exists(), "未被引用的 UUID 图标应被清理");
    assert!(referenced.is_file(), "仍被应用引用的图标不能删");
    assert!(foreign.exists(), "非 UUID 命名的文件不能删");
    assert!(other.exists(), "非 PNG 文件不能删");
}

#[test]
fn invalid_app_source_and_wrong_node_kinds_are_rejected() {
    let directory = TestDir::new("app-validation");
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "分类".to_owned(),
        },
    )
    .unwrap();

    let error = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: Some(category.id.clone()),
            source_path: " \0 ".to_owned(),
            name: None,
        },
    )
    .expect_err("NUL source should fail");
    assert_eq!(error.code, "invalid_source_path");

    let error = catalog::delete_group(
        &state,
        DeleteRequest {
            id: category.id.clone(),
        },
    )
    .unwrap_err();
    assert_eq!(error.code, "group_not_found");
    let error = catalog::delete_category(
        &state,
        DeleteRequest {
            id: "missing".to_owned(),
        },
    )
    .unwrap_err();
    assert_eq!(error.code, "category_not_found");

    let data = catalog::get_all_data(&state).unwrap();
    assert_eq!(data.categories.len(), 1);
    assert!(data.categories[0].groups.is_empty());
    assert!(data.categories[0]
        .groups
        .iter()
        .all(|item| matches!(item, Item::Group(_) | Item::App(_))));
}

#[test]
fn missing_local_app_source_is_rejected_even_when_a_fallback_icon_exists() {
    let directory = TestDir::new("missing-app-source");
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "分类".to_owned(),
        },
    )
    .unwrap();

    let error = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: Some(category.id),
            source_path: directory
                .child("missing.txt")
                .to_string_lossy()
                .into_owned(),
            name: None,
        },
    )
    .expect_err("missing local source should be rejected");

    assert_eq!(error.code, "source_not_found");
    assert_eq!(
        catalog::get_all_data(&state).unwrap().categories[0]
            .groups
            .len(),
        0
    );
}

#[test]
fn url_app_without_a_reachable_site_falls_back_to_the_builtin_icon() {
    let directory = TestDir::new("url-icon-fallback");
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "网址".to_owned(),
        },
    )
    .unwrap();

    let app = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: Some(category.id),
            // 端口 1 上没有服务，连接会立刻被拒：抓取必然失败，又不会像解析
            // 不存在的域名那样把测试套件拖慢几秒。
            source_path: "http://127.0.0.1:1/".to_owned(),
            name: None,
        },
    )
    .expect("抓不到图标不应该让「添加网址」这件事失败");

    assert_eq!(app.icon_type, IconType::Url);
    assert!(
        app.icon_url.starts_with("data:image/svg+xml,"),
        "抓不到时应退回内置图标，实际为 {}",
        app.icon_url
    );
    // 没抓到就没有缓存文件，孤儿对账也不会凭空多出一个 PNG。
    assert!(!directory.child(&format!("icons/{}.png", app.id)).exists());
}

#[test]
fn root_level_apps_can_be_added_renamed_and_deleted() {
    let directory = TestDir::new("app-root-level");
    let source_dir = directory.child("root-folder");
    fs::create_dir_all(&source_dir).expect("source folder should be created");
    let state = directory.state();

    // 顶层没有父节点：`parent_id` 传 `None`，不进任何分类。
    let app = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: None,
            source_path: source_dir.to_string_lossy().into_owned(),
            name: Some("顶层应用".to_owned()),
        },
    )
    .expect("root level app should be added");

    let data = catalog::get_all_data(&state).unwrap();
    assert_eq!(data.apps.len(), 1, "顶层应用应落在根节点的 apps 上");
    assert_eq!(data.apps[0].id, app.id);
    assert!(data.categories.is_empty(), "不应该凭空多出一个分类");

    // 顶层应用必须能被后续所有按 ID 的操作找到：重命名、启动、删除。
    let renamed = catalog::rename_app(
        &state,
        RenameRequest {
            id: app.id.clone(),
            name: "改名后的顶层应用".to_owned(),
        },
    )
    .expect("root level app should be renamed");
    assert_eq!(renamed.name, "改名后的顶层应用");

    let icon_path = directory.child(&format!("icons/{}.png", app.id));
    assert!(icon_path.is_file(), "顶层应用的图标同样要缓存");

    catalog::delete_app(&state, DeleteRequest { id: app.id.clone() }).unwrap();
    let data = catalog::get_all_data(&state).unwrap();
    assert!(data.apps.is_empty(), "顶层应用必须能删掉");
    assert!(!icon_path.exists(), "删除顶层应用也要清理图标缓存");
}

#[test]
fn root_level_app_icons_are_not_treated_as_orphans() {
    let directory = TestDir::new("root-icon-sweep");
    let source_dir = directory.child("root-sweep");
    fs::create_dir_all(&source_dir).expect("source folder should be created");
    let state = directory.state();

    let app = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: None,
            source_path: source_dir.to_string_lossy().into_owned(),
            name: None,
        },
    )
    .unwrap();

    let removed = catalog::sweep_orphan_icons(&state).unwrap();
    assert!(
        removed.is_empty(),
        "顶层应用仍然引用着自己的图标，不能被当成孤儿删掉：{removed:?}"
    );
    assert!(directory.child(&format!("icons/{}.png", app.id)).is_file());
}

#[test]
fn legacy_data_without_the_root_apps_field_still_loads() {
    let directory = TestDir::new("legacy-no-apps");
    let store = directory.store();

    // 老版本 `data.json` 里没有 `apps` 字段。带 `#[serde(default)]` 之后
    // 这类文件必须照常能读，而不是解析失败。
    let legacy = r#"{"version":1,"categories":[]}"#;
    fs::write(directory.child("config/data.json"), legacy).expect("legacy file should be written");

    let data = store.load_data().expect("legacy data should load");
    assert!(data.apps.is_empty());
    assert!(data.categories.is_empty());
}

#[test]
fn persisted_data_can_be_loaded_by_a_new_store_instance() {
    let directory = TestDir::new("reopen");
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "持久化".to_owned(),
        },
    )
    .unwrap();

    let reopened = JsonStore::new(directory.path());
    let data = reopened.load_data().expect("new store should load data");
    assert_eq!(find_category(&data, &category.id).unwrap().name, "持久化");
    assert!(find_app(&data, "missing").is_none());
}
