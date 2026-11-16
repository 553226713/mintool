use super::{local::LocalBrowser, path::join_remote};
use crate::{app::MintoolApp, ui::common::text_input::text_input};
use gpui::*;
use gpui_component::input::{InputEvent, InputState};
use std::path::PathBuf;

pub struct PathInputs {
    remote: Entity<InputState>,
    local: Entity<InputState>,
    remote_location: Option<(String, String)>,
    local_location: Option<PathBuf>,
    _subscriptions: Vec<Subscription>,
}

impl PathInputs {
    fn new(
        owner: Option<(crate::model::session::SessionKey, u64)>,
        window: &mut Window,
        cx: &mut Context<MintoolApp>,
    ) -> Self {
        let remote =
            cx.new(|cx| InputState::new(window, cx).placeholder("远程目录，如 /var/log，回车跳转"));
        let local = cx.new(|cx| InputState::new(window, cx).placeholder("本地目录，回车跳转"));
        let remote_owner = owner.clone();
        let local_owner = remote_owner.clone();
        let remote_subscription = cx.subscribe(&remote, move |app, input, event, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                if let Some(owner) = &remote_owner {
                    app.activate_ssh_panel(owner.clone(), cx);
                }
                let value = input.read(cx).value().to_string();
                if let Some(browser) = &app.modules.ssh.panel().remote {
                    let path = resolve_remote(&browser.current, browser.home.as_deref(), &value);
                    app.navigate_remote(path, cx);
                }
            }
        });
        let local_subscription = cx.subscribe(&local, move |app, input, event, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                if let Some(owner) = &local_owner {
                    app.activate_ssh_panel(owner.clone(), cx);
                }
                let value = input.read(cx).value().to_string();
                let path = if value == "~" {
                    LocalBrowser::home()
                } else if let Some(relative) = value.strip_prefix("~/") {
                    LocalBrowser::home().join(relative)
                } else {
                    let path = PathBuf::from(value);
                    if path.is_absolute() {
                        path
                    } else {
                        app.modules
                            .ssh
                            .panel()
                            .local
                            .as_ref()
                            .map(|browser| browser.current.join(&path))
                            .unwrap_or(path)
                    }
                };
                app.navigate_local(path, cx);
            }
        });
        Self {
            remote,
            local,
            remote_location: None,
            local_location: None,
            _subscriptions: vec![remote_subscription, local_subscription],
        }
    }
}

impl MintoolApp {
    pub(crate) fn sync_ssh_addresses(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.modules.ssh.panel().path_inputs.is_none() {
            self.modules.ssh.panel_mut().path_inputs = Some(PathInputs::new(
                self.modules.ssh.active_panel.clone(),
                window,
                cx,
            ));
        }
        let panel = self.modules.ssh.panel_mut();
        let mut remembered = false;
        let inputs = panel.path_inputs.as_mut().unwrap();
        if let Some(browser) = &panel.remote {
            if browser.entries.contains_key(&browser.current) {
                remembered |= panel.preferences.remember_remote(&browser.current);
            }
            let location = (browser.connection.clone(), browser.current.clone());
            if inputs.remote_location.as_ref() != Some(&location) {
                inputs.remote.update(cx, |state, cx| {
                    state.set_value(
                        if browser.current.is_empty() {
                            "~".to_string()
                        } else {
                            browser.current.clone()
                        },
                        window,
                        cx,
                    )
                });
                inputs.remote_location = Some(location);
            }
        }
        if let Some(browser) = &panel.local {
            if browser.error.is_none() {
                remembered |= panel.preferences.remember_local(&browser.current);
            }
            if inputs.local_location.as_ref() != Some(&browser.current) {
                inputs.local.update(cx, |state, cx| {
                    state.set_value(browser.current.display().to_string(), window, cx)
                });
                inputs.local_location = Some(browser.current.clone());
            }
        }
        if remembered {
            self.changed(cx);
        }
    }

    pub(crate) fn ssh_address_bar(&self, remote: bool) -> AnyElement {
        let Some(inputs) = &self.modules.ssh.panel().path_inputs else {
            return div().into_any_element();
        };
        div()
            .flex()
            .flex_col()
            .w_full()
            .flex_none()
            .px_2()
            .py_1()
            .child(text_input(if remote {
                &inputs.remote
            } else {
                &inputs.local
            }))
            .into_any_element()
    }
}

pub(super) fn resolve_remote(current: &str, home: Option<&str>, value: &str) -> String {
    let value = value.trim();
    if value.is_empty() || value == "~" {
        home.unwrap_or("").to_string()
    } else if let Some(relative) = value.strip_prefix("~/") {
        join_remote(home.unwrap_or(""), relative)
    } else if value.starts_with('/') {
        value.to_string()
    } else {
        join_remote(current, value)
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_remote;

    #[test]
    fn resolves_address_paths_against_current_or_login_home() {
        assert_eq!(resolve_remote("/var/log", Some("/root"), "/etc"), "/etc");
        assert_eq!(
            resolve_remote("/var/log", Some("/root"), "nginx"),
            "/var/log/nginx"
        );
        assert_eq!(
            resolve_remote("/var/log", Some("/root"), ".."),
            "/var/log/.."
        );
        assert_eq!(resolve_remote("/var/log", Some("/root"), "~"), "/root");
        assert_eq!(
            resolve_remote("/var/log", Some("/root"), "~/中文 目录"),
            "/root/中文 目录"
        );
    }
}

impl MintoolApp {
    pub(crate) fn ssh_bookmarks(&self, remote: bool, cx: &mut Context<Self>) -> AnyElement {
        use adabraka_ui::components::button::{Button, ButtonSize, ButtonVariant};
        let panel = self.modules.ssh.panel();
        let ready = if remote {
            panel.remote.as_ref().is_some_and(|browser| {
                browser.error.is_none() && browser.entries.contains_key(&browser.current)
            })
        } else {
            panel
                .local
                .as_ref()
                .is_some_and(|browser| browser.error.is_none())
        };
        let mut row = div().flex().flex_wrap().gap_1().px_2().flex_none();
        row = row.child(
            Button::new("directory-history", "最近目录")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .on_click(cx.listener(move |app, event: &ClickEvent, _, cx| {
                    let owner = app.modules.ssh.active_panel.clone().unwrap();
                    let panel = app.modules.ssh.panel();
                    let locations = if remote {
                        panel.preferences.recent_remote.clone()
                    } else {
                        panel
                            .preferences
                            .recent_local
                            .iter()
                            .map(|path| path.to_string_lossy().into_owned())
                            .collect()
                    };
                    let mut items = Vec::new();
                    for (index, path) in locations.into_iter().enumerate() {
                        let target = path.clone();
                        let owner = owner.clone();
                        items.push(app.menu_action(
                            &format!("recent-{index}"),
                            &path,
                            cx,
                            move |app, _, cx| {
                                app.activate_ssh_panel(owner.clone(), cx);
                                if remote {
                                    app.navigate_remote(target.clone(), cx);
                                } else {
                                    app.navigate_local(PathBuf::from(&target), cx);
                                }
                            },
                        ));
                    }
                    if !items.is_empty() {
                        app.menu = Some((event.position(), items));
                        cx.notify();
                    }
                })),
        );
        row = row.child(
            Button::new("bookmark-current", "收藏目录")
                .disabled(!ready)
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .on_click(cx.listener(move |app, _, _, cx| {
                    let panel = app.modules.ssh.panel_mut();
                    if remote {
                        if let Some(browser) = &panel.remote {
                            if !panel.preferences.bookmarks.contains(&browser.current) {
                                panel.preferences.bookmarks.push(browser.current.clone());
                            }
                        }
                    } else if let Some(browser) = &panel.local {
                        if !panel.preferences.local_bookmarks.contains(&browser.current) {
                            panel
                                .preferences
                                .local_bookmarks
                                .push(browser.current.clone());
                        }
                    }
                    app.changed(cx);
                })),
        );
        row = row.child(
            Button::new("default-directory", "设为默认目录")
                .disabled(!ready)
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .on_click(cx.listener(|app, _, _, cx| {
                    let owner = app.modules.ssh.active_panel.clone().unwrap();
                    let panel = app.modules.ssh.panel();
                    let mut preference = panel.preferences.clone();
                    preference.remote = panel
                        .remote
                        .as_ref()
                        .map(|browser| browser.current.clone())
                        .unwrap_or_default();
                    preference.local = panel.local.as_ref().map(|browser| browser.current.clone());
                    app.modules
                        .ssh
                        .preferences
                        .insert(format!("{}/default", owner.0.id), preference);
                    app.changed(cx);
                })),
        );
        let bookmarks = if remote {
            panel.preferences.bookmarks.clone()
        } else {
            panel
                .preferences
                .local_bookmarks
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect()
        };
        for (index, path) in bookmarks.into_iter().enumerate() {
            let target = path.clone();
            let remove = path.clone();
            row = row
                .child(
                    Button::new(SharedString::from(format!("bookmark-{index}")), path)
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(cx.listener(move |app, _, _, cx| {
                            if remote {
                                app.navigate_remote(target.clone(), cx);
                            } else {
                                app.navigate_local(PathBuf::from(&target), cx);
                            }
                        })),
                )
                .child(
                    Button::new(SharedString::from(format!("unbookmark-{index}")), "×")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(cx.listener(move |app, _, _, cx| {
                            let preferences = &mut app.modules.ssh.panel_mut().preferences;
                            if remote {
                                preferences.bookmarks.retain(|path| path != &remove);
                            } else {
                                preferences
                                    .local_bookmarks
                                    .retain(|path| path != &PathBuf::from(&remove));
                            }
                            app.changed(cx);
                        })),
                );
        }
        row.into_any_element()
    }
    pub(crate) fn ssh_navigation_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        let remote = self.modules.ssh.panel().navigation_remote;
        if !remote {
            match (modifiers.alt, event.keystroke.key.as_str()) {
                (true, "up") => self.local_up(cx),
                (true, "left") => self.local_back(cx),
                (true, "right") => self.local_forward(cx),
                (_, "f5") => self.refresh_local(cx),
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        match (modifiers.alt, event.keystroke.key.as_str()) {
            (true, "up") => self.remote_up(cx),
            (true, "left") => self.remote_back(cx),
            (true, "right") => self.remote_forward(cx),
            (_, "f5") => {
                self.refresh_remote_current(cx);
                self.refresh_local(cx);
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}
