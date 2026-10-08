//! Left sidebar: app mark, one row per system, and Folder / About / theme.
//!
//! Uses GPUI Kit's `Sidebar` container with our own row type, because the
//! kit's `SidebarMenuItem` only takes a plain icon and each system here gets
//! a small solid tile in its accent colour.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::sidebar::{Sidebar, SidebarItem};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Collapsible, Icon, Sizable, StyledExt};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::rc::Rc;

use super::details;
use super::root::RootView;
use crate::assets;
use crate::systems::{self, SystemDef};
use crate::theme;

/// The system's icon as a single-colour glyph (`assets/icons/*.svg`), or a
/// generic gamepad if the file isn't there.
pub(super) fn system_glyph(system: &SystemDef, size: Pixels, color: Hsla) -> AnyElement {
    match assets::system_icon(system.id, system.icon) {
        Some(path) => svg()
            .external_path(path.to_string_lossy().to_string())
            .size(size)
            .flex_shrink_0()
            .text_color(color)
            .into_any_element(),
        None => Icon::new(IconName::Gamepad2).with_size(size).text_color(color).into_any_element(),
    }
}

/// Solid accent-coloured square with the system's glyph in white.
pub(super) fn system_tile(system: &SystemDef, tile: Pixels, glyph: Pixels, radius: Pixels) -> Div {
    div()
        .size(tile)
        .flex_shrink_0()
        .rounded(radius)
        .bg(theme::accent(system.accent))
        .flex()
        .items_center()
        .justify_center()
        .child(system_glyph(system, glyph, white()))
}

#[derive(Clone)]
struct SystemRow {
    system: &'static SystemDef,
    count: usize,
    active: bool,
    on_click: Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>,
}

impl Collapsible for SystemRow {
    fn collapsed(self, _: bool) -> Self {
        self
    }

    fn is_collapsed(&self) -> bool {
        false
    }
}

impl SidebarItem for SystemRow {
    fn render(self, id: impl Into<ElementId>, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let on_click = self.on_click.clone();
        h_flex()
            .id(id)
            .relative()
            .h(px(36.))
            .px_2()
            .gap_2p5()
            .rounded(t.radius)
            .text_sm()
            .cursor_pointer()
            .text_color(if self.active { t.sidebar_accent_foreground } else { t.sidebar_foreground.opacity(0.8) })
            .hover(|s| s.bg(t.sidebar_accent))
            .when(self.active, |row| {
                row.bg(t.sidebar_accent).font_medium().child(
                    // Active marker in the row's left padding.
                    div()
                        .absolute()
                        .left(px(1.))
                        .top(px(10.))
                        .bottom(px(10.))
                        .w(px(3.))
                        .rounded_r(px(2.))
                        .bg(theme::accent(self.system.accent)),
                )
            })
            .child(system_tile(self.system, px(24.), px(14.), px(6.)))
            .child(div().flex_1().min_w_0().truncate().child(self.system.display_name))
            .child(
                div()
                    .text_xs()
                    .text_color(if self.active { t.sidebar_accent_foreground } else { t.muted_foreground })
                    .child(self.count.to_string()),
            )
            .on_click(move |event, window, cx| on_click(event, window, cx))
    }
}

impl RootView {
    pub(super) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme();
        let is_dark = t.is_dark();

        let brand = v_flex()
            .gap_4()
            .child(
                h_flex()
                    .gap_2p5()
                    .px_2()
                    .pt_1()
                    .child(
                        div()
                            .size(px(32.))
                            .rounded(px(8.))
                            .bg(t.foreground)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(IconName::Gamepad2).with_size(px(18.)).text_color(t.background)),
                    )
                    .child(
                        v_flex()
                            .child(div().text_sm().font_semibold().child("ROM Manager"))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(t.muted_foreground)
                                    .child(concat!("for RetroPie · v", env!("CARGO_PKG_VERSION"))),
                            ),
                    ),
            )
            .child(
                div()
                    .px_2()
                    .text_xs()
                    .font_medium()
                    .text_color(t.muted_foreground)
                    .child("Systems"),
            );

        let rows = systems::SYSTEMS.iter().map(|system| {
            let listener = cx.listener(move |this: &mut RootView, _: &ClickEvent, window, cx| {
                this.select_system(system, window, cx)
            });
            SystemRow {
                system,
                count: self.counts.get(system.id).copied().unwrap_or(0),
                active: system.id == self.selected.id,
                on_click: Rc::new(listener),
            }
        });

        let roms_root = crate::library::roms_root();
        let footer = h_flex()
            .w_full()
            .gap_1()
            .child(
                Button::new("open-roms")
                    .ghost()
                    .small()
                    .icon(Icon::new(IconName::FolderOpen))
                    .label("Folder")
                    .tooltip("Open ~/RetroPie/roms")
                    .on_click(cx.listener(move |this, _, _, cx| this.open_folder(roms_root.clone(), cx))),
            )
            .child(
                Button::new("about")
                    .ghost()
                    .small()
                    .icon(Icon::new(IconName::Info))
                    .label("About")
                    .on_click(cx.listener(|this, _, window, cx| details::open_about(this, window, cx))),
            )
            .child(div().flex_1())
            .child(
                Button::new("theme")
                    .ghost()
                    .small()
                    .icon(Icon::new(if is_dark { IconName::Moon } else { IconName::Sun }))
                    .tooltip(if is_dark { "Switch to light" } else { "Switch to dark" })
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_theme(window, cx))),
            );

        Sidebar::new("systems")
            .w(px(240.))
            .collapsible(false)
            .header(brand)
            .children(rows)
            .footer(footer)
    }
}
