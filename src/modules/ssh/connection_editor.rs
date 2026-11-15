use super::dialogs::{run_test, test_spec};
use super::model::SshConnection;
use crate::app::MintoolApp;
use crate::ui::common::colors::{color_value, SWATCHES};
use crate::ui::theme;
use adabraka_ui::components::button::{Button, ButtonSize, ButtonVariant};
use adabraka_ui::components::input::Input;
use gpui::prelude::FluentBuilder;
use gpui::*;

impl MintoolApp {
    pub(crate) fn submit_ssh_connection(
        &mut self,
        edit: Option<String>,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let mut connection = self.form_connection(cx)?;
        let previous = edit.as_ref().and_then(|id| {
            self.modules
                .ssh
                .registry
                .connections
                .iter()
                .find(|connection| &connection.id == id)
                .cloned()
        });
        if let Some(id) = &edit {
            connection.id = id.clone();
        }
        let wants_keyring =
            self.modules.ssh.form.use_keyring && crate::services::keyring::available();
        if wants_keyring {
            let password = connection.password.take();
            if let Some(password) = &password {
                crate::services::keyring::store(&connection.id, password)?;
                connection.resolved_password = Some(password.clone());
            } else if let Some(previous) = &previous {
                connection.resolved_password = previous.resolved_password.clone();
            }
            connection.use_keyring =
                password.is_some() || previous.as_ref().is_some_and(|p| p.use_keyring);
        } else {
            connection.use_keyring = false;
            connection.resolved_password = None;
            if let Some(id) = &edit {
                if previous.as_ref().is_some_and(|p| p.use_keyring) {
                    let _ = crate::services::keyring::clear(id);
                }
            }
        }
        if let Some(id) = edit {
            connection.id = id;
            self.modules.ssh.registry.update(connection);
        } else {
            self.modules.ssh.registry.add(connection);
        }
        self.expand_ssh_group();
        self.changed(cx);
        Ok(())
    }
    fn form_connection(&self, cx: &mut Context<Self>) -> anyhow::Result<SshConnection> {
        let (name, host, port, user, password, identity, extra, forwards) = {
            let form = &self.modules.ssh.form;
            (
                form.name.read(cx).content().trim().to_string(),
                form.host.read(cx).content().trim().to_string(),
                form.port.read(cx).content().trim().to_string(),
                form.user.read(cx).content().trim().to_string(),
                form.password.read(cx).content().to_string(),
                form.identity.read(cx).content().trim().to_string(),
                form.extra.read(cx).content().trim().to_string(),
                form.forwards.read(cx).content().trim().to_string(),
            )
        };
        anyhow::ensure!(!host.is_empty(), "请填写主机名或 ~/.ssh/config 别名");
        let port = if port.is_empty() {
            22
        } else {
            match port.parse::<u16>() {
                Ok(port) if port > 0 => port,
                _ => anyhow::bail!("端口应为 1 到 65535 之间的数字"),
            }
        };
        let group = self
            .modules
            .ssh
            .group_select
            .read(cx)
            .selected_value()
            .cloned()
            .flatten();
        let mut connection =
            SshConnection::new(if name.is_empty() { &host } else { &name }, &host, group);
        connection.port = port;
        connection.user = user;
        connection.password = (!password.is_empty()).then_some(password);
        connection.identity = (!identity.is_empty()).then(|| expand_path(&identity));
        connection.extra_args = extra.split_whitespace().map(str::to_owned).collect();
        connection.forwards = forwards.split_whitespace().map(str::to_owned).collect();
        connection.color = self.modules.ssh.form.color.clone();
        Ok(connection)
    }
    pub(crate) fn test_ssh_connection(&mut self, cx: &mut Context<Self>) {
        if self.modules.ssh.form.testing {
            return;
        }
        let connection = match self.form_connection(cx) {
            Ok(connection) => connection,
            Err(error) => {
                self.modules.ssh.form.test_status = Some((false, format!("{error:#}")));
                cx.notify();
                return;
            }
        };
        let spec = test_spec(&connection);
        let executor = cx.background_executor().clone();
        self.modules.ssh.form.testing = true;
        self.modules.ssh.form.test_status = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = executor.spawn(async move { run_test(&spec) }).await;
            let _ = this.update(cx, |this, cx| {
                this.modules.ssh.form.testing = false;
                this.modules.ssh.form.test_status = Some(result);
                cx.notify();
            });
        })
        .detach();
    }
    fn expand_ssh_group(&mut self) {
        let group = self
            .modules
            .ssh
            .registry
            .selected()
            .and_then(|connection| connection.group.clone());
        if let Some(group) = group {
            if let Some(group) = self
                .modules
                .ssh
                .registry
                .groups
                .iter_mut()
                .find(|candidate| candidate.id == group)
            {
                group.collapsed = false;
            }
        }
    }
    /// Form body for the add/edit SSH connection dialog.
    pub(crate) fn render_ssh_form(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let form = &self.modules.ssh.form;
        let field = |label: &'static str, input: Input| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme::muted())
                        .child(label),
                )
                .child(
                    input
                        .map(|mut input| {
                            input.style().align_items = Some(AlignItems::Stretch);
                            input
                        })
                        .w_full(),
                )
        };
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(field("连接名称（留空使用主机名）", Input::new(&form.name)))
            .child(field("主机名或 ~/.ssh/config 别名", Input::new(&form.host)))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(div().w(px(110.)).child(field("端口", Input::new(&form.port))))
                    .child(div().flex_1().child(field("用户名", Input::new(&form.user)))),
            )
            .child(field("密码（可选）", Input::new(&form.password).password(true)))
            .child(
                Button::new(
                    "ssh-keyring-toggle",
                    if form.use_keyring {
                        "系统密钥环：已启用"
                    } else {
                        "保存到系统密钥环"
                    },
                )
                .variant(if form.use_keyring {
                    ButtonVariant::Default
                } else {
                    ButtonVariant::Outline
                })
                .size(ButtonSize::Sm)
                .disabled(!crate::services::keyring::available())
                .tooltip(if crate::services::keyring::available() {
                    "密码保存到系统密钥环（secret-tool），不写入配置文件"
                } else {
                    "未检测到 secret-tool，密码将按原方式保存在配置文件中"
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.modules.ssh.form.use_keyring =
                        !this.modules.ssh.form.use_keyring;
                    cx.notify();
                })),
            )
            .child(field("私钥路径（可选）", Input::new(&form.identity)))
            .child(field("高级参数（可选）", Input::new(&form.extra)))
            .child(field(
                "端口转发（空格分隔；R: 远程，D: 动态，默认本地）",
                Input::new(&form.forwards),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme::muted())
                            .child("标记颜色"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .children(SWATCHES.iter().enumerate().map(|(index, (hex, _))| {
                                let value = hex.to_string();
                                let selected = self.modules.ssh.form.color.as_deref() == Some(*hex);
                                let id = format!("ssh-color-{index}");
                                div()
                                    .id(SharedString::from(id))
                                    .size(px(20.))
                                    .rounded_full()
                                    .bg(rgb(color_value(hex)))
                                    .cursor_pointer()
                                    .border_2()
                                    .border_color(if selected {
                                        theme::text()
                                    } else {
                                        theme::panel()
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.modules.ssh.form.color = Some(value.clone());
                                        cx.notify();
                                    }))
                            })),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(theme::muted())
                    .child("密码会以明文保存在仅当前用户可读的 SSH 配置文件中；也可以留空并使用密钥或 ssh-agent。"),
            )
            .when(self.modules.ssh.form.testing, |form| {
                form.child(
                    div()
                        .text_size(px(11.))
                        .text_color(theme::warning())
                        .child("正在测试连接…"),
                )
            })
            .when_some(
                self.modules.ssh.form.test_status.clone(),
                |form, (success, message)| {
                    form.child(
                        div()
                            .text_size(px(11.))
                            .text_color(if success {
                                theme::success()
                            } else {
                                theme::danger()
                            })
                            .child(if success {
                                format!("✓ {message}")
                            } else {
                                format!("✗ {message}")
                            }),
                    )
                },
            )
    }
}

fn expand_path(value: &str) -> std::path::PathBuf {
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return std::path::PathBuf::from(home).join(rest);
        }
    }
    std::path::PathBuf::from(value)
}
