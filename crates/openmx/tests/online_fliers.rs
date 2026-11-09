//! P318 — fliers online: a seat's pick of a flier pack's plane goes over the wire in its pad, and
//! is open only where every seat's machine holds the pack.
//!
//! Two shells on one machine, host and guest, ride free09 online over `--peer`s, each on its own
//! mods folder: (a) the host's with MX vs. ATV's fliers pack and the guest's without — both list the
//! pack's planes greyed, missing for 2P, and both refuse a pick of one (`--online-script`'s
//! `biplane@mxvatv_fliers/biplane3`), every seat staying on its bike; (b) both with it — the guest
//! switches to MX vs. ATV's second model (the Spitfire, `biplane3`) mid-ride, both machines fly it
//! as that plane, and the two rides agree (no desync, the same state hash).
//!
//! Under them, [`online_fliers`]: the byte a pick sends, the offer and who lacks what, and the packs
//! a `--peer` machine sends wrapped with its saved markers.
//!
//! Needs the disc and `mods/machines/mxvatv_fliers` (`./kit mods`); skips without either, saying so.

#[allow(dead_code)]
#[path = "../src/online_fliers.rs"]
mod online_fliers;

use game_render::Disc;
use openmx_net::Pack;
use std::path::Path;
use std::process::Command;

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            eprintln!("skipped: {why}");
            None
        }
    }
}

/// The repo's mods folder, where it holds MX vs. ATV's fliers pack.
fn fliers_pack() -> Option<&'static str> {
    let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/machines/mxvatv_fliers/mod.json");
    if pack.is_file() {
        Some("mods")
    } else {
        eprintln!("skipped: no {}", pack.display());
        None
    }
}

fn line_with<'a>(said: &'a str, part: &str) -> &'a str {
    said.lines().find(|l| l.contains(part)).unwrap_or_else(|| panic!("no line with `{part}`:\n{said}"))
}

/// Two shells riding `track` online for `frames` ticks, host's and guest's on `scripts`, each on its
/// own mods folder (`mods`, `off` for none): each one's output. As tests/online_car.rs's.
fn ride_online(track: &str, frames: u32, scripts: [Option<&str>; 2], mods: [&str; 2]) -> [String; 2] {
    // Each shell binds a port of its own (port 0) and keeps it: a port picked free here and bound
    // by the shell after its load was a window another slot's test could take it in (K29). The
    // host says its port; the guest is started on it; the host takes the guest's from its first
    // datagram (`net::udp`'s port-0 peer).
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let frames = frames.to_string();
    let ride = |me: &str, peer: &str, script: Option<&str>, mods: &str| {
        let mut shell = Command::new(env!("CARGO_BIN_EXE_openmx"));
        shell
            .current_dir(&root)
            .args(["--headless", "--track", track, "--frames", &frames, "--seats", "host,g"])
            .args(["--online", me, "--peer", peer, "--mods", mods])
            .stdout(std::process::Stdio::piped());
        if let Some(s) = script {
            shell.args(["--online-script", s]);
        }
        let mut kid = shell.spawn().expect("the shell starts");
        // Its output, each line also sent on as it comes (the host's port).
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
    let (mut host, host_said, heard) = ride("host@127.0.0.1:0", "g@127.0.0.1:0", scripts[0], mods[0]);
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
    let (guest, guest_said, _) = ride("g@127.0.0.1:0", &format!("host@{at}"), scripts[1], mods[1]);
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

/// (a): the host holds the fliers pack and the guest does not. Both machines list its planes greyed
/// with the guest's seat as the one lacking it, both refuse their player's pick of the Spitfire, and
/// both seats ride on, on their bikes, the two rides agreeing.
#[test]
fn a_flier_pack_one_machine_lacks_is_greyed_and_refused_on_both() {
    let (Some(_), Some(mods)) = (disc(), fliers_pack()) else { return };
    let pick = "40:biplane@mxvatv_fliers/biplane3";
    let said = ride_online("free09", 330, [Some(pick), Some(pick)], [mods, "off"]);
    for (who, s) in ["host", "guest"].iter().zip(&said) {
        let greyed = line_with(s, "online: flier mxvatv_fliers/");
        assert!(greyed.contains("greyed: needs MX vs. ATV Unleashed biplane and Kiowa") && greyed.ends_with("missing for 2P"), "{who}: {greyed}\n{s}");
        assert!(!s.lines().any(|l| l.starts_with("online: flier ") && l.ends_with(" open")), "{who}: a flier open:\n{s}");
        let refused = line_with(s, "fliers: biplane@mxvatv_fliers/biplane3 is not offered online: ");
        assert!(refused.ends_with("missing for 2P"), "{who}: {refused}");
        assert_eq!(line_with(s, "online: fliers "), "online: fliers 1P bike, 2P bike", "{who}:\n{s}");
        assert!(line_with(s, "desyncs").contains(" 0 desyncs"), "{who}:\n{s}");
    }
    let state = |s: &str| line_with(s, "state before tick 300").rsplit(' ').next().unwrap().to_string();
    assert_eq!(state(&said[0]), state(&said[1]));
}

/// (b): both machines hold it. The guest picks the Spitfire at tick 40 and opens the throttle: both
/// machines switch its seat onto that plane at one tick and fly it alike.
#[test]
fn both_machines_holding_the_pack_fly_the_guests_spitfire_alike() {
    let (Some(_), Some(mods)) = (disc(), fliers_pack()) else { return };
    let said = ride_online("free09", 420, [None, Some("40:biplane@mxvatv_fliers/biplane3,41:gas")], [mods, mods]);
    for (who, s) in ["host", "guest"].iter().zip(&said) {
        assert!(line_with(s, "online: flier mxvatv_fliers/biplane3 ").ends_with(" open"), "{who}:\n{s}");
        let switched: Vec<&str> = s.lines().filter(|l| l.starts_with("vehicle: ") && !l.contains("every machine agrees")).collect();
        assert_eq!(switched, ["vehicle: bike", "vehicle: biplane"], "{who}:\n{s}");
        assert_eq!(line_with(s, "online: fliers "), "online: fliers 1P bike, 2P mxvatv_fliers/biplane3", "{who}:\n{s}");
        assert!(line_with(s, "desyncs").contains(" 0 desyncs"), "{who}:\n{s}");
    }
    let state = |s: &str| line_with(s, "state before tick 300").rsplit(' ').next().unwrap().to_string();
    assert_eq!(state(&said[0]), state(&said[1]), "the two machines flew the Spitfire apart");
}

fn pack(id: &str, hash: u64) -> Pack {
    Pack { kind: "machine".into(), id: id.into(), hash, name: format!("{id} pack"), how: String::new() }
}

fn skin(pack: &str, skin: &str) -> online_fliers::Skin {
    online_fliers::Skin { pack: pack.into(), skin: skin.into(), name: skin.to_uppercase() }
}

#[test]
fn a_pick_is_a_packs_slot_and_its_skins_place() {
    assert_eq!(online_fliers::byte(0, 2), Some(0x12));
    assert_eq!(online_fliers::of_byte(0x12), Some((0, 2)));
    assert_eq!(online_fliers::of_byte(0), None, "0 is the ride's own biplane");
    assert_eq!(online_fliers::byte(online_fliers::SLOTS, 0), None);
    assert_eq!(online_fliers::byte(0, online_fliers::SKINS), None);
}

#[test]
fn the_offer_greys_a_pack_some_seat_lacks_and_numbers_packs_alike_everywhere() {
    let (a, b) = (pack("a_fliers", 1), pack("b_fliers", 2));
    let host = vec![b.clone(), a.clone(), Pack { kind: "tricks".into(), ..pack("t", 3) }];
    // the ride's packs: the reference machine's machine packs, in key order
    let ride = online_fliers::ride_packs(&host);
    assert_eq!(ride.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["a_fliers", "b_fliers"]);
    // the guest holds a only, and b under other files
    let guest = vec![a.clone(), pack("b_fliers", 9)];
    let seats = [(0, Some(&host[..])), (1, Some(&guest[..]))];
    let skins = [skin("a_fliers", "p1"), skin("a_fliers", "p2"), skin("b_fliers", "q1")];
    let o = online_fliers::offer(&ride, &seats, &host, &skins);
    let rows: Vec<(u8, &str, bool)> = o.iter().map(|o| (o.byte, o.skin.as_str(), o.open())).collect();
    assert_eq!(rows, [(0x10, "p1", true), (0x11, "p2", true), (0x20, "q1", false)]);
    assert_eq!(o[2].why, "needs b_fliers pack, missing for 2P");
    // the guest's own list: b's skins are not the host's files, so one greyed row for the pack
    let theirs = online_fliers::offer(&ride, &seats, &guest, &skins);
    assert_eq!(theirs.iter().map(|o| (o.byte, o.skin.as_str(), o.open())).collect::<Vec<_>>(), [(0x10, "p1", true), (0x11, "p2", true), (0x20, "", false)]);
    // a seat that said no packs lacks them all
    assert_eq!(online_fliers::lacking(&a, &[(0, Some(&host[..])), (2, None)]), ["3P"]);
    assert_eq!(online_fliers::byte_of(&o, "a_fliers", "p2", "mxu"), Some(0x11));
    assert_eq!(online_fliers::byte_of(&o, "b_fliers", "q1", "mxu"), None, "greyed");
    assert_eq!(online_fliers::byte_of(&o, "mxu", "biplane2", "mxu"), Some(0));
    assert_eq!(online_fliers::key_of(&o, 0x11).as_deref(), Some("machine/a_fliers"));
    assert_eq!(online_fliers::key_of(&o, 0), None);
}

#[test]
fn a_peers_packs_ride_with_its_saved_markers() {
    let packs = vec![pack("a_fliers", 1)];
    let wrapped = online_fliers::wrap(b"marks", &packs);
    let (marks, back) = online_fliers::unwrap(&wrapped);
    assert_eq!((marks, back), (&b"marks"[..], packs));
    // an older build's markers alone: no packs
    assert_eq!(online_fliers::unwrap(b"marks"), (&b"marks"[..], Vec::new()));
    assert_eq!(online_fliers::unwrap(&online_fliers::wrap(b"", &[])), (&b""[..], Vec::new()));
}
