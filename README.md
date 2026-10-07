# RetroPie ROM Manager

[![Latest release](https://img.shields.io/github/v/release/fa-saikat/retropie-rom-installer)](https://github.com/fa-saikat/retropie-rom-installer/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A fast, simple desktop GUI for installing and uninstalling RetroPie ROMs on Linux — pick a system, pick your downloaded files, done. Multi-file games stay one tidy entry.

![Populated PlayStation library](screenshots/hero-library.png)

## Features

- **Six systems out of the box** — Arcade (MAME), Dreamcast, Game Boy Advance, Sega Genesis / MD, PlayStation, Nintendo 64, each with its own icon, accent, and accepted file types.
- **Install from downloads** — pick a ROM file in the native file picker; matching files are copied straight into `~/RetroPie/roms/<system>`, zips are extracted automatically (standard zips in-process, tricky ones via `unzip`/`7z` fallbacks).
- **Multi-file games collapse into one entry** — a PSX game spread across a `.cue`, six track `.bin`s, and a generated `.srm` shows up once, as `Doom (USA) (Rev 1)`.
- **Uninstall removes everything** — deleting that entry removes every file recorded for it, so no orphaned track bins or save files linger.
- **Safe by construction** — every delete asks first, and the confirm dialog tells you exactly how many files go away.

![Empty library with dropzone](screenshots/empty-state.png)

## Install

Download the `.deb` from the [latest release](https://github.com/fa-saikat/retropie-rom-installer/releases/latest), then:

```
sudo dpkg -i retropie-rom-manager_*_amd64.deb
sudo apt-get install -f   # only if dpkg reports missing dependencies
```

Optional helpers the app will use when present: `unzip` (stubborn zips), `p7zip-full` (multi-part archives).

## Usage

1. **Pick a system** in the left sidebar — the header shows how many games are installed.
2. **Add ROM** via the dropzone card — the accepted formats for that system are listed right on it.
3. **Browse one entry per game**, with the file count tucked underneath as metadata.
4. **Delete** a game with the red button — confirm once, and every file belonging to it is removed.

![Delete confirmation](screenshots/delete-confirm.png)

## How multi-file grouping works

A PSX or Dreamcast "game" is often several files on disk (`Doom (USA) (Rev 1).cue` plus `(Track 1..6).bin` plus a `.srm` the emulator wrote later). The listing collapses these into one entry by treating files with anchor extensions (`.cue`, `.gdi`, `.m3u`, …) as the file that names the game, then assigning every file to the longest anchor stem that prefixes it at a clean word boundary — so `Doom` never swallows an unrelated `Doomsday`. Uninstall deletes exactly the file list captured when the entry was built, so a half-finished install elsewhere can't sweep up another game's files.

This is a naming heuristic in the No-Intro/Redump style, not a parser of disc-image contents: two discs of one game merge only if an `.m3u` anchors them.

## Supported systems and extensions

Extensions come from RetroPie's own docs and `platforms.cfg`, not guesses:

| System | Folder | Accepted files |
|---|---|---|
| Arcade (MAME) | `arcade` | `.zip`, `.7z` (kept zipped — MAME needs them that way) |
| Dreamcast | `dreamcast` | `.cdi`, `.chd`, `.cue`, `.gdi`, `.zip`, `.m3u` |
| Game Boy Advance | `gba` | `.gba`, `.zip`, `.7z` |
| Sega Genesis / MD | `megadrive` | `.smd`, `.bin`, `.gen`, `.md`, `.zip`, `.7z` |
| PlayStation | `psx` | `.cue`, `.bin`, `.img`, `.mdf`, `.pbp`, `.toc`, `.cbn`, `.m3u`, `.ccd`, `.chd`, `.iso` |
| Nintendo 64 | `n64` | `.n64`, `.z64`, `.v64`, `.zip` |

## Building from source

```
cargo build --release
```

The UI is built with [GPUI](https://www.gpui.rs), which is pre-1.0 and moves fast: if the build complains about element APIs, check [Zed's GPUI source](https://github.com/zed-industries/zed/tree/main/crates/gpui) or pin a git rev (commented example in `Cargo.toml`). The filesystem/grouping logic has no GPUI dependency and is covered by `cargo test` on its own.

Linux system deps for the GUI: a working OpenGL/Vulkan stack plus `libxkbcommon`, `libasound2`, `libssl`, `fontconfig`. The native file picker uses a desktop portal on Wayland or GTK3 on X11. Debian packaging: `./scripts/build-deb.sh` (changelog via `./scripts/generate-changelog.sh` after bumping `Cargo.toml`).

## License

Licensed under the MIT License — see [LICENSE](LICENSE).
