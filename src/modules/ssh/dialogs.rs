use super::model::SshConnection;
use crate::app::MintoolApp;
use crate::model::session::SessionKey;
use crate::ui::dialogs::DialogContent;
use crate::ui::theme;
use adabraka_ui::components::input::Input;
use adabraka_ui::components::input_state::SelectAll;
use adabraka_ui::components::select::{Select, SelectOption};
use gpui::prelude::FluentBuilder;
use gpui::*;

#[derive(Clone)]
pub enum SshDialog {
    Connection {
        edit: Option<String>,
        group: Option<String>,
    },
    Remove(String, String),
    Move(String, String),
    GroupAdd,
    GroupRename(String, String),
    GroupRemove(String, String),
}

impl SshDialog {
    fn has_group_select(&self) -> bool {
        matches!(self, Self::Connection { .. } | Self::Move(..))
    }
}

impl DialogContent for SshDialog {
    fn title(&self) -> String {
        match self {
            Self::Connection { edit, .. } => {
                if edit.is_some() {
                    "编辑 SSH 连接"
                } else {
                    "新建 SSH 连接"
                }
            }
            Self::Remove(..) => "删除 SSH 连接",
            Self::Move(..) => "移动 SSH 连接",
            Self::GroupAdd => "新建 SSH 分组",
            Self::GroupRename(..) => "重命名 SSH 分组",
            Self::GroupRemove(..) => "删除 SSH 分组",
        }
        .into()
    }
    fn input_value(&self, _app: &MintoolApp) -> String {
        match self {
            Self::GroupRename(_, name) => name.clone(),
            _ => String::new(),
        }
    }
    fn input_hint(&self) -> Option<&'static str> {
        match self {
            Self::GroupAdd | Self::GroupRename(..) => Some("名称"),
            _ => None,
        }
    }
    fn accepts_empty_input(&self) -> bool {
        matches!(
            self,
            Self::Connection { .. } | Self::Remove(..) | Self::Move(..) | Self::GroupRemove(..)
        )
    }
    fn subject(&self) -> Option<String> {
        match self {
            Self::Remove(_, name)
            | Self::Move(_, name)
            | Self::GroupRename(_, name)
            | Self::GroupRemove(_, name) => Some(name.clone()),
            _ => None,
        }
    }
    fn note(&self, _app: &MintoolApp) -> Option<String> {
        match self {
            Self::Remove(..) => Some("删除此连接配置，已打开的 SSH 会话会关闭。".into()),
            Self::GroupRemove(..) => Some("删除此分组，连接会移到未分组。".into()),
            _ => None,
        }
    }
    fn confirm_label(&self) -> &'static str {
        match self {
            Self::Connection { .. } => "保存连接",
            Self::Remove(..) | Self::GroupRemove(..) => "删除",
            Self::Move(..) => "移动",
            Self::GroupAdd => "新建分组",
            Self::GroupRename(..) => "保存",
        }
    }
    fn enabled(&self, app: &MintoolApp, cx: &mut Context<MintoolApp>) -> bool {
        if matches!(self, Self::Connection { .. }) {
            !app.modules
                .ssh
                .form
                .host
                .read(cx)
                .content()
                .trim()
                .is_empty()
        } else {
            self.accepts_empty_input() || !app.dialog_input.read(cx).content().trim().is_empty()
        }
    }
    fn prepare(&self, app: &mut MintoolApp, window: &mut Window, cx: &mut Context<MintoolApp>) {
        if !self.has_group_select() {
            return;
        }
        let connection = match self {
            Self::Connection { edit, .. } => edit.as_ref().and_then(|id| {
                app.modules
                    .ssh
                    .registry
                    .connections
                    .iter()
                    .find(|connection| &connection.id == id)
                    .cloned()
            }),
            _ => None,
        };
        let selected_group = match self {
            Self::Connection { group, .. } => connection
                .as_ref()
                .and_then(|connection| connection.group.clone())
                .or_else(|| group.clone()),
            Self::Move(id, _) => app
                .modules
                .ssh
                .registry
                .connections
                .iter()
                .find(|connection| &connection.id == id)
                .and_then(|connection| connection.group.clone()),
            _ => None,
        };
        let options: Vec<_> = std::iter::once(SelectOption::new(None, "未分组"))
            .chain(
                app.modules
                    .ssh
                    .registry
                    .groups
                    .iter()
                    .map(|group| SelectOption::new(Some(group.id.clone()), group.name.clone())),
            )
            .collect();
        let selected = options
            .iter()
            .position(|option| option.value == selected_group)
            .unwrap_or(0);
        app.modules.ssh.form.color = connection
            .as_ref()
            .and_then(|connection| connection.color.clone());
        app.modules.ssh.group_select = cx.new(|cx| {
            Select::new(cx)
                .options(options)
                .selected_index(Some(selected))
                .w_full()
        });
        if let Self::Connection { .. } = self {
            app.modules.ssh.form.testing = false;
            app.modules.ssh.form.test_status = None;
            let (name, host, port, user, password, identity, extra) = connection.map_or_else(
                || {
                    (
                        String::new(),
                        String::new(),
                        "22".to_string(),
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                    )
                },
                |connection| {
                    (
                        connection.name,
                        connection.host,
                        connection.port.to_string(),
                        connection.user,
                        connection.password.unwrap_or_default(),
                        connection
                            .identity
                            .map(|path| path.display().to_string())
                            .unwrap_or_default(),
                        connection.extra_args.join(" "),
                    )
                },
            );
            for (input, value) in [
                (&app.modules.ssh.form.name, name),
                (&app.modules.ssh.form.host, host),
                (&app.modules.ssh.form.port, port),
                (&app.modules.ssh.form.user, user),
                (&app.modules.ssh.form.password, password),
                (&app.modules.ssh.form.identity, identity),
                (&app.modules.ssh.form.extra, extra),
            ] {
                input.update(cx, |state, cx| {
                    state.select_all(&SelectAll, window, cx);
                    state.set_value(value, window, cx);
                });
            }
        }
    }
    fn focus(&self, app: &MintoolApp, window: &mut Window, cx: &mut Context<MintoolApp>) {
        if matches!(self, Self::Connection { .. }) {
            app.modules
                .ssh
                .form
                .host
                .read(cx)
                .focus_handle(cx)
                .focus(window);
        } else {
            app.dialog_input.read(cx).focus_handle(cx).focus(window);
        }
    }
    fn render_extra(&self, app: &MintoolApp, cx: &mut Context<MintoolApp>) -> Option<AnyElement> {
        let mut extra = div().flex().flex_col().gap_4();
        if matches!(self, Self::Connection { .. }) {
            extra = extra.child(app.render_ssh_form(cx));
        }
        if self.has_group_select() {
            extra = extra
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme::muted())
                        .child("所属分组"),
                )
                .child(app.modules.ssh.group_select.clone());
        }
        Some(extra.into_any_element())
    }
    fn render_footer(&self, app: &MintoolApp, cx: &mut Context<MintoolApp>) -> Option<AnyElement> {
        if !matches!(self, Self::Connection { .. }) {
            return None;
        }
        Some(
            div()
                .child(
                    adabraka_ui::components::button::Button::new(
                        "test-ssh-connection",
                        if app.modules.ssh.form.testing {
                            "测试中…"
                        } else {
                            "测试连接"
                        },
                    )
                    .variant(adabraka_ui::components::button::ButtonVariant::Outline)
                    .disabled(
                        app.modules.ssh.form.testing
                            || app
                                .modules
                                .ssh
                                .form
                                .host
                                .read(cx)
                                .content()
                                .trim()
                                .is_empty(),
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.test_ssh_connection(cx))),
                )
                .into_any_element(),
        )
    }
    fn submit(
        &self,
        app: &mut MintoolApp,
        value: String,
        cx: &mut Context<MintoolApp>,
    ) -> anyhow::Result<()> {
        match self {
            Self::Connection { edit, .. } => app.submit_ssh_connection(edit.clone(), cx),
            Self::Remove(id, _) => {
                app.modules.ssh.registry.remove(id);
                if app.modules.ssh.opened.as_deref() == Some(id.as_str()) {
                    app.modules.ssh.opened = None;
                    app.modules.ssh.remote = None;
                }
                app.remove_session(&SessionKey::ssh(id.clone()), cx);
                Ok(())
            }
            Self::Move(id, _) => {
                let group = app
                    .modules
                    .ssh
                    .group_select
                    .read(cx)
                    .selected_value()
                    .cloned()
                    .flatten();
                app.modules
                    .ssh
                    .registry
                    .move_connection(id, group.clone(), None);
                if let Some(group) = group {
                    if let Some(group) = app
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
                Ok(())
            }
            Self::GroupAdd => app.modules.ssh.registry.add_group(&value).map(|_| ()),
            Self::GroupRename(id, _) => {
                if app
                    .modules
                    .ssh
                    .registry
                    .groups
                    .iter()
                    .any(|group| &group.id != id && group.name == value)
                {
                    Err(anyhow::anyhow!("分组名称已存在"))
                } else {
                    if let Some(group) = app
                        .modules
                        .ssh
                        .registry
                        .groups
                        .iter_mut()
                        .find(|group| &group.id == id)
                    {
                        group.name = value;
                    }
                    Ok(())
                }
            }
            Self::GroupRemove(id, _) => {
                app.modules.ssh.registry.remove_group(id);
                Ok(())
            }
        }
    }
}

impl MintoolApp {
    pub(crate) fn submit_ssh_connection(
        &mut self,
        edit: Option<String>,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let mut connection = self.form_connection(cx)?;
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
        let (name, host, port, user, password, identity, extra) = {
            let form = &self.modules.ssh.form;
            (
                form.name.read(cx).content().trim().to_string(),
                form.host.read(cx).content().trim().to_string(),
                form.port.read(cx).content().trim().to_string(),
                form.user.read(cx).content().trim().to_string(),
                form.password.read(cx).content().to_string(),
                form.identity.read(cx).content().trim().to_string(),
                form.extra.read(cx).content().trim().to_string(),
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
            .child(field("私钥路径（可选）", Input::new(&form.identity)))
            .child(field("高级参数（可选）", Input::new(&form.extra)))
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

const SWATCHES: [(&str, &str); 6] = [
    ("#6ee7b7", "绿"),
    ("#8aaff0", "蓝"),
    ("#c49de1", "紫"),
    ("#e1bd85", "橙"),
    ("#ee8598", "红"),
    ("#8c9aaa", "灰"),
];

fn color_value(hex: &str) -> u32 {
    u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0x8c9aaa)
}

fn expand_path(value: &str) -> std::path::PathBuf {
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return std::path::PathBuf::from(home).join(rest);
        }
    }
    std::path::PathBuf::from(value)
}

struct TestSpec {
    args: Vec<String>,
    password: Option<String>,
}

fn test_spec(connection: &SshConnection) -> TestSpec {
    let mut args = Vec::new();
    if connection.port != 22 {
        args.push("-p".to_string());
        args.push(connection.port.to_string());
    }
    if let Some(identity) = &connection.identity {
        args.push("-i".to_string());
        args.push(identity.display().to_string());
    }
    args.extend([
        "-o".to_string(),
        "ConnectTimeout=8".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        "-o".to_string(),
        "NumberOfPasswordPrompts=1".to_string(),
        "-o".to_string(),
        "LogLevel=ERROR".to_string(),
        "-T".to_string(),
    ]);
    if connection.password.is_none() {
        args.extend(["-o".to_string(), "BatchMode=yes".to_string()]);
    }
    args.extend(connection.extra_args.iter().cloned());
    args.push(connection.target());
    args.push("true".to_string());
    TestSpec {
        args,
        password: connection.password.clone(),
    }
}

fn run_test(spec: &TestSpec) -> (bool, String) {
    let timeout = if std::path::Path::new("/usr/bin/timeout").exists() {
        "/usr/bin/timeout"
    } else {
        "timeout"
    };
    let mut command = std::process::Command::new(timeout);
    command.arg("12").arg("ssh").args(&spec.args);
    if let Some(password) = &spec.password {
        if let Some(executable) = super::session::askpass_executable() {
            command
                .env("SSH_ASKPASS", executable)
                .env("SSH_ASKPASS_REQUIRE", "force")
                .env("MINTOOL_ASKPASS", "1")
                .env("MINTOOL_ASKPASS_PASSWORD", password);
        }
    }
    let output = match command.output() {
        Ok(output) => output,
        Err(error) => return (false, format!("无法启动 ssh：{error}")),
    };
    if output.status.success() {
        return (true, "连接成功".to_string());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let mut message = if output.status.code() == Some(124) {
        "连接超时（12 秒）".to_string()
    } else if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("ssh 退出码 {:?}", output.status.code())
    };
    message = message
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(4)
        .collect::<Vec<_>>()
        .join(" / ");
    if spec.password.is_none() && message.contains("Permission denied") {
        message.push_str("（未填写密码，只能测试密钥/agent 登录）");
    }
    (false, message)
}

#[cfg(test)]
mod tests {
    use super::{test_spec, SshConnection};

    #[test]
    fn test_command_uses_batch_mode_without_a_password() {
        let connection = SshConnection::new("prod", "prod-alias", None);
        let spec = test_spec(&connection);
        assert!(spec.password.is_none());
        assert!(spec
            .args
            .windows(2)
            .any(|pair| pair == ["-o".to_string(), "BatchMode=yes".to_string()]));
        assert_eq!(spec.args.last().unwrap(), "true");
        assert_eq!(spec.args[spec.args.len() - 2], "prod-alias");
    }

    #[test]
    fn test_command_keeps_port_and_password() {
        let mut connection = SshConnection::new("box", "10.0.0.8", None);
        connection.port = 2222;
        connection.user = "tester".into();
        connection.password = Some("secret".into());
        let spec = test_spec(&connection);
        assert_eq!(spec.password.as_deref(), Some("secret"));
        assert!(spec
            .args
            .windows(2)
            .any(|pair| pair == ["-p".to_string(), "2222".to_string()]));
        assert!(!spec.args.iter().any(|argument| argument == "BatchMode=yes"));
        assert_eq!(spec.args[spec.args.len() - 2], "tester@10.0.0.8");
    }
}
