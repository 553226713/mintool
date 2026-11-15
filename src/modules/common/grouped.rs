use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A user-defined grouping shared by every module's sidebar.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub collapsed: bool,
}
impl Group {
    pub fn new(name: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            collapsed: false,
        }
    }
}

/// Items that can be grouped, ordered and selected inside a module sidebar.
pub trait GroupedItem {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn group(&self) -> Option<&str>;
    fn set_group(&mut self, group: Option<String>);
}

pub fn add_group(groups: &mut Vec<Group>, name: &str) -> Result<String> {
    let name = name.trim();
    ensure!(!name.is_empty(), "分组名称不能为空");
    ensure!(
        !groups.iter().any(|group| group.name == name),
        "分组名称已存在"
    );
    let group = Group::new(name);
    let id = group.id.clone();
    groups.push(group);
    Ok(id)
}

pub fn remove_group<T: GroupedItem>(groups: &mut Vec<Group>, items: &mut [T], id: &str) {
    groups.retain(|group| group.id != id);
    for item in items {
        if item.group() == Some(id) {
            item.set_group(None);
        }
    }
}

pub fn move_group(groups: &mut Vec<Group>, id: &str, before: &str) {
    if id == before {
        return;
    }
    if let Some(index) = groups.iter().position(|group| group.id == id) {
        let group = groups.remove(index);
        let target = groups
            .iter()
            .position(|group| group.id == before)
            .unwrap_or(groups.len());
        groups.insert(target, group);
    }
}

pub fn shift_group(groups: &mut [Group], id: &str, direction: isize) {
    let Some(index) = groups.iter().position(|group| group.id == id) else {
        return;
    };
    if let Some(next) = index
        .checked_add_signed(direction)
        .filter(|&next| next < groups.len())
    {
        groups.swap(index, next);
    }
}

pub fn shift_item<T: GroupedItem>(items: &mut [T], id: &str, direction: isize) {
    let Some(index) = items.iter().position(|item| item.id() == id) else {
        return;
    };
    let group = items[index].group().map(str::to_owned);
    let peers: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.group() == group.as_deref())
        .map(|(index, _)| index)
        .collect();
    let Some(position) = peers.iter().position(|&peer| peer == index) else {
        return;
    };
    if let Some(next) = position
        .checked_add_signed(direction)
        .and_then(|position| peers.get(position))
    {
        items.swap(index, *next);
    }
}

pub fn move_item<T: GroupedItem>(
    items: &mut Vec<T>,
    id: &str,
    group: Option<String>,
    before: Option<&str>,
) {
    if before == Some(id) {
        return;
    }
    if let Some(index) = items.iter().position(|item| item.id() == id) {
        let mut item = items.remove(index);
        item.set_group(group);
        let target = before
            .and_then(|before| items.iter().position(|item| item.id() == before))
            .unwrap_or(items.len());
        items.insert(target, item);
    }
}
