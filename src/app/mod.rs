use crate::modules::docker::DockerModule;
use crate::modules::ssh::remote_edit::RemoteEdit;
use crate::modules::{
    database::{model::DatabaseRegistry, DatabaseModule},
    projects::{model::ProjectRegistry, quick_open::QuickOpen, ProjectModule},
    ssh::{SshForm, SshModule},
    ModuleId, Modules, SessionKey,
};
use crate::services::file_monitor::FileMonitor;
use crate::shell::command_palette::CommandPalette;
use crate::ui::file_preview::FilePreview;
use crate::ui::image_preview::ImagePreview;
use crate::workspace::{dock_workspace::DockWorkspace, layout::Axis, session::Workspace};
use crate::{
    model::settings::Settings,
    modules::projects::model::Project,
    services::{git::GitMonitor, mutex::lock_unpoisoned, storage::Storage},
    ui::{
        dialogs::{ConflictDialog, Dialog},
        terminal::TerminalView,
    },
};
use crate::{
    services::directory_browser::DirectoryBrowser,
    ui::directory_picker::{DirectoryPicker, DirectoryPickerEvent, DirectoryPickerMode},
};
use adabraka_ui::components::{
    input_state::{InputEvent, InputState},
    resizable::ResizableState,
    select::{Select, SelectOption},
};
use adabraka_ui::navigation::menu::MenuItem;
use gpui::*;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

mod directory_result;
mod render;

pub use directory_result::{DirectoryResult, DirectoryTarget};
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
    pub(crate) monitor: GitMonitor,
    pub(crate) dialog_input: Entity<InputState>,
    pub(crate) dialog_focus: FocusHandle,
    pub(crate) font_input: Entity<InputState>,
    pub(crate) refresh_input: Entity<InputState>,
    pub(crate) scrollback_input: Entity<InputState>,
    pub(crate) dialog_generation: u64,
    pub(crate) pending_directory: Option<DirectoryResult>,
    pub(crate) dialog: Option<Dialog>,
    pub(crate) dialog_error: Option<String>,
    /// External-change prompts waiting to be shown, one at a time.
    pub(crate) pending_conflicts: VecDeque<ConflictDialog>,
    pub(crate) quick_open: Option<QuickOpen>,
    pub(crate) quick_open_subscription: Option<Subscription>,
    pub(crate) command_palette: Option<CommandPalette>,
    pub(crate) command_palette_subscription: Option<Subscription>,
    pub(crate) notice_history: VecDeque<(String, Instant)>,
    pub(crate) remote_edit: Option<RemoteEdit>,
    transfer_tick: Option<Instant>,
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        settings.font_size = settings.font_size.clamp(10., 28.);
        if settings.active_module == ModuleId::Database {
            settings.active_module = ModuleId::Projects;
        }
        crate::services::terminal::set_scrollback(settings.terminal_scrollback);
        let project_search = cx
            .new(|cx| gpui_component::input::InputState::new(window, cx).placeholder("搜索项目…"));
        let commit_input = cx
            .new(|cx| gpui_component::input::InputState::new(window, cx).placeholder("提交说明…"));
        let file_filter = cx
            .new(|cx| gpui_component::input::InputState::new(window, cx).placeholder("筛选文件…"));
        let rename_input = cx.new(InputState::new);
        let ssh_search = cx.new(|cx| InputState::new(cx).placeholder("搜索 SSH 连接…"));
        let docker_search = cx.new(|cx| InputState::new(cx).placeholder("搜索容器或镜像…"));
        let database_search = cx.new(|cx| InputState::new(cx).placeholder("搜索数据库连接…"));
        let database_engine_select = cx.new(|cx| {
            Select::new(cx)
                .options(
                    crate::modules::database::model::DatabaseEngine::ALL
                        .into_iter()
                        .map(|engine| SelectOption::new(engine, engine.label()))
                        .collect(),
                )
                .selected_index(Some(0))
        });
        let database_group_select = cx.new(Select::new);
        let database_form = crate::modules::database::form::DatabaseForm {
            engine: database_engine_select,
            group: database_group_select,
            name: cx.new(|cx| InputState::new(cx).placeholder("连接名称")),
            path: cx.new(|cx| InputState::new(cx).placeholder("~/data/app.sqlite")),
            host: cx.new(|cx| InputState::new(cx).placeholder("127.0.0.1")),
            port: cx.new(|cx| InputState::new(cx).placeholder("5432 / 3306")),
            database: cx.new(|cx| InputState::new(cx).placeholder("数据库名")),
            user: cx.new(|cx| InputState::new(cx).placeholder("用户名")),
            password: cx.new(|cx| InputState::new(cx).placeholder("密码（可选）")),
            testing: false,
            test_status: None,
        };
        let docker_run_form = crate::modules::docker::DockerRunForm {
            name: cx.new(|cx| InputState::new(cx).placeholder("容器名称（可选）")),
            ports: cx.new(|cx| InputState::new(cx).placeholder("端口映射，例如 8080:80")),
            env: cx.new(|cx| InputState::new(cx).placeholder("环境变量，例如 TZ=Asia/Shanghai")),
            mounts: cx.new(|cx| InputState::new(cx).placeholder("挂载，例如 /tmp:/data")),
            command: cx.new(|cx| InputState::new(cx).placeholder("启动命令（可选）")),
        };
        let dialog_input = cx.new(InputState::new);
        let font_input = cx.new(InputState::new);
        let refresh_input = cx.new(InputState::new);
        let scrollback_input = cx.new(InputState::new);
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
        let (database_registry, database_error) = match DatabaseRegistry::load(&storage) {
            Ok(registry) => (registry, None),
            Err(error) => (
                DatabaseRegistry::default(),
                Some(format!("数据库配置读取失败：{error:#}")),
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
                InputState::new(cx).placeholder("高级参数（可选，例如 -o ProxyJump=bastion）")
            }),
            forwards: cx.new(|cx| {
                InputState::new(cx).placeholder(
                    "端口转发（可选，例如 8080:localhost:80 R:9000:localhost:9000 D:1080）",
                )
            }),
            color: None,
            use_keyring: false,
            testing: false,
            test_status: None,
        };
        let project_subscription = cx.subscribe(&project_search, |_, _, event, cx| {
            if matches!(event, gpui_component::input::InputEvent::Change) {
                cx.notify();
            }
        });
        let commit_subscription = cx.subscribe(&commit_input, |_, _, event, cx| {
            if matches!(event, gpui_component::input::InputEvent::Change) {
                cx.notify();
            }
        });
        let filter_subscription = cx.subscribe(&file_filter, |_, _, event, cx| {
            if matches!(event, gpui_component::input::InputEvent::Change) {
                cx.notify();
            }
        });
        let rename_subscription = cx.subscribe(&rename_input, |this, _, event, cx| {
            if matches!(event, InputEvent::Enter) {
                this.commit_inline_rename(cx);
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
            &ssh_form.forwards,
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
        let database_subscription = cx.subscribe(&database_search, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let database_engine_subscription =
            cx.subscribe(&database_form.engine, |_, _, _, cx| cx.notify());
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
        let ssh_browser_preferences =
            crate::modules::ssh::preferences::load(&storage).unwrap_or_default();
        Self {
            storage,
            settings: settings.clone(),
            modules: Modules {
                projects: ProjectModule {
                    registry: projects,
                    search: project_search,
                    group_select: project_group_select,
                    commit_input,
                    file_filter,
                    rename_input,
                    selected_files: Default::default(),
                    renaming: None,
                    show_archived: false,
                    sort_recent: false,
                    filter_cache: None,
                    expanded: settings.projects_sidebar,
                },
                ssh: SshModule {
                    registry: ssh_registry,
                    search: ssh_search,
                    group_select: ssh_group_select,
                    form: ssh_form,
                    expanded: settings.ssh_sidebar,
                    opened: None,
                    preferences: ssh_browser_preferences,
                    transfers: Vec::new(),
                    active_panel: None,
                    panels: HashMap::new(),
                    remote_previews: HashMap::new(),
                    pane_splits: HashMap::new(),
                },
                docker: DockerModule::new(docker_search, settings.docker_sidebar, docker_run_form),
                database: DatabaseModule::with_form(
                    database_registry,
                    database_search,
                    database_form,
                    settings.database_sidebar,
                ),
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
            refresh_input,
            scrollback_input,
            dialog_generation: 0,
            pending_directory: None,
            dialog: None,
            dialog_error: None,
            pending_conflicts: VecDeque::new(),
            quick_open: None,
            quick_open_subscription: None,
            command_palette: None,
            command_palette_subscription: None,
            notice_history: VecDeque::new(),
            remote_edit: None,
            transfer_tick: None,
            changes: false,
            focus_pending: true,
            quit_confirmed: false,
            error: ssh_error.or(database_error),
            error_shown: None,
            toast_hovered: false,
            dirty: Some(Instant::now()),
            directory_picker: None,
            directory_subscription: None,
            _subscriptions: vec![
                project_subscription,
                commit_subscription,
                filter_subscription,
                rename_subscription,
                ssh_subscription,
                docker_subscription,
                database_subscription,
                database_engine_subscription,
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
    pub(crate) fn keep_subscription(&mut self, subscription: Subscription) {
        self._subscriptions.push(subscription);
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
            self.modules.projects.filter_cache = None;
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
            if self.dialog.is_none() && self.pending_conflicts.is_empty() {
                let mut found = Vec::new();
                for ((session, pane), (path, _, state)) in &self.previews {
                    if !events.rescan && !events.paths.contains(path) {
                        continue;
                    }
                    let view = state.read(cx);
                    if !view.has_unsaved_changes() {
                        continue;
                    }
                    let Ok(disk) = crate::services::files::read_text(path) else {
                        continue;
                    };
                    if view.external_change_needs_prompt(&disk) {
                        found.push(ConflictDialog::new(
                            session.clone(),
                            *pane,
                            path.clone(),
                            disk,
                        ));
                    }
                }
                self.pending_conflicts.extend(found);
            }
            if let Some(error) = events.error {
                self.error = Some(error);
            }
            cx.notify();
        }
        self.pump_ssh_transfers(cx);
        let transferring =
            self.modules.ssh.transfers.iter().any(|task| {
                task.state == crate::modules::ssh::transfer_task::TransferState::Running
            });
        if transferring {
            if self
                .transfer_tick
                .is_none_or(|tick| tick.elapsed() >= Duration::from_millis(200))
            {
                self.transfer_tick = Some(Instant::now());
                cx.notify();
            }
        } else {
            self.transfer_tick = None;
        }
        self.auto_refresh_docker(cx);
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
        let picker =
            cx.new(|cx| DirectoryPicker::new(browser, DirectoryPickerMode::Directory, window, cx));
        self.directory_subscription = Some(cx.subscribe(&picker, move |this, _, event, cx| {
            if let DirectoryPickerEvent::Selected(path) = event {
                if generation == this.dialog_generation && this.dialog.is_some() {
                    this.pending_directory = Some(DirectoryResult {
                        path: path.clone(),
                        generation,
                        target: DirectoryTarget::DialogInput,
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

    pub(crate) fn choose_database_form_file(
        &mut self,
        create: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let home = crate::services::desktop::home_dir();
        let entered = PathBuf::from(self.modules.database.form.path.read(cx).content().trim());
        let initial = if entered.is_dir() {
            entered
        } else if let Some(parent) = entered.parent().filter(|parent| parent.is_dir()) {
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
        let mode = if create {
            DirectoryPickerMode::NewFile
        } else {
            DirectoryPickerMode::OpenFile
        };
        let picker = cx.new(|cx| DirectoryPicker::new(browser, mode, window, cx));
        self.directory_subscription = Some(cx.subscribe(&picker, move |this, _, event, cx| {
            if let DirectoryPickerEvent::Selected(path) = event {
                if generation == this.dialog_generation && this.dialog.is_some() {
                    this.pending_directory = Some(DirectoryResult {
                        path: path.clone(),
                        generation,
                        target: DirectoryTarget::DatabaseFormPath,
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
                let project_root = self.current().map(|project| project.path);
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
                    if let Some(project_path) = &project_root {
                        let mut parent = path.parent();
                        while let Some(directory) = parent {
                            if directory == project_path {
                                break;
                            }
                            workspace.state.expanded.insert(directory.to_path_buf());
                            parent = directory.parent();
                        }
                    }
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
                    if key.module == ModuleId::Database {
                        workspace.state.add_query(next);
                    }
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
        self.modules.stop_transfers();
        crate::services::terminal_processes::TerminalProcesses::shutdown_all();
        if let Err(error) = self.save() {
            eprintln!("配置保存失败：{error:#}");
        }
    }
}
