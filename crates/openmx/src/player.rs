//! A desktop build a player can start (P190), as opentennis's `hst-base/src/disc.rs` starts its own.
//!
//! The disc is the shell's first argument when it has one (a file dropped on the binary arrives the
//! same way); else, in a dev checkout, `kit.toml`'s `[game] iso` as always; else the path the last run
//! remembered; else a file dialog asks (zenity or kdialog, PowerShell's on Windows, AppleScript's on a
//! Mac), or the terminal when there is no dialog to show. With no `kit.toml` above the working
//! directory the shell's own files — `settings.txt`, `controls.txt`, `mods/`, the picks and markers —
//! live in the per-user data folder ([`data_dir`]), and the disc's path is remembered there in
//! [`REMEMBERED`].

use game_render::disc::Disc;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The file in the data folder holding the last disc's path, one line.
pub const REMEMBERED: &str = "disc.txt";

/// Where the disc came from, as `--where` says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Argument,
    KitToml,
    Remembered,
    Picked,
}

impl Source {
    pub fn word(self) -> &'static str {
        match self {
            Source::Argument => "the command line",
            Source::KitToml => "kit.toml",
            Source::Remembered => "the path remembered",
            Source::Picked => "the picker",
        }
    }
}

/// What [`start`] decided: where the disc came from, and the data folder when the shell is a
/// player's (no `kit.toml`), `None` in a dev checkout.
pub struct Start {
    pub from: Source,
    pub data: Option<PathBuf>,
}

/// The per-user data folder: `$XDG_DATA_HOME/openmx` (else `~/.local/share/openmx`), `%APPDATA%\openmx`
/// on Windows, `~/Library/Application Support/openmx` on a Mac. `env` reads the environment.
pub fn data_dir_in(env: impl Fn(&str) -> Option<String>) -> PathBuf {
    let env = |k: &str| env(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        return env("APPDATA").unwrap_or_default().join("openmx");
    }
    let home = env("HOME").unwrap_or_default();
    if cfg!(target_os = "macos") {
        return home.join("Library/Application Support/openmx");
    }
    env("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share")).join("openmx")
}

/// [`data_dir_in`] this process's environment.
pub fn data_dir() -> PathBuf {
    data_dir_in(|k| std::env::var(k).ok())
}

/// The disc a command line names: its first word when that is not a flag. Taken off `argv`, so the
/// flags parse as they always have.
pub fn take_disc(argv: &mut Vec<String>) -> Option<PathBuf> {
    argv.first().is_some_and(|a| !a.starts_with('-')).then(|| PathBuf::from(argv.remove(0)))
}

/// The disc the data folder remembers, if its file says one.
pub fn remembered(data: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(data.join(REMEMBERED)).ok()?;
    let line = text.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#'))?;
    Some(PathBuf::from(line))
}

/// Remember `disc` in the data folder for the next start.
pub fn remember(data: &Path, disc: &Path) -> Result<(), String> {
    std::fs::create_dir_all(data).map_err(|e| format!("{}: {e}", data.display()))?;
    let disc = std::path::absolute(disc).unwrap_or_else(|_| disc.to_path_buf());
    let path = data.join(REMEMBERED);
    let text = format!("# the disc openMX last started from; delete this file to be asked again\n{}\n", disc.display());
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Whether `path` is a disc the shell can read: an ISO 9660 image with `MX.PSR` in it.
fn opens(path: &Path) -> Result<(), String> {
    Disc::open(path).map(drop)
}

/// Choose the disc and the data folder (see the module doc) and set both for the rest of the run:
/// `game_render::disc::set_player_disc` and `menu::set_data_dir`. `disc` is [`take_disc`]'s, `kit`
/// whether a `kit.toml` was found, `data` the data folder and `ask` the dialog.
pub fn start_with(disc: Option<PathBuf>, kit: bool, data: PathBuf, ask: impl FnOnce() -> Option<PathBuf>) -> Result<Start, String> {
    let data = (!kit).then_some(data);
    if let Some(dir) = &data {
        crate::menu::set_data_dir(dir.clone());
    }
    let (disc, from) = if let Some(disc) = disc {
        opens(&disc).map_err(|e| format!("the disc on the command line won't open: {e}"))?;
        (disc, Source::Argument)
    } else if kit {
        return Ok(Start { from: Source::KitToml, data });
    } else {
        let dir = data.as_deref().expect("no kit.toml, so a data folder");
        match remembered(dir).filter(|d| opens(d).is_ok()) {
            Some(disc) => (disc, Source::Remembered),
            None => {
                let disc = ask().ok_or_else(|| {
                    format!(
                        "no disc: start openMX with your own MX Unleashed (USA, SLUS-20767) image, \
                         `openmx \"/path/to/MX Unleashed (USA).iso\"`, or drop the .iso on it. \
                         It is remembered in {} after that.",
                        dir.join(REMEMBERED).display()
                    )
                })?;
                opens(&disc).map_err(|e| format!("the disc picked won't open: {e}"))?;
                (disc, Source::Picked)
            }
        }
    };
    if let Some(dir) = &data {
        if from != Source::Remembered {
            remember(dir, &disc)?;
        }
    }
    let disc = std::path::absolute(&disc).unwrap_or(disc);
    game_render::disc::set_player_disc(disc);
    Ok(Start { from, data })
}

/// [`start_with`] for this process: its command line, the working directory's `kit.toml`, the
/// per-user data folder and a dialog.
pub fn start(argv: &mut Vec<String>) -> Result<Start, String> {
    let disc = take_disc(argv);
    start_with(disc, game_render::disc::kit_toml().is_some(), data_dir(), ask)
}

/// The disc from a file dialog, if there is one to show and a file was picked; else from the
/// terminal, if the shell was started in one.
pub fn ask() -> Option<PathBuf> {
    let title = "openMX: pick your MX Unleashed disc image (SLUS-20767)";
    let tries: Vec<(&str, Vec<&str>)> = if cfg!(windows) {
        let script = "Add-Type -AssemblyName System.Windows.Forms; \
                      $d = New-Object System.Windows.Forms.OpenFileDialog; \
                      $d.Title = 'openMX: pick your MX Unleashed disc image (SLUS-20767)'; \
                      $d.Filter = 'Disc images (*.iso)|*.iso|All files (*.*)|*.*'; \
                      if ($d.ShowDialog() -eq 'OK') { $d.FileName }";
        vec![("powershell", vec!["-NoProfile", "-STA", "-Command", script])]
    } else if cfg!(target_os = "macos") {
        let script = "POSIX path of (choose file with prompt \"openMX: pick your MX Unleashed disc image (SLUS-20767)\")";
        vec![("osascript", vec!["-e", script])]
    } else {
        vec![
            ("zenity", vec!["--file-selection", "--title", title, "--file-filter", "Disc image | *.iso *.ISO", "--file-filter", "All files | *"]),
            ("kdialog", vec!["--title", title, "--getopenfilename", ".", "*.iso *.ISO"]),
        ]
    };
    let mut shown = false;
    for (cmd, args) in &tries {
        let Ok(out) = Command::new(cmd).args(args).output() else { continue };
        shown = true;
        let picked = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if out.status.success() && !picked.is_empty() {
            return Some(PathBuf::from(picked));
        }
    }
    // a dialog that was shown and cancelled is the player's answer: no terminal question after it
    use std::io::{BufRead, IsTerminal, Write};
    if shown || !std::io::stdin().is_terminal() {
        return None;
    }
    print!("{title}\nits path: ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).ok()?;
    let line = line.trim().trim_matches(['"', '\'']);
    (!line.is_empty()).then(|| PathBuf::from(line))
}
