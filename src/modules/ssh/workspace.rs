use crate::{app::MintoolApp, model::session::SessionView, ui::theme, workspace::layout::Pane};
use adabraka_ui::components::resizable::{h_resizable, resizable_panel, ResizableState};
use gpui::*;

impl MintoolApp {
    pub(crate) fn render_ssh_pane(
        &mut self,
        session: &SessionView,
        pane: &Pane,
        width: f32,
        terminal: AnyElement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !pane.is_terminal() || pane.terminal_closed {
            return terminal;
        }
        let previous = self.modules.ssh.active_panel.clone();
        self.activate_ssh_panel((session.key.clone(), pane.id), cx);
        self.sync_ssh_addresses(window, cx);
        let files = div()
            .size_full()
            .capture_key_down(
                cx.listener(|app, event: &KeyDownEvent, _, cx| app.ssh_navigation_key(event, cx)),
            )
            .child(self.render_ssh_explorer(cx));
        let key = (session.key.clone(), pane.id);
        if !self.modules.ssh.pane_splits.contains_key(&key) {
            let state = ResizableState::new(cx);
            let subscription = cx.subscribe(&state, |app, state, _, cx| {
                if let Some(width) = state.read(cx).sizes().last() {
                    app.settings.ssh_right_width = f32::from(*width);
                    app.changed(cx);
                }
            });
            self.modules
                .ssh
                .pane_splits
                .insert(key.clone(), (state, subscription));
        }
        let state = self.modules.ssh.pane_splits[&key].0.clone();
        let file_width = self
            .settings
            .ssh_right_width
            .min((width * 0.5).max(220.))
            .max(220.);
        let session_key = session.key.clone();
        let pane_id = pane.id;
        let split = h_resizable(
            SharedString::from(format!("ssh-pane-{}-{}", session.key.id, pane.id)),
            state,
        )
        .child(
            resizable_panel()
                .size_range(px(160.)..px(10000.))
                .flex_1()
                .overflow_hidden()
                .child(terminal),
        )
        .child(
            resizable_panel()
                .size(px(file_width))
                .size_range(px(220.)..px(10000.))
                .flex_none()
                .overflow_hidden()
                .bg(theme::panel())
                .child(files),
        );
        self.modules.ssh.active_panel = previous;
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .capture_any_mouse_down(cx.listener(move |app, _, _, cx| {
                app.activate_ssh_panel((session_key.clone(), pane_id), cx);
                if let Some(workspace) = app.workspaces.get_mut(&session_key) {
                    workspace.state.focused = pane_id;
                }
                cx.notify();
            }))
            .capture_key_down(cx.listener({
                let owner = (session.key.clone(), pane.id);
                move |app, event: &KeyDownEvent, _, cx| {
                    app.activate_ssh_panel(owner.clone(), cx);
                    let _ = event;
                }
            }))
            .child(split)
            .into_any_element()
    }
}
