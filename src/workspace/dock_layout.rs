use super::layout::Axis;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DockLayout {
    Split {
        axis: Axis,
        sizes: Vec<f32>,
        children: Vec<DockLayout>,
    },
    Tabs {
        panes: Vec<u64>,
        active: usize,
    },
}

impl DockLayout {
    pub fn add_tab_after(&mut self, anchor: u64, pane: u64) -> bool {
        match self {
            Self::Tabs { panes, active } => {
                if let Some(index) = panes.iter().position(|id| *id == anchor) {
                    panes.insert(index + 1, pane);
                    *active = index + 1;
                    true
                } else {
                    false
                }
            }
            Self::Split { children, .. } => children
                .iter_mut()
                .any(|child| child.add_tab_after(anchor, pane)),
        }
    }

    pub fn ids(&self, ids: &mut Vec<u64>) {
        match self {
            Self::Split { children, .. } => children.iter().for_each(|child| child.ids(ids)),
            Self::Tabs { panes, .. } => ids.extend(panes),
        }
    }

    pub fn valid_for(&self, expected: &[u64]) -> bool {
        let mut ids = Vec::new();
        self.ids(&mut ids);
        let unique: BTreeSet<_> = ids.iter().copied().collect();
        !ids.is_empty()
            && ids.len() == unique.len()
            && unique == expected.iter().copied().collect()
            && self.valid_nodes()
    }

    fn valid_nodes(&self) -> bool {
        match self {
            Self::Split {
                children, sizes, ..
            } => {
                !children.is_empty()
                    && children.len() == sizes.len()
                    && sizes.iter().all(|size| size.is_finite() && *size >= 0.)
                    && children.iter().all(Self::valid_nodes)
            }
            Self::Tabs { panes, active } => !panes.is_empty() && *active < panes.len(),
        }
    }
}
