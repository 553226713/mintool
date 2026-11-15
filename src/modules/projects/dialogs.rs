//! Project dialogs: add/locate/rename/move/remove projects and groups, plus
//! file actions from the explorer.

use crate::app::MintoolApp;
use crate::services::file_actions::FileAction;
use crate::ui::common::colors::{color_value, SWATCHES};
use crate::ui::dialogs::DialogContent;
use crate::ui::theme;
use adabraka_ui::components::button::{Button, ButtonSize, ButtonVariant};
use adabraka_ui::components::select::{Select, SelectOption};
use gpui::prelude::*;
use gpui::*;

#[derive(Clone)]
pub enum ProjectDialog {
    AddProject(Option<String>),
    AddGroup,
    MoveProject(String, String),
    RenameProject(String, String),
    RenameGroup(String, String),
    Locate(String, String),
    Remove(String, String),
    RemoveGroup(String, String),
    File(FileAction),
    SetCommand {
        id: String,
        name: String,
        command: Option<String>,
    },
    SetColor {
        id: String,
        name: String,
    },
    AddTask {
        id: String,
        name: String,
    },
}

impl ProjectDialog {
    fn group(&self, app: &MintoolApp) -> Option<String> {
        match self {
            Self::AddProject(group) => group.clone(),
            Self::MoveProject(id, _) => app
                .modules
                .projects
                .registry
                .projects
                .iter()
                .find(|project| &project.id == id)
                .and_then(|project| project.group.clone()),
            _ => None,
        }
    }
    fn has_group_select(&self) -> bool {
        matches!(self, Self::AddProject(_) | Self::MoveProject(..))
    }
}

impl DialogContent for ProjectDialog {
    fn title(&self) -> String {
        match self {
            Self::AddProject(_) => "添加项目",
            Self::AddGroup => "新建分组",
            Self::MoveProject(..) => "移动项目",
            Self::RenameProject(..) => "重命名项目",
            Self::RenameGroup(..) => "重命名分组",
            Self::Locate(..) => "重新定位目录",
            Self::Remove(..) => "移除项目",
            Self::RemoveGroup(..) => "删除分组",
            Self::File(action) => action.title(),
            Self::SetCommand { .. } => "设置启动命令",
            Self::SetColor { .. } => "设置项目颜色",
            Self::AddTask { .. } => "添加项目任务",
        }
        .into()
    }
    fn input_value(&self, _app: &MintoolApp) -> String {
        match self {
            Self::RenameProject(_, name) | Self::RenameGroup(_, name) | Self::Locate(_, name) => {
                name.clone()
            }
            Self::SetCommand { command, .. } => command.clone().unwrap_or_default(),
            Self::File(FileAction::Rename(path)) => path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            _ => String::new(),
        }
    }
    fn input_hint(&self) -> Option<&'static str> {
        match self {
            Self::AddProject(_) | Self::Locate(..) => Some("项目目录的完整路径"),
            Self::AddGroup
            | Self::RenameProject(..)
            | Self::RenameGroup(..)
            | Self::File(
                FileAction::Rename(_) | FileAction::NewFile(_) | FileAction::NewDirectory(_),
            ) => Some("名称"),
            Self::SetCommand { .. } => {
                Some("启动命令（留空禁用；每次新建终端时执行，之后进入交互 Shell）")
            }
            Self::MoveProject(..)
            | Self::Remove(..)
            | Self::RemoveGroup(..)
            | Self::File(_)
            | Self::SetColor { .. }
            | Self::AddTask { .. } => None,
        }
    }
    fn accepts_empty_input(&self) -> bool {
        matches!(
            self,
            Self::Remove(..)
                | Self::RemoveGroup(..)
                | Self::MoveProject(..)
                | Self::File(FileAction::Trash(_))
                | Self::SetCommand { .. }
                | Self::SetColor { .. }
        )
    }
    fn has_actions(&self) -> bool {
        !matches!(self, Self::SetColor { .. })
    }
    fn is_directory(&self) -> bool {
        matches!(self, Self::AddProject(_) | Self::Locate(..))
    }
    fn subject(&self) -> Option<String> {
        match self {
            Self::Remove(_, name)
            | Self::RemoveGroup(_, name)
            | Self::MoveProject(_, name)
            | Self::SetCommand { name, .. }
            | Self::SetColor { name, .. }
            | Self::AddTask { name, .. } => Some(name.clone()),
            _ => None,
        }
    }
    fn path(&self) -> Option<String> {
        match self {
            Self::File(action) => Some(action.path().display().to_string()),
            _ => None,
        }
    }
    fn note(&self, _app: &MintoolApp) -> Option<String> {
        Some(match self {
            Self::Remove(..) => {
                "只移除软件内的记录，保留磁盘文件。该项目的终端会关闭。".to_string()
            }
            Self::RemoveGroup(..) => {
                "删除此分组，项目会移到未分组。保留磁盘文件和运行中的终端。".to_string()
            }
            Self::Locate(..) => "保留布局，并在新目录重新启动终端。".to_string(),
            Self::SetCommand { .. } => {
                "命令会在新建终端时执行，执行完进入交互 Shell；已有终端不受影响。".to_string()
            }
            Self::SetColor { .. } => "选择项目列表中的标记颜色。".to_string(),
            Self::AddTask { .. } => "任务会在当前已打开的内置终端中执行。".to_string(),
            Self::File(FileAction::Trash(_)) => {
                "将此项目及其内容移到系统回收站，可通过系统文件管理器恢复。".to_string()
            }
            _ => return None,
        })
    }
    fn confirm_label(&self) -> &'static str {
        match self {
            Self::File(FileAction::Trash(_)) => "移到回收站",
            Self::RemoveGroup(..) => "删除分组",
            Self::Remove(..) => "移除记录",
            Self::AddProject(..) => "添加项目",
            Self::MoveProject(..) => "移动",
            _ => "保存",
        }
    }
    fn prepare(&self, app: &mut MintoolApp, _window: &mut Window, cx: &mut Context<MintoolApp>) {
        if !self.has_group_select() {
            return;
        }
        let selected_group = self.group(app);
        let options: Vec<_> = std::iter::once(SelectOption::new(None, "未分组"))
            .chain(
                app.modules
                    .projects
                    .registry
                    .groups
                    .iter()
                    .map(|group| SelectOption::new(Some(group.id.clone()), group.name.clone())),
            )
            .collect();
        let selected = options
            .iter()
            .position(|option| option.value == selected_group)
            .unwrap_or(0);
        app.modules.projects.group_select = cx.new(|cx| {
            Select::new(cx)
                .options(options)
                .selected_index(Some(selected))
                .w_full()
        });
    }
    fn render_extra(&self, app: &MintoolApp, cx: &mut Context<MintoolApp>) -> Option<AnyElement> {
        if let Self::SetColor { id, .. } = self {
            let current = app
                .modules
                .projects
                .registry
                .projects
                .iter()
                .find(|project| &project.id == id)
                .and_then(|project| project.color.clone());
            let mut row = div().flex().items_center().gap_2();
            for (index, (hex, _label)) in SWATCHES.iter().enumerate() {
                let project_id = id.clone();
                let value = hex.to_string();
                row = row.child(
                    div()
                        .id(SharedString::from(format!("project-color-{index}")))
                        .size(px(24.))
                        .rounded_full()
                        .bg(rgb(color_value(hex)))
                        .cursor_pointer()
                        .border_2()
                        .border_color(if current.as_deref() == Some(*hex) {
                            theme::text()
                        } else {
                            theme::panel()
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(project) = this
                                .modules
                                .projects
                                .registry
                                .projects
                                .iter_mut()
                                .find(|project| project.id == project_id)
                            {
                                project.color = Some(value.clone());
                            }
                            this.dialog = None;
                            this.changed(cx);
                        })),
                );
            }
            let clear_id = id.clone();
            row = row.child(
                Button::new("clear-project-color", "无")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(project) = this
                            .modules
                            .projects
                            .registry
                            .projects
                            .iter_mut()
                            .find(|project| project.id == clear_id)
                        {
                            project.color = None;
                        }
                        this.dialog = None;
                        this.changed(cx);
                    })),
            );
            return Some(row.into_any_element());
        }
        if !self.has_group_select() {
            return None;
        }
        Some(
            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(crate::ui::theme::muted())
                        .child("所属分组"),
                )
                .child(app.modules.projects.group_select.clone())
                .into_any_element(),
        )
    }
    fn submit(
        &self,
        app: &mut MintoolApp,
        value: String,
        cx: &mut Context<MintoolApp>,
    ) -> anyhow::Result<()> {
        match self {
            Self::File(action) => app.apply_file_action(action, &value, cx),
            Self::AddProject(_) => {
                let group = app
                    .modules
                    .projects
                    .group_select
                    .read(cx)
                    .selected_value()
                    .cloned()
                    .flatten();
                app.add_path(value.into(), None, group, cx)
            }
            Self::Locate(id, _) => app.add_path(value.into(), Some(id.clone()), None, cx),
            Self::MoveProject(id, _) => {
                let group = app
                    .modules
                    .projects
                    .group_select
                    .read(cx)
                    .selected_value()
                    .cloned()
                    .flatten();
                app.modules
                    .projects
                    .registry
                    .move_project(id, group.clone(), None);
                if let Some(group) = group {
                    if let Some(group) = app
                        .modules
                        .projects
                        .registry
                        .groups
                        .iter_mut()
                        .find(|candidate| candidate.id == group)
                    {
                        group.collapsed = false;
                    }
                }
                Ok(())
            }
            Self::AddGroup => app.modules.projects.registry.add_group(&value).map(|_| ()),
            Self::RenameProject(id, _) => {
                if let Some(project) = app
                    .modules
                    .projects
                    .registry
                    .projects
                    .iter_mut()
                    .find(|project| &project.id == id)
                {
                    project.name = value;
                }
                Ok(())
            }
            Self::RenameGroup(id, _) => {
                if app
                    .modules
                    .projects
                    .registry
                    .groups
                    .iter()
                    .any(|group| &group.id != id && group.name == value)
                {
                    Err(anyhow::anyhow!("分组名称已存在"))
                } else {
                    if let Some(group) = app
                        .modules
                        .projects
                        .registry
                        .groups
                        .iter_mut()
                        .find(|group| &group.id == id)
                    {
                        group.name = value;
                    }
                    Ok(())
                }
            }
            Self::Remove(id, _) => {
                app.remove_project(id, cx);
                Ok(())
            }
            Self::RemoveGroup(id, _) => {
                app.modules.projects.registry.remove_group(id);
                Ok(())
            }
            Self::SetCommand { id, .. } => {
                let command = value.trim();
                if let Some(project) = app
                    .modules
                    .projects
                    .registry
                    .projects
                    .iter_mut()
                    .find(|project| &project.id == id)
                {
                    project.run_command = (!command.is_empty()).then(|| command.to_string());
                }
                Ok(())
            }
            Self::SetColor { .. } => Ok(()),
            Self::AddTask { id, .. } => {
                let task = value.trim();
                if !task.is_empty() {
                    if let Some(project) = app
                        .modules
                        .projects
                        .registry
                        .projects
                        .iter_mut()
                        .find(|project| &project.id == id)
                    {
                        if !project.tasks.iter().any(|existing| existing == task) {
                            project.tasks.push(task.to_string());
                        }
                    }
                }
                Ok(())
            }
        }
    }
}
