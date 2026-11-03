//! P271: a second player joining a running ride goes to split screen and the shell stays up.
//!
//! The Steam Deck went down as player 2 came in. The cause: the tick a player joined on was sent to
//! the sim thread (N23a: always under `--sim-thread on`, and under `auto` once two seats ride and the
//! ticks do not fit beside the pictures, which a Deck's do not), so its second half, which lays out the
//! views' riders and cameras, waited for the picture after it, and that picture drew two views from
//! the one rider the tick before had: an index past the first seat. And a window reporting no size (a
//! headless run with no display) split a 1x1 frame into views no pixel wide.
//!
//! Each case runs the shell itself, headless, from the front end (`--menu --screen pause`, the pause
//! over a Free Ride on the settings' track), with two stand-in pads (`OPENMX_PAD_ROOT`, laid out as
//! pad.rs's test fakes are): pad 1 rides from the start, and pad 2 is plugged in mid-ride and presses
//! Cross. Over the pause, and riding (pad 1's Start closes it first), with the sim thread on and off.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// One shell at a time: each draws two views, and two at once under a busy gate only slow both.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

const O_NONBLOCK: i32 = 0o4000;
/// The joystick interface's event kinds, and its initial-state flag.
const BUTTON: u8 = 0x01;
const AXIS: u8 = 0x02;
const INIT: u8 = 0x80;
/// The js numbers an Xbox pad's buttons come as, which a node's sysfs entry with no keys reads by.
const CROSS: u8 = 0;
const START: u8 = 7;

/// How long a step may take in a debug-ish build under a busy gate before the case fails.
const PATIENCE: Duration = Duration::from_secs(120);

/// A `/dev/input/jsN` stand-in: a fifo the test writes joystick events into.
struct Pad {
    _hold: std::fs::File,
    writer: std::fs::File,
    born: Instant,
}

impl Pad {
    fn plug(root: &Path, n: usize) -> Pad {
        let node = format!("js{n}");
        let dev = root.join("dev");
        let sys = root.join("sys").join(&node).join("device");
        std::fs::create_dir_all(&dev).unwrap();
        std::fs::create_dir_all(sys.join("id")).unwrap();
        for (file, word) in [
            ("name", format!("Pad {n}")),
            ("id/vendor", "045e".into()),
            ("id/product", "028e".into()),
            ("uniq", String::new()),
            ("phys", format!("usb-{}/input0", n + 1)),
        ] {
            std::fs::write(sys.join(file), format!("{word}\n")).unwrap();
        }
        let path = dev.join(&node);
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

    /// A press as a hand makes one: down, a tenth of a second, up.
    fn press(&mut self, button: u8) {
        self.send(BUTTON, button, 1);
        std::thread::sleep(Duration::from_millis(120));
        self.send(BUTTON, button, 0);
    }
}

/// The running shell and its lines as they come.
struct Shell {
    child: Child,
    lines: Receiver<String>,
    said: Vec<String>,
}

impl Shell {
    /// The next line `like` accepts, or the case fails saying what the shell said.
    fn until(&mut self, what: &str, like: impl Fn(&str) -> bool) -> String {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(line) => {
                    self.said.push(line.clone());
                    if like(&line) {
                        return line;
                    }
                }
                Err(_) => {
                    let _ = self.child.kill();
                    let out = self.child.wait_with_output_now();
                    panic!("no {what} line; the shell said:\n{}\n{out}", self.said.join("\n"));
                }
            }
        }
    }
}

trait Ended {
    fn wait_with_output_now(&mut self) -> String;
}

impl Ended for Child {
    /// Its status and what it wrote to stderr, once it has stopped.
    fn wait_with_output_now(&mut self) -> String {
        let status = self.wait().map_or_else(|e| e.to_string(), |s| s.to_string());
        let mut err = String::new();
        if let Some(mut e) = self.stderr.take() {
            let _ = std::io::Read::read_to_string(&mut e, &mut err);
        }
        format!("status {status}; stderr:\n{err}")
    }
}

/// The disc, linked into a folder of the case's own so its settings.txt is never the user's.
fn folder(name: &str) -> Option<PathBuf> {
    let iso = match game_render::disc::configured_image() {
        Ok(p) if p.is_file() => std::path::absolute(p).unwrap(),
        _ => {
            eprintln!("skipped: no disc");
            return None;
        }
    };
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("p271-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(&iso, dir.join("disc.iso")).unwrap();
    Some(dir)
}

/// A ride with pad 1 in it, `riding` (the pause closed) or not, joined by pad 2 plugged in after
/// it has run a second; the shell is still up after two seconds of the two-way split.
fn second_pad_joins(name: &str, sim_thread: &str, riding: bool) {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let Some(dir) = folder(name) else { return };
    let root = dir.join("pads");
    let mut one = Pad::plug(&root, 0);
    let mut child = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&dir)
        .env("OPENMX_PAD_ROOT", &root)
        .args(["disc.iso", "--headless", "--menu", "--screen", "pause", "--no-sound", "--seconds", "600"])
        .args(["--sim-thread", sim_thread])
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
    let mut shell = Shell { child, lines, said: Vec::new() };
    let fps = |l: &str| l.trim_start().split_once("s ").is_some_and(|(s, rest)| s.parse::<u32>().is_ok() && rest.contains(" fps,"));
    shell.until("first second of the ride", fps);
    if riding {
        one.press(START);
        shell.until("riding (no longer frozen)", |l| fps(l) && !l.contains("(frozen)"));
    }
    let mut two = Pad::plug(&root, 1);
    shell.until("2P", |l| l.starts_with("2P: Pad 1"));
    // The press a moment after the plug, as a hand would: the join reads a press, not a held button.
    std::thread::sleep(Duration::from_millis(300));
    two.press(CROSS);
    let joined = shell.until("join", |l| l.starts_with("players: player 2 joined") || l.starts_with("players: player 2 could not join"));
    assert!(joined.contains("views: 2 players"), "{joined}");
    // Two seconds of the split, every second's line saying so.
    for _ in 0..2 {
        let l = shell.until("split second", fps);
        assert!(l.ends_with(", 2 views"), "{l}");
    }
    let still = shell.child.try_wait().unwrap();
    let _ = shell.child.kill();
    let out = shell.child.wait_with_output_now();
    assert!(still.is_none(), "the shell stopped with the split up: {out}");
    assert!(!out.contains("panicked"), "{out}");
    drop(two);
}

#[test]
fn a_second_pad_joins_mid_ride_on_the_sim_thread() {
    second_pad_joins("riding-on", "on", true);
}

#[test]
fn a_second_pad_joins_mid_ride_on_the_loop() {
    second_pad_joins("riding-off", "off", true);
}

#[test]
fn a_second_pad_joins_over_the_pause_on_the_sim_thread() {
    second_pad_joins("pause-on", "on", false);
}

#[test]
fn a_second_pad_joins_over_the_pause_on_the_loop() {
    second_pad_joins("pause-off", "off", false);
}
