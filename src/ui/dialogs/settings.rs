//! Application settings dialog (app-level, not owned by a feature module).

use super::DialogContent;
use crate::app::MintoolApp;
use crate::ui::theme;
use adabraka_ui::components::input::Input;
use adabraka_ui::components::input_state::SelectAll;
use adabraka_ui::components::select::{Select, SelectOption};
use gpui::prelude::FluentBuilder;
use gpui::*;

struct SettingsDialog;
static SETTINGS_DIALOG: SettingsDialog = SettingsDialog;

impl DialogContent for SettingsDialog {
    fn title(&self) -> String {
        "工作台设置".into()
    }
    fn input_value(&self, app: &MintoolApp) -> String {
        app.settings.shell.clone()
    }
    fn input_hint(&self) -> Option<&'static str> {
        Some("默认 Shell（新终端生效）")
    }
    fn render_extra(&self, app: &MintoolApp, _cx: &mut Context<MintoolApp>) -> Option<AnyElement> {
        Some(
            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme::muted())
                        .child("终端后端（新建终端生效）"),
                )
                .child(app.terminal_select.clone())
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(theme::muted())
                        .child("系统终端保持嵌在面板内；支持 Alacritty、xterm、urxvt、st，需 X11。自动模式无可用项时使用内置终端。Konsole / Ptyxis 不提供此嵌入接口。"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme::muted())
                        .child("终端字号（10–28）"),
                )
                .child(
                    Input::new(&app.font_input)
                        .map(|mut input| {
                            input.style().align_items = Some(AlignItems::Stretch);
                            input
                        })
                        .w_full(),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme::muted())
                        .child("终端滚动行数（200–20000）"),
                )
                .child(
                    Input::new(&app.scrollback_input)
                        .map(|mut input| {
                            input.style().align_items = Some(AlignItems::Stretch);
                            input
                        })
                        .w_full(),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme::muted())
                        .child("Docker 自动刷新间隔（2–60 秒）"),
                )
                .child(
                    Input::new(&app.refresh_input)
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
                        .child(format!("配置：{}", app.storage.root.display())),
                )
                .into_any_element(),
        )
    }
    fn prepare(&self, app: &mut MintoolApp, window: &mut Window, cx: &mut Context<MintoolApp>) {
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
            .position(|option| option.value == app.settings.terminal_backend);
        if selected.is_none() {
            options.push(SelectOption::new(
                app.settings.terminal_backend.clone(),
                format!("{}（未安装）", app.settings.terminal_backend),
            ));
        }
        let selected = selected.unwrap_or(options.len() - 1);
        app.terminal_select = cx.new(|cx| {
            Select::new(cx)
                .options(options)
                .selected_index(Some(selected))
                .w_full()
        });
        let font_size = app.settings.font_size.to_string();
        app.font_input.update(cx, |state, cx| {
            state.select_all(&SelectAll, window, cx);
            state.set_value(font_size, window, cx);
        });
        let refresh = app.settings.docker_refresh_seconds.to_string();
        app.refresh_input.update(cx, |state, cx| {
            state.select_all(&SelectAll, window, cx);
            state.set_value(refresh, window, cx);
        });
        let scrollback = app.settings.terminal_scrollback.to_string();
        app.scrollback_input.update(cx, |state, cx| {
            state.select_all(&SelectAll, window, cx);
            state.set_value(scrollback, window, cx);
        });
    }
    fn submit(
        &self,
        app: &mut MintoolApp,
        value: String,
        cx: &mut Context<MintoolApp>,
    ) -> anyhow::Result<()> {
        let size = app.font_input.read(cx).content().parse::<f32>();
        let Ok(size) = size else {
            return Err(anyhow::anyhow!("字号应为 10 到 28 之间的数字"));
        };
        if !size.is_finite() || !(10.0..=28.0).contains(&size) {
            return Err(anyhow::anyhow!("字号应为 10 到 28 之间的数字"));
        }
        let refresh = app.refresh_input.read(cx).content().parse::<u64>();
        let Ok(refresh) = refresh else {
            return Err(anyhow::anyhow!("刷新间隔应为 2 到 60 之间的数字"));
        };
        if !(2..=60).contains(&refresh) {
            return Err(anyhow::anyhow!("刷新间隔应为 2 到 60 之间的数字"));
        }
        app.settings.shell = value;
        app.settings.font_size = size;
        let scrollback = app.scrollback_input.read(cx).content().parse::<usize>();
        let Ok(scrollback) = scrollback else {
            return Err(anyhow::anyhow!("滚动行数应为 200 到 20000 之间的数字"));
        };
        if !(200..=20_000).contains(&scrollback) {
            return Err(anyhow::anyhow!("滚动行数应为 200 到 20000 之间的数字"));
        }
        app.settings.docker_refresh_seconds = refresh;
        app.settings.terminal_scrollback = scrollback;
        crate::services::terminal::set_scrollback(scrollback);
        app.settings.terminal_backend = app
            .terminal_select
            .read(cx)
            .selected_value()
            .cloned()
            .unwrap_or_else(|| "auto".into());
        for terminal in app.terminals.values() {
            terminal.update(cx, |view, cx| {
                view.font_size = size;
                cx.notify();
            });
        }
        Ok(())
    }
}

pub(super) fn dialog() -> &'static dyn DialogContent {
    &SETTINGS_DIALOG
}
