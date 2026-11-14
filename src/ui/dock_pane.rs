use crate::{
    app::MintoolApp,
    model::{layout::Axis, project::Project},
};
use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent, PanelInfo, PanelState, TabPanel},
    Disableable, Sizable,
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
    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (app, id) = (self.app.clone(), self.id);
        let middle_app = app.clone();
        let (closed, file_name) = app.upgrade().map_or((false, None), |app| {
            let app = app.read(cx);
            let Some(workspace) = app.workspaces.get(&self.project.id) else {
                return (false, None);
            };
            let file_name = workspace
                .state
                .files
                .get(&id)
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned());
            let closed = workspace.state.files.contains_key(&id)
                || workspace
                    .state
                    .layout
                    .find(id)
                    .is_none_or(|pane| pane.terminal_closed);
            (closed, file_name)
        });
        div()
            .id(("terminal-tab", id))
            .h_full()
            .flex()
            .items_center()
            .gap_2()
            .child(if let Some(file_name) = file_name {
                file_name
            } else if closed {
                "空面板".to_string()
            } else {
                format!("终端 {id}")
            })
            .on_mouse_down(MouseButton::Middle, move |_, _, cx| {
                let _ = middle_app.update(cx, |app, cx| app.close_pane(id, cx));
                cx.stop_propagation();
            })
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
    fn toolbar_buttons(&mut self, _: &mut Window, cx: &mut Context<Self>) -> Option<Vec<Button>> {
        let app = self.app.upgrade()?;
        if app
            .read(cx)
            .workspaces
            .get(&self.project.id)
            .is_some_and(|workspace| workspace.state.files.contains_key(&self.id))
        {
            let app = self.app.clone();
            let id = self.id;
            let closable = self.count.load(Ordering::Relaxed) > 1;
            return Some(vec![Button::new("close-file")
                .icon(gpui_component::IconName::Close)
                .ghost()
                .small()
                .tooltip("关闭文件预览")
                .disabled(!closable)
                .on_click(move |_, _, cx| {
                    let _ = app.update(cx, |app, cx| app.close_pane(id, cx));
                })]);
        }
        let showing_file = app
            .read(cx)
            .workspaces
            .get(&self.project.id)
            .and_then(|workspace| workspace.state.layout.find(self.id))
            .is_some_and(|pane| pane.showing_file);
        if showing_file {
            return None;
        }

        let link_app = self.app.clone();
        let split_app = self.app.clone();
        let close_app = self.app.clone();
        let id = self.id;
        let closable = self.count.load(Ordering::Relaxed) > 1;
        Some(vec![
            Button::new("open-clipboard-link")
                .icon(gpui_component::Icon::default().path("icons/external-link.svg"))
                .ghost()
                .small()
                .tooltip("打开剪贴板中的链接")
                .on_click(move |_, _, cx| {
                    if let Err(error) = crate::services::desktop::Desktop::open_clipboard_link() {
                        let _ = link_app.update(cx, |app, cx| {
                            app.error = Some(format!("{error:#}"));
                            app.changed(cx);
                        });
                    }
                }),
            Button::new("horizontal-split")
                .icon(gpui_component::Icon::default().path("icons/columns-2.svg"))
                .ghost()
                .small()
                .tooltip("左右拆分")
                .on_click({
                    let app = split_app.clone();
                    move |_, _, cx| {
                        let _ = app.update(cx, |app, cx| app.split(id, Axis::Horizontal, cx));
                    }
                }),
            Button::new("vertical-split")
                .icon(gpui_component::Icon::default().path("icons/rows-2.svg"))
                .ghost()
                .small()
                .tooltip("上下拆分")
                .on_click(move |_, _, cx| {
                    let _ = split_app.update(cx, |app, cx| app.split(id, Axis::Vertical, cx));
                }),
            Button::new("close-terminal")
                .icon(gpui_component::IconName::Close)
                .ghost()
                .small()
                .tooltip("关闭终端并停止关联进程")
                .disabled(!closable)
                .on_click(move |_, _, cx| {
                    let _ = close_app.update(cx, |app, cx| app.close_pane(id, cx));
                }),
        ])
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
                let workspace = app.workspaces.get_mut(&project.id)?;
                let pane = workspace
                    .state
                    .file_pane(id)
                    .or_else(|| workspace.state.layout.find_mut(id).cloned())?;
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
