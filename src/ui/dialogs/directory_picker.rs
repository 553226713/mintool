use crate::{services::directory_browser::DirectoryBrowser, ui::theme};
use adabraka_ui::components::{
    button::{Button, ButtonSize, ButtonVariant},
    icon::Icon,
    input::Input,
    input_state::{InputEvent, InputState, SelectAll},
};
use gpui::{prelude::FluentBuilder, *};
use std::path::PathBuf;

pub enum DirectoryPickerEvent {
    Selected(PathBuf),
    Cancelled,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DirectoryPickerMode {
    Directory,
    OpenFile,
    NewFile,
}

pub struct DirectoryPicker {
    browser: DirectoryBrowser,
    mode: DirectoryPickerMode,
    address: Entity<InputState>,
    selected: Option<PathBuf>,
    show_hidden: bool,
    error: Option<String>,
    _subscription: Subscription,
}
impl EventEmitter<DirectoryPickerEvent> for DirectoryPicker {}
impl DirectoryPicker {
    pub fn new(
        browser: DirectoryBrowser,
        mode: DirectoryPickerMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let address = cx.new(InputState::new);
        let initial = match mode {
            DirectoryPickerMode::Directory | DirectoryPickerMode::OpenFile => {
                browser.path.display().to_string()
            }
            DirectoryPickerMode::NewFile => {
                browser.path.join("mintool.sqlite").display().to_string()
            }
        };
        address.update(cx, |state, cx| state.set_value(initial, window, cx));
        let subscription = cx.subscribe_in(&address, window, |this, _, event, window, cx| {
            if matches!(event, InputEvent::Enter) {
                let value = this.address.read(cx).content().to_string();
                this.activate(&value, window, cx);
            }
        });
        Self {
            browser,
            mode,
            address,
            selected: None,
            show_hidden: false,
            error: None,
            _subscription: subscription,
        }
    }
    fn navigate(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        match DirectoryBrowser::open(&path) {
            Ok(browser) => {
                let target = match self.mode {
                    DirectoryPickerMode::Directory | DirectoryPickerMode::OpenFile => {
                        browser.path.display().to_string()
                    }
                    DirectoryPickerMode::NewFile => {
                        browser.path.join("mintool.sqlite").display().to_string()
                    }
                };
                self.address.update(cx, |state, cx| {
                    state.select_all(&SelectAll, window, cx);
                    state.set_value(target, window, cx);
                });
                self.browser = browser;
                self.selected = None;
                self.error = None;
            }
            Err(error) => self.error = Some(format!("{error:#}")),
        }
        cx.notify();
    }
    fn activate(&mut self, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        let result = match self.mode {
            DirectoryPickerMode::Directory => self.browser.resolve(value).map(|path| (path, true)),
            DirectoryPickerMode::OpenFile => {
                self.browser.resolve_file(value).map(|path| (path, false))
            }
            DirectoryPickerMode::NewFile => self
                .browser
                .resolve_new_file(value)
                .map(|path| (path, false)),
        };
        match result {
            Ok((path, is_directory)) if is_directory => self.navigate(path, window, cx),
            Ok((path, _)) => {
                self.selected = Some(path);
                self.error = None;
                cx.notify();
            }
            Err(error) => {
                self.error = Some(format!("{error:#}"));
                cx.notify();
            }
        }
    }
    fn choose(&mut self, cx: &mut Context<Self>) {
        let value = self.address.read(cx).content().to_string();
        let result = match self.mode {
            DirectoryPickerMode::Directory => self.browser.resolve(&value),
            DirectoryPickerMode::OpenFile => self.browser.resolve_file(&value),
            DirectoryPickerMode::NewFile => self.browser.resolve_new_file(&value),
        };
        match result {
            Ok(path) => cx.emit(DirectoryPickerEvent::Selected(path)),
            Err(error) => {
                self.error = Some(format!("{error:#}"));
                cx.notify();
            }
        }
    }
}
impl Render for DirectoryPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let parent = self.browser.path.parent().map(|p| p.to_path_buf());
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let mut entries: Vec<_> = self
            .browser
            .directories
            .iter()
            .cloned()
            .map(|path| (path, true))
            .collect();
        if self.mode == DirectoryPickerMode::OpenFile {
            entries.extend(self.browser.files.iter().cloned().map(|path| (path, false)));
        }
        let rows: Vec<_> = entries
            .into_iter()
            .filter(|path| {
                self.show_hidden
                    || !path
                        .0
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .starts_with('.')
            })
            .map(|(path, directory)| {
                let path = path.clone();
                let icon = if directory {
                    "folder-closed"
                } else {
                    "file-database"
                };
                let selected = self.selected.as_ref() == Some(&path);
                div()
                    .id(SharedString::from(path.to_string_lossy().into_owned()))
                    .h(px(36.))
                    .flex_none()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(if selected {
                        theme::elevated()
                    } else {
                        theme::panel()
                    })
                    .hover(|s| s.bg(theme::elevated()))
                    .child(Icon::new(icon).size(px(17.)).color(theme::accent()))
                    .child(
                        div().flex_1().min_w_0().truncate().child(
                            path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned(),
                        ),
                    )
                    .when(selected, |row| {
                        row.child(Icon::new("check").size(px(14.)).color(theme::accent()))
                    })
                    .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                        if event.click_count() >= 2 {
                            if directory {
                                this.navigate(path.clone(), window, cx);
                            } else {
                                cx.emit(DirectoryPickerEvent::Selected(path.clone()));
                            }
                        } else {
                            this.address.update(cx, |state, cx| {
                                state.select_all(&SelectAll, window, cx);
                                state.set_value(path.display().to_string(), window, cx);
                            });
                            this.selected = Some(path.clone());
                            if this.mode == DirectoryPickerMode::NewFile && directory {
                                this.address.update(cx, |state, cx| {
                                    state.set_value(
                                        path.join("mintool.sqlite").display().to_string(),
                                        window,
                                        cx,
                                    );
                                });
                                this.selected = None;
                            }
                            this.error = None;
                            cx.notify();
                        }
                    }))
            })
            .collect();
        let empty = rows.is_empty();
        div()
            .id("directory-picker-layer")
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x080b13bb))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px((f32::from(window.viewport_size().width) - 48.).min(780.)))
                    .h(px(
                        (f32::from(window.viewport_size().height) - 64.).min(660.)
                    ))
                    .p_5()
                    .rounded_xl()
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::panel())
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(match self.mode {
                                        DirectoryPickerMode::Directory => "选择项目目录",
                                        DirectoryPickerMode::OpenFile => "选择 SQLite 文件",
                                        DirectoryPickerMode::NewFile => "新建 SQLite 数据库",
                                    }),
                            )
                            .child(
                                Button::new("close-picker", "")
                                    .icon("x")
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Sm)
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(DirectoryPickerEvent::Cancelled)
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("parent-directory", "上一级")
                                    .variant(ButtonVariant::Outline)
                                    .size(ButtonSize::Sm)
                                    .disabled(parent.is_none())
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if let Some(path) = &parent {
                                            this.navigate(path.clone(), window, cx);
                                        }
                                    })),
                            )
                            .child(
                                Button::new("home-directory", "主目录")
                                    .variant(ButtonVariant::Outline)
                                    .size(ButtonSize::Sm)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if let Some(path) = &home {
                                            this.navigate(path.clone(), window, cx);
                                        }
                                    })),
                            )
                            .child(div().flex_1())
                            .child(
                                Button::new(
                                    "hidden-directories",
                                    if self.show_hidden {
                                        "隐藏点目录"
                                    } else {
                                        "显示隐藏目录"
                                    },
                                )
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Sm)
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.show_hidden = !this.show_hidden;
                                        cx.notify();
                                    },
                                )),
                            ),
                    )
                    .child(
                        Input::new(&self.address)
                            .map(|mut input| {
                                input.style().align_items = Some(AlignItems::Stretch);
                                input
                            })
                            .w_full(),
                    )
                    .child(div().text_size(px(11.)).text_color(theme::muted()).child(
                        match self.mode {
                            DirectoryPickerMode::Directory => {
                                "单击选中目录，双击进入；也可输入路径后按 Enter。"
                            }
                            DirectoryPickerMode::OpenFile => {
                                "单击选择文件，双击打开；也可输入完整文件路径后按 Enter。"
                            }
                            DirectoryPickerMode::NewFile => {
                                "选择目录并在路径末尾填写文件名；文件会在确认添加后创建。"
                            }
                        },
                    ))
                    .child(
                        div()
                            .id("directory-list")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(rows)
                            .when(empty, |list| {
                                list.child(div().p_4().text_color(theme::muted()).child(
                                    if self.mode == DirectoryPickerMode::Directory {
                                        "此目录下没有可显示的子目录，可直接选择当前目录。"
                                    } else {
                                        "此目录下没有可显示的文件或子目录。"
                                    },
                                ))
                            }),
                    )
                    .when_some(self.error.clone(), |card, error| {
                        card.child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme::danger())
                                .child(error),
                        )
                    })
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme::muted())
                            .truncate()
                            .child(self.selected.as_ref().map_or_else(
                                || self.address.read(cx).content().to_string(),
                                |path| path.display().to_string(),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("cancel-picker", "取消")
                                    .variant(ButtonVariant::Outline)
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(DirectoryPickerEvent::Cancelled)
                                    })),
                            )
                            .child(
                                Button::new(
                                    "choose-directory",
                                    match self.mode {
                                        DirectoryPickerMode::Directory => {
                                            if self.selected.is_some() {
                                                "选择选中目录"
                                            } else {
                                                "选择当前目录"
                                            }
                                        }
                                        DirectoryPickerMode::OpenFile => "选择此文件",
                                        DirectoryPickerMode::NewFile => "使用此文件名",
                                    },
                                )
                                .on_click(cx.listener(|this, _, _, cx| this.choose(cx))),
                            ),
                    ),
            )
    }
}
