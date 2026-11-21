//! N27: a lobby's ride with no front end, for the shell's tests. `--lobby host` hosts a LAN room on
//! the ride's track and starts it once every seat is taken and ready; `--lobby join:CODE` joins the
//! room by its code and readies. Both then ride it as the front end's lobby would hand it over
//! ([`crate::online::Online::lobby`]), over the same connection — a ride that takes a second player
//! on this machine as it runs ([`crate::online::Online::can_sit`]), which a `--online` ride does not.
//!
//! LAN only (mDNS, no relay, no board, no code server), so nothing leaves the machine. The host says
//! `lobby: code CODE` once its room is out, for the joining shell to be started on.

use std::sync::Arc;
use std::time::{Duration, Instant};

use openmx_net::{Board, Cmd, Event, Net, Online, Pick, Relay, Room};

use crate::lobby::DiscFiles;

/// How long the lobby waits for a peer, a code or the start before giving up.
const WAIT: Duration = Duration::from_secs(300);

/// What `--lobby` asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    /// Host a room of this many seats.
    Host(usize),
    /// Join the room of this code.
    Join(String),
}

impl Ask {
    /// `host`, `host:SEATS` or `join:CODE`.
    pub fn parse(s: &str) -> Result<Ask, String> {
        let bad = || format!("--lobby: host, host:SEATS or join:CODE, not {s:?}");
        match s.split_once(':') {
            None if s == "host" => Ok(Ask::Host(2)),
            Some(("host", n)) => n.parse().ok().filter(|n| (2..=openmx_net::SEATS).contains(n)).map(Ask::Host).ok_or_else(bad),
            Some(("join", code)) if !code.is_empty() => Ok(Ask::Join(code.to_string())),
            _ => Err(bad()),
        }
    }
}

/// The lobby's ride as it started: what the front end's lobby keeps (`menu.lobby.start`, its
/// network's connection and disc files).
#[derive(Clone)]
pub struct Started {
    pub room: Room,
    pub me: String,
    pub net: Arc<Net>,
    pub disc: Arc<DiscFiles>,
}

/// Meet in the lobby as `ask` says, on `track` (the host's; a joiner rides the room's), as `name`
/// with `pick`: the ride once every machine has started it.
pub fn meet(ask: &Ask, track: &str, name: &str, pick: Pick) -> Result<Started, String> {
    let disc = Arc::new(DiscFiles::default());
    let cfg = Online { name: name.to_string(), relay: Relay::Off, board: Board::Off, lan: true, code_servers: Some(Vec::new()), ..Online::default() };
    println!("online: starting the network (LAN only)");
    let net = Arc::new(Net::start(cfg, disc.clone()));
    net.send(Cmd::Name(name.to_string()));
    let until = Instant::now() + WAIT;
    match ask {
        Ask::Host(seats) => {
            let mut room = Room::new(name, track, *seats);
            room.take(openmx_net::HOST, &[pick]);
            net.send(Cmd::Host(room));
            let mut told = false;
            let mut started = false;
            loop {
                match next(&net, until)? {
                    Event::Room(r) => {
                        if !told && !r.short.is_empty() {
                            told = true;
                            println!("lobby: code {}", r.short);
                        }
                        if !started && r.taken() == r.seats.len() && r.all_ready() {
                            started = true;
                            println!("lobby: {} seated and ready, starting", r.taken());
                            net.send(Cmd::Start);
                        }
                    }
                    Event::Start(room, me) => return Ok(Started { room, me, net, disc }),
                    e => said(e)?,
                }
            }
        }
        Ask::Join(code) => {
            net.send(Cmd::Join(code.clone(), vec![pick]));
            let mut ready = false;
            loop {
                match next(&net, until)? {
                    Event::Room(_) if !ready => {
                        ready = true;
                        net.send(Cmd::Prepared(true));
                    }
                    Event::Room(_) => {}
                    Event::Start(room, me) => return Ok(Started { room, me, net, disc }),
                    e => said(e)?,
                }
            }
        }
    }
}

/// The network's next event, before `until`.
#[cfg(not(target_arch = "wasm32"))]
fn next(net: &Net, until: Instant) -> Result<Event, String> {
    loop {
        let left = until.checked_duration_since(Instant::now()).ok_or(format!("lobby: nothing started in {} s", WAIT.as_secs()))?;
        if let Some(e) = net.next(left) {
            return Ok(e);
        }
    }
}

/// A page can't block on the network (it has no `Net::next`), and `--lobby` is a shell flag: a headless lobby
/// ride is native only.
#[cfg(target_arch = "wasm32")]
fn next(_: &Net, _: Instant) -> Result<Event, String> {
    Err("lobby: a headless lobby ride needs the native app".to_string())
}

/// An event the lobby only says, or the end of it.
fn said(e: Event) -> Result<(), String> {
    match e {
        Event::Failed(why) => Err(format!("lobby: {why}")),
        Event::Status(s) => {
            println!("lobby: {s}");
            Ok(())
        }
        Event::Dropped(why) => {
            println!("lobby: {why}");
            Ok(())
        }
        _ => Ok(()),
    }
}
