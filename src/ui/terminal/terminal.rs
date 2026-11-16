use crate::{
    services::{
        mutex::lock_unpoisoned,
        terminal::TerminalSession,
        terminal_clipboard::TerminalClipboard,
        terminal_input::{self, KeyModifiers},
    },
    ui::{terminal_surface::TerminalSurface, theme},
};
use gpui::*;
use gpui_component::scroll::{Scrollbar, ScrollbarHandle, ScrollbarShow};
use std::sync::{Arc, Mutex};

pub(crate) const SCROLLBAR_WIDTH: Pixels = px(16.);
actions!(
    mintool_terminal,
    [Complete, BackTab, FindOutput, FindNext, FindPrevious]
);

fn terminal_keybindings() -> [KeyBinding; 5] {
    [
        KeyBinding::new("tab", Complete, Some("MintoolTerminal")),
        KeyBinding::new("shift-tab", BackTab, Some("MintoolTerminal")),
        KeyBinding::new("ctrl-shift-f", FindOutput, Some("MintoolTerminal")),
        KeyBinding::new("f3", FindNext, Some("MintoolTerminal")),
        KeyBinding::new("shift-f3", FindPrevious, Some("MintoolTerminal")),
    ]
}

#[derive(Clone)]
struct TerminalScrollbarHandle {
    session: Arc<Mutex<TerminalSession>>,
    viewport_height: Pixels,
    row_height: Pixels,
}

fn scrollbar_offset(position: usize, scrollback_len: usize, row_height: Pixels) -> Pixels {
    let rows_from_bottom = scrollback_len.saturating_sub(position.min(scrollback_len));
    -row_height * rows_from_bottom as f32
}

fn scrollback_position(offset: Pixels, scrollback_len: usize, row_height: Pixels) -> usize {
    if row_height <= px(0.) {
        return 0;
    }
    let rows_from_bottom = (f32::from(-offset) / f32::from(row_height))
        .round()
        .clamp(0., scrollback_len as f32) as usize;
    scrollback_len - rows_from_bottom
}

impl ScrollbarHandle for TerminalScrollbarHandle {
    fn offset(&self) -> Point<Pixels> {
        let (position, scrollback_len) = lock_unpoisoned(&self.session).scrollback_position();
        point(
            px(0.),
            scrollbar_offset(position, scrollback_len, self.row_height),
        )
    }

    fn set_offset(&self, offset: Point<Pixels>) {
        let mut session = lock_unpoisoned(&self.session);
        let (_, scrollback_len) = session.scrollback_position();
        session.set_scrollback_position(scrollback_position(
            offset.y,
            scrollback_len,
            self.row_height,
        ));
    }

    fn content_size(&self) -> Size<Pixels> {
        let session = lock_unpoisoned(&self.session);
        let (_, scrollback_len) = session.scrollback_position();
        size(
            px(1.),
            self.viewport_height + self.row_height * scrollback_len as f32,
        )
    }
}

pub struct TerminalView {
    pub session: Arc<Mutex<TerminalSession>>,
    pub focus: FocusHandle,
    pub font_size: f32,
    pub bounds: Bounds<Pixels>,
    pub cursor_bounds: Bounds<Pixels>,
    pub composition: String,
    pub selection: Option<((usize, u16), (usize, u16))>,
    pub selecting: bool,
    pub cell_size: Size<Pixels>,
    pub error: Option<String>,
    clipboard: TerminalClipboard,
    pub(crate) search_input: Option<Entity<gpui_component::input::InputState>>,
    pub(crate) search_matches: Vec<(usize, u16, u16)>,
    pub(crate) search_index: usize,
    pub(crate) search_subscription: Option<Subscription>,
}
impl TerminalView {
    pub fn init(cx: &mut App) {
        cx.bind_keys(terminal_keybindings());
    }
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
            search_input: None,
            search_matches: Vec::new(),
            search_index: 0,
            search_subscription: None,
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
    fn paste_clipboard(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        if text.is_empty() {
            return;
        }
        let bracketed = lock_unpoisoned(&self.session).bracketed_paste();
        self.send(&terminal_input::paste(&text, bracketed), cx);
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
                    let bracketed = lock_unpoisoned(&self.session).bracketed_paste();
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
        let application = lock_unpoisoned(&self.session).application_cursor();
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
    fn cell_at(&self, point: Point<Pixels>) -> (usize, u16) {
        let size = lock_unpoisoned(&self.session).size;
        let col = ((point.x - self.bounds.origin.x) / self.cell_size.width).max(0.) as u16;
        let row = ((point.y - self.bounds.origin.y) / self.cell_size.height).max(0.) as u16;
        let session = lock_unpoisoned(&self.session);
        let parser = lock_unpoisoned(&session.parser);
        let screen = parser.screen();
        (
            screen.scrollback_len() - screen.scrollback()
                + row.min(size.0.saturating_sub(1)) as usize,
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
        crate::services::terminal_text::selected_text(screen, start, end)
    }
}
impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for TerminalView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scrollbar = TerminalScrollbarHandle {
            session: self.session.clone(),
            viewport_height: self.bounds.size.height,
            row_height: self.cell_size.height,
        };
        let entity_id = cx.entity_id();
        div()
            .id("terminal")
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(theme::background())
            .key_context("MintoolTerminal")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Complete, _, cx| {
                this.send(b"\t", cx);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &BackTab, _, cx| {
                this.send(b"\x1b[Z", cx);
                cx.stop_propagation();
            }))
            .on_action(
                cx.listener(|this, _: &FindOutput, window, cx| {
                    this.open_terminal_search(window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &FindNext, _, cx| this.next_terminal_match(1, cx)))
            .on_action(
                cx.listener(|this, _: &FindPrevious, _, cx| this.next_terminal_match(-1, cx)),
            )
            .cursor(CursorStyle::IBeam)
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.focus.focus(window);
                    this.selecting = true;
                    let cell = this.cell_at(event.position);
                    this.selection = Some((cell, cell));
                    if event.click_count >= 2 {
                        this.select_terminal_word(cell, event.click_count >= 3, cx);
                        this.selecting = false;
                        cx.write_to_clipboard(ClipboardItem::new_string(this.selected_text()));
                    }
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if !this.bounds.contains(&event.position) {
                        return;
                    }
                    this.focus.focus(window);
                    this.paste_clipboard(cx);
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.selecting {
                    if event.position.y < this.bounds.origin.y {
                        lock_unpoisoned(&this.session).scroll(1);
                    } else if event.position.y > this.bounds.bottom() {
                        lock_unpoisoned(&this.session).scroll(-1);
                    }
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
            .children(self.render_terminal_search(cx))
            .child(
                Scrollbar::vertical(&scrollbar)
                    .id(("terminal-scrollbar", entity_id))
                    .scrollbar_show(ScrollbarShow::Always),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{scrollback_position, scrollbar_offset, terminal_keybindings, BackTab, Complete};
    use gpui::{px, KeyBinding, KeyContext, Keymap, Keystroke};

    #[test]
    fn terminal_tab_takes_precedence_over_root_focus_navigation() {
        let mut keymap = Keymap::default();
        keymap.add_bindings(terminal_keybindings());
        // Even later bindings at the Root must lose to the terminal context.
        keymap.add_bindings([KeyBinding::new("tab", BackTab, Some("Root"))]);
        let root = KeyContext::parse("Root").unwrap();
        let terminal = KeyContext::parse("MintoolTerminal").unwrap();
        let (bindings, pending) = keymap.bindings_for_input(
            &[Keystroke::parse("tab").unwrap()],
            &[root.clone(), terminal.clone()],
        );
        assert!(!pending);
        assert!(bindings[0].action().partial_eq(&Complete));
        let (bindings, _) = keymap.bindings_for_input(
            &[Keystroke::parse("shift-tab").unwrap()],
            &[root.clone(), terminal],
        );
        assert!(bindings[0].action().partial_eq(&BackTab));
        let (bindings, _) = keymap.bindings_for_input(&[Keystroke::parse("tab").unwrap()], &[root]);
        assert!(bindings[0].action().partial_eq(&BackTab));
    }

    #[test]
    fn scrollbar_maps_live_view_to_bottom_and_history_to_top() {
        let row_height = px(20.);
        assert_eq!(scrollbar_offset(0, 100, row_height), px(-2000.));
        assert_eq!(scrollbar_offset(100, 100, row_height), px(0.));
        assert_eq!(scrollback_position(px(-2000.), 100, row_height), 0);
        assert_eq!(scrollback_position(px(0.), 100, row_height), 100);
    }
}
