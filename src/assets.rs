//! Asset loading: the Lucide icons the UI uses (embedded, via GPUI Kit) and
//! the per-system icons, which are plain SVG files on disk.
//!
//! System icons are *not* embedded — they live in an `assets/` folder next
//! to the app (or the installed `/usr/share/...` copy) so people can swap
//! them. Every lookup is a simple existence check and returns `None` when
//! nothing is found, so callers fall back to a generic gamepad icon.
//!
//! Expected layout:
//! ```text
//! assets/
//!   icons/
//!     psx.svg              <- emulator-specific, checked first
//!     device-gamepad.svg   <- generic fallback (SystemDef::icon), e.g. Tabler icons
//! ```

use gpui_kit::{AssetSource, Result, SharedString};
use std::borrow::Cow;
use std::path::PathBuf;

// GPUI Kit's default bundle covers ~100 Lucide icons; these are the extra
// ones this app uses. Unlisted names fall through to the default bundle.
gpui_kit::assets::icon_assets!(ExtraIcons, [
    Trash, LayoutGrid, List, ImageDown, ImageOff, Gamepad2, Upload, X, VideoOff, Files, Download,
    FolderInput, TriangleAlert, ArrowRight,
]);

/// The app's asset source: our extra icons first, then GPUI Kit's defaults.
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = ExtraIcons.load(path)? {
            return Ok(Some(bytes));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

/// Root of the on-disk asset tree. Resolution order:
/// 1. `assets/` next to the running executable (installed/release layout)
/// 2. `assets/` in the current working directory (`cargo run` from repo root)
/// 3. the Debian package's `/usr/share/retropie-rom-manager/assets`
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

/// Icon for a system.
///
/// Looks for an emulator-specific icon first (`icons/<system_id>.svg`,
/// e.g. `icons/psx.svg`), then falls back to the generic icon named on
/// `SystemDef::icon` (`icons/<icon>.svg` — Tabler icon names, since that's
/// what `SystemDef::icon` stores). Only SVGs: they're drawn as single-colour
/// masks so they can sit white-on-accent in the UI.
pub fn system_icon(system_id: &str, generic_icon: &str) -> Option<PathBuf> {
    let root = assets_root()?;
    [system_id, generic_icon]
        .iter()
        .map(|stem| root.join("icons").join(format!("{stem}.svg")))
        .find(|candidate| candidate.is_file())
}
