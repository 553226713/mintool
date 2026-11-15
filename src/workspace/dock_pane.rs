use crate::app::MintoolApp;
use crate::model::session::SessionView;
use crate::workspace::layout::Axis;
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
    pub session: SessionView,
    pub id: u64,
    pub group: Option<WeakEntity<TabPanel>>,
    pub count: Arc<AtomicUsize>,
    focus: FocusHandle,
    bounds: Size<Pixels>,
}

impl DockPane {
    pub fn new(
        app: WeakEntity<MintoolApp>,
        session: SessionView,
        id: u64,
        count: Arc<AtomicUsize>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            app,
            session,
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
        let key = self.session.key.clone();
        let middle_key = key.clone();
        let middle_app = app.clone();
        let (closed, file_name, query_name, is_document, showing_document) =
            app.upgrade()
                .map_or((false, None, None, false, false), |app| {
                    let app = app.read(cx);
                    let Some(workspace) = app.workspaces.get(&key) else {
                        return (false, None, None, false, false);
                    };
                    let remote_path = workspace.state.remote_files.get(&id);
                    let is_file = workspace.state.files.contains_key(&id) || remote_path.is_some();
                    let is_query = workspace.state.queries.contains_key(&id);
                    let file_name = workspace
                        .state
                        .files
                        .get(&id)
                        .and_then(|path| path.file_name())
                        .map(|name| name.to_string_lossy().into_owned())
                        .or_else(|| {
                            remote_path.map(|path| {
                                path.rsplit('/').next().unwrap_or(path.as_str()).to_string()
                            })
                        });
                    let query_name = workspace
                        .state
                        .queries
                        .get(&id)
                        .map(|query| query.title.clone());
                    let pane = workspace.state.layout.find(id);
                    let showing_file = pane.is_some_and(|pane| pane.showing_file);
                    let showing_document = showing_file || is_query;
                    let closed =
                        is_file || (!is_query && pane.is_none_or(|pane| pane.terminal_closed));
                    (
                        closed,
                        file_name,
                        query_name,
                        is_file || is_query,
                        showing_document,
                    )
                });
        let name = if let Some(name) = file_name.or(query_name) {
            name
        } else if closed {
            "空面板".to_string()
        } else {
            format!("终端 {id}")
        };
        let close_app = app.clone();
        let close_button = (!showing_document || is_document).then(|| {
            Button::new(SharedString::from(format!(
                "tab-close-{}-{id}",
                self.session.key.module.slug()
            )))
            .icon(gpui_component::IconName::Close)
            .ghost()
            .xsmall()
            .tab_stop(false)
            .tooltip(if is_document {
                "关闭文档"
            } else {
                "关闭终端并停止关联进程"
            })
            .on_click(move |_, window, cx| {
                let _ =
                    close_app.update(cx, |app, cx| app.request_close_pane(&key, id, window, cx));
            })
        });
        div()
            .id(SharedString::from(format!(
                "terminal-tab-{}-{id}",
                self.session.key.module.slug()
            )))
            .h_full()
            .flex()
            .items_center()
            .gap_1()
            .child(name)
            .children(close_button)
            .on_mouse_down(MouseButton::Middle, move |_, window, cx| {
                let _ = middle_app.update(cx, |app, cx| {
                    app.request_close_pane(&middle_key, id, window, cx)
                });
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
        let (app, key, id) = (self.app.clone(), self.session.key.clone(), self.id);
        window.defer(cx, move |_, cx| {
            let _ = app.update(cx, |app, cx| {
                if let Some(workspace) = app.workspaces.get_mut(&key) {
                    workspace.state.focused = id;
                }
                app.focus_pending = true;
                app.changed(cx);
            });
        });
    }
    fn toolbar_buttons(&mut self, _: &mut Window, cx: &mut Context<Self>) -> Option<Vec<Button>> {
        let app = self.app.upgrade()?;
        let key = self.session.key.clone();
        if app.read(cx).workspaces.get(&key).is_some_and(|workspace| {
            workspace.state.files.contains_key(&self.id)
                || workspace.state.remote_files.contains_key(&self.id)
                || workspace.state.queries.contains_key(&self.id)
        }) {
            return None;
        }
        let showing_file = app
            .read(cx)
            .workspaces
            .get(&key)
            .and_then(|workspace| workspace.state.layout.find(self.id))
            .is_some_and(|pane| pane.showing_file);
        if showing_file {
            return None;
        }

        let link_app = self.app.clone();
        let split_app = self.app.clone();
        let id = self.id;
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
        let stored = self.session.clone();
        let key = stored.key.clone();
        let id = self.id;
        let bounds = (f32::from(self.bounds.width), f32::from(self.bounds.height));
        let content = self
            .app
            .update(cx, |app, cx| {
                let session = app.session_view(&key).unwrap_or(stored);
                let workspace = app.workspaces.get_mut(&key)?;
                let pane = workspace
                    .state
                    .file_pane(id)
                    .or_else(|| workspace.state.layout.find_mut(id).cloned())?;
                Some(app.render_pane(&session, &pane, bounds, window, cx))
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
