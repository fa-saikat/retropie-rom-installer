//! Root view. One flat state struct, one render fn — GPUI apps tend to
//! stay simplest when a screen this size isn't split into many entities.
//!
//! NOTE ON GPUI API STABILITY: GPUI is pre-1.0 and its element-building API
//! (div/flex/.bg/.child/etc.) shifts between Zed releases. This file follows
//! the patterns documented at https://www.gpui.rs and in zed-industries/zed
//! as of early 2026. If something here doesn't compile against the GPUI
//! version you pull in, the fastest fix is almost always `cargo doc --open
//! -p gpui` and diffing method names — the overall shape (Entity + Context +
//! Window + Render) has been stable for a long time even as helpers churn.

use gpui::prelude::FluentBuilder;
use gpui::*;
use std::path::PathBuf;

use crate::assets;
use crate::library::{self, GameEntry};
use crate::systems::{self, SystemDef};
use crate::theme;

pub struct RootView {
    selected: &'static SystemDef,
    games: Vec<GameEntry>,
    status: Option<String>,
    pending_delete: Option<GameEntry>,
    show_about: bool,
}

impl RootView {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let selected = &systems::SYSTEMS[0];
        let games = library::list_installed_games(selected).unwrap_or_default();
        cx.notify();
        Self {
            selected,
            games,
            status: None,
            pending_delete: None,
            show_about: false,
        }
    }

    fn select_system(&mut self, system: &'static SystemDef, cx: &mut Context<Self>) {
        self.selected = system;
        self.refresh(cx);
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.games = library::list_installed_games(self.selected).unwrap_or_default();
        cx.notify();
    }

    fn count_for(&self, system: &SystemDef) -> usize {
        library::list_installed_games(system)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    /// Open the native "Add ROM" file picker off the UI thread, then hop
    /// back onto it to actually install + refresh once the user picks a
    /// file (or cancels).
    fn pick_and_install(&mut self, cx: &mut Context<Self>) {
        let system = self.selected;
        let extensions: Vec<String> = system.extensions.iter().map(|e| e.trim_start_matches('.').to_string()).collect();

        cx.spawn(async move |this, cx| {
            let picked: Option<PathBuf> = cx
                .background_spawn(async move {
                    let ext_refs: Vec<&str> = extensions.iter().map(String::as_str).collect();
                    rfd::FileDialog::new()
                        .set_title("Select a ROM file")
                        .add_filter(system.display_name, &ext_refs)
                        .add_filter("All files", &["*"])
                        .pick_file()
                })
                .await;

            let Some(path) = picked else { return };

            let result = cx
                .background_spawn({
                    let path = path.clone();
                    async move { library::install_file(&path, system) }
                })
                .await;

            this.update(cx, |this, cx| {
                this.status = Some(match result {
                    Ok(detail) => format!("{}: {detail}", path.display()),
                    Err(err) => format!("Failed to install {}: {err}", path.display()),
                });
                this.refresh(cx);
            })
            .ok();
        })
        .detach();
    }

    fn request_delete(&mut self, entry: GameEntry, cx: &mut Context<Self>) {
        self.pending_delete = Some(entry);
        cx.notify();
    }

    fn cancel_delete(&mut self, cx: &mut Context<Self>) {
        self.pending_delete = None;
        cx.notify();
    }

    fn open_about(&mut self, cx: &mut Context<Self>) {
    self.show_about = true;
    cx.notify();
    }

    fn close_about(&mut self, cx: &mut Context<Self>) {
        self.show_about = false;
        cx.notify();
    }

    fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        if let Some(entry) = self.pending_delete.take() {
            self.status = Some(match library::uninstall_game(&entry) {
                Ok(n) => format!("Removed {} ({n} file(s))", entry.name),
                Err(err) => format!("Failed to remove {}: {err}", entry.name),
            });
            self.refresh(cx);
        }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Solid color is always applied as the base layer — it's the
        // fallback if no background asset is found, and it's also what
        // shows through before/around the image (e.g. while it loads).
        div()
            .relative()
            .size_full()
            .bg(rgb(theme::WINDOW_BG))
            .flex()
            .flex_row()
            .font_family("sans-serif")
            .when_some(assets::background_image(), |el, bg_path| {
                el.child(
                    div()
                        .absolute()
                        .inset_0()
                        .child(img(bg_path).size_full().object_fit(ObjectFit::Cover)),
                )
            })
            .child(self.render_sidebar(cx))
            .child(self.render_main(cx))
            .when_some(self.pending_delete.clone(), |el, entry| {
                el.child(self.render_confirm_dialog(entry, cx))
            })
            .when(self.show_about, |el| el.child(self.render_about_dialog(cx)))
    }
}

impl RootView {
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .w(px(200.))
            .flex_shrink_0()
            .h_full()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(2.))
            // Sits on top of the window background image/color so sidebar
            // text stays legible regardless of what's behind it.
            .bg(rgba(0x0f1419cc))
            .child(
                div()
                    .text_size(px(15.))
                    .text_color(rgb(theme::TEXT_PRIMARY))
                    .pb(px(16.))
                    .child("ROM manager"),
            )
            .children(systems::SYSTEMS.iter().map(|system| {
                let is_selected = system.id == self.selected.id;
                let count = self.count_for(system);
                let icon_path = assets::system_icon(system.id, system.icon);
                div()
                    .id(SharedString::from(system.id))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .when(is_selected, |el| el.bg(rgb(theme::SIDEBAR_ITEM_SELECTED_BG)))
                    .child(
                        div()
                            .size(px(18.))
                            .flex_shrink_0()
                            .rounded(px(4.))
                            .when(icon_path.is_none(), |el| el.bg(rgb(system.accent)))
                            .when_some(icon_path, |el, path| el.child(img(path).size_full())),
                    )
                    .text_size(px(13.))
                    .text_color(rgb(if is_selected {
                        theme::TEXT_PRIMARY
                    } else {
                        theme::SIDEBAR_TEXT
                    }))
                    .child(system.display_name)
                    .child(
                        div()
                            .ml_auto()
                            .text_size(px(11.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(count.to_string()),
                    )
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.select_system(system, cx);
                    }))
            }))
            .child(
                div()
                .id("about-button")
                .mt_auto()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .px(px(10.))
                .py(px(8.))
                .rounded(px(8.))
                .cursor_pointer()
                .text_size(px(13.))
                .text_color(rgb(theme::SIDEBAR_TEXT))
                .child("About")
                .on_click(cx.listener(|this, _event, _window, cx| {
                    this.open_about(cx);
                })),
            )
    }

    fn render_main(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .flex_1()
            .h_full()
            .p(px(20.))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pb(px(14.))
                    .child(
                        div().flex().flex_col().child(
                            div()
                                .text_size(px(16.))
                                .text_color(rgb(theme::TEXT_PRIMARY))
                                .child(self.selected.display_name),
                        ).child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(theme::TEXT_MUTED))
                                .child(format!("{} game(s) installed", self.games.len())),
                        ),
                    ),
            )
            .child(self.render_dropzone(cx))
            .child(self.render_grid(cx))
            .when_some(self.status.clone(), |el, status| {
                el.child(
                    div()
                        .mt(px(14.))
                        .text_size(px(12.))
                        .text_color(rgb(theme::TEXT_SECONDARY))
                        .child(status),
                )
            })
    }

    fn render_dropzone(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("dropzone")
            .border_1()
            .border_color(rgb(theme::DROPZONE_BORDER))
            .rounded(px(10.))
            .p(px(14.))
            .mb(px(16.))
            .flex()
            .items_center()
            .gap(px(12.))
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(theme::TEXT_PRIMARY))
                            .child("Add a ROM"),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(self.selected.extensions.join(" · ")),
                    ),
            )
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.pick_and_install(cx);
            }))
    }

    fn render_grid(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.games.is_empty() {
            return div()
                .text_size(px(12.))
                .text_color(rgb(theme::TEXT_MUTED))
                .child("No ROMs installed for this system yet.")
                .into_any_element();
        }

        div()
            .flex()
            .flex_wrap()
            .gap(px(10.))
            .children(self.games.iter().cloned().map(|entry| self.render_card(entry, cx)))
            .into_any_element()
    }

    fn render_card(&self, entry: GameEntry, cx: &mut Context<Self>) -> impl IntoElement {
        let file_count = entry.files.len();
        let name_for_click = entry.clone();
        div()
            .w(px(200.))
            .bg(rgb(theme::CARD_BG))
            .border_1()
            .border_color(rgb(theme::CARD_BORDER))
            .rounded(px(10.))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_start()
                    .child(
                        div()
                            .size(px(30.))
                            .rounded(px(6.))
                            .bg(rgb(theme::CARD_ICON_BG)),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("delete-{}", entry.name)))
                            .text_size(px(13.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .cursor_pointer()
                            .child("Delete")
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                this.request_delete(name_for_click.clone(), cx);
                            })),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(rgb(theme::TEXT_PRIMARY))
                    .child(entry.name.clone()),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(format!("{file_count} file(s)")),
            )
    }

    fn render_about_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(0x000000aa))
        .child(
            div()
            .w(px(340.))
            .bg(rgb(theme::CARD_BG))
            .border_1()
            .border_color(rgb(theme::CARD_BORDER))
            .rounded(px(12.))
            .p(px(24.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.))
            .child(
                div()
                .size(px(56.))
                .rounded(px(12.))
                .bg(rgb(theme::ACCENT))
                .mb(px(10.)),
            )
            .child(
                div()
                .text_size(px(15.))
                .text_color(rgb(theme::TEXT_PRIMARY))
                .child("ROM manager"),
            )
            .child(
                div()
                .text_size(px(12.))
                .text_color(rgb(theme::TEXT_MUTED))
                .child(env!("CARGO_PKG_VERSION")),
            )
            .child(
                div()
                .text_size(px(12.))
                .text_color(rgb(theme::TEXT_SECONDARY))
                .mt(px(10.))
                .child("A fast, simple ROM manager for RetroPie."),
            )
            .child(
                div()
                .text_size(px(11.))
                .text_color(rgb(theme::TEXT_MUTED))
                .mt(px(14.))
                .child("Copyright © 2026 Saikat"),
            )
            .child(
                div()
                .id("close-about")
                .mt(px(16.))
                .px(px(20.))
                .py(px(6.))
                .rounded(px(8.))
                .bg(rgb(theme::ACCENT))
                .text_size(px(13.))
                .text_color(rgb(0xffffff))
                .cursor_pointer()
                .child("Close")
                .on_click(cx.listener(|this, _event, _window, cx| {
                    this.close_about(cx);
                })),
            ),
        )
    }

    fn render_confirm_dialog(&self, entry: GameEntry, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x000000aa))
            .child(
                div()
                    .w(px(360.))
                    .bg(rgb(theme::CARD_BG))
                    .border_1()
                    .border_color(rgb(theme::CARD_BORDER))
                    .rounded(px(12.))
                    .p(px(20.))
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(rgb(theme::TEXT_PRIMARY))
                            .child(format!("Delete \"{}\"?", entry.name)),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(theme::TEXT_SECONDARY))
                            .child(format!(
                                "This deletes {} file(s) from {}. This can't be undone.",
                                entry.files.len(),
                                self.selected.display_name
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(10.))
                            .child(
                                div()
                                    .id("cancel-delete")
                                    .px(px(16.))
                                    .py(px(6.))
                                    .rounded(px(8.))
                                    .border_1()
                                    .border_color(rgb(theme::CARD_BORDER))
                                    .text_size(px(13.))
                                    .text_color(rgb(theme::TEXT_SECONDARY))
                                    .cursor_pointer()
                                    .child("Cancel")
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.cancel_delete(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("confirm-delete")
                                    .px(px(16.))
                                    .py(px(6.))
                                    .rounded(px(8.))
                                    .bg(rgb(theme::DANGER))
                                    .text_size(px(13.))
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .child("Delete")
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.confirm_delete(cx);
                                    })),
                            ),
                    ),
            )
    }
}
