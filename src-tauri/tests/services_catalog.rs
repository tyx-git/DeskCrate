mod support;

use std::fs;

use app_lib::{
    models::{
        AddAppRequest, AddCategoryRequest, AddGroupRequest, AppData, AppEXE, Category,
        DeleteRequest, IconType, Item, MoveAppRequest, MoveGroupRequest, RenameRequest, RootItem,
    },
    services::catalog,
    state::AppState,
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
    let categories: Vec<&Category> = data.root_categories().collect();
    assert_eq!(categories.len(), 1);
    assert_eq!(categories[0].id, first.id);
    assert_eq!(categories[0].name, "工具");
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

    assert!(catalog::get_all_data(&state).unwrap().items.is_empty());
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
            root_index: None,
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
            root_index: None,
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
            root_index: None,
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
            root_index: None,
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
            root_index: None,
        },
    )
    .unwrap();
    let icon_path = directory.child(&format!("icons/{}.png", app.id));
    assert!(icon_path.exists());

    catalog::delete_category(&state, DeleteRequest { id: category.id }).unwrap();
    assert!(!icon_path.exists());
    assert!(catalog::get_all_data(&state).unwrap().items.is_empty());
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
            root_index: None,
        },
    )
    .unwrap();

    let referenced = directory.child(&format!("icons/{}.png", app.id));
    assert!(referenced.is_file(), "应用图标应已缓存");

    // 伪造四种文件：真正的 PNG/SVG 孤儿、非 UUID 命名、以及非图标扩展名。
    // 后两种不是本程序写的，对账必须放过它们。
    let orphan_name = "11111111-2222-3333-4444-555555555555.png";
    let orphan_svg_name = "11111111-2222-3333-4444-666666666666.svg";
    let orphan = directory.child(&format!("icons/{orphan_name}"));
    let orphan_svg = directory.child(&format!("icons/{orphan_svg_name}"));
    let foreign = directory.child("icons/readme.png");
    let other = directory.child("icons/notes.txt");
    for path in [&orphan, &orphan_svg, &foreign, &other] {
        fs::write(path, b"fixture").expect("fixture should be written");
    }

    let removed = catalog::sweep_orphan_icons(&state).unwrap();

    assert_eq!(
        removed,
        vec![orphan_name.to_owned(), orphan_svg_name.to_owned()]
    );
    assert!(!orphan.exists(), "未被引用的 UUID PNG 图标应被清理");
    assert!(!orphan_svg.exists(), "未被引用的 UUID SVG 图标应被清理");
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
            root_index: None,
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
    let categories: Vec<&Category> = data.root_categories().collect();
    assert_eq!(categories.len(), 1);
    assert!(categories[0].groups.is_empty());
    assert!(categories[0]
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
            root_index: None,
        },
    )
    .expect_err("missing local source should be rejected");

    assert_eq!(error.code, "source_not_found");
    assert_eq!(
        catalog::get_all_data(&state)
            .unwrap()
            .root_categories()
            .next()
            .expect("分类应仍然存在")
            .groups
            .len(),
        0
    );
}

#[test]
fn top_level_apps_can_be_inserted_at_any_position_between_categories() {
    let directory = TestDir::new("root-insert-position");
    let state = directory.state();

    let first = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "第一个分类".to_owned(),
        },
    )
    .unwrap();
    let second = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "第二个分类".to_owned(),
        },
    )
    .unwrap();

    let add_at = |name: &str, index: usize| {
        catalog::add_app(
            &state,
            AddAppRequest {
                parent_id: None,
                source_path: format!("https://example.com/{name}"),
                name: Some(name.to_owned()),
                root_index: Some(index),
            },
        )
        .expect("top level app should be added")
    };

    // 插到两个分类之间：期望顺序是 分类 / 应用 / 分类。
    let between = add_at("中间", 1);
    // 插到最上方（下标 0）。
    let top = add_at("最上面", 0);
    // 下标越界时应当被夹到末尾，而不是报错。
    let bottom = add_at("最下面", usize::MAX);

    let data = catalog::get_all_data(&state).unwrap();
    let shape: Vec<String> = data
        .items
        .iter()
        .map(|item| match item {
            RootItem::Apps(apps) => format!(
                "Apps({})",
                apps.iter()
                    .map(|a| a.name.clone())
                    .collect::<Vec<_>>()
                    .join("+")
            ),
            RootItem::Category(category) => format!("Category({})", category.name),
        })
        .collect();

    assert_eq!(
        shape,
        vec![
            format!("Apps({})", top.name),
            format!("Category({})", first.name),
            format!("Apps({})", between.name),
            format!("Category({})", second.name),
            format!("Apps({})", bottom.name),
        ],
        "顶层应用应能与分类交错排列，且顺序与插入位置一致"
    );
}

#[test]
fn a_top_level_row_holds_several_apps_and_empties_itself_when_drained() {
    let directory = TestDir::new("root-row-multi");
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "分类".to_owned(),
        },
    )
    .unwrap();

    let add = |name: &str| {
        catalog::add_app(
            &state,
            AddAppRequest {
                parent_id: None,
                source_path: format!("https://example.com/{name}"),
                name: Some(name.to_owned()),
                // 都插到分类之前：连续的顶层应用应当并进同一行。
                root_index: Some(0),
            },
        )
        .expect("top level app should be added")
    };

    let a = add("甲");
    let b = add("乙");
    let c = add("丙");

    let data = catalog::get_all_data(&state).unwrap();
    assert_eq!(data.items.len(), 2, "三个应用应并成一行，再跟一个分类");
    match &data.items[0] {
        RootItem::Apps(apps) => {
            assert_eq!(apps.len(), 3, "一行里应当能动态存放多个顶层应用");
            // 都插在同一行，按插入先后排列。
            assert_eq!(
                apps.iter().map(|app| app.id.clone()).collect::<Vec<_>>(),
                vec![a.id.clone(), b.id.clone(), c.id.clone()]
            );
        }
        RootItem::Category(_) => panic!("第一个节点应当是一行顶层应用"),
    }
    assert!(matches!(&data.items[1], RootItem::Category(c) if c.id == category.id));

    // 删空这一行时，空节点要一并摘掉，不能留下一条没有内容的分界线。
    for app in [&a, &b, &c] {
        catalog::delete_app(&state, DeleteRequest { id: app.id.clone() }).unwrap();
    }
    let data = catalog::get_all_data(&state).unwrap();
    assert_eq!(data.items.len(), 1, "删空后空行应被移除");
    assert!(matches!(&data.items[0], RootItem::Category(_)));
}

#[test]
fn an_app_can_be_moved_between_the_root_and_a_category() {
    let directory = TestDir::new("move-app");
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "分类".to_owned(),
        },
    )
    .unwrap();

    let make = |name: &str| {
        catalog::add_app(
            &state,
            AddAppRequest {
                parent_id: None,
                source_path: format!("https://example.com/{name}"),
                name: Some(name.to_owned()),
                root_index: None,
            },
        )
        .expect("app should be added")
    };

    let a = make("甲");
    let b = make("乙");

    // 甲 移进分类（分类此时还是空的，用「追加到末尾」）。
    catalog::move_app(
        &state,
        MoveAppRequest {
            id: a.id.clone(),
            parent_id: Some(category.id.clone()),
            before_id: None,
        },
    )
    .expect("move should succeed");

    let data = catalog::get_all_data(&state).unwrap();
    assert_eq!(data.items.len(), 2, "顶层剩一行乙 + 一个分类");
    assert!(data.root_apps().all(|app| app.id != a.id), "甲 已不在顶层");
    let in_category = data
        .root_categories()
        .flat_map(|c| c.groups.iter())
        .filter_map(|item| match item {
            Item::App(app) => Some(app.id.clone()),
            Item::Group(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(in_category, vec![a.id.clone()], "甲 进了分类");

    // 再把它挪回顶层，插到「乙」之前。
    catalog::move_app(
        &state,
        MoveAppRequest {
            id: a.id.clone(),
            parent_id: None,
            before_id: Some(b.id.clone()),
        },
    )
    .expect("move back should succeed");

    let data = catalog::get_all_data(&state).unwrap();
    let top: Vec<String> = data.root_apps().map(|app| app.name.clone()).collect();
    assert_eq!(top, vec!["甲".to_owned(), "乙".to_owned()], "甲 应排到最前");
}

#[test]
fn reordering_inside_one_container_compensates_for_the_removal_shift() {
    let directory = TestDir::new("move-app-shift");
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "分类".to_owned(),
        },
    )
    .unwrap();

    let mut ids = Vec::new();
    for name in ["一", "二", "三"] {
        ids.push(
            catalog::add_app(
                &state,
                AddAppRequest {
                    parent_id: Some(category.id.clone()),
                    source_path: format!("https://example.com/{name}"),
                    name: Some(name.to_owned()),
                    root_index: None,
                },
            )
            .expect("app should be added")
            .id,
        );
    }

    // 把「一」挪到末尾（参照为 null 即追加）。
    catalog::move_app(
        &state,
        MoveAppRequest {
            id: ids[0].clone(),
            parent_id: Some(category.id.clone()),
            before_id: None,
        },
    )
    .expect("move should succeed");

    let names = |data: &app_lib::models::AppData| {
        data.root_categories()
            .flat_map(|c| c.groups.iter())
            .filter_map(|item| match item {
                Item::App(app) => Some(app.name.clone()),
                Item::Group(_) => None,
            })
            .collect::<Vec<_>>()
    };

    let data = catalog::get_all_data(&state).unwrap();
    assert_eq!(
        names(&data),
        vec!["二".to_owned(), "三".to_owned(), "一".to_owned()],
        "同一容器内前移需要修正下标偏移，否则会插偏一位"
    );

    // 再把「三」挪到最前（插到「二」之前）。
    catalog::move_app(
        &state,
        MoveAppRequest {
            id: ids[2].clone(),
            parent_id: Some(category.id.clone()),
            before_id: Some(ids[1].clone()),
        },
    )
    .expect("move should succeed");
    let data = catalog::get_all_data(&state).unwrap();
    assert_eq!(
        names(&data),
        vec!["三".to_owned(), "二".to_owned(), "一".to_owned()]
    );

    let group = catalog::add_group(
        &state,
        AddGroupRequest {
            parent_id: category.id.clone(),
            name: "夹在应用间的分组".to_owned(),
        },
    )
    .unwrap();
    catalog::move_app(
        &state,
        MoveAppRequest {
            id: ids[0].clone(),
            parent_id: Some(category.id.clone()),
            before_id: Some(group.id.clone()),
        },
    )
    .expect("应用可以插到同级分组之前");
    let data = catalog::get_all_data(&state).unwrap();
    let siblings = &find_category(&data, &category.id).unwrap().groups;
    assert!(matches!(&siblings[2], Item::App(app) if app.id == ids[0]));
    assert!(matches!(&siblings[3], Item::Group(candidate) if candidate.id == group.id));
}

#[test]
fn an_app_can_be_reordered_within_one_top_level_row() {
    let directory = TestDir::new("move-app-in-row");
    let state = directory.state();

    let mut ids = Vec::new();
    for name in ["红", "绿", "蓝"] {
        ids.push(
            catalog::add_app(
                &state,
                AddAppRequest {
                    parent_id: None,
                    source_path: format!("https://example.com/{name}"),
                    name: Some(name.to_owned()),
                    // 都插到第 0 行，并成一行。
                    root_index: Some(0),
                },
            )
            .expect("app should be added")
            .id,
        );
    }

    let order = || {
        catalog::get_all_data(&state)
            .unwrap()
            .root_apps()
            .map(|app| app.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(order(), vec!["红", "绿", "蓝"]);

    // 「红」拖到「蓝」前面 —— 必须真的落在两个图标之间，而不是被并入行尾。
    catalog::move_app(
        &state,
        MoveAppRequest {
            id: ids[0].clone(),
            parent_id: None,
            before_id: Some(ids[2].clone()),
        },
    )
    .expect("move should succeed");
    assert_eq!(order(), vec!["绿", "红", "蓝"]);
}

#[test]
fn groups_can_be_reordered_among_mixed_siblings_without_losing_their_subtree() {
    let directory = TestDir::new("move-group-siblings");
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "工具".into(),
        },
    )
    .unwrap();
    let first = catalog::add_group(
        &state,
        AddGroupRequest {
            parent_id: category.id.clone(),
            name: "第一组".into(),
        },
    )
    .unwrap();
    let middle_app = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: Some(category.id.clone()),
            source_path: "https://example.com/app".into(),
            name: Some("中间应用".into()),
            root_index: None,
        },
    )
    .unwrap();
    let second = catalog::add_group(
        &state,
        AddGroupRequest {
            parent_id: category.id.clone(),
            name: "第二组".into(),
        },
    )
    .unwrap();
    let child = catalog::add_group(
        &state,
        AddGroupRequest {
            parent_id: first.id.clone(),
            name: "子分组".into(),
        },
    )
    .unwrap();

    catalog::move_group(
        &state,
        MoveGroupRequest {
            id: second.id.clone(),
            parent_id: category.id.clone(),
            before_id: Some(middle_app.id.clone()),
        },
    )
    .unwrap();

    let data = catalog::get_all_data(&state).unwrap();
    let groups = &find_category(&data, &category.id).unwrap().groups;
    let ids: Vec<&str> = groups
        .iter()
        .map(|item| match item {
            Item::App(app) => app.id.as_str(),
            Item::Group(group) => group.id.as_str(),
        })
        .collect();
    assert_eq!(
        ids,
        vec![
            first.id.as_str(),
            second.id.as_str(),
            middle_app.id.as_str()
        ]
    );
    assert_eq!(
        find_group(&data, &child.id).unwrap().name,
        "子分组",
        "分组子树应整体保留"
    );

    catalog::move_group(
        &state,
        MoveGroupRequest {
            id: first.id.clone(),
            parent_id: category.id.clone(),
            before_id: None,
        },
    )
    .unwrap();
    let data = catalog::get_all_data(&state).unwrap();
    let groups = &find_category(&data, &category.id).unwrap().groups;
    assert!(matches!(&groups[2], Item::Group(group) if group.id == first.id));
    assert!(find_group(&data, &child.id).is_some());
}

#[test]
fn moving_a_group_into_its_descendant_is_rejected_without_persisting() {
    let directory = TestDir::new("move-group-descendant");
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "工具".into(),
        },
    )
    .unwrap();
    let parent = catalog::add_group(
        &state,
        AddGroupRequest {
            parent_id: category.id.clone(),
            name: "父组".into(),
        },
    )
    .unwrap();
    let child = catalog::add_group(
        &state,
        AddGroupRequest {
            parent_id: parent.id.clone(),
            name: "子组".into(),
        },
    )
    .unwrap();

    let error = catalog::move_group(
        &state,
        MoveGroupRequest {
            id: parent.id.clone(),
            parent_id: category.id.clone(),
            before_id: Some(child.id.clone()),
        },
    )
    .expect_err("不能把分组移到其子孙节点之前");
    assert_eq!(error.code, "invalid_group_position");
    let data = catalog::get_all_data(&state).unwrap();
    assert!(find_group(&data, &parent.id).is_some());
    assert!(find_group(&data, &child.id).is_some());
}

#[test]
fn moving_a_group_across_parent_containers_is_rejected() {
    let directory = TestDir::new("move-group-cross-parent");
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "工具".into(),
        },
    )
    .unwrap();
    let other_category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "其他".into(),
        },
    )
    .unwrap();
    let group = catalog::add_group(
        &state,
        AddGroupRequest {
            parent_id: category.id.clone(),
            name: "分组".into(),
        },
    )
    .unwrap();

    let error = catalog::move_group(
        &state,
        MoveGroupRequest {
            id: group.id.clone(),
            parent_id: other_category.id,
            before_id: None,
        },
    )
    .expect_err("本接口仅允许同级排序");
    assert_eq!(error.code, "invalid_group_parent");
    assert_eq!(
        find_group(&catalog::get_all_data(&state).unwrap(), &group.id)
            .unwrap()
            .name,
        "分组"
    );
}

#[test]
fn moving_an_unknown_app_reports_not_found() {
    let directory = TestDir::new("move-app-missing");
    let state = directory.state();

    let error = catalog::move_app(
        &state,
        MoveAppRequest {
            id: "no-such-app".to_owned(),
            parent_id: None,
            before_id: None,
        },
    )
    .expect_err("未知应用应当报错");
    assert_eq!(error.code, "app_not_found");
}

#[test]
fn adding_a_url_app_does_not_wait_for_the_icon() {
    let directory = TestDir::new("add-url-no-wait");
    let state = directory.state();

    // 抓取是几秒量级的网络操作（实测最慢 18 秒），添加网址不该等它。
    // 这里用一个必然抓不到的站点，确保「返回得快」不是因为抓到了什么。
    let started = std::time::Instant::now();
    let app = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: None,
            source_path: "https://icon.invalid.example".to_owned(),
            name: Some("慢站点".to_owned()),
            root_index: None,
        },
    )
    .expect("url app should be added");
    let elapsed = started.elapsed();

    assert_eq!(app.icon_type, IconType::Url, "网址应用仍是 Url 类型");
    assert!(
        elapsed.as_secs() < 3,
        "添加网址应当立刻返回，实测耗时 {:?}",
        elapsed
    );
}

#[test]
fn refresh_url_icon_only_touches_url_apps_still_using_the_placeholder() {
    let directory = TestDir::new("refresh-url-icon");
    let state = directory.state();

    // 本地应用不参与补抓。
    let local = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: None,
            source_path: directory.path().to_string_lossy().into_owned(),
            name: Some("本地".to_owned()),
            root_index: None,
        },
    )
    .expect("local app should be added");
    assert!(catalog::refresh_url_icon(&state, &local.id)
        .expect("不应报错")
        .is_none());

    // 未知 ID 要明确报错，方便调用方区分「没抓到」和「根本没有这个应用」。
    let error = catalog::refresh_url_icon(&state, "no-such-app").expect_err("未知应用应当报错");
    assert_eq!(error.code, "app_not_found");
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
            root_index: None,
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

// 把 `app_id` 伪装成「已经抓到过图标」：写一份缓存文件，并把 `icon_url` 指过去。
//
// 测试不该依赖网络，所以这两步直接做出来。`icon_url` 的具体内容不重要 ——
// 判断「有没有真实图标」只跟内置占位图那一个值比对。
fn mark_icon_as_fetched(directory: &TestDir, app_id: &str, bytes: &[u8]) {
    let icons = directory.child("icons");
    fs::create_dir_all(&icons).expect("icons dir should be created");
    fs::write(icons.join(format!("{app_id}.png")), bytes).expect("cached icon should be written");

    let store = directory.store();
    let mut data = store.load_data().expect("data should load");
    root_app_mut(&mut data, app_id).icon_url = format!("http://asset.localhost/{app_id}.png");
    store.save_data(&data).expect("data should save");
}

// 顶层应用一定在 `RootItem::Apps` 那一行里（本文件的用例都按 `parent_id: None` 加）。
fn root_app_mut<'a>(data: &'a mut AppData, app_id: &str) -> &'a mut AppEXE {
    for item in &mut data.items {
        if let RootItem::Apps(apps) = item {
            if let Some(app) = apps.iter_mut().find(|app| app.id == app_id) {
                return app;
            }
        }
    }

    panic!("root app {app_id} should exist");
}

// 加一个网址应用，返回它是不是**立刻**拿到了真实图标（也就是走了同源复用）。
//
// `catalog::add_app` 本身不联网 —— 联网那条路在 Command 层起的后台线程里，这里
// 调不到，因此这个判断只可能来自复用。
fn adds_with_real_icon(state: &AppState, url: &str) -> bool {
    let app = catalog::add_app(
        state,
        AddAppRequest {
            parent_id: None,
            source_path: url.to_owned(),
            name: None,
            root_index: None,
        },
    )
    .expect("url app should be added");

    app.icon_url.starts_with("http://asset.localhost/")
}

#[test]
fn a_url_app_reuses_the_icon_already_fetched_for_the_same_origin() {
    let directory = TestDir::new("url-icon-reuse");
    let state = directory.state();

    let first = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: None,
            source_path: "https://reuse.example/docs/a".to_owned(),
            name: Some("第一个".to_owned()),
            root_index: None,
        },
    )
    .expect("第一个应用应当添加成功");
    mark_icon_as_fetched(&directory, &first.id, b"icon-bytes");

    let second = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: None,
            source_path: "https://reuse.example/docs/b".to_owned(),
            name: Some("第二个".to_owned()),
            root_index: None,
        },
    )
    .expect("第二个应用应当添加成功");

    assert_eq!(second.icon_type, IconType::Url, "复用不改变应用类型");
    assert!(
        second.icon_url.starts_with("http://asset.localhost/"),
        "同源复用应当立刻拿到真实图标，实际为 {}",
        second.icon_url
    );
    assert_eq!(
        fs::read(directory.child(&format!("icons/{}.png", second.id))).unwrap(),
        b"icon-bytes",
        "复用是把缓存文件复制一份"
    );
    // 各存各的一份：删掉第一个不能把第二个变成裂图。
    assert!(
        directory.child(&format!("icons/{}.png", first.id)).exists(),
        "来源应用的缓存不该被移走"
    );
}

#[test]
fn icons_are_only_reused_within_the_same_origin() {
    let directory = TestDir::new("url-icon-origin");
    let state = directory.state();

    let source = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: None,
            source_path: "https://origin.example/docs".to_owned(),
            name: Some("源站点".to_owned()),
            root_index: None,
        },
    )
    .expect("源应用应当添加成功");
    mark_icon_as_fetched(&directory, &source.id, b"icon-bytes");

    // 同源、只是路径不同 —— 复用。
    assert!(
        adds_with_real_icon(&state, "https://origin.example/other"),
        "同源的另一个入口应当复用"
    );
    // 子域是**另一个站点**，图标可能完全不一样。
    assert!(
        !adds_with_real_icon(&state, "https://docs.origin.example/"),
        "子域不该复用"
    );
    // 端口不同就是不同站点（`localhost:3000` 和 `localhost:5173` 是两回事）。
    assert!(
        !adds_with_real_icon(&state, "https://origin.example:8443/"),
        "端口不同不该复用"
    );
    assert!(
        !adds_with_real_icon(&state, "http://origin.example/"),
        "协议不同不该复用"
    );
}

#[test]
fn an_app_still_holding_the_builtin_icon_is_not_a_reuse_source() {
    let directory = TestDir::new("url-icon-no-source");
    let state = directory.state();

    // 端口 1 上没有服务：第一个应用抓不到图标，只有内置占位图。
    catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: None,
            source_path: "http://127.0.0.1:1/a".to_owned(),
            name: Some("没图标的".to_owned()),
            root_index: None,
        },
    )
    .expect("第一个应用应当添加成功");

    assert!(
        !adds_with_real_icon(&state, "http://127.0.0.1:1/b"),
        "来源应用自己都没图标，没有可复用的东西"
    );
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
            root_index: None,
        },
    )
    .expect("root level app should be added");

    let data = catalog::get_all_data(&state).unwrap();
    assert_eq!(data.items.len(), 1, "顶层应用应落在根节点上");
    assert_eq!(
        data.root_apps().next().map(|a| a.id.clone()),
        Some(app.id.clone())
    );
    assert!(
        data.root_categories().next().is_none(),
        "不应该凭空多出一个分类"
    );

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
            root_index: None,
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
    assert!(data.items.is_empty());
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
