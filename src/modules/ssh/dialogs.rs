use super::model::SshConnection;
use crate::app::MintoolApp;
use crate::model::session::SessionKey;
use crate::ui::dialogs::DialogContent;
use crate::ui::theme;
use adabraka_ui::components::input_state::SelectAll;
use adabraka_ui::components::select::{Select, SelectOption};
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
    RemoteMkdir {
        parent: String,
    },
    RemoteRename {
        path: String,
        name: String,
    },
    RemoteDelete {
        path: String,
        name: String,
        directory: bool,
    },
    RemoteChmod {
        path: String,
        name: String,
    },
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
            Self::RemoteMkdir { .. } => "新建远程文件夹",
            Self::RemoteRename { .. } => "重命名远程文件",
            Self::RemoteDelete { directory, .. } => {
                if *directory {
                    "删除远程目录"
                } else {
                    "删除远程文件"
                }
            }
            Self::RemoteChmod { .. } => "修改远程权限",
        }
        .into()
    }
    fn input_value(&self, _app: &MintoolApp) -> String {
        match self {
            Self::GroupRename(_, name) | Self::RemoteRename { name, .. } => name.clone(),
            Self::RemoteChmod { .. } => "644".into(),
            _ => String::new(),
        }
    }
    fn input_hint(&self) -> Option<&'static str> {
        match self {
            Self::GroupAdd | Self::GroupRename(..) => Some("名称"),
            Self::RemoteMkdir { .. } => Some("文件夹名称"),
            Self::RemoteRename { .. } => Some("新名称"),
            Self::RemoteChmod { .. } => Some("权限（如 644）"),
            _ => None,
        }
    }
    fn accepts_empty_input(&self) -> bool {
        matches!(
            self,
            Self::Connection { .. }
                | Self::Remove(..)
                | Self::Move(..)
                | Self::GroupRemove(..)
                | Self::RemoteDelete { .. }
        )
    }
    fn subject(&self) -> Option<String> {
        match self {
            Self::Remove(_, name)
            | Self::Move(_, name)
            | Self::GroupRename(_, name)
            | Self::GroupRemove(_, name)
            | Self::RemoteRename { name, .. }
            | Self::RemoteDelete { name, .. }
            | Self::RemoteChmod { name, .. } => Some(name.clone()),
            _ => None,
        }
    }
    fn note(&self, _app: &MintoolApp) -> Option<String> {
        match self {
            Self::Remove(..) => Some("删除此连接配置，已打开的 SSH 会话会关闭。".into()),
            Self::GroupRemove(..) => Some("删除此分组，连接会移到未分组。".into()),
            Self::RemoteDelete {
                directory: true, ..
            } => Some("删除此远程目录，目录必须为空。".into()),
            Self::RemoteDelete {
                directory: false, ..
            } => Some("永久删除此远程文件，无法恢复。".into()),
            _ => None,
        }
    }
    fn confirm_label(&self) -> &'static str {
        match self {
            Self::Connection { .. } => "保存连接",
            Self::Remove(..) | Self::GroupRemove(..) | Self::RemoteDelete { .. } => "删除",
            Self::Move(..) => "移动",
            Self::GroupAdd => "新建分组",
            Self::RemoteMkdir { .. } => "新建",
            Self::RemoteRename { .. } => "重命名",
            Self::GroupRename(..) | Self::RemoteChmod { .. } => "保存",
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
        app.modules.ssh.form.use_keyring = connection
            .as_ref()
            .is_some_and(|connection| connection.use_keyring);
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
            let (name, host, port, user, password, identity, extra, forwards) = connection
                .map_or_else(
                    || {
                        (
                            String::new(),
                            String::new(),
                            "22".to_string(),
                            String::new(),
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
                            connection.forwards.join(" "),
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
                (&app.modules.ssh.form.forwards, forwards),
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
            Self::RemoteMkdir { parent } => {
                app.remote_mkdir(parent.clone(), value, cx);
                Ok(())
            }
            Self::RemoteRename { path, .. } => {
                app.remote_rename(path.clone(), value, cx);
                Ok(())
            }
            Self::RemoteDelete {
                path, directory, ..
            } => {
                app.remote_delete(path.clone(), *directory, cx);
                Ok(())
            }
            Self::RemoteChmod { path, .. } => {
                app.remote_chmod(path.clone(), value, cx);
                Ok(())
            }
        }
    }
}

pub(super) struct TestSpec {
    args: Vec<String>,
    password: Option<String>,
}

pub(super) fn test_spec(connection: &SshConnection) -> TestSpec {
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
    if connection.password_value().is_none() {
        args.extend(["-o".to_string(), "BatchMode=yes".to_string()]);
    }
    args.extend(connection.extra_args.iter().cloned());
    args.push(connection.target());
    args.push("true".to_string());
    TestSpec {
        args,
        password: connection.password_value().map(str::to_string),
    }
}

pub(super) fn run_test(spec: &TestSpec) -> (bool, String) {
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
