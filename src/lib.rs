pub mod app;
pub mod model;
pub mod modules;
pub mod services;
pub mod shell;
pub mod ui;
pub mod workspace;

/// Release version shared by the interface and command-line entry point.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
