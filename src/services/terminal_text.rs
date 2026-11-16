//! Text access using absolute physical rows so selections survive scrolling.

pub fn rows(screen: &vt100::Screen) -> Vec<String> {
    let mut snapshot = screen.clone();
    let history = snapshot.scrollback_len();
    let height = snapshot.size().0.get() as usize;
    let columns = snapshot.size().1.get();
    let total = history + height;
    let mut result = Vec::with_capacity(total);
    let mut start = 0;
    while start < total {
        snapshot.set_scrollback(history.saturating_sub(start));
        let visible_start = history - snapshot.scrollback();
        let visible = snapshot.rows(0, columns).collect::<Vec<_>>();
        let skip = start - visible_start;
        for row in visible.into_iter().skip(skip).take(total - start) {
            result.push(row);
            start += 1;
        }
    }
    result
}

pub fn selected_text(screen: &vt100::Screen, start: (usize, u16), end: (usize, u16)) -> String {
    let (start, end) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    let mut snapshot = screen.clone();
    let history = snapshot.scrollback_len();
    let height = snapshot.size().0.get() as usize;
    let columns = snapshot.size().1.get();
    let mut result = Vec::new();
    for absolute in start.0..=end.0.min(history + height - 1) {
        snapshot.set_scrollback(history.saturating_sub(absolute));
        let row = (absolute - (history - snapshot.scrollback())) as u16;
        let left = if absolute == start.0 { start.1 } else { 0 };
        let right = if absolute == end.0 {
            end.1
        } else {
            columns - 1
        };
        let mut line = String::new();
        for column in left..=right.min(columns - 1) {
            if let Some(cell) = snapshot.cell(row, column) {
                if !cell.is_wide_continuation() {
                    let contents = cell.contents();
                    line.push_str(if contents.is_empty() { " " } else { contents });
                }
            }
        }
        result.push(line.trim_end().to_string());
    }
    result.join("\n")
}

pub fn find(screen: &vt100::Screen, query: &str) -> Vec<(usize, u16, u16)> {
    if query.is_empty() {
        return Vec::new();
    }
    rows(screen)
        .iter()
        .enumerate()
        .flat_map(|(row, line)| {
            line.match_indices(query)
                .map(move |(byte, text)| {
                    // vt100 writes cells rather than UTF-8 byte offsets; wide characters occupy two columns.
                    let width = |text: &str| {
                        unicode_width::UnicodeWidthStr::width(text).min(u16::MAX as usize) as u16
                    };
                    let start = width(&line[..byte]);
                    let count = width(text).max(1);
                    (row, start, start.saturating_add(count - 1))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}
