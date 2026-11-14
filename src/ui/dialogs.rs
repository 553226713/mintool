use crate::services::file_actions::FileAction;
use crate::{app::MintoolApp, ui::theme};
use adabraka_ui::components::button::{Button, ButtonSize, ButtonVariant};
use adabraka_ui::components::select::{Select, SelectOption};
use adabraka_ui::components::{input::Input, input_state::SelectAll};
use gpui::prelude::FluentBuilder;
use gpui::*;

#[derive(Clone)]
pub enum Dialog {
    AddProject(Option<String>),
    AddGroup,
    MoveProject(String, String),
    RenameProject(String, String),
    RenameGroup(String, String),
    Locate(String, String),
    Remove(String, String),
    RemoveGroup(String, String),
    Settings,
    File(FileAction),
}
impl MintoolApp {
    pub(crate) fn show_dialog(
        &mut self,
        dialog: Dialog,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dialog_generation += 1;
        self.dialog_error = None;
        if matches!(dialog, Dialog::Settings) {
            let detected = crate::services::system_terminal::SystemTerminal::detect();
            let automatic = detected
                .first()
                .map(|terminal| terminal.name)
                .unwrap_or("内置终端");
            let mut options = vec![
                SelectOption::new("auto".to_string(), format!("自动选择（{automatic}）")),
                SelectOption::new("builtin".to_string(), "内置终端"),
            ];
            options.extend(detected.into_iter().map(|terminal| {
                SelectOption::new(
                    terminal.name.to_string(),
                    format!("{}（嵌入 · X11）", terminal.name),
                )
            }));
            let selected = options
                .iter()
                .position(|option| option.value == self.settings.terminal_backend);
            if selected.is_none() {
                options.push(SelectOption::new(
                    self.settings.terminal_backend.clone(),
                    format!("{}（未安装）", self.settings.terminal_backend),
                ));
            }
            let selected = selected.unwrap_or(options.len() - 1);
            self.terminal_select = cx.new(|cx| {
                Select::new(cx)
                    .options(options)
                    .selected_index(Some(selected))
                    .w_full()
            });
        }
        let group = match &dialog {
            Dialog::AddProject(group) => Some(group.clone()),
            Dialog::MoveProject(id, _) => Some(
                self.projects
                    .projects
                    .iter()
                    .find(|p| &p.id == id)
                    .and_then(|p| p.group.clone()),
            ),
            _ => None,
        };
        if let Some(group) = group {
            let options: Vec<_> = std::iter::once(SelectOption::new(None, "未分组"))
                .chain(
                    self.projects
                        .groups
                        .iter()
                        .map(|g| SelectOption::new(Some(g.id.clone()), g.name.clone())),
                )
                .collect();
            let selected = options.iter().position(|o| o.value == group).unwrap_or(0);
            self.group_select = cx.new(|cx| {
                Select::new(cx)
                    .options(options)
                    .selected_index(Some(selected))
                    .w_full()
            });
        }
        let value = match &dialog {
            Dialog::RenameProject(_, name)
            | Dialog::RenameGroup(_, name)
            | Dialog::Locate(_, name) => name.clone(),
            Dialog::Settings => self.settings.shell.clone(),
            Dialog::File(FileAction::Rename(path)) => path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            _ => String::new(),
        };
        self.dialog_input.update(cx, |state, cx| {
            state.select_all(&SelectAll, window, cx);
            state.set_value(value, window, cx);
        });
        self.font_input.update(cx, |state, cx| {
            state.select_all(&SelectAll, window, cx);
            state.set_value(self.settings.font_size.to_string(), window, cx);
        });
        self.dialog = Some(dialog);
        cx.notify();
        self.dialog_input.read(cx).focus_handle(cx).focus(window);
    }
    pub(crate) fn submit_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.dialog.clone() else {
            return;
        };
        let value = self.dialog_input.read(cx).content().trim().to_string();
        if value.is_empty()
            && !matches!(
                dialog,
                Dialog::Remove(..)
                    | Dialog::RemoveGroup(..)
                    | Dialog::MoveProject(..)
                    | Dialog::File(FileAction::Trash(_))
            )
        {
            self.dialog_error = Some("请填写内容后再保存".into());
            cx.notify();
            return;
        }
        let result = match dialog {
            Dialog::File(action) => self.apply_file_action(&action, &value, cx),
            Dialog::AddProject(_) => {
                let group = self
                    .group_select
                    .read(cx)
                    .selected_value()
                    .cloned()
                    .flatten();
                self.add_path(value.into(), None, group, cx)
            }
            Dialog::Locate(id, _) => self.add_path(value.into(), Some(id), None, cx),
            Dialog::MoveProject(id, _) => {
                let group = self
                    .group_select
                    .read(cx)
                    .selected_value()
                    .cloned()
                    .flatten();
                self.projects.move_project(&id, group.clone(), None);
                if let Some(group) = self
                    .projects
                    .groups
                    .iter_mut()
                    .find(|g| Some(&g.id) == group.as_ref())
                {
                    group.collapsed = false;
                }
                Ok(())
            }
            Dialog::AddGroup => self.projects.add_group(&value).map(|_| ()),
            Dialog::RenameProject(id, _) => {
                if let Some(p) = self.projects.projects.iter_mut().find(|p| p.id == id) {
                    p.name = value;
                }
                Ok(())
            }
            Dialog::RenameGroup(id, _) => {
                if self
                    .projects
                    .groups
                    .iter()
                    .any(|g| g.id != id && g.name == value)
                {
                    Err(anyhow::anyhow!("分组名称已存在"))
                } else {
                    if let Some(g) = self.projects.groups.iter_mut().find(|g| g.id == id) {
                        g.name = value;
                    }
                    Ok(())
                }
            }
            Dialog::Remove(id, _) => {
                self.remove_project(&id, cx);
                Ok(())
            }
            Dialog::RemoveGroup(id, _) => {
                self.projects.remove_group(&id);
                Ok(())
            }
            Dialog::Settings => match self.font_input.read(cx).content().parse::<f32>() {
                Ok(size) if size.is_finite() && (10.0..=28.0).contains(&size) => {
                    self.settings.shell = value;
                    self.settings.font_size = size;
                    self.settings.terminal_backend = self
                        .terminal_select
                        .read(cx)
                        .selected_value()
                        .cloned()
                        .unwrap_or_else(|| "auto".into());
                    for terminal in self.terminals.values() {
                        terminal.update(cx, |view, cx| {
                            view.font_size = size;
                            cx.notify();
                        });
                    }
                    Ok(())
                }
                _ => Err(anyhow::anyhow!("字号应为 10 到 28 之间的数字")),
            },
        };
        match result {
            Ok(()) => {
                self.dialog = None;
                self.dialog_error = None;
                self.changed(cx);
            }
            Err(error) => {
                self.dialog_error = Some(format!("{error:#}"));
                cx.notify();
            }
        }
    }
    pub(crate) fn render_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dialog = self.dialog.clone()?;
        let title = match &dialog {
            Dialog::AddProject(_) => "添加项目",
            Dialog::AddGroup => "新建分组",
            Dialog::MoveProject(_, _) => "移动项目",
            Dialog::RenameProject(_, _) => "重命名项目",
            Dialog::RenameGroup(_, _) => "重命名分组",
            Dialog::Locate(_, _) => "重新定位目录",
            Dialog::Remove(_, _) => "移除项目",
            Dialog::RemoveGroup(_, _) => "删除分组",
            Dialog::Settings => "工作台设置",
            Dialog::File(action) => action.title(),
        };
        let directory = matches!(dialog, Dialog::AddProject(_) | Dialog::Locate(_, _));
        let settings = matches!(dialog, Dialog::Settings);
        let removing = matches!(
            dialog,
            Dialog::Remove(_, _) | Dialog::RemoveGroup(_, _) | Dialog::File(FileAction::Trash(_))
        );
        let moving = matches!(dialog, Dialog::MoveProject(..));
        let enabled = removing || moving || !self.dialog_input.read(cx).content().trim().is_empty();
        let app = cx.entity().downgrade();
        Some(
            div()
                .id("dialog-backdrop")
                .absolute()
                .inset_0()
                .bg(rgba(0x080b1399))
                .flex()
                .items_center()
                .justify_center()
                .occlude()
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" {
                        this.dialog = None;
                        cx.notify();
                        cx.stop_propagation();
                    } else if event.keystroke.key == "enter" {
                        this.submit_dialog(cx);
                        cx.stop_propagation();
                    }
                }))
                .child(
                    div()
                        .w(px(460.))
                        .rounded_xl()
                        .border_1()
                        .border_color(theme::border())
                        .shadow_2xl()
                        .bg(theme::panel())
                        .p_6()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(px(18.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(title),
                                )
                                .child(
                                    Button::new("cancel-icon", "")
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Sm)
                                        .icon("x")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.dialog = None;
                                            cx.notify();
                                        })),
                                ),
                        )
                        .when(removing, |card| {
                            card.child(div().text_color(theme::muted()).child(
                                if matches!(dialog, Dialog::File(FileAction::Trash(_))) {
                                    "将此项目及其内容移到系统回收站，可通过系统文件管理器恢复。"
                                } else if matches!(dialog, Dialog::RemoveGroup(..)) {
                                    "删除此分组，项目会移到未分组。保留磁盘文件和运行中的终端。"
                                } else {
                                    "只移除软件内的记录，保留磁盘文件。该项目的终端会关闭。"
                                },
                            ))
                        })
                        .when_some(
                            match &dialog {
                                Dialog::File(action) => Some(action.path().display().to_string()),
                                _ => None,
                            },
                            |card, path| {
                                card.child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(theme::muted())
                                        .child(path),
                                )
                            },
                        )
                        .when(matches!(dialog, Dialog::Locate(_, _)), |card| {
                            card.child(
                                div()
                                    .text_color(theme::muted())
                                    .child("保留布局，并在新目录重新启动终端。"),
                            )
                        })
                        .when_some(
                            match &dialog {
                                Dialog::Remove(_, name)
                                | Dialog::RemoveGroup(_, name)
                                | Dialog::MoveProject(_, name) => Some(name.clone()),
                                _ => None,
                            },
                            |card, name| {
                                card.child(div().font_weight(FontWeight::SEMIBOLD).child(name))
                            },
                        )
                        .when(!removing && !moving, |card| {
                            card.child(div().text_size(px(12.)).text_color(theme::muted()).child(
                                if directory {
                                    "项目目录的完整路径"
                                } else if settings {
                                    "默认 Shell（新终端生效）"
                                } else {
                                    "名称"
                                },
                            ))
                            .child(
                                Input::new(&self.dialog_input)
                                    .map(|mut input| {
                                        input.style().align_items = Some(AlignItems::Stretch);
                                        input
                                    })
                                    .w_full(),
                            )
                        })
                        .when(matches!(dialog, Dialog::AddProject(_)) || moving, |card| {
                            card.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme::muted())
                                    .child("所属分组"),
                            )
                            .child(self.group_select.clone())
                        })
                        .when(settings, |card| {
                            card.child(
                                div().text_size(px(12.)).text_color(theme::muted()).child("终端后端（新建终端生效）"),
                            )
                            .child(self.terminal_select.clone())
                            .child(div().text_size(px(11.)).text_color(theme::muted()).child("系统终端保持嵌在面板内；支持 Alacritty、xterm、urxvt、st，需 X11。自动模式无可用项时使用内置终端。Konsole / Ptyxis 不提供此嵌入接口。"))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme::muted())
                                    .child("终端字号（10–28）"),
                            )
                            .child(
                                Input::new(&self.font_input)
                                    .map(|mut input| {
                                        input.style().align_items = Some(AlignItems::Stretch);
                                        input
                                    })
                                    .w_full(),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(theme::muted())
                                    .child(format!("配置：{}", self.storage.root.display())),
                            )
                        })
                        .when_some(self.dialog_error.clone(), |card, error| {
                            card.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme::danger())
                                    .child(error),
                            )
                        })
                        .child(
                            div()
                                .pt_2()
                                .flex()
                                .gap_2()
                                .items_center()
                                .when(directory, |row| {
                                    row.child(
                                        Button::new("browse", "选择目录…")
                                            .icon("folder-open")
                                            .on_click(move |_, window, cx| {
                                                let _ = app.update(cx, |this, cx| {
                                                    this.choose_directory(window, cx)
                                                });
                                            }),
                                    )
                                })
                                .child(div().flex_1())
                                .child(
                                    Button::new("cancel-dialog", "取消")
                                        .variant(ButtonVariant::Outline)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.dialog = None;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new(
                                        "submit-dialog",
                                        match dialog {
                                            Dialog::File(FileAction::Trash(_)) => "移到回收站",
                                            Dialog::RemoveGroup(..) => "删除分组",
                                            Dialog::Remove(..) => "移除记录",
                                            Dialog::AddProject(..) => "添加项目",
                                            Dialog::MoveProject(..) => "移动",
                                            _ => "保存",
                                        },
                                    )
                                    .variant(ButtonVariant::Default)
                                    .disabled(!enabled)
                                    .on_click(cx.listener(|this, _, _, cx| this.submit_dialog(cx))),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}
