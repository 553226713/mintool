use crate::app::MintoolApp;
use crate::ui::dialogs::DialogContent;
use adabraka_ui::components::button::{Button, ButtonSize, ButtonVariant};
use gpui::*;

#[derive(Clone)]
pub enum DatabaseDialog {
    Connection {
        edit: Option<String>,
        group: Option<String>,
    },
    GroupAdd,
    GroupRename {
        id: String,
        name: String,
    },
    GroupRemove {
        id: String,
        name: String,
    },
    Remove {
        id: String,
        name: String,
    },
}

impl DialogContent for DatabaseDialog {
    fn title(&self) -> String {
        match self {
            Self::Connection { edit, .. } => {
                if edit.is_some() {
                    "编辑数据库连接"
                } else {
                    "新建数据库连接"
                }
            }
            Self::GroupAdd => "新建数据库分组",
            Self::GroupRename { .. } => "重命名数据库分组",
            Self::GroupRemove { .. } => "删除数据库分组",
            Self::Remove { .. } => "移除数据库连接",
        }
        .into()
    }

    fn accepts_empty_input(&self) -> bool {
        matches!(
            self,
            Self::Connection { .. } | Self::GroupRemove { .. } | Self::Remove { .. }
        )
    }

    fn input_hint(&self) -> Option<&'static str> {
        match self {
            Self::GroupAdd | Self::GroupRename { .. } => Some("分组名称"),
            _ => None,
        }
    }

    fn subject(&self) -> Option<String> {
        match self {
            Self::Remove { name, .. }
            | Self::GroupRename { name, .. }
            | Self::GroupRemove { name, .. } => Some(name.clone()),
            _ => None,
        }
    }

    fn note(&self, _app: &MintoolApp) -> Option<String> {
        match self {
            Self::Connection { .. } => Some("选择数据库类型和分组，再填写对应连接信息。".into()),
            Self::GroupRemove { .. } => Some("删除分组，连接会移到未分组。".into()),
            Self::Remove { .. } => Some("仅移除此连接配置，不会删除数据库或表数据。".into()),
            Self::GroupAdd | Self::GroupRename { .. } => None,
        }
    }

    fn confirm_label(&self) -> &'static str {
        match self {
            Self::Connection { .. } => "保存连接",
            Self::GroupAdd => "新建分组",
            Self::GroupRename { .. } => "保存",
            Self::GroupRemove { .. } | Self::Remove { .. } => "删除",
        }
    }

    fn enabled(&self, app: &MintoolApp, cx: &mut Context<MintoolApp>) -> bool {
        if matches!(self, Self::Connection { .. }) {
            let form = &app.modules.database.form;
            !form.name.read(cx).content().trim().is_empty()
                || !form.host.read(cx).content().trim().is_empty()
                || !form.path.read(cx).content().trim().is_empty()
        } else {
            self.accepts_empty_input() || !app.dialog_input.read(cx).content().trim().is_empty()
        }
    }

    fn prepare(&self, app: &mut MintoolApp, window: &mut Window, cx: &mut Context<MintoolApp>) {
        if let Self::Connection { edit, group } = self {
            app.prepare_database_form(edit.clone(), group.clone(), window, cx);
        }
    }

    fn focus(&self, app: &MintoolApp, window: &mut Window, cx: &mut Context<MintoolApp>) {
        if matches!(self, Self::Connection { .. }) {
            app.modules
                .database
                .form
                .name
                .read(cx)
                .focus_handle(cx)
                .focus(window);
        } else {
            app.dialog_input.read(cx).focus_handle(cx).focus(window);
        }
    }

    fn render_extra(&self, app: &MintoolApp, cx: &mut Context<MintoolApp>) -> Option<AnyElement> {
        matches!(self, Self::Connection { .. })
            .then(|| app.render_database_form(cx).into_any_element())
    }

    fn render_footer(&self, app: &MintoolApp, cx: &mut Context<MintoolApp>) -> Option<AnyElement> {
        if !matches!(self, Self::Connection { .. }) {
            return None;
        }
        Some(
            div()
                .child(
                    Button::new(
                        "test-database-connection",
                        if app.modules.database.form.testing {
                            "测试中…"
                        } else {
                            "测试连接"
                        },
                    )
                    .variant(ButtonVariant::Outline)
                    .size(ButtonSize::Sm)
                    .disabled(app.modules.database.form.testing)
                    .on_click(cx.listener(|this, _, _, cx| this.test_database_connection(cx))),
                )
                .into_any_element(),
        )
    }

    fn submit(
        &self,
        app: &mut MintoolApp,
        _value: String,
        cx: &mut Context<MintoolApp>,
    ) -> anyhow::Result<()> {
        match self {
            Self::Connection { edit, .. } => {
                app.submit_database_connection(edit.clone(), cx)?;
            }
            Self::GroupAdd => {
                let value = app.dialog_input.read(cx).content().trim().to_string();
                app.modules.database.registry.add_group(&value)?;
                app.changed(cx);
            }
            Self::GroupRename { id, .. } => {
                let value = app.dialog_input.read(cx).content().trim().to_string();
                app.modules.database.registry.rename_group(id, &value)?;
                app.changed(cx);
            }
            Self::GroupRemove { id, .. } => {
                app.modules.database.registry.remove_group(id);
                app.changed(cx);
            }
            Self::Remove { id, .. } => {
                app.remove_database_connection(id.clone(), cx);
            }
        }
        Ok(())
    }
}
