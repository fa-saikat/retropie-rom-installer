//! Work out which system a ROM is really for by looking inside it, since
//! extensions overlap (`.zip`, `.7z`, `.bin`, `.cue`, `.chd`, `.m3u` are
//! each accepted by several systems).
//!
//! Every check here is a *positive* identification from a fixed signature:
//!
//! - N64: byte-order marker in the first 4 bytes (z64 / v64 / n64 dumps).
//! - GBA: fixed value `0x96` at `0xB2` plus the start of the Nintendo logo.
//! - Genesis: `"SEGA"` at `0x100`, or the SMD interleaved-dump header.
//! - PlayStation: ISO9660 volume descriptor in sector 16 whose system id
//!   is `"PLAYSTATION"` (cooked 2048-byte or raw 2352-byte sectors).
//! - Dreamcast: `"SEGA SEGAKATANA"` IP.BIN header, or a CHD with GD-ROM
//!   track metadata.
//!
//! Anything that doesn't match is `None` ("don't know"), never a guess, so
//! callers only warn when a file clearly belongs somewhere else. Arcade has
//! no signature at all; it's only ever the system a file is *in*, not one
//! it's detected *as*.
//!
//! Like `library`, no GPUI dependency, so it's unit tested with `cargo test`.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::library::{ext_lower, GameEntry};
use crate::systems::{self, SystemDef, SYSTEMS};

/// Enough to reach the PlayStation volume descriptor in a raw image:
/// sector 16 × 2352 bytes + 24 bytes of sync/header/subheader + 16.
const HEAD_LEN: u64 = 40 * 1024;

/// How many files a disc sheet or multi-file zip gets to point us at
/// before we give up. Keeps a scan of a big library cheap.
const MAX_PROBES: usize = 4;

/// Best guess at the system `path` is for, or `None` if it can't be told.
pub fn detect(path: &Path) -> Option<&'static SystemDef> {
    detect_depth(path, 0)
}

fn detect_depth(path: &Path, depth: usize) -> Option<&'static SystemDef> {
    let ext = ext_lower(path);
    let by_content = match ext.as_str() {
        ".zip" => detect_zip(path),
        ".cue" => detect_sheet(parse_cue(path)),
        ".gdi" => detect_sheet(parse_gdi(path)),
        ".m3u" if depth < 2 => parse_m3u(path)
            .into_iter()
            .take(MAX_PROBES)
            .find_map(|disc| detect_depth(&disc, depth + 1))
            .map(|s| s.id),
        ".chd" => detect_chd(path),
        // Archives we can't peek into without an external tool.
        ".7z" | ".m3u" => None,
        _ => read_head(path).ok().and_then(|head| sniff(&head)),
    };
    by_content
        .and_then(systems::by_id)
        .or_else(|| unique_owner(&ext))
}

/// The one system that accepts `ext`, if exactly one does (`.gba`, `.gdi`,
/// `.pbp`, ...). Archive extensions never count: a `.zip` says nothing.
fn unique_owner(ext: &str) -> Option<&'static SystemDef> {
    if ext.is_empty() || ext == ".zip" || ext == ".7z" {
        return None;
    }
    let mut owners = SYSTEMS.iter().filter(|s| s.extensions.contains(&ext));
    match (owners.next(), owners.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

fn read_head(path: &Path) -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    fs::File::open(path)?.take(HEAD_LEN).read_to_end(&mut buf)?;
    Ok(buf)
}

fn at(buf: &[u8], offset: usize, needle: &[u8]) -> bool {
    buf.get(offset..offset + needle.len()) == Some(needle)
}

/// Disc signatures first (long, unambiguous), then cartridge headers.
fn sniff(buf: &[u8]) -> Option<&'static str> {
    sniff_disc(buf).or_else(|| sniff_cart(buf).map(|(id, _)| id))
}

/// For a bare cartridge dump: the system it's for and the extension its
/// emulator expects, so an N64 dump named `.bin` can be saved as `.z64`.
pub fn cartridge(path: &Path) -> Option<(&'static SystemDef, &'static str)> {
    let head = read_head(path).ok()?;
    let (id, ext) = sniff_cart(&head)?;
    Some((systems::by_id(id)?, ext))
}

fn sniff_disc(buf: &[u8]) -> Option<&'static str> {
    // Sega disc header (IP.BIN) sits at 0 in a cooked image, after the
    // 16-byte sync + header in a raw one.
    for base in [0, 0x10] {
        if at(buf, base, b"SEGA SEGAKATANA") {
            return Some("dreamcast");
        }
    }
    // ISO9660 primary volume descriptor in sector 16: cooked 2048-byte
    // sectors, raw Mode 1 (16-byte prefix) and raw Mode 2 XA (24-byte
    // prefix, what PlayStation discs use).
    for offset in [16 * 2048, 16 * 2352 + 16, 16 * 2352 + 24] {
        if at(buf, offset, b"\x01CD001") && at(buf, offset + 8, b"PLAYSTATION") {
            return Some("psx");
        }
    }
    None
}

fn sniff_cart(buf: &[u8]) -> Option<(&'static str, &'static str)> {
    // Sega CD / Saturn discs also carry "SEGA" at 0x100, so rule them out
    // before reading that as a Genesis cartridge header.
    if [0, 0x10].iter().any(|&b| at(buf, b, b"SEGADISCSYSTEM") || at(buf, b, b"SEGA SEGASATURN")) {
        return None;
    }
    // N64 byte order: big-endian (.z64), byte-swapped (.v64), little-endian (.n64).
    for (magic, ext) in [
        (b"\x80\x37\x12\x40", ".z64"),
        (b"\x37\x80\x40\x12", ".v64"),
        (b"\x40\x12\x37\x80", ".n64"),
    ] {
        if at(buf, 0, magic) {
            return Some(("n64", ext));
        }
    }
    if buf.get(0xB2) == Some(&0x96) && at(buf, 0x04, b"\x24\xFF\xAE\x51\x69\x9A\xA2\x21") {
        return Some(("gba", ".gba"));
    }
    if at(buf, 0x100, b"SEGA") || at(buf, 0x101, b"SEGA") {
        return Some(("megadrive", ".md"));
    }
    // .smd: 512-byte copier header, bytes 8–10 are 0xAA 0xBB 0x06.
    if buf.len() > 0x200 && at(buf, 8, b"\xAA\xBB\x06") {
        return Some(("megadrive", ".smd"));
    }
    None
}

// ---------------------------------------------------------------------------
// Disc sheets
// ---------------------------------------------------------------------------

/// Check the files a sheet points at, in order, until one is recognised.
fn detect_sheet(files: Vec<PathBuf>) -> Option<&'static str> {
    files
        .iter()
        .take(MAX_PROBES)
        .find_map(|f| read_head(f).ok().and_then(|head| sniff_disc(&head)))
}

fn sibling(sheet: &Path, name: &str) -> PathBuf {
    sheet.parent().unwrap_or(Path::new(".")).join(name)
}

/// `FILE "Game (Track 1).bin" BINARY` lines of a .cue sheet.
fn parse_cue(path: &Path) -> Vec<PathBuf> {
    let Ok(text) = fs::read_to_string(path) else { return Vec::new() };
    text.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("FILE ")?.trim();
            let name = match rest.strip_prefix('"') {
                Some(quoted) => quoted.split('"').next()?,
                None => rest.rsplit_once(' ').map_or(rest, |(name, _kind)| name),
            };
            Some(sibling(path, name))
        })
        .collect()
}

/// Track lines of a .gdi: `3 45000 4 2352 "track03.bin" 0`. The data
/// track holding IP.BIN is track 3, so that goes first.
fn parse_gdi(path: &Path) -> Vec<PathBuf> {
    let Ok(text) = fs::read_to_string(path) else { return Vec::new() };
    let mut tracks: Vec<(u32, PathBuf)> = text
        .lines()
        .skip(1)
        .filter_map(|line| {
            let line = line.trim();
            let mut fields = line.splitn(5, char::is_whitespace);
            let number: u32 = fields.next()?.parse().ok()?;
            let rest = fields.nth(3)?.trim();
            let name = match rest.strip_prefix('"') {
                Some(quoted) => quoted.split('"').next()?,
                None => rest.split_whitespace().next()?,
            };
            Some((number, sibling(path, name)))
        })
        .collect();
    tracks.sort_by_key(|(n, _)| if *n == 3 { 0 } else { *n });
    tracks.into_iter().map(|(_, p)| p).collect()
}

/// Non-comment lines of a multi-disc playlist.
fn parse_m3u(path: &Path) -> Vec<PathBuf> {
    let Ok(text) = fs::read_to_string(path) else { return Vec::new() };
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| sibling(path, l))
        .collect()
}

// ---------------------------------------------------------------------------
// Containers
// ---------------------------------------------------------------------------

/// A one-file zip (how No-Intro sets ship) gets its contents sniffed. A
/// multi-file zip is far more likely an arcade set full of chip dumps, so
/// only disc signatures and system-specific extensions count there.
fn detect_zip(path: &Path) -> Option<&'static str> {
    let file = fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let names: Vec<(usize, String)> = (0..archive.len())
        .filter_map(|i| {
            let entry = archive.by_index_raw(i).ok()?;
            (!entry.is_dir()).then(|| (i, entry.name().to_string()))
        })
        .collect();

    let read_entry = |archive: &mut zip::ZipArchive<fs::File>, i: usize| -> Option<Vec<u8>> {
        let mut buf = Vec::new();
        archive.by_index(i).ok()?.take(HEAD_LEN).read_to_end(&mut buf).ok()?;
        Some(buf)
    };
    let inner_owner = |name: &str| unique_owner(&ext_lower(Path::new(name))).map(|s| s.id);

    if let [(i, name)] = names.as_slice() {
        return read_entry(&mut archive, *i)
            .and_then(|head| sniff(&head))
            .or_else(|| inner_owner(name));
    }

    let owners: Vec<&str> = names.iter().filter_map(|(_, n)| inner_owner(n)).collect();
    if let Some(first) = owners.first() {
        if owners.iter().all(|o| o == first) {
            return Some(first);
        }
    }
    let has_sheet = names.iter().any(|(_, n)| {
        matches!(ext_lower(Path::new(n)).as_str(), ".cue" | ".gdi" | ".toc" | ".ccd" | ".m3u")
    });
    if !has_sheet {
        return None;
    }
    let images: Vec<usize> = names
        .iter()
        .filter(|(_, n)| matches!(ext_lower(Path::new(n)).as_str(), ".bin" | ".img" | ".iso"))
        .map(|(i, _)| *i)
        .take(MAX_PROBES)
        .collect();
    images
        .into_iter()
        .find_map(|i| read_entry(&mut archive, i).and_then(|head| sniff_disc(&head)))
}

/// CHD metadata names the track layout: `CHGD` is a GD-ROM, which only a
/// Dreamcast uses. A plain CD layout could be either disc system (Dreamcast
/// homebrew ships as CD images too), so that stays unknown.
fn detect_chd(path: &Path) -> Option<&'static str> {
    let mut file = fs::File::open(path).ok()?;
    let mut header = [0u8; 56];
    file.read_exact(&mut header).ok()?;
    if &header[..8] != b"MComprHD" {
        return None;
    }
    let be_u64 = |b: &[u8]| u64::from_be_bytes(b.try_into().unwrap());
    let version = u32::from_be_bytes(header[12..16].try_into().unwrap());
    let mut offset = match version {
        5 => be_u64(&header[48..56]),
        3 | 4 => be_u64(&header[36..44]),
        _ => return None,
    };
    // Walk the metadata chain: [tag u32][flags u8 + len u24][next u64].
    for _ in 0..64 {
        if offset == 0 {
            break;
        }
        let mut entry = [0u8; 16];
        file.seek(SeekFrom::Start(offset)).ok()?;
        file.read_exact(&mut entry).ok()?;
        if &entry[..4] == b"CHGD" {
            return Some("dreamcast");
        }
        offset = be_u64(&entry[8..16]);
    }
    None
}

// ---------------------------------------------------------------------------
// Library audit
// ---------------------------------------------------------------------------

/// The file that best stands for a game: its highest-priority anchor (the
/// .m3u / .gdi / .cue), else the first file the system accepts at all.
pub fn representative(system: &SystemDef, entry: &GameEntry) -> Option<PathBuf> {
    let ext_of = |f: &&PathBuf| ext_lower(f);
    system
        .primary_exts
        .iter()
        .find_map(|anchor| entry.files.iter().find(|f| ext_of(f) == *anchor))
        .or_else(|| entry.files.iter().find(|f| system.extensions.contains(&ext_of(f).as_str())))
        .cloned()
}

/// Remembers results by path + size + mtime, so re-scanning a library
/// after every install only reads files that changed.
#[derive(Default)]
pub struct Cache {
    seen: HashMap<PathBuf, (u64, Option<SystemTime>, Option<&'static str>)>,
}

impl Cache {
    pub fn detect(&mut self, path: &Path) -> Option<&'static SystemDef> {
        let meta = fs::metadata(path).ok()?;
        let stamp = (meta.len(), meta.modified().ok());
        if let Some((len, modified, id)) = self.seen.get(path) {
            if (*len, *modified) == stamp {
                return id.and_then(systems::by_id);
            }
        }
        let found = detect(path);
        self.seen.insert(path.to_path_buf(), (stamp.0, stamp.1, found.map(|s| s.id)));
        found
    }

    /// The system `entry` really belongs to, if that's positively a
    /// different one from the folder it's sitting in.
    pub fn misplaced(&mut self, system: &SystemDef, entry: &GameEntry) -> Option<&'static SystemDef> {
        let file = representative(system, entry)?;
        self.detect(&file).filter(|found| found.id != system.id)
    }
}

// ---------------------------------------------------------------------------
// Tests: synthetic headers, built byte-for-byte from the formats above.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    fn id(path: &Path) -> Option<&'static str> {
        detect(path).map(|s| s.id)
    }

    fn n64() -> Vec<u8> {
        let mut rom = vec![0u8; 0x1000];
        rom[..4].copy_from_slice(b"\x80\x37\x12\x40");
        rom
    }

    fn gba() -> Vec<u8> {
        let mut rom = vec![0u8; 0x200];
        rom[0x04..0x0C].copy_from_slice(b"\x24\xFF\xAE\x51\x69\x9A\xA2\x21");
        rom[0xB2] = 0x96;
        rom
    }

    fn genesis() -> Vec<u8> {
        let mut rom = vec![0u8; 0x400];
        rom[0x100..0x110].copy_from_slice(b"SEGA MEGA DRIVE ");
        rom
    }

    /// Raw 2352-byte Mode 2 image with a PlayStation volume descriptor.
    fn psx_raw() -> Vec<u8> {
        let mut img = vec![0u8; 17 * 2352];
        let pvd = 16 * 2352 + 24;
        img[pvd..pvd + 6].copy_from_slice(b"\x01CD001");
        img[pvd + 8..pvd + 19].copy_from_slice(b"PLAYSTATION");
        img
    }

    /// Raw Dreamcast track 3 (IP.BIN after the 16-byte sector prefix).
    fn dreamcast_raw() -> Vec<u8> {
        let mut img = vec![0u8; 2352];
        img[0x10..0x1F].copy_from_slice(b"SEGA SEGAKATANA");
        img
    }

    fn zip_of(dir: &Path, name: &str, files: &[(&str, &[u8])]) -> PathBuf {
        let path = dir.join(name);
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        for (inner, bytes) in files {
            zip.start_file(*inner, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    #[test]
    fn cartridge_headers() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        assert_eq!(id(&write(d, "a.z64", &n64())), Some("n64"));
        assert_eq!(id(&write(d, "b.gba", &gba())), Some("gba"));
        assert_eq!(id(&write(d, "c.bin", &genesis())), Some("megadrive"));
        // Content beats the extension: a GBA dump misnamed .bin.
        assert_eq!(id(&write(d, "d.bin", &gba())), Some("gba"));
    }

    #[test]
    fn disc_images() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        assert_eq!(id(&write(d, "ps.bin", &psx_raw())), Some("psx"));
        assert_eq!(id(&write(d, "dc.bin", &dreamcast_raw())), Some("dreamcast"));
    }

    #[test]
    fn unknown_stays_unknown() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        // .bin is shared by Genesis and PS1 and has no signature here.
        assert_eq!(id(&write(d, "blank.bin", &[0u8; 4096])), None);
        // Sega CD carries "SEGA" at 0x100 too; it must not read as Genesis.
        let mut segacd = genesis();
        segacd[..14].copy_from_slice(b"SEGADISCSYSTEM");
        assert_eq!(id(&write(d, "scd.bin", &segacd)), None);
        assert_eq!(id(&write(d, "set.7z", b"7z")), None);
    }

    #[test]
    fn falls_back_to_an_extension_only_one_system_takes() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        assert_eq!(id(&write(d, "garbage.gba", b"??")), Some("gba"));
        assert_eq!(id(&write(d, "x.pbp", b"\0PBP")), Some("psx"));
    }

    #[test]
    fn cue_and_gdi_sheets_follow_their_tracks() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        write(d, "Game (Track 1).bin", &psx_raw());
        let cue = write(d, "Game.cue", b"FILE \"Game (Track 1).bin\" BINARY\n  TRACK 01 MODE2/2352\n");
        assert_eq!(id(&cue), Some("psx"));

        write(d, "track01.bin", &[0u8; 2352]);
        write(d, "track03.bin", &dreamcast_raw());
        let gdi = write(d, "Crazy Taxi.gdi", b"3\n1 0 4 2352 track01.bin 0\n2 600 0 2352 track02.raw 0\n3 45000 4 2352 track03.bin 0\n");
        assert_eq!(id(&gdi), Some("dreamcast"));

        // A Dreamcast .cue (Redump layout) must not pass for PlayStation.
        let dc_cue = write(d, "DC.cue", b"FILE \"track01.bin\" BINARY\nFILE \"track03.bin\" BINARY\n");
        assert_eq!(id(&dc_cue), Some("dreamcast"));

        let m3u = write(d, "Game.m3u", b"# discs\nGame.cue\n");
        assert_eq!(id(&m3u), Some("psx"));
    }

    #[test]
    fn zips() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        assert_eq!(id(&zip_of(d, "sonic.zip", &[("Sonic.bin", &genesis())])), Some("megadrive"));
        assert_eq!(id(&zip_of(d, "mario.zip", &[("Mario.z64", &n64())])), Some("n64"));
        // Arcade set: several chip dumps, nothing recognisable.
        let arcade = zip_of(d, "pacman.zip", &[("pacman.6e", &[1; 64]), ("pacman.6f", &[2; 64]), ("82s123.7f", &[3; 32])]);
        assert_eq!(id(&arcade), None);
        // Zipped multi-track PS1 game.
        let psx = zip_of(d, "ff.zip", &[("FF.cue", b"FILE \"FF.bin\" BINARY\n"), ("FF.bin", &psx_raw())]);
        assert_eq!(id(&psx), Some("psx"));
    }

    #[test]
    fn chd_gd_rom_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let mut chd = vec![0u8; 124];
        chd[..8].copy_from_slice(b"MComprHD");
        chd[8..12].copy_from_slice(&124u32.to_be_bytes());
        chd[12..16].copy_from_slice(&5u32.to_be_bytes());
        chd[48..56].copy_from_slice(&124u64.to_be_bytes());
        chd.extend_from_slice(b"CHGD\x00\x00\x00\x10");
        chd.extend_from_slice(&0u64.to_be_bytes());
        let path = write(tmp.path(), "dc.chd", &chd);
        assert_eq!(id(&path), Some("dreamcast"));

        chd[124..128].copy_from_slice(b"CHT2");
        let path = write(tmp.path(), "cd.chd", &chd);
        assert_eq!(id(&path), None);
    }

    #[test]
    fn audit_flags_only_positive_mismatches() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        let genesis_sys = systems::by_id("megadrive").unwrap();
        let mut cache = Cache::default();

        let gba_zip = zip_of(d, "Zelda.zip", &[("Zelda.gba", &gba())]);
        let entry = GameEntry { name: "Zelda".into(), files: vec![gba_zip] };
        assert_eq!(cache.misplaced(genesis_sys, &entry).map(|s| s.id), Some("gba"));

        let sonic = write(d, "Sonic.md", &genesis());
        let entry = GameEntry { name: "Sonic".into(), files: vec![sonic] };
        assert!(cache.misplaced(genesis_sys, &entry).is_none());
    }
}
