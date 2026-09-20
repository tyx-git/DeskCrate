mod support;

use app_lib::{
    models::{AddCategoryRequest, AddGroupRequest, DeleteRequest},
    services::catalog,
};

use support::{find_category, find_group, TestDir};

#[test]
fn service_tree_can_place_apps_and_groups_at_category_or_group_level() {
    let directory = TestDir::new("tree-placement");
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "根分类".to_owned(),
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

    let root_url = catalog::add_app(
        &state,
        app_lib::models::AddAppRequest {
            parent_id: Some(category.id.clone()),
            source_path: "https://example.com/root".to_owned(),
            name: Some("根网址".to_owned()),
        },
    )
    .unwrap();
    let child_url = catalog::add_app(
        &state,
        app_lib::models::AddAppRequest {
            parent_id: Some(group.id.clone()),
            source_path: "https://example.com/child".to_owned(),
            name: Some("子网址".to_owned()),
        },
    )
    .unwrap();

    let data = catalog::get_all_data(&state).unwrap();
    let category = find_category(&data, &category.id).unwrap();
    assert_eq!(category.groups.len(), 2);
    assert!(find_group(&data, &group.id).is_some());
    assert!(find_group(&data, &root_url.id).is_none());
    assert!(find_group(&data, &child_url.id).is_none());

    catalog::delete_app(&state, DeleteRequest { id: root_url.id }).unwrap();
    catalog::delete_app(&state, DeleteRequest { id: child_url.id }).unwrap();
}
