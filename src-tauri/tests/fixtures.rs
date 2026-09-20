mod support;

use std::fs;

use app_lib::{
    models::{AddAppRequest, AddCategoryRequest, IconType},
    services::catalog,
};

use support::TestDir;

#[test]
fn fixture_directory_can_be_used_as_a_real_folder_application_source() {
    let directory = TestDir::new("fixture-folder");
    let source = directory.child("fixture-folder");
    fs::create_dir_all(&source).expect("fixture folder should exist");
    let state = directory.state();
    let category = catalog::add_category(
        &state,
        AddCategoryRequest {
            name: "Fixture".to_owned(),
        },
    )
    .unwrap();
    let app = catalog::add_app(
        &state,
        AddAppRequest {
            parent_id: Some(category.id),
            source_path: source.to_string_lossy().into_owned(),
            name: None,
        },
    )
    .expect("fixture folder should be accepted");

    assert_eq!(app.icon_type, IconType::Folder);
    assert!(directory.child(&format!("icons/{}.png", app.id)).exists());
}
