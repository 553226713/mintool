//! Read-only notification history.

use super::{Dialog, DialogContent};
use crate::app::MintoolApp;
use crate::ui::theme;
use gpui::*;
use std::time::Duration;

impl MintoolApp {
    pub(crate) fn open_notice_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let lines = self
            .notice_history
            .iter()
            .rev()
            .map(|(message, at)| format!("{}  ·  {}", relative_time(at.elapsed()), message))
            .collect();
        self.show_dialog(Dialog::Notices(NoticeHistoryDialog::new(lines)), window, cx);
    }
}

fn relative_time(elapsed: Duration) -> String {
    if elapsed.as_secs() < 5 {
        "刚刚".into()
    } else if elapsed.as_secs() < 60 {
        format!("{} 秒前", elapsed.as_secs())
    } else if elapsed.as_secs() < 3600 {
        format!("{} 分钟前", elapsed.as_secs() / 60)
    } else {
        format!("{} 小时前", elapsed.as_secs() / 3600)
    }
}

#[derive(Clone)]
pub struct NoticeHistoryDialog {
    lines: Vec<String>,
}

impl NoticeHistoryDialog {
    pub fn new(lines: Vec<String>) -> Self {
        Self { lines }
    }
}

impl DialogContent for NoticeHistoryDialog {
    fn title(&self) -> String {
        "通知历史".into()
    }
    fn accepts_empty_input(&self) -> bool {
        true
    }
    fn has_actions(&self) -> bool {
        false
    }
    fn note(&self, _app: &MintoolApp) -> Option<String> {
        Some("最近 50 条提示，关闭后仍会保留到本次退出。".into())
    }
    fn render_extra(&self, _app: &MintoolApp, _cx: &mut Context<MintoolApp>) -> Option<AnyElement> {
        let lines = if self.lines.is_empty() {
            vec![div()
                .text_color(theme::muted())
                .child("还没有提示")
                .into_any_element()]
        } else {
            self.lines
                .iter()
                .map(|line| {
                    div()
                        .py(px(2.))
                        .text_size(px(11.))
                        .text_color(theme::text())
                        .child(line.clone())
                        .into_any_element()
                })
                .collect()
        };
        Some(
            div()
                .id("notice-history")
                .w_full()
                .max_h(px(440.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .children(lines)
                .into_any_element(),
        )
    }
    fn submit(
        &self,
        _app: &mut MintoolApp,
        _value: String,
        _cx: &mut Context<MintoolApp>,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
