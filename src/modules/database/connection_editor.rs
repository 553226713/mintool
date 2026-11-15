use super::{
    driver,
    model::{expand_path, DatabaseConnection, DatabaseEngine},
};
use crate::app::MintoolApp;
use crate::ui::theme;
use adabraka_ui::components::{
    button::{Button, ButtonSize, ButtonVariant},
    input::Input,
    input_state::SelectAll,
    select::{Select, SelectOption},
};
use gpui::{prelude::FluentBuilder, *};
use std::path::PathBuf;

impl MintoolApp {
    pub(crate) fn prepare_database_form(
        &mut self,
        edit: Option<String>,
        group: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let connection = edit.as_ref().and_then(|id| {
            self.modules
                .database
                .registry
                .connections
                .iter()
                .find(|connection| &connection.id == id)
                .cloned()
        });
        let engine = connection
            .as_ref()
            .map_or(DatabaseEngine::Sqlite, |connection| connection.engine);
        let engine_options = DatabaseEngine::ALL
            .into_iter()
            .map(|candidate| SelectOption::new(candidate, candidate.label()))
            .collect::<Vec<_>>();
        let selected_engine = DatabaseEngine::ALL
            .iter()
            .position(|candidate| *candidate == engine)
            .unwrap_or(0);
        self.modules.database.form.engine = cx.new(|cx| {
            Select::new(cx)
                .options(engine_options)
                .selected_index(Some(selected_engine))
        });
        let engine_subscription = cx
            .subscribe(&self.modules.database.form.engine, |_, _, _, cx| {
                cx.notify()
            });
        self.keep_subscription(engine_subscription);

        let selected_group = connection
            .as_ref()
            .and_then(|connection| connection.group.clone())
            .or(group);
        let options: Vec<_> = std::iter::once(SelectOption::new(None, "未分组"))
            .chain(
                self.modules
                    .database
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
        self.modules.database.form.group = cx.new(|cx| {
            Select::new(cx)
                .options(options)
                .selected_index(Some(selected))
        });

        let connection = connection.unwrap_or_default();
        let values = [
            (connection.name, &self.modules.database.form.name),
            (
                connection.path.display().to_string(),
                &self.modules.database.form.path,
            ),
            (connection.host, &self.modules.database.form.host),
            (
                if connection.port != 0 {
                    connection.port.to_string()
                } else {
                    engine.default_port().to_string()
                },
                &self.modules.database.form.port,
            ),
            (connection.database, &self.modules.database.form.database),
            (connection.user, &self.modules.database.form.user),
            (
                connection.password.unwrap_or_default(),
                &self.modules.database.form.password,
            ),
        ];
        for (value, input) in values {
            input.update(cx, |state, cx| {
                state.select_all(&SelectAll, window, cx);
                state.set_value(value, window, cx);
            });
        }
        self.modules.database.form.testing = false;
        self.modules.database.form.test_status = None;
    }

    pub(crate) fn submit_database_connection(
        &mut self,
        edit: Option<String>,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<String> {
        let mut connection = self.database_connection_from_form(edit, cx)?;
        let id = self.modules.database.registry.upsert(connection.clone())?;
        connection.id = id.clone();
        if let Some(group) = &connection.group {
            if let Some(group) = self
                .modules
                .database
                .registry
                .groups
                .iter_mut()
                .find(|candidate| &candidate.id == group)
            {
                group.collapsed = false;
            }
        }
        self.modules.database.catalogs.remove(&id);
        self.modules.database.expanded_nodes.remove(&id);
        self.open_database_connection(id.clone(), cx);
        Ok(id)
    }

    pub(crate) fn test_database_connection(&mut self, cx: &mut Context<Self>) {
        if self.modules.database.form.testing {
            return;
        }
        let connection = match self.database_connection_from_form(None, cx) {
            Ok(connection) => connection,
            Err(error) => {
                self.modules.database.form.test_status = Some((false, format!("{error:#}")));
                cx.notify();
                return;
            }
        };
        self.modules.database.form.testing = true;
        self.modules.database.form.test_status = None;
        let executor = cx.background_executor().clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move { driver::test(&connection) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.modules.database.form.testing = false;
                this.modules.database.form.test_status = Some(match result {
                    Ok(()) => (true, "连接成功".into()),
                    Err(error) => (false, format!("{error:#}")),
                });
                cx.notify();
            });
        })
        .detach();
    }

    fn database_connection_from_form(
        &self,
        edit: Option<String>,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<DatabaseConnection> {
        let form = &self.modules.database.form;
        let engine = form
            .engine
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or(DatabaseEngine::Sqlite);
        let host = form.host.read(cx).content().trim().to_string();
        let database = form.database.read(cx).content().trim().to_string();
        let path_text = form.path.read(cx).content().trim().to_string();
        let name = form.name.read(cx).content().trim().to_string();
        let port_text = form.port.read(cx).content().trim().to_string();
        let port = if port_text.is_empty() {
            engine.default_port()
        } else {
            port_text
                .parse::<u16>()
                .ok()
                .filter(|port| *port > 0)
                .ok_or_else(|| anyhow::anyhow!("端口应为 1 到 65535 之间的数字"))?
        };
        let path = if engine.is_file() {
            anyhow::ensure!(!path_text.is_empty(), "请选择或新建 SQLite 文件");
            let requested = expand_path(&path_text);
            let parent = requested
                .parent()
                .filter(|parent| parent.is_dir())
                .ok_or_else(|| anyhow::anyhow!("SQLite 文件目录不存在"))?;
            if !requested.exists() {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&requested)?;
            }
            parent.canonicalize()?.join(
                requested
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("文件名无效"))?,
            )
        } else {
            anyhow::ensure!(!host.is_empty(), "请填写数据库主机");
            anyhow::ensure!(
                !form.user.read(cx).content().trim().is_empty(),
                "请填写用户名"
            );
            PathBuf::new()
        };
        let display_name = if !name.is_empty() {
            name
        } else if engine.is_file() {
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        } else if !database.is_empty() {
            database.clone()
        } else {
            host.clone()
        };
        Ok(DatabaseConnection {
            id: edit.unwrap_or_default(),
            name: display_name,
            engine,
            path,
            group: form.group.read(cx).selected_value().cloned().flatten(),
            host,
            port,
            database,
            user: form.user.read(cx).content().trim().to_string(),
            password: {
                let password = form.password.read(cx).content().to_string();
                (!password.is_empty()).then_some(password)
            },
        })
    }

    pub(crate) fn render_database_form(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let form = &self.modules.database.form;
        let engine = form
            .engine
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or(DatabaseEngine::Sqlite);
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
                .child(input.w_full())
        };
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme::muted())
                                    .child("数据库类型"),
                            )
                            .child(form.engine.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme::muted())
                                    .child("分组"),
                            )
                            .child(form.group.clone()),
                    ),
            )
            .child(field("连接名称", Input::new(&form.name)))
            .when(engine.is_file(), |body| {
                body.child(field("SQLite 文件", Input::new(&form.path)))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("database-form-open", "选择文件…")
                                    .variant(ButtonVariant::Outline)
                                    .size(ButtonSize::Sm)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.choose_database_form_file(false, window, cx)
                                    })),
                            )
                            .child(
                                Button::new("database-form-new", "新建 SQLite…")
                                    .variant(ButtonVariant::Outline)
                                    .size(ButtonSize::Sm)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.choose_database_form_file(true, window, cx)
                                    })),
                            ),
                    )
            })
            .when(!engine.is_file(), |body| {
                body.child(field("主机", Input::new(&form.host)))
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(
                                div()
                                    .w(px(110.))
                                    .child(field("端口", Input::new(&form.port))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .child(field("数据库（可选）", Input::new(&form.database))),
                            ),
                    )
                    .child(field("用户名", Input::new(&form.user)))
                    .child(field("密码（可选）", Input::new(&form.password).password(true)))
            })
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(theme::muted())
                    .child(if engine.is_file() {
                        "SQLite 文件可直接选择已有文件，或新建文件后添加连接。"
                    } else {
                        "PostgreSQL / MySQL / MariaDB 使用原生驱动连接；密码会保存在权限为 0600 的数据库配置中。"
                    }),
            )
            .when(form.testing, |body| {
                body.child(
                    div()
                        .text_size(px(11.))
                        .text_color(theme::warning())
                        .child("正在测试连接…"),
                )
            })
            .when_some(form.test_status.clone(), |body, (success, message)| {
                body.child(
                    div()
                        .text_size(px(11.))
                        .text_color(if success {
                            theme::success()
                        } else {
                            theme::danger()
                        })
                        .child(message),
                )
            })
    }
}
