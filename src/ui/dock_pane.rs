use crate::{
    app::MintoolApp,
    model::{layout::Axis, project::Project},
};
use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent, PanelInfo, PanelState, TabPanel},
    Sizable,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

pub struct DockPane {
    pub app: WeakEntity<MintoolApp>,
    pub project: Project,
    pub id: u64,
    pub group: Option<WeakEntity<TabPanel>>,
    pub count: Arc<AtomicUsize>,
    focus: FocusHandle,
    bounds: Size<Pixels>,
}

impl DockPane {
    pub fn new(
        app: WeakEntity<MintoolApp>,
        project: Project,
        id: u64,
        count: Arc<AtomicUsize>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            app,
            project,
            id,
            group: None,
            count,
            focus: cx.focus_handle(),
            bounds: size(px(500.), px(500.)),
        }
    }
}
impl EventEmitter<PanelEvent> for DockPane {}
impl Focusable for DockPane {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Panel for DockPane {
    fn panel_name(&self) -> &'static str {
        "MintoolPane"
    }
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (app, id) = (self.app.clone(), self.id);
        let middle_app = app.clone();
        div()
            .id(("terminal-tab", id))
            .h_full()
            .flex()
            .items_center()
            .gap_2()
            .child(format!("终端 {id}"))
            .on_mouse_down(MouseButton::Middle, move |_, _, cx| {
                let _ = middle_app.update(cx, |app, cx| app.close_pane(id, cx));
                cx.stop_propagation();
            })
            .child(
                Button::new(("close-terminal", id))
                    .icon(gpui_component::IconName::Close)
                    .ghost()
                    .xsmall()
                    .tooltip("关闭终端并停止关联进程")
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(move |_, _, cx| {
                        let _ = app.update(cx, |app, cx| app.close_pane(id, cx));
                        cx.stop_propagation();
                    }),
            )
    }
    fn closable(&self, _: &App) -> bool {
        self.count.load(Ordering::Relaxed) > 1
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
    fn on_added_to(&mut self, group: WeakEntity<TabPanel>, _: &mut Window, _: &mut Context<Self>) {
        self.group = Some(group);
    }
    fn set_active(&mut self, active: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !active {
            return;
        }
        let (app, project, id) = (self.app.clone(), self.project.id.clone(), self.id);
        window.defer(cx, move |_, cx| {
            let _ = app.update(cx, |app, cx| {
                if let Some(workspace) = app.workspaces.get_mut(&project) {
                    workspace.state.focused = id;
                }
                app.focus_pending = true;
                app.changed(cx);
            });
        });
    }
    fn toolbar_buttons(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<Vec<Button>> {
        Some(
            [
                (Axis::Horizontal, "左右拆分", "icons/columns-2.svg"),
                (Axis::Vertical, "上下拆分", "icons/rows-2.svg"),
            ]
            .into_iter()
            .map(|(axis, tip, icon)| {
                let (app, id) = (self.app.clone(), self.id);
                Button::new(tip)
                    .icon(gpui_component::Icon::default().path(icon))
                    .ghost()
                    .small()
                    .tooltip(tip)
                    .on_click(move |_, _, cx| {
                        let _ = app.update(cx, |app, cx| app.split(id, axis, cx));
                    })
            })
            .collect(),
        )
    }
    fn dump(&self, _: &App) -> PanelState {
        PanelState {
            panel_name: "MintoolPane".into(),
            children: vec![],
            info: PanelInfo::panel(serde_json::json!(self.id)),
        }
    }
}
impl Render for DockPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let project = self.project.clone();
        let id = self.id;
        let bounds = (f32::from(self.bounds.width), f32::from(self.bounds.height));
        let content = self
            .app
            .update(cx, |app, cx| {
                let pane = app
                    .workspaces
                    .get_mut(&project.id)?
                    .state
                    .layout
                    .find_mut(id)?
                    .clone();
                Some(app.render_pane(&project, &pane, bounds, window, cx))
            })
            .ok()
            .flatten();
        let view = cx.entity();
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .min_h_0()
            .min_w_0()
            .track_focus(&self.focus)
            .children(content)
            .child(
                canvas(
                    move |bounds, _, cx| {
                        view.update(cx, |view, cx| {
                            if view.bounds != bounds.size {
                                view.bounds = bounds.size;
                                cx.notify();
                            }
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
    }
}
