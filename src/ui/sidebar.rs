//! Left sidebar: app mark, one row per listed system, "Add emulator", and
//! Folder / About / theme.
//!
//! Uses GPUI Kit's `Sidebar` container with our own row type, because the
//! kit's `SidebarMenuItem` only takes a plain icon and each system here gets
//! a small solid tile in its accent colour.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::sidebar::{Sidebar, SidebarItem};
use gpui_kit::component::tag::Tag;
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

type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(Clone)]
struct SystemRow {
    system: &'static SystemDef,
    count: usize,
    /// Games in this system that belong to another one.
    flagged: usize,
    active: bool,
    on_click: Handler,
    /// `None` when it's the last system left (the list can't be empty).
    on_remove: Option<Handler>,
}

/// The dashed "Add emulator" row after the systems.
#[derive(Clone)]
struct AddRow {
    /// Systems not listed yet.
    available: usize,
    /// ...of which have games waiting for them.
    suggested: usize,
    on_click: Handler,
}

#[derive(Clone)]
enum Row {
    System(SystemRow),
    Add(AddRow),
}

impl Collapsible for Row {
    fn collapsed(self, _: bool) -> Self {
        self
    }

    fn is_collapsed(&self) -> bool {
        false
    }
}

impl SidebarItem for Row {
    fn render(self, id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> impl IntoElement {
        match self {
            Row::System(row) => row.render(id.into(), window, cx).into_any_element(),
            Row::Add(row) => row.render(id.into(), cx).into_any_element(),
        }
    }
}

impl SystemRow {
    fn render(self, id: ElementId, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let on_click = self.on_click.clone();
        let muted = if self.active { t.sidebar_accent_foreground } else { t.muted_foreground };
        h_flex()
            .id(id)
            .group("system-row")
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
            .when(self.flagged > 0, |row| {
                row.child(div().size(px(6.)).flex_shrink_0().rounded_full().bg(t.danger))
            })
            // The count makes way for the remove button on hover.
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .when(self.on_remove.is_some(), |count| count.group_hover("system-row", |s| s.invisible()))
                    .child(self.count.to_string()),
            )
            .when_some(self.on_remove, |row, on_remove| {
                row.child(
                    div()
                        .absolute()
                        .right(px(4.))
                        .top(px(6.))
                        .invisible()
                        .group_hover("system-row", |s| s.visible())
                        .child(
                            Button::new(SharedString::from(format!("remove-{}", self.system.id)))
                                .ghost()
                                .xsmall()
                                .icon(Icon::new(IconName::X))
                                .tooltip(format!("Remove {}", self.system.display_name))
                                .on_click(move |event, window, cx| {
                                    cx.stop_propagation();
                                    on_remove(event, window, cx);
                                }),
                        ),
                )
            })
            .on_click(move |event, window, cx| on_click(event, window, cx))
    }
}

impl AddRow {
    fn render(self, id: ElementId, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let on_click = self.on_click.clone();
        h_flex()
            .id(id)
            .mt_1()
            .h(px(36.))
            .px_2()
            .gap_2p5()
            .rounded(t.radius)
            .border_1()
            .border_dashed()
            .border_color(t.border)
            .text_sm()
            .cursor_pointer()
            .text_color(t.sidebar_foreground.opacity(0.8))
            .hover(|s| s.bg(t.sidebar_accent).border_color(t.muted_foreground))
            .child(
                div()
                    .size(px(24.))
                    .flex_shrink_0()
                    .rounded(px(6.))
                    .bg(t.muted)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(IconName::Plus).with_size(px(14.)).text_color(t.muted_foreground)),
            )
            .child(div().flex_1().min_w_0().truncate().child("Add emulator"))
            .child(if self.suggested > 0 {
                // Systems with games already waiting for them.
                Tag::danger().small().rounded_full().child(self.suggested.to_string()).into_any_element()
            } else {
                div().text_xs().text_color(t.muted_foreground).child(self.available.to_string()).into_any_element()
            })
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

        let can_remove = self.enabled.len() > 1;
        let mut rows: Vec<Row> = self
            .enabled
            .iter()
            .map(|&system| {
                let select = cx.listener(move |this: &mut RootView, _: &ClickEvent, window, cx| {
                    this.select_system(system, window, cx)
                });
                let remove = cx.listener(move |_: &mut RootView, _: &ClickEvent, window, cx| {
                    details::confirm_remove_system(system, window, cx)
                });
                Row::System(SystemRow {
                    system,
                    count: self.counts.get(system.id).copied().unwrap_or(0),
                    flagged: self.flagged_in(system),
                    active: system.id == self.selected.id,
                    on_click: Rc::new(select),
                    on_remove: can_remove.then(|| Rc::new(remove) as Handler),
                })
            })
            .collect();
        let missing: Vec<&SystemDef> = systems::SYSTEMS.iter().filter(|s| !self.is_enabled(s)).collect();
        if !missing.is_empty() {
            let suggested = missing.iter().filter(|s| !self.waiting_for(s).is_empty()).count();
            rows.push(Row::Add(AddRow {
                available: missing.len(),
                suggested,
                on_click: Rc::new(cx.listener(|this: &mut RootView, _: &ClickEvent, window, cx| {
                    details::open_add_system(this, window, cx)
                })),
            }));
        }

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
