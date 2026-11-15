//! Right-hand auxiliary panel for Docker logs/exec sessions.

use super::model::{parse_session_id, DockerView};
use crate::app::MintoolApp;
use crate::model::session::SessionView;
use crate::ui::theme;
use adabraka_ui::components::button::{Button, ButtonSize, ButtonVariant};
use adabraka_ui::components::icon::Icon;
use gpui::*;

impl MintoolApp {
    /// Render the container's logs/terminal in a side panel while the resource
    /// list stays visible in the main area.
    pub(crate) fn render_docker_session_panel(
        &mut self,
        session: &SessionView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(workspace) = self.workspaces.get(&session.key) else {
            return div().into_any_element();
        };
        let layout = workspace.state.layout.clone();
        let Some((container_id, view)) = parse_session_id(&session.key.id) else {
            return div().into_any_element();
        };
        let name = self
            .modules
            .docker
            .container(&container_id)
            .map(|container| container.name.clone())
            .unwrap_or_else(|| container_id.clone());
        let logs_id = container_id.clone();
        let exec_id = container_id.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::background())
            .child(
                div()
                    .h(px(36.))
                    .px_3()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .bg(theme::panel())
                    .border_b_1()
                    .border_color(theme::border())
                    .child(Icon::new("docker").size(px(14.)).color(theme::accent()))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{} · {}", name, view.label())),
                    )
                    .child(
                        Button::new("docker-panel-logs", "日志")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .selected(view == DockerView::Logs)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_docker_session(logs_id.clone(), DockerView::Logs, cx)
                            })),
                    )
                    .child(
                        Button::new("docker-panel-exec", "终端")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .selected(view == DockerView::Exec)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_docker_session(exec_id.clone(), DockerView::Exec, cx)
                            })),
                    )
                    .child(
                        Button::new("docker-panel-close", "")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .icon("x")
                            .tooltip("关闭面板")
                            .on_click(cx.listener(|this, _, _, cx| this.close_docker_session(cx))),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_layout(session, &layout, window, cx)),
            )
            .into_any_element()
    }
}
