//! N3: the front end's online screens. The menu ([`menu::Menu`]) is driven by pad reports alone, as
//! `tests/menu.rs` drives it, and by the network's events: first with events written by hand, then
//! with two real `openmx_net` networks on this machine's LAN (mDNS; no relay, no board, no code
//! server — nothing leaves the machine), where a host's menu opens a lobby, a guest's menu types the
//! room code on the on-screen keyboard and joins, says ready, and the host's Start gives both menus the
//! same ride and seed; a guest leaving mid-ride puts the host back in its lobby; a guest whose disc
//! holds another race125p is refused and told why. Last, what the shell hashes for the handshake,
//! against the disc when there is one.

// the browser's own clock, threads and files, which are std's here (P196)
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

#[path = "../src/lobby.rs"]
#[allow(dead_code)]
mod lobby;

#[path = "../src/track_mods.rs"]
#[allow(dead_code)]
mod track_mods;

#[path = "../src/trick_packs.rs"]
#[allow(dead_code)]
mod trick_packs;

#[path = "../src/join.rs"]
#[allow(dead_code)]
mod join;

#[path = "../src/split.rs"]
#[allow(dead_code)]
mod split;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use game_sim::input::{Control, Report};
use menu::{Act, Controls, Item, LOBBY_SEATS, Menu, Reader, Screen, Settings};
use openmx_net::{Board, Cmd, Disc, Event, HOST, Net, Online, Pick, Relay, Room};

fn tap(menu: &mut Menu, reader: &mut Reader, control: Control) -> Vec<Act> {
    let mut acts = Vec::new();
    for report in [Report::from_buttons(control.mask()), Report::idle(), Report::idle(), Report::idle()] {
        acts.push(menu.step(reader.read(&report)));
    }
    acts
}

fn taps(menu: &mut Menu, reader: &mut Reader, controls: &[Control]) {
    for &c in controls {
        tap(menu, reader, c);
    }
}

/// A menu named `name` riding race125p as `rider`, with two bike sets on offer, at rest.
fn fresh(name: &str, rider: &str) -> (Menu, Reader) {
    let mut reader = Reader::new();
    let settings = Settings { name: name.into(), ..Settings::default() };
    let mut menu = Menu::new(settings, Controls::default());
    menu.lobby.pick = Pick::new(rider, "race125p");
    menu.lobby.bikes = vec![("race125p".into(), "125 Race P".into()), ("pro125a".into(), "125 Pro A".into())];
    menu.step(reader.read(&Report::idle()));
    (menu, reader)
}

/// The on-screen keyboard's letters, as `menu.rs` lays them out.
const KEYS: [&str; 4] = ["abcdefghij", "klmnopqrst", "uvwxyz0123", "456789-'.&"];

/// Types `text` on the keyboard from its first key, then Start to finish.
fn type_text(menu: &mut Menu, reader: &mut Reader, text: &str) {
    assert_eq!(menu.screen(), Some(Screen::Keyboard));
    let (mut row, mut col) = (0usize, 0usize);
    for c in text.to_ascii_lowercase().chars() {
        let (r, k) = KEYS
            .iter()
            .enumerate()
            .find_map(|(r, keys)| keys.find(c).map(|k| (r, k)))
            .unwrap_or_else(|| panic!("{c} is not on the keyboard"));
        while row != r {
            tap(menu, reader, if r > row { Control::Down } else { Control::Up });
            row = if r > row { row + 1 } else { row - 1 };
        }
        while col != k {
            tap(menu, reader, Control::Right);
            col = (col + 1) % 10;
        }
        tap(menu, reader, Control::Cross);
    }
    tap(menu, reader, Control::Start);
}

/// Main menu → Online, under Free Ride and the garage.
fn to_online(menu: &mut Menu, reader: &mut Reader) {
    taps(menu, reader, &[Control::Down, Control::Down, Control::Cross]);
    assert_eq!(menu.screen(), Some(Screen::Online));
}

fn guest_room() -> Room {
    let mut room = Room::new("Hosty", "free06", LOBBY_SEATS);
    room.take(HOST, &[Pick::new("thq1", "race125p")]);
    room.take("guest-id", &[Pick::new("thq2", "race125p")]);
    room.short = "AB2C".into();
    room
}

#[test]
fn the_online_screens_go_by_pad_and_the_network_s_word_alone() {
    let (mut m, mut r) = fresh("Hosty", "thq1");
    to_online(&mut m, &mut r);

    // Host: the lobby opens at once, its room asked for with this player seated as the host.
    taps(&mut m, &mut r, &[Control::Down, Control::Cross]);
    assert_eq!(m.screen(), Some(Screen::Lobby));
    let Some(Cmd::Host(room)) = m.lobby.out.pop() else { panic!("{:?}", m.lobby.out) };
    assert_eq!(room.seats.len(), LOBBY_SEATS);
    assert_eq!(room.track, m.settings.track);
    assert_eq!(room.seats[0].as_ref().map(|s| (s.peer.as_str(), s.pick.clone())), Some((HOST, m.lobby.pick.clone())));
    assert_eq!(m.lobby.items(), vec![Item::Leave], "no lobby yet: only a way out");

    m.heard(Event::Me(HOST.into()));
    m.heard(Event::Short("AB2C".into()));
    m.heard(Event::Room(room.clone()));
    assert_eq!(m.lobby.code, "AB2C");
    assert_eq!(m.lobby.items()[..3], [Item::Track, Item::Wrecks, Item::Bike]);
    // Right on the track: the next one; down to the bike and right: the next set.
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.lobby.out.pop(), Some(Cmd::Ride { track: "free07".into(), wrecks: room.wrecks, biplane: false }));
    taps(&mut m, &mut r, &[Control::Down, Control::Down, Control::Right]);
    assert_eq!(m.lobby.out.pop(), Some(Cmd::Picks(vec![Pick::new("thq1", "pro125a")])));
    // Past Private, the seats (N12): right opens one more, left from two goes round to the cap,
    // and the next lobby this machine hosts opens with the last.
    taps(&mut m, &mut r, &[Control::Down, Control::Down, Control::Right]);
    assert_eq!(m.lobby.out.pop(), Some(Cmd::Seats(LOBBY_SEATS + 1)));
    assert_eq!(m.settings.lobby_seats, LOBBY_SEATS + 1);
    let mut two = room.clone();
    two.seats.truncate(2);
    m.heard(Event::Room(two));
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.lobby.out.pop(), Some(Cmd::Seats(openmx_net::SEATS)));
    m.heard(Event::Room(room.clone()));
    // Start is the host's.
    taps(&mut m, &mut r, &[Control::Down, Control::Down, Control::Cross]);
    assert_eq!(m.lobby.out.pop(), Some(Cmd::Start));
    // Circle leaves the lobby for the online menu.
    tap(&mut m, &mut r, Control::Circle);
    assert_eq!(m.lobby.out.pop(), Some(Cmd::Leave));
    assert_eq!(m.screen(), Some(Screen::Online));
    assert!(m.lobby.room.is_none());

    // Join by Code: four rows down from Host is Join by Code (wrapping past the network row).
    let (mut m, mut r) = fresh("Guesty", "thq2");
    to_online(&mut m, &mut r);
    taps(&mut m, &mut r, &[Control::Down, Control::Down, Control::Down, Control::Cross]);
    type_text(&mut m, &mut r, "ab2c");
    assert_eq!(m.screen(), Some(Screen::Lobby));
    assert_eq!(m.lobby.out.pop(), Some(Cmd::Join("AB2C".into(), vec![Pick::new("thq2", "race125p")])));
    m.heard(Event::Me("guest-id".into()));
    m.heard(Event::Room(guest_room()));
    assert_eq!(m.lobby.items(), vec![Item::Bike, Item::Chat, Item::Ready, Item::Modifiers, Item::Leave]);
    // Ready, and chat on the keyboard.
    taps(&mut m, &mut r, &[Control::Down, Control::Down, Control::Cross]);
    assert_eq!(m.lobby.out.pop(), Some(Cmd::Prepared(true)));
    taps(&mut m, &mut r, &[Control::Up, Control::Cross]);
    type_text(&mut m, &mut r, "hi");
    assert_eq!(m.lobby.out.pop(), Some(Cmd::Chat("hi".into())));
    assert_eq!(m.lobby.chat, vec!["Guesty: hi".to_string()]);
    m.heard(Event::Chat { from: HOST.into(), name: "Hosty".into(), text: "go".into() });
    assert_eq!(m.lobby.chat.last().map(String::as_str), Some("Hosty: go"));

    // The host's Start: the ride, with nothing over it.
    let mut riding = guest_room();
    riding.seed = 0x1234_5678;
    riding.riding = true;
    assert_eq!(m.heard(Event::Start(riding.clone(), "guest-id".into())), Act::Online);
    assert!(m.riding());
    assert_eq!(m.lobby.start.as_ref().map(|s| s.0.seed), Some(0x1234_5678));
    // A peer drops: the ride ends and the lobby is where it comes back to, saying why.
    m.heard(Event::Dropped("Thirdy left the ride.".into()));
    let mut back = guest_room();
    back.riding = false;
    m.heard(Event::Room(back));
    m.back_from_online();
    assert_eq!(m.screen(), Some(Screen::Lobby));
    assert_eq!(m.note.as_deref(), Some("Thirdy left the ride."));
    // The host leaving ends the lobby: back to the online menu.
    m.heard(Event::Failed("The host left.".into()));
    assert_eq!(m.screen(), Some(Screen::Online));
    assert!(m.lobby.room.is_none());
}

#[test]
fn the_network_row_switches_to_the_lan_only() {
    let (mut m, mut r) = fresh("Rider", "thq1");
    to_online(&mut m, &mut r);
    assert!(!m.lobby.lan);
    tap(&mut m, &mut r, Control::Up);
    tap(&mut m, &mut r, Control::Right);
    assert!(m.lobby.lan && m.lobby.network_changed);
    assert!(m.lobby.out.is_empty(), "switching the network asks the network nothing");
}

// ---------------------------------------------------------------------------------------------------
// Two menus, two real networks
// ---------------------------------------------------------------------------------------------------

/// A disc whose race125p hashes as given.
struct Bikes(u64);

impl Disc for Bikes {
    fn track(&self, name: &str) -> Option<u64> {
        Some(openmx_net::handshake::fnv(name.as_bytes()))
    }
    fn bike(&self, name: &str) -> Option<u64> {
        HashMap::from([("race125p", self.0), ("pro125a", 7)]).get(name).copied()
    }
}

/// The LAN only, as the network row's "LAN only" starts it.
fn lan(name: &str, race125p: u64) -> Net {
    let cfg = Online { name: name.into(), relay: Relay::Off, board: Board::Off, lan: true, code_servers: Some(vec![]), ..Online::default() };
    Net::start(cfg, Arc::new(Bikes(race125p)))
}

/// What the shell's `lobby::Network::pump` does a frame: the menu's commands out, the events in.
fn pump(m: &mut Menu, net: &Net) -> Vec<Act> {
    for c in std::mem::take(&mut m.lobby.out) {
        if matches!(c, Cmd::Host(_) | Cmd::Join(..) | Cmd::Quick(_)) {
            net.send(Cmd::Name(m.settings.name.clone()));
        }
        net.send(c);
    }
    net.poll().into_iter().map(|e| m.heard(e)).collect()
}

/// Pumps both menus until `done` says so, for up to 30 s.
fn until(a: (&mut Menu, &Net), b: (&mut Menu, &Net), what: &str, mut done: impl FnMut(&Menu, &Menu, &[Act], &[Act]) -> bool) {
    let end = Instant::now() + Duration::from_secs(30);
    let (mut acts_a, mut acts_b) = (Vec::new(), Vec::new());
    while Instant::now() < end {
        acts_a.extend(pump(a.0, a.1));
        acts_b.extend(pump(b.0, b.1));
        if done(a.0, b.0, &acts_a, &acts_b) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("{what}: not in 30 s; host {:?} / guest {:?}", a.0.lobby, b.0.lobby);
}

/// The host's menu hosts and the guest's menu types its code; both wait until both are seated.
fn host_and_join(h: &mut Menu, hr: &mut Reader, hn: &Net, g: &mut Menu, gr: &mut Reader, gn: &Net) {
    to_online(h, hr);
    taps(h, hr, &[Control::Down, Control::Cross]);
    until((h, hn), (g, gn), "the host's room code", |h, _, _, _| !h.lobby.code.is_empty() && h.lobby.room.is_some());
    let code = h.lobby.code.clone();
    to_online(g, gr);
    taps(g, gr, &[Control::Down, Control::Down, Control::Down, Control::Cross]);
    type_text(g, gr, &code);
    until((h, hn), (g, gn), "both seated", |h, g, _, _| {
        let two = |m: &Menu| m.lobby.room.as_ref().is_some_and(|r| r.taken() == 2);
        two(h) && two(g)
    });
    assert_eq!(g.lobby.code, code, "the guest's lobby shows the code to pass on");
}

#[test]
fn a_host_s_menu_and_a_joiner_s_find_each_other_by_room_code_on_the_lan_and_ride() {
    let (hn, gn) = (lan("Hosty", 1), lan("Guesty", 1));
    let (mut h, mut hr) = fresh("Hosty", "thq1");
    let (mut g, mut gr) = fresh("Guesty", "thq2");
    host_and_join(&mut h, &mut hr, &hn, &mut g, &mut gr, &gn);
    // The players with their riders and bikes, on both.
    for m in [&h, &g] {
        let room = m.lobby.room.as_ref().unwrap();
        let seated: Vec<(&str, &str)> = room.seats.iter().flatten().map(|s| (s.pick.rider.as_str(), s.pick.bike.as_str())).collect();
        assert_eq!(seated, [("thq1", "race125p"), ("thq2", "race125p")]);
        assert_eq!(room.names.values().cloned().collect::<Vec<_>>().len(), 2);
    }
    // The guest says ready; the host starts once it shows.
    taps(&mut g, &mut gr, &[Control::Down, Control::Down, Control::Cross]);
    until((&mut h, &hn), (&mut g, &gn), "ready", |h, _, _, _| h.lobby.room.as_ref().is_some_and(|r| r.all_ready()));
    let start = h.lobby.items().iter().position(|i| *i == Item::Start).unwrap();
    for _ in 0..start {
        tap(&mut h, &mut hr, Control::Down);
    }
    tap(&mut h, &mut hr, Control::Cross);
    let mut rode = (false, false);
    until((&mut h, &hn), (&mut g, &gn), "the ride", |_, _, ha, ga| {
        rode = (ha.contains(&Act::Online), ga.contains(&Act::Online));
        rode == (true, true)
    });
    let (hs, gs) = (h.lobby.start.clone().unwrap(), g.lobby.start.clone().unwrap());
    assert_eq!(hs.0.seed, gs.0.seed, "one seed");
    assert_ne!(hs.0.seed, 0);
    assert_eq!((hs.1.as_str(), hs.0.seats.len()), (HOST, LOBBY_SEATS));
    assert!(h.riding() && g.riding());
    // The guest leaves the ride (its pause's Main Menu, `Online::leave`): the host's room rides on
    // without its seat (N11), and the host is told who went.
    gn.send(Cmd::Leave);
    until((&mut h, &hn), (&mut g, &gn), "the drop", |h, _, _, _| {
        h.lobby.status == "Guesty left the ride." && h.lobby.room.as_ref().is_some_and(|r| r.riding && r.taken() == 1)
    });
    assert!(h.riding(), "the host rides on");
}

#[test]
fn a_joiner_s_menu_whose_bike_set_differs_is_refused_and_told_why() {
    let (hn, gn) = (lan("Hosty", 1), lan("Guesty", 2));
    let (mut h, mut hr) = fresh("Hosty", "thq1");
    let (mut g, mut gr) = fresh("Guesty", "thq2");
    host_and_join(&mut h, &mut hr, &hn, &mut g, &mut gr, &gn);
    taps(&mut g, &mut gr, &[Control::Down, Control::Down, Control::Cross]);
    until((&mut h, &hn), (&mut g, &gn), "ready", |h, _, _, _| h.lobby.room.as_ref().is_some_and(|r| r.all_ready()));
    h.lobby.out.push(Cmd::Start);
    until((&mut h, &hn), (&mut g, &gn), "the refusal", |h, g, ha, ga| {
        assert!(!ha.contains(&Act::Online) && !ga.contains(&Act::Online), "nobody rides");
        g.screen() == Some(Screen::Online) && h.lobby.room.as_ref().is_some_and(|r| r.taken() == 1)
    });
    let why = g.note.clone().unwrap_or_default();
    assert!(why.contains("race125p"), "{why}");
    assert!(g.lobby.room.is_none());
    println!("guest told: {why}");
}

/// A `mods/` folder in the temp dir holding one MX vs. ATV trick pack, `mxvatv`, whose race125p file
/// says `phy`.
fn trick_mods(name: &str, phy: &str) -> std::path::PathBuf {
    let mods = std::env::temp_dir().join(format!("openmx-md19-{}-{name}", std::process::id())).join("mods");
    let pack = mods.join(trick_packs::TRICKS_DIR).join("mxvatv");
    let _ = std::fs::remove_dir_all(&mods);
    std::fs::create_dir_all(pack.join("Bike")).unwrap();
    let json = r#"{"kind": "tricks", "id": "mxvatv", "riders": [{"rider": "MX", "table": "stunts.ini"}], "phy": ["Bike/mx_race125p.phy"]}"#;
    std::fs::write(pack.join("mod.json"), json).unwrap();
    std::fs::write(pack.join("stunts.ini"), "[Stunt01]\nName=threesixtyL\n").unwrap();
    std::fs::write(pack.join("Bike/mx_race125p.phy"), phy).unwrap();
    mods
}

/// The LAN only, on the shell's own [`lobby::DiscFiles`] reading trick packs from `mods`.
fn lan_with_tricks(name: &str, mods: &std::path::Path) -> Net {
    let cfg = Online { name: name.into(), relay: Relay::Off, board: Board::Off, lan: true, code_servers: Some(vec![]), ..Online::default() };
    let files = lobby::DiscFiles::default();
    files.tricks_from(Some(mods.to_path_buf()));
    Net::start(cfg, Arc::new(files))
}

/// [`pump`], naming the trick pack in the host's room as `lobby::Network::pump` does (MD19).
fn pump_tricks(m: &mut Menu, net: &Net, mods: &std::path::Path) -> Vec<Act> {
    lobby::name_tricks(&mut m.lobby.out, m.settings.tricks, Some(mods));
    pump(m, net)
}

/// MD19: a host on MX vs. ATV's tricks names its pack in the room; a guest with the same pack's
/// files rides, and one whose race125p file in it differs is refused with the pack named.
#[test]
fn every_seat_of_an_online_ride_has_the_hosts_trick_pack() {
    for (guest_phy, rides) in [("[Three Sixty]\n", true), ("[Three Sixty]\nchanged = 1\n", false)] {
        let (hm, gm) = (trick_mods(&format!("host-{rides}"), "[Three Sixty]\n"), trick_mods(&format!("guest-{rides}"), guest_phy));
        let (hn, gn) = (lan_with_tricks("Hosty", &hm), lan_with_tricks("Guesty", &gm));
        let (mut h, mut hr) = fresh("Hosty", "thq1");
        let (mut g, mut gr) = fresh("Guesty", "thq2");
        h.settings.tricks = menu::Tricks::MxVsAtv;
        to_online(&mut h, &mut hr);
        taps(&mut h, &mut hr, &[Control::Down, Control::Cross]);
        let both = |h: &mut Menu, g: &mut Menu, what: &str, done: &mut dyn FnMut(&Menu, &Menu, &[Act], &[Act]) -> bool| {
            let end = Instant::now() + Duration::from_secs(30);
            let (mut ha, mut ga) = (Vec::new(), Vec::new());
            while Instant::now() < end {
                ha.extend(pump_tricks(h, &hn, &hm));
                ga.extend(pump_tricks(g, &gn, &gm));
                if done(h, g, &ha, &ga) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            panic!("{what}: not in 30 s; host {:?} / guest {:?}", h.lobby, g.lobby);
        };
        both(&mut h, &mut g, "the host's room code", &mut |h, _, _, _| !h.lobby.code.is_empty() && h.lobby.room.is_some());
        assert_eq!(h.lobby.room.as_ref().unwrap().tricks, "mxvatv", "the host's room names its pack");
        let code = h.lobby.code.clone();
        to_online(&mut g, &mut gr);
        taps(&mut g, &mut gr, &[Control::Down, Control::Down, Control::Down, Control::Cross]);
        type_text(&mut g, &mut gr, &code);
        both(&mut h, &mut g, "both seated", &mut |h, g, _, _| [h, g].iter().all(|m| m.lobby.room.as_ref().is_some_and(|r| r.taken() == 2)));
        taps(&mut g, &mut gr, &[Control::Down, Control::Down, Control::Cross]);
        both(&mut h, &mut g, "ready", &mut |h, _, _, _| h.lobby.room.as_ref().is_some_and(|r| r.all_ready()));
        h.lobby.out.push(Cmd::Start);
        if rides {
            both(&mut h, &mut g, "the ride", &mut |_, _, ha, ga| ha.contains(&Act::Online) && ga.contains(&Act::Online));
            assert_eq!(g.lobby.start.as_ref().unwrap().0.tricks, "mxvatv", "the guest rides the room's pack");
        } else {
            both(&mut h, &mut g, "the refusal", &mut |h, g, ha, ga| {
                assert!(!ha.contains(&Act::Online) && !ga.contains(&Act::Online), "nobody rides");
                g.screen() == Some(Screen::Online) && h.lobby.room.as_ref().is_some_and(|r| r.taken() == 1)
            });
            let why = g.note.clone().unwrap_or_default();
            assert!(why.contains("trick pack mxvatv"), "{why}");
            println!("guest told: {why}");
        }
        for m in [hm, gm] {
            let _ = std::fs::remove_dir_all(m.parent().unwrap());
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// What the shell hashes
// ---------------------------------------------------------------------------------------------------

#[test]
fn the_disc_s_hashes_are_of_what_the_ride_loads() {
    if game_render::Disc::open_configured().is_err() {
        println!("no disc configured: the disc's hashes not checked");
        return;
    }
    let files = lobby::DiscFiles::default();
    let race = files.phy("race125p").expect("race125p on the disc");
    let pro = files.phy("pro125a").expect("pro125a on the disc");
    // The bytes are a `.phy` the ride can read, and what the handshake compares is their hash.
    let tables = game_sim::rider::Tables::build();
    game_sim::phy::Phy::read_bytes(&race, &tables.ride.curve);
    assert_eq!(files.bike("race125p"), Some(openmx_net::handshake::fnv(&race)));
    assert_ne!(files.bike("race125p"), files.bike("pro125a"));
    assert_ne!(race, pro);
    // Every one of the sixteen sets is on the retail disc.
    for (set, label) in lobby::bikes() {
        assert!(files.bike(&set).is_some(), "{set} ({label})");
    }
    assert_eq!(files.bike("race999x"), None);
    // Each freeride track hashes, and differently.
    let hashes: Vec<u64> = menu::TRACKS.iter().map(|t| files.track(t.0).unwrap_or_else(|| panic!("{}", t.0))).collect();
    for (i, a) in hashes.iter().enumerate() {
        assert!(!hashes[i + 1..].contains(a), "{} hashes as another track", menu::TRACKS[i].0);
    }
    assert_eq!(files.track("nosuchtrack"), None);
    println!("race125p {:#018x}, free06 {:#018x}", files.bike("race125p").unwrap(), hashes[0]);
}

// ---------------------------------------------------------------------------------------------------
// The lobby's ride
// ---------------------------------------------------------------------------------------------------

#[path = "../src/net/mod.rs"]
#[allow(dead_code)]
mod net;

#[path = "../src/online.rs"]
#[allow(dead_code)]
mod online;

#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

#[path = "../src/saved_markers.rs"]
#[allow(dead_code)]
mod saved_markers;

#[path = "../src/peer_looks.rs"]
#[allow(dead_code)]
mod peer_looks;

/// The `Event::Start` a network says, waiting up to 30 s.
fn started(net: &Net, who: &str) -> (Room, String) {
    let end = Instant::now() + Duration::from_secs(30);
    while Instant::now() < end {
        for e in net.poll() {
            if let Event::Start(room, me) = e {
                return (room, me);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("{who}: no start in 30 s");
}

#[test]
fn the_lobby_s_ride_goes_over_the_lobby_s_connection_in_step_and_a_leaver_lets_the_host_ride_on() {
    use game_sim::world::Flat;
    let (hn, gn) = (Arc::new(lan("Hosty", 1)), Arc::new(lan("Guesty", 1)));
    let (mut h, mut hr) = fresh("Hosty", "thq1");
    let (mut g, mut gr) = fresh("Guesty", "thq2");
    host_and_join(&mut h, &mut hr, &hn, &mut g, &mut gr, &gn);
    gn.send(Cmd::Prepared(true));
    until((&mut h, &hn), (&mut g, &gn), "ready", |h, _, _, _| h.lobby.room.as_ref().is_some_and(|r| r.all_ready()));
    hn.send(Cmd::Start);
    let ((hroom, hme), (groom, gme)) = (started(&hn, "host"), started(&gn, "guest"));
    assert_eq!((hroom.seed, &hroom.seats), (groom.seed, &groom.seats));
    // A sim a seat, every seat on the room's seed, as the shell's ride from the lobby has them (the
    // stock bike here: no disc).
    let sims = |room: &Room| -> Vec<game_sim::Sim> {
        let first = game_sim::Sim::new(1);
        (0..room.seats.len())
            .map(|_| {
                let mut s = first.clone();
                s.rng = room.seed;
                s
            })
            .collect()
    };
    let mut ho = online::Online::lobby(hn.clone(), &hroom, &hme, sims(&hroom)).unwrap();
    let mut go = online::Online::lobby(gn.clone(), &groom, &gme, sims(&groom)).unwrap();
    let (mut hw, mut gw) = (Flat::default(), Flat::default());
    // Four seconds of riding at the ride's 30 Hz (N6): the host on the gas, the guest leaning.
    let gas = Report::from_buttons(game_sim::input::Control::Cross.mask());
    let mut lean = Report::idle();
    lean.set_axis(2, 0xe0);
    for _ in 0..120 {
        ho.frame(gas, &mut hw, None).unwrap();
        go.frame(lean, &mut gw, None).unwrap();
        std::thread::sleep(Duration::from_millis(33));
    }
    let ((ha, hd), (ga, gd)) = (ho.settled(), go.settled());
    let both = ha.len().min(ga.len());
    println!("settled: host {} guest {} ticks; {}", ha.len(), ga.len(), ho.summary());
    assert!(both >= 60, "only {both} ticks settled on both in 4 s");
    assert_eq!(ha[..both], ga[..both], "the two machines' rides parted");
    assert_eq!((hd, gd), (0, 0), "desyncs");
    // The guest leaves from the pause's Leave Session (N11): its seat goes at a tick both agree on,
    // its ride ends once the host has that, and the host rides on alone, its room still riding.
    go.leave_session(0).unwrap();
    assert!(go.leaving());
    let end = Instant::now() + Duration::from_secs(15);
    let why = loop {
        assert!(Instant::now() < end, "the guest never got out of the ride");
        ho.frame(gas, &mut hw, None).expect("the host's ride ended when the guest left");
        if let Err(why) = go.frame(lean, &mut gw, None) {
            break why;
        }
        std::thread::sleep(Duration::from_millis(33));
    };
    println!("guest's ride ended: {why}");
    assert!(why.contains("you left the ride"), "{why}");
    for _ in 0..90 {
        ho.frame(gas, &mut hw, None).expect("the host's ride ended after the guest left");
        std::thread::sleep(Duration::from_millis(33));
    }
    assert_eq!(ho.riders(), 1, "the guest's seat is out of the host's ride");
    for e in ho.kept() {
        h.heard(e);
    }
    until((&mut h, &hn), (&mut g, &gn), "the host's room with one seat", |h, _, _, _| h.lobby.room.as_ref().is_some_and(|r| r.riding && r.taken() == 1));
}

/// N10, the menu's side: a local freeride's pause offers "Open to Online" above Main Menu and Quit
/// (P211), which asks the shell to open the ride, and then shows the room code with Seats under it; a
/// ride that cannot be opened has no such row. A guest's join that lands in a ride under way is the lobby's ride with
/// [`menu::Lobby::drop_in`] set, and Find says a listing is a ride to drop in on.
#[test]
fn a_freerides_pause_opens_it_online() {
    use menu::Opening;
    let (mut m, mut r) = fresh("Hosty", "thq1");
    m.pause();
    assert!(!m.pause_rows().iter().any(|row| row.contains("Online")), "{:?}", m.pause_rows());
    m.opening = Opening::Can;
    let rows = m.pause_rows();
    assert_eq!(rows[rows.len() - 3..], ["Open to Online", "Main Menu", "Quit"]);
    // no Seats before the ride is opened (P211)
    assert!(!rows.iter().any(|row| row.starts_with("Seats")), "{rows:?}");
    let online = rows.len() - 3;
    // five rows down from Resume: Open to Online, and the pause closes over a ride that goes on
    let mut acts = Vec::new();
    for _ in 0..online {
        acts.extend(tap(&mut m, &mut r, Control::Down));
    }
    acts.extend(tap(&mut m, &mut r, Control::Cross));
    assert_eq!(acts.last(), Some(&Act::Stay));
    assert!(acts.contains(&Act::OpenOnline), "{acts:?}");
    assert!(m.riding());
    // opened: the row is the code once the network says it, and pressing it does nothing
    m.opening = Opening::Open;
    m.pause();
    assert_eq!(m.pause_rows()[online], "Online: opening...");
    m.heard(Event::Short("QKX4".into()));
    assert_eq!(m.pause_rows()[online], "Online: QKX4");
    assert_eq!(m.pause_rows()[online + 1..], [format!("Seats: {LOBBY_SEATS}"), "Main Menu".into(), "Quit".into()]);
    taps(&mut m, &mut r, &vec![Control::Down; online]);
    let acts = tap(&mut m, &mut r, Control::Cross);
    assert!(acts.iter().all(|a| *a == Act::Stay) && m.screen() == Some(Screen::Pause), "{acts:?}");
    // the last row is still Quit, under Seats (N14) and Main Menu
    // (which asks first since P110: Up off Stay onto Quit)
    let acts = taps_acts(&mut m, &mut r, &[Control::Down, Control::Down, Control::Down, Control::Cross, Control::Up, Control::Cross]);
    assert!(acts.contains(&Act::Quit), "{acts:?}");

    // a guest dropping in
    let (mut g, _) = fresh("Guesty", "thq2");
    let mut riding = guest_room();
    (riding.riding, riding.drop_in) = (true, true);
    assert_eq!(g.heard(Event::DropIn(riding.clone(), "guest-id".into())), Act::Online);
    assert!(g.lobby.drop_in && g.riding());
    assert_eq!(g.lobby.start.as_ref().map(|s| s.1.as_str()), Some("guest-id"));
    assert_eq!(g.heard(Event::Arrived { peer: "x".into(), seats: vec![2] }), Act::Stay);
    g.back_from_online();
    assert!(!g.lobby.drop_in);
    assert_eq!(g.heard(Event::Start(riding, "guest-id".into())), Act::Online);
    assert!(!g.lobby.drop_in, "a ride that starts is not a drop-in");
}

/// N14: the pause of an opened ride has a Seats row under the online row (only once opened, P211:
/// before that the lobby's pick the room opens with is the online row's chip). It asks the network,
/// keeps the pick as the lobby's, and shows the room's count once the network says it.
#[test]
fn an_opened_rides_pause_has_a_seats_row() {
    use menu::Opening;
    let (mut m, mut r) = fresh("Hosty", "thq1");
    m.opening = Opening::Can;
    m.pause();
    assert!(!m.pause_rows().iter().any(|row| row.starts_with("Seats")), "{:?}", m.pause_rows());
    m.opening = Opening::Open;
    let seats = m.pause_rows().len() - 3;
    assert_eq!(m.pause_rows()[seats], format!("Seats: {LOBBY_SEATS}"));
    taps(&mut m, &mut r, &vec![Control::Down; seats]);
    let acts = taps_acts(&mut m, &mut r, &[Control::Right, Control::Right, Control::Left, Control::Cross]);
    assert!(acts.iter().all(|a| *a == Act::Stay) && m.screen() == Some(Screen::Pause), "{acts:?}");
    assert_eq!(m.settings.lobby_seats, LOBBY_SEATS + 2);
    assert_eq!(m.pause_rows()[seats], format!("Seats: {}", LOBBY_SEATS + 2));
    assert_eq!(m.lobby.out.last(), Some(&Cmd::Seats(LOBBY_SEATS + 2)), "each step is asked of the room");
    assert!(m.changed(), "the pick is saved as the lobby's is");
    // round from two to the cap
    m.settings.lobby_seats = 2;
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.lobby_seats, openmx_net::SEATS);
    m.lobby.out.clear();

    // the room's own count, once the network says it
    let mut room = Room::new("Hosty", "free06", 5);
    room.take(HOST, &[Pick::new("thq1", "race125p")]);
    (room.riding, room.drop_in) = (true, true);
    m.heard(Event::Room(room.clone()));
    assert_eq!(m.pause_rows()[seats], "Seats: 5");
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.lobby.out.pop(), Some(Cmd::Seats(6)));
    assert_eq!(m.pause_rows()[seats], "Seats: 5", "the room's count until the network says otherwise");
    room.seats.resize(6, None);
    m.heard(Event::Room(room));
    assert_eq!(m.pause_rows()[seats], "Seats: 6");
    assert_eq!(m.settings.lobby_seats, 6);
    assert_eq!(m.screen(), Some(Screen::Pause));
}

/// N11, the menu's side: an online ride's pause has "Leave Session" just above Main Menu and Quit
/// (under an opened ride's online rows, P211), which asks first and then hands the shell the pausing player's seat to let
/// go, the ride back on under it. A guest the room is handed to hosts the lobby from there.
#[test]
fn an_online_rides_pause_leaves_the_session() {
    use menu::Opening;
    let (mut m, mut r) = fresh("Guesty", "thq2");
    m.pause();
    assert!(!m.pause_rows().iter().any(|row| row == "Leave Session"), "a local ride has none");
    m.session = true;
    m.pause();
    let rows = m.pause_rows();
    assert_eq!(rows[rows.len() - 3..], ["Leave Session", "Main Menu", "Quit"]);
    let leave = rows.len() - 3;
    taps(&mut m, &mut r, &vec![Control::Down; leave]);
    let acts = tap(&mut m, &mut r, Control::Cross);
    assert!(acts.iter().all(|a| *a == Act::Stay), "{acts:?}");
    assert_eq!(m.confirming().map(|c| (c.1, c.2)), Some((["Leave Session", "Stay"], 1)), "asks first, on Stay");
    let acts = taps_acts(&mut m, &mut r, &[Control::Up, Control::Cross]);
    assert!(acts.contains(&Act::LeaveSession(0)), "{acts:?}");
    assert!(m.riding(), "back to the ride while it lets go");
    // a second local player's pause, their own (P84), lets their seat alone go
    m.pause_for(1);
    let acts = m.with_seat(1, |m| {
        taps(m, &mut r, &vec![Control::Down; leave]);
        taps_acts(m, &mut r, &[Control::Cross, Control::Up, Control::Cross])
    });
    assert!(acts.contains(&Act::LeaveSession(1)), "{acts:?}");
    // Stay keeps the ride
    m.pause();
    taps(&mut m, &mut r, &vec![Control::Down; leave]);
    let acts = taps_acts(&mut m, &mut r, &[Control::Cross, Control::Cross]);
    assert!(!acts.iter().any(|a| matches!(a, Act::LeaveSession(_))) && m.screen() == Some(Screen::Pause), "{acts:?}");
    // an opened ride: under its online rows
    m.opening = Opening::Open;
    let rows = m.pause_rows();
    assert_eq!(rows[rows.len() - 5..], ["Online: opening...", &format!("Seats: {LOBBY_SEATS}"), "Leave Session", "Main Menu", "Quit"]);

    // the host gone, the room handed to this machine: it hosts the lobby
    m.heard(Event::Me("guest-id".into()));
    m.heard(Event::Room(guest_room()));
    assert!(!m.lobby.hosting());
    m.heard(Event::HandedOver("someone-else".into()));
    assert!(!m.lobby.hosting());
    m.heard(Event::HandedOver("guest-id".into()));
    assert!(m.lobby.hosting());
    assert_eq!(m.lobby.status, "The host left: you host the game now.");
}

fn taps_acts(menu: &mut Menu, reader: &mut Reader, controls: &[Control]) -> Vec<Act> {
    controls.iter().flat_map(|&c| tap(menu, reader, c)).collect()
}

/// N13: this machine's player's controller preset rides in its pick — the lobby it hosts or joins
/// says it, a change in an open lobby goes out as a repick, and a ride under way keeps the preset it
/// started on.
#[test]
fn the_players_controller_preset_rides_in_the_pick() {
    let (mut m, mut r) = fresh("Guesty", "thq2");
    m.lobby.preset(2);
    assert!(m.lobby.out.is_empty(), "no lobby yet: nothing to tell");
    to_online(&mut m, &mut r);
    taps(&mut m, &mut r, &[Control::Down, Control::Down, Control::Down, Control::Cross]);
    type_text(&mut m, &mut r, "ab2c");
    let on = |preset: u8| Pick { preset, ..Pick::new("thq2", "race125p") };
    assert_eq!(m.lobby.out.pop(), Some(Cmd::Join("AB2C".into(), vec![on(2)])));
    m.heard(Event::Me("guest-id".into()));
    m.heard(Event::Room(guest_room()));
    m.lobby.preset(2);
    assert!(m.lobby.out.is_empty(), "the same preset: nothing new");
    m.lobby.preset(9);
    assert_eq!(m.lobby.out.pop(), Some(Cmd::Picks(vec![on(3)])), "the original has four");
    m.lobby.preset(0);
    assert_eq!(m.lobby.out.pop(), Some(Cmd::Picks(vec![on(0)])));
    let mut riding = guest_room();
    riding.riding = true;
    m.heard(Event::Room(riding));
    m.lobby.preset(1);
    assert!(m.lobby.out.is_empty(), "a ride under way keeps its presets");
    // back in the lobby, it hears of the change once
    m.back_from_online();
    m.heard(Event::Room(guest_room()));
    m.lobby.preset(1);
    assert_eq!(m.lobby.out.pop(), Some(Cmd::Picks(vec![on(1)])));
    m.lobby.preset(1);
    assert!(m.lobby.out.is_empty());
}

/// N18: a guest whose ride setup picked the biplane says so in its lobby pick, which reaches the
/// host's room; once the host starts, both machines put that seat in the plane, in its own bay,
/// before the first tick (`Online::start_planes` from `Online::picked_planes`, as the shell does), the
/// host's seat on its bike, and the two rides stay in step with it flying.
#[test]
fn a_lobby_pick_of_the_plane_starts_its_seat_in_it_on_every_machine() {
    use game_sim::world::Flat;
    use ps2_float::Vf;
    let file = match game_render::disc::Disc::open_configured() {
        Ok(mut disc) => {
            let name = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("machine.res")).expect("machine.res").name.clone();
            let bytes = disc.res(&name).expect("machine.res").read_suffix(game_sim::phy::BIPLANE_PHY).expect("biplane.phy");
            game_sim::phy::Plane::read_bytes(&bytes)
        }
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let (hn, gn) = (Arc::new(lan("Planey", 1)), Arc::new(lan("Pilot", 1)));
    let (mut h, mut hr) = fresh("Planey", "thq1");
    let (mut g, mut gr) = fresh("Pilot", "thq2");
    g.lobby.set_machine("biplane");
    h.lobby.set_machine("bike");
    assert_eq!((h.lobby.pick.machine.as_str(), g.lobby.pick.machine.as_str()), ("", "biplane"));
    host_and_join(&mut h, &mut hr, &hn, &mut g, &mut gr, &gn);
    gn.send(Cmd::Prepared(true));
    until((&mut h, &hn), (&mut g, &gn), "ready", |h, _, _, _| h.lobby.room.as_ref().is_some_and(|r| r.all_ready()));
    hn.send(Cmd::Start);
    let ((hroom, hme), (groom, gme)) = (started(&hn, "host"), started(&gn, "guest"));
    let flies = online::Online::picked_planes(&hroom);
    assert_eq!(flies, online::Online::picked_planes(&groom), "both machines read the same picks");
    assert_eq!(flies[..2], [false, true], "the guest's seat picked the plane: {:?}", hroom.seats);
    let sims = |room: &Room| -> Vec<game_sim::Sim> {
        (0..room.seats.len())
            .map(|_| {
                let mut s = game_sim::Sim::new(1);
                s.rng = room.seed;
                s
            })
            .collect()
    };
    let like = game_sim::flight::Flight::new(&file);
    let bays: Vec<vehicle::Spawn> =
        (0..hroom.seats.len()).map(|s| vehicle::Spawn { at: Vf::new(40.0 * s as f32, 3.0, -30.0, 1.0), facing: Vf::new(0.0, 0.0, 1.0, 0.0) }).collect();
    let start = |net: &Arc<Net>, room: &Room, me: &str| {
        let mut on = online::Online::lobby(net.clone(), room, me, sims(room)).unwrap();
        on.planes(Some(file), Some(&like));
        on.bays(bays.clone());
        on.start_planes(&online::Online::picked_planes(room)).unwrap();
        on
    };
    let (mut ho, mut go) = (start(&hn, &hroom, &hme), start(&gn, &groom, &gme));
    // shell seat 0 is this machine's own; the host's 1 is the guest's seat
    let flying = |on: &online::Online, d: usize| {
        let mut sim = game_sim::Sim::new(1);
        on.show(d, &mut sim, None);
        sim.plane.map(|f| f.spawn_origin())
    };
    assert_eq!((flying(&ho, 0), flying(&ho, 1)), (None, Some(bays[1].at)), "the host's start");
    assert_eq!((flying(&go, 0), flying(&go, 1)), (Some(bays[1].at), None), "the guest's start");
    let (mut hw, mut gw) = (Flat::default(), Flat::default());
    let gas = Report::from_buttons(Control::Cross.mask());
    let mut climb = Report::from_buttons(Control::Cross.mask());
    climb.set_axis(3, 0x20);
    for _ in 0..120 {
        ho.frame(gas, &mut hw, None).unwrap();
        go.frame(climb, &mut gw, None).unwrap();
        std::thread::sleep(Duration::from_millis(33));
    }
    let ((ha, hd), (ga, gd)) = (ho.settled(), go.settled());
    let both = ha.len().min(ga.len());
    println!("settled: host {} guest {} ticks; {}", ha.len(), ga.len(), ho.summary());
    assert!(both >= 60, "only {both} ticks settled on both in 4 s");
    assert_eq!(ha[..both], ga[..both], "the two machines' rides parted");
    assert_eq!((hd, gd), (0, 0), "desyncs");
    assert!(flying(&ho, 1).is_some() && flying(&go, 0).is_some(), "the guest's seat flies on");
    assert!(flying(&ho, 0).is_none(), "the host's seat rides its bike");
    let _ = go.leave();
}

/// N19: a guest whose ride setup picked the biplane drops in on a freeride opened online, and rides
/// in flying: the host's `Online` reads the machine off the guest's pick as it takes it on, and the
/// state it sends every machine has the guest's seat in the plane, standing in its own bay
/// (`rides::spawn_flying`), the host's seat on its bike; the two rides stay in step with it flying.
#[test]
fn a_drop_in_whose_setup_picked_the_plane_rides_in_flying() {
    use game_sim::world::Flat;
    use ps2_float::Vf;
    let file = match game_render::disc::Disc::open_configured() {
        Ok(mut disc) => {
            let name = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("machine.res")).expect("machine.res").name.clone();
            let bytes = disc.res(&name).expect("machine.res").read_suffix(game_sim::phy::BIPLANE_PHY).expect("biplane.phy");
            game_sim::phy::Plane::read_bytes(&bytes)
        }
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let (hn, gn) = (Arc::new(lan("Openy", 1)), Arc::new(lan("Flyer", 1)));
    let like = game_sim::flight::Flight::new(&file);
    let bays: Vec<vehicle::Spawn> = (0..4).map(|s| vehicle::Spawn { at: Vf::new(40.0 * s as f32, 3.0, -30.0, 1.0), facing: Vf::new(0.0, 0.0, 1.0, 0.0) }).collect();
    // the host's freeride of one, opened from its pause as the shell opens it
    let mut room = Room::new("Openy", "free06", 4);
    room.take(HOST, &[Pick::new("thq1", "race125p")]);
    (room.riding, room.drop_in) = (true, true);
    hn.send(Cmd::Host(room));
    let spawn = game_sim::Sim::new(1);
    let mut ho = online::Online::open(hn.clone(), spawn.clone(), &spawn).unwrap();
    ho.planes(Some(file), Some(&like));
    ho.bays(bays.clone());
    let (mut hw, mut gw) = (Flat::default(), Flat::default());
    let gas = Report::from_buttons(Control::Cross.mask());
    let mut climb = Report::from_buttons(Control::Cross.mask());
    climb.set_axis(3, 0x20);
    let end = Instant::now() + Duration::from_secs(30);
    let code = loop {
        assert!(Instant::now() < end, "no room code in 30 s");
        ho.frame(gas, &mut hw, None).unwrap();
        if let Some(code) = ho.kept().into_iter().find_map(|e| match e {
            Event::Room(r) if !r.short.is_empty() => Some(r.short),
            _ => None,
        }) {
            break code;
        }
        std::thread::sleep(Duration::from_millis(33));
    };
    // the guest's pick says the plane, as `Lobby::set_machine` puts the setup's machine in it
    let pick = Pick { machine: vehicle::Kind::Biplane.name().to_string(), ..Pick::new("thq2", "race125p") };
    gn.send(Cmd::Name("Flyer".into()));
    gn.send(Cmd::Join(code, vec![pick]));
    let mut go = loop {
        assert!(Instant::now() < end, "the guest never dropped in");
        ho.frame(gas, &mut hw, None).unwrap();
        if let Some((room, me)) = gn.poll().into_iter().find_map(|e| match e {
            Event::DropIn(room, me) => Some((room, me)),
            _ => None,
        }) {
            let mut on = online::Online::drop_in(gn.clone(), &room, &me, game_sim::Sim::new(1)).unwrap();
            on.planes(Some(file), Some(&like));
            on.bays(bays.clone());
            break on;
        }
        std::thread::sleep(Duration::from_millis(33));
    };
    let flying = |on: &online::Online, d: usize| {
        let mut sim = game_sim::Sim::new(1);
        on.show(d, &mut sim, None);
        sim.plane.map(|f| f.spawn_origin())
    };
    // shell seat 0 is this machine's own; the host's 1 is the guest's seat
    let end = Instant::now() + Duration::from_secs(30);
    while !(go.seated() && ho.riders() == 2) {
        assert!(Instant::now() < end, "the guest was not taken on in 30 s");
        ho.frame(gas, &mut hw, None).unwrap();
        go.frame(climb, &mut gw, None).unwrap();
        std::thread::sleep(Duration::from_millis(33));
    }
    assert_eq!((flying(&ho, 0), flying(&ho, 1)), (None, Some(bays[1].at)), "the host's ride as the guest comes in");
    assert_eq!((flying(&go, 0), flying(&go, 1)), (Some(bays[1].at), None), "the guest's ride as it comes in");
    for _ in 0..120 {
        ho.frame(gas, &mut hw, None).unwrap();
        go.frame(climb, &mut gw, None).unwrap();
        std::thread::sleep(Duration::from_millis(33));
    }
    let ((ha, hd), (ga, gd)) = (ho.settled(), go.settled());
    let both = ha.len().min(ga.len());
    println!("settled: host {} guest {} ticks; {}", ha.len(), ga.len(), ho.summary());
    assert!(both >= 60, "only {both} ticks settled on both");
    assert_eq!(ha[both - 30..both], ga[both - 30..both], "the two machines' rides parted");
    assert_eq!((hd, gd), (0, 0), "desyncs");
    assert!(flying(&ho, 1).is_some() && flying(&go, 0).is_some(), "the guest's seat flies on");
    assert!(flying(&ho, 0).is_none(), "the host's seat rides its bike");
    let _ = go.leave();
}

/// P134: each player's own modifiers ride in their pick, the host's room gathers every seat's, a
/// row a seat, and both lobbies show each seat's; the host's All Players reaches the guest's seat
/// too, and a guest's page cannot touch the world's.
#[test]
fn the_lobby_gathers_every_seats_own_modifiers() {
    use game_sim::modifiers::{Knob, Mods};
    let (hn, gn) = (lan("Hosty", 1), lan("Guesty", 1));
    let (mut h, mut hr) = fresh("Hosty", "thq1");
    let (mut g, mut gr) = fresh("Guesty", "thq2");
    // the host's gravity is the world's; its bike speed its own
    h.settings.modifiers.set(0, Knob::Gravity, 80);
    host_and_join(&mut h, &mut hr, &hn, &mut g, &mut gr, &gn);
    // the guest's own faster bike, and a gravity of its own the room never takes
    g.settings.modifiers.set(0, Knob::BikeSpeed, 150);
    g.settings.modifiers.set(0, Knob::Gravity, 300);
    let room_mods = |m: &Menu| Mods::parse(&m.lobby.room.as_ref().unwrap().modifiers).unwrap();
    let both = |h: &mut Menu, g: &mut Menu, what: &str, done: &dyn Fn(&Mods) -> bool| {
        let end = Instant::now() + Duration::from_secs(30);
        while !(done(&room_mods(h)) && done(&room_mods(g))) {
            assert!(Instant::now() < end, "{what}: not in 30 s; host {:?} guest {:?}", room_mods(h), room_mods(g));
            // as the shell's frame: the pick's modifiers, then its preset's line sends a changed pick
            for m in [&mut *h, &mut *g] {
                m.lobby_mods();
                m.lobby.preset(0);
            }
            pump(h, &hn);
            pump(g, &gn);
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    both(&mut h, &mut g, "the guest's own", &|m| m.get(1, Knob::BikeSpeed) == 150);
    let m = room_mods(&g);
    assert_eq!((m.get(0, Knob::BikeSpeed), m.get(0, Knob::Gravity)), (100, 80), "the host's own and the world's: {m:?}");
    let room = g.lobby.room.clone().unwrap();
    assert_eq!(menu::Lobby::seat_mods(&room, 1), "  Bike top speed 150%");
    assert_eq!(menu::Lobby::seat_mods(&room, 0), "");

    // The host's page from its lobby, on All Players: grip for every seat, the guest's too.
    let at = h.lobby.items().iter().position(|i| *i == Item::Modifiers).unwrap();
    for _ in 0..at {
        tap(&mut h, &mut hr, Control::Down);
    }
    tap(&mut h, &mut hr, Control::Cross);
    assert_eq!(h.screen(), Some(Screen::Modifiers));
    tap(&mut h, &mut hr, Control::Left);
    assert_eq!(h.mods_whose(), "All");
    while h.page()[h.cursor(Screen::Modifiers)].label != Knob::Grip.label() {
        tap(&mut h, &mut hr, Control::Down);
    }
    tap(&mut h, &mut hr, Control::Right);
    tap(&mut h, &mut hr, Control::Circle);
    both(&mut h, &mut g, "the host's All", &|m| m.get(0, Knob::Grip) == 110 && m.get(1, Knob::Grip) == 110);
    assert_eq!(room_mods(&h).get(1, Knob::BikeSpeed), 150, "the guest's own speed stays");

    // The guest's page from its lobby is its own seat's: the world's are not its to set.
    let at = g.lobby.items().iter().position(|i| *i == Item::Modifiers).unwrap();
    for _ in 0..at {
        tap(&mut g, &mut gr, Control::Down);
    }
    tap(&mut g, &mut gr, Control::Cross);
    assert_eq!(g.screen(), Some(Screen::Modifiers));
    let gravity = g.page().into_iter().find(|r| r.key == menu::Key::Knob(Knob::Gravity)).unwrap();
    assert!(gravity.off.is_some(), "{gravity:?}");
    assert!(g.page()[0].off.is_some(), "the guest has no All Players");
}

/// N16: an online ride draws every peer's rider in their own look. Two real lobbies on the LAN, each
/// player in a look of their own (P86's pick line) on a bike model of their own; the host's Start
/// gives each machine's ride a shell seat for the other's rider, dressed from the room's pick as the
/// shell's load dresses it ([`peer_looks::lobby_others`]), and that seat is the other's in the
/// session, sat in and drawn, with nothing left to dress again once it rides.
#[test]
fn a_lobby_ride_draws_each_peer_in_their_own_look() {
    use game_sim::world::Flat;
    let (hn, gn) = (Arc::new(lan("Hosty", 1)), Arc::new(lan("Guesty", 1)));
    let (mut h, mut hr) = fresh("Hosty", "thq1");
    let (mut g, mut gr) = fresh("Guesty", "thq2");
    let (host_look, guest_look) = ("thq3, , , , , 72, Kaito", ", , , , , 9, Rina");
    (h.lobby.pick.look, h.lobby.pick.model) = (host_look.into(), "125sx".into());
    (g.lobby.pick.look, g.lobby.pick.model) = (guest_look.into(), "250sx".into());
    host_and_join(&mut h, &mut hr, &hn, &mut g, &mut gr, &gn);
    gn.send(Cmd::Prepared(true));
    until((&mut h, &hn), (&mut g, &gn), "ready", |h, _, _, _| h.lobby.room.as_ref().is_some_and(|r| r.all_ready()));
    hn.send(Cmd::Start);
    let ((hroom, hme), (groom, gme)) = (started(&hn, "host"), started(&gn, "guest"));
    let sims = |room: &Room| -> Vec<game_sim::Sim> { (0..room.seats.len()).map(|_| game_sim::Sim::new(1)).collect() };
    let own = roster::Pick::default();
    // What each machine's shell loads its other seats in: one seat for the other player, then the
    // room's open seats, which draw as this machine's own and are not drawn while open.
    let (ho_picks, go_picks) = (peer_looks::lobby_others(&hroom, &hme, &own).unwrap(), peer_looks::lobby_others(&groom, &gme, &own).unwrap());
    assert_eq!((ho_picks.len(), go_picks.len()), (hroom.seats.len() - 1, groom.seats.len() - 1), "a shell seat for every room seat");
    let (on_host, on_guest) = (&ho_picks[0], &go_picks[0]);
    assert_eq!((on_host.rider.as_str(), on_host.bike.as_str()), ("thq2", "250sx"), "the host draws the guest's rider and bike");
    assert_eq!(on_host.look, roster::Look::parse(guest_look), "the host draws the guest's look");
    assert_eq!((on_host.look.name.as_str(), on_host.look.number.as_str()), ("Rina", "9"));
    assert_eq!((on_guest.rider.as_str(), on_guest.bike.as_str()), ("thq1", "125sx"), "the guest draws the host's rider and bike");
    assert_eq!(on_guest.look, roster::Look::parse(host_look), "the guest draws the host's look");
    assert_eq!((on_guest.look.outfit.as_str(), on_guest.look.name.as_str(), on_guest.look.number.as_str()), ("thq3", "Kaito", "72"));
    let mut ho = online::Online::lobby(hn.clone(), &hroom, &hme, sims(&hroom)).unwrap();
    let mut go = online::Online::lobby(gn.clone(), &groom, &gme, sims(&groom)).unwrap();
    let (mut hw, mut gw) = (Flat::default(), Flat::default());
    let gas = Report::from_buttons(Control::Cross.mask());
    for _ in 0..60 {
        ho.frame(gas, &mut hw, None).unwrap();
        go.frame(gas, &mut gw, None).unwrap();
        std::thread::sleep(Duration::from_millis(33));
    }
    for (who, on, room, me, other) in [("host", &ho, &hroom, &hme, &gme), ("guest", &go, &groom, &gme, &hme)] {
        let mut sim = game_sim::Sim::new(1);
        assert!(on.show(1, &mut sim, None), "{who}: shell seat 1 is sat in and drawn");
        assert!(!on.show(2, &mut sim, None), "{who}: an open seat is not drawn");
        assert_eq!(on.seat_pick(0).map(|s| &s.peer), Some(me), "{who}: shell seat 0 is its own");
        assert_eq!(on.seat_pick(1).map(|s| &s.peer), Some(other), "{who}: shell seat 1 is the other's");
        let sat: Vec<_> = (0..room.seats.len()).map(|d| on.seat_pick(d)).collect();
        assert_eq!(peer_looks::Dressed::lobby(room, me).changed(&sat), Vec::<usize>::new(), "{who}: the load dressed every seat");
    }
    assert_eq!(ho.seat_pick(1).unwrap().pick.look, guest_look);
    assert_eq!(go.seat_pick(1).unwrap().pick.look, host_look);
    let _ = go.leave();
}

/// N20, end to end over two real lobbies on the LAN: in a running lobby ride a second player on the
/// guest's machine takes a seat — the room's first open one, the host seating them at a tick both
/// machines take — and rides it on their own pad and preset as the guest's shell seat 1, right
/// after its first player's (a view of their own); then they leave it again from their pause and the
/// two first players ride on, the two machines' settled ticks agreeing all through.
#[test]
fn a_second_player_on_an_online_machine_sits_down_and_leaves_again() {
    use game_sim::world::Flat;
    let (hn, gn) = (Arc::new(lan("Hosty", 1)), Arc::new(lan("Guesty", 1)));
    let (mut h, mut hr) = fresh("Hosty", "thq1");
    let (mut g, mut gr) = fresh("Guesty", "thq2");
    host_and_join(&mut h, &mut hr, &hn, &mut g, &mut gr, &gn);
    gn.send(Cmd::Prepared(true));
    until((&mut h, &hn), (&mut g, &gn), "ready", |h, _, _, _| h.lobby.room.as_ref().is_some_and(|r| r.all_ready()));
    hn.send(Cmd::Start);
    let ((hroom, hme), (groom, gme)) = (started(&hn, "host"), started(&gn, "guest"));
    let sims = |room: &Room| -> Vec<game_sim::Sim> { (0..room.seats.len()).map(|_| game_sim::Sim::new(1)).collect() };
    let mut ho = online::Online::lobby(hn.clone(), &hroom, &hme, sims(&hroom)).unwrap();
    let mut go = online::Online::lobby(gn.clone(), &groom, &gme, sims(&groom)).unwrap();
    assert!(hroom.seats.len() > 2, "an open seat to sit in: {}", hroom.seats.len());
    let (mut hw, mut gw) = (Flat::default(), Flat::default());
    let gas = Report::from_buttons(Control::Cross.mask());
    let mut lean = Report::from_buttons(Control::Cross.mask());
    lean.set_axis(2, 0x10);
    let mut both = |ho: &mut online::Online, go: &mut online::Online, rounds: usize, done: &dyn Fn(&online::Online) -> bool| {
        for _ in 0..rounds {
            ho.frame(gas, &mut hw, None).unwrap();
            go.frame_seats(&[gas, lean], &mut gw, None).unwrap();
            std::thread::sleep(Duration::from_millis(33));
            if done(go) {
                return true;
            }
        }
        false
    };
    both(&mut ho, &mut go, 45, &|_| false);
    assert_eq!(go.my_seats(), [1]);
    go.sit(2).unwrap();
    assert!(both(&mut ho, &mut go, 300, &|go| go.my_seats().len() == 2), "the guest's second player never sat");
    // the host has the seat too by now (it made the restart)
    assert_eq!(ho.names()[2].as_deref(), Some("Guesty"), "the host sees the guest's second player, by the guest's name");
    assert_eq!(go.my_seats(), [1, 2]);
    assert_eq!(go.drawn()[..2], [1, 2], "the guest's players lead its shell seats");
    assert!(ho.can_sit() && go.can_sit());
    both(&mut ho, &mut go, 60, &|_| false);
    let mut sim = game_sim::Sim::new(1);
    assert!(go.show(1, &mut sim, None), "the second player's seat is sat in and drawn");
    assert_eq!(net::rides::preset(&sim), Some(2), "on their own controller preset");
    assert!(sim.ticks > 30, "and riding: {} ticks", sim.ticks);
    // their pause's Leave Session: the guest's local player 2
    go.leave_session(1).unwrap();
    assert!(both(&mut ho, &mut go, 300, &|go| go.my_seats().len() == 1), "the guest's second player never left");
    both(&mut ho, &mut go, 60, &|_| false);
    assert_eq!(ho.names()[2], None, "the seat is empty on the host");
    assert!(go.seated() && !go.leaving(), "the guest's first player rides on");
    let ((ha, hd), (ga, gd)) = (ho.settled(), go.settled());
    let n = ha.len().min(ga.len());
    assert!(n > 100, "{n} settled ticks");
    assert_eq!((hd, gd), (0, 0), "no desync");
    assert_eq!(ha[..n], ga[..n], "the two machines agree on every settled tick");
    let _ = go.leave();
}

/// N16, mid-ride: a freeride opened online (N10) takes a peer on, and each machine's session grows a
/// seat the shell's ride did not load with. That seat says who sits in it and in what look, so the
/// shell dresses it as it comes ([`peer_looks::Dressed::changed`]) — the host the joiner in the
/// joiner's look, the joiner the host in the host's — and once dressed it stays dressed.
#[test]
fn a_peer_who_drops_in_is_drawn_in_their_own_look() {
    use game_sim::world::Flat;
    let (hn, gn) = (Arc::new(lan("Openy", 1)), Arc::new(lan("Dropper", 1)));
    let (host_look, guest_look) = (", , , , , 5, Openy", "thq2, , , , , 31, Drop");
    let mut room = Room::new("Openy", "free06", 4);
    room.take(HOST, &[Pick { look: host_look.into(), model: "125sx".into(), ..Pick::new("thq1", "race125p") }]);
    (room.riding, room.drop_in) = (true, true);
    hn.send(Cmd::Host(room));
    let spawn = game_sim::Sim::new(1);
    let mut ho = online::Online::open(hn.clone(), spawn.clone(), &spawn).unwrap();
    let (mut hw, mut gw) = (Flat::default(), Flat::default());
    let gas = Report::from_buttons(Control::Cross.mask());
    let end = Instant::now() + Duration::from_secs(30);
    let code = loop {
        assert!(Instant::now() < end, "no room code in 30 s");
        ho.frame(gas, &mut hw, None).unwrap();
        if let Some(code) = ho.kept().into_iter().find_map(|e| match e {
            Event::Room(r) if !r.short.is_empty() => Some(r.short),
            _ => None,
        }) {
            break code;
        }
        std::thread::sleep(Duration::from_millis(33));
    };
    // the ride as the host's shell loaded it: its own seat alone, nothing to dress
    let mut host_dress = peer_looks::Dressed::default();
    assert_eq!(ho.seat_of().0, 1);
    gn.send(Cmd::Name("Dropper".into()));
    gn.send(Cmd::Join(code, vec![Pick { look: guest_look.into(), model: "250sx".into(), ..Pick::new("thq2", "race125p") }]));
    let mut go = loop {
        assert!(Instant::now() < end, "the guest never dropped in");
        ho.frame(gas, &mut hw, None).unwrap();
        if let Some((room, me)) = gn.poll().into_iter().find_map(|e| match e {
            Event::DropIn(room, me) => Some((room, me)),
            _ => None,
        }) {
            break online::Online::drop_in(gn.clone(), &room, &me, game_sim::Sim::new(1)).unwrap();
        }
        std::thread::sleep(Duration::from_millis(33));
    };
    let mut guest_dress = peer_looks::Dressed::default();
    let end = Instant::now() + Duration::from_secs(30);
    while !(go.seated() && ho.riders() == 2 && go.seat_of().0 == 2) {
        assert!(Instant::now() < end, "the guest was not taken on in 30 s");
        ho.frame(gas, &mut hw, None).unwrap();
        go.frame(gas, &mut gw, None).unwrap();
        std::thread::sleep(Duration::from_millis(33));
    }
    for _ in 0..30 {
        ho.frame(gas, &mut hw, None).unwrap();
        go.frame(gas, &mut gw, None).unwrap();
        std::thread::sleep(Duration::from_millis(33));
    }
    for (who, on, dress, look, rider, model) in [("host", &ho, &mut host_dress, guest_look, "thq2", "250sx"), ("guest", &go, &mut guest_dress, host_look, "thq1", "125sx")] {
        assert_eq!(on.seat_of().0, 2, "{who}: the session grew a seat");
        let mut sim = game_sim::Sim::new(1);
        assert!(on.show(1, &mut sim, None), "{who}: the other's seat is sat in and drawn");
        let sat: Vec<_> = (0..2).map(|d| on.seat_pick(d)).collect();
        assert_eq!(dress.changed(&sat), vec![1], "{who}: the other's seat is the one to dress");
        let seat = sat[1].expect("sat in");
        let pick = peer_looks::garage_pick(&seat.pick, &roster::Pick::default());
        assert_eq!((pick.rider.as_str(), pick.bike.as_str(), &pick.look), (rider, model, &roster::Look::parse(look)), "{who}: dressed in the other's look");
        dress.dress(1, seat);
        assert_eq!(dress.changed(&sat), Vec::<usize>::new(), "{who}: dressed once");
    }
    assert_eq!(ho.seat_pick(1).unwrap().pick.look, guest_look);
    assert_eq!(go.seat_pick(1).unwrap().pick.look, host_look);
    let _ = go.leave();
}

/// The teleport's player pills name each seat by the room's name for its peer, not the peer's id: a
/// joiner's id is a long hex string, which once filled the pill.
#[test]
fn a_ride_names_its_players_not_their_peer_ids() {
    let id = "cb20b207d9bab2a665e005aa5ab684935:8f085ea4c22b39249c7c241";
    let mut room = openmx_net::Room { name: "Hosting Hank".into(), ..Default::default() };
    room.names.insert(id.into(), "Rider With A Long Name".into());
    assert_eq!(online::name_of(Some(&room), openmx_net::HOST), "Hosting Hank");
    assert_eq!(online::name_of(Some(&room), id), "Rider With A");
    assert_eq!(online::name_of(Some(&room), "someone_else"), "", "unknown: the display's P2");
    assert_eq!(online::name_of(None, id), "");
}
