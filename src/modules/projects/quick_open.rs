//! Ctrl+P quick file opener for the active project.

use crate::app::MintoolApp;
use crate::ui::theme;
use adabraka_ui::components::input::Input;
use adabraka_ui::components::input_state::InputState;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::path::{Path, PathBuf};

pub struct QuickOpen {
    pub input: Entity<InputState>,
    pub files: Vec<PathBuf>,
    pub selected: usize,
}

const MAX_FILES: usize = 5000;

impl MintoolApp {
    pub(crate) fn open_quick_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.quick_open.is_some() {
            self.close_quick_open(cx);
            return;
        }
        let Some(project) = self.current() else {
            return;
        };
        let mut files = Vec::new();
        collect_files(&project.path, &project.path, 0, &mut files);
        let input = cx.new(|cx| InputState::new(cx).placeholder("搜索项目文件…"));
        let subscription = cx.subscribe(&input, |this, _, event, cx| {
            if matches!(
                event,
                adabraka_ui::components::input_state::InputEvent::Change
            ) {
                if let Some(quick) = &mut this.quick_open {
                    quick.selected = 0;
                }
                cx.notify();
            }
        });
        self.quick_open_subscription = Some(subscription);
        input.update(cx, |state, cx| {
            state.focus_handle(cx).focus(window);
        });
        self.quick_open = Some(QuickOpen {
            input,
            files,
            selected: 0,
        });
        cx.notify();
    }

    pub(crate) fn close_quick_open(&mut self, cx: &mut Context<Self>) {
        self.quick_open = None;
        self.quick_open_subscription = None;
        cx.notify();
    }

    pub(crate) fn move_quick_open(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(quick) = &mut self.quick_open else {
            return;
        };
        let filter = quick.input.read(cx).content().trim().to_lowercase();
        let count = quick.matches(&filter).len();
        if count == 0 {
            return;
        }
        quick.selected = (quick.selected as isize + delta).rem_euclid(count as isize) as usize;
        cx.notify();
    }

    pub(crate) fn open_quick_open_selected(&mut self, cx: &mut Context<Self>) {
        let Some(quick) = &self.quick_open else {
            return;
        };
        let filter = quick.input.read(cx).content().trim().to_lowercase();
        let Some(path) = quick.matches(&filter).get(quick.selected).cloned() else {
            return;
        };
        self.close_quick_open(cx);
        self.open_file(path, cx);
    }

    pub(crate) fn render_quick_open(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let quick = self.quick_open.as_ref()?;
        let filter = quick.input.read(cx).content().trim().to_lowercase();
        let matches = quick.matches(&filter);
        let selected = quick.selected.min(matches.len().saturating_sub(1));
        let rows: Vec<AnyElement> = matches
            .iter()
            .take(50)
            .enumerate()
            .map(|(index, path)| {
                let target = path.clone();
                div()
                    .id(SharedString::from(format!("quick-open-{}", path.display())))
                    .h(px(30.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .when(index == selected, |row| row.bg(theme::selected()))
                    .hover(|row| row.bg(theme::elevated()))
                    .child(
                        div().flex_none().child(
                            path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned(),
                        ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(10.))
                            .text_color(theme::muted())
                            .child(path.parent().unwrap_or(path).display().to_string()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.close_quick_open(cx);
                        this.open_file(target.clone(), cx);
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
                                    Input::new(&quick.input)
                                        .map(|mut input| {
                                            input.style().align_items = Some(AlignItems::Stretch);
                                            input
                                        })
                                        .w_full(),
                                ),
                        )
                        .child(
                            div()
                                .id("quick-open-results")
                                .flex_1()
                                .min_h_0()
                                .max_h(px(440.))
                                .overflow_y_scroll()
                                .py_1()
                                .when(matches.is_empty(), |list| {
                                    list.child(
                                        div()
                                            .p_4()
                                            .text_size(px(11.))
                                            .text_color(theme::muted())
                                            .child(if filter.is_empty() {
                                                "没有可打开的文件"
                                            } else {
                                                "没有匹配的文件"
                                            }),
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
                                .child("↑↓ 选择 · Enter 打开 · Esc 关闭"),
                        ),
                )
                .into_any_element(),
        )
    }
}

impl QuickOpen {
    fn matches(&self, filter: &str) -> Vec<PathBuf> {
        if filter.is_empty() {
            return self.files.clone();
        }
        self.files
            .iter()
            .filter(|path| path.to_string_lossy().to_lowercase().contains(filter))
            .cloned()
            .collect()
    }
}

fn collect_files(root: &Path, path: &Path, depth: usize, files: &mut Vec<PathBuf>) {
    if depth > 16 || files.len() >= MAX_FILES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        if files.len() >= MAX_FILES {
            return;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".git" || name == "target" || name == "node_modules" || name == ".cache" {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            collect_files(root, &path, depth + 1, files);
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    let _ = root;
}
