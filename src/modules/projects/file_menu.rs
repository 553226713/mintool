use crate::{
    app::MintoolApp,
    model::session::{ModuleId, SessionKey, SessionView},
    services::{desktop::Desktop, file_actions::FileAction},
    ui::dialogs::Dialog,
    workspace::layout::Axis,
};
use adabraka_ui::navigation::menu::MenuItem;
use gpui::*;
use std::path::PathBuf;

impl MintoolApp {
    pub(crate) fn synchronize_file_watches(&mut self) {
        let mut paths = std::collections::BTreeSet::new();
        if let Some(project) = self.current() {
            paths.insert(project.path.clone());
            // The parent reports removal/recreation of the project root itself.
            if let Some(parent) = project.path.parent() {
                paths.insert(parent.to_path_buf());
            }
            if let Some(workspace) = self.workspaces.get(&SessionKey::project(&project.id)) {
                for file in workspace.state.files.values() {
                    if let Some(parent) = file.parent() {
                        paths.insert(parent.to_path_buf());
                    }
                }
                for path in &workspace.state.expanded {
                    if path.starts_with(&project.path)
                        && path
                            .ancestors()
                            .skip(1)
                            .take_while(|parent| *parent != project.path)
                            .all(|parent| workspace.state.expanded.contains(parent))
                    {
                        paths.insert(path.clone());
                    }
                }
                let mut ids = Vec::new();
                workspace.state.layout.ids(&mut ids);
                for id in ids {
                    if let Some(file) = workspace
                        .state
                        .layout
                        .find(id)
                        .and_then(|pane| pane.file.as_ref())
                    {
                        if let Some(parent) = file.parent() {
                            paths.insert(parent.to_path_buf());
                        }
                    }
                }
            }
        }
        let added = self.file_monitor.synchronize(paths);
        if let Some(project) = self.current() {
            if let Some(workspace) = self.workspaces.get_mut(&SessionKey::project(&project.id)) {
                workspace.cache.invalidate_paths(&added);
            }
        }
    }

    pub(crate) fn refresh_files(&mut self, cx: &mut Context<Self>) {
        self.file_monitor.retry();
        if let Some(project) = self.current() {
            if let Some(workspace) = self.workspaces.get_mut(&SessionKey::project(&project.id)) {
                workspace.cache.invalidate();
            }
        }
        cx.notify();
    }
    pub(crate) fn apply_file_action(
        &mut self,
        action: &FileAction,
        name: &str,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let destination = action.execute(name)?;
        let mut removed_file_panes = Vec::new();
        for (session_key, workspace) in self
            .workspaces
            .iter_mut()
            .filter(|(key, _)| key.module == ModuleId::Projects)
        {
            workspace.cache.invalidate();
            match action {
                FileAction::Rename(old) => {
                    if let Some(new) = &destination {
                        for file in workspace.state.files.values_mut() {
                            if let Ok(relative) = file.strip_prefix(old) {
                                *file = new.join(relative);
                            }
                        }
                        let mut ids = Vec::new();
                        workspace.state.layout.ids(&mut ids);
                        for id in ids {
                            if let Some(pane) = workspace.state.layout.find_mut(id) {
                                if let Some(file) = &pane.file {
                                    if let Ok(relative) = file.strip_prefix(old) {
                                        pane.file = Some(new.join(relative));
                                    }
                                }
                            }
                        }
                        workspace.state.expanded = workspace
                            .state
                            .expanded
                            .iter()
                            .map(|path| {
                                path.strip_prefix(old)
                                    .map(|relative| new.join(relative))
                                    .unwrap_or_else(|_| path.clone())
                            })
                            .collect();
                    }
                }
                FileAction::NewFile(parent) | FileAction::NewDirectory(parent) => {
                    workspace.state.expanded.insert(parent.clone());
                }
                FileAction::Trash(path) => {
                    workspace.state.expanded.retain(|p| !p.starts_with(path));
                    let removed: Vec<_> = workspace
                        .state
                        .files
                        .iter()
                        .filter(|(_, file)| file.starts_with(path))
                        .map(|(id, _)| *id)
                        .collect();
                    workspace
                        .state
                        .files
                        .retain(|_, file| !file.starts_with(path));
                    removed_file_panes.extend(
                        removed
                            .into_iter()
                            .map(|file_id| (session_key.clone(), file_id)),
                    );
                }
            }
        }
        for close in removed_file_panes {
            self.clear_preview(&close.0, close.1);
            self.dock_closes.push(close);
        }
        if matches!(action, FileAction::NewFile(_)) {
            if let Some(path) = destination {
                self.open_file(path, cx);
            }
        }
        self.changed(cx);
        Ok(())
    }
    pub(crate) fn file_menu(
        &mut self,
        path: PathBuf,
        directory: bool,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self.current() else {
            return;
        };
        let root = path == project.path;
        let folder = if directory {
            path.clone()
        } else {
            path.parent().unwrap_or(&project.path).to_path_buf()
        };
        let mut items = Vec::new();
        if !directory {
            let file = path.clone();
            items.push(self.menu_action(
                "preview",
                "打开预览 / 定位变更",
                cx,
                move |this, _, cx| {
                    if let Some(project) = this.current() {
                        if let Some(w) = this.workspaces.get(&SessionKey::project(&project.id)) {
                            this.clear_preview(&SessionKey::project(&project.id), w.state.focused);
                        }
                    }
                    this.open_file(file.clone(), cx);
                },
            ));
        }
        let browse = folder.clone();
        items.push(self.menu_action(
            "browse",
            "在文件管理器中打开目录",
            cx,
            move |this, _, _| {
                if let Err(error) = Desktop::open_directory(&browse) {
                    this.error = Some(format!("{error:#}"));
                }
            },
        ));
        let cwd = folder.clone();
        items.push(self.menu_action(
            "terminal-here",
            "在此处新建终端",
            cx,
            move |this, _, cx| {
                let Some(project) = this.current() else {
                    return;
                };
                let Some(workspace) = this.workspaces.get(&SessionKey::project(&project.id)) else {
                    return;
                };
                let focused = workspace.state.focused;
                this.split(focused, Axis::Horizontal, cx);
                let id = this.workspaces[&SessionKey::project(&project.id)]
                    .state
                    .focused;
                let mut session = SessionView::project(&project);
                session.cwd = cwd.clone();
                this.terminal(&session, id, cx);
            },
        ));
        items.push(MenuItem::separator());
        for action in [
            FileAction::NewFile(folder.clone()),
            FileAction::NewDirectory(folder.clone()),
        ] {
            items.push(self.menu_action(
                action.title(),
                format!("{}…", action.title()),
                cx,
                move |this, window, cx| {
                    this.show_dialog(
                        Dialog::Project(crate::modules::projects::dialogs::ProjectDialog::File(
                            action.clone(),
                        )),
                        window,
                        cx,
                    );
                },
            ));
        }
        let absolute = path.display().to_string();
        items.push(self.menu_action(
            "copy-absolute",
            "复制完整路径",
            cx,
            move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(absolute.clone()));
            },
        ));
        let relative = path
            .strip_prefix(&project.path)
            .unwrap_or(&path)
            .display()
            .to_string();
        items.push(self.menu_action(
            "copy-relative",
            "复制相对路径",
            cx,
            move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(relative.clone()));
            },
        ));
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        items.push(
            self.menu_action("copy-name", "复制名称", cx, move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(name.clone()));
            }),
        );
        if !directory {
            let file = path.clone();
            items.push(
                self.menu_action("duplicate", "创建文件副本", cx, move |this, _, cx| {
                    match FileAction::duplicate(&file) {
                        Ok(path) => {
                            this.refresh_files(cx);
                            this.open_file(path, cx);
                        }
                        Err(error) => this.error = Some(format!("{error:#}")),
                    }
                })
                .disabled(!path.is_file()),
            );
        }
        if !root {
            for action in [
                FileAction::Rename(path.clone()),
                FileAction::Trash(path.clone()),
            ] {
                items.push(
                    self.menu_action(
                        action.title(),
                        format!("{}…", action.title()),
                        cx,
                        move |this, window, cx| {
                            this.show_dialog(
                                Dialog::Project(
                                    crate::modules::projects::dialogs::ProjectDialog::File(
                                        action.clone(),
                                    ),
                                ),
                                window,
                                cx,
                            );
                        },
                    )
                    .disabled(!path.exists() && path.symlink_metadata().is_err()),
                );
            }
        }
        items.push(MenuItem::separator());
        items.push(
            self.menu_action("refresh-files", "刷新资源树", cx, |this, _, cx| {
                this.refresh_files(cx)
            }),
        );
        items.push(
            self.menu_action("collapse-files", "折叠全部目录", cx, |this, _, cx| {
                if let Some(project) = this.current() {
                    if let Some(w) = this.workspaces.get_mut(&SessionKey::project(&project.id)) {
                        w.state.expanded.clear();
                    }
                }
                this.changed(cx);
            }),
        );
        self.menu = Some((position, items));
        cx.notify();
    }
}
