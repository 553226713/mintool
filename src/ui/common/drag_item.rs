use crate::ui::theme;
use gpui::*;
use std::path::PathBuf;
#[derive(Clone)]
pub enum DragKind {
    Project(String),
    Group(String),
    SshConnection(String),
    SshGroup(String),
    DatabaseConnection(String),
    DatabaseGroup(String),
    Files(Vec<PathBuf>),
    SshRemoteFiles {
        owner: (crate::model::session::SessionKey, u64),
        paths: Vec<(String, bool)>,
    },
}
#[derive(Clone)]
pub struct DragItem {
    pub kind: DragKind,
    pub name: String,
}
impl Render for DragItem {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_4()
            .py_2()
            .rounded_lg()
            .shadow_lg()
            .bg(theme::elevated())
            .border_1()
            .border_color(theme::accent())
            .text_color(theme::text())
            .child(self.name.clone())
    }
}
