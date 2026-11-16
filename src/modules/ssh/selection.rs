use crate::{app::MintoolApp, ui::theme};
use gpui::*;

impl MintoolApp {
    pub(crate) fn ssh_row_measure(
        &self,
        remote: bool,
        path: String,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let owner = self.modules.ssh.active_panel.clone().unwrap();
        let app = cx.entity().downgrade();
        canvas(
            move |bounds, _, cx| {
                let _ = app.update(cx, |app, _| {
                    if let Some(panel) = app.modules.ssh.panels.get_mut(&owner) {
                        panel.row_bounds.insert((remote, path.clone()), bounds);
                    }
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
    }
    pub(crate) fn ssh_list_measure(&self, remote: bool, cx: &Context<Self>) -> impl IntoElement {
        let owner = self.modules.ssh.active_panel.clone().unwrap();
        let app = cx.entity().downgrade();
        canvas(
            move |bounds, _, cx| {
                let _ = app.update(cx, |app, _| {
                    if let Some(panel) = app.modules.ssh.panels.get_mut(&owner) {
                        panel.list_bounds.insert(remote, bounds);
                    }
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
    }
    pub(crate) fn ssh_box_start(
        &mut self,
        remote: bool,
        point: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.modules.ssh.panel_mut().selection_drag = Some((remote, point, point));
        cx.notify();
    }
    pub(crate) fn ssh_box_move(
        &mut self,
        remote: bool,
        point: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some((side, start, _)) = self.modules.ssh.panel().selection_drag else {
            return;
        };
        if remote != side {
            return;
        }
        let visible = if remote {
            self.ssh_remote_entries()
                .into_iter()
                .map(|entry| entry.path)
                .collect::<Vec<_>>()
        } else {
            self.ssh_local_entries()
                .into_iter()
                .map(|entry| entry.path.to_string_lossy().into_owned())
                .collect()
        };
        let panel = self.modules.ssh.panel_mut();
        panel.selection_drag = Some((remote, start, point));
        let bounds = Bounds::from_corners(point_min(start, point), point_max(start, point));
        let paths = visible
            .into_iter()
            .filter(|path| {
                panel
                    .row_bounds
                    .get(&(remote, path.clone()))
                    .is_some_and(|row| row.intersects(&bounds))
            })
            .collect::<Vec<_>>();
        if remote {
            if let Some(browser) = &mut panel.remote {
                browser.selected_paths = paths.into_iter().collect();
            }
        } else if let Some(browser) = &mut panel.local {
            browser.selected_paths = paths.into_iter().map(std::path::PathBuf::from).collect();
        }
        cx.notify();
    }
    pub(crate) fn ssh_box_overlay(&self, remote: bool) -> AnyElement {
        let panel = self.modules.ssh.panel();
        let Some((side, start, end)) = panel.selection_drag else {
            return div().into_any_element();
        };
        let Some(list) = panel.list_bounds.get(&remote) else {
            return div().into_any_element();
        };
        if side != remote {
            return div().into_any_element();
        }
        let low = point_min(start, end);
        let high = point_max(start, end);
        div()
            .absolute()
            .left(low.x - list.origin.x)
            .top(low.y - list.origin.y)
            .w(high.x - low.x)
            .h(high.y - low.y)
            .bg(theme::accent().opacity(0.12))
            .border_1()
            .border_color(theme::accent())
            .into_any_element()
    }
}
fn point_min(a: Point<Pixels>, b: Point<Pixels>) -> Point<Pixels> {
    point(a.x.min(b.x), a.y.min(b.y))
}
fn point_max(a: Point<Pixels>, b: Point<Pixels>) -> Point<Pixels> {
    point(a.x.max(b.x), a.y.max(b.y))
}
