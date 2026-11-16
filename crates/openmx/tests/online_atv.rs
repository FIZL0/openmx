//! MD152 — the quad in an online ride: every machine runs every seat's quad.
//!
//! Two shells on one machine, host and guest, ride free09 online over `--peer`s, each with this
//! checkout's mods folder and its own quad pick (`--atv`). Each machine's pick goes to the other with
//! its saved markers; every machine reads every seat's quad off the same pack. Both seats switch to
//! their quads (`--online-script`, the pad's switch byte) and open the throttle. Both machines must
//! see both switches, build both quads, move them the same distance, and agree on the settled
//! ticks' state hash (no desync, the same state before tick 300) while both quads are out.
//!
//! Under it, the pad byte: the quad's switch is byte 6 and reads back as the quad.
//!
//! MD174: `--peer`s have no handshake, so each machine's pick goes out with every quad pack it holds
//! and their hashes, and a pick rides only where every machine holds its pack with the same files.
//! Two shells whose packs differ by one file refuse both quads on both machines and still agree.
//!
//! Skips with no disc, and with no `mods/machines/mxvatv_atv` (openmx-mods' import-atv makes it).

#[allow(unused)]
#[path = "../src/thread.rs"]
mod thread;
#[allow(unused)]
#[path = "../src/clock.rs"]
mod clock;
#[allow(unused)]
#[path = "../src/files.rs"]
mod files;
#[path = "../src/menu.rs"]
#[allow(dead_code)]
mod menu;
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
#[allow(dead_code)]
#[path = "../src/window.rs"]
mod window;
#[allow(dead_code)]
#[path = "../src/pad.rs"]
mod pad;
#[allow(dead_code)]
#[path = "../src/audio.rs"]
mod audio;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;
#[allow(dead_code)]
#[path = "../src/net/mod.rs"]
mod net;
#[path = "../src/online_quads.rs"]
mod online_quads;

use game_render::Disc;
use std::path::{Path, PathBuf};
use std::process::Command;
use vehicle::{Kind, Wanted};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Whether the disc and the quad pack are both here; each missing says so.
fn ready() -> bool {
    if let Err(why) = Disc::open_configured() {
        eprintln!("skipped: {why}");
        return false;
    }
    let there = root().join("mods/machines/mxvatv_atv/mod.json").is_file();
    if !there {
        eprintln!("SKIPPED: no mods/machines/mxvatv_atv (openmx-mods' import-atv makes it)");
    }
    there
}

fn line_with<'a>(said: &'a str, part: &str) -> &'a str {
    said.lines().find(|l| l.contains(part)).unwrap_or_else(|| panic!("no line with `{part}`:\n{said}"))
}

/// Two shells riding `track` online for `frames` ticks with this checkout's mods, host's and
/// guest's quad picks `atv` and scripts `scripts`: each one's output. As tests/online_car.rs's
/// `ride_online`: each shell binds a port of its own, the host says it, the guest starts on it.
fn ride_online(track: &str, frames: u32, atv: [&str; 2], scripts: [&str; 2]) -> [String; 2] {
    ride_online_on(track, frames, atv, scripts, ["mods", "mods"])
}

/// [`ride_online`] with each shell's own mods folder, host's and guest's `mods`.
fn ride_online_on(track: &str, frames: u32, atv: [&str; 2], scripts: [&str; 2], mods: [&str; 2]) -> [String; 2] {
    let root = root();
    let frames = frames.to_string();
    let ride = |me: &str, peer: &str, atv: &str, script: &str, mods: &str| {
        let mut shell = Command::new(env!("CARGO_BIN_EXE_openmx"));
        shell
            .current_dir(&root)
            .args(["--headless", "--track", track, "--frames", &frames, "--seats", "host,g", "--mods", mods, "--atv", atv])
            .args(["--online", me, "--peer", peer, "--online-script", script])
            .stdout(std::process::Stdio::piped());
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
    };
    let (mut host, host_said, heard) = ride("host@127.0.0.1:0", "g@127.0.0.1:0", atv[0], scripts[0], mods[0]);
    let at = loop {
        match heard.recv_timeout(std::time::Duration::from_secs(600)) {
            Ok(l) if l.starts_with("online: host on ") => break l["online: host on ".len()..].split(',').next().unwrap().to_string(),
            Ok(_) => {}
            Err(_) => {
                let _ = host.kill();
                let _ = host.wait();
                panic!("the host said no port:\n{}", host_said.join().unwrap_or_default());
            }
        }
    };
    let (guest, guest_said, _) = ride("g@127.0.0.1:0", &format!("host@{at}"), atv[1], scripts[1], mods[1]);
    // A shell that fails takes its peer down with it: the peer would wait for its pads for ever.
    let mut kids = [host, guest];
    let readers = [host_said, guest_said];
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

#[test]
fn the_quads_switch_is_pad_byte_six() {
    let byte = Wanted { to: Some(Kind::Atv), ..Default::default() }.byte();
    // 6: its low two bits in bits 0 and 1, its third in bit 7
    assert_eq!(byte & 0x83, 0x82, "{byte:#04x}");
    assert_eq!(Wanted::of(byte).to, Some(Kind::Atv));
}

#[test]
fn two_online_shells_both_ride_their_quads_and_agree() {
    if !ready() {
        return;
    }
    let said = ride_online("free09", 420, ["atv_Race250P", "atv_Pro250A"], ["40:atv,41:gas", "60:atv,61:gas"]);
    for (who, s) in ["host", "guest"].iter().zip(&said) {
        // Every seat's quad, read here off its machine's pick.
        assert!(s.contains("online: seat 1 quad mxvatv_atv/atv_Race250P"), "{who}:\n{s}");
        assert!(s.contains("online: seat 2 quad mxvatv_atv/atv_Pro250A"), "{who}:\n{s}");
        // This machine's seat's switch: the load's bike, then the quad.
        let switched: Vec<&str> = s.lines().filter(|l| l.starts_with("vehicle: ") && !l.contains("every machine agrees")).collect();
        assert_eq!(switched.first(), Some(&"vehicle: bike"), "{who}:\n{s}");
        assert!(switched.contains(&"vehicle: atv"), "{who}:\n{s}");
        assert!(line_with(s, "desyncs").contains(" 0 desyncs"), "{who}:\n{s}");
        // Both quads out at the end, each gone somewhere on the throttle.
        let quads = line_with(s, "online: quads ");
        for (seat, stem) in [("1P", "atv_Race250P"), ("2P", "atv_Pro250A")] {
            let at = format!("{seat} {stem} ");
            let rest = &quads[quads.find(&at).unwrap_or_else(|| panic!("{who}: {seat} is not on its quad: {quads}")) + at.len()..];
            let gone: f32 = rest.split(' ').next().unwrap().parse().expect("a distance");
            assert!(gone > 5.0, "{who}: {seat}'s quad stayed where it was taken: {quads}");
        }
    }
    // And both machines rode both quads to the same place, on the same state.
    assert_eq!(line_with(&said[0], "online: quads "), line_with(&said[1], "online: quads "));
    let state = |s: &str| line_with(s, "state before tick 300").rsplit(' ').next().unwrap().to_string();
    assert_eq!(state(&said[0]), state(&said[1]), "the two machines' rides parted with the quads out");
}

/// MD137: one seat on its quad, the other on its bike. The guest switches, the host rides on; both
/// machines must see the guest's switch, run its quad alone, and agree on the settled state.
#[test]
fn one_seat_switches_to_its_quad_and_both_agree() {
    if !ready() {
        return;
    }
    let said = ride_online("free09", 420, ["atv_Race250P", "atv_Pro250A"], ["40:gas", "60:atv,61:gas"]);
    for (who, s) in ["host", "guest"].iter().zip(&said) {
        let switched: Vec<&str> = s.lines().filter(|l| l.starts_with("vehicle: ") && !l.contains("every machine agrees")).collect();
        assert_eq!(switched.first(), Some(&"vehicle: bike"), "{who}:\n{s}");
        // The switch line names no seat: both machines print the guest's.
        assert!(switched.contains(&"vehicle: atv"), "{who}:\n{s}");
        assert!(line_with(s, "desyncs").contains(" 0 desyncs"), "{who}:\n{s}");
        let quads = line_with(s, "online: quads ");
        assert!(quads.contains("1P off"), "{who}: the host left its bike: {quads}");
        let at = "2P atv_Pro250A ";
        let rest = &quads[quads.find(at).unwrap_or_else(|| panic!("{who}: 2P is not on its quad: {quads}")) + at.len()..];
        let gone: f32 = rest.split(' ').next().unwrap().parse().expect("a distance");
        assert!(gone > 5.0, "{who}: 2P's quad stayed where it was taken: {quads}");
    }
    assert_eq!(line_with(&said[0], "online: quads "), line_with(&said[1], "online: quads "));
    let state = |s: &str| line_with(s, "state before tick 300").rsplit(' ').next().unwrap().to_string();
    assert_eq!(state(&said[0]), state(&said[1]), "the two machines' rides parted with one quad out");
}

fn card(pick: &str, held: &[(&str, u64)]) -> online_quads::Card {
    online_quads::Card { pick: pick.into(), held: held.iter().map(|&(id, h)| (id.to_string(), h)).collect() }
}

#[test]
fn a_quad_card_wraps_the_markers_and_reads_back() {
    let inner = b"P318 markers".to_vec();
    let mine = card("mxvatv_atv/atv_Race250P", &[("mxvatv_atv", 0x0123_4567_89ab_cdef), ("other", 7)]);
    let wrapped = online_quads::wrap(&mine, inner.clone());
    assert_eq!(online_quads::unwrap(&wrapped), (mine, &inner[..]));
    // No pick but packs held: still sent, as the others' picks need this machine's hashes.
    let holds = card("", &[("mxvatv_atv", 9)]);
    assert_eq!(online_quads::unwrap(&online_quads::wrap(&holds, inner.clone())), (holds, &inner[..]));
    // Nothing to say: the blob alone, and bytes not wrapped (or cut short) have an empty card.
    assert_eq!(online_quads::wrap(&card("", &[]), inner.clone()), inner);
    assert_eq!(online_quads::unwrap(&inner), (card("", &[]), &inner[..]));
    let cut = &wrapped[..wrapped.len() - inner.len() - 3];
    assert_eq!(online_quads::unwrap(cut), (card("", &[]), cut));
}

#[test]
fn a_quad_pick_rides_only_where_every_machine_holds_the_same_pack() {
    let pick = "mxvatv_atv/atv_Race250P";
    let same = [card(pick, &[("mxvatv_atv", 5)]), card("", &[("mxvatv_atv", 5), ("more", 1)])];
    assert_eq!(online_quads::refused(&same, 0), None);
    assert_eq!(online_quads::refused(&same, 1), None, "no pick, nothing to refuse");
    let differs = [card(pick, &[("mxvatv_atv", 5)]), card(pick, &[("mxvatv_atv", 6)])];
    assert_eq!(online_quads::refused(&differs, 0).as_deref(), Some("seat 2's mxvatv_atv differs from seat 1's"));
    assert_eq!(online_quads::refused(&differs, 1).as_deref(), Some("seat 1's mxvatv_atv differs from seat 2's"));
    let lacks = [card(pick, &[("mxvatv_atv", 5)]), card("", &[])];
    assert_eq!(online_quads::refused(&lacks, 0).as_deref(), Some("seat 2's machine has no quad pack mxvatv_atv"));
    let unsent = [card(pick, &[]), card("", &[("mxvatv_atv", 5)])];
    assert_eq!(online_quads::refused(&unsent, 0).as_deref(), Some("seat 1's machine sent no hash of mxvatv_atv"));
    assert!(online_quads::refused(&[card("nopack", &[])], 0).is_some());
}

/// `from` copied to `to`, every file under it.
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a folder");
    for e in std::fs::read_dir(from).expect("the pack's folder") {
        let path = e.expect("an entry").path();
        let into = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_tree(&path, &into);
        } else {
            std::fs::copy(&path, &into).expect("a copy");
        }
    }
}

#[test]
fn two_online_shells_refuse_quads_whose_packs_differ() {
    if !ready() {
        return;
    }
    // The guest's mods folder: the same quad pack and one file more.
    let guest = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md174_mods");
    let _ = std::fs::remove_dir_all(&guest);
    copy_tree(&root().join("mods/machines/mxvatv_atv"), &guest.join("machines/mxvatv_atv"));
    std::fs::write(guest.join("machines/mxvatv_atv/extra.txt"), "not on the host").expect("the extra file");
    let said = ride_online_on("free09", 320, ["atv_Race250P", "atv_Pro250A"], ["40:atv,41:gas", "60:atv,61:gas"], ["mods", guest.to_str().unwrap()]);
    let _ = std::fs::remove_dir_all(&guest);
    for (who, s) in ["host", "guest"].iter().zip(&said) {
        // Both picks refused on both machines, neither quad built, nobody switched.
        assert!(s.contains("online: seat 1 picked the quad mxvatv_atv/atv_Race250P, refused on every machine: seat 2's mxvatv_atv differs from seat 1's"), "{who}:\n{s}");
        assert!(s.contains("online: seat 2 picked the quad mxvatv_atv/atv_Pro250A, refused on every machine: seat 1's mxvatv_atv differs from seat 2's"), "{who}:\n{s}");
        assert!(!s.contains("online: seat 1 quad ") && !s.contains("online: seat 2 quad "), "{who}:\n{s}");
        assert!(!s.lines().any(|l| l == "vehicle: atv"), "{who}:\n{s}");
        assert!(line_with(s, "desyncs").contains(" 0 desyncs"), "{who}:\n{s}");
    }
    let state = |s: &str| line_with(s, "state before tick 300").rsplit(' ').next().unwrap().to_string();
    assert_eq!(state(&said[0]), state(&said[1]), "the two machines' rides parted");
}
