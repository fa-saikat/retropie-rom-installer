//! App-level theming on top of GPUI Kit's Default Light/Dark themes.
//!
//! The kit theme supplies every neutral (background, borders, muted text…);
//! the only thing we add is the selected system's accent, which becomes the
//! theme's primary colour so primary buttons, focus rings and progress bars
//! pick it up. It's applied flat — no gradients or glows.

use gpui_kit::component::{ActiveTheme, Colorize, Theme, ThemeMode};
use gpui_kit::{rgb, white, App, Hsla, Window};

pub fn accent(color: u32) -> Hsla {
    rgb(color).into()
}

/// Make `color` the theme's primary colour.
pub fn apply_accent(color: u32, cx: &mut App) {
    let c = accent(color);
    Theme::update(cx, |t| {
        t.primary = c;
        t.primary_hover = c.opacity(0.9);
        t.primary_active = c.darken(0.1);
        t.primary_foreground = white();
        t.button_primary = c;
        t.button_primary_hover = c.opacity(0.9);
        t.button_primary_active = c.darken(0.1);
        t.button_primary_foreground = white();
        t.ring = c;
        t.sidebar_primary = c;
        t.progress_bar = c;
    });
}

/// Flip light/dark. Changing mode reloads the kit's colours, so the accent
/// is re-applied afterwards.
pub fn toggle_mode(accent_color: u32, window: &mut Window, cx: &mut App) {
    let mode = if cx.theme().is_dark() { ThemeMode::Light } else { ThemeMode::Dark };
    Theme::change(mode, Some(window), cx);
    apply_accent(accent_color, cx);
}

/// Background for raised surfaces (game cards, the list view): one step up
/// from the window background in either mode.
pub fn card_bg(cx: &App) -> Hsla {
    cx.theme().secondary.opacity(0.45)
}
