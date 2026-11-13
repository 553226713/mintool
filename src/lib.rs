pub mod app;
pub mod model;
pub mod services;
pub mod ui;

/// Release version shared by the interface and command-line entry point.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
