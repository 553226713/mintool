use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::PathBuf};

#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq)]
pub enum Axis {
    Horizontal,
    Vertical,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Pane {
    pub id: u64,
    pub file: Option<PathBuf>,
    pub showing_file: bool,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Layout {
    Pane(Pane),
    Split {
        axis: Axis,
        ratio: f32,
        first: Box<Layout>,
        second: Box<Layout>,
    },
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct WorkspaceState {
    pub layout: Layout,
    pub focused: u64,
    pub next_id: u64,
    pub expanded: BTreeSet<PathBuf>,
}
impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            layout: Layout::pane(1),
            focused: 1,
            next_id: 2,
            expanded: BTreeSet::new(),
        }
    }
}
impl Layout {
    pub fn pane(id: u64) -> Self {
        Self::Pane(Pane {
            id,
            file: None,
            showing_file: false,
        })
    }
    pub fn first_id(&self) -> u64 {
        match self {
            Self::Pane(p) => p.id,
            Self::Split { first, .. } => first.first_id(),
        }
    }
    pub fn ids(&self, ids: &mut Vec<u64>) {
        match self {
            Self::Pane(p) => ids.push(p.id),
            Self::Split { first, second, .. } => {
                first.ids(ids);
                second.ids(ids);
            }
        }
    }
    pub fn find_mut(&mut self, id: u64) -> Option<&mut Pane> {
        match self {
            Self::Pane(p) => (p.id == id).then_some(p),
            Self::Split { first, second, .. } => first.find_mut(id).or_else(|| second.find_mut(id)),
        }
    }
    pub fn split(&mut self, id: u64, new_id: u64, axis: Axis) -> bool {
        match self {
            Self::Pane(p) if p.id == id => {
                *self = Self::Split {
                    axis,
                    ratio: 0.5,
                    first: Box::new(self.clone()),
                    second: Box::new(Self::pane(new_id)),
                };
                true
            }
            Self::Split { first, second, .. } => {
                first.split(id, new_id, axis) || second.split(id, new_id, axis)
            }
            _ => false,
        }
    }
    pub fn set_ratio(&mut self, first_id: u64, second_id: u64, value: f32) -> bool {
        match self {
            Self::Split {
                ratio,
                first,
                second,
                ..
            } => {
                if first.first_id() == first_id && second.first_id() == second_id {
                    *ratio = value.clamp(0.1, 0.9);
                    true
                } else {
                    first.set_ratio(first_id, second_id, value)
                        || second.set_ratio(first_id, second_id, value)
                }
            }
            _ => false,
        }
    }
    pub fn close(&mut self, id: u64) -> bool {
        if let Self::Split { first, second, .. } = self {
            if matches!(first.as_ref(), Self::Pane(p) if p.id == id) {
                *self = *second.clone();
                return true;
            }
            if matches!(second.as_ref(), Self::Pane(p) if p.id == id) {
                *self = *first.clone();
                return true;
            }
            return first.close(id) || second.close(id);
        }
        false
    }
}
