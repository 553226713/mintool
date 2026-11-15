use crate::services::files;
use crate::ui::theme;
use adabraka_ui::components::button::{Button, ButtonSize, ButtonVariant};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    input::{Input, InputEvent, InputState, TabSize},
    text::TextView,
};
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

/// Editable text buffer for an open file, with an optional rendered preview.
pub struct FilePreview {
    path: PathBuf,
    text: String,
    height: f32,
    rich_kind: Option<RichKind>,
    rendered: bool,
    editor: Entity<InputState>,
    editor_dirty: bool,
    save_error: Option<String>,
    saved_text: Option<String>,
    /// Disk content waiting to be applied on the next paint (needs a `Window`).
    pending_external: Option<String>,
    /// External content the user chose to ignore for this dirty buffer.
    acknowledged_external: Option<String>,
    _editor_subscription: Subscription,
}
impl FilePreview {
    pub fn new(
        path: PathBuf,
        text: String,
        height: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let rich_kind = rich_kind(&path);
        let language = language_name(&path);
        let editor = cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .code_editor(language)
                .soft_wrap(false)
                .tab_size(TabSize {
                    tab_size: 4,
                    hard_tabs: false,
                });
            state.set_value(text.clone(), window, cx);
            state
        });
        let editor_subscription = cx.subscribe(
            &editor,
            |this, editor: Entity<InputState>, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.editor_dirty = editor.read(cx).value().as_str() != this.text.as_str();
                    cx.notify();
                }
            },
        );
        editor.update(cx, |state, cx| state.focus(window, cx));
        Self {
            path,
            text,
            height,
            rich_kind,
            rendered: false,
            editor,
            editor_dirty: false,
            save_error: None,
            saved_text: None,
            pending_external: None,
            acknowledged_external: None,
            _editor_subscription: editor_subscription,
        }
    }
    pub fn has_unsaved_changes(&self) -> bool {
        self.editor_dirty
    }
    /// True when the disk changed under a dirty buffer and the user has not
    /// already chosen to keep their version for this content.
    pub fn external_change_needs_prompt(&self, disk_text: &str) -> bool {
        self.editor_dirty
            && self.text != disk_text
            && self.acknowledged_external.as_deref() != Some(disk_text)
    }
    pub fn acknowledge_external(&mut self, disk_text: &str) {
        self.acknowledged_external = Some(disk_text.to_string());
    }
    /// Apply the disk version after the next paint, when a `Window` is available.
    pub fn request_external_reload(&mut self, disk_text: String, cx: &mut Context<Self>) {
        self.pending_external = Some(disk_text);
        cx.notify();
    }
    /// Returns the text written to disk by the last save and clears the pending notice.
    pub fn take_saved_text(&mut self) -> Option<String> {
        self.saved_text.take()
    }
    pub fn save(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        self.acknowledged_external = None;
        self.pending_external = None;
        let text = self.editor.read(cx).value().to_string();
        if text != self.text {
            if let Err(error) = files::write_text(&self.path, &text) {
                let message = format!("保存失败：{error:#}");
                self.save_error = Some(message.clone());
                cx.notify();
                return Err(message);
            }
            self.text = text.clone();
            self.saved_text = Some(text);
        }
        self.editor_dirty = false;
        self.save_error = None;
        cx.notify();
        Ok(())
    }
    pub fn resize(&mut self, height: f32, cx: &mut Context<Self>) {
        if (self.height - height).abs() > 0.5 {
            self.height = height;
            cx.notify();
        }
    }
    /// Reload disk content while the buffer has no unsaved changes.
    pub fn replace(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor_dirty || text == self.text {
            return;
        }
        self.acknowledged_external = None;
        self.pending_external = None;
        self.text = text.clone();
        self.editor
            .update(cx, |state, cx| state.set_value(text, window, cx));
        self.save_error = None;
        cx.notify();
    }
}
impl Render for FilePreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(text) = self.pending_external.take() {
            self.text = text.clone();
            self.editor
                .update(cx, |state, cx| state.set_value(text, window, cx));
            self.editor_dirty = false;
            self.acknowledged_external = None;
            self.save_error = None;
        }
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
        let dirty = self.editor_dirty;
        let status = self.save_error.clone().unwrap_or_else(|| {
            if dirty {
                "未保存 · Ctrl+S 保存".to_string()
            } else {
                String::new()
            }
        });
        let status_color = if self.save_error.is_some() {
            theme::danger()
        } else {
            theme::warning()
        };
        div()
            .id("file-editor")
            .w_full()
            .h(px(self.height))
            .flex_none()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.modifiers.control
                    && event.keystroke.key.eq_ignore_ascii_case("s")
                {
                    let _ = this.save(cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .h(px(33.))
                    .px_3()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(10.))
                    .text_color(theme::muted())
                    .border_b_1()
                    .border_color(theme::border())
                    .child(div().flex_1().min_w_0().truncate().child(format!(
                        "{}",
                        self.path.file_name().unwrap_or_default().to_string_lossy()
                    )))
                    .when(!status.is_empty(), |row| {
                        row.child(div().flex_none().text_color(status_color).child(status))
                    })
                    .when(self.rich_kind.is_some(), |row| {
                        row.child(
                            Button::new("rendered-preview", "预览")
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Sm)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.rendered = true;
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                div()
                    .h(px((self.height - 33.).max(1.)))
                    .w_full()
                    .min_h_0()
                    .p_2()
                    .child(
                        Input::new(&self.editor)
                            .appearance(false)
                            .bordered(false)
                            .font_family("JetBrains Mono")
                            .text_size(px(13.))
                            .h_full(),
                    ),
            )
            .into_any_element()
    }
}

pub(crate) fn language_name(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("rs") => "rust",
        Some("js" | "mjs" | "cjs" | "jsx") => "javascript",
        Some("ts" | "tsx" | "mts") => "typescript",
        Some("py" | "pyi") => "python",
        Some("go") => "go",
        Some("java") => "java",
        Some("c" | "h") => "c",
        Some("cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx") => "cpp",
        Some("cs") => "csharp",
        Some("rb") => "ruby",
        Some("php") => "php",
        Some("sh" | "bash" | "zsh") => "bash",
        Some("json" | "jsonc") => "json",
        Some("toml") => "toml",
        Some("yaml" | "yml") => "yaml",
        Some("md" | "markdown" | "mdown" | "mkd") => "markdown",
        Some("html" | "htm") => "html",
        Some("css") => "css",
        Some("scss") => "scss",
        Some("xml" | "svg") => "xml",
        Some("sql") => "sql",
        Some("lua") => "lua",
        Some("vim") => "vim",
        Some("diff" | "patch") => "diff",
        _ => "text",
    }
}

fn kind_label(kind: RichKind) -> &'static str {
    match kind {
        RichKind::Markdown => "Markdown 预览",
        RichKind::Html => "HTML 预览",
    }
}
