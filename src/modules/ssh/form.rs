use adabraka_ui::components::input_state::InputState;
use gpui::Entity;

/// Inputs backing the add/edit connection dialog.
pub struct SshForm {
    pub name: Entity<InputState>,
    pub host: Entity<InputState>,
    pub port: Entity<InputState>,
    pub user: Entity<InputState>,
    pub password: Entity<InputState>,
    pub identity: Entity<InputState>,
    pub extra: Entity<InputState>,
    pub forwards: Entity<InputState>,
    pub color: Option<String>,
    pub use_keyring: bool,
    pub testing: bool,
    pub test_status: Option<(bool, String)>,
}
