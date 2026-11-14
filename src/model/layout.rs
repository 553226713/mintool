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
    #[serde(default)]
    pub terminal_closed: bool,
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
    pub dock_layout: Option<super::dock_layout::DockLayout>,
    pub focused: u64,
    pub next_id: u64,
    pub expanded: BTreeSet<PathBuf>,
}
impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            layout: Layout::pane(1),
            dock_layout: None,
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
            terminal_closed: false,
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
    pub fn find(&self, id: u64) -> Option<&Pane> {
        match self {
            Self::Pane(p) => (p.id == id).then_some(p),
            Self::Split { first, second, .. } => first.find(id).or_else(|| second.find(id)),
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
            Self::Split {
                first,
                second,
                axis: current_axis,
                ..
            } => {
                let balance = *current_axis == axis
                    && (first.axis_contains(id, axis) || second.axis_contains(id, axis));
                let changed = first.split(id, new_id, axis) || second.split(id, new_id, axis);
                if changed && balance {
                    self.balance_axis(axis);
                }
                changed
            }
            _ => false,
        }
    }
    fn axis_contains(&self, id: u64, axis: Axis) -> bool {
        match self {
            Self::Pane(pane) => pane.id == id,
            Self::Split {
                axis: current,
                first,
                second,
                ..
            } if *current == axis => {
                first.axis_contains(id, axis) || second.axis_contains(id, axis)
            }
            _ => false,
        }
    }
    fn axis_count(&self, axis: Axis) -> usize {
        match self {
            Self::Split {
                axis: current,
                first,
                second,
                ..
            } if *current == axis => first.axis_count(axis) + second.axis_count(axis),
            _ => 1,
        }
    }
    fn balance_axis(&mut self, axis: Axis) {
        if let Self::Split {
            axis: current,
            ratio,
            first,
            second,
        } = self
        {
            if *current == axis {
                let a = first.axis_count(axis);
                let b = second.axis_count(axis);
                *ratio = a as f32 / (a + b) as f32;
                first.balance_axis(axis);
                second.balance_axis(axis);
            }
        }
    }
    /// Includes all descendants so nested headers and terminal rows stay usable.
    pub fn minimum_size(&self) -> (f32, f32) {
        match self {
            Self::Pane(_) => (240., 140.),
            Self::Split {
                axis,
                first,
                second,
                ..
            } => {
                let a = first.minimum_size();
                let b = second.minimum_size();
                match axis {
                    Axis::Horizontal => (a.0 + b.0, a.1.max(b.1)),
                    Axis::Vertical => (a.0.max(b.0), a.1 + b.1),
                }
            }
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
                    *ratio = value.clamp(0.01, 0.99);
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
