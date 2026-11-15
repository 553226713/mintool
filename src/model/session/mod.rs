//! Module-neutral session types.
//!
//! These types describe "what is open" without depending on any feature
//! module. Feature modules build `SessionView` values from their own models,
//! and the workspace layer only consumes the neutral description.

mod capabilities;
mod key;
mod launcher;
mod module_id;
mod ssh_launch;
mod view;

pub use capabilities::SessionCapabilities;
pub use key::SessionKey;
pub use launcher::TerminalLauncher;
pub use module_id::{ModuleId, MODULES};
pub use ssh_launch::SshLaunch;
pub use view::SessionView;
