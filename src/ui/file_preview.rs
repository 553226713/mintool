use crate::services::file_changes::{ChangeKind, LineChange};
use crate::ui::theme;
use adabraka_ui::components::{
    button::{Button, ButtonSize, ButtonVariant},
    text::Text,
};
use gpui::*;
use gpui_component::text::TextView;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
enum RichKind {
    Markdown,
    Html,
}

fn rich_kind(path: &Path) -> Option<RichKind> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())?
        .to_ascii_lowercase()
        .as_str()
    {
        "md" | "markdown" | "mdown" | "mkd" => Some(RichKind::Markdown),
        "html" | "htm" => Some(RichKind::Html),
        _ => None,
    }
}

/// Read-only, virtualized file viewer. Selection and scrolling never modify disk content.
pub struct FilePreview {
    path: PathBuf,
    text: String,
    height: f32,
    lines: Vec<SharedString>,
    focus: FocusHandle,
    selection: Option<(usize, usize)>,
    selecting: bool,
    changes: Vec<LineChange>,
    active_change: usize,
    scroll: UniformListScrollHandle,
    rich_kind: Option<RichKind>,
    rendered: bool,
}
impl FilePreview {
    pub fn new(path: PathBuf, text: String, height: f32, cx: &mut Context<Self>) -> Self {
        let rich_kind = rich_kind(&path);
        Self {
            path,
            lines: text
                .split('\n')
                .map(|line| SharedString::from(line.to_string()))
                .collect(),
            text,
            height,
            focus: cx.focus_handle(),
            selection: None,
            selecting: false,
            changes: Vec::new(),
            active_change: 0,
            scroll: UniformListScrollHandle::new(),
            rich_kind,
            rendered: false,
        }
    }
    pub fn resize(&mut self, height: f32, cx: &mut Context<Self>) {
        if (self.height - height).abs() > 0.5 {
            self.height = height;
            cx.notify();
        }
    }
    pub fn replace(&mut self, text: String, cx: &mut Context<Self>) {
        self.lines = text
            .split('\n')
            .map(|line| SharedString::from(line.to_string()))
            .collect();
        self.text = text;
        self.selection = None;
        cx.notify();
    }
    pub fn set_changes(&mut self, changes: Vec<LineChange>, cx: &mut Context<Self>) {
        if self.changes == changes {
            return;
        }
        let first_load = self.changes.is_empty();
        self.changes = changes;
        self.active_change = self.active_change.min(self.changes.len().saturating_sub(1));
        if first_load && !self.changes.is_empty() {
            self.jump_change(0, cx);
        }
        cx.notify();
    }
    pub fn changes_equal(&self, changes: &[LineChange]) -> bool {
        self.changes == changes
    }
    fn jump_change(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.changes.is_empty() {
            return;
        }
        self.active_change =
            (self.active_change as isize + delta).rem_euclid(self.changes.len() as isize) as usize;
        let line = self.changes[self.active_change]
            .start
            .min(self.lines.len().saturating_sub(1));
        self.scroll.scroll_to_item(line, ScrollStrategy::Center);
        cx.notify();
    }
    fn copy(&self, cx: &mut App) {
        let text = if let Some((a, b)) = self.selection {
            self.lines[a.min(b)..=a.max(b)]
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<&str>>()
                .join("\n")
        } else {
            self.text.clone()
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }
}
impl Render for FilePreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.rendered {
            if let Some(kind) = self.rich_kind {
                let text_view = match kind {
                    RichKind::Markdown => TextView::markdown(
                        ("markdown-preview", cx.entity_id()),
                        self.text.clone(),
                        window,
                        cx,
                    ),
                    RichKind::Html => TextView::html(
                        ("html-preview", cx.entity_id()),
                        self.text.clone(),
                        window,
                        cx,
                    ),
                }
                .selectable(true)
                .scrollable(true)
                .size_full();
                return div()
                    .id("rich-file-preview")
                    .w_full()
                    .h(px(self.height))
                    .flex_none()
                    .min_h_0()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(33.))
                            .px_3()
                            .flex_none()
                            .flex()
                            .items_center()
                            .text_size(px(10.))
                            .text_color(theme::muted())
                            .border_b_1()
                            .border_color(theme::border())
                            .child(div().flex_1().min_w_0().truncate().child(format!(
                                "{} · {}",
                                kind_label(kind),
                                self.path.file_name().unwrap_or_default().to_string_lossy()
                            )))
                            .child(
                                Button::new("source-preview", "源码")
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Sm)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.rendered = false;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .h(px((self.height - 33.).max(1.)))
                            .w_full()
                            .min_h_0()
                            .p_4()
                            .child(text_view),
                    )
                    .into_any_element();
            }
        }
        let rows = uniform_list(
            "source-lines",
            self.lines.len(),
            cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                range
                    .map(|index| {
                        let selected = this
                            .selection
                            .is_some_and(|(a, b)| index >= a.min(b) && index <= a.max(b));
                        let position = this.changes.partition_point(|change| change.start <= index);
                        let change = position
                            .checked_sub(1)
                            .and_then(|position| this.changes.get(position))
                            .filter(|change| index < change.start.saturating_add(change.count));
                        let marker = change.map(|change| match change.kind {
                            ChangeKind::Added => theme::success(),
                            ChangeKind::Modified => theme::warning(),
                            ChangeKind::Deleted => theme::danger(),
                        });
                        div()
                            .id(index)
                            .h(px(25.))
                            .flex()
                            .items_center()
                            .min_w_full()
                            .text_size(px(13.))
                            .font_family("JetBrains Mono")
                            .bg(if selected {
                                theme::selection()
                            } else if marker.is_some() {
                                theme::changed_background()
                            } else {
                                theme::background()
                            })
                            .child(
                                div()
                                    .w(px(3.))
                                    .h_full()
                                    .flex_none()
                                    .bg(marker.unwrap_or_else(theme::background)),
                            )
                            .child(
                                div()
                                    .w(px(48.))
                                    .flex_none()
                                    .pr_3()
                                    .text_right()
                                    .text_color(theme::muted())
                                    .child((index + 1).to_string()),
                            )
                            .child(
                                div().pr_6().child(
                                    Text::new(this.lines[index].clone())
                                        .no_wrap()
                                        .font("JetBrains Mono")
                                        .size(px(13.)),
                                ),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.focus.focus(window);
                                    this.selection = Some((index, index));
                                    this.selecting = true;
                                    cx.notify();
                                }),
                            )
                            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                                if this.selecting {
                                    if let Some((start, _)) = this.selection {
                                        this.selection = Some((start, index));
                                        cx.notify();
                                    }
                                }
                            }))
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(self.scroll.clone())
        .w_full()
        .h(px((self.height - 33.).max(1.)))
        .with_sizing_behavior(ListSizingBehavior::Auto)
        .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained);
        div()
            .id("file-preview")
            .w_full()
            .h(px(self.height))
            .flex_none()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .track_focus(&self.focus)
            .key_context("FilePreview")
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.modifiers.control {
                    match event.keystroke.key.as_str() {
                        "c" => {
                            this.copy(cx);
                            cx.stop_propagation();
                        }
                        "a" => {
                            this.selection = Some((0, this.lines.len().saturating_sub(1)));
                            cx.notify();
                            cx.stop_propagation();
                        }
                        _ => (),
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .child(
                div()
                    .h(px(33.))
                    .px_3()
                    .flex_none()
                    .flex()
                    .items_center()
                    .text_size(px(10.))
                    .text_color(theme::muted())
                    .child(div().flex_1().min_w_0().truncate().child(format!(
                        "只读 · {} 行 · 绿增 / 黄改 / 红删",
                        self.lines.len()
                    )))
                    .child(
                        Button::new("rendered-preview", "预览")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .disabled(self.rich_kind.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.rendered = true;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("previous-change", "↑ 变更")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .disabled(self.changes.is_empty())
                            .on_click(cx.listener(|this, _, _, cx| this.jump_change(-1, cx))),
                    )
                    .child(
                        Button::new("next-change", "↓ 变更")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .disabled(self.changes.is_empty())
                            .on_click(cx.listener(|this, _, _, cx| this.jump_change(1, cx))),
                    )
                    .child(
                        Button::new("copy-file", "复制全文")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .flex_none()
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(this.text.clone()))
                            })),
                    ),
            )
            .child(
                div()
                    .h(px((self.height - 33.).max(1.)))
                    .w_full()
                    .child(rows),
            )
            .into_any_element()
    }
}

fn kind_label(kind: RichKind) -> &'static str {
    match kind {
        RichKind::Markdown => "Markdown 预览",
        RichKind::Html => "HTML 预览",
    }
}
