//! Shared presentation layer.
//!
//! `common` holds app-wide primitives, `terminal` and `preview` hold reusable
//! widgets, and `dialogs` holds the generic modal shell. Module-specific UI
//! lives under the owning feature module.

pub mod common;
pub mod dialogs;
pub mod preview;
pub mod terminal;

// Keep the historical `crate::ui::<name>` paths working for the rest of the
// crate while the files live in focused subdirectories.
pub use common::{assets, context_menu, drag_item, file_icon, format, resize, theme};
pub use dialogs::directory_picker;
pub use preview::{file_preview, image_preview};
pub use terminal::{system_terminal, terminal_input, terminal_surface};
