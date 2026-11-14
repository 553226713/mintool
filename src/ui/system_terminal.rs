use crate::{
    services::{embedded_terminal::EmbeddedTerminal, system_terminal::SystemTerminal},
    ui::theme,
};
use gpui::prelude::FluentBuilder;
use gpui::*;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::path::PathBuf;

const PANEL_DIVIDER_GUTTER: Pixels = px(4.);

impl crate::app::MintoolApp {
    pub(crate) fn system_terminal(
        &mut self,
        project: &crate::model::project::Project,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let key = (project.id.clone(), id);
        if let Some(view) = self.system_terminals.get(&key) {
            return Some(view.clone().into_any_element());
        }
        if self.terminals.contains_key(&key) || self.settings.terminal_backend == "builtin" {
            return None;
        }
        let x11 = HasWindowHandle::window_handle(window).is_ok_and(|handle| {
            matches!(
                handle.as_raw(),
                RawWindowHandle::Xcb(_) | RawWindowHandle::Xlib(_)
            )
        });
        let terminals = SystemTerminal::detect();
        let terminal = if self.settings.terminal_backend == "auto" {
            if !x11 {
                return None;
            }
            terminals.into_iter().next()?
        } else {
            match terminals
                .into_iter()
                .find(|terminal| terminal.name == self.settings.terminal_backend)
            {
                Some(terminal) => terminal,
                None => {
                    return Some(
                        div()
                            .p_4()
                            .child(format!(
                                "所选系统终端 {} 不可用，请在设置中重新选择",
                                self.settings.terminal_backend
                            ))
                            .into_any_element(),
                    )
                }
            }
        };
        let view = cx.new(|_| {
            SystemTerminalView::new(terminal, project.path.clone(), self.settings.shell.clone())
        });
        self.system_terminals.insert(key, view.clone());
        Some(view.into_any_element())
    }
}

pub struct SystemTerminalView {
    terminal: SystemTerminal,
    directory: PathBuf,
    shell: String,
    host: Option<EmbeddedTerminal>,
    pub allowed: bool,
    error: Option<String>,
    exited: bool,
}

impl SystemTerminalView {
    pub fn stop(&mut self) {
        self.host = None;
        self.exited = true;
    }
    pub fn new(terminal: SystemTerminal, directory: PathBuf, shell: String) -> Self {
        Self {
            terminal,
            directory,
            shell,
            host: None,
            allowed: true,
            error: None,
            exited: false,
        }
    }
    pub fn set_allowed(&mut self, allowed: bool) {
        self.allowed = allowed;
        if !allowed {
            if let Some(host) = &mut self.host {
                let _ = host.show(false);
            }
        }
    }
    pub fn poll(&mut self, cx: &mut Context<Self>) {
        if let Some(host) = &mut self.host {
            let _ = host.refresh_children();
            let _ = host.poll_focus();
        }
        if !self.exited && self.host.as_mut().is_some_and(|host| host.exited()) {
            self.exited = true;
            if let Some(host) = &mut self.host {
                let _ = host.show(false);
            }
            cx.notify();
        }
    }
    fn position(&mut self, bounds: Bounds<Pixels>, window: &mut Window) -> anyhow::Result<()> {
        if !self.allowed || self.exited || self.error.is_some() {
            return Ok(());
        }
        let mask = window.content_mask().bounds;
        if !bounds.intersects(&mask) {
            if let Some(host) = &mut self.host {
                host.show(false)?;
            }
            return Ok(());
        }
        let bounds = bounds.intersect(&mask);
        let content_bounds = Bounds::new(
            bounds.origin,
            size(
                (bounds.size.width - PANEL_DIVIDER_GUTTER).max(px(1.)),
                (bounds.size.height - PANEL_DIVIDER_GUTTER).max(px(1.)),
            ),
        );
        if self.host.is_none() {
            let handle = HasWindowHandle::window_handle(window)
                .map_err(|error| anyhow::anyhow!("无法取得主窗口句柄：{error}"))?;
            let parent = match handle.as_raw() {
                RawWindowHandle::Xcb(handle) => handle.window.get(),
                RawWindowHandle::Xlib(handle) => handle.window as u32,
                _ => anyhow::bail!("系统终端嵌入需要 X11 窗口；当前窗口使用 Wayland，请改用内置终端或在 X11 下启动 Mintool"),
            };
            self.host = Some(EmbeddedTerminal::spawn(
                parent,
                &self.terminal,
                &self.directory,
                &self.shell,
            )?);
        }
        let scale = window.scale_factor();
        self.host.as_mut().unwrap().position(
            (f32::from(content_bounds.origin.x) * scale).round() as i16,
            (f32::from(content_bounds.origin.y) * scale).round() as i16,
            (f32::from(content_bounds.size.width) * scale)
                .round()
                .max(1.) as u16,
            (f32::from(content_bounds.size.height) * scale)
                .round()
                .max(1.) as u16,
        )
    }
}
impl Render for SystemTerminalView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        div()
            .size_full()
            .relative()
            .bg(theme::background())
            .child(
                canvas(
                    move |bounds, window, cx| {
                        view.update(cx, |view, cx| {
                            if let Err(error) = view.position(bounds, window) {
                                view.error = Some(format!("{error:#}"));
                                cx.notify();
                            }
                        });
                    },
                    |_, _, _, _| {},
                )
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .h_full()
                    .w(px(1.))
                    .bg(theme::border()),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .h(px(1.))
                    .w_full()
                    .bg(theme::border()),
            )
            .when(self.error.is_some() || self.exited, |body| {
                body.child(
                    div()
                        .absolute()
                        .inset_0()
                        .p_4()
                        .text_color(theme::muted())
                        .child(
                            self.error
                                .clone()
                                .unwrap_or_else(|| "系统终端已退出".into()),
                        )
                        .child(
                            adabraka_ui::components::button::Button::new(
                                "restart-system-terminal",
                                "重新启动",
                            )
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.host = None;
                                view.error = None;
                                view.exited = false;
                                cx.notify();
                            })),
                        ),
                )
            })
    }
}
