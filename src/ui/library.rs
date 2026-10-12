//! Main column: system header, notices, toolbar and the game grid / list.

use gpui_kit::assets::IconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonGroup, ButtonVariants};
use gpui_kit::component::empty::{
    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle,
};
use gpui_kit::component::input::Input;
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::rating::Rating;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Disableable, Icon, Selectable, Sizable, StyledExt};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use super::details;
use super::root::{RootView, ViewMode};
use super::sidebar::{system_glyph, system_tile};
use crate::library::GameEntry;
use crate::scraper::ScrapeStatus;
use crate::systems::SystemDef;
use crate::theme;

const SIDEBAR_WIDTH: f32 = 240.;
const CONTENT_PADDING: f32 = 24.;
const CARD_MIN_WIDTH: f32 = 196.;
const GRID_GAP: f32 = 14.;
const ART_HEIGHT: f32 = 118.;

/// Rounded 0–1 rating to whole stars, as the kit's `Rating` draws them.
fn stars(rating: f32) -> usize {
    (rating * 5.0).round().clamp(0.0, 5.0) as usize
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// Initials on a neutral tile: the stand-in for missing, loading or
/// unreadable box art.
fn initials_tile(bg: Hsla, fg: Hsla, initials: String) -> Div {
    div()
        .size_full()
        .bg(bg)
        .flex()
        .items_end()
        .px(px(14.))
        .pb(px(6.))
        .text_size(px(40.))
        .font_bold()
        .text_color(fg)
        .child(initials)
}

/// "Looks like a GBA game" chip for a game sitting in the wrong system.
fn wrong_system_tag(to: &'static SystemDef) -> Tag {
    Tag::danger().small().child(format!("{} game?", to.short_name))
}

/// First genre only — ScreenScraper often sends "Shooter / Run and gun".
fn short_genre(genre: &str) -> &str {
    genre.split(['/', ',']).next().unwrap_or(genre).trim()
}

impl RootView {
    pub(super) fn render_main(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .drag_over::<ExternalPaths>(|style, _, _, cx| style.bg(cx.theme().drop_target))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                this.install_paths(paths.paths().to_vec(), window, cx)
            }))
            .child(
                div().size_full().overflow_y_scrollbar().child(
                    v_flex()
                        .p(px(CONTENT_PADDING))
                        .gap_5()
                        .child(self.render_header(cx))
                        .children(self.render_notice(cx))
                        .child(self.render_toolbar(cx))
                        .child(self.render_library(window, cx)),
                ),
            )
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme();
        let system = self.selected;
        let file_count: usize = self.games.iter().map(|g| g.files.len()).sum();
        let scraping = self.scrape.as_ref().filter(|job| job.system.id == system.id);

        let stats = h_flex()
            .gap_1p5()
            .mt_2()
            .child(Tag::secondary().small().child(plural(self.games.len(), "game")))
            .child(Tag::secondary().small().child(plural(file_count, "file")))
            .child(
                Tag::secondary()
                    .small()
                    .child(h_flex().gap_1().child(Icon::new(IconName::Folder).xsmall()).child(format!("roms/{}", system.folder))),
            );

        let progress = scraping.map(|job| {
            v_flex()
                .mt_3()
                .w(px(380.))
                .gap_1p5()
                .child(
                    h_flex()
                        .justify_between()
                        .text_xs()
                        .text_color(t.muted_foreground)
                        .child("Scraping with Skyscraper")
                        .child(div().text_color(t.foreground).child(format!("{} / {}", job.done, job.total))),
                )
                .child(Progress::new("scrape-progress").value(job.done as f32 / job.total as f32 * 100.))
        });

        let folder = self.system_folder();
        h_flex()
            .gap_4()
            .items_start()
            .child(system_tile(system, px(52.), px(26.), px(12.)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_2xl().font_semibold().child(system.display_name))
                    .child(
                        div()
                            .text_sm()
                            .text_color(t.muted_foreground)
                            .child("Drop files anywhere in the window, or pick them from disk. Zips are extracted automatically."),
                    )
                    .child(stats)
                    .children(progress),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("open-system-folder")
                            .outline()
                            .icon(Icon::new(IconName::FolderOpen))
                            .tooltip("Open in file manager")
                            .on_click(cx.listener(move |this, _, _, cx| this.open_folder(folder.clone(), cx))),
                    )
                    .child(
                        Button::new("scrape-all")
                            .outline()
                            .icon(Icon::new(IconName::ImageDown))
                            .label(if scraping.is_some() { "Scraping…" } else { "Scrape" })
                            .loading(self.scrape.is_some())
                            .disabled(!self.skyscraper_installed || self.games.is_empty())
                            .tooltip("Fetch box art & metadata for every game")
                            .on_click(cx.listener(|this, _, window, cx| this.start_scrape(Vec::new(), window, cx))),
                    )
                    .child(
                        Button::new("add-rom")
                            .primary()
                            .icon(Icon::new(IconName::Plus))
                            .label("Add ROM")
                            .on_click(cx.listener(|this, _, window, cx| this.pick_and_install(window, cx))),
                    ),
            )
    }

    /// Icon tile + bold lead + muted sentence + optional action, tinted with
    /// `tone`. Shared by the notices that need a button (the kit's `Alert`
    /// has no action slot).
    fn notice_row(
        tone: Hsla,
        icon: impl Into<Icon>,
        lead: String,
        body: impl IntoElement,
        action: Option<Button>,
        cx: &App,
    ) -> AnyElement {
        let t = cx.theme();
        h_flex()
            .gap_3()
            .px_4()
            .py_3()
            .rounded(t.radius_lg)
            .border_1()
            .border_color(tone.opacity(0.35))
            .bg(tone.opacity(0.08))
            .child(
                div()
                    .size(px(28.))
                    .flex_shrink_0()
                    .rounded(px(7.))
                    .bg(tone)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon.into().small().text_color(white())),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .child(div().font_semibold().child(lead))
                    .child(div().text_color(t.muted_foreground).child(body)),
            )
            .children(action)
            .into_any_element()
    }

    /// At most one notice, most important first.
    fn render_notice(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = cx.theme();
        if let Some(job) = &self.setup {
            let secs = job.started.elapsed().as_secs();
            let body = h_flex()
                .gap_2()
                .child(Spinner::new().small())
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_family(t.mono_font_family.clone())
                        .text_xs()
                        .child(job.line.clone()),
                );
            return Some(Self::notice_row(
                theme::accent(self.selected.accent),
                Icon::new(IconName::Download),
                format!(
                    "Installing Skyscraper · {}:{:02}  — this can take 10–20 minutes on a Raspberry Pi",
                    secs / 60,
                    secs % 60
                ),
                body,
                None,
                cx,
            ));
        }
        if let Some(game) = &self.running {
            return Some(Self::notice_row(
                theme::accent(self.selected.accent),
                Icon::new(IconName::Play),
                format!("Playing {}", game.title),
                format!("Running in {}. Quit the emulator to come back here.", game.emulator),
                None,
                cx,
            ));
        }
        // A game in the wrong folder won't even start, so this outranks
        // anything about artwork.
        let misplaced = self.misplaced_games();
        if !misplaced.is_empty() {
            let system = self.selected;
            let one = misplaced.len() == 1;
            let new = misplaced.iter().filter(|m| !self.is_enabled(m.to)).count();
            let (lead, label) = match misplaced.as_slice() {
                [m] => (
                    format!("“{}” looks like a {} game", m.entry.title_and_tags().0, m.to.display_name),
                    if new > 0 { format!("Add {} & move", m.to.short_name) } else { format!("Move to {}", m.to.short_name) },
                ),
                _ => (
                    format!("{} look like they're for other systems", plural(misplaced.len(), "game")),
                    format!("Move {}", misplaced.len()),
                ),
            };
            let body = match misplaced.as_slice() {
                [m] if new > 0 => format!(
                    "It won't run with the {} emulator. {} isn't in your list yet: add it and the game moves there, or delete the game.",
                    system.display_name, m.to.display_name
                ),
                _ => format!(
                    "EmulationStation starts everything in roms/{} with the {} emulator, so {}. Move {} where {}{}, or delete {}.",
                    system.folder,
                    system.display_name,
                    if one { "it won't run" } else { "they won't run" },
                    if one { "it" } else { "them" },
                    if one { "it belongs" } else { "they belong" },
                    if new > 0 { " (systems you don't have yet are added)" } else { "" },
                    if one { "it" } else { "them" },
                ),
            };
            // One game moves straight away (nothing is lost); several get
            // a confirmation listing where each goes.
            let single = match misplaced.as_slice() {
                [m] => Some((m.entry.clone(), m.to)),
                _ => None,
            };
            return Some(Self::notice_row(
                t.danger,
                Icon::new(IconName::TriangleAlert),
                lead,
                body,
                Some(
                    Button::new("move-misplaced")
                        .primary()
                        .small()
                        .icon(Icon::new(IconName::FolderInput))
                        .label(label)
                        .on_click(cx.listener(move |this, _, window, cx| match single.clone() {
                            Some((entry, to)) => this.move_game(entry, to, window, cx),
                            None => details::confirm_move_misplaced(this, window, cx),
                        })),
                ),
                cx,
            ));
        }
        if !self.skyscraper_installed {
            if self.can_install_skyscraper {
                return Some(Self::notice_row(
                    t.warning,
                    Icon::new(IconName::ImageDown),
                    "Box art needs Skyscraper".into(),
                    "RetroPie's scraper isn't installed yet. Install it here with RetroPie-Setup — no API keys needed.",
                    Some(
                        Button::new("install-skyscraper")
                            .primary()
                            .small()
                            .icon(Icon::new(IconName::Download))
                            .label("Install Skyscraper")
                            .on_click(cx.listener(|_, _, window, cx| {
                                details::confirm_install_skyscraper(window, cx)
                            })),
                    ),
                    cx,
                ));
            }
            return Some(
                Alert::warning(
                    "skyscraper-missing",
                    format!(
                        "RetroPie-Setup wasn't found in your home folder, so the app can't install it for you. On a RetroPie system, run `{}`.",
                        crate::skyscraper_setup::manual_command()
                    ),
                )
                .title("Box art needs Skyscraper")
                .into_any_element(),
            );
        }
        if self.es_running {
            return Some(
                Alert::warning(
                    "es-running",
                    "It rewrites gamelist.xml when it quits, so new artwork is cached now and applied after you exit it.",
                )
                .title("EmulationStation is running")
                .into_any_element(),
            );
        }
        let missing = self.unscraped_games().len();
        if missing == 0 || self.scrape.is_some() {
            return None;
        }
        Some(Self::notice_row(
            theme::accent(self.selected.accent),
            Icon::new(IconName::ImageDown),
            format!("{} without artwork", plural(missing, "game")),
            "Scrape them to get box art and descriptions here and in EmulationStation.",
            Some(
                Button::new("scrape-missing")
                    .primary()
                    .small()
                    .label(format!("Scrape {missing}"))
                    .on_click(cx.listener(|this, _, window, cx| this.scrape_unscraped(window, cx))),
            ),
            cx,
        ))
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme();
        let count = if self.query.trim().is_empty() {
            self.games.len().to_string()
        } else {
            format!("{} of {}", self.visible_games().len(), self.games.len())
        };
        h_flex()
            .gap_2()
            .child(div().text_base().font_semibold().child("Library"))
            .when(!self.games.is_empty(), |el| el.child(div().text_color(t.muted_foreground).child(count)))
            .child(div().flex_1())
            .child(
                Input::new(&self.search)
                    .w(px(240.))
                    .prefix(Icon::new(IconName::Search).small().text_color(t.muted_foreground))
                    .when_some(Keystroke::parse("/").ok(), |input, key| input.suffix(Kbd::new(key)))
                    .cleanable(true),
            )
            .child(
                ButtonGroup::new("view-mode")
                    .outline()
                    .child(
                        Button::new("view-grid")
                            .icon(Icon::new(IconName::LayoutGrid))
                            .tooltip("Grid")
                            .selected(self.view_mode == ViewMode::Grid),
                    )
                    .child(
                        Button::new("view-list")
                            .icon(Icon::new(IconName::List))
                            .tooltip("List")
                            .selected(self.view_mode == ViewMode::List),
                    )
                    .on_click(cx.listener(|this, selected: &Vec<usize>, _, cx| {
                        let mode = if selected.contains(&1) { ViewMode::List } else { ViewMode::Grid };
                        this.set_view_mode(mode, cx);
                    })),
            )
    }

    fn render_library(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.games.is_empty() {
            return self.render_empty(cx).into_any_element();
        }
        let games = self.visible_games();
        if games.is_empty() {
            return Empty::new()
                .header(
                    EmptyHeader::new()
                        .media(EmptyMedia::new().child(Icon::new(IconName::Search)))
                        .title(EmptyTitle::new().child("No matches"))
                        .description(EmptyDescription::new().child(format!(
                            "Nothing in {} matches “{}”.",
                            self.selected.display_name,
                            self.query.trim()
                        ))),
                )
                .into_any_element();
        }
        match self.view_mode {
            ViewMode::Grid => self.render_grid(&games, window, cx).into_any_element(),
            ViewMode::List => self.render_list(&games, cx).into_any_element(),
        }
    }

    fn render_empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme();
        let system = self.selected;
        Empty::new()
            .header(
                EmptyHeader::new()
                    .media(EmptyMedia::new().child(system_tile(system, px(56.), px(28.), px(14.))))
                    .title(EmptyTitle::new().child(format!("No {} games yet", system.display_name)))
                    .description(EmptyDescription::new().child(format!(
                        "Add a ROM and it lands in ~/RetroPie/roms/{}, ready for EmulationStation. Multi-file games show up as one entry.",
                        system.folder
                    ))),
            )
            .content(
                EmptyContent::new()
                    .child(
                        Button::new("add-first")
                            .primary()
                            .icon(Icon::new(IconName::Plus))
                            .label("Add your first ROM")
                            .on_click(cx.listener(|this, _, window, cx| this.pick_and_install(window, cx))),
                    )
                    .child(
                        h_flex()
                            .mt_2()
                            .gap_1()
                            .justify_center()
                            .children(system.extensions.iter().map(|ext| {
                                div()
                                    .px_1()
                                    .rounded(px(4.))
                                    .bg(t.muted)
                                    .text_xs()
                                    .font_family(t.mono_font_family.clone())
                                    .text_color(t.muted_foreground)
                                    .child(*ext)
                            })),
                    ),
            )
    }

    fn render_grid(&self, games: &[&GameEntry], window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // As many ≥196px columns as fit next to the sidebar.
        let available = window.viewport_size().width.as_f32() - SIDEBAR_WIDTH - 2. * CONTENT_PADDING;
        let columns = ((available + GRID_GAP) / (CARD_MIN_WIDTH + GRID_GAP)).floor().max(1.) as u16;

        div()
            .grid()
            .grid_cols(columns)
            .gap(px(GRID_GAP))
            .when(self.query.trim().is_empty(), |grid| grid.child(self.render_add_tile(cx)))
            .children(games.iter().enumerate().map(|(i, g)| self.render_card(i, g, cx)))
    }

    fn render_add_tile(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme();
        v_flex()
            .id("add-tile")
            .min_h(px(ART_HEIGHT + 110.))
            .p_4()
            .gap_2()
            .items_center()
            .justify_center()
            .rounded(t.radius_lg)
            .border_1()
            .border_dashed()
            .border_color(t.border)
            .cursor_pointer()
            .hover(|s| s.bg(t.muted.opacity(0.4)).border_color(t.muted_foreground))
            .child(
                div()
                    .size(px(44.))
                    .rounded(px(10.))
                    .bg(theme::accent(self.selected.accent))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(IconName::Plus).with_size(px(20.)).text_color(white())),
            )
            .child(div().text_sm().font_semibold().child("Add ROM"))
            .child(div().text_xs().text_color(t.muted_foreground).child("Click or drop files"))
            .child(
                h_flex()
                    .flex_wrap()
                    .justify_center()
                    .gap_1()
                    .max_w(px(170.))
                    .children(self.selected.extensions.iter().map(|ext| {
                        div()
                            .px_1()
                            .rounded(px(4.))
                            .bg(t.muted)
                            .text_xs()
                            .font_family(t.mono_font_family.clone())
                            .text_color(t.muted_foreground)
                            .child(*ext)
                    })),
            )
            .on_click(cx.listener(|this, _, window, cx| this.pick_and_install(window, cx)))
    }

    /// Neutral stand-in for missing box art: initials plus a faint glyph.
    fn art_placeholder(&self, entry: &GameEntry, cx: &App) -> Div {
        let t = cx.theme();
        div()
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(t.muted)
            .child(
                div()
                    .absolute()
                    .right(px(-12.))
                    .top(px(-10.))
                    .opacity(0.08)
                    .child(system_glyph(self.selected, px(92.), t.foreground)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(14.))
                    .bottom(px(6.))
                    .text_size(px(40.))
                    .font_bold()
                    .text_color(t.muted_foreground)
                    .child(entry.monogram()),
            )
    }

    fn render_card(&self, index: usize, entry: &GameEntry, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme();
        let status = self.status_of(entry);
        let (title, tags) = entry.title_and_tags();
        let cover = match status {
            ScrapeStatus::Scraped(m) => m.cover.clone().or(m.screenshot.clone()).filter(|p| p.is_file()),
            _ => None,
        };
        let placeholder = self.art_placeholder(entry, cx);

        let misplaced = self.misplaced_as(entry);
        // A misplaced game would start in the wrong emulator.
        let can_play = self.can_run() && misplaced.is_none();

        let art = div()
            .relative()
            .h(px(ART_HEIGHT))
            .overflow_hidden()
            .map(|art| match cover {
                Some(path) => {
                    // While decoding, or if the file is unreadable, show the
                    // same initials tile as a game without art.
                    let (bg, fg, mono) = (t.muted, t.muted_foreground, entry.monogram());
                    let loading = mono.clone();
                    art.child(
                        img(path)
                            .size_full()
                            .object_fit(ObjectFit::Cover)
                            .with_loading(move || initials_tile(bg, fg, loading.clone()).into_any_element())
                            .with_fallback(move || initials_tile(bg, fg, mono.clone()).into_any_element()),
                    )
                }
                None => art.child(placeholder),
            })
            .child(
                h_flex()
                    .absolute()
                    .left(px(10.))
                    .top(px(10.))
                    .gap_1()
                    .px_1p5()
                    .py(px(1.))
                    .rounded(px(5.))
                    .bg(t.background)
                    .border_1()
                    .border_color(t.border)
                    .text_xs()
                    .font_medium()
                    .text_color(t.muted_foreground)
                    .child(system_glyph(self.selected, px(11.), t.muted_foreground))
                    .child(self.selected.short_name),
            )
            .child(
                div()
                    .absolute()
                    .right(px(10.))
                    .top(px(10.))
                    .rounded(t.radius)
                    .bg(t.background)
                    .invisible()
                    .group_hover("card", |s| s.visible())
                    .flex()
                    .gap_1()
                    .when(can_play, |overlay| {
                        let entry = entry.clone();
                        overlay.child(
                            Button::new(("play", index))
                                .primary()
                                .small()
                                .icon(Icon::new(IconName::Play))
                                .tooltip("Play")
                                .disabled(self.running.is_some())
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.run_game(&entry, window, cx);
                                })),
                        )
                    })
                    .child({
                        let entry = entry.clone();
                        Button::new(("delete", index))
                            .outline()
                            .small()
                            .icon(Icon::new(IconName::Trash))
                            .tooltip("Delete")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                details::confirm_delete_dialog(this, entry.clone(), window, cx);
                            }))
                    }),
            );

        // Row 1: what the game is. Row 2: rating (if scraped) and file count.
        let (chips, rating) = match status {
            ScrapeStatus::Scraped(m) => (
                h_flex()
                    .gap_1()
                    .when_some(m.year.clone(), |row, year| row.child(Tag::secondary().small().child(year)))
                    .when_some(m.genre.clone(), |row, genre: String| {
                        row.child(Tag::secondary().small().child(short_genre(&genre).to_string()))
                    }),
                m.rating.map(|r| Rating::new(("rating", index)).value(stars(r)).xsmall().disabled(true)),
            ),
            other => (
                h_flex()
                    .gap_1()
                    .children(tags.iter().take(2).map(|tag| Tag::secondary().small().child(tag.clone())))
                    .child(Tag::warning().small().child(if other == ScrapeStatus::NoMatch { "No match" } else { "No art" })),
                None,
            ),
        };

        let chips = h_flex().gap_1().children(misplaced.map(wrong_system_tag)).child(chips);

        let open_entry = entry.clone();
        v_flex()
            .id(("card", index))
            .group("card")
            .rounded(t.radius_lg)
            .overflow_hidden()
            .border_1()
            .border_color(if misplaced.is_some() { t.danger.opacity(0.6) } else { t.border.opacity(0.7) })
            .bg(theme::card_bg(cx))
            .cursor_pointer()
            .hover(|s| s.border_color(t.border))
            .child(art)
            .child(
                v_flex()
                    .p_3()
                    .gap_2()
                    .child(div().h(px(38.)).text_sm().font_semibold().line_clamp(2).child(title))
                    .child(div().overflow_hidden().child(chips))
                    .child(
                        h_flex()
                            .h(px(16.))
                            .justify_between()
                            .child(div().children(rating))
                            .child(
                                h_flex()
                                    .gap_1()
                                    .text_xs()
                                    .text_color(t.muted_foreground)
                                    .child(Icon::new(IconName::Files).xsmall())
                                    .child(plural(entry.files.len(), "file")),
                            ),
                    ),
            )
            .on_click(cx.listener(move |this, _, window, cx| details::open_details(this, &open_entry, window, cx)))
    }

    fn render_list(&self, games: &[&GameEntry], cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme();
        let head = h_flex()
            .h(px(34.))
            .px_3()
            .gap_3()
            .bg(t.table_head)
            .text_xs()
            .font_medium()
            .text_color(t.muted_foreground)
            .child(div().w(px(36.)))
            .child(div().flex_1().child("Title"))
            .child(div().w(px(200.)).child("Details"))
            .child(div().w(px(56.)).child("Files"))
            .child(div().w(px(64.)));

        let rows = games.iter().enumerate().map(|(i, entry)| {
            let status = self.status_of(entry);
            let (title, tags) = entry.title_and_tags();
            let thumb_cover = match status {
                ScrapeStatus::Scraped(m) => m.cover.clone().filter(|p| p.is_file()),
                _ => None,
            };
            let byline = match status {
                ScrapeStatus::Scraped(m) => [m.year.clone(), m.developer.clone()].into_iter().flatten().collect::<Vec<_>>().join(" · "),
                ScrapeStatus::NoMatch => "No match on ScreenScraper".into(),
                ScrapeStatus::NotScraped => "Not scraped yet".into(),
            };
            let details_cell = match status {
                ScrapeStatus::Scraped(m) => h_flex()
                    .gap_1()
                    .when_some(m.genre.clone(), |row, g| row.child(Tag::secondary().small().child(short_genre(&g).to_string())))
                    .when_some(m.rating, |row, r| row.child(Rating::new(("list-rating", i)).value(stars(r)).xsmall().disabled(true))),
                _ => h_flex().gap_1().children(tags.iter().take(2).map(|tag| Tag::secondary().small().child(tag.clone()))),
            };
            let details_cell = h_flex().gap_1().children(self.misplaced_as(entry).map(wrong_system_tag)).child(details_cell);
            let open_entry = (*entry).clone();
            let delete_entry = (*entry).clone();
            let play_entry = (*entry).clone();
            let can_play = self.can_run() && self.misplaced_as(entry).is_none();
            h_flex()
                .id(("row", i))
                .h(px(54.))
                .px_3()
                .gap_3()
                .border_t_1()
                .border_color(t.border.opacity(0.7))
                .cursor_pointer()
                .hover(|s| s.bg(t.table_hover))
                .child(
                    div()
                        .size(px(36.))
                        .flex_shrink_0()
                        .rounded(px(6.))
                        .overflow_hidden()
                        .bg(t.muted)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .font_bold()
                        .text_color(t.muted_foreground)
                        .map(|thumb| match thumb_cover {
                            Some(path) => thumb.child(img(path).size_full().object_fit(ObjectFit::Cover)),
                            None => thumb.child(entry.monogram()),
                        }),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().text_sm().font_medium().truncate().child(title))
                        .child(div().text_xs().text_color(t.muted_foreground).truncate().child(byline)),
                )
                .child(div().w(px(200.)).child(details_cell))
                .child(div().w(px(56.)).text_sm().text_color(t.muted_foreground).child(entry.files.len().to_string()))
                .child(
                    h_flex().w(px(64.)).justify_end().gap_1()
                    .when(can_play, |cell| cell.child(
                        Button::new(("list-play", i))
                            .ghost()
                            .small()
                            .icon(Icon::new(IconName::Play))
                            .tooltip("Play")
                            .disabled(self.running.is_some())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.run_game(&play_entry, window, cx);
                            })),
                    ))
                    .child(
                        Button::new(("list-delete", i))
                            .ghost()
                            .small()
                            .icon(Icon::new(IconName::Trash))
                            .tooltip("Delete")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                details::confirm_delete_dialog(this, delete_entry.clone(), window, cx);
                            })),
                    ),
                )
                .on_click(cx.listener(move |this, _, window, cx| details::open_details(this, &open_entry, window, cx)))
        });

        v_flex()
            .rounded(t.radius_lg)
            .overflow_hidden()
            .border_1()
            .border_color(t.border.opacity(0.7))
            .bg(theme::card_bg(cx))
            .child(head)
            .children(rows)
    }
}
