pub mod dialogs;
pub mod explorer;
pub mod file_menu;
pub mod group_menu;
pub mod menu;
pub mod model;
pub mod sidebar;

use adabraka_ui::components::{input_state::InputState, select::Select};
use gpui::Entity;
use model::ProjectRegistry;

/// Sidebar state owned by the Projects module.
pub struct ProjectModule {
    pub registry: ProjectRegistry,
    pub search: Entity<InputState>,
    pub group_select: Entity<Select<Option<String>>>,
    pub expanded: bool,
}
