//! The 6 supported emulators and their RetroPie ROM-folder conventions.
//!
//! Extensions are taken from RetroPie's own `platforms.cfg` / per-system docs
//! (https://retropie.org.uk/docs/), not guessed. A couple of notes on choices:
//!
//! - `arcade` ships as `.zip`/`.7z` MAME sets (the folder itself is often
//!   shared across several MAME core versions in real RetroPie installs, but
//!   for this tool we only care about what a user would drop in).
//! - `psx` and `dreamcast` are the two systems where one *game* is commonly
//!   made of several *files* (multi-track .cue/.bin, .gdi + track files,
//!   plus generated .srm/.mcr save files). `primary_exts` lists the
//!   extensions that can anchor a game entry, in priority order — see
//!   `library::list_installed_games` for how that's used.
//! - Systems with an empty `primary_exts` (arcade, gba, megadrive, n64) are
//!   effectively "one file = one game"; every file anchors itself.

#[derive(Debug, Clone, Copy)]
pub struct SystemDef {
    /// Stable identifier, also used as a storage/lookup key.
    pub id: &'static str,
    /// Display name shown in the UI.
    pub display_name: &'static str,
    /// Subfolder under `RetroPie/roms/`.
    pub folder: &'static str,
    /// Extensions (lowercase, with leading dot) install_file will accept.
    pub extensions: &'static [&'static str],
    /// Extensions that can "anchor" a multi-file game, in priority order
    /// (first = preferred canonical file when more than one anchor could
    /// describe the same stem, e.g. an .m3u over a lone .cue).
    /// Empty = every file anchors itself (no multi-file grouping needed).
    pub primary_exts: &'static [&'static str],
    /// Tabler icon name (without the `ti-` prefix) used in the UI.
    pub icon: &'static str,
    /// Accent color, `0xRRGGBB`.
    pub accent: u32,
}

pub const SYSTEMS: &[SystemDef] = &[
    SystemDef {
        id: "arcade",
        display_name: "Arcade (MAME)",
        folder: "arcade",
        extensions: &[".zip", ".7z"],
        primary_exts: &[],
        icon: "device-gamepad-2",
        accent: 0xff6644,
    },
    SystemDef {
        id: "dreamcast",
        display_name: "Dreamcast",
        folder: "dreamcast",
        extensions: &[".cdi", ".chd", ".cue", ".gdi", ".zip", ".m3u"],
        // .m3u (multi-disc playlist) wins over .gdi/.cue if all three are
        // somehow present for the same game; .gdi is the normal anchor.
        primary_exts: &[".m3u", ".gdi", ".cue"],
        icon: "disc",
        accent: 0xcc3333,
    },
    SystemDef {
        id: "gba",
        display_name: "Game Boy Advance",
        folder: "gba",
        extensions: &[".gba", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-mobile",
        accent: 0xb946d1,
    },
    SystemDef {
        id: "megadrive",
        display_name: "Sega Genesis / MD",
        folder: "megadrive",
        extensions: &[".smd", ".bin", ".gen", ".md", ".zip", ".7z"],
        primary_exts: &[],
        icon: "cube",
        accent: 0x3388cc,
    },
    SystemDef {
        id: "psx",
        display_name: "PlayStation",
        folder: "psx",
        extensions: &[
            ".cue", ".bin", ".img", ".mdf", ".pbp", ".toc", ".cbn", ".m3u", ".ccd", ".chd",
            ".iso",
        ],
        // .m3u (multi-disc) > .cue/.toc/.ccd (single-disc sheet) > lone .pbp/.chd.
        primary_exts: &[".m3u", ".cue", ".toc", ".ccd", ".pbp", ".chd", ".iso"],
        icon: "device-gamepad",
        accent: 0x444466,
    },
    SystemDef {
        id: "n64",
        display_name: "Nintendo 64",
        folder: "n64",
        extensions: &[".n64", ".z64", ".v64", ".zip"],
        primary_exts: &[],
        icon: "square-rounded",
        accent: 0x00bfff,
    },
];

pub fn by_id(id: &str) -> Option<&'static SystemDef> {
    SYSTEMS.iter().find(|s| s.id == id)
}
