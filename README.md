# RetroPie ROM manager (GPUI)

A desktop GUI for installing/uninstalling ROMs into a RetroPie
`~/RetroPie/roms/<system>` tree, for 6 systems: Arcade (MAME), Dreamcast,
Game Boy Advance, Sega Genesis/MD, PlayStation, and Nintendo 64.

## Layout

```
src/
  main.rs      entry point, opens the window
  systems.rs   the 6 SystemDef entries: folder, accepted extensions, icon
  library.rs   pure filesystem logic — install, list, uninstall, grouping
  theme.rs     color tokens (matches the approved draft)
  ui/
    root.rs    the whole UI: sidebar + library grid + dropzone + confirm dialog
```

`library.rs` has no GPUI dependency and is unit tested on its own —
`cargo test` covers the multi-file grouping logic without needing a display.

## The grouping problem

A PSX or Dreamcast "game" is often several files on disk:

```
Doom (USA) (Rev 1).cue
Doom (USA) (Rev 1).srm
Doom (USA) (Rev 1) (Track 1).bin
...
Doom (USA) (Rev 1) (Track 6).bin
```

`list_installed_games` collapses this into one `GameEntry` by:

1. Treating files whose extension is in a system's `primary_exts` (`.cue`,
   `.gdi`, `.m3u`, ...) as **anchors** — the file that names the game.
   Systems with no `primary_exts` (arcade, gba, megadrive, n64) treat every
   file as its own anchor, i.e. one file = one entry.
2. Assigning every file to the **longest** anchor stem that is a
   "grouping-prefix" of its own stem — a prefix match where the remainder
   starts at a clear boundary (space, `_`, `-`, `.`) rather than continuing
   the same word. That boundary check is what stops `Doom` from swallowing
   an unrelated `Doomsday`, and the longest-match rule is what lets
   `Final Fantasy VII (Disc 2) (Track 1).bin` prefer the `(Disc 2)` anchor
   over a shorter `(Disc 1)` one if both exist.
3. `uninstall_game` deletes exactly the file list captured when the entry
   was built — no re-scanning, so there's no window for a second game's
   files to get swept up if the directory changes between list and delete.

This is heuristic, not a parser of `.cue`/`.m3u` file contents, so it works
for standard No-Intro/Redump-style naming but won't, say, tie two discs of
the same game together unless an `.m3u` anchors them.

## Extensions

Pulled from RetroPie's own system docs / `platforms.cfg`, not guessed —
see the comments in `systems.rs` for per-system sourcing notes and the
priority order used for multi-file anchors.

## Building

```
cargo build --release
```

Notes:

- **GPUI is pre-1.0** and its element-building API changes between Zed
  releases faster than crates.io tags land. If `cargo build` complains
  about missing methods on `div()`/`Context`/etc., check
  https://github.com/zed-industries/zed/tree/main/crates/gpui for the
  current shape and/or pin a git rev in `Cargo.toml` (commented example
  included there). This project was written against the API as documented
  in early 2026; I have not been able to compile it in this environment
  (no network access here), so treat `ui/root.rs` as a strong first draft
  to build against, not a guaranteed-green build.
- Linux system deps: GPUI needs a working OpenGL/Vulkan stack and (per
  Zed's docs) `libxkbcommon`, `libasound2`, `libssl`, `fontconfig`. `rfd`'s
  native file picker uses a desktop portal on Wayland or GTK3 on X11 — if
  neither is available it falls back to a plain Zenity/kdialog prompt.
- 7z extraction fallback (multi-part/split archives) needs `p7zip-full`
  installed; `unzip` fallback needs the `unzip` binary. Both are optional —
  the `zip` crate handles the vast majority of downloads on its own.

## Not yet wired up

- OS-level drag-and-drop of a file onto the dropzone (currently it's a
  click target that opens the native picker — functionally equivalent, just
  not literal drag-and-drop).
- `.m3u` multi-disc playlists aren't parsed for their member discs, so two
  discs of one game only merge into one entry if you generate/ship an
  `.m3u` that RetroPie itself would also use as the anchor.
- Activity log / toast on install-uninstall (the Python prototype had a
  persistent log pane; this draft shows the last action inline instead).
