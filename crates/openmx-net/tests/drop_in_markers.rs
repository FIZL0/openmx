//! P105: a peer dropping in on a ride under way brings its player's saved markers with its handshake —
//! it joined before it knew the ride's track, and the room takes no new pick once riding — and the
//! host puts them in the peer's pick before the ride takes it on ([`Event::Arrived`]), so the ride
//! reads them where it reads the peer's controller preset. On the LAN (mDNS), nothing off this machine.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use openmx_net::handshake::Handshake;
use openmx_net::{Board, Cmd, Disc, Event, HOST, Net, Online, Pick, Relay, Room};

/// A disc that hashes every name, and whose player kept `markers` on free06 for race125p (the
/// pick's text, `saved_markers::to_pick`'s); every question it was asked, kept.
struct Kept {
    markers: &'static str,
    asked: Mutex<Vec<(String, String)>>,
}

impl Disc for Kept {
    fn track(&self, name: &str) -> Option<u64> {
        Some(openmx_net::handshake::fnv(name.as_bytes()))
    }
    fn bike(&self, name: &str) -> Option<u64> {
        Some(openmx_net::handshake::fnv(name.as_bytes()))
    }
    fn markers(&self, track: &str, bike: &str) -> String {
        self.asked.lock().unwrap().push((track.into(), bike.into()));
        if (track, bike) == ("free06", "race125p") { self.markers.into() } else { String::new() }
    }
}

fn kept(markers: &'static str) -> Arc<Kept> {
    Arc::new(Kept { markers, asked: Mutex::new(Vec::new()) })
}

fn lan(name: &str) -> Online {
    Online { name: name.into(), relay: Relay::Off, board: Board::Off, lan: true, code_servers: Some(vec![]), ..Online::default() }
}

/// Events from `net` until `pick` takes one, within `secs`; the last room seen on the way too.
fn until<T>(net: &Net, who: &str, secs: u64, room: &mut Option<Room>, mut pick: impl FnMut(&Event) -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(secs);
    let mut seen = Vec::new();
    while let Some(left) = end.checked_duration_since(Instant::now()) {
        if let Some(e) = net.next(left) {
            if let Event::Room(r) = &e {
                *room = Some(r.clone());
            }
            if let Some(t) = pick(&e) {
                return t;
            }
            seen.push(e);
        }
    }
    panic!("{who}: nothing wanted in {secs} s; saw {seen:#?}");
}

#[test]
fn a_drop_in_brings_its_saved_markers_to_the_host_before_it_is_taken_on() {
    // the host's ride of one, open to drop-ins, on free06 and race125p
    let h = Net::start(lan("Hosty"), kept("free06:not the host's"));
    let mut r = Room::new("", "free06", 4);
    r.take(HOST, &[Pick::new("thq1", "race125p")]);
    (r.riding, r.drop_in) = (true, true);
    h.send(Cmd::Host(r));
    let mut room = None;
    let code = until(&h, "host", 30, &mut room, |e| match e {
        Event::Room(r) if !r.short.is_empty() => Some(r.short.clone()),
        _ => None,
    });

    // g picked another bike in its own lobby: the ride's is the host's, and its markers are asked for on it
    let g_disc = kept("free06:00aa11bb");
    let g = Net::start(lan("Guesty"), g_disc.clone());
    g.send(Cmd::Join(code, vec![Pick::new("thq2", "pro250a")]));
    let mut host_room = None;
    let (peer, seats) = until(&h, "host", 60, &mut host_room, |e| match e {
        Event::Arrived { peer, seats } => Some((peer.clone(), seats.clone())),
        _ => None,
    });
    assert_eq!(seats, [1]);
    assert_eq!(g_disc.asked.lock().unwrap()[..], [("free06".to_string(), "race125p".to_string())], "asked once, on the ride's track and bike");
    // the room the host had when the peer arrived: its pick carries them, on the ride's bike
    let room = host_room.expect("a room before the arrival");
    let seat = room.seats[1].as_ref().expect("g's seat");
    assert_eq!((seat.peer.as_str(), seat.pick.bike.as_str(), seat.pick.markers.as_str()), (peer.as_str(), "race125p", "free06:00aa11bb"));
    assert_eq!(room.seats[0].as_ref().unwrap().pick.markers, "", "the host's own pick is not touched");
    // and the guest's lobby shows the same
    let mut g_room = None;
    until(&g, "guest", 30, &mut g_room, |e| match e {
        Event::Room(r) if r.seats.get(1).and_then(Option::as_ref).is_some_and(|s| s.pick.markers == "free06:00aa11bb") => Some(()),
        _ => None,
    });
    g.send(Cmd::Leave);
    h.send(Cmd::Leave);
}

/// A lobby's own Start asks for no markers: they went with the lobby's picks (P99).
#[test]
fn a_lobby_start_brings_none_in_its_handshake() {
    let h = Net::start(lan("Hosty"), kept(""));
    let mut r = Room::new("", "free06", 2);
    r.take(HOST, &[Pick::new("thq1", "race125p")]);
    h.send(Cmd::Host(r));
    let mut room = None;
    let code = until(&h, "host", 30, &mut room, |e| match e {
        Event::Room(r) if !r.short.is_empty() => Some(r.short.clone()),
        _ => None,
    });
    let g_disc = kept("free06:00aa11bb");
    let g = Net::start(lan("Guesty"), g_disc.clone());
    g.send(Cmd::Join(code, vec![Pick::new("thq2", "race125p")]));
    until(&h, "host", 60, &mut room, |e| match e {
        Event::Room(r) if r.taken() == 2 => Some(()),
        _ => None,
    });
    g.send(Cmd::Prepared(true));
    until(&h, "host", 30, &mut room, |e| match e {
        Event::Room(r) if r.all_ready() => Some(()),
        _ => None,
    });
    h.send(Cmd::Start);
    until(&h, "host", 30, &mut room, |e| match e {
        Event::Start(..) => Some(()),
        _ => None,
    });
    assert!(g_disc.asked.lock().unwrap().is_empty(), "{:?}", g_disc.asked.lock().unwrap());
    g.send(Cmd::Leave);
    h.send(Cmd::Leave);
}

/// The markers are the peer's own: two handshakes that differ only in them ride together.
#[test]
fn the_markers_are_not_compared() {
    let mut r = Room::new("", "free06", 2);
    r.take(HOST, &[Pick::new("thq1", "race125p")]);
    let host = Handshake::of(&r, &openmx_net::Names);
    let joiner = Handshake { markers: "free06:00aa".into(), ..host.clone() };
    assert_eq!(joiner.check(&host), Ok(()));
}
