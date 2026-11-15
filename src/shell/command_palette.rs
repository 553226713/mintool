//! Ctrl+Shift+P command palette for cross-module actions.

use crate::app::MintoolApp;
use crate::modules::projects::dialogs::ProjectDialog;
use crate::modules::ModuleId;
use crate::ui::dialogs::Dialog;
use crate::ui::theme;
use adabraka_ui::components::input::Input;
use adabraka_ui::components::input_state::InputState;
use gpui::prelude::FluentBuilder;
use gpui::*;

pub struct CommandPalette {
    pub input: Entity<InputState>,
    pub selected: usize,
}

#[derive(Clone, Copy)]
enum PaletteAction {
    AddProject,
    Settings,
    ToggleTheme,
    QuickOpen,
    NewTerminal,
    RefreshDocker,
    ImportSsh,
    AddDatabase,
    SwitchModule(ModuleId),
}

struct Command {
    label: &'static str,
    hint: &'static str,
    action: PaletteAction,
}

fn commands() -> Vec<Command> {
    vec![
        Command {
            label: "添加项目…",
            hint: "项目",
            action: PaletteAction::AddProject,
        },
        Command {
            label: "快速打开文件…",
            hint: "Ctrl+P",
            action: PaletteAction::QuickOpen,
        },
        Command {
            label: "新建终端",
            hint: "当前会话",
            action: PaletteAction::NewTerminal,
        },
        Command {
            label: "工作台设置…",
            hint: "应用",
            action: PaletteAction::Settings,
        },
        Command {
            label: "切换深色/浅色主题",
            hint: "外观",
            action: PaletteAction::ToggleTheme,
        },
        Command {
            label: "从 ~/.ssh/config 导入 SSH 连接",
            hint: "SSH",
            action: PaletteAction::ImportSsh,
        },
        Command {
            label: "刷新 Docker 资源",
            hint: "Docker",
            action: PaletteAction::RefreshDocker,
        },
        Command {
            label: "新建数据库连接…",
            hint: "数据库",
            action: PaletteAction::AddDatabase,
        },
        Command {
            label: "切换到项目",
            hint: "模块",
            action: PaletteAction::SwitchModule(ModuleId::Projects),
        },
        Command {
            label: "切换到 SSH",
            hint: "模块",
            action: PaletteAction::SwitchModule(ModuleId::Ssh),
        },
        Command {
            label: "切换到 Docker",
            hint: "模块",
            action: PaletteAction::SwitchModule(ModuleId::Docker),
        },
        Command {
            label: "切换到数据库",
            hint: "模块",
            action: PaletteAction::SwitchModule(ModuleId::Database),
        },
    ]
}

impl MintoolApp {
    pub(crate) fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.command_palette.is_some() {
            self.close_command_palette(cx);
            return;
        }
        let input = cx.new(|cx| InputState::new(cx).placeholder("输入命令…"));
        let subscription = cx.subscribe(&input, |this, _, event, cx| {
            if matches!(
                event,
                adabraka_ui::components::input_state::InputEvent::Change
            ) {
                if let Some(palette) = &mut this.command_palette {
                    palette.selected = 0;
                }
                cx.notify();
            }
        });
        self.command_palette_subscription = Some(subscription);
        input.update(cx, |state, cx| {
            state.focus_handle(cx).focus(window);
        });
        self.command_palette = Some(CommandPalette { input, selected: 0 });
        cx.notify();
    }

    pub(crate) fn close_command_palette(&mut self, cx: &mut Context<Self>) {
        self.command_palette = None;
        self.command_palette_subscription = None;
        cx.notify();
    }

    pub(crate) fn move_command_palette(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(palette) = &mut self.command_palette else {
            return;
        };
        let filter = palette.input.read(cx).content().trim().to_lowercase();
        let count = commands()
            .iter()
            .filter(|command| command.label.to_lowercase().contains(&filter))
            .count();
        if count == 0 {
            return;
        }
        palette.selected = (palette.selected as isize + delta).rem_euclid(count as isize) as usize;
        cx.notify();
    }

    pub(crate) fn run_command_palette_selected(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(palette) = &self.command_palette else {
            return;
        };
        let filter = palette.input.read(cx).content().trim().to_lowercase();
        let commands: Vec<_> = commands()
            .into_iter()
            .filter(|command| command.label.to_lowercase().contains(&filter))
            .collect();
        let Some(command) = commands.get(palette.selected) else {
            return;
        };
        let action = command.action;
        self.close_command_palette(cx);
        match action {
            PaletteAction::AddProject => {
                self.show_dialog(Dialog::Project(ProjectDialog::AddProject(None)), window, cx)
            }
            PaletteAction::Settings => self.show_dialog(Dialog::Settings, window, cx),
            PaletteAction::ToggleTheme => {
                self.settings.appearance = self.settings.appearance.toggle();
                theme::install(cx, self.settings.appearance);
                self.changed(cx);
            }
            PaletteAction::QuickOpen => self.open_quick_open(window, cx),
            PaletteAction::NewTerminal => {
                if let Some(key) = self.active_session_key() {
                    self.ensure_session(&key);
                    if let Some(workspace) = self.workspaces.get(&key) {
                        let focused = workspace.state.focused;
                        self.split(focused, crate::workspace::layout::Axis::Horizontal, cx);
                    }
                }
            }
            PaletteAction::RefreshDocker => self.refresh_docker(cx),
            PaletteAction::ImportSsh => self.import_ssh_config(cx),
            PaletteAction::AddDatabase => self.show_dialog(
                Dialog::Database(
                    crate::modules::database::dialogs::DatabaseDialog::Connection {
                        edit: None,
                        group: None,
                    },
                ),
                window,
                cx,
            ),
            PaletteAction::SwitchModule(module) => self.switch_module(module, cx),
        }
    }

    pub(crate) fn render_command_palette(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let palette = self.command_palette.as_ref()?;
        let filter = palette.input.read(cx).content().trim().to_lowercase();
        let commands: Vec<_> = commands()
            .into_iter()
            .filter(|command| command.label.to_lowercase().contains(&filter))
            .collect();
        let selected = palette.selected.min(commands.len().saturating_sub(1));
        let rows: Vec<AnyElement> = commands
            .iter()
            .enumerate()
            .map(|(index, command)| {
                let label = command.label;
                div()
                    .id(SharedString::from(format!("command-{index}")))
                    .h(px(32.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .cursor_pointer()
                    .when(index == selected, |row| row.bg(theme::selected()))
                    .hover(|row| row.bg(theme::elevated()))
                    .child(div().flex_1().child(label))
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme::muted())
                            .child(command.hint),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(palette) = &mut this.command_palette {
                            palette.selected = index;
                        }
                        this.run_command_palette_selected(window, cx);
                    }))
                    .into_any_element()
            })
            .collect();
        Some(
            div()
                .absolute()
                .top(px(64.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    div()
                        .w(px(560.))
                        .max_h(px(520.))
                        .flex()
                        .flex_col()
                        .rounded_xl()
                        .border_1()
                        .border_color(theme::border())
                        .shadow_2xl()
                        .bg(theme::panel())
                        .overflow_hidden()
                        .child(
                            div()
                                .p_3()
                                .border_b_1()
                                .border_color(theme::border())
                                .child(
                                    Input::new(&palette.input)
                                        .map(|mut input| {
                                            input.style().align_items = Some(AlignItems::Stretch);
                                            input
                                        })
                                        .w_full(),
                                ),
                        )
                        .child(
                            div()
                                .id("command-results")
                                .max_h(px(440.))
                                .overflow_y_scroll()
                                .py_1()
                                .when(commands.is_empty(), |list| {
                                    list.child(
                                        div()
                                            .p_4()
                                            .text_size(px(11.))
                                            .text_color(theme::muted())
                                            .child("没有匹配的命令"),
                                    )
                                })
                                .children(rows),
                        )
                        .child(
                            div()
                                .px_3()
                                .py_2()
                                .border_t_1()
                                .border_color(theme::border())
                                .text_size(px(10.))
                                .text_color(theme::muted())
                                .child("↑↓ 选择 · Enter 执行 · Esc 关闭"),
                        ),
                )
                .into_any_element(),
        )
    }
}
