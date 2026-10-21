//! The terminal lobby (opentennis's `hst net`), for trying the transport before the menu's lobby screens:
//!
//! ```text
//! openmx-net host  [--track free06] [--bike race125p] [--seats 4] [--no-wrecks] [--biplane] [--private]
//! openmx-net join CODE [--bike race125p]
//! openmx-net find
//! openmx-net quick [--bike race125p]
//! ```
//!
//! Common flags: `--name N`, `--relay n0|off|URL`, `--no-lan`, `--no-board`, `--code-server URL`, `--no-codes` (no
//! pkarr relay at all: codes only on the LAN). In a lobby, lines
//! typed are commands: `ready`, `unready`, `start` (host), `bike NAME`, `lobby` (host: end the ride), `quit`;
//! anything else is a chat line. The handshake compares names only here (no disc is read).

use std::io::BufRead;
use std::sync::Arc;
use std::time::Duration;

use openmx_net::{Board, Cmd, Event, HOST, Names, Net, Online, Pick, Relay, Room};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    let opt = |f: &str| args.iter().position(|a| a == f).and_then(|i| args.get(i + 1)).cloned();
    let Some(verb) = args.first().cloned() else {
        eprintln!("usage: openmx-net host|join CODE|find|quick [flags] (see the source's header)");
        std::process::exit(2);
    };
    let mut cfg = Online { name: opt("--name").unwrap_or_else(|| "Rider".into()), lan: !flag("--no-lan"), private: flag("--private"), ..Online::default() };
    if flag("--no-board") {
        cfg.board = Board::Off;
    }
    match opt("--relay").as_deref() {
        None | Some("n0") => {}
        Some("off") => cfg.relay = Relay::Off,
        Some(url) => match openmx_net::net::relay_url(url) {
            Ok(u) => (cfg.relay, cfg.relay_url) = (Relay::Own(u.clone()), u),
            Err(e) => {
                eprintln!("--relay {url}: {e}");
                std::process::exit(2);
            }
        },
    }
    if let Some(s) = opt("--code-server") {
        cfg.code_server = s;
    }
    if flag("--no-codes") {
        cfg.code_servers = Some(Vec::new());
    }
    let pick = Pick::new("thq1", &opt("--bike").unwrap_or_else(|| "race125p".into()));
    let mut room = Room::new("", &opt("--track").unwrap_or_else(|| "free06".into()), opt("--seats").and_then(|s| s.parse().ok()).unwrap_or(4));
    room.wrecks = !flag("--no-wrecks");
    room.biplane = flag("--biplane");
    room.take(HOST, std::slice::from_ref(&pick));
    let net = Net::start(cfg, Arc::new(Names));
    match verb.as_str() {
        "host" => net.send(Cmd::Host(room)),
        "join" => {
            let Some(code) = args.get(1) else {
                eprintln!("join needs a code");
                std::process::exit(2);
            };
            net.send(Cmd::Join(code.clone(), vec![pick]));
        }
        "find" => net.send(Cmd::Find),
        "quick" => net.send(Cmd::Quick(room)),
        v => {
            eprintln!("{v}: not host, join, find or quick");
            std::process::exit(2);
        }
    }
    let (tx, lines) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for l in std::io::stdin().lock().lines().map_while(Result::ok) {
            let _ = tx.send(l);
        }
    });
    let mut found = 0;
    loop {
        for l in lines.try_iter() {
            let l = l.trim();
            match l.split_once(' ').unwrap_or((l, "")) {
                ("ready", _) => net.send(Cmd::Prepared(true)),
                ("unready", _) => net.send(Cmd::Prepared(false)),
                ("start", _) => net.send(Cmd::Start),
                ("lobby", _) => net.send(Cmd::Lobby),
                ("bike", b) if !b.is_empty() => net.send(Cmd::Picks(vec![Pick::new("thq1", b)])),
                ("quit", _) => {
                    net.send(Cmd::Leave);
                    std::thread::sleep(Duration::from_millis(300));
                    return;
                }
                ("", _) => {}
                _ => net.send(Cmd::Chat(l.into())),
            }
        }
        let Some(e) = net.next(Duration::from_millis(100)) else { continue };
        match e {
            Event::Status(s) => println!("{s}"),
            Event::Hosting => println!("Hosting."),
            Event::Code(t) => println!("Ticket: {t}"),
            Event::Short(c) => println!("Room code: {c}"),
            Event::Me(_) => {}
            Event::Found(list) => {
                for l in &list {
                    println!("  {}  {:<24} {:<10} {}/{}{}", if l.code.is_empty() { "----" } else { &l.code }, l.name, l.track, l.taken, l.seats, if !l.same_build() { "  (another version)" } else if l.riding { "  (riding: drop in)" } else { "" });
                }
                println!("{} lobbies.", list.len());
                found += 1;
                // the LAN's answer, then the board's
                if verb == "find" && found == 2 {
                    return;
                }
            }
            Event::Room(r) => {
                println!("{} on {} (wrecks {}, biplane {}){}:", r.name, r.track, if r.wrecks { "on" } else { "off" }, if r.biplane { "on" } else { "off" }, if r.short.is_empty() { String::new() } else { format!(", code {}", r.short) });
                for (i, s) in r.seats.iter().enumerate() {
                    match s {
                        Some(s) => {
                            let ready = s.peer == HOST || r.ready.contains(&s.peer);
                            let ping = r.ping.get(&s.peer).map(|p| format!(" {p} ms")).unwrap_or_default();
                            println!("  {}P {:<24} {} {}{}{ping}", i + 1, r.names.get(&s.peer).map_or("?", String::as_str), s.pick.rider, s.pick.bike, if ready { " ready" } else { "" });
                        }
                        None => println!("  {}P (open)", i + 1),
                    }
                }
            }
            Event::Start(r, me) => println!("The ride starts (seed {:08x}, you are {me}). `lobby` (host) ends it.", r.seed),
            Event::DropIn(r, me) => println!("Into the ride under way on {} (you are {me}).", r.track),
            Event::Arrived { peer, seats } => println!("{peer} is in, on seat {seats:?}: a ride takes it on."),
            Event::Dropped(why) => println!("{why}"),
            Event::Failed(why) => {
                println!("{why}");
                return;
            }
            Event::Ride(from, d) => println!("ride message from {from}: {} bytes", d.len()),
            Event::Datagram(..) => {}
            Event::Chat { name, text, .. } => println!("<{name}> {text}"),
        }
    }
}
