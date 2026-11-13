use crate::services::file_monitor::FileMonitor;
use crate::ui::file_preview::FilePreview;
use crate::{
    model::{
        layout::Axis,
        project::{Project, ProjectRegistry},
        settings::Settings,
    },
    services::{
        git::GitMonitor, storage::Storage, terminal::TerminalSession, workspace::Workspace,
    },
    ui::{dialogs::Dialog, terminal::TerminalView, theme},
};
use crate::{
    services::directory_browser::DirectoryBrowser,
    ui::directory_picker::{DirectoryPicker, DirectoryPickerEvent},
};
use adabraka_ui::components::{
    button::{Button, ButtonSize, ButtonVariant},
    icon::Icon,
};
use adabraka_ui::components::{
    input_state::{Enter as InputEnter, Escape as InputEscape, InputEvent, InputState},
    resizable::{h_resizable, resizable_panel, ResizableState},
    select::Select,
};
use adabraka_ui::navigation::menu::MenuItem;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{atomic::Ordering, Arc, Mutex},
    time::{Duration, Instant},
};

pub struct DirectoryResult {
    pub path: PathBuf,
    pub generation: u64,
}
pub struct MintoolApp {
    pub(crate) storage: Storage,
    pub(crate) settings: Settings,
    pub(crate) projects: ProjectRegistry,
    pub(crate) workspaces: HashMap<String, Workspace>,
    pub(crate) terminals: HashMap<(String, u64), Entity<TerminalView>>,
    pub(crate) previews: HashMap<(String, u64), (PathBuf, String, Entity<FilePreview>)>,
    pub(crate) panel_states: HashMap<String, (Entity<ResizableState>, Subscription)>,
    viewport_size: Option<Size<Pixels>>,
    pub(crate) menu: Option<(Point<Pixels>, Vec<MenuItem>)>,
    monitor: GitMonitor,
    pub(crate) search: Entity<InputState>,
    pub(crate) dialog_input: Entity<InputState>,
    pub(crate) font_input: Entity<InputState>,
    pub(crate) group_select: Entity<Select<Option<String>>>,
    pub(crate) dialog_generation: u64,
    pub(crate) pending_directory: Option<DirectoryResult>,
    pub(crate) dialog: Option<Dialog>,
    pub(crate) dialog_error: Option<String>,
    pub(crate) changes: bool,
    pub(crate) focus_pending: bool,
    pub(crate) error: Option<String>,
    dirty: Option<Instant>,
    directory_picker: Option<Entity<DirectoryPicker>>,
    directory_subscription: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
    pub(crate) file_monitor: FileMonitor,
}
impl MintoolApp {
    pub fn new(
        storage: Storage,
        mut settings: Settings,
        projects: ProjectRegistry,
        cx: &mut Context<Self>,
    ) -> Self {
        settings.font_size = settings.font_size.clamp(10., 28.);
        let search = cx.new(|cx| InputState::new(cx).placeholder("搜索项目…"));
        let dialog_input = cx.new(InputState::new);
        let font_input = cx.new(InputState::new);
        let group_select = cx.new(Select::new);
        let subscription = cx.subscribe(&search, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let dialog_subscription = cx.subscribe(&dialog_input, |this, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
            if matches!(event, InputEvent::Enter) {
                this.submit_dialog(cx);
            }
        });
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| loop {
            executor.timer(Duration::from_millis(33)).await;
            if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                break;
            }
        })
        .detach();
        Self {
            storage,
            settings,
            projects,
            workspaces: HashMap::new(),
            terminals: HashMap::new(),
            previews: HashMap::new(),
            panel_states: HashMap::new(),
            viewport_size: None,
            menu: None,
            monitor: GitMonitor::default(),
            search,
            dialog_input,
            font_input,
            group_select,
            dialog_generation: 0,
            pending_directory: None,
            dialog: None,
            dialog_error: None,
            changes: false,
            focus_pending: true,
            error: None,
            dirty: Some(Instant::now()),
            directory_picker: None,
            directory_subscription: None,
            _subscriptions: vec![subscription, dialog_subscription],
            file_monitor: FileMonitor::default(),
        }
    }
    pub(crate) fn changed(&mut self, cx: &mut Context<Self>) {
        self.dirty = Some(Instant::now());
        cx.notify();
    }
    pub(crate) fn current(&self) -> Option<Project> {
        self.projects
            .projects
            .iter()
            .find(|p| Some(&p.id) == self.projects.selected.as_ref())
            .cloned()
    }
    fn tick(&mut self, cx: &mut Context<Self>) {
        for terminal in self.terminals.values() {
            let dirty = terminal
                .read(cx)
                .session
                .lock()
                .unwrap()
                .dirty
                .swap(false, Ordering::Relaxed);
            if dirty {
                terminal.update(cx, |_, cx| cx.notify());
            }
        }
        if let Some(project) = self.current() {
            for (path, git) in self.monitor.poll(&project.path) {
                for p in &self.projects.projects {
                    if p.path == path {
                        if let Some(w) = self.workspaces.get_mut(&p.id) {
                            w.git = git.clone();
                        }
                    }
                }
                cx.notify();
            }
        }
        if let Some(events) = self.file_monitor.poll() {
            if let Some(id) = &self.projects.selected {
                if let Some(workspace) = self.workspaces.get_mut(id) {
                    if events.rescan {
                        workspace.cache.invalidate();
                    } else {
                        workspace.cache.invalidate_paths(&events.paths);
                    }
                }
            }
            if let Some(error) = events.error {
                self.error = Some(error);
            }
            cx.notify();
        }
        if self
            .dirty
            .is_some_and(|time| time.elapsed() >= Duration::from_millis(600))
        {
            if let Err(error) = self.save() {
                self.error = Some(format!("配置保存失败：{error:#}"));
                self.dirty = None;
                cx.notify();
            }
        }
    }
    fn save(&mut self) -> anyhow::Result<()> {
        self.storage.save(&self.settings, &self.projects)?;
        for (id, workspace) in &self.workspaces {
            self.storage
                .write(&self.storage.workspace_path(id)?, &workspace.state)?;
        }
        self.dirty = None;
        Ok(())
    }
    pub(crate) fn choose_directory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/"));
        let entered = PathBuf::from(self.dialog_input.read(cx).content().trim());
        let initial = if entered.is_dir() {
            entered
        } else if let Some(parent) = entered.parent().filter(|p| p.is_dir()) {
            parent.to_path_buf()
        } else {
            home
        };
        let browser = match DirectoryBrowser::open(&initial) {
            Ok(browser) => browser,
            Err(error) => {
                self.dialog_error = Some(format!("{error:#}"));
                cx.notify();
                return;
            }
        };
        let generation = self.dialog_generation;
        let picker = cx.new(|cx| DirectoryPicker::new(browser, window, cx));
        self.directory_subscription = Some(cx.subscribe(&picker, move |this, _, event, cx| {
            if let DirectoryPickerEvent::Selected(path) = event {
                if generation == this.dialog_generation && this.dialog.is_some() {
                    this.pending_directory = Some(DirectoryResult {
                        path: path.clone(),
                        generation,
                    });
                }
            }
            this.directory_picker = None;
            this.directory_subscription = None;
            cx.notify();
        }));
        self.directory_picker = Some(picker);
        cx.notify();
    }
    pub(crate) fn add_path(
        &mut self,
        path: PathBuf,
        relocate: Option<String>,
        group: Option<String>,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        if let Some(id) = relocate {
            let path = path.canonicalize()?;
            anyhow::ensure!(path.is_dir(), "请选择目录");
            anyhow::ensure!(
                !self
                    .projects
                    .projects
                    .iter()
                    .any(|p| p.id != id && p.path == path),
                "这个目录已添加为另一个项目"
            );
            if let Some(project) = self.projects.projects.iter_mut().find(|p| p.id == id) {
                let old = std::mem::replace(&mut project.path, path.clone());
                self.terminals.retain(|(project, _), _| project != &id);
                if let Some(workspace) = self.workspaces.get_mut(&id) {
                    workspace.relocate(&old, &path);
                }
            }
        } else {
            let id = self.projects.add_in_group(path, group)?;
            if let Some(group) = self
                .projects
                .projects
                .iter()
                .find(|p| p.id == id)
                .and_then(|p| p.group.as_ref())
            {
                if let Some(group) = self.projects.groups.iter_mut().find(|g| &g.id == group) {
                    group.collapsed = false;
                }
            }
        }
        self.focus_pending = true;
        self.changed(cx);
        Ok(())
    }
    pub(crate) fn select_project(&mut self, id: String, cx: &mut Context<Self>) {
        let group = self
            .projects
            .projects
            .iter()
            .find(|p| p.id == id)
            .and_then(|p| p.group.as_ref());
        if let Some(group) = self
            .projects
            .groups
            .iter_mut()
            .find(|g| Some(&g.id) == group)
        {
            group.collapsed = false;
        }
        self.projects.selected = Some(id);
        self.focus_pending = true;
        self.changed(cx);
    }
    pub(crate) fn remove_project(&mut self, id: &str, cx: &mut Context<Self>) {
        self.terminals.retain(|(project, _), _| project != id);
        self.previews.retain(|(project, _), _| project != id);
        self.workspaces.remove(id);
        self.projects.remove(id);
        self.focus_pending = true;
        self.changed(cx);
    }
    pub(crate) fn open_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(id) = self.projects.selected.as_ref() {
            if let Some(w) = self.workspaces.get_mut(id) {
                if let Some(p) = w.state.layout.find_mut(w.state.focused) {
                    self.previews.remove(&(id.clone(), w.state.focused));
                    p.file = Some(path);
                    p.showing_file = true;
                }
            }
        }
        self.changed(cx);
    }
    pub(crate) fn split(&mut self, pane: u64, axis: Axis, cx: &mut Context<Self>) {
        if let Some(id) = self.projects.selected.as_ref() {
            if let Some(w) = self.workspaces.get_mut(id) {
                let next = w.state.next_id;
                if w.state.layout.split(pane, next, axis) {
                    w.state.next_id += 1;
                    w.state.focused = next;
                    self.focus_pending = true;
                }
            }
        }
        self.changed(cx);
    }
    pub(crate) fn close_pane(&mut self, pane: u64, cx: &mut Context<Self>) {
        if let Some(id) = self.projects.selected.as_ref() {
            if let Some(w) = self.workspaces.get_mut(id) {
                if w.state.layout.close(pane) {
                    w.terminals.remove(&pane);
                    w.errors.remove(&pane);
                    self.terminals.remove(&(id.clone(), pane));
                    if w.state.focused == pane {
                        w.state.focused = w.state.layout.first_id();
                    }
                }
            }
        }
        self.changed(cx);
    }
    pub(crate) fn terminal(
        &mut self,
        project: &Project,
        id: u64,
        cx: &mut Context<Self>,
    ) -> Option<Entity<TerminalView>> {
        let key = (project.id.clone(), id);
        if let Some(terminal) = self.terminals.get(&key) {
            return Some(terminal.clone());
        }
        let workspace = self.workspaces.get_mut(&project.id)?;
        if workspace.errors.contains_key(&id) {
            return None;
        }
        let session = match TerminalSession::spawn(&project.path, &self.settings.shell) {
            Ok(session) => Arc::new(Mutex::new(session)),
            Err(error) => {
                workspace.errors.insert(id, format!("{error:#}"));
                return None;
            }
        };
        workspace.terminals.insert(id, session.clone());
        let terminal = cx.new(|cx| TerminalView::new(session, self.settings.font_size, cx));
        self.terminals.insert(key, terminal.clone());
        Some(terminal)
    }
    fn toolbar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.current();
        div()
            .flex()
            .items_center()
            .px_5()
            .gap_3()
            .flex_none()
            .h(px(62.))
            .bg(theme::panel())
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .child(
                        div()
                            .size(px(25.))
                            .rounded_md()
                            .bg(theme::accent())
                            .text_color(theme::background())
                            .flex()
                            .items_center()
                            .justify_center()
                            .font_weight(FontWeight::BOLD)
                            .child("M"),
                    )
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Mintool"))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme::muted())
                            .child(format!("v{}", crate::VERSION)),
                    )
                    .child(div().h(px(16.)).w(px(1.)).bg(theme::border()).mx_2())
                    .child(
                        div()
                            .text_color(theme::muted())
                            .text_size(px(12.))
                            .child("工作空间"),
                    )
                    .child(
                        Icon::new("chevron-right")
                            .size(px(12.))
                            .text_color(theme::muted()),
                    )
                    .child(
                        div().min_w_0().truncate().child(
                            current
                                .as_ref()
                                .map(|p| p.name.clone())
                                .unwrap_or_else(|| "欢迎".into()),
                        ),
                    ),
            )
            .child(
                Button::new("toggle-theme", self.settings.appearance.label())
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .icon(self.settings.appearance.icon())
                    .tooltip(format!(
                        "切换为{}主题",
                        self.settings.appearance.toggle().label()
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings.appearance = this.settings.appearance.toggle();
                        theme::install(cx, this.settings.appearance);
                        this.changed(cx);
                    })),
            )
            .child(
                Button::new("settings", "")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .icon("settings-2")
                    .tooltip("设置")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.show_dialog(Dialog::Settings, window, cx)
                    })),
            )
    }
    fn status_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        let project = self.current();
        let workspace = project.as_ref().and_then(|p| self.workspaces.get(&p.id));
        div()
            .h(px(29.))
            .flex_none()
            .px_4()
            .flex()
            .items_center()
            .gap_3()
            .text_size(px(11.))
            .text_color(theme::muted())
            .border_t_1()
            .border_color(theme::border())
            .bg(theme::panel())
            .child(div().size(px(6.)).rounded_full().bg(theme::success()))
            .child(
                div().flex_1().min_w_0().truncate().child(
                    project
                        .as_ref()
                        .map(|p| p.path.display().to_string())
                        .unwrap_or_else(|| "就绪".into()),
                ),
            )
            .when_some(workspace.filter(|w| w.git.root.is_some()), |bar, w| {
                bar.child(Icon::new("git-branch").size(px(13.)))
                    .child(
                        div()
                            .text_color(theme::accent())
                            .child(w.git.branch.clone()),
                    )
                    .child(format!("{} 项变更", w.git.changes.len()))
            })
            .child(format!(
                "{} 个终端",
                workspace.map_or(0, |w| w.terminals.len())
            ))
            .child("UTF-8")
            .child("Adabraka UI")
            .map(|bar| {
                let _ = cx;
                bar
            })
    }
}
impl Drop for MintoolApp {
    fn drop(&mut self) {
        if let Err(error) = self.save() {
            eprintln!("配置保存失败：{error:#}");
        }
    }
}
impl Render for MintoolApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(result) = self.pending_directory.take() {
            if result.generation == self.dialog_generation && self.dialog.is_some() {
                self.dialog_input.update(cx, |state, cx| {
                    state.select_all(&adabraka_ui::components::input_state::SelectAll, window, cx);
                    state.set_value(result.path.display().to_string(), window, cx);
                });
            }
        }
        let viewport = window.viewport_size();
        if self.viewport_size != Some(viewport) {
            // Reapply saved split ratios to the new available space. The component
            // caches pixel sizes, which otherwise grow every panel equally.
            self.panel_states.clear();
            self.viewport_size = Some(viewport);
        }
        let (left_width, right_width) = self.settings.panel_widths(viewport.width.into());
        let current = self.current();
        if let Some(project) = &current {
            if !self.workspaces.contains_key(&project.id) {
                match self.storage.workspace(&project.id) {
                    Ok(state) => {
                        self.workspaces
                            .insert(project.id.clone(), Workspace::new(state));
                    }
                    Err(error) => {
                        self.error = Some(format!("{error:#}"));
                        self.projects.selected = None;
                    }
                }
            }
        }
        let navigation = self.render_navigation(window, cx);
        let workspace = if let Some(project) = &current {
            if !project.path.is_dir() {
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(theme::muted())
                    .child("项目目录不存在，请右键项目重新定位")
                    .into_any_element()
            } else if let Some(w) = self.workspaces.get(&project.id) {
                let layout = w.state.layout.clone();
                self.render_layout(project, &layout, window, cx)
            } else {
                div().into_any_element()
            }
        } else {
            div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    Icon::new("square-terminal")
                        .size(px(46.))
                        .text_color(theme::accent()),
                )
                .child(
                    div()
                        .text_size(px(24.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("每个项目，都有自己的工作现场"),
                )
                .child(
                    div()
                        .text_color(theme::muted())
                        .child("独立终端 · 自由分屏 · Git 文件树"),
                )
                .child(
                    Button::new("first-project", "添加项目目录")
                        .variant(ButtonVariant::Default)
                        .icon("plus")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.show_dialog(Dialog::AddProject(None), window, cx)
                        })),
                )
                .into_any_element()
        };
        self.synchronize_file_watches();
        let explorer = self.render_explorer(cx);
        let state = self.resize_state("app-panels".into(), None, cx);
        let body = h_resizable("app-panels", state)
            .child(
                resizable_panel()
                    .size(px(left_width))
                    .size_range(px(180.)..px((f32::from(viewport.width) * 0.25).min(400.)))
                    .flex_none()
                    .overflow_hidden()
                    .child(navigation),
            )
            .child(
                resizable_panel()
                    .size_range(px(250.)..px(10000.))
                    .flex_1()
                    .overflow_hidden()
                    .child(workspace),
            )
            .child(
                resizable_panel()
                    .size(px(right_width))
                    .size_range(px(200.)..px((f32::from(viewport.width) * 0.30).min(560.)))
                    .flex_none()
                    .overflow_hidden()
                    .child(explorer),
            );
        let toolbar = self.toolbar(cx);
        let status = self.status_bar(cx);
        let dialog = self.render_dialog(cx);
        let menu = self.render_context_menu(cx);
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(theme::background())
            .text_color(theme::text())
            .text_size(px(13.))
            .font_family("Inter")
            .capture_action(cx.listener(|this, _: &InputEscape, _, cx| {
                if this.directory_picker.is_some() {
                    this.directory_picker = None;
                    this.directory_subscription = None;
                    cx.notify();
                    cx.stop_propagation();
                } else if this.dialog.is_some() || this.menu.is_some() {
                    this.dialog = None;
                    this.menu = None;
                    cx.notify();
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            }))
            .capture_action(cx.listener(|this, _: &InputEnter, _, cx| {
                if this.directory_picker.is_some() {
                    cx.propagate();
                } else if this.dialog.is_some() {
                    this.submit_dialog(cx);
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            }))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if this.directory_picker.is_some() && event.keystroke.key == "escape" {
                    this.directory_picker = None;
                    this.directory_subscription = None;
                    cx.notify();
                    cx.stop_propagation();
                } else if this.dialog.is_some() && event.keystroke.key == "escape" {
                    this.dialog = None;
                    cx.notify();
                    cx.stop_propagation();
                } else if this.menu.is_some() {
                    if event.keystroke.key == "escape" {
                        this.menu = None;
                        cx.notify();
                    }
                    cx.stop_propagation();
                }
            }))
            .child(toolbar)
            .when_some(self.error.clone(), |root, error| {
                root.child(
                    div()
                        .px_4()
                        .py_2()
                        .bg(theme::error_background())
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(div().flex_1().child(error))
                        .child(
                            Button::new("dismiss-error", "")
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Sm)
                                .icon("x")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.error = None;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(div().flex_1().min_h_0().child(body))
            .child(status)
            .children(dialog)
            .children(self.directory_picker.clone())
            .children(menu)
    }
}
