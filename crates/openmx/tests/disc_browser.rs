//! P220: the menu's own disc browser. A player's start (no kit.toml above the working directory) with
//! no disc remembered, no dialog program on the PATH and no terminal on stdin opens a page that walks
//! the folders for the `.iso` with the pad. Here a stand-in pad (`OPENMX_PAD_ROOT`, laid out as
//! drop_in.rs's) walks from the home folder into `Games`, tries an image that is not the game's, which
//! is refused, and takes the disc; the shell remembers it, starts over as a fresh process, and that
//! one's `--where` says the disc came from the path remembered.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

const O_NONBLOCK: i32 = 0o4000;
/// The joystick interface's event kinds, and its initial-state flag.
const BUTTON: u8 = 0x01;
const AXIS: u8 = 0x02;
const INIT: u8 = 0x80;
/// The js numbers an Xbox pad's buttons come as: A is Cross.
const CROSS: u8 = 0;

/// How long a step may take in a debug-ish build under a busy gate before the case fails.
const PATIENCE: Duration = Duration::from_secs(120);

/// A `/dev/input/jsN` stand-in: a fifo the test writes joystick events into.
struct Pad {
    _hold: std::fs::File,
    writer: std::fs::File,
    born: Instant,
}

impl Pad {
    fn plug(root: &Path) -> Pad {
        let dev = root.join("dev");
        let sys = root.join("sys/js0/device");
        std::fs::create_dir_all(&dev).unwrap();
        std::fs::create_dir_all(sys.join("id")).unwrap();
        for (file, word) in [("name", "Pad 0"), ("id/vendor", "045e"), ("id/product", "028e"), ("uniq", ""), ("phys", "usb-1/input0")] {
            std::fs::write(sys.join(file), format!("{word}\n")).unwrap();
        }
        let path = dev.join("js0");
        assert!(Command::new("mkfifo").arg(&path).status().expect("mkfifo").success());
        let hold = std::fs::OpenOptions::new().read(true).custom_flags(O_NONBLOCK).open(&path).unwrap();
        let writer = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        let mut pad = Pad { _hold: hold, writer, born: Instant::now() };
        for b in 0..11 {
            pad.send(BUTTON | INIT, b, 0);
        }
        // The sticks at rest and the triggers let go.
        for (a, v) in [0, 0, -32767, 0, 0, -32767, 0, 0].into_iter().enumerate() {
            pad.send(AXIS | INIT, a as u8, v);
        }
        pad
    }

    fn send(&mut self, kind: u8, number: u8, value: i16) {
        let ms = self.born.elapsed().as_millis() as u32;
        let [t0, t1, t2, t3] = ms.to_le_bytes();
        let [lo, hi] = value.to_le_bytes();
        self.writer.write_all(&[t0, t1, t2, t3, lo, hi, kind, number]).unwrap();
    }

    /// A press as a hand makes one: down, a tenth of a second, up, and a rest before the next.
    fn press(&mut self, button: u8) {
        self.send(BUTTON, button, 1);
        std::thread::sleep(Duration::from_millis(120));
        self.send(BUTTON, button, 0);
        std::thread::sleep(Duration::from_millis(400));
    }

    /// The d-pad's down, which the js interface gives as its axis 7.
    fn down(&mut self) {
        self.send(AXIS, 7, 32767);
        std::thread::sleep(Duration::from_millis(120));
        self.send(AXIS, 7, 0);
        std::thread::sleep(Duration::from_millis(400));
    }
}

/// The disc this checkout's kit.toml names.
fn disc() -> Option<PathBuf> {
    match game_render::disc::configured_image() {
        Ok(p) if p.is_file() => Some(std::path::absolute(p).unwrap()),
        _ => {
            eprintln!("skipped: no disc");
            None
        }
    }
}

/// A fresh folder with no kit.toml in it or above it, as player_start.rs's.
fn player_folder() -> Option<PathBuf> {
    let has_kit = |d: &Path| d.ancestors().any(|a| a.join("kit.toml").is_file());
    let base = [std::env::temp_dir(), PathBuf::from("/tmp"), PathBuf::from("/dev/shm")]
        .into_iter()
        .find(|d| d.is_dir() && !has_kit(d));
    let Some(base) = base else {
        eprintln!("skipped: every temp folder has a kit.toml above it");
        return None;
    };
    let dir = base.join(format!("openmx-p220-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("cwd")).unwrap();
    Some(dir)
}

#[test]
fn the_pad_walks_to_the_disc_and_the_shell_starts_from_it() {
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("skipped: no display for the browser's window");
        return;
    }
    let (Some(iso), Some(dir)) = (disc(), player_folder()) else { return };
    // home: Games (the disc, and an image that is not the game's, which sorts first) and Music
    let games = dir.join("home/Games");
    std::fs::create_dir_all(&games).unwrap();
    std::fs::create_dir_all(dir.join("home/Music")).unwrap();
    std::fs::write(games.join("A not a disc.iso"), b"nothing").unwrap();
    std::os::unix::fs::symlink(&iso, games.join("MX Unleashed.iso")).unwrap();
    let mut pad = Pad::plug(&dir.join("pads"));
    let empty = dir.join("no-programs");
    std::fs::create_dir_all(&empty).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .arg("--where")
        .current_dir(dir.join("cwd"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("HOME", dir.join("home"))
        .env("PATH", &empty)
        .env("OPENMX_PAD_ROOT", dir.join("pads"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the shell runs");
    let out = child.stdout.take().unwrap();
    let (send, lines) = channel();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if send.send(line).is_err() {
                break;
            }
        }
    });
    let mut said = Vec::new();
    let mut until = |what: &str, like: &dyn Fn(&str) -> bool, child: &mut std::process::Child| -> String {
        let deadline = Instant::now() + PATIENCE;
        loop {
            match lines.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(line) => {
                    said.push(line.clone());
                    if like(&line) {
                        return line;
                    }
                }
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let mut err = String::new();
                    if let Some(mut e) = child.stderr.take() {
                        let _ = std::io::Read::read_to_string(&mut e, &mut err);
                    }
                    panic!("no {what} line; the shell said:\n{}\nstderr:\n{err}", said.join("\n"));
                }
            }
        }
    };
    let home = dir.join("home");
    let browsing = until("browsing", &|l| l.starts_with("disc: none, browsing"), &mut child);
    assert!(browsing.ends_with(&home.display().to_string()), "{browsing}");
    // the page's first frames, and the pad read, before the first press
    std::thread::sleep(Duration::from_secs(1));
    // home lights Games (its first folder); Cross opens it, which lights its first image
    pad.press(CROSS);
    pad.press(CROSS);
    let refused = until("refused", &|l| l.contains("won't open"), &mut child);
    assert!(refused.contains("A not a disc.iso"), "{refused}");
    pad.down();
    pad.press(CROSS);
    // the pick is remembered and the shell starts over from it
    let picked = until("disc", &|l| l.starts_with("disc: /"), &mut child);
    assert_eq!(picked, format!("disc: {} (from the path remembered)", games.join("MX Unleashed.iso").display()));
    let status = child.wait().unwrap();
    assert!(status.success(), "{status}");
    let kept = std::fs::read_to_string(dir.join("data/openmx/disc.txt")).unwrap();
    assert!(kept.lines().any(|l| Path::new(l) == games.join("MX Unleashed.iso")), "{kept}");
    let _ = std::fs::remove_dir_all(&dir);
}
