//! Root view: app state plus every action the UI can trigger. Rendering is
//! split across `sidebar`, `library` and `details`; they all read from this
//! one struct, which keeps a screen this size simple to reason about.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme, WindowExt};
use gpui_kit::*;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config;
use crate::detect;
use crate::launch;
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

/// A game in one system's folder whose contents say it's for another.
#[derive(Clone)]
pub struct Misplaced {
    pub from: &'static SystemDef,
    pub entry: GameEntry,
    pub to: &'static SystemDef,
}

/// A game started with Play, until its emulator quits.
pub struct RunningGame {
    pub title: String,
    pub emulator: String,
}

pub struct RootView {
    pub(super) selected: &'static SystemDef,
    /// Systems in the sidebar: the ones the user added, plus any with games.
    pub(super) enabled: Vec<&'static SystemDef>,
    /// The user's own picks, as saved by `config`.
    saved: Vec<&'static str>,
    /// Systems RetroPie has an emulator set up for (Play works).
    pub(super) runnable: HashSet<&'static str>,
    pub(super) running: Option<RunningGame>,
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
    /// Games across every listed system that belong somewhere else.
    /// Filled in the background.
    pub(super) misplaced: Vec<Misplaced>,
    detect_cache: Arc<Mutex<detect::Cache>>,
    /// Bumped per audit, so a slow older one can't overwrite a newer one.
    audit_generation: u64,
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

        // Debug builds can pick the opening system with
        // ROM_MANAGER_SYSTEM=<id>, for screenshots.
        let requested = cfg!(debug_assertions)
            .then(|| std::env::var("ROM_MANAGER_SYSTEM").ok())
            .flatten()
            .and_then(|id| systems::by_id(&id));
        let mut view = Self {
            selected: requested.unwrap_or(&systems::SYSTEMS[0]),
            enabled: Vec::new(),
            saved: config::load_enabled(),
            runnable: HashSet::new(),
            running: None,
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
            misplaced: Vec::new(),
            detect_cache: Arc::default(),
            audit_generation: 0,
            focus_handle,
            _subscriptions: vec![subscription],
        };
        view.refresh(cx);
        // Otherwise open on the first listed system that has something in it.
        let first_with_games = view.enabled.iter().find(|s| view.counts.get(s.id).is_some_and(|n| *n > 0));
        if let Some(system) = requested.or(first_with_games.copied()) {
            view.selected = system;
            view.refresh(cx);
        }
        theme::apply_accent(view.selected.accent, cx);
        #[cfg(debug_assertions)]
        cx.defer_in(window, Self::debug_startup);
        view
    }

    /// Debug builds only: put the window in a given state at launch so
    /// screenshots don't need clicking around.
    /// `ROM_MANAGER_VIEW=list`, `ROM_MANAGER_THEME=light`,
    /// `ROM_MANAGER_OPEN=<card index>` (details sheet),
    /// `ROM_MANAGER_DIALOG=delete|about|install-skyscraper|add-system|remove-system|move`,
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
            // After the first audit, so suggestions show.
            Some("add-system") => cx
                .spawn_in(window, async move |this, cx| {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    this.update_in(cx, |this, window, cx| super::details::open_add_system(this, window, cx)).ok();
                })
                .detach(),
            Some("remove-system") => super::details::confirm_remove_system(self.selected, window, cx),
            Some("move") => super::details::confirm_move_misplaced(self, window, cx),
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

    /// Re-read everything from disk: which systems are listed, the selected
    /// system's games and gamelist, per-system counts, and the scraper and
    /// emulator environment.
    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
        let libraries: Vec<(&'static SystemDef, Vec<GameEntry>)> = systems::SYSTEMS
            .iter()
            .map(|s| (s, library::list_installed_games(s).unwrap_or_default()))
            .collect();
        self.counts = libraries.iter().map(|(s, games)| (s.id, games.len())).collect();
        self.enabled = systems::SYSTEMS
            .iter()
            .filter(|s| self.saved.contains(&s.id) || self.counts.get(s.id).is_some_and(|n| *n > 0))
            .collect();
        if !self.is_enabled(self.selected) {
            if let Some(first) = self.enabled.first() {
                self.selected = first;
                theme::apply_accent(first.accent, cx);
            }
        }
        self.games = libraries
            .iter()
            .find(|(s, _)| s.id == self.selected.id)
            .map(|(_, games)| games.clone())
            .unwrap_or_default();
        self.gamelist = scraper::read_gamelist(self.selected);
        self.settings = scraper::load_settings();
        self.skyscraper_installed = scraper::skyscraper_binary().is_some();
        self.can_install_skyscraper = skyscraper_setup::retropie_setup_script().is_some();
        self.es_running = scraper::emulationstation_running();
        self.runnable = systems::SYSTEMS.iter().filter(|s| launch::can_run(s)).map(|s| s.id).collect();
        let listed = libraries.into_iter().filter(|(s, _)| self.is_enabled(s)).collect();
        self.audit(listed, cx);
        cx.notify();
    }

    /// Look inside every listed system's games, off the UI thread, for any
    /// that belong to a different system.
    fn audit(&mut self, libraries: Vec<(&'static SystemDef, Vec<GameEntry>)>, cx: &mut Context<Self>) {
        self.audit_generation += 1;
        let generation = self.audit_generation;
        let cache = self.detect_cache.clone();
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_spawn(async move {
                    let mut cache = cache.lock().unwrap();
                    libraries
                        .into_iter()
                        .flat_map(|(from, games)| games.into_iter().map(move |entry| (from, entry)))
                        .filter_map(|(from, entry)| {
                            let to = cache.misplaced(from, &entry)?;
                            Some(Misplaced { from, entry, to })
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            this.update(cx, |this, cx| {
                if this.audit_generation == generation {
                    this.misplaced = found;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn is_enabled(&self, system: &SystemDef) -> bool {
        self.enabled.iter().any(|s| s.id == system.id)
    }

    /// The system `entry` (in the selected system) really belongs to.
    pub(super) fn misplaced_as(&self, entry: &GameEntry) -> Option<&'static SystemDef> {
        self.misplaced
            .iter()
            .find(|m| m.from.id == self.selected.id && m.entry.files.first() == entry.files.first())
            .map(|m| m.to)
    }

    /// Flagged games in the selected system.
    pub(super) fn misplaced_games(&self) -> Vec<&Misplaced> {
        self.misplaced.iter().filter(|m| m.from.id == self.selected.id).collect()
    }

    /// How many games in `system` are flagged, for the sidebar.
    pub(super) fn flagged_in(&self, system: &SystemDef) -> usize {
        self.misplaced.iter().filter(|m| m.from.id == system.id).count()
    }

    /// Flagged games, anywhere, that belong to `system`.
    pub(super) fn waiting_for(&self, system: &SystemDef) -> Vec<&Misplaced> {
        self.misplaced.iter().filter(|m| m.to.id == system.id).collect()
    }

    pub(super) fn can_run(&self) -> bool {
        self.runnable.contains(self.selected.id)
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
                    super::details::confirm_wrong_system(this, system, elsewhere, window, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// Install files into the systems they were identified as, adding any of
    /// those systems that aren't listed yet.
    pub(super) fn install_elsewhere(
        &mut self,
        jobs: Vec<(PathBuf, &'static SystemDef)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (_, system) in &jobs {
            self.enable(system, window, cx);
        }
        self.install_into(jobs, window, cx);
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

    // -----------------------------------------------------------------------
    // Moving games where they belong
    // -----------------------------------------------------------------------

    /// Move a game in the selected system to the system its contents say
    /// it's for, adding that system to the list if needed.
    pub(super) fn move_game(
        &mut self,
        entry: GameEntry,
        to: &'static SystemDef,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (title, _) = entry.title_and_tags();
        let added = !self.is_enabled(to);
        if self.relocate(vec![Misplaced { from: self.selected, entry, to }], window, cx) == 1 {
            let detail = if added {
                format!("{} is in your list now, with the game in it.", to.display_name)
            } else {
                format!("It's in ~/RetroPie/roms/{} now.", to.folder)
            };
            window.push_notification(
                Notification::success(detail).title(format!("Moved {title} to {}", to.display_name)),
                cx,
            );
        }
        window.close_sheet(cx);
        self.refresh(cx);
    }

    /// Move every flagged game in the selected system to where it belongs.
    pub(super) fn move_all_misplaced(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let jobs: Vec<Misplaced> = self.misplaced_games().into_iter().cloned().collect();
        let moved = self.relocate(jobs, window, cx);
        if moved > 0 {
            window.push_notification(
                Notification::success("Each one is in the folder for the system it was made for.")
                    .title(format!("Moved {moved} {}", if moved == 1 { "game" } else { "games" })),
                cx,
            );
        }
        self.refresh(cx);
    }

    /// Move each game to its target system, adding systems that aren't
    /// listed yet. Scraped art stays behind (it was looked up as the wrong
    /// system), so it's removed. Returns how many moved; failures are
    /// reported as they happen.
    fn relocate(&mut self, jobs: Vec<Misplaced>, window: &mut Window, cx: &mut Context<Self>) -> usize {
        let mut gamelists: HashMap<&'static str, HashMap<PathBuf, GameMeta>> = HashMap::new();
        let mut moved = 0;
        for job in jobs {
            self.enable(job.to, window, cx);
            let gamelist = gamelists.entry(job.from.id).or_insert_with(|| scraper::read_gamelist(job.from));
            match library::move_game(&job.entry, job.from, job.to) {
                Ok(_) => {
                    if let Some(meta) = scraper::meta_for(&job.entry, gamelist) {
                        scraper::remove_media(job.from, meta);
                    }
                    moved += 1;
                }
                Err(err) => window.push_notification(
                    Notification::error(err.to_string())
                        .title(format!("Couldn't move {}", job.entry.title_and_tags().0)),
                    cx,
                ),
            }
        }
        moved
    }

    // -----------------------------------------------------------------------
    // Adding / removing systems
    // -----------------------------------------------------------------------

    /// Put `system` in the sidebar for good. Its folder is created so
    /// EmulationStation and the file picker agree it exists.
    fn enable(&mut self, system: &'static SystemDef, window: &mut Window, cx: &mut Context<Self>) {
        if self.saved.contains(&system.id) {
            return;
        }
        self.saved.push(system.id);
        if !self.is_enabled(system) {
            self.enabled.push(system);
        }
        let _ = library::ensure_system_dir(system);
        self.save_enabled(window, cx);
    }

    fn save_enabled(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(err) = config::save_enabled(&self.saved) {
            window.push_notification(
                Notification::warning(format!("It'll be back to the old list next time: {err}"))
                    .title("Couldn't save your systems"),
                cx,
            );
        }
    }

    /// Add `system` from the picker, move over any games that were waiting
    /// for it, and switch to it.
    pub(super) fn add_system(&mut self, system: &'static SystemDef, window: &mut Window, cx: &mut Context<Self>) {
        self.enable(system, window, cx);
        let waiting: Vec<Misplaced> = self.waiting_for(system).into_iter().cloned().collect();
        let moved = self.relocate(waiting, window, cx);
        let detail = if moved > 0 {
            format!("Moved {moved} {} here that belonged to it.", if moved == 1 { "game" } else { "games" })
        } else {
            format!("Drop its games here, or into ~/RetroPie/roms/{}.", system.folder)
        };
        window.push_notification(Notification::success(detail).title(format!("Added {}", system.display_name)), cx);
        self.select_system(system, window, cx);
    }

    /// Take `system` out of the sidebar, deleting its games (the user has
    /// confirmed). The folder itself stays: it belongs to RetroPie.
    pub(super) fn remove_system(&mut self, system: &'static SystemDef, window: &mut Window, cx: &mut Context<Self>) {
        let games = library::list_installed_games(system).unwrap_or_default();
        let gamelist = scraper::read_gamelist(system);
        let mut deleted = 0;
        let mut failed = Vec::new();
        for game in &games {
            match library::uninstall_game(game) {
                Ok(_) => {
                    if let Some(meta) = scraper::meta_for(game, &gamelist) {
                        scraper::remove_media(system, meta);
                    }
                    deleted += 1;
                }
                Err(err) => failed.push(err.to_string()),
            }
        }
        self.saved.retain(|id| *id != system.id);
        self.save_enabled(window, cx);
        if failed.is_empty() {
            let detail = match deleted {
                0 => "You can add it back any time.".to_string(),
                n => format!("Deleted {n} {}.", if n == 1 { "game" } else { "games" }),
            };
            window.push_notification(Notification::success(detail).title(format!("Removed {}", system.display_name)), cx);
        } else {
            window.push_notification(
                Notification::error(failed.join("\n"))
                    .title(format!("Some {} games couldn't be deleted, so it stays listed", system.display_name))
                    .autohide(false),
                cx,
            );
        }
        let was_selected = self.selected.id == system.id;
        self.refresh(cx);
        if was_selected {
            let next = self.selected;
            self.select_system(next, window, cx);
        }
    }

    // -----------------------------------------------------------------------
    // Playing
    // -----------------------------------------------------------------------

    /// Start `entry` with RetroPie's default emulator for the selected
    /// system, and watch for it to quit.
    pub(super) fn run_game(&mut self, entry: &GameEntry, window: &mut Window, cx: &mut Context<Self>) {
        if self.running.is_some() {
            return;
        }
        let system = self.selected;
        let (title, _) = entry.title_and_tags();
        let started = detect::representative(system, entry)
            .ok_or_else(|| anyhow::anyhow!("no ROM file to start"))
            .and_then(|rom| launch::start(system, &rom));
        let launch::Running { emulator, mut child, log } = match started {
            Ok(running) => running,
            Err(err) => {
                window.push_notification(
                    Notification::error(err.to_string()).title(format!("Couldn't start {title}")),
                    cx,
                );
                return;
            }
        };
        window.close_sheet(cx);
        self.running = Some(RunningGame { title: title.clone(), emulator: emulator.clone() });
        cx.notify();

        // Wait on a plain thread (a game can run for hours), and poll it.
        let exit = Arc::new(Mutex::new(None));
        let slot = exit.clone();
        std::thread::spawn(move || {
            let status = child.wait();
            *slot.lock().unwrap() = Some(status);
        });
        cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(500)).await;
            let Some(status) = exit.lock().unwrap().take() else { continue };
            this.update_in(cx, |this, window, cx| {
                this.running = None;
                let failure = match status {
                    Ok(status) if status.success() => None,
                    Ok(status) => Some(match launch::log_tail(&log, 6) {
                        tail if tail.is_empty() => format!("{emulator} quit with {status}."),
                        tail => tail,
                    }),
                    Err(err) => Some(err.to_string()),
                };
                if let Some(detail) = failure {
                    window.push_notification(
                        Notification::error(detail).title(format!("{title} stopped with an error")).autohide(false),
                        cx,
                    );
                }
                cx.notify();
            })
            .ok();
            break;
        })
        .detach();
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
