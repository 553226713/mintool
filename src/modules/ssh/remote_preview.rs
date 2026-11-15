use super::model::SshConnection;
use super::sftp;
use crate::app::MintoolApp;
use crate::model::session::SessionView;
use crate::modules::Modules;
use crate::ui::{image_preview::ImagePreview, theme};
use adabraka_ui::components::button::{Button, ButtonSize, ButtonVariant};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::scroll::{Scrollbar, ScrollbarShow};
use std::path::{Path, PathBuf};

impl Modules {
    /// Render (or reuse) the SSH remote-file preview for a pane. Keeping this
    /// here lets the module-neutral workspace layer stay unaware of SSH types.
    pub fn render_remote_preview(
        &mut self,
        session: &SessionView,
        pane_id: u64,
        remote: &str,
        height: f32,
        cx: &mut Context<MintoolApp>,
    ) -> AnyElement {
        let connection = self
            .ssh
            .registry
            .connections
            .iter()
            .find(|connection| connection.id == session.key.id)
            .cloned();
        let Some(connection) = connection else {
            return div()
                .p_4()
                .text_color(theme::danger())
                .child("SSH 连接已不存在")
                .into_any_element();
        };
        let key = (session.key.clone(), pane_id);
        if let Some(preview) = self.ssh.remote_previews.get(&key) {
            let preview = preview.clone();
            preview.update(cx, |preview, cx| preview.resize(height, cx));
            return preview.into_any_element();
        }
        let preview = cx.new(|cx| RemotePreview::new(connection, remote.to_string(), height, cx));
        self.ssh.remote_previews.insert(key, preview.clone());
        preview.into_any_element()
    }
}

enum State {
    Loading,
    Lines(Vec<SharedString>),
    Image(Entity<ImagePreview>),
    Error(String),
}

enum Loaded {
    Text(String),
    Image(PathBuf),
}

/// Read-only preview for a remote SFTP file, backed by a temporary download.
pub struct RemotePreview {
    remote_path: String,
    name: String,
    height: f32,
    state: State,
    scroll: UniformListScrollHandle,
    _task: Task<()>,
}
impl RemotePreview {
    pub fn new(
        connection: SshConnection,
        remote_path: String,
        height: f32,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = remote_path
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(&remote_path)
            .to_string();
        let executor = cx.background_executor().clone();
        let task_path = remote_path.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move { load(&connection, &task_path) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.state = match result {
                    Ok(Loaded::Text(text)) => State::Lines(
                        text.split('\n')
                            .map(|line| SharedString::from(line.to_string()))
                            .collect(),
                    ),
                    Ok(Loaded::Image(path)) => {
                        State::Image(cx.new(|cx| ImagePreview::new(path, this.height, cx)))
                    }
                    Err(error) => State::Error(format!("{error:#}")),
                };
                cx.notify();
            });
        });
        Self {
            remote_path,
            name,
            height,
            state: State::Loading,
            scroll: UniformListScrollHandle::new(),
            _task: task,
        }
    }
    pub fn resize(&mut self, height: f32, cx: &mut Context<Self>) {
        if (self.height - height).abs() > 0.5 {
            self.height = height;
            if let State::Image(image) = &self.state {
                image.update(cx, |image, cx| image.resize((height - 33.).max(1.), cx));
            }
            cx.notify();
        }
    }
}

fn load(connection: &SshConnection, remote: &str) -> anyhow::Result<Loaded> {
    let entry = sftp::stat_file(connection, remote)?;
    anyhow::ensure!(!entry.directory, "这是目录，无法作为文件预览");
    let image = crate::ui::image_preview::is_image_path(Path::new(remote));
    let max = if image {
        20 * 1024 * 1024
    } else {
        2 * 1024 * 1024
    };
    anyhow::ensure!(
        entry.size <= max,
        "文件超过 {} MiB，暂不预览",
        max / 1024 / 1024
    );
    let local = sftp::download_to_temp(connection, remote)?;
    if image {
        return Ok(Loaded::Image(local));
    }
    let bytes = std::fs::read(&local)?;
    if bytes.contains(&0) {
        let _ = std::fs::remove_file(&local);
        anyhow::bail!("二进制文件暂不支持预览");
    }
    match String::from_utf8(bytes) {
        Ok(text) => Ok(Loaded::Text(text)),
        Err(_) => anyhow::bail!("暂仅支持 UTF-8 文本预览"),
    }
}

impl Render for RemotePreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let copy_text = match &self.state {
            State::Lines(lines) => Some(
                lines
                    .iter()
                    .map(AsRef::as_ref)
                    .collect::<Vec<&str>>()
                    .join("\n"),
            ),
            _ => None,
        };
        let header = div()
            .h(px(33.))
            .px_3()
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .text_size(px(10.))
            .text_color(theme::muted())
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(format!("远程 · {} · 只读", self.remote_path)),
            )
            .child(div().flex_none().child(self.name.clone()))
            .when_some(copy_text, |header, text| {
                header.child(
                    Button::new("copy-remote-text", "复制")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                        }),
                )
            });
        let body = match &mut self.state {
            State::Loading => div()
                .p_4()
                .text_color(theme::muted())
                .child("正在读取远程文件…")
                .into_any_element(),
            State::Error(error) => div()
                .p_4()
                .text_color(theme::danger())
                .child(error.clone())
                .into_any_element(),
            State::Image(image) => image.clone().into_any_element(),
            State::Lines(_) => {
                let rows = uniform_list(
                    "remote-lines",
                    match &self.state {
                        State::Lines(lines) => lines.len(),
                        _ => 0,
                    },
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        let State::Lines(lines) = &this.state else {
                            return Vec::new();
                        };
                        let _ = cx;
                        range
                            .map(|index| {
                                div()
                                    .id(index)
                                    .h(px(24.))
                                    .flex()
                                    .items_center()
                                    .min_w_full()
                                    .text_size(px(13.))
                                    .font_family("JetBrains Mono")
                                    .child(
                                        div()
                                            .w(px(48.))
                                            .flex_none()
                                            .pr_3()
                                            .text_right()
                                            .text_color(theme::muted())
                                            .child((index + 1).to_string()),
                                    )
                                    .child(
                                        div()
                                            .pr_6()
                                            .child(SharedString::from(lines[index].to_string())),
                                    )
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(self.scroll.clone())
                .w_full()
                .h_full()
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained);
                div()
                    .relative()
                    .w_full()
                    .h_full()
                    .child(rows)
                    .child(
                        Scrollbar::vertical(&self.scroll)
                            .id(("remote-preview-scroll", cx.entity_id()))
                            .scrollbar_show(ScrollbarShow::Always),
                    )
                    .into_any_element()
            }
        };
        div()
            .id("remote-file-preview")
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(header)
            .child(
                div()
                    .h(px((self.height - 33.).max(1.)))
                    .w_full()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(body),
            )
    }
}
