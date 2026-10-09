//! Every RetroPie system the app knows, and their ROM-folder conventions.
//!
//! Extensions are taken from RetroPie's own `platforms.cfg` / per-system docs
//! (https://retropie.org.uk/docs/), not guessed. A couple of notes on choices:
//!
//! - `arcade` ships as `.zip`/`.7z` MAME sets (the folder itself is often
//!   shared across several MAME core versions in real RetroPie installs, but
//!   for this tool we only care about what a user would drop in).
//! - Disc systems (`psx`, `dreamcast`, `segacd`, `pcengine`, `ps2`) are the
//!   ones where one *game* is commonly made of several *files* (multi-track
//!   .cue/.bin, .gdi + track files, plus generated .srm/.mcr save files).
//!   `primary_exts` lists the extensions that can anchor a game entry, in
//!   priority order — see `library::list_installed_games` for how that's used.
//! - Systems with an empty `primary_exts` are effectively "one file = one
//!   game"; every file anchors itself.
//!
//! Only some of these are shown in the sidebar at once; see `config`.

#[derive(Debug, Clone, Copy)]
pub struct SystemDef {
    /// Stable identifier, also used as a storage/lookup key.
    pub id: &'static str,
    /// Display name shown in the UI.
    pub display_name: &'static str,
    /// Compact label for badges on game cards ("PS1", "N64").
    pub short_name: &'static str,
    /// Who made it, for grouping in the "Add emulator" picker.
    pub maker: &'static str,
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
    /// Photo in `assets/systems/<photo>.jpg`, for the "Add emulator" picker.
    pub photo: &'static str,
    /// Accent color, `0xRRGGBB`. Used flat (icon tile, active marker,
    /// primary button), so each one is tuned to hold white text and read on
    /// both the light and dark theme.
    pub accent: u32,
}

/// Shown in the sidebar until the user picks their own set.
pub const DEFAULT_ENABLED: &[&str] = &["arcade", "dreamcast", "gba", "megadrive", "psx", "n64"];

/// Group order in the "Add emulator" picker.
pub const MAKERS: &[&str] = &["Nintendo", "Sega", "Sony", "Atari", "SNK", "NEC", "Arcade", "Computers", "Other"];

pub const SYSTEMS: &[SystemDef] = &[
    SystemDef {
        id: "amstradcpc",
        display_name: "Amstrad CPC",
        short_name: "CPC",
        maker: "Computers",
        folder: "amstradcpc",
        extensions: &[".cdt", ".cpc", ".dsk"],
        primary_exts: &[],
        icon: "keyboard",
        photo: "amstradcpc",
        accent: 0x2f8f8a,
    },
    SystemDef {
        id: "arcade",
        display_name: "Arcade (MAME)",
        short_name: "Arcade",
        maker: "Arcade",
        folder: "arcade",
        extensions: &[".zip", ".7z"],
        primary_exts: &[],
        icon: "device-gamepad-2",
        photo: "arcade",
        accent: 0xe0663a,
    },
    SystemDef {
        id: "atari2600",
        display_name: "Atari 2600",
        short_name: "2600",
        maker: "Atari",
        folder: "atari2600",
        extensions: &[".a26", ".bin", ".rom", ".gz", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-gamepad-2",
        photo: "atari2600",
        accent: 0xc2410c,
    },
    SystemDef {
        id: "atari5200",
        display_name: "Atari 5200",
        short_name: "5200",
        maker: "Atari",
        folder: "atari5200",
        extensions: &[".a52", ".bas", ".bin", ".car", ".xex", ".atr", ".xfd", ".dcm", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-gamepad-2",
        photo: "atari5200",
        accent: 0xb45309,
    },
    SystemDef {
        id: "atari7800",
        display_name: "Atari 7800",
        short_name: "7800",
        maker: "Atari",
        folder: "atari7800",
        extensions: &[".a78", ".bin", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-gamepad-2",
        photo: "atari7800",
        accent: 0x9a3412,
    },
    SystemDef {
        id: "atari800",
        display_name: "Atari 800",
        short_name: "800",
        maker: "Atari",
        folder: "atari800",
        extensions: &[
            ".a52", ".atr", ".bas", ".bin", ".car", ".cas", ".com", ".dcm", ".xex", ".xfd", ".zip", ".7z",
        ],
        primary_exts: &[],
        icon: "keyboard",
        photo: "atari800",
        accent: 0xa16207,
    },
    SystemDef {
        id: "atarilynx",
        display_name: "Atari Lynx",
        short_name: "Lynx",
        maker: "Atari",
        folder: "atarilynx",
        extensions: &[".lnx", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-mobile",
        photo: "atarilynx",
        accent: 0xb7791f,
    },
    SystemDef {
        id: "channelf",
        display_name: "Fairchild Channel F",
        short_name: "Channel F",
        maker: "Other",
        folder: "channelf",
        extensions: &[".bin", ".chf", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-gamepad-2",
        photo: "channelf",
        accent: 0x7c6f64,
    },
    SystemDef {
        id: "coleco",
        display_name: "ColecoVision",
        short_name: "Coleco",
        maker: "Other",
        folder: "coleco",
        extensions: &[".bin", ".col", ".rom", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-gamepad",
        photo: "coleco",
        accent: 0x4b5563,
    },
    SystemDef {
        id: "dreamcast",
        display_name: "Dreamcast",
        short_name: "Dreamcast",
        maker: "Sega",
        folder: "dreamcast",
        extensions: &[".cdi", ".chd", ".cue", ".gdi", ".zip", ".m3u"],
        // .m3u (multi-disc playlist) wins over .gdi/.cue if all three are
        // somehow present for the same game; .gdi is the normal anchor.
        primary_exts: &[".m3u", ".gdi", ".cue"],
        icon: "disc",
        photo: "dreamcast",
        accent: 0xd9475b,
    },
    SystemDef {
        id: "fba",
        display_name: "FinalBurn Alpha",
        short_name: "FBA",
        maker: "Arcade",
        folder: "fba",
        extensions: &[".zip", ".7z", ".fba", ".cue"],
        primary_exts: &[],
        icon: "device-gamepad-2",
        photo: "arcade",
        accent: 0xdb5a2c,
    },
    SystemDef {
        id: "fds",
        display_name: "Famicom Disk System",
        short_name: "FDS",
        maker: "Nintendo",
        folder: "fds",
        extensions: &[".fds", ".nes", ".zip", ".7z"],
        primary_exts: &[],
        icon: "disc",
        photo: "fds",
        accent: 0xc53030,
    },
    SystemDef {
        id: "gamegear",
        display_name: "Sega Game Gear",
        short_name: "Game Gear",
        maker: "Sega",
        folder: "gamegear",
        extensions: &[".gg", ".bin", ".sms", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-mobile",
        photo: "gamegear",
        accent: 0x0e7490,
    },
    SystemDef {
        id: "gb",
        display_name: "Game Boy",
        short_name: "GB",
        maker: "Nintendo",
        folder: "gb",
        extensions: &[".gb", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-mobile",
        photo: "gb",
        accent: 0x5f7a3a,
    },
    SystemDef {
        id: "gba",
        display_name: "Game Boy Advance",
        short_name: "GBA",
        maker: "Nintendo",
        folder: "gba",
        extensions: &[".gba", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-mobile",
        photo: "gba",
        accent: 0x8f63cf,
    },
    SystemDef {
        id: "gbc",
        display_name: "Game Boy Color",
        short_name: "GBC",
        maker: "Nintendo",
        folder: "gbc",
        extensions: &[".gbc", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-mobile",
        photo: "gbc",
        accent: 0xb83280,
    },
    SystemDef {
        id: "gc",
        display_name: "GameCube",
        short_name: "GC",
        maker: "Nintendo",
        folder: "gc",
        extensions: &[".gcm", ".iso", ".gcz", ".ciso", ".rvz", ".dol", ".elf"],
        primary_exts: &[],
        icon: "disc",
        photo: "gc",
        accent: 0x6d28d9,
    },
    SystemDef {
        id: "mame-libretro",
        display_name: "MAME (libretro)",
        short_name: "MAME",
        maker: "Arcade",
        folder: "mame-libretro",
        extensions: &[".zip", ".7z", ".cmd"],
        primary_exts: &[],
        icon: "device-gamepad-2",
        photo: "arcade",
        accent: 0xbe4b2b,
    },
    SystemDef {
        id: "mastersystem",
        display_name: "Sega Master System",
        short_name: "SMS",
        maker: "Sega",
        folder: "mastersystem",
        extensions: &[".sms", ".bin", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-gamepad",
        photo: "mastersystem",
        accent: 0xb91c1c,
    },
    SystemDef {
        id: "megadrive",
        display_name: "Sega Genesis / MD",
        short_name: "Genesis",
        maker: "Sega",
        folder: "megadrive",
        extensions: &[".smd", ".bin", ".gen", ".md", ".zip", ".7z"],
        primary_exts: &[],
        icon: "cube",
        photo: "megadrive",
        accent: 0x3a7bd5,
    },
    SystemDef {
        id: "msx",
        display_name: "MSX",
        short_name: "MSX",
        maker: "Computers",
        folder: "msx",
        extensions: &[".rom", ".mx1", ".mx2", ".cas", ".dsk", ".m3u", ".zip", ".7z"],
        primary_exts: &[],
        icon: "keyboard",
        photo: "msx",
        accent: 0x1d4ed8,
    },
    SystemDef {
        id: "n64",
        display_name: "Nintendo 64",
        short_name: "N64",
        maker: "Nintendo",
        folder: "n64",
        extensions: &[".n64", ".z64", ".v64", ".zip"],
        primary_exts: &[],
        icon: "device-gamepad-3",
        photo: "n64",
        accent: 0x2c9a63,
    },
    SystemDef {
        id: "neogeo",
        display_name: "Neo Geo",
        short_name: "Neo Geo",
        maker: "SNK",
        folder: "neogeo",
        extensions: &[".zip", ".7z"],
        primary_exts: &[],
        icon: "device-gamepad-2",
        photo: "neogeo",
        accent: 0x9a7b1c,
    },
    SystemDef {
        id: "nes",
        display_name: "Nintendo Entertainment System",
        short_name: "NES",
        maker: "Nintendo",
        folder: "nes",
        extensions: &[".nes", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-gamepad",
        photo: "nes",
        accent: 0xdc2626,
    },
    SystemDef {
        id: "ngp",
        display_name: "Neo Geo Pocket",
        short_name: "NGP",
        maker: "SNK",
        folder: "ngp",
        extensions: &[".ngp", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-mobile",
        photo: "ngp",
        accent: 0x57534e,
    },
    SystemDef {
        id: "ngpc",
        display_name: "Neo Geo Pocket Color",
        short_name: "NGPC",
        maker: "SNK",
        folder: "ngpc",
        extensions: &[".ngc", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-mobile",
        photo: "ngpc",
        accent: 0x0f766e,
    },
    SystemDef {
        id: "pcengine",
        display_name: "PC Engine / TurboGrafx-16",
        short_name: "PCE",
        maker: "NEC",
        folder: "pcengine",
        extensions: &[".pce", ".cue", ".ccd", ".chd", ".zip", ".7z"],
        // HuCards are one file each; CD games anchor on their sheet.
        primary_exts: &[".cue", ".ccd", ".chd"],
        icon: "device-gamepad",
        photo: "pcengine",
        accent: 0xea580c,
    },
    SystemDef {
        id: "ps2",
        display_name: "PlayStation 2",
        short_name: "PS2",
        maker: "Sony",
        folder: "ps2",
        extensions: &[".iso", ".chd", ".cso", ".bin", ".img", ".mdf", ".nrg", ".cue", ".gz", ".elf"],
        primary_exts: &[".cue", ".iso", ".chd", ".cso", ".mdf", ".nrg"],
        icon: "disc",
        photo: "ps2",
        accent: 0x1e40af,
    },
    SystemDef {
        id: "psp",
        display_name: "PlayStation Portable",
        short_name: "PSP",
        maker: "Sony",
        folder: "psp",
        extensions: &[".iso", ".cso", ".pbp", ".chd"],
        primary_exts: &[],
        icon: "device-mobile",
        photo: "psp",
        accent: 0x334155,
    },
    SystemDef {
        id: "psx",
        display_name: "PlayStation",
        short_name: "PS1",
        maker: "Sony",
        folder: "psx",
        extensions: &[
            ".cue", ".bin", ".img", ".mdf", ".pbp", ".toc", ".cbn", ".m3u", ".ccd", ".chd",
            ".iso",
        ],
        // .m3u (multi-disc) > .cue/.toc/.ccd (single-disc sheet) > lone .pbp/.chd.
        primary_exts: &[".m3u", ".cue", ".toc", ".ccd", ".pbp", ".chd", ".iso"],
        icon: "device-gamepad",
        photo: "psx",
        accent: 0x5b6ee1,
    },
    SystemDef {
        id: "sega32x",
        display_name: "Sega 32X",
        short_name: "32X",
        maker: "Sega",
        folder: "sega32x",
        extensions: &[".32x", ".bin", ".md", ".smd", ".zip", ".7z"],
        primary_exts: &[],
        icon: "cube",
        photo: "sega32x",
        accent: 0xe11d48,
    },
    SystemDef {
        id: "segacd",
        display_name: "Sega CD",
        short_name: "Sega CD",
        maker: "Sega",
        folder: "segacd",
        extensions: &[".cue", ".bin", ".chd", ".iso", ".m3u", ".7z"],
        primary_exts: &[".m3u", ".cue", ".chd", ".iso"],
        icon: "disc",
        photo: "segacd",
        accent: 0x4338ca,
    },
    SystemDef {
        id: "sg-1000",
        display_name: "Sega SG-1000",
        short_name: "SG-1000",
        maker: "Sega",
        folder: "sg-1000",
        extensions: &[".sg", ".bin", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-gamepad",
        photo: "sg-1000",
        accent: 0x64748b,
    },
    SystemDef {
        id: "snes",
        display_name: "Super Nintendo",
        short_name: "SNES",
        maker: "Nintendo",
        folder: "snes",
        extensions: &[".sfc", ".smc", ".fig", ".swc", ".mgd", ".bin", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-gamepad",
        photo: "snes",
        accent: 0x6b46c1,
    },
    SystemDef {
        id: "vectrex",
        display_name: "Vectrex",
        short_name: "Vectrex",
        maker: "Other",
        folder: "vectrex",
        extensions: &[".vec", ".gam", ".bin", ".zip", ".7z"],
        primary_exts: &[],
        icon: "device-gamepad-2",
        photo: "vectrex",
        accent: 0x475569,
    },
    SystemDef {
        id: "wii",
        display_name: "Wii",
        short_name: "Wii",
        maker: "Nintendo",
        folder: "wii",
        extensions: &[".iso", ".wbfs", ".rvz", ".gcz", ".ciso", ".wad", ".dol", ".elf", ".gcm"],
        primary_exts: &[],
        icon: "disc",
        photo: "wii",
        accent: 0x0284c7,
    },
    SystemDef {
        id: "zxspectrum",
        display_name: "ZX Spectrum",
        short_name: "ZX",
        maker: "Computers",
        folder: "zxspectrum",
        extensions: &[
            ".tzx", ".tap", ".z80", ".sna", ".szx", ".udi", ".mgt", ".trd", ".scl", ".img", ".dsk", ".gz",
            ".zip", ".7z",
        ],
        primary_exts: &[],
        icon: "keyboard",
        photo: "zxspectrum",
        accent: 0x9333ea,
    },
];

pub fn by_id(id: &str) -> Option<&'static SystemDef> {
    SYSTEMS.iter().find(|s| s.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_consistent() {
        for (i, s) in SYSTEMS.iter().enumerate() {
            assert!(SYSTEMS[..i].iter().all(|o| o.id != s.id), "duplicate id {}", s.id);
            assert!(MAKERS.contains(&s.maker), "{} has unknown maker {}", s.id, s.maker);
            for ext in s.extensions.iter().chain(s.primary_exts) {
                assert!(ext.starts_with('.') && *ext == ext.to_lowercase(), "{}: bad extension {ext}", s.id);
            }
            assert!(s.primary_exts.iter().all(|e| s.extensions.contains(e)), "{}: anchor not accepted", s.id);
        }
        assert!(DEFAULT_ENABLED.iter().all(|id| by_id(id).is_some()));
        for s in SYSTEMS {
            let photo = std::path::Path::new("assets/systems").join(format!("{}.jpg", s.photo));
            assert!(photo.is_file(), "{} has no photo at {}", s.id, photo.display());
        }
    }
}
