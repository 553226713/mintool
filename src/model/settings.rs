use super::appearance::Appearance;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub font_size: f32,
    pub appearance: Appearance,
    pub shell: String,
    pub terminal_backend: String,
    pub left_width: f32,
    pub right_width: f32,
}
impl Settings {
    /// Keep saved sidebar widths when space permits, while reserving room for
    /// the workspace on smaller windows. Resizing does not overwrite preferences.
    pub fn panel_widths(&self, window_width: f32) -> (f32, f32) {
        (
            self.left_width
                .clamp(180., (window_width * 0.25).clamp(180., 400.)),
            self.right_width
                .clamp(200., (window_width * 0.30).clamp(200., 560.)),
        )
    }
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            appearance: Appearance::default(),
            shell: std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into()),
            terminal_backend: "auto".into(),
            left_width: 210.0,
            right_width: 270.0,
        }
    }
}
