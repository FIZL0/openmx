//! P127: a ride's load on a thread of its own while the window draws the loading screen.
//!
//! The worker itself ([`loading::on_worker`]) with no disc: a load held at a stage keeps the waiting
//! side turning frames, a cancel ends it with the thread gone, and a load that ends hands back what it
//! built, borrowing what outlives it as the ride's does. The loading screen drawn from the disc. And
//! the shell, as a user runs it: a load from the front end held by the test hook keeps drawing frames,
//! one cancelled goes back to the pick with nothing left running, and a ride built on the worker is
//! the plain build's ride, hash for hash.

// the browser's own clock, threads and files, which are std's here (P196)
#[allow(unused)]
#[path = "../src/clock.rs"]
mod clock;
#[allow(unused)]
#[path = "../src/files.rs"]
mod files;
#[path = "../src/loading.rs"]
#[allow(dead_code)]
mod loading;
#[path = "../src/menu.rs"]
#[allow(dead_code)]
mod menu;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;
#[path = "../src/roster.rs"]
#[allow(dead_code)]
mod roster;
#[path = "../src/dress.rs"]
#[allow(dead_code)]
mod dress;
#[path = "../src/join.rs"]
#[allow(dead_code)]
mod join;
#[path = "../src/split.rs"]
#[allow(dead_code)]
mod split;

use loading::{Progress, Stage};
use menu::{Controls, Menu, Screen, Settings};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The tests that count [`loading::workers`] one at a time, so another's thread is not counted.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// A build that walks every stage but the window's, counting a "ground" of `chunks`.
fn walk(progress: &Progress, chunks: usize) -> Result<usize, String> {
    for stage in [Stage::Scene, Stage::Tiles, Stage::Props, Stage::Models, Stage::Ground] {
        progress.reach(stage)?;
    }
    for n in 0..chunks {
        progress.count(n, chunks)?;
    }
    progress.count(chunks, chunks)?;
    Ok(chunks)
}

#[test]
fn a_held_load_keeps_the_frames_turning_and_a_cancel_leaves_no_thread() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let progress = Progress::with_hold(Some((Stage::Props, None)));
    let mut frames = 0u32;
    let mut held_frames = 0u32;
    let left = loading::on_worker(
        &progress,
        || walk(&progress, 50),
        || {
            frames += 1;
            if progress.held() {
                held_frames += 1;
                assert_eq!(loading::workers(), 1, "the load's thread runs while it is held");
                assert_eq!(progress.stage(), Stage::Props);
                assert_eq!(progress.line(), "Props");
            }
            std::thread::sleep(Duration::from_millis(2));
            // Back, after thirty frames of the held load.
            Ok((held_frames >= 30).then_some("back"))
        },
    )
    .expect("no error");
    assert_eq!(left, Err("back"), "the cancel is what the wait stopped for");
    assert!(held_frames >= 30 && frames >= held_frames, "{frames} frames, {held_frames} held");
    assert!(progress.cancelled());
    assert_eq!(loading::workers(), 0, "the load's thread is gone once the cancel is back");
}

#[test]
fn a_load_that_ends_hands_back_what_it_built_and_the_bar_fills() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let progress = Progress::with_hold(Some((Stage::Ground, Some(Duration::from_millis(100)))));
    // The ride's build hands back what borrows its terrain's bytes, kept outside it.
    let bytes = vec![7u8; 64];
    let kept = std::sync::OnceLock::new();
    let mut seen = Vec::new();
    let at = Instant::now();
    let built = loading::on_worker(
        &progress,
        || {
            let n = walk(&progress, 10)?;
            let slice: &[u8] = kept.get_or_init(|| &bytes[..n]);
            Ok(slice)
        },
        || {
            seen.push(progress.done());
            std::thread::sleep(Duration::from_millis(1));
            Ok(None::<()>)
        },
    )
    .expect("no error");
    assert_eq!(built, Ok(&[7u8; 10][..]));
    assert!(at.elapsed() >= Duration::from_millis(100), "the hold held");
    assert!(seen.len() > 10, "{} frames", seen.len());
    assert!(seen.windows(2).all(|w| w[1] >= w[0]), "the bar only fills: {seen:?}");
    // Five of the six stages, the ground counted, and the upload the window's to do.
    assert!((progress.done() - 5.0 / 6.0).abs() < 1e-6, "{}", progress.done());
    assert_eq!(progress.line(), "Ground 10 of 10");
    assert_eq!(loading::workers(), 0);
}

#[test]
fn a_load_that_fails_or_panics_is_an_error_and_its_thread_is_gone() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let progress = Progress::default();
    let failed = loading::on_worker(&progress, || Err::<(), _>("scene: no such file".to_string()), || Ok(None::<()>));
    assert_eq!(failed, Err("scene: no such file".to_string()));
    let panicked = loading::on_worker(&progress, || -> Result<(), String> { panic!("a bad chunk") }, || Ok(None::<()>));
    assert!(panicked.as_ref().is_err_and(|e| e.contains("a bad chunk")), "{panicked:?}");
    assert_eq!(loading::workers(), 0);
}

#[test]
fn the_hook_reads_a_stage_and_a_time() {
    assert_eq!(loading::parse_hold("ground"), Ok(Some((Stage::Ground, None))));
    assert_eq!(loading::parse_hold("models:1.5"), Ok(Some((Stage::Models, Some(Duration::from_millis(1500))))));
    assert!(loading::parse_hold("sky").is_err());
    assert!(loading::parse_hold("tiles:soon").is_err());
}

#[test]
fn a_cancel_goes_back_to_the_pick_it_was_ridden_from() {
    let mut m = Menu::new(Settings::default(), Controls::default());
    m.open(Screen::Tracks);
    m.settings.track = "free06".into();
    m.start_loading("free06");
    let l = m.loading.clone().expect("loading");
    assert_eq!(l.pack, "MX Unleashed");
    assert!(!l.name.is_empty() && l.name != "free06", "the pick's name: {}", l.name);
    m.back_to_pick();
    assert_eq!(m.loading, None);
    assert_eq!(m.screen(), Some(Screen::Tracks));
}

#[test]
fn the_loading_screen_draws_from_the_disc() {
    let mut disc = match game_render::Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let art = menu::Art::load(&mut disc).expect("the menus' art");
    let mut m = Menu::new(Settings::default(), Controls::default());
    m.start_loading("free06");
    let draw = |m: &Menu| {
        let mut frame = game_render::Frame::new(640, 360, [0, 0, 0]);
        art.draw(m, &mut frame);
        frame
    };
    let (mut early, mut late) = (m.clone(), m.clone());
    early.loading.as_mut().unwrap().done = 0.2;
    late.loading.as_mut().unwrap().done = 0.8;
    let (a, b) = (draw(&early), draw(&late));
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p127-loading");
    std::fs::create_dir_all(&dir).unwrap();
    game_render::png::write_rgb(&dir.join("loading.png"), a.width, a.height, &a.color).unwrap();
    let lit = a.color.chunks(3).filter(|p| *p != [0, 0, 0]).count();
    assert!(lit > a.width * a.height * 9 / 10, "drew over {lit} pixels");
    // The bar is the difference: more of it lit, and nothing else moved much.
    let moved = a.color.chunks(3).zip(b.color.chunks(3)).filter(|(x, y)| x != y).count();
    assert!(moved > 500 && moved < a.width * a.height / 10, "{moved} pixels moved");
    // And it is not the pick under it.
    let mut pick = Menu::new(Settings::default(), Controls::default());
    pick.open(Screen::Tracks);
    let p = draw(&pick);
    let apart = a.color.chunks(3).zip(p.color.chunks(3)).filter(|(x, y)| x != y).count();
    assert!(apart > a.width * a.height / 10, "{apart} pixels apart from the pick");
}

/// A directory of the shell's own for `name` (its files beside a link to the disc, never the user's);
/// none without a disc.
fn store(name: &str) -> Option<PathBuf> {
    let iso = match game_render::disc::Disc::configured_path() {
        Ok(p) if p.is_file() => p,
        _ => {
            eprintln!("skipped: no disc");
            return None;
        }
    };
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("p127-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("kit.toml"), "[game]\niso = \"disc.iso\"\n").unwrap();
    std::os::unix::fs::symlink(&iso, dir.join("disc.iso")).unwrap();
    let m = Menu::new(Settings::default(), Controls::default());
    menu::save(&dir, &m.settings, &m.controls).unwrap();
    Some(dir)
}

/// What the shell says run in `dir` with `env` and `args`.
fn shell(dir: &Path, env: &[(&str, &str)], args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(dir)
        .envs(env.iter().copied())
        .env_remove(loading::THREADED)
        .args(args)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    said
}

fn line<'a>(said: &'a str, what: &str) -> &'a str {
    said.lines().find(|l| l.starts_with(what)).unwrap_or_else(|| panic!("no {what:?} line in:\n{said}"))
}

/// The frames and seconds of a "loading: built" or "loading: done" line.
fn frames_over(l: &str) -> (u32, f32) {
    let words: Vec<&str> = l.split([' ', ',']).filter(|w| !w.is_empty()).collect();
    let at = |w: &str| words.iter().position(|x| *x == w).unwrap_or_else(|| panic!("{l}"));
    (words[at("frames") - 1].parse().unwrap(), words[at("s") - 1].parse().unwrap())
}

#[test]
fn a_held_load_from_the_front_end_keeps_drawing_the_loading_screen() {
    let Some(dir) = store("held") else { return };
    // Four seconds held at the models: the window's thread draws through them.
    let said = shell(
        &dir,
        &[(loading::HOLD, "models:4")],
        &["--headless", "--menu", "--screen", "pause", "--frames", "2", "--no-sound"],
    );
    line(&said, "loading: held at models");
    let (frames, seconds) = frames_over(line(&said, "loading: built,"));
    assert!(seconds >= 4.0, "{seconds} s");
    // One a second at least: a debug build's menu frame under a busy gate is far from the release
    // build's 60, and a frozen window draws none.
    assert!(frames as f32 >= seconds, "{frames} frames over {seconds} s");
    let (all, _) = frames_over(line(&said, "loading: done,"));
    assert!(all > frames, "the upload drew frames between its slices too: {all} after {frames}");
    line(&said, "ride hash");
}

#[test]
fn a_cancelled_load_goes_back_to_the_pick_with_nothing_running() {
    let Some(dir) = store("cancel") else { return };
    // Held until cancelled, and Back after ten frames.
    let said = shell(
        &dir,
        &[(loading::HOLD, "models"), (loading::CANCEL, "10")],
        &["--headless", "--menu", "--screen", "pause", "--seconds", "20", "--no-sound"],
    );
    line(&said, "loading: held at models");
    assert_eq!(line(&said, "loading: cancelled"), "loading: cancelled at Models, back on Some(Tracks)");
    assert_eq!(line(&said, "loading: stopped"), "loading: stopped, the load's thread is gone (0 running)");
    assert!(!said.contains("ride hash") && !said.contains("ride:"), "no ride ran:\n{said}");
}

/// Full throttle and nothing else: Cross down at full pressure (tests/fixed_rate.rs's tape).
fn throttle_tape(frames: usize) -> String {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape full throttle\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    text
}

#[test]
fn a_ride_built_on_the_worker_is_the_plain_builds_ride() {
    let Some(dir) = store("hash") else { return };
    let tape = dir.join("throttle.tape");
    std::fs::write(&tape, throttle_tape(240)).unwrap();
    let tape = tape.to_str().unwrap();
    let args = ["--headless", "--track", "free06", "--frames", "120", "--no-sound", "--play", tape];
    let plain = shell(&dir, &[], &args);
    let threaded = shell(&dir, &[(loading::THREADED, "1")], &args);
    assert_eq!(line(&plain, "ride hash"), line(&threaded, "ride hash"));
    assert_eq!(line(&plain, "ridden for").split(" at ").nth(1), line(&threaded, "ridden for").split(" at ").nth(1));
}
