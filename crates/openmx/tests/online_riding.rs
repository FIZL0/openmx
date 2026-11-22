//! P133 — the riding ragdoll online: two headless shells on free09 draw their riders from the riding
//! ragdoll's nodes (`--trace-picture`'s `picture riding:` line), as an offline ride does (S90), and
//! the session's ride is untouched by it (no desync).
//!
//! The host's first player holds Cross from tick 10; the guest's sits still. The guest's seat, ridden
//! by an idle pad from the ride's start, stands as the offline ride's on the same idle pad does, so
//! its drawn nodes are held to an offline shell's beside it.
//!
//! Needs the disc and skips without it, saying so.

use std::path::Path;
use std::process::Command;

const FRAMES: u32 = 150;

fn disc() -> bool {
    match game_render::Disc::open_configured() {
        Ok(_) => true,
        Err(why) => {
            eprintln!("skipped: {why}");
            false
        }
    }
}

/// A headless shell on `words`: it, its output once it ends, and each line as it comes.
fn shell(words: &[&str]) -> (std::process::Child, std::thread::JoinHandle<String>, std::sync::mpsc::Receiver<String>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut shell = Command::new(env!("CARGO_BIN_EXE_openmx"));
    shell.current_dir(&root).args(words).stdout(std::process::Stdio::piped());
    let mut kid = shell.spawn().expect("the shell starts");
    let mut out = std::io::BufReader::new(kid.stdout.take().expect("piped stdout"));
    let (lines, heard) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut said = String::new();
        let mut line = String::new();
        while std::io::BufRead::read_line(&mut out, &mut line).is_ok_and(|n| n > 0) {
            let _ = lines.send(line.clone());
            said.push_str(&line);
            line.clear();
        }
        said
    });
    (kid, reader, heard)
}

/// Two shells online on free09, the host's on `--online-script 10:gas`, each one's output. Each
/// binds port 0 (K29): the host says its port and the guest is started on it.
fn ride_online() -> [String; 2] {
    let frames = FRAMES.to_string();
    let words = |me: &'static str, peer: String| {
        let mut w: Vec<String> = ["--headless", "--track", "free09", "--frames", &frames, "--trace-picture", "--seats", "host,g", "--online", me, "--peer"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        w.push(peer);
        w
    };
    let mut host_words = words("host@127.0.0.1:0", "g@127.0.0.1:0".into());
    host_words.extend(["--online-script".into(), "10:gas".into()]);
    let (mut host, host_said, heard) = shell(&host_words.iter().map(String::as_str).collect::<Vec<_>>());
    let at = loop {
        match heard.recv_timeout(std::time::Duration::from_secs(600)) {
            Ok(l) if l.starts_with("online: host on ") => break l["online: host on ".len()..].trim().split(',').next().unwrap().to_string(),
            Ok(_) => {}
            Err(_) => {
                let _ = host.kill();
                let _ = host.wait();
                panic!("the host said no port:\n{}", host_said.join().unwrap_or_default());
            }
        }
    };
    let guest_words = words("g@127.0.0.1:0", format!("host@{at}"));
    let (guest, guest_said, _) = shell(&guest_words.iter().map(String::as_str).collect::<Vec<_>>());
    both_end([host, guest], [host_said, guest_said])
}

/// Two shells waited on to the end, each one's output; one that fails ends the other.
fn both_end(mut kids: [std::process::Child; 2], readers: [std::thread::JoinHandle<String>; 2]) -> [String; 2] {
    let started = std::time::Instant::now();
    let mut ended = [false; 2];
    while !ended.iter().all(|&e| e) {
        let late = started.elapsed() > std::time::Duration::from_secs(600);
        for i in 0..2 {
            match kids[i].try_wait().expect("the shell's status") {
                Some(status) if !status.success() || late => {
                    let _ = kids[1 - i].kill();
                    let _ = kids[1 - i].wait();
                    let [a, b] = readers.map(|r| r.join().unwrap_or_default());
                    panic!("an online shell failed:\n{}", if i == 0 { a } else { b });
                }
                Some(_) => ended[i] = true,
                None if late => {
                    for k in &mut kids {
                        let _ = k.kill();
                        let _ = k.wait();
                    }
                    let [a, b] = readers.map(|r| r.join().unwrap_or_default());
                    panic!("the online shells did not finish in ten minutes:\n{a}\n----\n{b}");
                }
                None => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    readers.map(|r| r.join().expect("the shell's output"))
}

/// Each `picture riding:` line's tick and, seat by seat, whether its rider was drawn from the riding
/// ragdoll.
fn riding(said: &str) -> Vec<(u64, Vec<bool>)> {
    said.lines()
        .filter_map(|l| l.strip_prefix("picture riding: tick "))
        .map(|l| {
            let mut w = l.split(' ');
            (w.next().unwrap().parse().expect("a tick"), w.map(|s| s == "1").collect())
        })
        .collect()
}

/// The last `picture pose:` line's nodes, each in the bike's frame: the ride's last tick, which
/// `--frames` counts in the session's ticks online, so an offline ride's of the same length.
fn last_pose(said: &str) -> Vec<f32> {
    let line = said.lines().filter(|l| l.starts_with("picture pose: ")).last().unwrap_or_else(|| panic!("no pose:\n{said}"));
    line.split(' ').skip(8).map(|w| w.parse().expect("a number")).collect()
}

#[test]
fn headless_online_shells_show_the_riding_nodes() {
    if !disc() {
        return;
    }
    let said = ride_online();
    for (who, s) in ["host", "guest"].iter().zip(&said) {
        assert!(s.lines().any(|l| l.contains(", 0 desyncs")), "{who}: the ride parted:\n{s}");
        let drawn = riding(s);
        assert!(!drawn.is_empty(), "{who}: no picture lines:\n{s}");
        // Both seats, this machine's own and the other's: from its first step (on the first grips
        // drawn, once the session ticks: a host waits for its guest first) to the end, every frame.
        for seat in 0..2 {
            let on = |(_, seats): &(u64, Vec<bool>)| seats.get(seat).copied().unwrap_or(false);
            let first = drawn.iter().position(on).unwrap_or_else(|| panic!("{who}: seat {seat} never drawn from the riding ragdoll:\n{s}"));
            let off: Vec<u64> = drawn[first..].iter().filter(|l| !on(l)).map(|&(t, _)| t).collect();
            assert!(off.is_empty(), "{who}: seat {seat} drawn off the clip pose again at ticks {off:?}");
            assert!(drawn.len() - first > 60, "{who}: seat {seat} drawn from the riding ragdoll on only {} frames", drawn.len() - first);
        }
    }
    // The guest's idle seat beside an offline ride on the same idle pad: the same body standing, to
    // within a tenth of a foot (a body off its bike, or off the clip by its whole lean, is feet off).
    // Not closer: the seat stands in the second bay and its body machine is the session's, not the
    // shell's (P133's journal: 0.03 ft at the end of the ride, cause not pinned down).
    let (kid, offline, _) = shell(&["--headless", "--track", "free09", "--frames", &FRAMES.to_string(), "--trace-picture"]);
    let mut kid = kid;
    assert!(kid.wait().expect("the offline shell ends").success());
    let offline = offline.join().expect("its output");
    let (on, off) = (last_pose(&said[1]), last_pose(&offline));
    assert_eq!(on.len(), off.len(), "node counts");
    let worst = on.iter().zip(&off).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
    println!("the online idle seat's riding nodes within {worst:.4} ft of offline's");
    assert!(worst < 0.1,"the online riding body is {worst} ft off the offline one");
}
