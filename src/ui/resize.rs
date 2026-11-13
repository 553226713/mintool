use crate::app::MintoolApp;
use adabraka_ui::components::resizable::{ResizablePanelEvent, ResizableState};
use gpui::*;

impl MintoolApp {
    pub(crate) fn resize_state(
        &mut self,
        key: String,
        project: Option<(String, u64, u64)>,
        cx: &mut Context<Self>,
    ) -> Entity<ResizableState> {
        if let Some((state, _)) = self.panel_states.get(&key) {
            return state.clone();
        }
        let state = ResizableState::new(cx);
        let subscription = cx.subscribe(
            &state,
            move |this, state, event: &ResizablePanelEvent, cx| {
                let sizes = state.read(cx).sizes().clone();
                if let Some((project, a, b)) = &project {
                    if sizes.len() == 2 {
                        let sum = f32::from(sizes[0] + sizes[1]);
                        if let Some(w) = this.workspaces.get_mut(project) {
                            w.state
                                .layout
                                .set_ratio(*a, *b, f32::from(sizes[0]) / sum.max(1.));
                        }
                    }
                } else if sizes.len() == 3 {
                    match event {
                        ResizablePanelEvent::Resized { panel_index: 0, .. } => {
                            this.settings.left_width = sizes[0].into();
                        }
                        ResizablePanelEvent::Resized { panel_index: 1, .. } => {
                            this.settings.right_width = sizes[2].into();
                        }
                        _ => (),
                    }
                    this.panel_states.retain(|key, _| key == "app-panels");
                }
                this.changed(cx);
            },
        );
        self.panel_states.insert(key, (state.clone(), subscription));
        state
    }
}
