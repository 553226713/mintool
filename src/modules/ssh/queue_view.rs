use super::transfer_task::{ConflictPolicy, TransferState};
use crate::{
    app::MintoolApp,
    ui::{format::format_size, theme},
};
use adabraka_ui::components::button::{Button, ButtonSize, ButtonVariant};
use gpui::*;

impl MintoolApp {
    pub(crate) fn render_ssh_queue(&self, cx: &mut Context<Self>) -> AnyElement {
        let owner = self.modules.ssh.active_panel.clone();
        let tasks = self
            .modules
            .ssh
            .transfers
            .iter()
            .filter(|task| Some(&task.owner) == owner.as_ref());
        let mut rows = Vec::new();
        for task in tasks {
            let id = task.id;
            let status = match task.state {
                TransferState::Ready => "等待",
                TransferState::Running => "传输中",
                TransferState::Conflict => "目标已存在",
                TransferState::Finished => "完成",
                TransferState::Failed => "失败",
                TransferState::Cancelled => "已取消",
                TransferState::Skipped => "已跳过",
            };
            let (speed, eta) = task.metrics();
            let progress = task
                .progress
                .percent()
                .map(|p| format!(" {p}%"))
                .unwrap_or_default();
            let details = if task.state == TransferState::Running {
                format!(
                    " · {}/s{}",
                    format_size(speed),
                    eta.map(|seconds| format!(" · 剩余 {seconds}s"))
                        .unwrap_or_default()
                )
            } else {
                String::new()
            };
            let mut row = div()
                .flex()
                .flex_col()
                .gap_1()
                .px_2()
                .py_1()
                .border_b_1()
                .border_color(theme::border())
                .child(
                    div()
                        .text_size(px(10.))
                        .child(format!("{} · {status}{progress}{details}", task.name())),
                );
            row = row.child(
                div()
                    .text_size(px(9.))
                    .text_color(theme::muted())
                    .truncate()
                    .child(task.destination()),
            );
            if let Some(error) = &task.error {
                row = row.child(
                    div()
                        .text_size(px(10.))
                        .text_color(theme::danger())
                        .child(error.clone()),
                );
            }
            let mut actions = div().flex().flex_wrap().gap_1();
            if task.state == TransferState::Conflict {
                for (policy, label) in [
                    (
                        ConflictPolicy::Overwrite,
                        if task.directory {
                            "合并覆盖"
                        } else {
                            "覆盖"
                        },
                    ),
                    (ConflictPolicy::Skip, "跳过"),
                    (ConflictPolicy::Rename, "重命名"),
                    (ConflictPolicy::Resume, "续传"),
                ] {
                    if task.directory && policy == ConflictPolicy::Resume {
                        continue;
                    }
                    actions = actions.child(
                        Button::new(SharedString::from(format!("conflict-{id}-{label}")), label)
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.resolve_transfer(id, policy, cx)
                            })),
                    );
                }
            }
            if matches!(
                task.state,
                TransferState::Ready | TransferState::Running | TransferState::Conflict
            ) {
                actions = actions.child(
                    Button::new(SharedString::from(format!("cancel-{id}")), "取消")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(cx.listener(move |app, _, _, cx| {
                            if let Some(task) = app
                                .modules
                                .ssh
                                .transfers
                                .iter_mut()
                                .find(|task| task.id == id)
                            {
                                task.request_cancel();
                                if task.state != TransferState::Running {
                                    task.state = TransferState::Cancelled;
                                }
                            }
                            cx.notify();
                        })),
                );
            }
            if matches!(task.state, TransferState::Failed | TransferState::Cancelled) {
                actions = actions.child(
                    Button::new(SharedString::from(format!("retry-{id}")), "重试")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(cx.listener(move |app, _, _, cx| {
                            if let Some(task) = app
                                .modules
                                .ssh
                                .transfers
                                .iter_mut()
                                .find(|task| task.id == id)
                            {
                                task.retry();
                            }
                            app.pump_ssh_transfers(cx);
                            cx.notify();
                        })),
                );
            }
            rows.push(row.child(actions).into_any_element());
        }
        div()
            .id("ssh-transfer-queue")
            .flex_none()
            .max_h(px(220.))
            .overflow_y_scroll()
            .border_t_1()
            .border_color(theme::border())
            .child(
                div()
                    .px_2()
                    .py_1()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child("传输队列")
                    .child(
                        Button::new("apply-conflicts", "应用到全部同名文件")
                            .selected(self.modules.ssh.panel().apply_conflicts)
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|app, _, _, cx| {
                                let panel = app.modules.ssh.panel_mut();
                                panel.apply_conflicts = !panel.apply_conflicts;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("clear-finished", "清理完成")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|app, _, _, cx| {
                                let owner = app.modules.ssh.active_panel.clone();
                                app.modules.ssh.transfers.retain(|task| {
                                    Some(&task.owner) != owner.as_ref()
                                        || !matches!(
                                            task.state,
                                            TransferState::Finished
                                                | TransferState::Skipped
                                                | TransferState::Cancelled
                                        )
                                });
                                cx.notify();
                            })),
                    ),
            )
            .children(rows)
            .into_any_element()
    }
}
