use gpui::*;
use mintool::{
    app::MintoolApp, model::settings::Settings, modules::projects::model::ProjectRegistry,
    services::storage::Storage, ui::theme,
};
fn main() -> anyhow::Result<()> {
    if std::env::var_os("MINTOOL_ASKPASS").is_some() {
        if let Ok(password) = std::env::var("MINTOOL_ASKPASS_PASSWORD") {
            print!("{password}");
        }
        return Ok(());
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|argument| argument == "--version" || argument == "-V")
    {
        println!("Mintool {}", mintool::VERSION);
        return Ok(());
    }
    mintool::services::ime::configure_x11_ime();
    let storage = Storage::user()?;
    let settings = Settings::load(&storage)?;
    let mut projects = ProjectRegistry::load(&storage)?;
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
            // Map the first frame at the display's full bounds. Using a smaller
            // restore size here makes X11 show a short windowed frame before
            // the window manager applies the maximize hint.
            let bounds = Bounds::maximized(None, cx);
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
            cx.open_window(options, |window, cx| {
                window.set_window_title(&format!("Mintool {}", mintool::VERSION));
                let app = cx.new(|cx| MintoolApp::new(storage, settings, projects, window, cx));
                let close_app = app.clone();
                let root = cx.new(|cx| gpui_component::Root::new(app, window, cx));
                window.on_window_should_close(cx, move |window, cx| {
                    close_app.update(cx, |app, cx| app.request_quit(window, cx))
                });
                root
            })
            .expect("无法打开 Mintool 窗口");
            cx.activate(true);
        });
    Ok(())
}
