//! N3: the front end's online screens. The menu ([`menu::Menu`]) is driven by pad reports alone, as
//! `tests/menu.rs` drives it, and by the network's events: first with events written by hand, then
//! with two real `openmx_net` networks on this machine's LAN (mDNS; no relay, no board, no code
//! server — nothing leaves the machine), where a host's menu opens a lobby, a guest's menu types the
//! room code on the on-screen keyboard and joins, says ready, and the host's Start gives both menus the
//! same ride and seed; a guest leaving mid-ride puts the host back in its lobby; a guest whose disc
//! holds another race125p is refused and told why. Last, what the shell hashes for the handshake,
//! against the disc when there is one.

#[path = "../src/menu.rs"]
#[allow(dead_code)]
mod menu;

#[path = "../src/roster.rs"]
#[allow(dead_code)]
mod roster;

#[path = "../src/lobby.rs"]
#[allow(dead_code)]
mod lobby;

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
    for report in [Report::from_buttons(control.mask()), Report::idle()] {
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
    assert_eq!(m.lobby.items(), vec![Item::Bike, Item::Chat, Item::Ready, Item::Leave]);
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
    // The guest leaves the ride (its pause's Main Menu, `Online::leave`): the host is back in its lobby.
    gn.send(Cmd::Leave);
    let mut dropped = false;
    until((&mut h, &hn), (&mut g, &gn), "the drop", |h, _, _, _| {
        dropped |= h.note.is_some();
        dropped && h.lobby.room.as_ref().is_some_and(|r| !r.riding && r.taken() == 1)
    });
    h.back_from_online();
    assert_eq!(h.screen(), Some(Screen::Lobby));
    println!("host's note: {:?}", h.note);
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
fn the_lobby_s_ride_goes_over_the_lobby_s_connection_in_step_and_a_leaver_ends_it() {
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
    // The guest leaves from the pause: the host's ride ends, and what ended it is kept for its menu.
    assert!(!go.leave(), "a guest is out of the lobby once it leaves");
    let end = Instant::now() + Duration::from_secs(15);
    let why = loop {
        assert!(Instant::now() < end, "the host rode on after the guest left");
        if let Err(why) = ho.frame(gas, &mut hw, None) {
            break why;
        }
        std::thread::sleep(Duration::from_millis(33));
    };
    println!("host's ride ended: {why}");
    for e in ho.kept() {
        h.heard(e);
    }
    until((&mut h, &hn), (&mut g, &gn), "the host's lobby back", |h, _, _, _| h.lobby.room.as_ref().is_some_and(|r| !r.riding && r.taken() == 1));
    h.back_from_online();
    assert_eq!(h.screen(), Some(Screen::Lobby));
}

/// N10, the menu's side: a local freeride's pause offers "Open to Online" above Quit, which asks the
/// shell to open the ride, and then shows the room code; a ride that cannot be opened has no such
/// row and Quit where it was. A guest's join that lands in a ride under way is the lobby's ride with
/// [`menu::Lobby::drop_in`] set, and Find says a listing is a ride to drop in on.
#[test]
fn a_freerides_pause_opens_it_online() {
    use menu::Opening;
    let (mut m, mut r) = fresh("Hosty", "thq1");
    m.pause();
    assert_eq!(m.pause_rows().last().map(String::as_str), Some("Quit"));
    let quit = m.pause_rows().len() - 1;
    m.opening = Opening::Can;
    let rows = m.pause_rows();
    assert_eq!(rows[rows.len() - 2..], ["Open to Online", "Quit"]);
    // six rows down from Resume: Open to Online, and the pause closes over a ride that goes on
    let mut acts = Vec::new();
    for _ in 0..quit {
        acts.extend(tap(&mut m, &mut r, Control::Down));
    }
    acts.extend(tap(&mut m, &mut r, Control::Cross));
    assert_eq!(acts.last(), Some(&Act::Stay));
    assert!(acts.contains(&Act::OpenOnline), "{acts:?}");
    assert!(m.riding());
    // opened: the row is the code once the network says it, and pressing it does nothing
    m.opening = Opening::Open;
    m.pause();
    assert_eq!(m.pause_rows()[quit], "Online: opening...");
    m.heard(Event::Short("QKX4".into()));
    assert_eq!(m.pause_rows()[quit], "Online: QKX4");
    taps(&mut m, &mut r, &vec![Control::Down; quit]);
    let acts = tap(&mut m, &mut r, Control::Cross);
    assert!(acts.iter().all(|a| *a == Act::Stay) && m.screen() == Some(Screen::Pause), "{acts:?}");
    // the last row is still Quit
    let acts = taps_acts(&mut m, &mut r, &[Control::Down, Control::Cross]);
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

fn taps_acts(menu: &mut Menu, reader: &mut Reader, controls: &[Control]) -> Vec<Act> {
    controls.iter().flat_map(|&c| tap(menu, reader, c)).collect()
}
