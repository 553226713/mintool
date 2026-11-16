use mintool::services::terminal_text;
use std::num::NonZeroU16;
fn parser() -> vt100::Parser {
    let mut parser = vt100::Parser::new(
        NonZeroU16::new(3).unwrap(),
        NonZeroU16::new(20).unwrap(),
        32,
    );
    parser.process("first\r\nsecond\r\nthird 中文\r\nfourth\r\nfifth\r\nsixth".as_bytes());
    parser
}
#[test]
fn history_search_finds_scrollback_and_unicode_cell_columns_without_scrolling() {
    let mut parser = parser();
    parser.screen_mut().set_scrollback(1);
    let before = parser.screen().scrollback();
    let lines = terminal_text::rows(parser.screen());
    assert_eq!(
        lines,
        vec!["first", "second", "third 中文", "fourth", "fifth", "sixth"]
    );
    assert_eq!(
        terminal_text::find(parser.screen(), "first"),
        vec![(0, 0, 4)]
    );
    assert_eq!(
        terminal_text::find(parser.screen(), "中文"),
        vec![(2, 6, 9)]
    );
    assert_eq!(parser.screen().scrollback(), before);
    assert!(terminal_text::find(parser.screen(), "").is_empty());
}
#[test]
fn selections_cross_screen_boundaries_and_can_be_reversed() {
    let parser = parser();
    let expected = "cond\nthird 中文\nfourth\nfifth\nsix";
    assert_eq!(
        terminal_text::selected_text(parser.screen(), (1, 2), (5, 2)),
        expected
    );
    assert_eq!(
        terminal_text::selected_text(parser.screen(), (5, 2), (1, 2)),
        expected
    );
}
