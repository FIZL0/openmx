//! N4 — a desktop and a browser ride online together and agree, tick by tick.
//!
//! The desktop is this test: [`peer::Peer`] on the native transport ([`Net`]), hosting a lobby of
//! two through a relay. The browser is the page's own build: tools/web.sh's module and glue, its
//! `Online` (crates/openmx/src/web.rs) on the wasm transport — relay-only, as a page's has to be.
//! Twice: web/ itself in headless chromium (tests/web/page.mjs, the page's `?join=`), and the module
//! alone in node with the frame loop web/worker.js runs (tests/web/peer.mjs). It joins by the host's
//! ticket, readies, and both ride N2's test ride ([`peer`] says which) on [`peer::mash`]'s pads, each
//! machine at its own 60 frames a second by its own clock, through iroh's relay server run in this
//! process on loopback (no internet, the way crates/openmx-net's tests run it).
//!
//! What "agree" means: the first [`TICKS`] ticks, settled on both, have the same line on both
//! ([`peer::ticks`]: the state's hash before the tick and every seat's pad bytes it ran on), and
//! neither saw a desync.

#[allow(dead_code)]
#[path = "../src/net/mod.rs"]
mod net;
#[allow(dead_code)]
#[path = "../src/peer.rs"]
mod peer;

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use openmx_net::{Board, Cmd, Names, Net, Online, Pick, Relay, Room, HOST};

/// How many ticks both machines must agree on: ten seconds of riding at the ride's 30 Hz (N6).
const TICKS: usize = 300;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `tools/web.sh --check`: the directory of the module and its glue (as tests/web.rs's `built`).
fn built() -> PathBuf {
    let script = repo().join("tools/web.sh");
    let out = Command::new("sh").arg(&script).arg("--check").output().expect("tools/web.sh must be runnable");
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "{} --check failed:\n{text}", script.display());
    let path = PathBuf::from(String::from_utf8_lossy(&out.stdout).lines().last().unwrap_or("").trim());
    assert!(path.join("openmx_web_bg.wasm").is_file() && path.join("openmx_web.js").is_file(), "tools/web.sh --check named no module:\n{text}");
    path
}

/// iroh's relay server on loopback, plain HTTP, for as long as the runtime lives.
fn local_relay() -> (String, tokio::runtime::Runtime) {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().worker_threads(1).build().unwrap();
    let url = rt.block_on(async {
        let mut cfg = iroh_relay::server::ServerConfig::default();
        cfg.relay = Some(iroh_relay::server::RelayConfig::new(([127, 0, 0, 1], 0)));
        let server = iroh_relay::server::Server::spawn(cfg).await.expect("the local relay");
        let url = format!("http://{}", server.http_addr().unwrap());
        Box::leak(Box::new(server));
        url
    });
    (url, rt)
}

/// The browser peer, killed when the test is done with it however the test ends.
struct Node(std::process::Child);

impl Drop for Node {
    /// stdin closed first: page.mjs then closes its chromium and removes the profile; a kill only if
    /// it has not gone in five seconds.
    fn drop(&mut self) {
        drop(self.0.stdin.take());
        let until = Instant::now() + Duration::from_secs(5);
        while Instant::now() < until && matches!(self.0.try_wait(), Ok(None)) {
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The page as a user opens it, in headless chromium (tests/web/page.mjs): `?join=` the ticket.
#[test]
fn a_desktop_and_a_browser_ride_the_same_ride() {
    ride(|pkg, relay, ticket| {
        let mut c = Command::new("node");
        c.arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/web/page.mjs")).arg(repo().join("web")).arg(pkg).arg(relay).arg(ticket).arg(TICKS.to_string());
        c
    });
}

/// The same module and the worker's frame loop in node (tests/web/peer.mjs): no page, no worker,
/// so a failure here is the module's and one only in the test above is the page's.
#[test]
fn the_browser_build_rides_the_same_ride_in_node() {
    ride(|pkg, relay, ticket| {
        let mut c = Command::new("node");
        c.arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/web/peer.mjs")).arg(pkg).arg(relay).arg(ticket).arg(TICKS.to_string());
        c
    });
}

/// The desktop hosts; `browser(pkg, relay, ticket)` is the command that joins it and prints
/// peer.mjs's lines; the first [`TICKS`] ticks must be alike.
fn ride(browser: impl FnOnce(&Path, &str, &str) -> Command) {
    let pkg = built();
    let (relay, _rt) = local_relay();
    let settings = Online { name: "Desktop".into(), relay: Relay::Own(relay.clone()), relay_url: relay.clone(), board: Board::Off, lan: false, code_servers: Some(vec![]), relay_only: true, ..Online::default() };
    let mut desk = peer::Peer::new(Net::start(settings, Arc::new(Names)));
    desk.dumps = std::env::temp_dir().join(format!("openmx-n4-desyncs-{}", std::process::id()));
    let mut room = Room::new("", "free06", 2);
    room.take(HOST, &[Pick::new("thq1", "race125p")]);
    desk.net.send(Cmd::Host(room));

    // one call a tick of the ride's clock, as the shell calls it (`pace::TICK`)
    let frame = Duration::from_nanos(1_001_000_000_000 / 30_000);
    let started = Instant::now();
    let mut last = Instant::now();
    // what the desktop's lobby said last, printed when it changes (the log of a failed run)
    let mut was = String::new();
    let mut step = |desk: &mut peer::Peer| {
        std::thread::sleep(frame.saturating_sub(last.elapsed()));
        let dt = last.elapsed().as_secs_f32().min(0.25);
        last = Instant::now();
        desk.frame(&peer::mash, dt);
        for line in desk.said.drain(..) {
            eprintln!("desktop: {line}");
        }
        let now = format!("{} ({} seated, ready {})", desk.status, desk.room.as_ref().map_or(0, Room::taken), desk.room.as_ref().is_some_and(Room::all_ready));
        if now != was {
            eprintln!("desktop at {:.1} s: {now}", started.elapsed().as_secs_f32());
            was = now;
        }
        assert!(desk.failed.is_none(), "the desktop: {:?} ({})", desk.failed, desk.status);
        assert!(started.elapsed() < Duration::from_secs(120), "not done after two minutes; the desktop: {}", desk.status);
    };
    while desk.ticket.is_empty() || desk.room.is_none() {
        step(&mut desk);
    }

    let child = browser(&pkg, &relay, &desk.ticket).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("node must be on PATH");
    let mut node = Node(child);
    let (tx, rx) = mpsc::channel();
    let out = node.0.stdout.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let mut theirs = Vec::new();
    let mut state = String::new();
    let mut said_start = false;
    loop {
        step(&mut desk);
        for line in rx.try_iter() {
            if let Some(t) = line.strip_prefix("tick ") {
                theirs.push(t.to_string());
            } else if let Some(s) = line.strip_prefix("state ") {
                let status = |s: &str| s.split("\"status\":").nth(1).and_then(|s| s.split("\",").next()).unwrap_or_default().to_string();
                if status(s) != status(&state) {
                    eprintln!("browser at {:.1} s: {s}", started.elapsed().as_secs_f32());
                }
                state = s.to_string();
            }
        }
        if let Ok(Some(status)) = node.0.try_wait() {
            panic!("the browser peer exited ({status}); its last state: {state}");
        }
        assert!(started.elapsed() < Duration::from_secs(120), "the browser's last state: {state}");
        if !said_start && desk.riding.is_none() && desk.room.as_ref().is_some_and(|r| r.taken() == 2 && r.all_ready()) {
            desk.net.send(Cmd::Start);
            said_start = true;
        }
        let Some(r) = &desk.riding else { continue };
        if r.np.tick() % 600 == 0 {
            eprintln!("at {:.1} s: the desktop at tick {} (settled {}), the browser {state}", started.elapsed().as_secs_f32(), r.np.tick(), r.np.confirmed());
        }
        // settled can be ahead of the ticks run: both must have run them all
        if theirs.len() >= TICKS && r.np.confirmed() >= TICKS as i64 && r.np.tick() >= TICKS {
            break;
        }
    }

    let r = desk.riding.as_ref().unwrap();
    let ours: Vec<String> = peer::ticks(&r.np, 0, TICKS).lines().map(String::from).collect();
    assert_eq!(ours.len(), TICKS);
    // the bikes move: the agreement is about something
    let states: std::collections::BTreeSet<&str> = ours.iter().map(|l| &l[..16]).collect();
    assert!(states.len() > TICKS / 2, "only {} different states in {TICKS} ticks", states.len());
    if let Some(t) = (0..TICKS).find(|&t| ours[t] != theirs[t]) {
        panic!("tick {t}: the desktop {} and the browser {} (hash, pads)", ours[t], theirs[t]);
    }
    assert!(r.np.desyncs.is_empty(), "the desktop saw desyncs: {:?}", r.np.desyncs);
    assert!(state.contains("\"desyncs\":0"), "the browser saw desyncs: {state}");
    assert_eq!(desk.too_big, 0, "the desktop dropped datagrams the transport cannot carry");
    assert!(state.contains("\"too_big\":0"), "the browser dropped datagrams the transport cannot carry: {state}");
    eprintln!("{TICKS} ticks alike in {:.1} s; the browser: {state}", started.elapsed().as_secs_f32());
}
