use crate::{
    services::{
        terminal::{lock_unpoisoned, TerminalSession},
        terminal_clipboard::TerminalClipboard,
        terminal_input::{self, KeyModifiers},
    },
    ui::{terminal_surface::TerminalSurface, theme},
};
use gpui::*;
use std::sync::{Arc, Mutex};

pub struct TerminalView {
    pub session: Arc<Mutex<TerminalSession>>,
    pub focus: FocusHandle,
    pub font_size: f32,
    pub bounds: Bounds<Pixels>,
    pub cursor_bounds: Bounds<Pixels>,
    pub composition: String,
    pub selection: Option<((u16, u16), (u16, u16))>,
    pub selecting: bool,
    pub cell_size: Size<Pixels>,
    pub error: Option<String>,
    clipboard: TerminalClipboard,
}
impl TerminalView {
    pub fn new(
        session: Arc<Mutex<TerminalSession>>,
        font_size: f32,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            session,
            focus: cx.focus_handle(),
            font_size,
            bounds: Bounds::default(),
            cursor_bounds: Bounds::default(),
            composition: String::new(),
            selection: None,
            selecting: false,
            cell_size: size(px(8.), px(20.)),
            error: None,
            clipboard: TerminalClipboard::default(),
        }
    }
    pub fn send(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        let result = lock_unpoisoned(&self.session).send(bytes);
        if let Err(error) = result {
            self.error = Some(error.to_string());
        }
        self.selection = None;
        cx.notify();
    }
    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        if modifiers.control && key == "v" {
            if let Some(item) = cx.read_from_clipboard() {
                let text = if let Some(image) = item.entries().iter().find_map(|entry| {
                    if let ClipboardEntry::Image(image) = entry {
                        Some(image)
                    } else {
                        None
                    }
                }) {
                    // Image-aware terminal programs handle Ctrl+V themselves.
                    // Ctrl+Shift+V explicitly inserts an image file path instead.
                    if !modifiers.shift {
                        self.send(&[0x16], cx);
                        cx.stop_propagation();
                        return;
                    }
                    let extension = match image.format {
                        ImageFormat::Png => "png",
                        ImageFormat::Jpeg => "jpg",
                        ImageFormat::Webp => "webp",
                        ImageFormat::Gif => "gif",
                        ImageFormat::Svg => "svg",
                        ImageFormat::Bmp => "bmp",
                        ImageFormat::Tiff => "tiff",
                    };
                    match self.clipboard.save_image(extension, &image.bytes) {
                        Ok(path) => Some(TerminalClipboard::quote_path(&path)),
                        Err(error) => {
                            self.error = Some(format!("{error:#}"));
                            cx.notify();
                            None
                        }
                    }
                } else {
                    item.text()
                };
                if let Some(text) = text {
                    let bracketed = lock_unpoisoned(&lock_unpoisoned(&self.session).parser)
                        .screen()
                        .bracketed_paste();
                    self.send(&terminal_input::paste(&text, bracketed), cx);
                }
            } else if !modifiers.shift {
                self.send(&[0x16], cx);
            }
            cx.stop_propagation();
            return;
        }
        if modifiers.control && key == "c" && (modifiers.shift || self.selection.is_some()) {
            cx.write_to_clipboard(ClipboardItem::new_string(self.selected_text()));
            cx.stop_propagation();
            return;
        }
        let application = lock_unpoisoned(&lock_unpoisoned(&self.session).parser)
            .screen()
            .application_cursor();
        let encoded = terminal_input::key(
            key,
            KeyModifiers {
                control: modifiers.control,
                alt: modifiers.alt,
                shift: modifiers.shift,
            },
            application,
        );
        if let Some(data) = encoded {
            self.send(&data, cx);
            cx.stop_propagation();
        } else if modifiers.alt && !modifiers.control {
            if let Some(text) = event.keystroke.key_char.as_ref() {
                let mut data = vec![27];
                data.extend(text.as_bytes());
                self.send(&data, cx);
                cx.stop_propagation();
            }
        }
        let _ = window;
    }
    fn cell_at(&self, point: Point<Pixels>) -> (u16, u16) {
        let size = lock_unpoisoned(&self.session).size;
        let col = ((point.x - self.bounds.origin.x) / self.cell_size.width).max(0.) as u16;
        let row = ((point.y - self.bounds.origin.y) / self.cell_size.height).max(0.) as u16;
        (
            row.min(size.0.saturating_sub(1)),
            col.min(size.1.saturating_sub(1)),
        )
    }
    fn finish_selection(&mut self, cx: &mut Context<Self>) {
        if !self.selecting {
            return;
        }
        self.selecting = false;
        if let Some((a, b)) = self.selection {
            if a == b {
                self.selection = None;
            } else {
                let text = self.selected_text();
                if !text.is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }
        }
        cx.notify();
    }
    pub fn selected_text(&self) -> String {
        let session = lock_unpoisoned(&self.session);
        let parser = lock_unpoisoned(&session.parser);
        let screen = parser.screen();
        let Some((start, end)) = self.selection else {
            return screen.contents();
        };
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let mut lines = Vec::new();
        for row in start.0..=end.0 {
            let left = if row == start.0 { start.1 } else { 0 };
            let right = if row == end.0 {
                end.1
            } else {
                session.size.1.saturating_sub(1)
            };
            let mut line = String::new();
            for col in left..=right {
                if let Some(cell) = screen.cell(row, col) {
                    if !cell.is_wide_continuation() {
                        let s = cell.contents();
                        line.push_str(if s.is_empty() { " " } else { &s });
                    }
                }
            }
            lines.push(line.trim_end().to_string());
        }
        lines.join("\n")
    }
}
impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for TerminalView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("terminal")
            .size_full()
            .overflow_hidden()
            .bg(theme::background())
            .key_context("MintoolTerminal")
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.focus.focus(window);
                    this.selecting = true;
                    let cell = this.cell_at(event.position);
                    this.selection = Some((cell, cell));
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.selecting {
                    let cell = this.cell_at(event.position);
                    if let Some((start, _)) = this.selection {
                        this.selection = Some((start, cell));
                    }
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_selection(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_selection(cx)),
            )
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                let y = event.delta.pixel_delta(px(this.font_size * 1.5)).y;
                lock_unpoisoned(&this.session)
                    .scroll((f32::from(y) / this.font_size / 1.5).round() as i32);
                cx.notify();
                cx.stop_propagation();
            }))
            .child(TerminalSurface { view: cx.entity() })
    }
}
