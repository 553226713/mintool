use super::MintoolApp;
use crate::modules::ModuleId;
use crate::ui::dialogs::Dialog;
use crate::ui::theme;
use adabraka_ui::components::input_state::{Enter as InputEnter, Escape as InputEscape};
use adabraka_ui::components::resizable::{h_resizable, resizable_panel};
use gpui::*;

impl Render for MintoolApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.dialog.is_none() {
            if let Some(conflict) = self.pending_conflicts.pop_front() {
                self.show_dialog(Dialog::Conflict(conflict), window, cx);
            }
        }
        let overlays = self.dialog.is_some()
            || self.menu.is_some()
            || self.directory_picker.is_some()
            || self.quick_open.is_some()
            || self.command_palette.is_some()
            || self.remote_edit.is_some()
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
                .is_some_and(|pane| pane.is_terminal());
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
                let input = match result.target {
                    crate::app::DirectoryTarget::DialogInput => self.dialog_input.clone(),
                    crate::app::DirectoryTarget::DatabaseFormPath => {
                        self.modules.database.form.path.clone()
                    }
                };
                input.update(cx, |state, cx| {
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
            ModuleId::Projects => self.render_navigation(cx),
            ModuleId::Ssh => self.render_ssh_sidebar(window, cx),
            ModuleId::Docker => self.render_docker_sidebar(window, cx),
            ModuleId::Database => self.render_database_sidebar(window, cx),
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
            ModuleId::Ssh => false,
            ModuleId::Docker => docker_auxiliary,
            ModuleId::Database => false,
        };
        let explorer = if auxiliary {
            Some(match self.modules.active {
                ModuleId::Projects => self.render_explorer(cx),
                ModuleId::Ssh => div().into_any_element(),
                ModuleId::Docker => match &docker_session {
                    Some(session) => self.render_docker_session_panel(session, window, cx),
                    None => div().into_any_element(),
                },
                ModuleId::Database => div().into_any_element(),
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
            .capture_action(cx.listener(|this, _: &InputEscape, window, cx| {
                if this.remote_edit.is_some() && this.dialog.is_none() {
                    this.close_remote_edit(window, cx);
                    cx.stop_propagation();
                } else if this.command_palette.is_some() {
                    this.close_command_palette(cx);
                    cx.stop_propagation();
                } else if this.quick_open.is_some() {
                    this.close_quick_open(cx);
                    cx.stop_propagation();
                } else if this.modules.projects.renaming.is_some() {
                    this.modules.projects.renaming = None;
                    cx.notify();
                    cx.stop_propagation();
                } else if this.directory_picker.is_some() {
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
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let control =
                    event.keystroke.modifiers.control || event.keystroke.modifiers.platform;
                if control
                    && event.keystroke.modifiers.shift
                    && event.keystroke.key.eq_ignore_ascii_case("p")
                {
                    if this.command_palette.is_some() {
                        this.close_command_palette(cx);
                    } else {
                        this.open_command_palette(window, cx);
                    }
                    cx.stop_propagation();
                } else if this.remote_edit.is_some() && this.dialog.is_none() {
                    if control && event.keystroke.key.eq_ignore_ascii_case("s") {
                        this.save_remote_edit(false, cx);
                        cx.stop_propagation();
                    } else if event.keystroke.key == "escape" {
                        this.close_remote_edit(window, cx);
                        cx.stop_propagation();
                    }
                } else if this.command_palette.is_some() {
                    match event.keystroke.key.as_str() {
                        "escape" => {
                            this.close_command_palette(cx);
                            cx.stop_propagation();
                        }
                        "down" => {
                            this.move_command_palette(1, cx);
                            cx.stop_propagation();
                        }
                        "up" => {
                            this.move_command_palette(-1, cx);
                            cx.stop_propagation();
                        }
                        "enter" => {
                            this.run_command_palette_selected(window, cx);
                            cx.stop_propagation();
                        }
                        _ => {}
                    }
                } else if control && event.keystroke.key.eq_ignore_ascii_case("p") {
                    if this.quick_open.is_some() {
                        this.close_quick_open(cx);
                    } else {
                        this.open_quick_open(window, cx);
                    }
                    cx.stop_propagation();
                } else if this.quick_open.is_some() {
                    match event.keystroke.key.as_str() {
                        "escape" => {
                            this.close_quick_open(cx);
                            cx.stop_propagation();
                        }
                        "down" => {
                            this.move_quick_open(1, cx);
                            cx.stop_propagation();
                        }
                        "up" => {
                            this.move_quick_open(-1, cx);
                            cx.stop_propagation();
                        }
                        "enter" => {
                            this.open_quick_open_selected(cx);
                            cx.stop_propagation();
                        }
                        _ => {}
                    }
                } else if this.directory_picker.is_some() && event.keystroke.key == "escape" {
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
            .children(self.render_quick_open(cx))
            .children(self.render_command_palette(cx))
            .children(self.render_remote_edit(window, cx))
            .children(dialog)
            .children(self.directory_picker.clone())
            .children(menu)
            .children(self.render_toast(cx))
    }
}
