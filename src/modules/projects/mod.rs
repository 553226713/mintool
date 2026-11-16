pub mod dialogs;
pub mod explorer;
pub mod file_menu;
pub mod git_actions;
pub mod group_menu;
pub mod menu;
pub mod model;
pub mod quick_open;
pub mod sidebar;
pub mod tasks;

use crate::services::files::Entry;
use adabraka_ui::components::{input_state::InputState, select::Select};
use gpui::Entity;
use model::ProjectRegistry;
use std::{collections::BTreeSet, path::PathBuf};

/// Sidebar state owned by the Projects module.
pub struct ProjectModule {
    pub registry: ProjectRegistry,
    pub search: Entity<gpui_component::input::InputState>,
    pub group_select: Entity<Select<Option<String>>>,
    pub commit_input: Entity<gpui_component::input::InputState>,
    pub file_filter: Entity<gpui_component::input::InputState>,
    pub rename_input: Entity<InputState>,
    pub selected_files: BTreeSet<PathBuf>,
    pub renaming: Option<PathBuf>,
    pub show_archived: bool,
    pub sort_recent: bool,
    pub filter_cache: Option<(String, Vec<Entry>)>,
    pub expanded: bool,
}
