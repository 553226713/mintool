use crate::{
    app::MintoolApp,
    model::{
        dock_layout::DockLayout,
        layout::{Axis, Layout},
        project::Project,
    },
    ui::dock_pane::DockPane,
};
use gpui::*;
use gpui_component::{
    dock::{DockArea, DockEvent, DockItem, PanelInfo, PanelState, PanelStyle, PanelView},
    Placement,
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub struct DockWorkspace {
    pub area: Entity<DockArea>,
    pub panels: HashMap<u64, Entity<DockPane>>,
    pub count: Arc<AtomicUsize>,
    pub pending_sync: Option<Instant>,
    _subscription: Subscription,
}

impl MintoolApp {
    pub(crate) fn render_dock(
        &mut self,
        project: &Project,
        layout: &Layout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = project.id.clone();
        let app = cx.entity().downgrade();
        if !self.docks.contains_key(&key) {
            let area = cx.new(|cx| {
                DockArea::new(format!("workspace-{key}"), Some(1), window, cx)
                    .panel_style(PanelStyle::TabBar)
            });
            let mut ids = Vec::new();
            layout.ids(&mut ids);
            let count = Arc::new(AtomicUsize::new(ids.len()));
            let panels = ids
                .iter()
                .map(|id| {
                    (
                        *id,
                        cx.new(|child_cx| {
                            DockPane::new(
                                app.clone(),
                                project.clone(),
                                *id,
                                count.clone(),
                                child_cx,
                            )
                        }),
                    )
                })
                .collect::<HashMap<_, _>>();
            let saved = self.workspaces[&key]
                .state
                .dock_layout
                .clone()
                .filter(|saved| saved.valid_for(&ids));
            let saved = split_root(saved.unwrap_or_else(|| from_legacy(layout)));
            let center = build_item(&saved, &panels, &area.downgrade(), window, cx);
            area.update(cx, |area, cx| area.set_center(center, window, cx));
            let project_id = key.clone();
            let subscription = cx.subscribe(&area, move |app, _, _: &DockEvent, cx| {
                if let Some(dock) = app.docks.get_mut(&project_id) {
                    dock.pending_sync = Some(Instant::now());
                }
                app.changed(cx);
            });
            self.docks.insert(
                key.clone(),
                DockWorkspace {
                    area,
                    panels,
                    count,
                    pending_sync: Some(Instant::now()),
                    _subscription: subscription,
                },
            );
        }
        let requests = std::mem::take(&mut self.dock_splits);
        for (project_id, target, next, axis) in requests {
            if project_id != key {
                self.dock_splits.push((project_id, target, next, axis));
                continue;
            }
            let dock = self.docks.get_mut(&key).unwrap();
            if dock.panels.contains_key(&next) {
                continue;
            }
            let panel = cx.new(|child_cx| {
                DockPane::new(
                    app.clone(),
                    project.clone(),
                    next,
                    dock.count.clone(),
                    child_cx,
                )
            });
            let group = dock
                .panels
                .get(&target)
                .and_then(|panel| panel.read(cx).group.clone());
            if let Some(group) = group {
                let panel_view: Arc<dyn PanelView> = Arc::new(panel.clone());
                let placement = if axis == Axis::Horizontal {
                    Placement::Right
                } else {
                    Placement::Bottom
                };
                let _ = group.update(cx, |group, cx| {
                    group.add_panel_at(panel_view, placement, None, window, cx)
                });
                dock.panels.insert(next, panel);
                dock.count.store(dock.panels.len(), Ordering::Relaxed);
            }
        }
        let requests = std::mem::take(&mut self.dock_closes);
        for (project_id, id) in requests {
            if project_id != key {
                self.dock_closes.push((project_id, id));
                continue;
            }
            let dock = &self.docks[&key];
            if let Some(panel) = dock.panels.get(&id) {
                if let Some(group) = panel.read(cx).group.clone() {
                    let view: Arc<dyn PanelView> = Arc::new(panel.clone());
                    let _ = group.update(cx, |group, cx| group.remove_panel(view, window, cx));
                }
            }
        }
        self.docks[&key].area.clone().into_any_element()
    }

    pub(crate) fn sync_docks(&mut self, cx: &mut Context<Self>) {
        if cx.has_active_drag() {
            return;
        }
        let keys: Vec<_> = self
            .docks
            .iter()
            .filter(|(_, dock)| {
                dock.pending_sync
                    .is_some_and(|time| time.elapsed() >= Duration::from_millis(150))
            })
            .map(|(key, _)| key.clone())
            .collect();
        for key in keys {
            let dock = self.docks.get_mut(&key).unwrap();
            dock.pending_sync = None;
            let state = dock.area.read(cx).dump(cx);
            let Some(layout) = snapshot(&state.center) else {
                continue;
            };
            let mut ids = Vec::new();
            layout.ids(&mut ids);
            if ids.is_empty()
                || !layout.valid_for(&ids)
                || ids.iter().any(|id| !dock.panels.contains_key(id))
            {
                continue;
            }
            let removed: Vec<_> = dock
                .panels
                .keys()
                .filter(|id| !ids.contains(id))
                .copied()
                .collect();
            for id in removed {
                dock.panels.remove(&id);
                if let Some(session) = self
                    .workspaces
                    .get_mut(&key)
                    .and_then(|workspace| workspace.terminals.remove(&id))
                {
                    session.lock().unwrap().shutdown();
                }
                self.terminals.remove(&(key.clone(), id));
                if let Some(terminal) = self.system_terminals.remove(&(key.clone(), id)) {
                    terminal.update(cx, |view, _| view.stop());
                }
                self.previews.remove(&(key.clone(), id));
                if let Some(workspace) = self.workspaces.get_mut(&key) {
                    workspace.state.layout.close(id);
                    workspace.terminals.remove(&id);
                    workspace.errors.remove(&id);
                }
            }
            dock.count.store(ids.len(), Ordering::Relaxed);
            if let Some(workspace) = self.workspaces.get_mut(&key) {
                workspace.state.dock_layout = Some(layout);
                if !ids.contains(&workspace.state.focused) {
                    workspace.state.focused = ids[0];
                }
            }
            self.changed(cx);
        }
    }
}

fn split_root(layout: DockLayout) -> DockLayout {
    // TabPanel split/drop operations require a StackPanel parent even when
    // there is only one tab group, both on first launch and after restoring.
    match layout {
        DockLayout::Split { .. } => layout,
        _ => DockLayout::Split {
            axis: Axis::Horizontal,
            sizes: vec![0.],
            children: vec![layout],
        },
    }
}

fn from_legacy(layout: &Layout) -> DockLayout {
    match layout {
        Layout::Pane(pane) => DockLayout::Tabs {
            panes: vec![pane.id],
            active: 0,
        },
        Layout::Split {
            axis,
            ratio,
            first,
            second,
        } => DockLayout::Split {
            axis: *axis,
            sizes: vec![ratio * 1000., (1. - ratio) * 1000.],
            children: vec![from_legacy(first), from_legacy(second)],
        },
    }
}
fn build_item(
    layout: &DockLayout,
    panels: &HashMap<u64, Entity<DockPane>>,
    area: &WeakEntity<DockArea>,
    window: &mut Window,
    cx: &mut App,
) -> DockItem {
    match layout {
        DockLayout::Tabs { panes, active } => DockItem::new_tabs(
            panes
                .iter()
                .map(|id| Arc::new(panels[id].clone()) as Arc<dyn PanelView>)
                .collect(),
            Some(*active),
            area,
            window,
            cx,
        ),
        DockLayout::Split {
            axis,
            sizes,
            children,
        } => {
            let children = children
                .iter()
                .map(|child| build_item(child, panels, area, window, cx))
                .collect();
            DockItem::split_with_sizes(
                if *axis == Axis::Horizontal {
                    gpui::Axis::Horizontal
                } else {
                    gpui::Axis::Vertical
                },
                children,
                sizes
                    .iter()
                    .map(|size| (*size > 0.).then(|| px(*size)))
                    .collect(),
                area,
                window,
                cx,
            )
        }
    }
}

fn snapshot(state: &PanelState) -> Option<DockLayout> {
    match &state.info {
        PanelInfo::Stack { sizes, axis } => Some(DockLayout::Split {
            axis: if *axis == 0 {
                Axis::Horizontal
            } else {
                Axis::Vertical
            },
            sizes: sizes.iter().map(|size| f32::from(*size)).collect(),
            children: state.children.iter().map(snapshot).collect::<Option<_>>()?,
        }),
        PanelInfo::Tabs { active_index } => Some(DockLayout::Tabs {
            panes: state
                .children
                .iter()
                .map(|child| {
                    if let PanelInfo::Panel(value) = &child.info {
                        value.as_u64()
                    } else {
                        None
                    }
                })
                .collect::<Option<_>>()?,
            active: *active_index,
        }),
        PanelInfo::Panel(value) => Some(DockLayout::Tabs {
            panes: vec![value.as_u64()?],
            active: 0,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{from_legacy, split_root};
    use crate::model::{dock_layout::DockLayout, layout::Layout};

    #[test]
    fn first_and_restored_single_tab_groups_have_a_split_parent() {
        for source in [
            from_legacy(&Layout::pane(1)),
            DockLayout::Tabs {
                panes: vec![2, 1],
                active: 1,
            },
        ] {
            let mut ids = Vec::new();
            source.ids(&mut ids);
            let root = split_root(source);
            assert!(root.valid_for(&ids));
            let DockLayout::Split {
                sizes, children, ..
            } = root
            else {
                panic!("split parent missing")
            };
            assert_eq!(sizes, [0.]);
            assert_eq!(children.len(), 1);
            assert!(matches!(children[0], DockLayout::Tabs { .. }));
        }
    }
}
