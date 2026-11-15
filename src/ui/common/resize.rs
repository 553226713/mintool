use crate::app::MintoolApp;
use adabraka_ui::components::resizable::{ResizablePanelEvent, ResizableState};
use gpui::*;

impl MintoolApp {
    /// Tracks the shell split widths. `sidebar`/`auxiliary` describe which
    /// optional panels are present so the panel indices stay meaningful.
    pub(crate) fn resize_state(
        &mut self,
        key: String,
        sidebar: bool,
        auxiliary: bool,
        ssh_auxiliary: bool,
        docker_auxiliary: bool,
        cx: &mut Context<Self>,
    ) -> Entity<ResizableState> {
        if let Some((state, _)) = self.panel_states.get(&key) {
            return state.clone();
        }
        let state = ResizableState::new(cx);
        let retain_key = key.clone();
        let subscription = cx.subscribe(
            &state,
            move |this, state, event: &ResizablePanelEvent, cx| {
                let sizes = state.read(cx).sizes().clone();
                let ResizablePanelEvent::Resized { panel_index, .. } = event;
                {
                    if sidebar && *panel_index == 0 {
                        if let Some(size) = sizes.first() {
                            this.settings.left_width = f32::from(*size);
                        }
                    } else if auxiliary && *panel_index + 1 == sizes.len() {
                        if let Some(size) = sizes.last() {
                            if ssh_auxiliary {
                                this.settings.ssh_right_width = f32::from(*size);
                            } else if docker_auxiliary {
                                this.settings.docker_right_width = f32::from(*size);
                            } else {
                                this.settings.right_width = f32::from(*size);
                            }
                        }
                    }
                }
                this.panel_states.retain(|key, _| key == &retain_key);
                this.changed(cx);
            },
        );
        self.panel_states.insert(key, (state.clone(), subscription));
        state
    }
}
