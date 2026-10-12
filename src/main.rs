mod assets;
mod detect;
mod library;
mod scraper;
mod skyscraper_setup;
mod systems;
mod theme;
mod ui;

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{px, size, AppContext, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions};

fn main() {
    gpui_kit::application()
        .with_assets(assets::AppAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            // We keep the native title bar, so sheets can use the full height.
            Theme::update(cx, |t| t.sheet.margin_top = px(0.));
            cx.bind_keys([KeyBinding::new("/", ui::FocusSearch, Some(ui::KEY_CONTEXT))]);

            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1280.), px(800.)),
                    cx,
                ))),
                // Below this the sidebar + card grid start wrapping badly.
                window_min_size: Some(size(px(1024.), px(680.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("ROM Manager".into()),
                    ..Default::default()
                }),
                app_id: Some("retropie-rom-manager".into()),
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| ui::RootView::new(window, cx))
            })
            .expect("failed to open window");

            cx.activate(true);
        });
}
