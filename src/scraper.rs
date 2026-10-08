//! Game metadata + artwork, fetched through RetroPie's own scraper,
//! Skyscraper (https://github.com/Gemba/skyscraper).
//!
//! Why Skyscraper instead of calling ScreenScraper / TheGamesDB directly:
//! both APIs require developer credentials issued to the calling app, and
//! Skyscraper already ships its own. It's an official RetroPie package
//! (RetroPie-Setup → Manage packages → opt → skyscraper), so on a RetroPie
//! box it's one install away, and running it here gives byte-for-byte the
//! same result as scraping from the RetroPie menu.
//!
//! The flow mirrors RetroPie-Setup's `scriptmodules/supplementary/skyscraper.sh`:
//!   1. *gather*   `Skyscraper -p <sys> -s <source> --flags … <rom files>`
//!                 downloads data + media into Skyscraper's local cache
//!   2. *generate* `Skyscraper -p <sys> --flags … -g <gamelists> -o <media>`
//!                 writes gamelist.xml + media from the cache (offline)
//! with the user's choices read from the same `skyscraper.cfg` that the
//! RetroPie menu edits, so both stay in agreement.
//!
//! Like `library`, this module has no GPUI dependency so it's unit-testable.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::library::{self, GameEntry};
use crate::systems::SystemDef;

const RETROPIE_SKYSCRAPER_CFG: &str = "/opt/retropie/configs/all/skyscraper.cfg";

/// Mirror of RetroPie's `skyscraper.cfg`. Defaults match
/// `_load_config_skyscraper` in RetroPie-Setup, so a box where the user
/// never opened the Skyscraper menu behaves exactly like one where they did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScraperSettings {
    pub scrape_source: String,
    pub use_rom_folder: bool,
    pub download_videos: bool,
    pub cache_marquees: bool,
    pub cache_covers: bool,
    pub cache_wheels: bool,
    pub cache_screenshots: bool,
    pub rom_name: bool,
    pub remove_brackets: bool,
    pub force_refresh: bool,
    pub only_missing: bool,
    pub bypass_all_flags: bool,
}

impl Default for ScraperSettings {
    fn default() -> Self {
        Self {
            scrape_source: "screenscraper".into(),
            use_rom_folder: false,
            download_videos: false,
            cache_marquees: true,
            cache_covers: true,
            cache_wheels: true,
            cache_screenshots: true,
            rom_name: false,
            remove_brackets: false,
            force_refresh: false,
            only_missing: false,
            bypass_all_flags: false,
        }
    }
}

/// Parse `key = "value"` lines (RetroPie's iniConfig format). Unknown keys
/// and malformed lines are ignored; missing keys keep their defaults.
pub fn parse_settings(text: &str) -> ScraperSettings {
    let mut s = ScraperSettings::default();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim().trim_matches('"');
        let flag = value == "1";
        match key.trim() {
            "scrape_source" if !value.is_empty() => s.scrape_source = value.to_string(),
            "use_rom_folder" => s.use_rom_folder = flag,
            "download_videos" => s.download_videos = flag,
            "cache_marquees" => s.cache_marquees = flag,
            "cache_covers" => s.cache_covers = flag,
            "cache_wheels" => s.cache_wheels = flag,
            "cache_screenshots" => s.cache_screenshots = flag,
            "rom_name" => s.rom_name = flag,
            "remove_brackets" => s.remove_brackets = flag,
            "force_refresh" => s.force_refresh = flag,
            "only_missing" => s.only_missing = flag,
            "bypass_all_flags" => s.bypass_all_flags = flag,
            _ => {}
        }
    }
    s
}

pub fn load_settings() -> ScraperSettings {
    fs::read_to_string(RETROPIE_SKYSCRAPER_CFG)
        .map(|t| parse_settings(&t))
        .unwrap_or_default()
}

/// Where RetroPie installs Skyscraper, falling back to `$PATH`.
pub fn skyscraper_binary() -> Option<PathBuf> {
    let known = [
        PathBuf::from("/usr/local/bin/Skyscraper"),
        PathBuf::from("/opt/retropie/supplementary/skyscraper/Skyscraper"),
    ];
    let on_path = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).map(|d| d.join("Skyscraper")).collect::<Vec<_>>())
        .unwrap_or_default();
    known.into_iter().chain(on_path).find(|p| p.is_file())
}

/// `~/.emulationstation` — on RetroPie a symlink to
/// `/opt/retropie/configs/all/emulationstation`.
fn es_home() -> PathBuf {
    dirs::home_dir()
        .expect("no home directory found")
        .join(".emulationstation")
}

/// (gamelist folder, media folder) exactly as RetroPie's Skyscraper menu
/// picks them.
pub fn output_dirs(system: &SystemDef, settings: &ScraperSettings) -> (PathBuf, PathBuf) {
    output_dirs_in(system, settings, &library::roms_root(), &es_home())
}

fn output_dirs_in(
    system: &SystemDef,
    settings: &ScraperSettings,
    roms_root: &Path,
    es_home: &Path,
) -> (PathBuf, PathBuf) {
    if settings.use_rom_folder {
        let dir = roms_root.join(system.folder);
        (dir.clone(), dir.join("media"))
    } else {
        (
            es_home.join("gamelists").join(system.folder),
            es_home.join("downloaded_media").join(system.folder),
        )
    }
}

/// Command line for one Skyscraper run. `source: Some(..)` = gather step
/// (network → cache) for `roms`, `None` = generate step (cache → gamelist).
/// Same parameter order and flag set as `_get_clioptions_skyscraper`.
pub fn skyscraper_args(
    system: &SystemDef,
    settings: &ScraperSettings,
    source: Option<&str>,
    roms: &[PathBuf],
) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["-p".into(), system.folder.into()];
    if let Some(source) = source {
        args.push("-s".into());
        args.push(source.into());
    }

    if !settings.bypass_all_flags {
        let mut flags = vec!["unattend", "skipped", "nohints"];
        if settings.download_videos {
            flags.push("videos");
        }
        if !settings.cache_marquees {
            flags.push("nomarquees");
        }
        if !settings.cache_covers {
            flags.push("nocovers");
        }
        if !settings.cache_screenshots {
            flags.push("noscreenshots");
        }
        if !settings.cache_wheels {
            flags.push("nowheels");
        }
        if settings.only_missing {
            flags.push("onlymissing");
        }
        if settings.rom_name {
            flags.push("forcefilename");
        }
        if settings.remove_brackets {
            flags.push("nobrackets");
        }
        if settings.use_rom_folder {
            flags.push("relative");
        }

        let (gamelist_dir, media_dir) = output_dirs(system, settings);
        args.push("-g".into());
        args.push(gamelist_dir.into());
        args.push("-o".into());
        args.push(media_dir.into());
        args.push("--flags".into());
        args.push(flags.join(",").into());
    }
    if settings.force_refresh {
        args.push("--refresh".into());
    }
    args.extend(roms.iter().map(|r| r.as_os_str().to_owned()));
    args
}

/// EmulationStation rewrites gamelist.xml from memory when it exits, which
/// would silently throw away anything written while it's running — the
/// same reason RetroPie only offers scraping from outside ES. The kernel
/// truncates `comm` to 15 chars, hence "emulationstatio".
pub fn emulationstation_running() -> bool {
    let Ok(procs) = fs::read_dir("/proc") else { return false };
    procs.flatten().any(|p| {
        fs::read_to_string(p.path().join("comm"))
            .map(|c| c.trim().starts_with("emulationstatio"))
            .unwrap_or(false)
    })
}

/// The files Skyscraper should be pointed at for a set of freshly installed
/// files: anchors (.cue/.gdi/.m3u) when there are any, so a multi-track
/// game is scraped once rather than once per .bin, otherwise every file
/// with an accepted ROM extension.
pub fn scrape_targets(system: &SystemDef, files: &[PathBuf]) -> Vec<PathBuf> {
    let has_ext = |f: &PathBuf, exts: &[&str]| exts.contains(&library::ext_lower(f).as_str());
    let anchors: Vec<PathBuf> = files
        .iter()
        .filter(|f| has_ext(f, system.primary_exts))
        .cloned()
        .collect();
    if !anchors.is_empty() {
        return anchors;
    }
    files
        .iter()
        .filter(|f| has_ext(f, system.extensions))
        .cloned()
        .collect()
}

#[derive(Debug, Clone, Default)]
pub struct ScrapeReport {
    /// Requested ROMs that ended up with metadata in gamelist.xml.
    pub matched: usize,
    pub requested: usize,
    /// True if gather ran but gamelist generation was skipped because
    /// EmulationStation is running.
    pub gamelist_deferred: bool,
}

impl ScrapeReport {
    pub fn summary(&self) -> String {
        if self.gamelist_deferred {
            return "artwork cached — quit EmulationStation and scrape again to update its game list"
                .into();
        }
        if self.requested == 0 {
            return "game list updated".into();
        }
        format!("found metadata for {} of {} game(s)", self.matched, self.requested)
    }
}

/// Drop ANSI colour/cursor sequences (`ESC [ … final-byte`) from a line of
/// terminal output.
pub fn strip_ansi(line: &str) -> String {
    let mut plain = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for c in chars.by_ref() {
                if ('@'..='~').contains(&c) && c != '[' {
                    break;
                }
            }
        } else {
            plain.push(c);
        }
    }
    plain
}

/// Skyscraper reports each finished game as `#<n>/<total> <details>`,
/// wrapped in ANSI colour codes. Returns `(n, total)` for those lines.
pub fn parse_progress(line: &str) -> Option<(usize, usize)> {
    let plain = strip_ansi(line);
    let counter = plain.trim_start().strip_prefix('#')?.split_whitespace().next()?;
    let (done, total) = counter.split_once('/')?;
    Some((done.parse().ok()?, total.parse().ok()?))
}

/// Run Skyscraper to completion, feeding per-game progress to `progress`.
fn run_skyscraper(bin: &Path, args: &[OsString], progress: &dyn Fn(usize, usize)) -> Result<()> {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("couldn't run Skyscraper: {e}"))?;

    // Drain stderr on its own thread so a chatty run can't fill the pipe
    // and stall Skyscraper while we're blocked reading stdout.
    let mut stderr = child.stderr.take().expect("piped stderr");
    let stderr_reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });

    let mut last_line = String::new();
    for line in BufReader::new(child.stdout.take().expect("piped stdout")).lines() {
        let Ok(line) = line else { break };
        if let Some((done, total)) = parse_progress(&line) {
            progress(done, total);
        }
        if !line.trim().is_empty() {
            last_line = line;
        }
    }
    let status = child.wait()?;
    let stderr = stderr_reader.join().unwrap_or_default();
    if status.success() {
        return Ok(());
    }
    let tail = stderr
        .lines()
        .filter(|l| !l.trim().is_empty())
        .last()
        .unwrap_or(&last_line);
    Err(anyhow!("Skyscraper exited with {:?}: {tail}", status.code()))
}

/// Scrape `roms` (or the whole system if empty) and regenerate the system's
/// gamelist.xml. Blocking — call from a background task. `progress` gets
/// `(games done, games total)` as Skyscraper works through the list.
pub fn scrape(
    system: &SystemDef,
    roms: &[PathBuf],
    progress: &dyn Fn(usize, usize),
) -> Result<ScrapeReport> {
    let bin = skyscraper_binary().ok_or_else(|| {
        anyhow!("Skyscraper isn't installed — add it from RetroPie-Setup → Manage packages → opt → skyscraper")
    })?;
    let settings = load_settings();

    run_skyscraper(
        &bin,
        &skyscraper_args(system, &settings, Some(&settings.scrape_source), roms),
        progress,
    )?;

    let mut report = ScrapeReport { requested: roms.len(), ..Default::default() };
    if emulationstation_running() {
        report.gamelist_deferred = true;
        return Ok(report);
    }
    run_skyscraper(&bin, &skyscraper_args(system, &settings, None, &[]), &|_, _| {})?;

    let meta = read_gamelist(system);
    report.matched = roms
        .iter()
        .filter(|r| meta.get(*r).is_some_and(GameMeta::has_metadata))
        .count();
    Ok(report)
}

// ---------------------------------------------------------------------------
// Reading gamelist.xml back, so the app can show art + descriptions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GameMeta {
    pub name: Option<String>,
    pub desc: Option<String>,
    /// Box art (`<thumbnail>`; Skyscraper writes covers there).
    pub cover: Option<PathBuf>,
    /// In-game screenshot (`<image>`).
    pub screenshot: Option<PathBuf>,
    pub marquee: Option<PathBuf>,
    pub video: Option<PathBuf>,
    /// 0.0–1.0, as EmulationStation stores it.
    pub rating: Option<f32>,
    /// First four digits of `releasedate` (`19951116T000000`).
    pub year: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genre: Option<String>,
    pub players: Option<String>,
}

impl GameMeta {
    pub fn has_metadata(&self) -> bool {
        self.cover.is_some() || self.screenshot.is_some() || self.desc.is_some()
    }

    fn media_files(&self) -> impl Iterator<Item = &PathBuf> {
        [&self.cover, &self.screenshot, &self.marquee, &self.video]
            .into_iter()
            .flatten()
    }
}

/// Whether a game ended up with metadata, as far as gamelist.xml says.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrapeStatus<'a> {
    Scraped(&'a GameMeta),
    /// In gamelist.xml but without art or a description: Skyscraper (run with
    /// `skipped`) listed it but found no match.
    NoMatch,
    NotScraped,
}

pub fn status_for<'a>(entry: &GameEntry, gamelist: &'a HashMap<PathBuf, GameMeta>) -> ScrapeStatus<'a> {
    match meta_for(entry, gamelist) {
        Some(m) if m.has_metadata() => ScrapeStatus::Scraped(m),
        Some(_) => ScrapeStatus::NoMatch,
        None => ScrapeStatus::NotScraped,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MediaState {
    Saved(PathBuf),
    /// Switched off in RetroPie's Skyscraper settings.
    Disabled,
    NotFound,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaSlot {
    pub label: &'static str,
    pub state: MediaState,
}

/// One entry per media type ES can show, so the details view can say what
/// was saved and, for the rest, whether it's turned off or just wasn't found.
pub fn media_slots(meta: &GameMeta, settings: &ScraperSettings) -> Vec<MediaSlot> {
    let slot = |label, path: &Option<PathBuf>, enabled: bool| MediaSlot {
        label,
        state: match path {
            Some(p) if p.is_file() => MediaState::Saved(p.clone()),
            _ if !enabled => MediaState::Disabled,
            _ => MediaState::NotFound,
        },
    };
    vec![
        slot("Box art", &meta.cover, settings.cache_covers),
        slot("Screenshot", &meta.screenshot, settings.cache_screenshots),
        slot("Marquee", &meta.marquee, settings.cache_marquees || settings.cache_wheels),
        slot("Video", &meta.video, settings.download_videos),
    ]
}

/// gamelist.xml location, in EmulationStation's own lookup order: the ROM
/// folder first, then `~/.emulationstation/gamelists/<system>/`.
fn gamelist_path(system: &SystemDef) -> Option<PathBuf> {
    [
        library::roms_root().join(system.folder).join("gamelist.xml"),
        es_home().join("gamelists").join(system.folder).join("gamelist.xml"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// Metadata keyed by absolute ROM path. Empty if there's no gamelist yet or
/// it can't be parsed — callers just show the game without art.
pub fn read_gamelist(system: &SystemDef) -> HashMap<PathBuf, GameMeta> {
    let Some(path) = gamelist_path(system) else { return HashMap::new() };
    let Ok(text) = fs::read_to_string(&path) else { return HashMap::new() };
    let home = dirs::home_dir().unwrap_or_default();
    parse_gamelist(&text, &library::roms_root().join(system.folder), &home)
}

fn resolve_path(raw: &str, rom_dir: &Path, home: &Path) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("./") {
        rom_dir.join(rest)
    } else if let Some(rest) = raw.strip_prefix("~/") {
        home.join(rest)
    } else if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        rom_dir.join(raw)
    }
}

pub fn parse_gamelist(xml: &str, rom_dir: &Path, home: &Path) -> HashMap<PathBuf, GameMeta> {
    let Ok(doc) = roxmltree::Document::parse(xml) else { return HashMap::new() };
    let mut out = HashMap::new();
    for game in doc.descendants().filter(|n| n.has_tag_name("game")) {
        let text = |tag: &str| {
            game.children()
                .find(|c| c.has_tag_name(tag))
                .and_then(|c| c.text())
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_string)
        };
        let path_of = |tag: &str| text(tag).map(|p| resolve_path(&p, rom_dir, home));
        let Some(rom) = path_of("path") else { continue };
        out.insert(
            rom,
            GameMeta {
                name: text("name"),
                desc: text("desc"),
                cover: path_of("thumbnail"),
                screenshot: path_of("image"),
                marquee: path_of("marquee"),
                video: path_of("video"),
                rating: text("rating").and_then(|r| r.parse().ok()),
                year: text("releasedate").filter(|d| d.len() >= 4).map(|d| d[..4].to_string()),
                developer: text("developer"),
                publisher: text("publisher"),
                genre: text("genre"),
                players: text("players"),
            },
        );
    }
    out
}

/// Metadata for a library entry: whichever of its files gamelist.xml
/// describes (the anchor for multi-file games, the ROM itself otherwise).
pub fn meta_for<'a>(entry: &GameEntry, gamelist: &'a HashMap<PathBuf, GameMeta>) -> Option<&'a GameMeta> {
    entry.files.iter().find_map(|f| gamelist.get(f))
}

/// Delete the scraped media for a game that's being uninstalled. Only
/// touches files inside the system's media folders (Skyscraper's and the
/// ES built-in scraper's), so a hand-edited gamelist pointing somewhere
/// else can't make uninstall delete unrelated files. The stale gamelist
/// entry itself is harmless — ES skips entries whose ROM is gone, and the
/// next scrape drops it.
pub fn remove_media(system: &SystemDef, meta: &GameMeta) -> usize {
    let settings = load_settings();
    let (_, media_dir) = output_dirs(system, &settings);
    let allowed = [media_dir, es_home().join("downloaded_images").join(system.folder)];
    meta.media_files()
        .filter(|f| allowed.iter().any(|dir| f.starts_with(dir)))
        .filter(|f| fs::remove_file(f).is_ok())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems;

    fn psx() -> SystemDef {
        *systems::by_id("psx").unwrap()
    }

    #[test]
    fn missing_config_uses_retropie_defaults() {
        let s = parse_settings("");
        assert_eq!(s, ScraperSettings::default());
        assert_eq!(s.scrape_source, "screenscraper");
    }

    #[test]
    fn parses_retropie_skyscraper_cfg() {
        let s = parse_settings(
            "scrape_source = \"thegamesdb\"\nuse_rom_folder = \"1\"\ncache_wheels = \"0\"\n# comment\ngarbage line\n",
        );
        assert_eq!(s.scrape_source, "thegamesdb");
        assert!(s.use_rom_folder);
        assert!(!s.cache_wheels);
        assert!(s.cache_covers);
    }

    #[test]
    fn gather_args_match_retropie_menu() {
        let s = ScraperSettings { cache_wheels: false, ..Default::default() };
        let rom = PathBuf::from("/home/pi/RetroPie/roms/psx/Doom (USA).cue");
        let args: Vec<String> = skyscraper_args(&psx(), &s, Some("screenscraper"), &[rom])
            .into_iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(&args[..4], ["-p", "psx", "-s", "screenscraper"]);
        let flags = &args[args.iter().position(|a| a == "--flags").unwrap() + 1];
        assert_eq!(flags, "unattend,skipped,nohints,nowheels");
        assert!(args.last().unwrap().ends_with("Doom (USA).cue"));
    }

    #[test]
    fn generate_args_have_no_source_and_no_roms() {
        let args = skyscraper_args(&psx(), &ScraperSettings::default(), None, &[]);
        assert!(!args.iter().any(|a| a == "-s"));
        assert!(args.iter().any(|a| a == "-g"));
    }

    #[test]
    fn bypass_all_flags_drops_flags_and_folders() {
        let s = ScraperSettings { bypass_all_flags: true, ..Default::default() };
        let args = skyscraper_args(&psx(), &s, None, &[]);
        assert_eq!(args, vec![OsString::from("-p"), OsString::from("psx")]);
    }

    #[test]
    fn output_dirs_follow_use_rom_folder() {
        let roms = Path::new("/r");
        let es = Path::new("/es");
        let (g, m) = output_dirs_in(&psx(), &ScraperSettings::default(), roms, es);
        assert_eq!(g, PathBuf::from("/es/gamelists/psx"));
        assert_eq!(m, PathBuf::from("/es/downloaded_media/psx"));

        let s = ScraperSettings { use_rom_folder: true, ..Default::default() };
        let (g, m) = output_dirs_in(&psx(), &s, roms, es);
        assert_eq!(g, PathBuf::from("/r/psx"));
        assert_eq!(m, PathBuf::from("/r/psx/media"));
    }

    #[test]
    fn scrape_targets_prefer_anchors_over_track_files() {
        let files: Vec<PathBuf> = ["Doom.cue", "Doom (Track 1).bin", "Doom (Track 2).bin"]
            .iter()
            .map(PathBuf::from)
            .collect();
        assert_eq!(scrape_targets(&psx(), &files), vec![PathBuf::from("Doom.cue")]);

        let gba = *systems::by_id("gba").unwrap();
        let files = vec![PathBuf::from("Metroid.gba"), PathBuf::from("readme.txt")];
        assert_eq!(scrape_targets(&gba, &files), vec![PathBuf::from("Metroid.gba")]);
    }

    #[test]
    fn parses_skyscraper_gamelist_with_relative_and_absolute_paths() {
        let xml = r#"<?xml version="1.0"?>
<gameList>
    <game>
        <path>./Doom (USA) (Rev 1).cue</path>
        <name>Doom</name>
        <thumbnail>/home/pi/.emulationstation/downloaded_media/psx/covers/Doom (USA) (Rev 1).png</thumbnail>
        <image>./media/screenshots/Doom (USA) (Rev 1).png</image>
        <marquee/>
        <rating>0.8</rating>
        <releasedate>19951116T000000</releasedate>
        <developer>id Software</developer>
        <desc>Rip &amp; tear.</desc>
    </game>
    <game><name>No path, ignored</name></game>
</gameList>"#;
        let rom_dir = Path::new("/home/pi/RetroPie/roms/psx");
        let meta = parse_gamelist(xml, rom_dir, Path::new("/home/pi"));
        assert_eq!(meta.len(), 1);
        let m = &meta[&rom_dir.join("Doom (USA) (Rev 1).cue")];
        assert_eq!(m.name.as_deref(), Some("Doom"));
        assert_eq!(m.desc.as_deref(), Some("Rip & tear."));
        assert_eq!(m.year.as_deref(), Some("1995"));
        assert_eq!(m.rating, Some(0.8));
        assert_eq!(m.marquee, None);
        assert_eq!(m.screenshot, Some(rom_dir.join("media/screenshots/Doom (USA) (Rev 1).png")));
        assert!(m.cover.as_ref().unwrap().is_absolute());
    }

    #[test]
    fn reads_skyscraper_progress_lines() {
        assert_eq!(parse_progress("\u{1b}[0;32m#3/8\u{1b}[0m \u{1b}[1;33m---- Game 'Doom' found! :) ----"), Some((3, 8)));
        assert_eq!(parse_progress("#12/120 Tekken 3"), Some((12, 120)));
        assert_eq!(parse_progress("Total number of games: 8"), None);
        assert_eq!(parse_progress("#not/progress"), None);
    }

    #[test]
    fn media_slots_explain_what_is_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let cover = tmp.path().join("cover.png");
        fs::write(&cover, "png").unwrap();
        let meta = GameMeta {
            cover: Some(cover.clone()),
            screenshot: Some(tmp.path().join("gone.png")),
            ..Default::default()
        };
        let slots = media_slots(&meta, &ScraperSettings::default());
        assert_eq!(slots[0].state, MediaState::Saved(cover));
        assert_eq!(slots[1].state, MediaState::NotFound);
        assert_eq!(slots[3].state, MediaState::Disabled); // videos are off by default
    }

    #[test]
    fn broken_gamelist_yields_nothing() {
        assert!(parse_gamelist("<gameList><game>", Path::new("/r"), Path::new("/h")).is_empty());
    }

    #[test]
    fn remove_media_never_leaves_the_media_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tmp.path().join("precious.cue");
        fs::write(&outside, "x").unwrap();
        let meta = GameMeta { cover: Some(outside.clone()), ..Default::default() };
        assert_eq!(remove_media(&psx(), &meta), 0);
        assert!(outside.exists());
    }
}
