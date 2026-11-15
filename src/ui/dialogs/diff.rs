//! Read-only diff viewer used by the project Git panel.

use super::DialogContent;
use crate::app::MintoolApp;
use crate::ui::theme;
use gpui::*;
use std::path::PathBuf;

#[derive(Clone)]
pub struct DiffDialog {
    path: PathBuf,
    text: String,
}

impl DiffDialog {
    pub fn new(path: PathBuf, text: String) -> Self {
        Self { path, text }
    }
    fn name(&self) -> String {
        self.path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.display().to_string())
    }
}

impl DialogContent for DiffDialog {
    fn title(&self) -> String {
        format!("差异 · {}", self.name())
    }
    fn accepts_empty_input(&self) -> bool {
        true
    }
    fn has_actions(&self) -> bool {
        false
    }
    fn note(&self, _app: &MintoolApp) -> Option<String> {
        let added = self
            .text
            .lines()
            .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
            .count();
        let removed = self
            .text
            .lines()
            .filter(|line| line.starts_with('-') && !line.starts_with("---"))
            .count();
        Some(format!("+{added}  −{removed}"))
    }
    fn render_extra(&self, _app: &MintoolApp, _cx: &mut Context<MintoolApp>) -> Option<AnyElement> {
        let lines = if self.text.trim().is_empty() {
            vec![div()
                .text_color(theme::muted())
                .child("没有可显示的差异")
                .into_any_element()]
        } else {
            self.text
                .lines()
                .map(|line| {
                    let color = if line.starts_with('+') && !line.starts_with("+++") {
                        theme::success()
                    } else if line.starts_with('-') && !line.starts_with("---") {
                        theme::danger()
                    } else if line.starts_with("@@") {
                        theme::warning()
                    } else {
                        theme::text()
                    };
                    div()
                        .font_family("DejaVu Sans Mono")
                        .text_size(px(11.))
                        .text_color(color)
                        .whitespace_nowrap()
                        .child(line.to_string())
                        .into_any_element()
                })
                .collect()
        };
        Some(
            div()
                .id("git-diff-body")
                .w_full()
                .max_h(px(480.))
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
