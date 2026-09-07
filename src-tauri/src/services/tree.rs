use crate::models::{AppData, AppEXE, Category, Group, Item};

// 目录树查询的内部实现。
//
// 分类服务和系统服务都需要在同一棵树中查找节点；把遍历集中在这里，
// 可以避免各服务分别维护一套递归逻辑，且不把树操作细节暴露给前端。

// 在所有分类中查找分类。
pub(crate) fn find_category<'a>(data: &'a AppData, id: &str) -> Option<&'a Category> {
    data.categories.iter().find(|category| category.id == id)
}

// 在所有分类中查找可变分类。
pub(crate) fn find_category_mut<'a>(data: &'a mut AppData, id: &str) -> Option<&'a mut Category> {
    data.categories
        .iter_mut()
        .find(|category| category.id == id)
}

// 在分类或分组下追加一个节点。
pub(crate) fn append_child(data: &mut AppData, parent_id: &str, item: Item) -> bool {
    if let Some(category) = find_category_mut(data, parent_id) {
        category.groups.push(item);
        return true;
    }

    if let Some(group) = find_group_mut(data, parent_id) {
        group.children.push(item);
        return true;
    }

    false
}

// 删除顶层分类，并返回完整分类。
pub(crate) fn remove_category(data: &mut AppData, id: &str) -> Option<Category> {
    let index = data
        .categories
        .iter()
        .position(|category| category.id == id)?;
    Some(data.categories.remove(index))
}

// 删除指定类型的节点，并返回完整节点；删除分组时会连同其后代一起移除。
#[derive(Clone, Copy)]
pub(crate) enum NodeKind {
    Group,
    App,
}

pub(crate) fn remove_node(data: &mut AppData, id: &str, kind: NodeKind) -> Option<Item> {
    data.categories
        .iter_mut()
        .find_map(|category| remove_from_items(&mut category.groups, id, kind))
}

// 在所有分类中查找分组。
pub(crate) fn find_group<'a>(data: &'a AppData, id: &str) -> Option<&'a Group> {
    data.categories
        .iter()
        .find_map(|category| find_group_in_items(&category.groups, id))
}

// 在所有分类中查找可变分组。
pub(crate) fn find_group_mut<'a>(data: &'a mut AppData, id: &str) -> Option<&'a mut Group> {
    for category in &mut data.categories {
        if let Some(group) = find_group_in_items_mut(&mut category.groups, id) {
            return Some(group);
        }
    }

    None
}

// 在所有分类中查找应用。
pub(crate) fn find_app<'a>(data: &'a AppData, id: &str) -> Option<&'a AppEXE> {
    if id.trim().is_empty() {
        return None;
    }

    data.categories
        .iter()
        .find_map(|category| find_app_in_items(&category.groups, id))
}

// 在所有分类中查找可变应用。
pub(crate) fn find_app_mut<'a>(data: &'a mut AppData, id: &str) -> Option<&'a mut AppEXE> {
    for category in &mut data.categories {
        if let Some(app) = find_app_in_items_mut(&mut category.groups, id) {
            return Some(app);
        }
    }

    None
}

// 判断一个 ID 是否可以作为分类或分组的父节点。
pub(crate) fn parent_exists(data: &AppData, id: &str) -> bool {
    find_category(data, id).is_some() || find_group(data, id).is_some()
}

// 返回父节点所在的分组深度。
//
// 顶层分类返回 0，分类下的直接分组返回 1，以此类推。
pub(crate) fn parent_depth(data: &AppData, id: &str) -> Option<usize> {
    if find_category(data, id).is_some() {
        return Some(0);
    }

    data.categories
        .iter()
        .find_map(|category| find_group_depth(&category.groups, id, 1))
}

fn find_group_in_items<'a>(items: &'a [Item], id: &str) -> Option<&'a Group> {
    items.iter().find_map(|item| match item {
        Item::Group(group) if group.id == id => Some(group),
        Item::Group(group) => find_group_in_items(&group.children, id),
        Item::App(_) => None,
    })
}

fn find_group_in_items_mut<'a>(items: &'a mut [Item], id: &str) -> Option<&'a mut Group> {
    for item in items {
        if let Item::Group(group) = item {
            // 先结束当前 ID 比较的借用，再递归借用 children，避免 E0499。
            if group.id == id {
                return Some(group);
            }

            if let Some(found) = find_group_in_items_mut(&mut group.children, id) {
                return Some(found);
            }
        }
    }

    None
}

fn find_app_in_items<'a>(items: &'a [Item], id: &str) -> Option<&'a AppEXE> {
    items.iter().find_map(|item| match item {
        Item::App(app) if app.id == id => Some(app),
        Item::App(_) => None,
        Item::Group(group) => find_app_in_items(&group.children, id),
    })
}

fn find_app_in_items_mut<'a>(items: &'a mut [Item], id: &str) -> Option<&'a mut AppEXE> {
    for item in items {
        match item {
            Item::App(app) if app.id == id => return Some(app),
            Item::App(_) => {}
            Item::Group(group) => {
                if let Some(found) = find_app_in_items_mut(&mut group.children, id) {
                    return Some(found);
                }
            }
        }
    }

    None
}

fn find_group_depth(items: &[Item], id: &str, depth: usize) -> Option<usize> {
    for item in items {
        match item {
            Item::Group(group) if group.id == id => return Some(depth),
            Item::Group(group) => {
                if let Some(found) = find_group_depth(&group.children, id, depth + 1) {
                    return Some(found);
                }
            }
            Item::App(_) => {}
        }
    }

    None
}

fn remove_from_items(items: &mut Vec<Item>, id: &str, kind: NodeKind) -> Option<Item> {
    if let Some(index) = items.iter().position(|item| matches_node(item, id, kind)) {
        return Some(items.remove(index));
    }

    items.iter_mut().find_map(|item| match item {
        Item::Group(group) => remove_from_items(&mut group.children, id, kind),
        Item::App(_) => None,
    })
}

fn matches_node(item: &Item, id: &str, kind: NodeKind) -> bool {
    match item {
        Item::Group(group) => matches!(kind, NodeKind::Group) && group.id == id,
        Item::App(app) => matches!(kind, NodeKind::App) && app.id == id,
    }
}
