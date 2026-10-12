//! Root view: app state plus every action the UI can trigger. Rendering is
//! split across `sidebar`, `library` and `details`; they all read from this
//! one struct, which keeps a screen this size simple to reason about.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme, WindowExt};
use gpui_kit::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::detect;
use crate::library::{self, GameEntry};
use crate::scraper::{self, GameMeta, ScrapeStatus, ScraperSettings};
use crate::skyscraper_setup;
use crate::systems::{self, SystemDef};
use crate::theme;

pub const KEY_CONTEXT: &str = "RomManager";

gpui_kit::actions!(rom_manager, [FocusSearch]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Grid,
    List,
}

/// A Skyscraper run in flight, with the latest `#done/total` it reported.
pub struct ScrapeJob {
    pub system: &'static SystemDef,
    pub done: usize,
    pub total: usize,
}

/// Skyscraper being installed through RetroPie-Setup.
pub struct SetupJob {
    pub started: Instant,
    /// Latest line of installer output.
    pub line: String,
}

pub struct RootView {
    pub(super) selected: &'static SystemDef,
    pub(super) games: Vec<GameEntry>,
    /// Scraped metadata for `selected`, keyed by ROM path (from gamelist.xml).
    pub(super) gamelist: HashMap<PathBuf, GameMeta>,
    /// Installed-game count per system id, for the sidebar.
    pub(super) counts: HashMap<&'static str, usize>,
    pub(super) settings: ScraperSettings,
    pub(super) skyscraper_installed: bool,
    /// RetroPie-Setup is present, so the app can install Skyscraper itself.
    pub(super) can_install_skyscraper: bool,
    pub(super) setup: Option<SetupJob>,
    pub(super) es_running: bool,
    pub(super) view_mode: ViewMode,
    pub(super) search: Entity<InputState>,
    pub(super) query: String,
    pub(super) scrape: Option<ScrapeJob>,
    /// Games in `selected` whose contents say they're for another system,
    /// keyed by the entry's first file. Filled in the background.
    pub(super) misplaced: HashMap<PathBuf, &'static SystemDef>,
    detect_cache: Arc<Mutex<detect::Cache>>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl RootView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search games"));
        let subscription = cx.subscribe_in(&search, window, |this, state, event: &InputEvent, _, cx| {
            if let InputEvent::Change = event {
                this.query = state.read(cx).value().to_string();
                cx.notify();
            }
        });
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);

        // Open on the first system that has something in it. Debug builds
        // can pick one with ROM_MANAGER_SYSTEM=<id>, for screenshots.
        let requested = cfg!(debug_assertions)
            .then(|| std::env::var("ROM_MANAGER_SYSTEM").ok())
            .flatten()
            .and_then(|id| systems::by_id(&id));
        let selected = requested.unwrap_or_else(|| {
            systems::SYSTEMS
                .iter()
                .find(|s| library::list_installed_games(s).is_ok_and(|g| !g.is_empty()))
                .unwrap_or(&systems::SYSTEMS[0])
        });
        theme::apply_accent(selected.accent, cx);
        let mut view = Self {
            selected,
            games: Vec::new(),
            gamelist: HashMap::new(),
            counts: HashMap::new(),
            settings: ScraperSettings::default(),
            skyscraper_installed: false,
            can_install_skyscraper: false,
            setup: None,
            es_running: false,
            view_mode: ViewMode::Grid,
            search,
            query: String::new(),
            scrape: None,
            misplaced: HashMap::new(),
            detect_cache: Arc::default(),
            focus_handle,
            _subscriptions: vec![subscription],
        };
        view.refresh(cx);
        #[cfg(debug_assertions)]
        cx.defer_in(window, Self::debug_startup);
        view
    }

    /// Debug builds only: put the window in a given state at launch so
    /// screenshots don't need clicking around.
    /// `ROM_MANAGER_VIEW=list`, `ROM_MANAGER_THEME=light`,
    /// `ROM_MANAGER_OPEN=<card index>` (details sheet),
    /// `ROM_MANAGER_DIALOG=delete|about|install-skyscraper`,
    /// `ROM_MANAGER_RUN=install-skyscraper|scrape` (start it right away),
    /// `ROM_MANAGER_INSTALL=<file>` (as if it were dropped on the window).
    #[cfg(debug_assertions)]
    fn debug_startup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let var = |name| std::env::var(name).ok();
        if var("ROM_MANAGER_VIEW").as_deref() == Some("list") {
            self.view_mode = ViewMode::List;
        }
        if var("ROM_MANAGER_THEME").as_deref() == Some("light") {
            self.toggle_theme(window, cx);
        }
        let pick = var("ROM_MANAGER_OPEN").and_then(|i| i.parse::<usize>().ok());
        if let Some(entry) = pick.and_then(|i| self.games.get(i)).cloned() {
            match var("ROM_MANAGER_DIALOG").as_deref() {
                Some("delete") => super::details::confirm_delete_dialog(self, entry, window, cx),
                _ => super::details::open_details(self, &entry, window, cx),
            }
        }
        match var("ROM_MANAGER_DIALOG").as_deref() {
            Some("about") => super::details::open_about(self, window, cx),
            Some("install-skyscraper") => super::details::confirm_install_skyscraper(window, cx),
            _ => {}
        }
        match var("ROM_MANAGER_RUN").as_deref() {
            Some("install-skyscraper") => self.install_skyscraper(window, cx),
            Some("scrape") => self.start_scrape(Vec::new(), window, cx),
            _ => {}
        }
        if let Some(path) = var("ROM_MANAGER_INSTALL") {
            self.install_paths(vec![PathBuf::from(path)], window, cx);
        }
        cx.notify();
    }

    /// Re-read everything from disk: the selected system's games and
    /// gamelist, per-system counts, and the scraper environment.
    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
        self.games = library::list_installed_games(self.selected).unwrap_or_default();
        self.gamelist = scraper::read_gamelist(self.selected);
        self.counts = systems::SYSTEMS
            .iter()
            .map(|s| (s.id, library::list_installed_games(s).map(|g| g.len()).unwrap_or(0)))
            .collect();
        self.settings = scraper::load_settings();
        self.skyscraper_installed = scraper::skyscraper_binary().is_some();
        self.can_install_skyscraper = skyscraper_setup::retropie_setup_script().is_some();
        self.es_running = scraper::emulationstation_running();
        self.audit(cx);
        cx.notify();
    }

    /// Look inside the selected system's games, off the UI thread, for any
    /// that belong to a different system.
    fn audit(&mut self, cx: &mut Context<Self>) {
        let system = self.selected;
        let games = self.games.clone();
        let cache = self.detect_cache.clone();
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_spawn(async move {
                    let mut cache = cache.lock().unwrap();
                    games
                        .iter()
                        .filter_map(|g| Some((g.files.first()?.clone(), cache.misplaced(system, g)?)))
                        .collect::<HashMap<_, _>>()
                })
                .await;
            this.update(cx, |this, cx| {
                if this.selected.id == system.id {
                    this.misplaced = found;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// The system `entry` really belongs to, if it isn't the selected one.
    pub(super) fn misplaced_as(&self, entry: &GameEntry) -> Option<&'static SystemDef> {
        entry.files.first().and_then(|f| self.misplaced.get(f).copied())
    }

    pub(super) fn misplaced_games(&self) -> Vec<(&GameEntry, &'static SystemDef)> {
        self.games
            .iter()
            .filter_map(|g| Some((g, self.misplaced_as(g)?)))
            .collect()
    }

    pub(super) fn select_system(
        &mut self,
        system: &'static SystemDef,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected = system;
        self.query.clear();
        self.search.update(cx, |s, cx| s.set_value("", window, cx));
        theme::apply_accent(system.accent, cx);
        self.refresh(cx);
    }

    /// Games in the selected system matching the search box.
    pub(super) fn visible_games(&self) -> Vec<&GameEntry> {
        let q = self.query.trim().to_lowercase();
        self.games
            .iter()
            .filter(|g| q.is_empty() || g.name.to_lowercase().contains(&q))
            .collect()
    }

    pub(super) fn status_of<'a>(&'a self, entry: &GameEntry) -> ScrapeStatus<'a> {
        scraper::status_for(entry, &self.gamelist)
    }

    /// Games Skyscraper hasn't looked at yet (a "no match" won't change by
    /// asking again, so those aren't counted).
    pub(super) fn unscraped_games(&self) -> Vec<&GameEntry> {
        self.games
            .iter()
            .filter(|g| matches!(self.status_of(g), ScrapeStatus::NotScraped))
            .collect()
    }

    fn focus_search(&mut self, _: &FocusSearch, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |s, cx| s.focus(window, cx));
    }

    pub(super) fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        self.view_mode = mode;
        cx.notify();
    }

    pub(super) fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        theme::toggle_mode(self.selected.accent, window, cx);
        cx.notify();
    }

    pub(super) fn open_folder(&self, folder: PathBuf, cx: &mut App) {
        let _ = std::fs::create_dir_all(&folder);
        cx.open_with_system(&folder);
    }

    pub(super) fn system_folder(&self) -> PathBuf {
        library::roms_root().join(self.selected.folder)
    }

    // -----------------------------------------------------------------------
    // Install
    // -----------------------------------------------------------------------

    /// Native "Add ROM" file picker off the UI thread, then install.
    pub(super) fn pick_and_install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let system = self.selected;
        let extensions: Vec<String> =
            system.extensions.iter().map(|e| e.trim_start_matches('.').to_string()).collect();

        cx.spawn_in(window, async move |this, cx| {
            let picked: Option<Vec<PathBuf>> = cx
                .background_spawn(async move {
                    let ext_refs: Vec<&str> = extensions.iter().map(String::as_str).collect();
                    rfd::FileDialog::new()
                        .set_title("Select ROM files")
                        .add_filter(system.display_name, &ext_refs)
                        .add_filter("All files", &["*"])
                        .pick_files()
                })
                .await;
            let Some(paths) = picked.filter(|p| !p.is_empty()) else { return };
            this.update_in(cx, |this, window, cx| this.install_paths(paths, window, cx)).ok();
        })
        .detach();
    }

    /// Install dropped/picked files into the selected system. Files whose
    /// contents say they're for a different system are held back and the
    /// user is asked where they should go.
    pub(super) fn install_paths(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        let system = self.selected;
        cx.spawn_in(window, async move |this, cx| {
            let checked = cx
                .background_spawn(async move {
                    paths
                        .into_iter()
                        .map(|p| {
                            let found = detect::detect(&p).filter(|found| found.id != system.id);
                            (p, found)
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            let mut here = Vec::new();
            let mut elsewhere = Vec::new();
            for (path, found) in checked {
                match found {
                    Some(other) => elsewhere.push((path, other)),
                    None => here.push((path, system)),
                }
            }
            this.update_in(cx, |this, window, cx| {
                if !here.is_empty() {
                    this.install_into(here, window, cx);
                }
                if !elsewhere.is_empty() {
                    super::details::confirm_wrong_system(system, elsewhere, window, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// Install each file into the system paired with it, report the outcome,
    /// then scrape whatever landed in the selected system.
    pub(super) fn install_into(
        &mut self,
        jobs: Vec<(PathBuf, &'static SystemDef)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            let results = cx
                .background_spawn(async move {
                    jobs.into_iter()
                        .map(|(p, system)| {
                            let r = library::install_file(&p, system);
                            (p, system, r)
                        })
                        .collect::<Vec<_>>()
                })
                .await;

            this.update_in(cx, |this, window, cx| {
                let selected = this.selected;
                let mut installed = Vec::new();
                for (path, system, result) in results {
                    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    match result {
                        Ok(done) => {
                            let title = if system.id == selected.id {
                                format!("Installed {name}")
                            } else {
                                format!("Installed {name} to {}", system.display_name)
                            };
                            window.push_notification(Notification::success(done.summary).title(title), cx);
                            if system.id == selected.id {
                                installed.extend(done.files);
                            }
                        }
                        Err(err) => window.push_notification(
                            Notification::error(err.to_string()).title(format!("Couldn't install {name}")),
                            cx,
                        ),
                    }
                }
                this.refresh(cx);
                let targets = scraper::scrape_targets(selected, &installed);
                if this.skyscraper_installed && !targets.is_empty() {
                    this.start_scrape(targets, window, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    // -----------------------------------------------------------------------
    // Scrape
    // -----------------------------------------------------------------------

    /// Fetch art + metadata for `roms` (empty = the whole system) with
    /// Skyscraper on a background thread, polling its progress into the UI.
    pub(super) fn start_scrape(&mut self, roms: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        if self.scrape.is_some() {
            return;
        }
        if !self.skyscraper_installed {
            window.push_notification(
                Notification::warning("Add it from RetroPie-Setup → Manage packages → opt → skyscraper.")
                    .title("Skyscraper isn't installed"),
                cx,
            );
            return;
        }
        let system = self.selected;
        let total = if roms.is_empty() { self.games.len() } else { roms.len() };
        self.scrape = Some(ScrapeJob { system, done: 0, total: total.max(1) });
        cx.notify();

        let progress = Arc::new(Mutex::new((0usize, total)));

        // Poll the shared counter while the run is going.
        let poll = progress.clone();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(250)).await;
            let (done, total) = *poll.lock().unwrap();
            let running = this
                .update(cx, |this, cx| match this.scrape.as_mut() {
                    Some(job) => {
                        job.done = done;
                        job.total = total.max(1);
                        cx.notify();
                        true
                    }
                    None => false,
                })
                .unwrap_or(false);
            if !running {
                break;
            }
        })
        .detach();

        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    scraper::scrape(system, &roms, &|done, total| *progress.lock().unwrap() = (done, total))
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.scrape = None;
                let note = match result {
                    Ok(report) if report.gamelist_deferred => Notification::warning(report.summary())
                        .title("EmulationStation is running"),
                    Ok(report) => Notification::success(report.summary())
                        .title(format!("Scraped {}", system.display_name)),
                    Err(err) => Notification::error(err.to_string()).title("Scrape failed"),
                };
                window.push_notification(note, cx);
                this.refresh(cx);
            })
            .ok();
        })
        .detach();
    }

    /// Scrape every game in the selected system that has no metadata yet.
    pub(super) fn scrape_unscraped(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let targets: Vec<PathBuf> = self
            .unscraped_games()
            .iter()
            .flat_map(|g| scraper::scrape_targets(self.selected, &g.files))
            .collect();
        if !targets.is_empty() {
            self.start_scrape(targets, window, cx);
        }
    }

    pub(super) fn scrape_one(&mut self, entry: &GameEntry, window: &mut Window, cx: &mut Context<Self>) {
        let targets = scraper::scrape_targets(self.selected, &entry.files);
        if !targets.is_empty() {
            self.start_scrape(targets, window, cx);
        }
    }

    // -----------------------------------------------------------------------
    // Installing Skyscraper
    // -----------------------------------------------------------------------

    /// Run RetroPie-Setup's Skyscraper install in the background, streaming
    /// its output into the notice, then scrape whatever still lacks art.
    pub(super) fn install_skyscraper(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.setup.is_some() {
            return;
        }
        let line = Arc::new(Mutex::new(String::from("Starting RetroPie-Setup…")));
        self.setup = Some(SetupJob { started: Instant::now(), line: line.lock().unwrap().clone() });
        cx.notify();

        // Refresh the output line and elapsed time while it runs.
        let poll = line.clone();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(500)).await;
            let latest = poll.lock().unwrap().clone();
            let running = this
                .update(cx, |this, cx| match this.setup.as_mut() {
                    Some(job) => {
                        job.line = latest;
                        cx.notify();
                        true
                    }
                    None => false,
                })
                .unwrap_or(false);
            if !running {
                break;
            }
        })
        .detach();

        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    skyscraper_setup::install(&|l| *line.lock().unwrap() = l.to_string())
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.setup = None;
                this.refresh(cx);
                match result {
                    Ok(()) => {
                        window.push_notification(
                            Notification::success("Fetching artwork for games that don't have any yet.")
                                .title("Skyscraper installed"),
                            cx,
                        );
                        this.scrape_unscraped(window, cx);
                    }
                    Err(err) => window.push_notification(
                        Notification::error(err.to_string())
                            .title("Couldn't install Skyscraper")
                            .autohide(false),
                        cx,
                    ),
                }
            })
            .ok();
        })
        .detach();
    }

    // -----------------------------------------------------------------------
    // Uninstall
    // -----------------------------------------------------------------------

    /// Move a game to the system its contents say it's for. Scraped art
    /// stays behind (it was looked up as the wrong system), so it's removed.
    pub(super) fn move_game(
        &mut self,
        entry: GameEntry,
        to: &'static SystemDef,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (title, _) = entry.title_and_tags();
        match self.move_one(&entry, to) {
            Ok(()) => window.push_notification(
                Notification::success(format!("It's in ~/RetroPie/roms/{} now.", to.folder))
                    .title(format!("Moved {title} to {}", to.display_name)),
                cx,
            ),
            Err(err) => window.push_notification(
                Notification::error(err.to_string()).title(format!("Couldn't move {title}")),
                cx,
            ),
        }
        window.close_sheet(cx);
        self.refresh(cx);
    }

    /// Move every flagged game in the selected system to where it belongs.
    pub(super) fn move_all_misplaced(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let jobs: Vec<(GameEntry, &'static SystemDef)> =
            self.misplaced_games().into_iter().map(|(g, to)| (g.clone(), to)).collect();
        let mut moved = 0;
        for (entry, to) in &jobs {
            match self.move_one(entry, to) {
                Ok(()) => moved += 1,
                Err(err) => window.push_notification(
                    Notification::error(err.to_string()).title(format!("Couldn't move {}", entry.title_and_tags().0)),
                    cx,
                ),
            }
        }
        if moved > 0 {
            window.push_notification(
                Notification::success("Each one is in the folder for the system it was made for.")
                    .title(format!("Moved {moved} {}", if moved == 1 { "game" } else { "games" })),
                cx,
            );
        }
        self.refresh(cx);
    }

    fn move_one(&mut self, entry: &GameEntry, to: &'static SystemDef) -> anyhow::Result<()> {
        library::move_game(entry, self.selected, to)?;
        if let Some(meta) = scraper::meta_for(entry, &self.gamelist) {
            scraper::remove_media(self.selected, meta);
        }
        Ok(())
    }

    pub(super) fn confirm_delete(&mut self, entry: GameEntry, window: &mut Window, cx: &mut Context<Self>) {
        let meta = scraper::meta_for(&entry, &self.gamelist).cloned();
        let (title, _) = entry.title_and_tags();
        match library::uninstall_game(&entry) {
            Ok(n) => {
                let media = meta.map(|m| scraper::remove_media(self.selected, &m)).unwrap_or(0);
                let detail = if media > 0 {
                    format!("{n} file(s) and {media} artwork file(s) deleted")
                } else {
                    format!("{n} file(s) deleted")
                };
                window.push_notification(Notification::success(detail).title(format!("Removed {title}")), cx);
            }
            Err(err) => window.push_notification(
                Notification::error(err.to_string()).title(format!("Couldn't remove {title}")),
                cx,
            ),
        }
        window.close_sheet(cx);
        self.refresh(cx);
    }
}

impl Focusable for RootView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::focus_search))
            .size_full()
            .flex()
            .flex_row()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_sidebar(cx))
            .child(self.render_main(window, cx))
    }
}
