use super::TerminalView;
use crate::{
    services::{mutex::lock_unpoisoned, terminal_text},
    ui::{common::text_input::text_input, theme},
};
use adabraka_ui::components::button::{Button, ButtonSize, ButtonVariant};
use gpui::*;
use gpui_component::input::{InputEvent, InputState};

impl TerminalView {
    pub(crate) fn open_terminal_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_input.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("查找终端历史输出…"));
            self.search_subscription = Some(cx.subscribe(&input, |view, input, event, cx| {
                match event {
                    InputEvent::Change => {
                        let query = input.read(cx).value();
                        {
                            let session = lock_unpoisoned(&view.session);
                            let parser = lock_unpoisoned(&session.parser);
                            view.search_matches = terminal_text::find(parser.screen(), &query);
                        }
                        view.search_index = view.search_matches.len().saturating_sub(1);
                        view.show_terminal_match(cx);
                    }
                    InputEvent::PressEnter { secondary } => {
                        view.next_terminal_match(if *secondary { -1 } else { 1 }, cx)
                    }
                    _ => {}
                }
                cx.notify();
            }));
            self.search_input = Some(input);
        }
        self.search_input
            .as_ref()
            .unwrap()
            .update(cx, |state, cx| state.focus(window, cx));
        cx.notify();
    }
    pub(crate) fn show_terminal_match(&mut self, cx: &mut Context<Self>) {
        if let Some(&(row, left, right)) = self.search_matches.get(self.search_index) {
            let mut session = lock_unpoisoned(&self.session);
            let history = lock_unpoisoned(&session.parser).screen().scrollback_len();
            let offset = history.saturating_sub(row.saturating_sub(session.size.0 as usize / 2));
            session.set_scrollback_position(offset);
            self.selection = Some(((row, left), (row, right)));
        }
        cx.notify();
    }
    pub(crate) fn next_terminal_match(&mut self, delta: isize, cx: &mut Context<Self>) {
        if let Some(input) = &self.search_input {
            let query = input.read(cx).value();
            let session = lock_unpoisoned(&self.session);
            let parser = lock_unpoisoned(&session.parser);
            self.search_matches = terminal_text::find(parser.screen(), &query);
        }
        if self.search_matches.is_empty() {
            return;
        }
        self.search_index = (self.search_index as isize + delta)
            .rem_euclid(self.search_matches.len() as isize) as usize;
        self.show_terminal_match(cx);
    }
    pub(crate) fn render_terminal_search(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let input = self.search_input.as_ref()?;
        Some(
            div()
                .absolute()
                .top_1()
                .right(px(20.))
                .w(px(390.))
                .max_w_full()
                .p_2()
                .rounded_md()
                .bg(theme::panel())
                .border_1()
                .border_color(theme::border())
                .flex()
                .items_center()
                .gap_1()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .capture_action(cx.listener(
                    |view, _: &gpui_component::input::Escape, window, cx| {
                        view.close_terminal_search(window, cx);
                        cx.stop_propagation();
                    },
                ))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .child(text_input(input)),
                )
                .child(format!(
                    "{}/{}",
                    if self.search_matches.is_empty() {
                        0
                    } else {
                        self.search_index + 1
                    },
                    self.search_matches.len()
                ))
                .child(
                    Button::new("terminal-find-prev", "↑")
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Ghost)
                        .on_click(cx.listener(|view, _, _, cx| view.next_terminal_match(-1, cx))),
                )
                .child(
                    Button::new("terminal-find-next", "↓")
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Ghost)
                        .on_click(cx.listener(|view, _, _, cx| view.next_terminal_match(1, cx))),
                )
                .child(
                    Button::new("terminal-find-close", "×")
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Ghost)
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.close_terminal_search(window, cx);
                        })),
                )
                .into_any_element(),
        )
    }
    pub(crate) fn close_terminal_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_input = None;
        self.search_subscription = None;
        self.search_matches.clear();
        self.selection = None;
        self.focus.focus(window);
        cx.notify();
    }
    pub(crate) fn select_terminal_word(
        &mut self,
        cell: (usize, u16),
        whole_line: bool,
        cx: &mut Context<Self>,
    ) {
        let session = lock_unpoisoned(&self.session);
        let parser = lock_unpoisoned(&session.parser);
        let mut screen = parser.screen().clone();
        let history = screen.scrollback_len();
        screen.set_scrollback(history.saturating_sub(cell.0));
        let row = (cell.0 - (history - screen.scrollback())) as u16;
        let columns = session.size.1;
        let word = |column| {
            screen.cell(row, column).is_some_and(|cell| {
                cell.is_wide_continuation()
                    || cell.contents().chars().any(|character| {
                        character.is_alphanumeric() || "_-./~:@".contains(character)
                    })
            })
        };
        let mut left = cell.1;
        let mut right = cell.1;
        if whole_line {
            left = 0;
            right = columns - 1;
        } else {
            let is_word = word(cell.1);
            while left > 0 && word(left - 1) == is_word {
                left -= 1;
            }
            while right + 1 < columns && word(right + 1) == is_word {
                right += 1;
            }
        }
        self.selection = Some(((cell.0, left), (cell.0, right)));
        cx.notify();
    }
}
