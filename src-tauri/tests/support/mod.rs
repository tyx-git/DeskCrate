#![allow(dead_code)]

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use lib::{
    models::{AppData, AppEXE, Category, Group, Item},
    state::AppState,
    storage::JsonStore,
};

/// 每个测试独占的临时目录。
///
/// DeskCrate 的生产数据位于程序目录，因此测试不能直接使用真实程序目录；
/// 这个 fixture 让每个测试都拥有独立的 `config/` 和 `icons/`。
pub struct TestDir {
    path: PathBuf,
}

impl TestDir {
    pub fn new(label: &str) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be valid")
            .as_nanos();

        for attempt in 0..100u32 {
            let path = std::env::temp_dir().join(format!(
                "deskcrate-{label}-{}-{timestamp}-{attempt}",
                std::process::id()
            ));

            if fs::create_dir(&path).is_ok() {
                return Self { path };
            }
        }

        panic!("could not create a unique temporary test directory");
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    pub fn store(&self) -> JsonStore {
        let store = JsonStore::new(&self.path);
        store
            .ensure_layout()
            .expect("test JSON layout should be created");
        store
    }

    pub fn state(&self) -> AppState {
        AppState::new(self.store())
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// 在数据树中按 ID 查找任意节点（只找分类/分组内部，不含顶层应用）。
pub fn find_item<'a>(data: &'a AppData, id: &str) -> Option<&'a Item> {
    data.root_categories()
        .flat_map(|category| category.groups.iter())
        .find_map(|item| find_item_in_item(item, id))
}

fn find_item_in_item<'a>(item: &'a Item, id: &str) -> Option<&'a Item> {
    match item {
        Item::App(app) if app.id == id => Some(item),
        Item::Group(group) if group.id == id => Some(item),
        Item::App(_) => None,
        Item::Group(group) => group
            .children
            .iter()
            .find_map(|child| find_item_in_item(child, id)),
    }
}

pub fn find_category<'a>(data: &'a AppData, id: &str) -> Option<&'a Category> {
    data.root_categories().find(|category| category.id == id)
}

pub fn find_group<'a>(data: &'a AppData, id: &str) -> Option<&'a Group> {
    find_item(data, id).and_then(|item| match item {
        Item::Group(group) => Some(group),
        Item::App(_) => None,
    })
}

// 顶层应用就在根节点上，`find_app` 只需按显示顺序扫一遍即可 —— `find_item`
// 只走分类内部，扫不到它们。
pub fn find_app<'a>(data: &'a AppData, id: &str) -> Option<&'a AppEXE> {
    data.root_apps().find(|app| app.id == id).or_else(|| {
        find_item(data, id).and_then(|item| match item {
            Item::App(app) => Some(app),
            Item::Group(_) => None,
        })
    })
}

pub fn count_apps(data: &AppData) -> usize {
    data.root_apps().count()
        + data
            .root_categories()
            .flat_map(|category| category.groups.iter())
            .map(count_apps_in_item)
            .sum::<usize>()
}

fn count_apps_in_item(item: &Item) -> usize {
    match item {
        Item::App(_) => 1,
        Item::Group(group) => group.children.iter().map(count_apps_in_item).sum(),
    }
}
