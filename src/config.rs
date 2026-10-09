//! Which systems the user keeps in the sidebar.
//!
//! Stored as one system id per line in
//! `~/.config/retropie-rom-manager/systems`. A system that has games on disk
//! is always shown whether it's listed or not (see `RootView::refresh`), so
//! this file only has to remember the ones added while still empty.
//!
//! Like `library`, no GPUI dependency, so it's unit tested with `cargo test`.

use anyhow::{anyhow, Result};
use std::fs;
use std::path::PathBuf;

use crate::systems::{self, DEFAULT_ENABLED};

fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("retropie-rom-manager").join("systems"))
}

/// Known ids from `text`, in file order, without duplicates.
fn parse(text: &str) -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(system) = systems::by_id(line) {
            if !ids.contains(&system.id) {
                ids.push(system.id);
            }
        }
    }
    ids
}

/// The user's saved list, or the defaults if they've never changed it.
pub fn load_enabled() -> Vec<&'static str> {
    match path().and_then(|p| fs::read_to_string(p).ok()) {
        // An emptied-out file means "start over", not "show nothing".
        Some(text) if !parse(&text).is_empty() => parse(&text),
        _ => DEFAULT_ENABLED.to_vec(),
    }
}

pub fn save_enabled(ids: &[&str]) -> Result<()> {
    let path = path().ok_or_else(|| anyhow!("no config folder found"))?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&path, ids.iter().map(|id| format!("{id}\n")).collect::<String>())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_known_ids_in_order_once() {
        assert_eq!(parse("snes\n  gb \nnot-a-system\nsnes\n\n"), ["snes", "gb"]);
        assert!(parse("").is_empty());
    }
}
