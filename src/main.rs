mod assets;
mod library;
mod systems;
mod theme;
mod ui;

use gpui::{
    px, size, App, AppContext, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions,
};

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1100.), px(720.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // Keep the sidebar + card grid usable — below this the
                // layout starts wrapping badly.
                window_min_size: Some(size(px(1024.), px(768.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("ROM manager".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| ui::RootView::new(window, cx)),
        )
        .expect("failed to open window");

        cx.activate(true);
    });
}
