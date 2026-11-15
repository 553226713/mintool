use super::{
    driver::{self, QueryCancel},
    model::{CellValue, DatabaseConnection, QueryResult},
};
use crate::model::query::statement_at;
use crate::ui::theme;
use crate::{
    app::MintoolApp,
    model::session::{SessionKey, SessionView},
    workspace::layout::Pane,
};
use adabraka_ui::components::button::{Button, ButtonSize, ButtonVariant};
use adabraka_ui::components::icon::Icon;
use gpui::{prelude::FluentBuilder, *};
use gpui_component::input::{Input, InputEvent, InputState, TabSize};
use gpui_component::resizable::{resizable_panel, v_resizable};
use std::sync::mpsc::{self, Receiver};

const MAX_RESULT_ROWS: usize = 1000;
const CELL_WIDTH: f32 = 168.;

#[derive(Clone, Debug)]
pub enum QueryViewEvent {
    SqlChanged(String),
}

impl EventEmitter<QueryViewEvent> for DatabaseQueryView {}

pub struct DatabaseQueryView {
    connection: DatabaseConnection,
    editor: Entity<InputState>,
    running: bool,
    cancelling: bool,
    cancel_receiver: Option<Receiver<QueryCancel>>,
    result: Option<QueryResult>,
    error: Option<String>,
    auto_run: bool,
    _editor_subscription: Subscription,
}

impl DatabaseQueryView {
    pub fn new(
        connection: DatabaseConnection,
        sql: String,
        auto_run: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor = cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .code_editor("sql")
                .line_number(true)
                .soft_wrap(false)
                .tab_size(TabSize {
                    tab_size: 2,
                    hard_tabs: false,
                })
                .placeholder("输入 SQL，Ctrl+Enter 执行当前语句");
            state.set_value(sql, window, cx);
            state
        });
        let editor_subscription = cx.subscribe(
            &editor,
            |_, editor: Entity<InputState>, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.emit(QueryViewEvent::SqlChanged(
                        editor.read(cx).value().to_string(),
                    ));
                    cx.notify();
                }
            },
        );
        Self {
            connection,
            editor,
            running: false,
            cancelling: false,
            cancel_receiver: None,
            result: None,
            error: None,
            auto_run,
            _editor_subscription: editor_subscription,
        }
    }

    pub fn focus_editor(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.read(cx).focus_handle(cx).focus(window);
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        if self.running {
            return;
        }
        let (sql, cursor) = {
            let editor = self.editor.read(cx);
            (editor.value().to_string(), editor.cursor())
        };
        let statement = statement_at(&sql, cursor);
        if statement.is_empty() {
            self.error = Some("请输入要执行的 SQL".into());
            cx.notify();
            return;
        }
        self.running = true;
        self.cancelling = false;
        self.error = None;
        self.result = None;
        self.cancel_receiver = None;
        cx.notify();

        let connection = self.connection.clone();
        let executor = cx.background_executor().clone();
        let (cancel_sender, cancel_receiver) = mpsc::channel();
        self.cancel_receiver = Some(cancel_receiver);
        cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move {
                    driver::execute(&connection, &statement, MAX_RESULT_ROWS, move |interrupt| {
                        let _ = cancel_sender.send(interrupt);
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.running = false;
                this.cancelling = false;
                this.cancel_receiver = None;
                match result {
                    Ok(result) => this.result = Some(result),
                    Err(error) => this.error = Some(driver::friendly_error(&error)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(receiver) = &self.cancel_receiver {
            if let Ok(interrupt) = receiver.try_recv() {
                let supported = interrupt.is_supported();
                interrupt.interrupt();
                if supported {
                    self.cancelling = true;
                } else {
                    self.error = Some("当前数据库驱动暂不支持取消正在执行的查询".into());
                }
                cx.notify();
            }
        }
    }

    fn render_result(&self, result: &QueryResult) -> AnyElement {
        if result.columns.is_empty() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme::muted())
                .child(format!(
                    "执行完成 · {} 行受影响 · {} ms",
                    result.affected_rows.unwrap_or(0),
                    result.elapsed_ms
                ))
                .into_any_element();
        }
        let table_width = (result.columns.len() as f32 * CELL_WIDTH).max(CELL_WIDTH);
        let columns = result
            .columns
            .iter()
            .enumerate()
            .map(|(index, column)| {
                div()
                    .w(px(CELL_WIDTH))
                    .flex_none()
                    .px_3()
                    .truncate()
                    .text_color(theme::accent())
                    .child(format!("{}  {column}", index + 1))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let rows = result
            .rows
            .iter()
            .enumerate()
            .map(|(row_index, row)| {
                let cells = row
                    .iter()
                    .map(|cell| {
                        let color = if matches!(cell, CellValue::Null) {
                            theme::muted()
                        } else {
                            theme::text()
                        };
                        div()
                            .w(px(CELL_WIDTH))
                            .flex_none()
                            .px_3()
                            .truncate()
                            .text_color(color)
                            .child(cell.display())
                            .into_any_element()
                    })
                    .collect::<Vec<_>>();
                div()
                    .h(px(27.))
                    .min_w(px(table_width))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(theme::border())
                    .when(row_index % 2 == 1, |row| row.bg(theme::panel()))
                    .children(cells)
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .id("database-result-scroll")
            .size_full()
            .overflow_scroll()
            .font_family("JetBrains Mono")
            .text_size(px(11.))
            .child(
                div()
                    .min_w(px(table_width))
                    .top_0()
                    .h(px(29.))
                    .flex()
                    .items_center()
                    .bg(theme::elevated())
                    .border_b_1()
                    .border_color(theme::border())
                    .children(columns),
            )
            .children(rows)
            .into_any_element()
    }
}

impl Render for DatabaseQueryView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.auto_run {
            self.auto_run = false;
            self.run(cx);
        }
        let executable = cx.entity().downgrade();
        let cancellable = cx.entity().downgrade();
        let status = if self.cancelling {
            "正在取消…".to_string()
        } else if self.running {
            "正在执行…".to_string()
        } else if let Some(result) = &self.result {
            let affected = result
                .affected_rows
                .map(|rows| format!(" · {rows} 行受影响"))
                .unwrap_or_default();
            let truncated = if result.truncated {
                format!(" · 仅显示前 {MAX_RESULT_ROWS} 行")
            } else {
                String::new()
            };
            format!(
                "{} 行 · {} ms{affected}{truncated}",
                result.rows.len(),
                result.elapsed_ms
            )
        } else {
            "Ctrl+Enter 执行当前语句".into()
        };
        let content = if let Some(error) = &self.error {
            div()
                .size_full()
                .p_4()
                .font_family("JetBrains Mono")
                .text_size(px(11.))
                .text_color(theme::danger())
                .child(error.clone())
                .into_any_element()
        } else if let Some(result) = &self.result {
            self.render_result(result)
        } else {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme::muted())
                .child(if self.running {
                    "正在执行查询…"
                } else {
                    "执行查询后在这里查看结果"
                })
                .into_any_element()
        };
        div()
            .id("database-query-pane")
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(theme::background())
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if (event.keystroke.modifiers.control || event.keystroke.modifiers.platform)
                    && event.keystroke.key.eq_ignore_ascii_case("enter")
                {
                    this.run(cx);
                    cx.stop_propagation();
                }
            }))
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
                    .child(
                        Icon::new("file-database")
                            .size(px(14.))
                            .color(theme::accent()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(11.))
                            .text_color(theme::muted())
                            .child(format!(
                                "{} · {}",
                                self.connection.engine.label(),
                                self.connection.subtitle()
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme::muted())
                            .child(status),
                    )
                    .when(self.running, |bar| {
                        bar.child(
                            Button::new("database-cancel", "取消")
                                .variant(ButtonVariant::Outline)
                                .size(ButtonSize::Sm)
                                .disabled(self.cancelling)
                                .on_click(move |_, _, cx| {
                                    let _ = cancellable.update(cx, |view, cx| view.cancel(cx));
                                }),
                        )
                    })
                    .when(!self.running, |bar| {
                        bar.child(
                            Button::new("database-run", "执行")
                                .variant(ButtonVariant::Default)
                                .size(ButtonSize::Sm)
                                .icon("square-terminal")
                                .tooltip("Ctrl+Enter")
                                .on_click(move |_, _, cx| {
                                    let _ = executable.update(cx, |view, cx| view.run(cx));
                                }),
                        )
                    }),
            )
            .child(
                div().flex_1().min_h_0().child(
                    v_resizable(("database-editor-results", cx.entity_id()))
                        .child(
                            resizable_panel()
                                .size(px(240.))
                                .size_range(px(100.)..px(900.))
                                .child(
                                    div().size_full().min_h_0().p_2().child(
                                        Input::new(&self.editor)
                                            .appearance(false)
                                            .bordered(false)
                                            .font_family("JetBrains Mono")
                                            .text_size(px(13.))
                                            .h_full(),
                                    ),
                                ),
                        )
                        .child(
                            resizable_panel().child(
                                div()
                                    .size_full()
                                    .min_h_0()
                                    .border_t_1()
                                    .border_color(theme::border())
                                    .child(content),
                            ),
                        ),
                ),
            )
    }
}

impl MintoolApp {
    pub(crate) fn render_database_pane(
        &mut self,
        session: &SessionView,
        pane: &Pane,
        focused: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(query) = self
            .workspaces
            .get(&session.key)
            .and_then(|workspace| workspace.state.queries.get(&pane.id))
            .cloned()
        else {
            let key = session.key.clone();
            let id = pane.id;
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .text_color(theme::muted())
                .child("查询已关闭")
                .child(
                    Button::new(
                        SharedString::from(format!("new-database-query-{id}")),
                        "新建查询",
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.create_database_query(key.clone(), id, cx);
                    })),
                )
                .into_any_element();
        };
        let Some(connection) = self.modules.database.connection(&session.key.id).cloned() else {
            return div()
                .size_full()
                .p_4()
                .text_color(theme::danger())
                .child("数据库连接不存在，请从左侧重新选择")
                .into_any_element();
        };
        let key = (session.key.clone(), pane.id);
        let view = if let Some(view) = self.modules.database.query_views.get(&key) {
            view.clone()
        } else {
            let view = cx.new(|cx| {
                DatabaseQueryView::new(connection, query.sql.clone(), query.auto_run, window, cx)
            });
            let session_key = session.key.clone();
            let pane_id = pane.id;
            let subscription = cx.subscribe(&view, move |app, _, event: &QueryViewEvent, cx| {
                let QueryViewEvent::SqlChanged(sql) = event;
                if let Some(query) = app
                    .workspaces
                    .get_mut(&session_key)
                    .and_then(|workspace| workspace.state.queries.get_mut(&pane_id))
                {
                    query.sql = sql.clone();
                }
                app.dirty_workspaces.insert(session_key.clone());
                app.dirty = Some(std::time::Instant::now());
                cx.notify();
            });
            self.modules.database.query_views.insert(key, view.clone());
            self.keep_subscription(subscription);
            view
        };
        if focused && self.focus_pending {
            view.update(cx, |view, cx| view.focus_editor(window, cx));
            self.focus_pending = false;
        }
        view.into_any_element()
    }

    pub(crate) fn create_database_query(
        &mut self,
        key: SessionKey,
        pane: u64,
        cx: &mut Context<Self>,
    ) {
        if let Some(workspace) = self.workspaces.get_mut(&key) {
            workspace.state.add_query(pane);
            if let Some(query) = workspace.state.layout.find_mut(pane) {
                query.terminal = false;
                query.terminal_closed = false;
            }
        }
        self.modules.database.drop_query(&key, pane);
        self.dirty_workspaces.insert(key);
        self.focus_pending = true;
        self.changed(cx);
    }
}
