//! Minimal on-disk asset resolution for the window background image and
//! sidebar icons.
//!
//! Deliberately *not* an embedded/bundled asset pipeline — these are plain
//! files a user drops into an `assets/` folder next to the app. Every
//! lookup here is a simple existence check and returns `None` when nothing
//! is found, so callers can fall back to a solid color / no icon without
//! any error handling of their own.
//!
//! Expected layout:
//! ```text
//! assets/
//!   background.jpg        (or .jpeg / .png / bg.jpg / bg.png)
//!   icons/
//!     psx.svg              <- emulator-specific, checked first
//!     device-gamepad.svg   <- generic fallback (SystemDef::icon), e.g. Tabler icons
//! ```

use std::path::PathBuf;

/// Root of the asset tree. Resolution order:
/// 1. `assets/` next to the running executable (installed/release layout)
/// 2. `assets/` in the current working directory (`cargo run` from repo root)
fn assets_root() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("assets");
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }

    let cwd_candidate = PathBuf::from("assets");
    if cwd_candidate.is_dir() {
        return Some(cwd_candidate);
    }

    let system_candidate = PathBuf::from("/usr/share/retropie-rom-manager/assets");
    if system_candidate.is_dir() {
        return Some(system_candidate);
    }

    None
}

/// Path to the window background image, if the user has dropped one into
/// the asset dir. Tries a handful of common filenames/extensions so people
/// don't have to guess the exact one expected. Returns `None` (and the UI
/// falls back to `theme::WINDOW_BG`) if nothing matches.
pub fn background_image() -> Option<PathBuf> {
    let root = assets_root()?;
    for name in [
        "background.jpg",
        "background.jpeg",
        "background.png",
        "bg.jpg",
        "bg.jpeg",
        "bg.png",
    ] {
        let candidate = root.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Icon for a system's sidebar row.
///
/// Looks for an emulator-specific icon first (`icons/<system_id>.{svg,png}`,
/// e.g. `icons/psx.svg`), then falls back to the generic icon named on
/// `SystemDef::icon` (`icons/<icon>.{svg,png}`, e.g. `icons/device-gamepad.svg`
/// — matches Tabler icon names, since that's what `SystemDef::icon` stores).
/// Returns `None` if neither exists; the UI then falls back to a plain
/// accent-colored swatch so the row never looks broken.
pub fn system_icon(system_id: &str, generic_icon: &str) -> Option<PathBuf> {
    let root = assets_root()?;
    for stem in [system_id, generic_icon] {
        for ext in ["svg", "png"] {
            let candidate = root.join("icons").join(format!("{stem}.{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}
