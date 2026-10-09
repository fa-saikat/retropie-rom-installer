//! Game details sheet, the delete / wrong-system confirmations, the
//! add / remove emulator dialogs and the About dialog.
//!
//! GPUI Kit re-runs a sheet/dialog builder on every render, so each one
//! works from a snapshot taken when it opens and talks back to `RootView`
//! through a weak handle.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_kit::component::description_list::DescriptionList;
use gpui_kit::component::dialog::AlertDialog;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::rating::Rating;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Disableable, Icon, Sizable, WindowExt, StyledExt};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::path::PathBuf;

use super::root::RootView;
use super::sidebar::{system_glyph, system_tile};
use crate::assets;
use crate::detect;
use crate::library::{self, GameEntry};
use crate::scraper::{self, GameMeta, MediaState, ScrapeStatus};
use crate::systems::{SystemDef, MAKERS, SYSTEMS};
use crate::theme;

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
    /// The system this game's contents say it's really for.
    misplaced: Option<&'static SystemDef>,
    /// ...and it isn't in the sidebar yet, so moving adds it.
    move_adds: bool,
    /// RetroPie has an emulator for this system.
    can_run: bool,
    /// Another game is already running.
    running: bool,
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
        // The background audit may not have reached this game yet; one
        // file is cheap enough to check here.
        misplaced: view.misplaced_as(entry).or_else(|| {
            let file = detect::representative(view.selected, entry)?;
            detect::detect(&file).filter(|found| found.id != view.selected.id)
        }),
        move_adds: false,
        can_run: view.can_run(),
        running: view.running.is_some(),
    };
    let snapshot = Snapshot { move_adds: snapshot.misplaced.is_some_and(|to| !view.is_enabled(to)), ..snapshot };
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

    let misplaced = s.misplaced.map(|other| {
        gpui_kit::component::alert::Alert::warning(
            "misplaced",
            format!(
                "It's in ~/RetroPie/roms/{}, so EmulationStation will try to start it with the {} emulator and it won't run. Move it to {}{}, or delete it.",
                s.system.folder,
                s.system.display_name,
                other.display_name,
                if s.move_adds { " (it'll be added to your list)" } else { "" },
            ),
        )
        .title(format!("This looks like a {} game", other.display_name))
    });

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
        .children(misplaced)
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
    let move_entry = s.entry.clone();
    let play_entry = s.entry.clone();
    let (rescrape, delete, relocate, play) = (this.clone(), this.clone(), this.clone(), this);
    let play_tooltip = if !s.can_run {
        format!("RetroPie has no emulator set up for {}", s.system.display_name)
    } else if s.running {
        "Another game is running".to_string()
    } else {
        "Play without opening EmulationStation".to_string()
    };
    h_flex()
        .w_full()
        .gap_2()
        .child(
            Button::new("sheet-folder")
                .ghost()
                .small()
                .icon(Icon::new(IconName::FolderOpen))
                .tooltip("Show in folder")
                .on_click(move |_, _, cx| {
                    let _ = std::fs::create_dir_all(&folder);
                    cx.open_with_system(&folder);
                }),
        )
        .child(div().flex_1())
        // Scraping as the wrong system would only fetch the wrong game.
        .when(s.misplaced.is_none(), |footer| footer.child(
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
        ))
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
        .map(|footer| match s.misplaced {
            // It won't run here, so moving it is the main action.
            Some(to) => footer.child(
                Button::new("sheet-move")
                    .primary()
                    .small()
                    .icon(Icon::new(IconName::FolderInput))
                    .label(if s.move_adds { format!("Add {} & move", to.short_name) } else { format!("Move to {}", to.short_name) })
                    .on_click(move |_, window, cx| {
                        relocate.update(cx, |view, cx| view.move_game(move_entry.clone(), to, window, cx)).ok();
                    }),
            ),
            None => footer.child(
                Button::new("sheet-play")
                    .primary()
                    .small()
                    .icon(Icon::new(IconName::Play))
                    .label("Play")
                    .tooltip(play_tooltip.clone())
                    .disabled(!s.can_run || s.running)
                    .on_click(move |_, window, cx| {
                        play.update(cx, |view, cx| view.run_game(&play_entry, window, cx)).ok();
                    }),
            ),
        })
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

fn file_name(path: &std::path::Path) -> String {
    path.file_name().unwrap_or_default().to_string_lossy().to_string()
}

/// One "thing → system" line, shared by the wrong-system dialogs. `adds`
/// marks a system that isn't in the sidebar yet.
fn destination_row(label: String, to: &'static SystemDef, adds: bool, cx: &App) -> Div {
    let t = cx.theme();
    h_flex()
        .gap_2()
        .px_2p5()
        .py_1p5()
        .border_b_1()
        .border_color(t.border)
        .text_xs()
        .child(div().flex_1().min_w_0().truncate().font_family(t.mono_font_family.clone()).child(label))
        .child(Icon::new(IconName::ArrowRight).xsmall().text_color(t.muted_foreground))
        .child(
            h_flex()
                .gap_1()
                .flex_shrink_0()
                .font_medium()
                .child(system_glyph(to, px(11.), t.foreground))
                .child(to.short_name),
        )
        .when(adds, |row| row.child(Tag::info().small().child("new")))
}

/// "Game Boy and SNES will be added to your list." for systems not listed.
fn added_note(new: &[&'static SystemDef]) -> Option<String> {
    let names: Vec<&str> = new.iter().map(|s| s.display_name).collect();
    let list = match names.as_slice() {
        [] => return None,
        [one] => one.to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    };
    Some(format!(" {list} will be added to your list."))
}

fn destination_list(rows: impl IntoIterator<Item = Div>, cx: &App) -> Div {
    let t = cx.theme();
    v_flex()
        .max_h(px(180.))
        .overflow_hidden()
        .rounded(t.radius)
        .border_1()
        .border_color(t.border)
        .children(rows)
}

/// Some of the files being installed into `here` are, by their contents,
/// games for other systems. Offer to put them where they belong.
pub(super) fn confirm_wrong_system(
    view: &mut RootView,
    here: &'static SystemDef,
    files: Vec<(PathBuf, &'static SystemDef)>,
    window: &mut Window,
    cx: &mut Context<RootView>,
) {
    let this = cx.entity().downgrade();
    let n = files.len();
    let title = match files.as_slice() {
        [(path, other)] => format!("{} looks like a {} game", file_name(path), other.display_name),
        _ => format!("{n} files look like games for other systems"),
    };
    let mut targets: Vec<&'static SystemDef> = Vec::new();
    for (_, to) in &files {
        if !targets.iter().any(|t| t.id == to.id) {
            targets.push(to);
        }
    }
    let new: Vec<&'static SystemDef> = targets.iter().copied().filter(|t| !view.is_enabled(t)).collect();
    let go_label = match targets.as_slice() {
        [only] if new.is_empty() => format!("Install to {}", only.display_name),
        [only] => format!("Add {} & install", only.display_name),
        _ => "Install where they belong".to_string(),
    };
    let note = format!(
        "You're adding to {}. EmulationStation would start {} with the {} emulator, and {} won't run there.{}",
        here.display_name,
        if n == 1 { "it" } else { "them" },
        here.display_name,
        if n == 1 { "it" } else { "they" },
        added_note(&new).unwrap_or_default(),
    );
    // "Anyway" only makes sense if this system takes at least one of them.
    let can_stay = files
        .iter()
        .any(|(p, _)| here.extensions.contains(&library::ext_lower(p).as_str()));
    window.open_dialog(cx, move |dialog, _, cx| {
        let t = cx.theme();
        let (go, stay) = (this.clone(), this.clone());
        let go_files = files.clone();
        let stay_files: Vec<(PathBuf, &'static SystemDef)> = files.iter().map(|(p, _)| (p.clone(), here)).collect();
        let rows = files.iter().map(|(p, to)| {
            destination_row(file_name(p), to, new.iter().any(|s| s.id == to.id), cx)
        });
        dialog
            .title(title.clone())
            .w(px(480.))
            .child(
                v_flex()
                    .gap_3()
                    .child(div().text_sm().text_color(t.muted_foreground).child(note.clone()))
                    .child(destination_list(rows, cx)),
            )
            .footer(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(Button::new("wrong-skip").ghost().label("Skip").on_click(|_, window, cx| window.close_dialog(cx)))
                    .child(div().flex_1())
                    .when(can_stay, |footer| {
                        footer.child(
                            Button::new("wrong-stay")
                                .outline()
                                .label(format!("Add to {} anyway", here.short_name))
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    stay.update(cx, |view, cx| view.install_into(stay_files.clone(), window, cx)).ok();
                                }),
                        )
                    })
                    .child(Button::new("wrong-go").primary().label(go_label.clone()).on_click(move |_, window, cx| {
                        window.close_dialog(cx);
                        go.update(cx, |view, cx| view.install_elsewhere(go_files.clone(), window, cx)).ok();
                    })),
            )
    });
}

/// Confirm moving every flagged game in the selected system.
pub(super) fn confirm_move_misplaced(view: &mut RootView, window: &mut Window, cx: &mut Context<RootView>) {
    let this = cx.entity().downgrade();
    let games: Vec<(String, &'static SystemDef, bool)> = view
        .misplaced_games()
        .into_iter()
        .map(|m| (m.entry.title_and_tags().0, m.to, !view.is_enabled(m.to)))
        .collect();
    let mut new: Vec<&'static SystemDef> = Vec::new();
    for (_, to, adds) in &games {
        if *adds && !new.iter().any(|s| s.id == to.id) {
            new.push(to);
        }
    }
    let description = format!(
        "Each one goes from ~/RetroPie/roms/{} to the folder for the system it was made for. Artwork scraped for the wrong system is removed.{}",
        view.selected.folder,
        added_note(&new).unwrap_or_default(),
    );
    window.open_alert_dialog(cx, move |alert: AlertDialog, _, cx| {
        let n = games.len();
        let this = this.clone();
        alert
            .confirm()
            .title(format!("Move {n} {}?", if n == 1 { "game" } else { "games" }))
            .description(description.clone())
            .child(destination_list(
                games.iter().map(|(title, to, adds)| destination_row(title.clone(), to, *adds, cx)),
                cx,
            ))
            .ok_text("Move games")
            .cancel_text("Cancel")
            .on_ok(move |_, window, cx| {
                this.update(cx, |view, cx| view.move_all_misplaced(window, cx)).ok();
                true
            })
    });
}

/// One system the "Add emulator" picker offers.
#[derive(Clone)]
struct Choice {
    system: &'static SystemDef,
    photo: Option<PathBuf>,
    /// Flagged games elsewhere that belong to it.
    waiting: usize,
    /// RetroPie has an emulator set up for it.
    installed: bool,
}

impl Choice {
    fn matches(&self, query: &str) -> bool {
        let s = self.system;
        query.is_empty()
            || [s.display_name, s.short_name, s.maker, s.id]
                .iter()
                .any(|field| field.to_lowercase().contains(query))
    }
}

/// The flat grey the system photos were composited onto, so the photo
/// well and the image read as one surface in both themes.
const PHOTO_WELL: u32 = 0xECECEF;

/// Searchable grid of photo cards for every system not in the sidebar,
/// grouped by maker, with the ones that have games waiting for them first.
pub(super) fn open_add_system(view: &mut RootView, window: &mut Window, cx: &mut Context<RootView>) {
    let this = cx.entity().downgrade();
    let choices: Vec<Choice> = SYSTEMS
        .iter()
        .filter(|s| !view.is_enabled(s))
        .map(|system| Choice {
            system,
            photo: assets::system_photo(system),
            waiting: view.waiting_for(system).len(),
            installed: view.runnable.contains(system.id),
        })
        .collect();
    let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search systems"));
    search.update(cx, |s, cx| s.focus(window, cx));
    window.open_dialog(cx, move |dialog, _, cx| {
        let t = cx.theme();
        let query = search.read(cx).value().trim().to_lowercase();
        let visible: Vec<&Choice> = choices.iter().filter(|c| c.matches(&query)).collect();

        let mut groups: Vec<(String, Vec<&Choice>)> = Vec::new();
        let suggested: Vec<&Choice> = visible.iter().copied().filter(|c| c.waiting > 0).collect();
        if !suggested.is_empty() {
            groups.push(("Suggested for games you already have".into(), suggested));
        }
        for maker in MAKERS {
            let items: Vec<&Choice> =
                visible.iter().copied().filter(|c| c.system.maker == *maker && c.waiting == 0).collect();
            if !items.is_empty() {
                groups.push((maker.to_string(), items));
            }
        }

        let body = if choices.is_empty() {
            empty_note("Every system the app knows is already in your list.", cx)
        } else if visible.is_empty() {
            empty_note(&format!("No system matches “{query}”."), cx)
        } else {
            v_flex().gap_6().pb_1().children(groups.into_iter().map(|(heading, items)| {
                let count = items.len();
                v_flex()
                    .gap_2p5()
                    .child(
                        h_flex()
                            .gap_2()
                            .text_xs()
                            .font_semibold()
                            .text_color(t.muted_foreground)
                            .child(heading)
                            .child(div().font_normal().opacity(0.7).child(count.to_string())),
                    )
                    .child(
                        div()
                            .grid()
                            .grid_cols(3)
                            .gap_3()
                            .children(items.into_iter().map(|choice| choice_card(choice, this.clone(), cx))),
                    )
            }))
        };

        dialog
            .title("Add an emulator")
            .w(px(760.))
            .child(
                v_flex()
                    .gap_4()
                    .child(
                        h_flex()
                            .gap_4()
                            .items_center()
                            .child(div().flex_1().min_w_0().text_sm().text_color(t.muted_foreground).child(
                                "Pick a system to add it to your list.",
                            ))
                            .child(
                                div().w(px(220.)).flex_shrink_0().child(
                                    Input::new(&search)
                                        .prefix(Icon::new(IconName::Search).small().text_color(t.muted_foreground))
                                        .cleanable(true),
                                ),
                            ),
                    )
                    .child(div().id("add-system-scroll").h(px(500.)).overflow_y_scroll().pr_1().child(body))
                    .child(
                        div()
                            .text_xs()
                            .text_color(t.muted_foreground.opacity(0.8))
                            .child("Photos: Wikimedia Commons, mostly by Evan-Amos (public domain). Full credits in assets/systems/CREDITS.md."),
                    ),
            )
    });
}

fn empty_note(text: &str, cx: &App) -> Div {
    div()
        .py_10()
        .flex()
        .justify_center()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
}

fn choice_card(choice: &Choice, this: WeakEntity<RootView>, cx: &App) -> impl IntoElement {
    let t = cx.theme();
    let system = choice.system;
    let accent = theme::accent(system.accent);
    let group = SharedString::from(format!("choice-{}", system.id));

    let photo = div()
        .relative()
        .h(px(128.))
        .bg(rgb(PHOTO_WELL))
        .flex()
        .items_center()
        .justify_center()
        .map(|well| match &choice.photo {
            Some(path) => well.child(img(path.clone()).size_full().object_fit(ObjectFit::Contain)),
            None => well.child(system_tile(system, px(56.), px(28.), px(14.))),
        })
        .when(choice.waiting > 0, |well| {
            well.child(
                div().absolute().left(px(8.)).top(px(8.)).child(
                    Tag::danger()
                        .small()
                        .child(format!("{} {} waiting", choice.waiting, if choice.waiting == 1 { "game" } else { "games" })),
                ),
            )
        })
        // "Add" pill fades in on hover, in the system's own colour.
        .child(
            h_flex()
                .absolute()
                .right(px(8.))
                .top(px(8.))
                .gap_1()
                .px_2()
                .py(px(3.))
                .rounded_full()
                .bg(accent)
                .text_xs()
                .font_medium()
                .text_color(white())
                .invisible()
                .group_hover(group.clone(), |s| s.visible())
                .child(Icon::new(IconName::Plus).xsmall())
                .child("Add"),
        );

    let (dot, status) = if choice.installed {
        (t.success, "Emulator installed")
    } else {
        (t.muted_foreground.opacity(0.5), "Emulator not installed yet")
    };
    let caption = h_flex()
        .gap_2p5()
        .px_3()
        .py_2p5()
        .child(system_tile(system, px(26.), px(14.), px(7.)))
        .child(
            v_flex()
                .min_w_0()
                .child(div().text_sm().font_semibold().truncate().child(system.display_name))
                .child(
                    h_flex()
                        .gap_1p5()
                        .text_xs()
                        .text_color(t.muted_foreground)
                        .child(div().size(px(6.)).flex_shrink_0().rounded_full().bg(dot))
                        .child(div().truncate().child(status)),
                ),
        );

    v_flex()
        .id(SharedString::from(format!("add-{}", system.id)))
        .group(group)
        .rounded(t.radius_lg)
        .overflow_hidden()
        .border_1()
        .border_color(if choice.waiting > 0 { t.danger.opacity(0.6) } else { t.border })
        .bg(theme::card_bg(cx))
        .cursor_pointer()
        .hover(|s| s.border_color(accent))
        .child(photo)
        .child(caption)
        .on_click(move |_, window, cx| {
            window.close_dialog(cx);
            this.update(cx, |view, cx| view.add_system(system, window, cx)).ok();
        })
}

/// Removing a system deletes its games, so say exactly what goes.
pub(super) fn confirm_remove_system(
    system: &'static SystemDef,
    window: &mut Window,
    cx: &mut Context<RootView>,
) {
    let this = cx.entity().downgrade();
    let games = library::list_installed_games(system).unwrap_or_default();
    let n = games.len();
    let files: usize = games.iter().map(|g| g.files.len()).sum();
    let bytes: u64 = games.iter().flat_map(|g| g.file_sizes()).sum();
    window.open_alert_dialog(cx, move |alert: AlertDialog, _, _| {
        let this = this.clone();
        let description = if n == 0 {
            "It has no games, so nothing is deleted. You can add it back from Add emulator any time.".to_string()
        } else {
            format!(
                "This permanently deletes its {n} {} ({files} {}, {}) from ~/RetroPie/roms/{}, along with {} scraped artwork. This can't be undone. RetroPie's emulator itself stays installed.",
                if n == 1 { "game" } else { "games" },
                if files == 1 { "file" } else { "files" },
                library::format_size(bytes),
                system.folder,
                if n == 1 { "its" } else { "their" },
            )
        };
        alert
            .confirm()
            .title(format!("Remove {}?", system.display_name))
            .description(description)
            .ok_text(if n == 0 { "Remove".to_string() } else { format!("Delete {n} {} & remove", if n == 1 { "game" } else { "games" }) })
            .ok_variant(if n == 0 { ButtonVariant::Primary } else { ButtonVariant::Danger })
            .cancel_text("Cancel")
            .on_ok(move |_, window, cx| {
                this.update(cx, |view, cx| view.remove_system(system, window, cx)).ok();
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

