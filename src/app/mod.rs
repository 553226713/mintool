use crate::modules::docker::DockerModule;
use crate::modules::{
    projects::{model::ProjectRegistry, ProjectModule},
    ssh::{SshForm, SshModule},
    ModuleId, Modules, SessionKey,
};
use crate::services::file_monitor::FileMonitor;
use crate::ui::file_preview::FilePreview;
use crate::ui::image_preview::ImagePreview;
use crate::workspace::{dock_workspace::DockWorkspace, layout::Axis, session::Workspace};
use crate::{
    model::settings::Settings,
    modules::projects::model::Project,
    services::{git::GitMonitor, mutex::lock_unpoisoned, storage::Storage},
    ui::{dialogs::Dialog, terminal::TerminalView, theme},
};
use crate::{
    services::directory_browser::DirectoryBrowser,
    ui::directory_picker::{DirectoryPicker, DirectoryPickerEvent},
};
use adabraka_ui::components::{
    input_state::{Enter as InputEnter, Escape as InputEscape, InputEvent, InputState},
    resizable::{h_resizable, resizable_panel, ResizableState},
    select::Select,
};
use adabraka_ui::navigation::menu::MenuItem;
use gpui::*;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

mod directory_result;

pub use directory_result::DirectoryResult;
pub struct MintoolApp {
    pub(crate) storage: Storage,
    pub(crate) settings: Settings,
    pub(crate) modules: Modules,
    pub(crate) workspaces: HashMap<SessionKey, Workspace>,
    pub(crate) dirty_workspaces: HashSet<SessionKey>,
    pub(crate) terminals: HashMap<(SessionKey, u64), Entity<TerminalView>>,
    pub(crate) system_terminals:
        HashMap<(SessionKey, u64), Entity<crate::ui::system_terminal::SystemTerminalView>>,
    pub(crate) terminal_select: Entity<Select<String>>,
    pub(crate) previews: HashMap<(SessionKey, u64), (PathBuf, String, Entity<FilePreview>)>,
    pub(crate) image_previews: HashMap<(SessionKey, u64), (PathBuf, Entity<ImagePreview>)>,
    pub(crate) panel_states: HashMap<String, (Entity<ResizableState>, Subscription)>,
    pub(crate) docks: HashMap<SessionKey, DockWorkspace>,
    pub(crate) dock_splits: Vec<(SessionKey, u64, u64, Axis)>,
    pub(crate) dock_file_tabs: Vec<(SessionKey, u64, u64)>,
    pub(crate) dock_closes: Vec<(SessionKey, u64)>,
    viewport_size: Option<Size<Pixels>>,
    pub(crate) menu: Option<(Point<Pixels>, Vec<MenuItem>)>,
    monitor: GitMonitor,
    pub(crate) dialog_input: Entity<InputState>,
    pub(crate) dialog_focus: FocusHandle,
    pub(crate) font_input: Entity<InputState>,
    pub(crate) dialog_generation: u64,
    pub(crate) pending_directory: Option<DirectoryResult>,
    pub(crate) dialog: Option<Dialog>,
    pub(crate) dialog_error: Option<String>,
    pub(crate) changes: bool,
    pub(crate) focus_pending: bool,
    pub(crate) quit_confirmed: bool,
    pub(crate) error: Option<String>,
    /// When the currently visible toast first appeared. Kept in sync from
    /// `render_toast` so every existing `error = Some(..)` assignment gets the
    /// same auto-dismiss behaviour.
    pub(crate) error_shown: Option<(String, Instant)>,
    pub(crate) toast_hovered: bool,
    pub(crate) dirty: Option<Instant>,
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
        let project_search = cx.new(|cx| InputState::new(cx).placeholder("搜索项目…"));
        let ssh_search = cx.new(|cx| InputState::new(cx).placeholder("搜索 SSH 连接…"));
        let docker_search = cx.new(|cx| InputState::new(cx).placeholder("搜索容器或镜像…"));
        let dialog_input = cx.new(InputState::new);
        let font_input = cx.new(InputState::new);
        let project_group_select = cx.new(Select::new);
        let ssh_group_select = cx.new(Select::new);
        let terminal_select = cx.new(Select::new);
        let (ssh_registry, ssh_error) =
            match crate::modules::ssh::model::SshRegistry::load(&storage) {
                Ok(registry) => (registry, None),
                Err(error) => (
                    crate::modules::ssh::model::SshRegistry::default(),
                    Some(format!("SSH 配置读取失败：{error:#}")),
                ),
            };
        let ssh_form = SshForm {
            name: cx.new(|cx| InputState::new(cx).placeholder("连接名称（可选）")),
            host: cx.new(|cx| InputState::new(cx).placeholder("主机名或 ~/.ssh/config 别名")),
            port: cx.new(|cx| InputState::new(cx).placeholder("22")),
            user: cx.new(|cx| InputState::new(cx).placeholder("用户名（可选）")),
            password: cx
                .new(|cx| InputState::new(cx).placeholder("密码（可选，会明文保存在配置中）")),
            identity: cx.new(|cx| InputState::new(cx).placeholder("私钥路径（可选）")),
            extra: cx.new(|cx| {
                InputState::new(cx).placeholder("高级参数（可选，例如 -L 8080:localhost:80）")
            }),
            color: None,
            testing: false,
            test_status: None,
        };
        let project_subscription = cx.subscribe(&project_search, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let ssh_form_subscriptions: Vec<_> = [
            &ssh_form.name,
            &ssh_form.host,
            &ssh_form.port,
            &ssh_form.user,
            &ssh_form.password,
            &ssh_form.identity,
            &ssh_form.extra,
        ]
        .into_iter()
        .map(|input| {
            cx.subscribe(input, |this, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
                if matches!(event, InputEvent::Enter) {
                    this.submit_dialog(cx);
                }
            })
        })
        .collect();
        let ssh_subscription = cx.subscribe(&ssh_search, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let docker_subscription = cx.subscribe(&docker_search, |_, _, event, cx| {
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
            settings: settings.clone(),
            modules: Modules {
                projects: ProjectModule {
                    registry: projects,
                    search: project_search,
                    group_select: project_group_select,
                    expanded: settings.projects_sidebar,
                },
                ssh: SshModule {
                    registry: ssh_registry,
                    search: ssh_search,
                    group_select: ssh_group_select,
                    form: ssh_form,
                    expanded: settings.ssh_sidebar,
                    opened: None,
                    remote: None,
                    local: None,
                    local_visible: false,
                    transfer_status: None,
                    remote_previews: HashMap::new(),
                },
                docker: DockerModule::new(docker_search, settings.docker_sidebar),
                active: settings.active_module,
            },
            workspaces: HashMap::new(),
            dirty_workspaces: HashSet::new(),
            terminals: HashMap::new(),
            system_terminals: HashMap::new(),
            terminal_select,
            previews: HashMap::new(),
            image_previews: HashMap::new(),
            panel_states: HashMap::new(),
            docks: HashMap::new(),
            dock_splits: Vec::new(),
            dock_file_tabs: Vec::new(),
            dock_closes: Vec::new(),
            viewport_size: None,
            menu: None,
            monitor: GitMonitor::default(),
            dialog_input,
            dialog_focus: cx.focus_handle(),
            font_input,
            dialog_generation: 0,
            pending_directory: None,
            dialog: None,
            dialog_error: None,
            changes: false,
            focus_pending: true,
            quit_confirmed: false,
            error: ssh_error,
            error_shown: None,
            toast_hovered: false,
            dirty: Some(Instant::now()),
            directory_picker: None,
            directory_subscription: None,
            _subscriptions: vec![
                project_subscription,
                ssh_subscription,
                docker_subscription,
                dialog_subscription,
            ]
            .into_iter()
            .chain(ssh_form_subscriptions)
            .collect(),
            file_monitor: FileMonitor::default(),
        }
    }
    pub(crate) fn changed(&mut self, cx: &mut Context<Self>) {
        if let Some(key) = self.active_session_key() {
            if self.workspaces.contains_key(&key) {
                self.dirty_workspaces.insert(key);
            }
        }
        self.dirty = Some(Instant::now());
        cx.notify();
    }
    pub(crate) fn current(&self) -> Option<Project> {
        if self.modules.active != ModuleId::Projects {
            return None;
        }
        let selected = self.modules.projects.registry.selected.as_ref()?;
        self.modules
            .projects
            .registry
            .projects
            .iter()
            .find(|project| &project.id == selected)
            .cloned()
    }
    fn tick(&mut self, cx: &mut Context<Self>) {
        self.sync_docks(cx);
        if !self.toast_hovered
            && self.error.is_some()
            && self
                .error_shown
                .as_ref()
                .is_some_and(|(_, shown)| shown.elapsed() >= crate::shell::toast::TOAST_LIFETIME)
        {
            self.error = None;
            self.error_shown = None;
            cx.notify();
        }
        for terminal in self.system_terminals.values() {
            terminal.update(cx, |view, cx| view.poll(cx));
        }
        let active = self.active_session_key();
        for ((session, _), terminal) in &self.terminals {
            let dirty = {
                let terminal = terminal.read(cx);
                let mut terminal_session = lock_unpoisoned(&terminal.session);
                terminal_session.poll_processes();
                if active.as_ref() == Some(session) {
                    terminal_session.dirty.swap(false, Ordering::Relaxed)
                } else {
                    false
                }
            };
            if dirty {
                terminal.update(cx, |_, cx| cx.notify());
            }
        }
        if let Some(project) = self.current() {
            for (path, git) in self.monitor.poll(&project.path) {
                for candidate in &self.modules.projects.registry.projects {
                    if candidate.path == path {
                        if let Some(workspace) =
                            self.workspaces.get_mut(&SessionKey::project(&candidate.id))
                        {
                            workspace.git = git.clone();
                        }
                    }
                }
                cx.notify();
            }
        }
        if let Some(events) = self.file_monitor.poll() {
            if let Some(id) = self
                .modules
                .projects
                .registry
                .selected
                .clone()
                .filter(|_| self.modules.active == ModuleId::Projects)
            {
                let key = SessionKey::project(id.clone());
                if events.rescan || !events.paths.is_empty() {
                    self.image_previews.retain(|(session, _), (path, _)| {
                        session != &key
                            || !(events.rescan
                                || events.paths.iter().any(|changed| path.starts_with(changed)))
                    });
                }
                if let Some(workspace) = self.workspaces.get_mut(&key) {
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
        self.settings.save(&self.storage)?;
        self.modules.save(&self.storage)?;
        let dirty: Vec<_> = self.dirty_workspaces.drain().collect();
        for key in dirty {
            let Some(workspace) = self.workspaces.get(&key) else {
                continue;
            };
            if let Err(error) = Workspace::save(&self.storage, &key, &workspace.state) {
                self.dirty_workspaces.insert(key);
                return Err(error);
            }
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
                    .modules
                    .projects
                    .registry
                    .projects
                    .iter()
                    .any(|project| project.id != id && project.path == path),
                "这个目录已添加为另一个项目"
            );
            let key = SessionKey::project(&id);
            self.stop_session_terminals(&key, cx);
            if let Some(project) = self
                .modules
                .projects
                .registry
                .projects
                .iter_mut()
                .find(|project| project.id == id)
            {
                let old = std::mem::replace(&mut project.path, path.clone());
                self.terminals.retain(|(session, _), _| session != &key);
                self.system_terminals
                    .retain(|(session, _), _| session != &key);
                self.docks.remove(&key);
                if let Some(workspace) = self.workspaces.get_mut(&key) {
                    workspace.relocate(&old, &path);
                }
            }
        } else {
            let id = self.modules.projects.registry.add_in_group(path, group)?;
            let group = self
                .modules
                .projects
                .registry
                .projects
                .iter()
                .find(|project| project.id == id)
                .and_then(|project| project.group.clone());
            if let Some(group) = group {
                if let Some(group) = self
                    .modules
                    .projects
                    .registry
                    .groups
                    .iter_mut()
                    .find(|candidate| candidate.id == group)
                {
                    group.collapsed = false;
                }
            }
        }
        self.focus_pending = true;
        self.changed(cx);
        Ok(())
    }
    pub(crate) fn remove_project(&mut self, id: &str, cx: &mut Context<Self>) {
        let key = SessionKey::project(id);
        self.stop_session_terminals(&key, cx);
        self.terminals.retain(|(session, _), _| session != &key);
        self.system_terminals
            .retain(|(session, _), _| session != &key);
        self.previews.retain(|(session, _), _| session != &key);
        self.image_previews
            .retain(|(session, _), _| session != &key);
        self.modules
            .ssh
            .remote_previews
            .retain(|(session, _), _| session != &key);
        self.workspaces.remove(&key);
        self.docks.remove(&key);
        self.dock_splits.retain(|(session, ..)| session != &key);
        self.dock_file_tabs.retain(|(session, ..)| session != &key);
        self.dock_closes.retain(|(session, ..)| session != &key);
        self.modules.projects.registry.remove(id);
        self.focus_pending = true;
        self.changed(cx);
    }
    pub(crate) fn open_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.modules.active == ModuleId::Projects {
            if let Some(key) = self.active_session_key() {
                if let Some(workspace) = self.workspaces.get_mut(&key) {
                    let anchor = if workspace
                        .state
                        .layout
                        .find(workspace.state.focused)
                        .is_some()
                        || workspace.state.files.contains_key(&workspace.state.focused)
                    {
                        workspace.state.focused
                    } else {
                        workspace.state.layout.first_id()
                    };
                    let file_id = workspace.state.next_id;
                    workspace.state.next_id = workspace.state.next_id.saturating_add(1);
                    workspace.state.files.insert(file_id, path);
                    self.dock_file_tabs.push((key, anchor, file_id));
                }
            }
        }
        self.changed(cx);
    }
    pub(crate) fn split(&mut self, pane: u64, axis: Axis, cx: &mut Context<Self>) {
        if let Some(key) = self.active_session_key() {
            if let Some(workspace) = self.workspaces.get_mut(&key) {
                let pane = if workspace.state.layout.find(pane).is_some() {
                    pane
                } else {
                    workspace.state.layout.first_id()
                };
                let next = workspace.state.next_id;
                if workspace.state.layout.split(pane, next, axis) {
                    self.dock_splits.push((key.clone(), pane, next, axis));
                    self.panel_states.retain(|key, _| key == "app-panels");
                    workspace.state.next_id += 1;
                    workspace.state.focused = next;
                    self.focus_pending = true;
                }
            }
        }
        self.changed(cx);
    }
}
impl Drop for MintoolApp {
    fn drop(&mut self) {
        crate::services::terminal_processes::TerminalProcesses::shutdown_all();
        if let Err(error) = self.save() {
            eprintln!("配置保存失败：{error:#}");
        }
    }
}
impl Render for MintoolApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let overlays = self.dialog.is_some()
            || self.menu.is_some()
            || self.directory_picker.is_some()
            || cx.has_active_drag();
        let active_session = self.active_session_key();
        for ((session, id), terminal) in &self.system_terminals {
            let selected = active_session.as_ref() == Some(session);
            let active = self
                .docks
                .get(session)
                .and_then(|dock| dock.panels.get(id))
                .is_some_and(|panel| {
                    panel
                        .read(cx)
                        .group
                        .as_ref()
                        .and_then(|group| group.upgrade())
                        .and_then(|group| group.read(cx).active_panel(cx))
                        .is_some_and(|active| active.panel_id(cx) == panel.entity_id())
                });
            let showing_terminal = self
                .workspaces
                .get_mut(session)
                .and_then(|workspace| workspace.state.layout.find_mut(*id))
                .is_some_and(|pane| !pane.showing_file);
            terminal.update(cx, |view, cx| {
                let allowed = !overlays && selected && active && showing_terminal;
                if view.allowed != allowed {
                    view.set_allowed(allowed);
                    cx.notify();
                }
            });
        }
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
        let ssh_auxiliary = self.modules.active == ModuleId::Ssh;
        let docker_auxiliary =
            self.modules.active == ModuleId::Docker && self.modules.docker.opened.is_some();
        let right_width = if ssh_auxiliary {
            self.settings
                .ssh_right_width
                .clamp(360., (f32::from(viewport.width) * 0.55).max(360.))
        } else if docker_auxiliary {
            self.settings
                .docker_right_width
                .clamp(420., (f32::from(viewport.width) * 0.75).max(420.))
        } else {
            right_width
        };
        let docker_resources =
            self.modules.active == ModuleId::Docker && self.modules.docker.show_resources;
        if docker_resources {
            self.ensure_docker_loaded(cx);
        }
        let session = if docker_resources {
            None
        } else {
            self.current_session()
        };
        if let Some(session) = &session {
            self.ensure_session(&session.key);
        }
        // Docker logs/exec live in the auxiliary panel so the resource list
        // stays visible instead of replacing the whole workspace.
        let docker_session = if docker_auxiliary {
            let session = self.modules.docker_session();
            if let Some(session) = &session {
                self.ensure_session(&session.key);
            }
            session
        } else {
            None
        };
        let navigation = match self.modules.active {
            ModuleId::Projects => self.render_navigation(window, cx),
            ModuleId::Ssh => self.render_ssh_sidebar(window, cx),
            ModuleId::Docker => self.render_docker_sidebar(window, cx),
        };
        let workspace = if docker_resources {
            self.render_docker_resources(cx)
        } else if let Some(session) = &session {
            let project_missing = session.capabilities.files
                && self.current().is_some_and(|project| !project.path.is_dir());
            if project_missing {
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(theme::muted())
                    .child("项目目录不存在，请右键项目重新定位")
                    .into_any_element()
            } else if let Some(workspace) = self.workspaces.get(&session.key) {
                let layout = workspace.state.layout.clone();
                self.render_layout(session, &layout, window, cx)
            } else {
                div().into_any_element()
            }
        } else {
            self.empty_state(cx)
        };
        self.synchronize_file_watches();
        if self.modules.active == ModuleId::Ssh {
            self.ensure_remote_browser();
        }
        let auxiliary = match self.modules.active {
            ModuleId::Projects => true,
            ModuleId::Ssh => self.modules.ssh.opened.is_some(),
            ModuleId::Docker => docker_auxiliary,
        };
        let explorer = if auxiliary {
            Some(match self.modules.active {
                ModuleId::Projects => self.render_explorer(cx),
                ModuleId::Ssh => self.render_ssh_explorer(cx),
                ModuleId::Docker => match &docker_session {
                    Some(session) => self.render_docker_session_panel(session, window, cx),
                    None => div().into_any_element(),
                },
            })
        } else {
            None
        };
        let sidebar_visible = self.modules.expanded(self.modules.active);
        let panels_key = format!(
            "app-panels-{}-{sidebar_visible}-{auxiliary}",
            self.modules.active.slug()
        );
        let state = self.resize_state(
            panels_key.clone(),
            sidebar_visible,
            auxiliary,
            ssh_auxiliary,
            docker_auxiliary,
            cx,
        );
        let mut panels = h_resizable(SharedString::from(panels_key), state);
        if sidebar_visible {
            panels = panels.child(
                resizable_panel()
                    .size(px(left_width))
                    .size_range(px(180.)..px((f32::from(viewport.width) * 0.25).min(400.)))
                    .flex_none()
                    .overflow_hidden()
                    .child(navigation),
            );
        }
        panels = panels.child(
            resizable_panel()
                .size_range(px(250.)..px(10000.))
                .flex_1()
                .overflow_hidden()
                .child(workspace),
        );
        if let Some(explorer) = explorer {
            let (min_right, max_right) = if ssh_auxiliary {
                (px(360.), px((f32::from(viewport.width) * 0.60).max(360.)))
            } else if docker_auxiliary {
                (px(420.), px((f32::from(viewport.width) * 0.75).max(420.)))
            } else {
                (px(200.), px((f32::from(viewport.width) * 0.30).min(560.)))
            };
            panels = panels.child(
                resizable_panel()
                    .size(px(right_width))
                    .size_range(min_right..max_right)
                    .flex_none()
                    .overflow_hidden()
                    .child(explorer),
            );
        }
        let body = div()
            .size_full()
            .flex()
            .child(self.render_activity_bar(cx))
            .child(div().flex_1().min_w_0().h_full().child(panels));
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
            .child(div().flex_1().min_h_0().child(body))
            .child(status)
            .children(dialog)
            .children(self.directory_picker.clone())
            .children(menu)
            .children(self.render_toast(cx))
    }
}
