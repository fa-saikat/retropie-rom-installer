//! Start a game straight from the app, without EmulationStation, using the
//! emulator RetroPie is set up to use for that system.
//!
//! EmulationStation launches through `runcommand.sh`, which needs a text
//! console for its menu. From a desktop app we skip that and do the one
//! part that matters ourselves: read the system's
//! `/opt/retropie/configs/<system>/emulators.cfg`, take the `default`
//! emulator's command line, and fill in its `%ROM%` token the way
//! runcommand does. Same emulator, same config files, same saves.
//!
//! Like `library`, no GPUI dependency, so it's unit tested with `cargo test`.

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::systems::SystemDef;

const RETROPIE_CONFIGS: &str = "/opt/retropie/configs";

/// Debug builds can point at a stand-in with ROM_MANAGER_CONFIGS=<dir>, so
/// Play can be tried on a machine without RetroPie.
fn configs_dir() -> PathBuf {
    let custom = cfg!(debug_assertions).then(|| std::env::var_os("ROM_MANAGER_CONFIGS")).flatten();
    custom.map(PathBuf::from).unwrap_or_else(|| PathBuf::from(RETROPIE_CONFIGS))
}

/// A system's `emulators.cfg`: which emulator is the default, and the
/// command line for each one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Emulators {
    pub default: Option<String>,
    pub commands: Vec<(String, String)>,
}

impl Emulators {
    pub fn default_command(&self) -> Option<(&str, &str)> {
        let name = self.default.as_deref()?;
        self.commands
            .iter()
            .find(|(n, _)| n == name)
            .map(|(n, c)| (n.as_str(), c.as_str()))
    }
}

/// `key = "value"` lines; `default` names the emulator, the rest are
/// emulator → command.
pub fn parse_emulators(text: &str) -> Emulators {
    let mut emulators = Emulators::default();
    for line in text.lines().map(str::trim) {
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim();
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(value)
            .to_string();
        if key == "default" {
            emulators.default = Some(value);
        } else if !key.is_empty() {
            emulators.commands.push((key.to_string(), value));
        }
    }
    emulators
}

pub fn emulators_for(system: &SystemDef) -> Option<Emulators> {
    let path = configs_dir().join(system.folder).join("emulators.cfg");
    let emulators = parse_emulators(&fs::read_to_string(path).ok()?);
    emulators.default_command().is_some().then_some(emulators)
}

/// RetroPie has an emulator set up for `system`, so its games can be run.
pub fn can_run(system: &SystemDef) -> bool {
    emulators_for(system).is_some()
}

/// Single-quote for bash.
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// The shell command for `rom`, built the way runcommand.sh builds it.
pub fn command_line(command: &str, rom: &Path) -> Result<String> {
    // runcommand prefixes: CON: (console app), XINIT…: (needs its own X
    // server). We're already on a desktop, so they only need dropping.
    let mut command = command;
    for prefix in ["CON:", "XINIT-WMC:", "XINIT-WM:", "XINIT:"] {
        if let Some(rest) = command.strip_prefix(prefix) {
            command = rest;
            break;
        }
    }
    // Anything besides %ROM% / %BASENAME% (%XRES%, %YRES%, …) depends on
    // runcommand's video-mode handling, which we don't reproduce.
    if let Some(token) = tokens(command).find(|t| *t != "%ROM%" && *t != "%BASENAME%") {
        return Err(anyhow!(
            "this emulator needs RetroPie's launcher ({token}); start the game from EmulationStation"
        ));
    }
    let basename = rom.file_stem().unwrap_or_default().to_string_lossy();
    let line = command
        .replace("%ROM%", &quote(&rom.to_string_lossy()))
        .replace("%BASENAME%", &quote(&basename));
    Ok(line)
}

/// `%UPPER_CASE%` placeholders in a command template.
fn tokens(command: &str) -> impl Iterator<Item = &str> {
    let mut rest = command;
    std::iter::from_fn(move || loop {
        let start = rest.find('%')?;
        let after = &rest[start + 1..];
        let len = after.find('%')?;
        let name = &after[..len];
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
            let token = &rest[start..start + len + 2];
            rest = &after[len + 1..];
            return Some(token);
        }
        rest = after;
    })
}

/// A game that's been started.
pub struct Running {
    pub emulator: String,
    pub child: Child,
    /// The emulator's output, for explaining a crash.
    pub log: PathBuf,
}

pub fn start(system: &SystemDef, rom: &Path) -> Result<Running> {
    let emulators = emulators_for(system).ok_or_else(|| {
        anyhow!("RetroPie has no emulator set up for {} (install one in RetroPie-Setup)", system.display_name)
    })?;
    let (emulator, command) = emulators.default_command().expect("checked by emulators_for");
    let line = command_line(command, rom)?;

    let log = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("retropie-rom-manager")
        .join("last-run.log");
    if let Some(dir) = log.parent() {
        fs::create_dir_all(dir)?;
    }
    let out = fs::File::create(&log)?;
    let child = Command::new("bash")
        .arg("-c")
        .arg(&line)
        .current_dir(dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")))
        .stdin(Stdio::null())
        .stdout(out.try_clone()?)
        .stderr(out)
        .spawn()
        .with_context(|| format!("couldn't start {emulator}"))?;
    Ok(Running { emulator: emulator.to_string(), child, log })
}

/// Last few lines of a run's output, for an error message.
pub fn log_tail(log: &Path, lines: usize) -> String {
    let text = fs::read_to_string(log).unwrap_or_default();
    let all: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNES_CFG: &str = r#"
lr-snes9x2010 = "/opt/retropie/emulators/retroarch/bin/retroarch -L /opt/retropie/libretrocores/lr-snes9x2010/snes9x2010_libretro.so --config /opt/retropie/configs/snes/retroarch.cfg %ROM%"
lr-snes9x = "/opt/retropie/emulators/retroarch/bin/retroarch -L /opt/retropie/libretrocores/lr-snes9x/snes9x_libretro.so --config /opt/retropie/configs/snes/retroarch.cfg %ROM%"
default = "lr-snes9x"
"#;

    #[test]
    fn picks_the_default_emulator() {
        let emulators = parse_emulators(SNES_CFG);
        let (name, command) = emulators.default_command().unwrap();
        assert_eq!(name, "lr-snes9x");
        assert!(command.contains("snes9x_libretro.so"));
        assert!(parse_emulators("lr-x = \"x %ROM%\"").default_command().is_none());
    }

    #[test]
    fn fills_in_the_rom_safely() {
        let rom = Path::new("/home/pi/RetroPie/roms/snes/Kirby's Dream Land $(rm -rf).sfc");
        let line = command_line("retroarch -L core.so %ROM%", rom).unwrap();
        assert_eq!(line, r"retroarch -L core.so '/home/pi/RetroPie/roms/snes/Kirby'\''s Dream Land $(rm -rf).sfc'");
        let line = command_line("XINIT:dosbox %BASENAME%", Path::new("/r/Doom.exe")).unwrap();
        assert_eq!(line, "dosbox 'Doom'");
    }

    #[test]
    fn refuses_tokens_only_runcommand_understands() {
        assert!(command_line("drastic --res %XRES%x%YRES% %ROM%", Path::new("/r/a.nds")).is_err());
        // A literal percent sign in a path is fine.
        assert!(command_line("emu %ROM%", Path::new("/r/100% Orange Juice.iso")).is_ok());
    }
}
