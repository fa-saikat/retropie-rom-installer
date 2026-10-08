//! Game details sheet, the delete confirmation and the About dialog.
//!
//! GPUI Kit re-runs a sheet/dialog builder on every render, so each one
//! works from a snapshot taken when it opens and talks back to `RootView`
//! through a weak handle.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_kit::component::description_list::DescriptionList;
use gpui_kit::component::dialog::AlertDialog;
use gpui_kit::component::rating::Rating;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Disableable, Icon, Sizable, WindowExt, StyledExt};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use super::root::RootView;
use super::sidebar::system_glyph;
use crate::library::{self, GameEntry};
use crate::scraper::{self, GameMeta, MediaState, ScrapeStatus};
use crate::systems::SystemDef;

/// Owned copy of everything the sheet shows.
#[derive(Clone)]
struct Snapshot {
    entry: GameEntry,
    system: &'static SystemDef,
    meta: Option<GameMeta>,
    no_match: bool,
    sizes: Vec<u64>,
    media: Vec<scraper::MediaSlot>,
    scrape_source: String,
    can_scrape: bool,
}

pub(super) fn open_details(view: &mut RootView, entry: &GameEntry, window: &mut Window, cx: &mut Context<RootView>) {
    let status = view.status_of(entry);
    let meta = match status {
        ScrapeStatus::Scraped(m) => Some(m.clone()),
        _ => None,
    };
    let snapshot = Snapshot {
        entry: entry.clone(),
        system: view.selected,
        media: meta.as_ref().map(|m| scraper::media_slots(m, &view.settings)).unwrap_or_default(),
        meta,
        no_match: status == ScrapeStatus::NoMatch,
        sizes: entry.file_sizes(),
        scrape_source: view.settings.scrape_source.clone(),
        can_scrape: view.skyscraper_installed && view.scrape.is_none(),
    };
    let this = cx.entity().downgrade();
    window.open_sheet(cx, move |sheet, _, cx| {
        sheet
            .size(px(440.))
            .child(render_body(&snapshot, cx))
            .footer(render_footer(&snapshot, this.clone()))
    });
}

fn section(label: &'static str, aside: Option<String>, body: impl IntoElement, cx: &App) -> impl IntoElement {
    let t = cx.theme();
    v_flex()
        .gap_2()
        .child(
            h_flex()
                .justify_between()
                .text_xs()
                .child(div().font_semibold().text_color(t.muted_foreground).child(label))
                .children(aside.map(|a| div().text_color(t.muted_foreground.opacity(0.8)).child(a))),
        )
        .child(body)
}

fn render_body(s: &Snapshot, cx: &App) -> impl IntoElement {
    let t = cx.theme();
    let (title, tags) = s.entry.title_and_tags();
    let meta = s.meta.as_ref();
    let screenshot = meta.and_then(|m| m.screenshot.clone()).filter(|p| p.is_file());
    let cover = meta.and_then(|m| m.cover.clone()).filter(|p| p.is_file());
    let has_banner = screenshot.is_some();

    let banner = screenshot.map(|path| {
        div()
            .h(px(180.))
            .rounded(t.radius_lg)
            .overflow_hidden()
            .bg(t.muted)
            .child(img(path).size_full().object_fit(ObjectFit::Cover))
    });

    let box_art = div()
        .w(px(96.))
        .h(px(128.))
        .flex_shrink_0()
        .rounded(t.radius)
        .overflow_hidden()
        .border_3()
        .border_color(t.background)
        .bg(t.muted)
        .shadow_md()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(30.))
        .font_bold()
        .text_color(t.muted_foreground)
        .map(|b| match cover {
            Some(path) => b.child(img(path).size_full().object_fit(ObjectFit::Cover)),
            None => b.child(s.entry.monogram()),
        });

    let byline = match meta {
        Some(m) => [m.developer.clone(), m.year.clone()].into_iter().flatten().collect::<Vec<_>>().join(" · "),
        None if s.no_match => "No match on ScreenScraper".into(),
        None => "Not scraped yet".into(),
    };

    let head = h_flex()
        .gap_4()
        .items_end()
        .when(has_banner, |h| h.mt(px(-56.)).pl_3())
        .child(box_art)
        .child(
            v_flex()
                .min_w_0()
                .pb_1()
                .gap_1()
                .child(div().text_xl().font_semibold().line_height(relative(1.2)).child(title))
                .child(div().text_sm().text_color(t.muted_foreground).child(byline))
                .child(h_flex().gap_1().children(tags.into_iter().map(|tag| Tag::secondary().small().child(tag)))),
        );

    let about = match meta {
        Some(m) => {
            let desc = match &m.desc {
                Some(d) => div().text_sm().line_height(relative(1.6)).text_color(t.muted_foreground).child(d.clone()),
                None => div().text_sm().italic().text_color(t.muted_foreground).child("No description in gamelist.xml."),
            };
            let mut details = DescriptionList::new().columns(1).label_width(px(96.)).bordered(false);
            for (label, value) in [
                ("Released", m.year.clone()),
                ("Developer", m.developer.clone()),
                ("Publisher", m.publisher.clone()),
                ("Genre", m.genre.clone()),
                ("Players", m.players.clone().map(|p| p.replace('-', "–"))),
            ] {
                if let Some(value) = value {
                    details = details.item(label, value, 1);
                }
            }
            if let Some(rating) = m.rating {
                details = details.item(
                    "Rating",
                    h_flex()
                        .gap_2()
                        .child(
                            Rating::new("detail-rating")
                                .value((rating * 5.0).round() as usize)
                                .small()
                                .disabled(true),
                        )
                        .child(div().text_color(t.muted_foreground).child(format!("{:.1} / 5", rating * 5.0)))
                        .into_any_element(),
                    1,
                );
            }
            v_flex()
                .gap_5()
                .child(desc)
                .child(section("Details", Some(pretty_source(&s.scrape_source)), details, cx))
                .child(section(
                    "Media",
                    Some(format!(
                        "{} of {} saved",
                        s.media.iter().filter(|m| matches!(m.state, MediaState::Saved(_))).count(),
                        s.media.len()
                    )),
                    render_media(s, cx),
                    cx,
                ))
                .into_any_element()
        }
        None => {
            let (heading, body) = if s.no_match {
                ("No match on ScreenScraper.", "Renaming the file to its No-Intro/Redump name usually fixes this.")
            } else {
                ("No artwork yet.", "Scrape to fetch box art, screenshots and a description.")
            };
            gpui_kit::component::alert::Alert::info("no-meta", body)
                .title(heading)
                .into_any_element()
        }
    };

    let total: u64 = s.sizes.iter().sum();
    let files = v_flex()
        .rounded(t.radius)
        .border_1()
        .border_color(t.border)
        .overflow_hidden()
        .text_xs()
        .children(s.entry.files.iter().zip(&s.sizes).map(|(file, size)| {
            h_flex()
                .gap_2()
                .px_2p5()
                .py_1p5()
                .border_b_1()
                .border_color(t.border)
                .child(Icon::new(IconName::File).xsmall().text_color(t.muted_foreground))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_family(t.mono_font_family.clone())
                        .child(file.file_name().unwrap_or_default().to_string_lossy().to_string()),
                )
                .child(div().text_color(t.muted_foreground).child(library::format_size(*size)))
        }))
        .child(
            h_flex()
                .gap_2()
                .px_2p5()
                .py_1p5()
                .bg(t.muted.opacity(0.5))
                .text_color(t.muted_foreground)
                .child(Icon::new(IconName::Folder).xsmall())
                .child(
                    div()
                        .font_family(t.mono_font_family.clone())
                        .child(format!("~/RetroPie/roms/{}/", s.system.folder)),
                ),
        );

    v_flex()
        .gap_5()
        .pb_4()
        .children(banner)
        .child(head)
        .child(about)
        .child(section(
            "Files",
            Some(format!(
                "{} {} · {}",
                s.entry.files.len(),
                if s.entry.files.len() == 1 { "file" } else { "files" },
                library::format_size(total)
            )),
            files,
            cx,
        ))
}

fn pretty_source(source: &str) -> String {
    match source {
        "screenscraper" => "ScreenScraper".into(),
        "thegamesdb" => "TheGamesDB".into(),
        "arcadedb" => "ArcadeDB".into(),
        "mobygames" => "MobyGames".into(),
        other => other.to_string(),
    }
}

/// One tile per media type: the image if Skyscraper saved it, otherwise a
/// dashed box saying whether it's switched off or just wasn't found.
fn render_media(s: &Snapshot, cx: &App) -> impl IntoElement {
    let t = cx.theme();
    h_flex().gap_2().items_start().children(s.media.iter().map(|slot| {
        let (tile, note) = match &slot.state {
            MediaState::Saved(path) => (
                div()
                    .rounded(t.radius)
                    .overflow_hidden()
                    .border_1()
                    .border_color(t.border)
                    .bg(t.muted)
                    .child(img(path.clone()).size_full().object_fit(ObjectFit::Cover)),
                "Saved",
            ),
            other => (
                div()
                    .rounded(t.radius)
                    .border_1()
                    .border_dashed()
                    .border_color(t.border)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(if slot.label == "Video" { IconName::VideoOff } else { IconName::ImageOff })
                            .small()
                            .text_color(t.muted_foreground),
                    ),
                if *other == MediaState::Disabled { "Off in skyscraper.cfg" } else { "Not found" },
            ),
        };
        v_flex()
            .flex_1()
            .min_w_0()
            .gap_1()
            .child(tile.h(px(64.)).w_full())
            .child(div().text_xs().child(slot.label))
            .child(div().text_xs().text_color(t.muted_foreground).line_height(relative(1.2)).child(note))
    }))
}

fn render_footer(s: &Snapshot, this: WeakEntity<RootView>) -> impl IntoElement {
    let folder = library::roms_root().join(s.system.folder);
    let scrape_entry = s.entry.clone();
    let delete_entry = s.entry.clone();
    let (rescrape, delete) = (this.clone(), this);
    h_flex()
        .w_full()
        .gap_2()
        .child(
            Button::new("sheet-folder")
                .ghost()
                .small()
                .icon(Icon::new(IconName::FolderOpen))
                .label("Show in folder")
                .on_click(move |_, _, cx| {
                    let _ = std::fs::create_dir_all(&folder);
                    cx.open_with_system(&folder);
                }),
        )
        .child(div().flex_1())
        .child(
            Button::new("sheet-scrape")
                .outline()
                .small()
                .icon(Icon::new(if s.meta.is_some() { IconName::RefreshCw } else { IconName::ImageDown }))
                .label(if s.meta.is_some() { "Re-scrape" } else { "Scrape" })
                .disabled(!s.can_scrape)
                .on_click(move |_, window, cx| {
                    window.close_sheet(cx);
                    rescrape.update(cx, |view, cx| view.scrape_one(&scrape_entry, window, cx)).ok();
                }),
        )
        .child(
            Button::new("sheet-delete")
                .danger()
                .outline()
                .small()
                .icon(Icon::new(IconName::Trash))
                .label("Delete")
                .on_click(move |_, window, cx| {
                    delete
                        .update(cx, |view, cx| confirm_delete_dialog(view, delete_entry.clone(), window, cx))
                        .ok();
                }),
        )
}

pub(super) fn confirm_delete_dialog(
    view: &mut RootView,
    entry: GameEntry,
    window: &mut Window,
    cx: &mut Context<RootView>,
) {
    let this = cx.entity().downgrade();
    let folder = view.selected.folder;
    window.open_alert_dialog(cx, move |alert: AlertDialog, _, cx| {
        let t = cx.theme();
        let (title, _) = entry.title_and_tags();
        let n = entry.files.len();
        let files = v_flex()
            .max_h(px(140.))
            .overflow_hidden()
            .rounded(t.radius)
            .border_1()
            .border_color(t.border)
            .px_2p5()
            .py_1p5()
            .gap_0p5()
            .text_xs()
            .font_family(t.mono_font_family.clone())
            .text_color(t.muted_foreground)
            .children(entry.files.iter().take(6).map(|f| {
                div().truncate().child(f.file_name().unwrap_or_default().to_string_lossy().to_string())
            }))
            .when(n > 6, |list| list.child(format!("+ {} more", n - 6)));
        let this = this.clone();
        let entry = entry.clone();
        alert
            .confirm()
            .title(format!("Delete {title}?"))
            .description(format!(
                "{n} {} will be permanently removed from ~/RetroPie/roms/{folder}, along with its scraped artwork. This can't be undone.",
                if n == 1 { "file" } else { "files" }
            ))
            .child(files)
            .ok_text("Delete files")
            .ok_variant(ButtonVariant::Danger)
            .cancel_text("Cancel")
            .on_ok(move |_, window, cx| {
                this.update(cx, |view, cx| view.confirm_delete(entry.clone(), window, cx)).ok();
                true
            })
    });
}

pub(super) fn confirm_install_skyscraper(window: &mut Window, cx: &mut Context<RootView>) {
    let this = cx.entity().downgrade();
    window.open_alert_dialog(cx, move |alert: AlertDialog, _, _| {
        let this = this.clone();
        alert
            .confirm()
            .title("Install Skyscraper?")
            .description(
                "This runs RetroPie-Setup's Skyscraper package, the same as Manage packages → opt → skyscraper. \
                 It needs administrator rights, so you may be asked for your password, and it can take \
                 10–20 minutes on a Raspberry Pi while it builds. You can keep using the app meanwhile.",
            )
            .ok_text("Install")
            .cancel_text("Not now")
            .on_ok(move |_, window, cx| {
                this.update(cx, |view, cx| view.install_skyscraper(window, cx)).ok();
                true
            })
    });
}

pub(super) fn open_about(view: &mut RootView, window: &mut Window, cx: &mut Context<RootView>) {
    let scraper_line = if view.skyscraper_installed {
        format!("Skyscraper ({})", pretty_source(&view.settings.scrape_source))
    } else {
        "Skyscraper not installed".to_string()
    };
    let system = view.selected;
    window.open_dialog(cx, move |dialog, _, cx| {
        let t = cx.theme();
        dialog
            .title("RetroPie ROM Manager")
            .w(px(420.))
            .child(
                v_flex()
                    .gap_4()
                    .child(
                        h_flex()
                            .gap_3()
                            .child(
                                div()
                                    .size(px(40.))
                                    .flex_shrink_0()
                                    .rounded(px(10.))
                                    .bg(t.foreground)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(system_glyph(system, px(20.), t.background)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .text_color(t.muted_foreground)
                                    .child("Install, browse and remove RetroPie ROMs, with box art fetched by RetroPie's own scraper."),
                            ),
                    )
                    .child(
                        DescriptionList::new()
                            .columns(1)
                            .label_width(px(110.))
                            .item("Version", env!("CARGO_PKG_VERSION"), 1)
                            .item("ROMs folder", "~/RetroPie/roms", 1)
                            .item("Artwork", scraper_line.clone(), 1)
                            .item("Developer", "Fahim A Saikat", 1)
                            .item("Copyright", "© 2026 JaduPc", 1)
                            .item("License", "MIT", 1),
                    ),
            )
            .footer(
                h_flex().w_full().justify_end().child(
                    Button::new("about-close")
                        .label("Close")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ),
            )
    });
}

