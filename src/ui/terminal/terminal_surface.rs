use crate::{
    services::mutex::lock_unpoisoned,
    ui::{
        terminal::{TerminalView, SCROLLBAR_WIDTH},
        theme,
    },
};
use gpui::*;

struct RenderedBackground {
    col: u16,
    width: u16,
    color: Hsla,
}

struct RenderedRow {
    text: SharedString,
    runs: Vec<TextRun>,
    backgrounds: Vec<RenderedBackground>,
}

struct TerminalFrame {
    rows: Vec<RenderedRow>,
    cursor: (u16, u16),
    hide_cursor: bool,
    scrollback: usize,
}

fn terminal_content_bounds(bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        bounds.origin,
        size(
            (bounds.size.width - SCROLLBAR_WIDTH).max(px(1.)),
            bounds.size.height,
        ),
    )
}

fn push_background(backgrounds: &mut Vec<RenderedBackground>, col: u16, width: u16, color: Hsla) {
    if let Some(last) = backgrounds.last_mut() {
        if last.col + last.width == col && last.color == color {
            last.width += width;
            return;
        }
    }
    backgrounds.push(RenderedBackground { col, width, color });
}

fn render_row(
    screen: &vt100::Screen,
    row: u16,
    cols: u16,
    selection: Option<((usize, u16), (usize, u16))>,
    search: &[(usize, u16, u16)],
    font: &Font,
) -> RenderedRow {
    let mut text = String::with_capacity(cols as usize);
    let mut runs: Vec<TextRun> = Vec::new();
    let mut backgrounds = Vec::new();
    let default_background = theme::background();
    let selection = selection.map(|(a, b)| if a <= b { (a, b) } else { (b, a) });

    for col in 0..cols {
        let Some(cell) = screen.cell(row, col) else {
            text.push(' ');
            continue;
        };
        if cell.is_wide_continuation() {
            continue;
        }

        let mut foreground = theme::terminal_color(cell.fgcolor(), theme::text());
        let mut background = theme::terminal_color(cell.bgcolor(), default_background);
        if cell.inverse() {
            std::mem::swap(&mut foreground, &mut background);
        }
        let absolute = screen.scrollback_len() - screen.scrollback() + row as usize;
        if search
            .iter()
            .any(|&(line, left, right)| line == absolute && col >= left && col <= right)
        {
            background = theme::warning().opacity(0.3);
        }
        if selection.is_some_and(|(start, end)| (absolute, col) >= start && (absolute, col) <= end)
        {
            background = theme::terminal_selection();
        }

        let width = if cell.is_wide() { 2 } else { 1 };
        if background != default_background {
            push_background(&mut backgrounds, col, width, background);
        }

        let contents = match cell.contents() {
            "" => " ",
            contents => contents,
        };
        text.push_str(contents);
        let weight = if cell.bold() {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        };
        let underline = cell.underline();
        let same_style = runs.last().is_some_and(|run| {
            run.font.weight == weight
                && run.color == foreground
                && run.underline.is_some() == underline
        });
        if same_style {
            runs.last_mut().unwrap().len += contents.len();
        } else {
            let mut run_font = font.clone();
            run_font.weight = weight;
            runs.push(TextRun {
                len: contents.len(),
                font: run_font,
                color: foreground,
                background_color: None,
                underline: underline.then_some(UnderlineStyle {
                    color: Some(foreground),
                    thickness: px(1.),
                    wavy: false,
                }),
                strikethrough: None,
            });
        }
    }

    RenderedRow {
        text: text.into(),
        runs,
        backgrounds,
    }
}

fn render_terminal(view: &TerminalView, font: &Font) -> TerminalFrame {
    let session = lock_unpoisoned(&view.session);
    let parser = lock_unpoisoned(&session.parser);
    let screen = parser.screen();
    let (rows, cols) = session.size;
    TerminalFrame {
        rows: (0..rows)
            .map(|row| {
                render_row(
                    screen,
                    row,
                    cols,
                    view.selection,
                    &view.search_matches,
                    font,
                )
            })
            .collect(),
        cursor: screen.cursor_position(),
        hide_cursor: screen.hide_cursor(),
        scrollback: screen.scrollback(),
    }
}

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
        let bounds = terminal_content_bounds(bounds);
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
            if let Err(error) = lock_unpoisoned(&view.session).resize(rows, cols) {
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
        let bounds = terminal_content_bounds(bounds);
        let focus = view.focus.clone();
        let cell_size = view.cell_size;
        let font_size = px(view.font_size);
        let composition = view.composition.clone();
        let error = view.error.clone();
        let font = font("JetBrains Mono");
        let frame = render_terminal(view, &font);
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );
        window.paint_layer(bounds, |window| {
            for (row_index, row) in frame.rows.iter().enumerate() {
                let row_origin = bounds.origin + point(px(0.), cell_size.height * row_index as f32);
                for background in &row.backgrounds {
                    window.paint_quad(fill(
                        Bounds::new(
                            row_origin + point(cell_size.width * background.col as f32, px(0.)),
                            size(cell_size.width * background.width as f32, cell_size.height),
                        ),
                        background.color,
                    ));
                }
                if row.text.chars().all(|character| character == ' ') {
                    continue;
                }
                let line =
                    window
                        .text_system()
                        .shape_line(row.text.clone(), font_size, &row.runs, None);
                let _ = line.paint(row_origin, cell_size.height, window, cx);
            }
            let (row, col) = frame.cursor;
            let origin =
                bounds.origin + point(cell_size.width * col as f32, cell_size.height * row as f32);
            if focus.is_focused(window) && !frame.hide_cursor && frame.scrollback == 0 {
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
        let (row, col) = frame.cursor;
        self.view.update(cx, |view, _| {
            view.cursor_bounds = Bounds::new(
                bounds.origin + point(cell_size.width * col as f32, cell_size.height * row as f32),
                cell_size,
            )
        });
    }
}

#[cfg(test)]
mod tests {
    use super::render_row;
    use gpui::font;
    use std::num::NonZeroU16;

    #[test]
    fn terminal_rows_batch_plain_text_and_skip_wide_continuations() {
        let mut parser =
            vt100::Parser::new(NonZeroU16::new(2).unwrap(), NonZeroU16::new(12).unwrap(), 0);
        parser.process("abc中文".as_bytes());

        let row = render_row(parser.screen(), 0, 12, None, &[], &font("JetBrains Mono"));

        assert_eq!(row.text.trim_end(), "abc中文");
        assert_eq!(row.runs.len(), 1);
    }

    #[test]
    fn terminal_rows_merge_adjacent_backgrounds() {
        let mut parser =
            vt100::Parser::new(NonZeroU16::new(2).unwrap(), NonZeroU16::new(12).unwrap(), 0);
        parser.process(b"\x1b[41mred\x1b[0m");

        let row = render_row(parser.screen(), 0, 12, None, &[], &font("JetBrains Mono"));

        assert_eq!(row.backgrounds.len(), 1);
        assert_eq!(row.backgrounds[0].col, 0);
        assert_eq!(row.backgrounds[0].width, 3);
    }
}
