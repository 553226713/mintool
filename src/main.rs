use gpui::*;
use mintool::{app::MintoolApp, services::storage::Storage, ui::theme};
fn main() -> anyhow::Result<()> {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|argument| argument == "--version" || argument == "-V")
    {
        println!("Mintool {}", mintool::VERSION);
        return Ok(());
    }
    mintool::services::ime::configure_x11_ime();
    let storage = Storage::user()?;
    let settings = storage.settings()?;
    let mut projects = storage.projects()?;
    if let Some(path) = std::env::args_os().nth(1) {
        projects.add(path.into())?;
    }
    if !projects
        .projects
        .iter()
        .any(|p| Some(&p.id) == projects.selected.as_ref())
    {
        projects.selected = projects.projects.first().map(|p| p.id.clone());
    }
    Application::new()
        .with_assets(mintool::ui::assets::Assets)
        .run(move |cx| {
            adabraka_ui::init(cx);
            gpui_component::init(cx);
            adabraka_ui::set_icon_base_path("icons");
            theme::install(cx, settings.appearance);
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Maximized(bounds)),
                window_min_size: Some(size(px(950.), px(550.))),
                app_id: Some("mintool".into()),
                titlebar: Some(TitlebarOptions {
                    title: Some("Mintool".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let window = cx
                .open_window(options, |window, cx| {
                    window.set_window_title(&format!("Mintool {}", mintool::VERSION));
                    let app = cx.new(|cx| MintoolApp::new(storage, settings, projects, cx));
                    cx.new(|cx| gpui_component::Root::new(app, window, cx))
                })
                .expect("无法打开 Mintool 窗口");
            // X11 window managers may ignore maximize requests before mapping.
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(150))
                    .await;
                let _ = window.update(cx, |_, window, _| {
                    if !window.is_maximized() {
                        window.zoom_window();
                    }
                });
            })
            .detach();
            cx.activate(true);
        });
    Ok(())
}
