//! Installing Skyscraper from inside the app, the same way RetroPie's own
//! menu does: `sudo ~/RetroPie-Setup/retropie_packages.sh skyscraper _auto_`
//! (`_auto_` = RetroPie's prebuilt binary if there is one for this
//! platform, otherwise build from source).
//!
//! The script refuses to run without root and installs for `$__user`
//! (defaulting to `$SUDO_USER`), so we pass the real user explicitly —
//! otherwise a `pkexec` run would install and configure Skyscraper for root.
//!
//! No GPUI dependency, like `library` and `scraper`.

use anyhow::{anyhow, Result};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::scraper::{self, strip_ansi};

/// How we get root for RetroPie-Setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevation {
    /// `sudo -n` works without a password (the RetroPie default for `pi`).
    Sudo,
    /// polkit's `pkexec`: the desktop shows its own password prompt.
    Pkexec,
    /// Already root, or (debug builds only) told to skip elevation.
    None,
}

/// `~/RetroPie-Setup/retropie_packages.sh`, if this is a RetroPie install.
pub fn retropie_setup_script() -> Option<PathBuf> {
    let script = dirs::home_dir()?.join("RetroPie-Setup").join("retropie_packages.sh");
    script.is_file().then_some(script)
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(program).is_file()))
        .unwrap_or(false)
}

/// Pick the least intrusive way to get root that will actually work.
pub fn elevation() -> Option<Elevation> {
    if cfg!(debug_assertions) && std::env::var_os("ROM_MANAGER_SETUP_NO_ELEVATE").is_some() {
        return Some(Elevation::None);
    }
    let passwordless_sudo = Command::new("sudo")
        .args(["-n", "true"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if passwordless_sudo {
        Some(Elevation::Sudo)
    } else if on_path("pkexec") {
        Some(Elevation::Pkexec)
    } else {
        None
    }
}

fn current_user() -> Option<String> {
    std::env::var("USER").ok().filter(|u| !u.is_empty()).or_else(|| {
        let out = Command::new("id").arg("-un").output().ok()?;
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|u| !u.is_empty())
    })
}

/// Program + arguments for the install run.
pub fn install_command(script: &Path, user: &str, elevation: Elevation) -> (OsString, Vec<OsString>) {
    let mut args: Vec<OsString> = vec![
        "env".into(),
        format!("__user={user}").into(),
        script.as_os_str().to_owned(),
        "skyscraper".into(),
        "_auto_".into(),
    ];
    match elevation {
        Elevation::Sudo => {
            args.insert(0, "-n".into());
            ("sudo".into(), args)
        }
        Elevation::Pkexec => ("pkexec".into(), args),
        Elevation::None => (args.remove(0), args),
    }
}

/// What to show when the app can't install it by itself.
pub fn manual_command() -> &'static str {
    "sudo ~/RetroPie-Setup/retropie_packages.sh skyscraper"
}

/// Run `program`, calling `on_line` with each non-empty output line (ANSI
/// colours stripped). On failure the error carries the last few lines.
pub fn run_streaming(program: &OsString, args: &[OsString], on_line: &dyn Fn(&str)) -> Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("couldn't start {}: {e}", program.to_string_lossy()))?;

    // stderr on its own thread so neither pipe can fill up and stall the run.
    let mut stderr = child.stderr.take().expect("piped stderr");
    let stderr_reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });

    let mut tail: VecDeque<String> = VecDeque::with_capacity(8);
    for line in BufReader::new(child.stdout.take().expect("piped stdout")).lines() {
        let Ok(line) = line else { break };
        let line = strip_ansi(&line).trim().to_string();
        if line.is_empty() {
            continue;
        }
        on_line(&line);
        if tail.len() == 8 {
            tail.pop_front();
        }
        tail.push_back(line);
    }
    let status = child.wait()?;
    let stderr = stderr_reader.join().unwrap_or_default();
    if status.success() {
        return Ok(());
    }
    let detail = stderr
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map(str::to_string)
        .or_else(|| tail.back().cloned())
        .unwrap_or_else(|| "no output".into());
    Err(anyhow!("exited with {:?}: {detail}", status.code()))
}

/// Install Skyscraper via RetroPie-Setup. Blocking — call from a background
/// task. `on_line` gets the installer's output as it runs.
pub fn install(on_line: &dyn Fn(&str)) -> Result<()> {
    let script = retropie_setup_script()
        .ok_or_else(|| anyhow!("RetroPie-Setup wasn't found in your home folder"))?;
    let user = current_user().ok_or_else(|| anyhow!("couldn't tell which user to install for"))?;
    let elevation = elevation().ok_or_else(|| {
        anyhow!("no way to get administrator rights (needs passwordless sudo or pkexec) — run `{}` in a terminal", manual_command())
    })?;
    let (program, args) = install_command(&script, &user, elevation);
    run_streaming(&program, &args, on_line)
        .map_err(|e| anyhow!("RetroPie-Setup failed: {e}"))?;
    if scraper::skyscraper_binary().is_none() {
        return Err(anyhow!("RetroPie-Setup finished, but Skyscraper still isn't on this system"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn strings(args: &[OsString]) -> Vec<String> {
        args.iter().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn sudo_install_matches_retropie_and_keeps_the_user() {
        let script = Path::new("/home/pi/RetroPie-Setup/retropie_packages.sh");
        let (program, args) = install_command(script, "pi", Elevation::Sudo);
        assert_eq!(program, "sudo");
        assert_eq!(
            strings(&args),
            ["-n", "env", "__user=pi", "/home/pi/RetroPie-Setup/retropie_packages.sh", "skyscraper", "_auto_"]
        );
    }

    #[test]
    fn pkexec_install_passes_the_user_explicitly() {
        let script = Path::new("/home/pi/RetroPie-Setup/retropie_packages.sh");
        let (program, args) = install_command(script, "pi", Elevation::Pkexec);
        assert_eq!(program, "pkexec");
        assert_eq!(strings(&args)[..2], ["env", "__user=pi"]);
    }

    #[test]
    fn unelevated_runs_the_script_through_env() {
        let (program, args) = install_command(Path::new("/x.sh"), "pi", Elevation::None);
        assert_eq!(program, "env");
        assert_eq!(strings(&args), ["__user=pi", "/x.sh", "skyscraper", "_auto_"]);
    }

    #[test]
    fn streams_lines_and_reports_failures() {
        let seen = Mutex::new(Vec::new());
        let sh: OsString = "sh".into();
        let ok = run_streaming(
            &sh,
            &["-c".into(), "printf '\\033[1;32mBuilding\\033[0m\\n\\nDone\\n'".into()],
            &|l| seen.lock().unwrap().push(l.to_string()),
        );
        assert!(ok.is_ok());
        assert_eq!(*seen.lock().unwrap(), ["Building", "Done"]);

        let err = run_streaming(&sh, &["-c".into(), "echo boom >&2; exit 3".into()], &|_| {}).unwrap_err();
        assert!(err.to_string().contains("boom"), "{err}");
    }
}
