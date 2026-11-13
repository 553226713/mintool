use crate::ui::{terminal::TerminalView, theme};
use gpui::*;

pub struct TerminalSurface {
    pub view: Entity<TerminalView>,
}
impl IntoElement for TerminalSurface {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for TerminalSurface {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let font = font("JetBrains Mono");
        let font_id = window.text_system().resolve_font(&font);
        self.view.update(cx, |view, _| {
            let width = window
                .text_system()
                .em_advance(font_id, px(view.font_size))
                .unwrap_or(px(view.font_size * 0.6));
            let height = px(view.font_size * 1.55);
            view.bounds = bounds;
            view.cell_size = size(width, height);
            let rows = (bounds.size.height / height).floor().max(1.) as u16;
            let cols = (bounds.size.width / width).floor().max(2.) as u16;
            if let Err(error) = view.session.lock().unwrap().resize(rows, cols) {
                view.error = Some(error.to_string());
            }
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let view = self.view.read(cx);
        let focus = view.focus.clone();
        let session = view.session.lock().unwrap();
        let screen = session.parser.lock().unwrap().screen().clone();
        let (rows, cols) = session.size;
        let cell_size = view.cell_size;
        let font_size = px(view.font_size);
        let selection = view.selection;
        let composition = view.composition.clone();
        let error = view.error.clone();
        drop(session);
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );
        let font = font("JetBrains Mono");
        window.paint_layer(bounds, |window| {
            for row in 0..rows {
                for col in 0..cols {
                    let Some(cell) = screen.cell(row, col) else {
                        continue;
                    };
                    if cell.is_wide_continuation() {
                        continue;
                    }
                    let origin = bounds.origin
                        + point(cell_size.width * col as f32, cell_size.height * row as f32);
                    let mut fg = theme::terminal_color(cell.fgcolor(), theme::text());
                    let mut bg = theme::terminal_color(cell.bgcolor(), theme::background());
                    if cell.inverse() {
                        std::mem::swap(&mut fg, &mut bg);
                    }
                    if let Some((a, b)) = selection {
                        let (a, b) = if a <= b { (a, b) } else { (b, a) };
                        if (row, col) >= a && (row, col) <= b {
                            bg = theme::terminal_selection();
                        }
                    }
                    let width = cell_size.width * if cell.is_wide() { 2. } else { 1. };
                    if bg != theme::background() {
                        window.paint_quad(fill(
                            Bounds::new(origin, size(width, cell_size.height)),
                            bg,
                        ));
                    }
                    let text = cell.contents();
                    if text.is_empty() || text == " " {
                        continue;
                    }
                    let mut cell_font = font.clone();
                    if cell.bold() {
                        cell_font.weight = FontWeight::BOLD;
                    }
                    let run = TextRun {
                        len: text.len(),
                        font: cell_font,
                        color: fg,
                        background_color: None,
                        underline: cell.underline().then_some(UnderlineStyle {
                            color: Some(fg),
                            thickness: px(1.),
                            wavy: false,
                        }),
                        strikethrough: None,
                    };
                    let line =
                        window
                            .text_system()
                            .shape_line(text.into(), font_size, &[run], None);
                    let _ = line.paint(origin, cell_size.height, window, cx);
                }
            }
            let (row, col) = screen.cursor_position();
            let origin =
                bounds.origin + point(cell_size.width * col as f32, cell_size.height * row as f32);
            if focus.is_focused(window) && !screen.hide_cursor() && screen.scrollback() == 0 {
                window.paint_quad(fill(
                    Bounds::new(origin, size(px(2.), cell_size.height)),
                    theme::accent(),
                ));
                if !composition.is_empty() {
                    let run = TextRun {
                        len: composition.len(),
                        font: font.clone(),
                        color: theme::accent(),
                        background_color: Some(theme::elevated()),
                        underline: None,
                        strikethrough: None,
                    };
                    let line = window.text_system().shape_line(
                        composition.into(),
                        font_size,
                        &[run],
                        None,
                    );
                    let _ = line.paint(origin, cell_size.height, window, cx);
                }
            }
            if let Some(error) = error {
                let run = TextRun {
                    len: error.len(),
                    font,
                    color: theme::danger(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let line = window
                    .text_system()
                    .shape_line(error.into(), font_size, &[run], None);
                let _ = line.paint(bounds.origin, cell_size.height, window, cx);
            }
        });
        let (row, col) = screen.cursor_position();
        self.view.update(cx, |view, _| {
            view.cursor_bounds = Bounds::new(
                bounds.origin + point(cell_size.width * col as f32, cell_size.height * row as f32),
                cell_size,
            )
        });
    }
}
