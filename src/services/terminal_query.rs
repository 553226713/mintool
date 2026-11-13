/// Answers terminal capability and cursor queries used by interactive TUI programs.
pub struct TerminalQueries {
    pub cursor: (u16, u16),
    pub replies: Vec<u8>,
}
impl vte::Perform for TerminalQueries {
    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        let first = params
            .iter()
            .next()
            .and_then(|p| p.first())
            .copied()
            .unwrap_or(0);
        match (action, intermediates, first) {
            ('n', [], 6) => self
                .replies
                .extend(format!("\x1b[{};{}R", self.cursor.0 + 1, self.cursor.1 + 1).bytes()),
            ('n', [], 5) => self.replies.extend(b"\x1b[0n"),
            ('c', [], 0) => self.replies.extend(b"\x1b[?1;2c"),
            ('c', [b'>'], 0) => self.replies.extend(b"\x1b[>0;1;0c"),
            _ => (),
        }
    }
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        match params {
            [b"10", b"?"] => self.replies.extend(b"\x1b]10;rgb:d2d2/d8d8/e2e2\x1b\\"),
            [b"11", b"?"] => self.replies.extend(b"\x1b]11;rgb:1e1e/2121/2828\x1b\\"),
            _ => (),
        }
    }
}
