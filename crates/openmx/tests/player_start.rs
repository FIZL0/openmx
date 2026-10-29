//! P190: a desktop build a player can start. With no kit.toml above the working directory the shell
//! takes its disc from its first argument and remembers it, starts from the remembered one next time,
//! and keeps settings.txt in the per-user data folder; a dev checkout's kit.toml works as before.
//! Each case runs the shell itself, `--where`, in a folder of its own with `XDG_DATA_HOME` there too.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The disc this checkout's kit.toml names, or the reason to skip.
fn disc() -> Option<PathBuf> {
    match game_render::disc::configured_image() {
        Ok(p) if p.is_file() => Some(std::path::absolute(p).unwrap()),
        Ok(p) => {
            eprintln!("skipped: no disc at {}", p.display());
            None
        }
        Err(e) => {
            eprintln!("skipped: {e}");
            None
        }
    }
}

/// A fresh folder with no kit.toml in it or above it: the player's working directory.
fn player_folder(name: &str) -> Option<PathBuf> {
    let has_kit = |d: &Path| d.ancestors().any(|a| a.join("kit.toml").is_file());
    let base = [std::env::temp_dir(), PathBuf::from("/tmp"), PathBuf::from("/dev/shm")]
        .into_iter()
        .find(|d| d.is_dir() && !has_kit(d));
    let Some(base) = base else {
        eprintln!("skipped: every temp folder has a kit.toml above it");
        return None;
    };
    let dir = base.join(format!("openmx-p190-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("cwd")).unwrap();
    Some(dir)
}

/// `openmx ARGS --where` from `dir/cwd`, its data under `dir/data`: its lines, or its error.
fn run(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .args(args)
        .arg("--where")
        .current_dir(dir.join("cwd"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("HOME", dir.join("home"))
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    if out.status.success() {
        Ok(text)
    } else {
        Err(format!("{text}{}", String::from_utf8_lossy(&out.stderr)))
    }
}

fn line<'a>(text: &'a str, key: &str) -> &'a str {
    text.lines().find_map(|l| l.strip_prefix(key)).unwrap_or_else(|| panic!("no {key} line in:\n{text}"))
}

#[test]
fn the_disc_from_the_command_line_then_from_the_path_remembered() {
    let (Some(iso), Some(dir)) = (disc(), player_folder("argv")) else { return };
    let data = dir.join("data").join("openmx");
    let arg = iso.to_str().unwrap();

    let first = run(&dir, &[arg]).unwrap();
    assert_eq!(line(&first, "disc: "), format!("{} (from the command line)", iso.display()));
    assert_eq!(line(&first, "data: "), data.display().to_string());
    let kept = std::fs::read_to_string(data.join("disc.txt")).unwrap();
    assert!(kept.lines().any(|l| l == arg), "disc.txt remembers the disc: {kept}");

    // no argument now: the one remembered
    let second = run(&dir, &[]).unwrap();
    assert_eq!(line(&second, "disc: "), format!("{} (from the path remembered)", iso.display()));

    // a disc that won't open is an error, not a remembered path
    let bad = dir.join("cwd").join("not a disc.iso");
    std::fs::write(&bad, b"nothing").unwrap();
    let err = run(&dir, &[bad.to_str().unwrap()]).unwrap_err();
    assert!(err.contains("won't open"), "{err}");
    assert!(std::fs::read_to_string(data.join("disc.txt")).unwrap().lines().any(|l| l == arg));

    // nothing remembered and no picker to ask (stdin is no terminal, the test's PATH may have a
    // dialog, so empty it): it says how to start it
    std::fs::remove_file(data.join("disc.txt")).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .arg("--where")
        .current_dir(dir.join("cwd"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("PATH", "")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no disc: start openMX with your own"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_data_folders_settings_when_no_kit_toml_is_found() {
    let (Some(iso), Some(dir)) = (disc(), player_folder("data")) else { return };
    let data = dir.join("data").join("openmx");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(data.join("settings.txt"), "name = Data Folder\n").unwrap();
    // settings beside the disc, where a dev checkout keeps them, are not the player's
    let out = run(&dir, &[iso.to_str().unwrap()]).unwrap();
    assert_eq!(line(&out, "settings: "), data.join("settings.txt").display().to_string());
    assert_eq!(line(&out, "rider: "), "Data Folder");
    // without XDG_DATA_HOME, ~/.local/share/openmx
    if cfg!(target_os = "linux") {
        let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
            .args([iso.to_str().unwrap(), "--where"])
            .current_dir(dir.join("cwd"))
            .env_remove("XDG_DATA_HOME")
            .env("HOME", dir.join("home"))
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert_eq!(line(&text, "data: "), dir.join("home/.local/share/openmx").display().to_string());
        assert!(dir.join("home/.local/share/openmx/disc.txt").is_file());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_dev_checkouts_kit_toml_keeps_working() {
    let Some(iso) = disc() else { return };
    // the test's own working directory is the crate's, under this checkout's kit.toml
    let toml = game_render::disc::kit_toml().expect("a kit.toml above the crate");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx")).arg("--where").stdin(std::process::Stdio::null()).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{text}{}", String::from_utf8_lossy(&out.stderr));
    assert!(line(&text, "disc: ").ends_with("(from kit.toml)"), "{text}");
    assert_eq!(line(&text, "data: "), "none, a dev checkout's kit.toml");
    // settings beside the disc, as before P190
    let settings = PathBuf::from(line(&text, "settings: "));
    assert_eq!(settings.parent(), iso.parent());
    assert!(toml.is_file());
}
