//! Install / list / uninstall logic for a single system's ROM folder.
//!
//! This module has no GPUI dependency on purpose — it's plain filesystem
//! logic, so it can be unit tested with `cargo test` without pulling in a
//! windowing toolkit.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::systems::SystemDef;

pub fn roms_root() -> PathBuf {
    dirs::home_dir()
        .expect("no home directory found")
        .join("RetroPie")
        .join("roms")
}

pub fn ensure_system_dir(system: &SystemDef) -> Result<PathBuf> {
    let dir = roms_root().join(system.folder);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// One row in the "installed games" list. `files` holds every file on disk
/// that belongs to this entry (cue + all track bins + save file, etc.) so
/// uninstall can delete exactly what listing found, with no re-scan races.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameEntry {
    pub name: String,
    pub files: Vec<PathBuf>,
}

fn ext_lower(path: &Path) -> String {
    path.extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default()
}

fn stem_string(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

/// True if `candidate` is a legitimate prefix of `full` for grouping
/// purposes: either an exact match, or `full` continues past the shared
/// prefix with a clear separator. This is what stops "Doom" from wrongly
/// swallowing an unrelated "Doomsday" — the remainder has to *start* a new
/// token (" (Track 1)", "_disc2", "-2", etc.), not just continue the word.
fn is_grouping_prefix(candidate: &str, full: &str) -> bool {
    if candidate.len() > full.len() || !full.starts_with(candidate) {
        return false;
    }
    let remainder = &full[candidate.len()..];
    remainder.is_empty()
        || remainder.starts_with(' ')
        || remainder.starts_with('_')
        || remainder.starts_with('-')
        || remainder.starts_with('.')
}

/// Scan `roms/<system>/` and collapse multi-file games (multi-track
/// .cue/.bin, .gdi + tracks, a .cue plus its generated .srm save, ...) into
/// one entry per game, the way EmulationStation's game list does it.
///
/// Algorithm:
/// 1. "Anchor" files are the ones whose extension is in
///    `system.primary_exts` (the file that names the game — a .cue, a
///    .gdi, an .m3u). If a system has no `primary_exts` (arcade, gba,
///    megadrive, n64) every file anchors itself: one file, one entry.
/// 2. Every file in the directory (anchors included) is assigned to the
///    anchor whose stem is the *longest* valid grouping-prefix of that
///    file's own stem. Longest-match is what lets "Doom 2 (Track 1).bin"
///    correctly prefer the anchor "Doom 2" over a shorter anchor "Doom" if
///    both exist.
/// 3. A file that matches no anchor becomes a singleton entry under its own
///    stem (this is also how non-anchor systems behave, since every file is
///    its own anchor there).
pub fn list_installed_games(system: &SystemDef) -> Result<Vec<GameEntry>> {
    let dir = roms_root().join(system.folder);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            files.push(entry.path());
        }
    }

    let anchor_exts: Vec<&str> = system.primary_exts.to_vec();
    let mut anchor_stems: Vec<String> = if anchor_exts.is_empty() {
        // No dedicated anchor extension for this system: every file anchors
        // itself (plain one-file-one-game systems).
        files.iter().map(|f| stem_string(f)).collect()
    } else {
        files
            .iter()
            .filter(|f| anchor_exts.contains(&ext_lower(f).as_str()))
            .map(|f| stem_string(f))
            .collect()
    };
    anchor_stems.sort_by_key(|s| std::cmp::Reverse(s.len())); // longest first
    anchor_stems.dedup();

    let mut groups: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for file in files {
        let file_stem = stem_string(&file);
        let best_anchor = anchor_stems
            .iter()
            .find(|anchor| is_grouping_prefix(anchor, &file_stem))
            .cloned()
            .unwrap_or_else(|| file_stem.clone());
        groups.entry(best_anchor).or_default().push(file);
    }

    let mut result: Vec<GameEntry> = groups
        .into_iter()
        .map(|(name, mut files)| {
            files.sort();
            GameEntry { name, files }
        })
        .collect();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
}

/// Delete every file recorded on a `GameEntry`. Returns how many files were
/// removed. Uses the file list captured at listing time rather than
/// re-deriving it, so a half-finished install elsewhere can't cause us to
/// delete files from a different game.
pub fn uninstall_game(entry: &GameEntry) -> Result<usize> {
    if entry.files.is_empty() {
        return Err(anyhow!("no files recorded for '{}'", entry.name));
    }
    let mut deleted = 0;
    let mut errors = Vec::new();
    for file in &entry.files {
        match fs::remove_file(file) {
            Ok(()) => deleted += 1,
            Err(e) => errors.push(format!("{}: {e}", file.display())),
        }
    }
    if !errors.is_empty() {
        return Err(anyhow!("errors deleting {}: {}", entry.name, errors.join(", ")));
    }
    Ok(deleted)
}

// ---------------------------------------------------------------------------
// Install
// ---------------------------------------------------------------------------

fn unique_dest(target_dir: &Path, filename: &str) -> PathBuf {
    let dest = target_dir.join(filename);
    if !dest.exists() {
        return dest;
    }
    let path = Path::new(filename);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let suffix = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let mut i = 1;
    loop {
        let candidate = target_dir.join(format!("{stem}_{i}{suffix}"));
        if !candidate.exists() {
            return candidate;
        }
        i += 1;
    }
}

/// Install a single downloaded file for `system`. Zips are extracted (or
/// copied as-is if they don't contain a matching rom); anything else is
/// copied verbatim if its extension is accepted.
pub fn install_file(src: &Path, system: &SystemDef) -> Result<String> {
    let target_dir = ensure_system_dir(system)?;
    let ext = ext_lower(src);

    if ext == ".zip" && system.extensions.contains(&".zip") {
        return install_zip(src, &target_dir, system);
    }

    if !system.extensions.contains(&ext.as_str()) {
        return Err(anyhow!(
            "'{ext}' isn't a supported extension for {}",
            system.display_name
        ));
    }

    let file_name = src
        .file_name()
        .ok_or_else(|| anyhow!("source has no filename"))?
        .to_string_lossy()
        .to_string();
    let dest = unique_dest(&target_dir, &file_name);
    fs::copy(src, &dest)?;
    Ok(format!("copied -> {}", dest.file_name().unwrap().to_string_lossy()))
}

fn rom_exts_for(system: &SystemDef) -> Vec<&'static str> {
    system.extensions.iter().copied().filter(|e| *e != ".zip").collect()
}

fn zip_contains_rom(src: &Path, rom_exts: &[&str]) -> bool {
    let file = match fs::File::open(src) {
        Ok(f) => f,
        Err(_) => return true, // can't even open it — let the extraction chain decide
    };
    match zip::ZipArchive::new(file) {
        Ok(mut archive) => (0..archive.len()).any(|i| {
            archive
                .by_index(i)
                .ok()
                .filter(|f| !f.is_dir())
                .map(|f| {
                    let name = f.name().to_lowercase();
                    rom_exts.iter().any(|ext| name.ends_with(ext))
                })
                .unwrap_or(false)
        }),
        Err(_) => true, // not parseable by the `zip` crate (split archive etc.) — try anyway
    }
}

/// Extraction chain, same order as the Python prototype:
///   1. `zip` crate (fast, zero external deps, handles standard zips)
///   2. system `unzip` binary (self-extracting stubs, Zip64 quirks)
///   3. system `7z` binary (split/multi-part archives, everything else)
fn extract_zip(src: &Path, tmp_dir: &Path) -> Result<()> {
    let file = fs::File::open(src)?;
    match zip::ZipArchive::new(file) {
        Ok(mut archive) => {
            archive.extract(tmp_dir)?;
            return Ok(());
        }
        Err(_) => { /* fall through to unzip */ }
    }

    let unzip = Command::new("unzip")
        .args(["-q", "-o"])
        .arg(src)
        .arg("-d")
        .arg(tmp_dir)
        .output();
    match unzip {
        Ok(out) if out.status.code() == Some(0) || out.status.code() == Some(1) => return Ok(()),
        _ => { /* fall through to 7z */ }
    }

    let sevenz = Command::new("7z")
        .arg("x")
        .arg(src)
        .arg(format!("-o{}", tmp_dir.display()))
        .arg("-y")
        .output();
    match sevenz {
        Ok(out) if out.status.code().map(|c| c <= 1).unwrap_or(false) => Ok(()),
        Ok(out) => Err(anyhow!(
            "7z exit {:?}: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        )),
        Err(_) => Err(anyhow!("7z not found — install it with: sudo apt install p7zip-full")),
    }
}

fn install_zip(src: &Path, target_dir: &Path, system: &SystemDef) -> Result<String> {
    let rom_exts = rom_exts_for(system);

    // Arcade-style systems: the zip *is* the rom, nothing to extract.
    if rom_exts.is_empty() || !zip_contains_rom(src, &rom_exts) {
        let file_name = src.file_name().unwrap().to_string_lossy().to_string();
        let dest = unique_dest(target_dir, &file_name);
        fs::copy(src, &dest)?;
        return Ok(format!("copied as-is -> {}", dest.file_name().unwrap().to_string_lossy()));
    }

    let tmp_dir = tempfile::Builder::new().prefix("retropie-rom-manager-").tempdir()?;
    extract_zip(src, tmp_dir.path())?;

    let mut moved = Vec::new();
    for entry in walk_files(tmp_dir.path())? {
        let ext = ext_lower(&entry);
        if !rom_exts.contains(&ext.as_str()) {
            continue;
        }
        let file_name = entry.file_name().unwrap().to_string_lossy().to_string();
        let dest = unique_dest(target_dir, &file_name);
        fs::rename(&entry, &dest).or_else(|_| fs::copy(&entry, &dest).map(|_| ()))?;
        moved.push(dest.file_name().unwrap().to_string_lossy().to_string());
    }

    if moved.is_empty() {
        let file_name = src.file_name().unwrap().to_string_lossy().to_string();
        let dest = unique_dest(target_dir, &file_name);
        fs::copy(src, &dest)?;
        return Ok(format!(
            "no matching roms inside, copied as-is -> {}",
            dest.file_name().unwrap().to_string_lossy()
        ));
    }
    Ok(format!("extracted {} file(s) -> {}", moved.len(), moved.join(", ")))
}

fn walk_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk_files(&path)?);
        } else {
            out.push(path);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Tests — the grouping algorithm is the trickiest part, so it's covered
// against real-world file layouts rather than GPUI.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn touch(dir: &Path, name: &str) {
        File::create(dir.join(name)).unwrap();
    }

    fn psx_system() -> SystemDef {
        *crate::systems::by_id("psx").unwrap()
    }

    fn dreamcast_system() -> SystemDef {
        *crate::systems::by_id("dreamcast").unwrap()
    }

    #[test]
    fn groups_multi_track_cue_bin_and_save_into_one_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        touch(dir, "Doom (USA) (Rev 1).cue");
        touch(dir, "Doom (USA) (Rev 1).srm");
        for i in 1..=6 {
            touch(dir, &format!("Doom (USA) (Rev 1) (Track {i}).bin"));
        }

        let files: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        let group = group_for_test(&psx_system(), files);

        assert_eq!(group.len(), 1);
        assert_eq!(group[0].name, "Doom (USA) (Rev 1)");
        assert_eq!(group[0].files.len(), 8);
    }

    #[test]
    fn does_not_merge_unrelated_games_sharing_a_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        touch(dir, "Doom.cue");
        touch(dir, "Doom.bin");
        touch(dir, "Doomsday.cue");
        touch(dir, "Doomsday.bin");

        let files: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        let group = group_for_test(&psx_system(), files);

        assert_eq!(group.len(), 2);
        let names: Vec<&str> = group.iter().map(|g| g.name.as_str()).collect();
        assert!(names.contains(&"Doom"));
        assert!(names.contains(&"Doomsday"));
    }

    #[test]
    fn longest_anchor_wins_for_multi_disc_games() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        touch(dir, "Final Fantasy VII (Disc 1).cue");
        touch(dir, "Final Fantasy VII (Disc 1) (Track 1).bin");
        touch(dir, "Final Fantasy VII (Disc 2).cue");
        touch(dir, "Final Fantasy VII (Disc 2) (Track 1).bin");

        let files: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        let group = group_for_test(&psx_system(), files);

        // Without an .m3u tying the discs together, each disc is its own
        // entry — which matches how RetroPie would launch them too.
        assert_eq!(group.len(), 2);
        for g in &group {
            assert_eq!(g.files.len(), 2);
        }
    }

    #[test]
    fn single_file_systems_are_one_file_one_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        touch(dir, "Sonic The Hedgehog (USA).zip");
        touch(dir, "Sonic The Hedgehog 2 (USA).zip");

        let genesis = *crate::systems::by_id("megadrive").unwrap();
        let files: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        let group = group_for_test(&genesis, files);

        assert_eq!(group.len(), 2);
    }

    #[test]
    fn dreamcast_gdi_groups_its_track_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        touch(dir, "Crazy Taxi (USA).gdi");
        touch(dir, "Crazy Taxi (USA) (Track 1).bin");
        touch(dir, "Crazy Taxi (USA) (Track 2).raw");

        let files: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        let group = group_for_test(&dreamcast_system(), files);

        assert_eq!(group.len(), 1);
        assert_eq!(group[0].files.len(), 3);
    }

    /// Test-only helper that runs the same grouping logic as
    /// `list_installed_games` but against an explicit file list instead of
    /// scanning `roms_root()`, so tests don't depend on $HOME.
    fn group_for_test(system: &SystemDef, files: Vec<PathBuf>) -> Vec<GameEntry> {
        let anchor_exts: Vec<&str> = system.primary_exts.to_vec();
        let mut anchor_stems: Vec<String> = if anchor_exts.is_empty() {
            files.iter().map(|f| stem_string(f)).collect()
        } else {
            files
                .iter()
                .filter(|f| anchor_exts.contains(&ext_lower(f).as_str()))
                .map(|f| stem_string(f))
                .collect()
        };
        anchor_stems.sort_by_key(|s| std::cmp::Reverse(s.len()));
        anchor_stems.dedup();

        let mut groups: HashMap<String, Vec<PathBuf>> = HashMap::new();
        for file in files {
            let file_stem = stem_string(&file);
            let best_anchor = anchor_stems
                .iter()
                .find(|anchor| is_grouping_prefix(anchor, &file_stem))
                .cloned()
                .unwrap_or_else(|| file_stem.clone());
            groups.entry(best_anchor).or_default().push(file);
        }
        let mut result: Vec<GameEntry> = groups
            .into_iter()
            .map(|(name, mut files)| {
                files.sort();
                GameEntry { name, files }
            })
            .collect();
        result.sort_by(|a, b| a.name.cmp(&b.name));
        result
    }
}
