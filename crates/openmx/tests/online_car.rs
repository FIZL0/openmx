//! S249 — a car online: the switch into one is sent over the wire, and every machine drives it.
//!
//! Two shells on one machine, host and guest, ride free09 online. The host's first player picks the
//! monster truck (`--online-script`, the M key's and the pause's pick in the pad's switch byte),
//! holds Cross, lets it go, and picks the bike back. Both machines must see the host's seat switch
//! into the truck and out of it, the truck must have gone somewhere, and the two rides must agree
//! (no desync, the same state hash) while the truck is out.
//!
//! Under it, the seat's lane itself (`vehicle::switch_lane_among`, `vehicle::drive_lane`): a seat
//! switched into a car and driven online drives as the same switch and pads drive offline's garage,
//! bit for bit, and a state sent in bytes (a resync, a join) drives on as the one it was taken from.
//!
//! T2 beyond the host's truck: the guest's own switch into the buggy, both seats in cars at once
//! (the baja and the truck), each over two shells; a resync and a join while a car is out, through
//! the session itself ([`Netplay`] over an in-memory wire, as tests/netplay.rs's harness, on
//! [`Rides`] with every seat's cars); and the online pause's switch row offering the cars.
//!
//! Needs the disc's `machine.res` cars and skips without the disc, saying so.

#[allow(unused)]
#[path = "../src/thread.rs"]
mod thread;
// the pause's switch row and what it needs (T2), as tests/machines_everywhere.rs mounts them
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
#[allow(dead_code)]
#[path = "../src/peer.rs"]
mod peer;

use game_render::Disc;
use game_sim::input::{PADS, Report};
use game_sim::world::Flat;
use game_sim::Sim;
use net::rides::{self, Rides};
use net::{Lineup, Netplay, Timing};
use ps2_float::F;
use vehicle::{CarRide, Garage, Kind, Machine};
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

fn line_with<'a>(said: &'a str, part: &str) -> &'a str {
    said.lines().find(|l| l.contains(part)).unwrap_or_else(|| panic!("no line with `{part}`:\n{said}"))
}

/// Two shells riding `track` online for `frames` ticks, host's and guest's on `scripts`: each one's
/// output.
fn ride_online(track: &str, frames: u32, scripts: [Option<&str>; 2]) -> [String; 2] {
    // Each shell binds a port of its own (port 0) and keeps it: a port picked free here and bound
    // by the shell after its load was a window another slot's test could take it in (K29). The
    // host says its port; the guest is started on it; the host takes the guest's from its first
    // datagram (`net::udp`'s port-0 peer).
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let frames = frames.to_string();
    let ride = |me: &str, peer: &str, script: Option<&str>| {
        let mut shell = Command::new(env!("CARGO_BIN_EXE_openmx"));
        shell
            .current_dir(&root)
            .args(["--headless", "--track", track, "--frames", &frames, "--seats", "host,g"])
            .args(["--online", me, "--peer", peer])
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
    let (mut host, host_said, heard) = ride("host@127.0.0.1:0", "g@127.0.0.1:0", scripts[0]);
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
    let (guest, guest_said, _) = ride("g@127.0.0.1:0", &format!("host@{at}"), scripts[1]);
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
fn two_online_shells_see_the_host_switch_into_the_monster_truck_and_back() {
    if disc().is_none() {
        return;
    }
    let said = ride_online("free09", 420, [Some("40:monster,41:gas,200:lift,330:bike"), None]);
    for (who, s) in ["host", "guest"].iter().zip(&said) {
        let switched: Vec<&str> = s.lines().filter(|l| l.starts_with("vehicle: ") && !l.contains("every machine agrees")).collect();
        // The load's bike first, then the session's two switches.
        assert_eq!(switched, ["vehicle: bike", "vehicle: monster", "vehicle: bike"], "{who}:\n{s}");
        assert!(line_with(s, "desyncs").contains(" 0 desyncs"), "{who}:\n{s}");
        // The truck left says how far it went from where the session's switch stood it.
        let left = line_with(s, "car: monster ");
        let gone: f32 = left["car: monster ".len()..].split(' ').next().unwrap().parse().expect("a distance");
        assert!(gone > 20.0, "{who}: the truck stayed where it was taken: {left}");
    }
    // And both drove it to the same place.
    assert_eq!(line_with(&said[0], "car: monster "), line_with(&said[1], "car: monster "));
    let state = |s: &str| line_with(s, "state before tick 300").rsplit(' ').next().unwrap().to_string();
    assert_eq!(state(&said[0]), state(&said[1]), "the two machines' rides parted with the truck out");
}

/// T2: what two online shells' runs must both say — the load's bike and then `switched`, every
/// seat's, in any order (K27: two seats' picks a moment apart land on ticks the two shells' wall
/// clocks choose, and a machine draws the other's a few frames late; each seat's own come in its
/// order, which `left`'s distances hold); 0 desyncs; each car of `left` left gone more than 20
/// ft from where it was taken, at the same place on both; the same state before tick 300; and the
/// cars the pause offers (the garage's, which the pause's `menu.cars` is set from).
fn both_agree(said: &[String; 2], switched: &[&str], left: &[&str]) {
    for (who, s) in ["host", "guest"].iter().zip(said) {
        let saw: Vec<&str> = s
            .lines()
            .filter(|l| l.starts_with("vehicle: ") && !l.contains("every machine agrees"))
            .map(|l| &l["vehicle: ".len()..])
            .collect();
        assert_eq!(saw[0], "bike", "{who}:\n{s}");
        let (mut saw, mut want) = (saw[1..].to_vec(), switched.to_vec());
        saw.sort_unstable();
        want.sort_unstable();
        assert_eq!(saw, want, "{who}:\n{s}");
        assert!(line_with(s, "desyncs").contains(" 0 desyncs"), "{who}:\n{s}");
        let offered = line_with(s, "(switch from the pause)");
        assert!(["buggy", "monster", "baja"].iter().all(|c| offered.contains(c)), "{who}: {offered}");
        for car in left {
            let at = format!("car: {car} ");
            let line = line_with(s, &at);
            let gone: f32 = line[at.len()..].split(' ').next().unwrap().parse().expect("a distance");
            assert!(gone > 20.0, "{who}: the {car} stayed where it was taken: {line}");
        }
    }
    for car in left {
        let at = format!("car: {car} ");
        assert_eq!(line_with(&said[0], &at), line_with(&said[1], &at), "the two machines drove the {car} apart");
    }
    let state = |s: &str| line_with(s, "state before tick 300").rsplit(' ').next().unwrap().to_string();
    assert_eq!(state(&said[0]), state(&said[1]), "the two machines' rides parted with a car out");
}

/// T2: the guest's own pick of the buggy goes over the wire as the host's of the truck does: both
/// machines switch the guest's seat in and out at one tick, and drive it alike.
#[test]
fn two_online_shells_see_the_guest_switch_into_the_buggy_and_back() {
    if disc().is_none() {
        return;
    }
    let said = ride_online("free09", 420, [None, Some("40:buggy,41:gas,200:lift,330:bike")]);
    both_agree(&said, &["buggy", "bike"], &["buggy"]);
}

/// T2: both seats in cars at once, each its own seat's copy — the host's in the baja, the guest's
/// in the truck, out together from tick 60 to 330.
#[test]
fn two_online_shells_drive_two_cars_at_once() {
    if disc().is_none() {
        return;
    }
    let said = ride_online("free09", 450, [Some("40:baja,41:gas,240:lift,330:bike"), Some("60:monster,61:gas,260:lift,370:bike")]);
    both_agree(&said, &["baja", "monster", "bike", "bike"], &["baja", "monster"]);
}

/// Seat 0's pads for a tick: Cross held (the shipped preset's gas) or nothing.
fn pads(gas: bool) -> [Report; PADS] {
    let mut r = [Report::missing(); PADS];
    r[0] = Report::from_buttons(if gas { game_sim::input::Control::Cross.mask() } else { 0 });
    r
}

#[test]
fn a_seats_car_lane_drives_as_the_offline_garage_and_through_its_bytes() {
    let Some(mut disc) = disc() else { return };
    let machines = disc.res("machine.res").expect("the machines' archive");
    let cars: Vec<CarRide> = [Machine::Buggy, Machine::Monster, Machine::Baja]
        .into_iter()
        .map(|m| CarRide::new(m, 0, None, &mut |n| machines.read_suffix(n).ok()).unwrap())
        .collect();
    let dt = F(1.0 / 30.0);
    for to in [Kind::Buggy, Kind::Monster, Kind::Baja] {
        let (mut online, mut offline) = (Sim::new(0x249), Sim::new(0x249));
        let (mut floor, mut floor2) = (Flat::default(), Flat::default());
        let mut garage = Garage::new(Kind::Bike, None).unwrap();
        garage.cars = cars.clone();
        // A second on the bike first, both the same.
        for _ in 0..30 {
            let c = online.poll(dt, &pads(false));
            online.step(dt, &c, &mut floor);
            let c = offline.poll(dt, &pads(false));
            garage.pass(&mut offline, dt, &c, 0, &[], &mut floor2);
        }
        assert_eq!(game_sim::hash::of(&online), game_sim::hash::of(&offline));

        assert_eq!(vehicle::switch_lane_among(to, None, &cars, &mut online, &mut floor), Ok(to));
        assert_eq!(garage.switch(to, &mut offline, &mut floor2), Ok(to));
        assert_eq!(vehicle::lane_up(&online), to, "the lane holds the car");
        assert!(online.car.as_ref().unwrap().struck.is_some(), "{to:?}: the car's collision object came with it");
        // And the switch byte carries the pick.
        assert_eq!(vehicle::Wanted::of(vehicle::Wanted { to: Some(to), paused: true, ..Default::default() }.byte()).to, Some(to));

        // Three seconds on the gas, then one off it; the bytes taken half way.
        let mut sent: Option<Sim> = None;
        for tick in 0..120 {
            let gas = tick < 90;
            online.poll(dt, &pads(gas));
            assert!(vehicle::drive_lane(&mut online, &cars, dt, &mut floor), "{to:?} driven");
            let c2 = offline.poll(dt, &pads(gas));
            garage.pass(&mut offline, dt, &c2, 0, &[], &mut floor2);
            let lane = &online.car.as_ref().unwrap().machine;
            let car = garage.car_up().and_then(|c| c.driven.as_ref()).unwrap();
            assert_eq!(format!("{lane:?}"), format!("{car:?}"), "{to:?} at tick {tick}: the lane's car is not the garage's");
            if let Some(s) = &mut sent {
                s.poll(dt, &pads(gas));
                vehicle::drive_lane(s, &cars, dt, &mut floor2);
                assert_eq!(game_sim::hash::of(s), game_sim::hash::of(&online), "{to:?} at tick {tick}: the state sent drove apart");
            }
            if tick == 45 {
                let bytes = game_sim::snapshot::encode(&online);
                let mut s = Sim::new(0x249);
                game_sim::snapshot::decode(&mut s, &bytes).unwrap();
                // The probe the codec leaves out, as the session puts it back (`Rides::probe`).
                let struck = s.car.as_mut().and_then(|l| l.struck.as_mut()).unwrap();
                assert!(struck.rays.segments.is_empty());
                struck.rays = std::sync::Arc::new(cars.iter().find(|c| c.machine == to.car().unwrap()).unwrap().rays.clone().unwrap());
                assert_eq!(game_sim::hash::of(&s), game_sim::hash::of(&online), "{to:?}: the bytes are not the state");
                sent = Some(s);
            }
        }
        let gone = {
            let at = online.car.as_ref().unwrap().machine.matrix.0[3];
            at.x().hypot(at.z())
        };
        assert!(gone > 30.0, "{to:?} went {gone} ft");

        // Back onto the bike: the lane empties, the rider stood by the car.
        let at = online.car.as_ref().unwrap().machine.matrix.0[3];
        assert_eq!(vehicle::switch_lane_among(Kind::Bike, None, &cars, &mut online, &mut floor), Ok(Kind::Bike));
        assert!(online.car.is_none());
        let p = online.rider.ride.position();
        assert!((p.x() - at.x()).hypot(p.z() - at.z()) < 1.0, "{to:?}: the bike stood at {p:?}, the car at {at:?}");
        // And a car pick with no copy of the car stays on the bike.
        assert!(vehicle::switch_lane_among(to, None, &[], &mut online, &mut floor).is_err());
        assert_eq!(vehicle::lane_up(&online), Kind::Bike);
    }
}

// ---------------------------------------------------------------------------------------------------
// T2: a resync and a join with a car out, through the session itself
// ---------------------------------------------------------------------------------------------------

/// A round of the harness: one tick of the ride's 30 Hz clock, as tests/netplay.rs's.
const ROUND: std::time::Duration = std::time::Duration::from_nanos(1_001_000_000_000 / 30_000);

/// The tick g's machine alone knocks seat 1's bike sideways (the next checksum after it is 330).
const GLITCH: u64 = 320;

/// One machine's ride, as the shell steps it online: a [`Sim`] a seat over a flat floor, every
/// seat's cars as [`vehicle::Lot::online_cars`] gives them. `glitch` is [`GLITCH`]'s knock.
struct Machine2 {
    sims: Vec<Sim>,
    world: Flat,
    cars: Vec<Vec<CarRide>>,
    glitch: bool,
    /// F23: seat, tick and speed its bike is set going at after that tick's step.
    shove: Option<(usize, u64, ps2_float::Vf)>,
}

impl Machine2 {
    fn new(seats: usize, cars: &[CarRide]) -> Machine2 {
        let first = Sim::new(1);
        let sims = (0..seats)
            .map(|s| {
                let mut sim = first.clone();
                sim.rng = 1 + s as u32;
                sim
            })
            .collect();
        Machine2 { sims, world: Flat::default(), cars: vec![cars.to_vec(); net::SEATS], glitch: false, shove: None }
    }

    fn frame(&mut self, np: &mut Netplay<rides::State>, local: &[Report]) {
        let (glitch, shove) = (self.glitch, self.shove);
        let mut after = |seat: usize, _: F, sim: &mut Sim, _: &game_sim::ride::Controls| {
            if glitch && seat == 1 && sim.ticks == GLITCH {
                sim.rider.ride.chassis.vel = ps2_float::Vf::new(0.5, 0.0, 0.0, 0.0);
            }
            if let Some((s, t, vel)) = shove
                && s == seat
                && sim.ticks == t
            {
                sim.rider.ride.chassis.vel = vel;
            }
        };
        let mut game = Rides { fliers: &mut [], sims: &mut self.sims, world: &mut self.world, after: Some(&mut after), time_scale: None, plane: None, cars: &self.cars };
        np.frame(&mut game, local, ROUND.as_secs_f32());
    }

    /// Where seat `seat`'s car is, if it is in one.
    fn car_at(&self, seat: usize) -> Option<(Kind, f32)> {
        let lane = self.sims.get(seat)?.car.as_ref()?;
        let at = lane.machine.matrix.0[3];
        Some((vehicle::lane_up(&self.sims[seat]), at.x().hypot(at.z())))
    }
}

/// Seat `seat`'s pad at tick `tick`: `script`'s pick from its tick on, the gas held from then too
/// and the stick a little to one side, so the car circles.
fn scripted(seat: usize, tick: usize, script: &[(usize, usize, Kind)]) -> Report {
    let mut r = Report::from_buttons(0);
    let pick = script.iter().filter(|&&(s, t, _)| s == seat && tick >= t).last();
    if let Some(&(_, _, to)) = pick {
        r = Report::from_buttons(game_sim::input::Control::Cross.mask());
        r.set_axis(2, 0xa0);
        r.0[net::SWITCH] = vehicle::Wanted { to: Some(to), dpad_rides: true, ..Default::default() }.byte();
    }
    r
}

/// ggrs' clock a [`ROUND`] a round from now on, on this thread: before any session is made.
fn driven() {
    ggrs::clock::drive(true);
}

fn dumps() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("openmx-online-car-{}", std::process::id()))
}

/// The machines `nets` with their rides `games` over a link `lag` rounds long that loses nothing,
/// each one's pads `script`'s, for `rounds`; `each(round, nets, games)` before every round, which may
/// add a machine (a joiner) to both. ggrs' clock is driven from [`driven`], called before the
/// sessions are made (ggrs reads the clock as it starts them), and let go after the last round.
fn rounds(
    nets: &mut Vec<Netplay<rides::State>>,
    games: &mut Vec<Machine2>,
    lag: u32,
    rounds: std::ops::Range<u32>,
    script: &[(usize, usize, Kind)],
    each: impl FnMut(u32, &mut Vec<Netplay<rides::State>>, &mut Vec<Machine2>),
) {
    rounds_on(nets, games, lag, rounds, &|s, t| scripted(s, t, script), each);
}

/// [`rounds`] with each seat's pad at each tick `pad`'s.
fn rounds_on(
    nets: &mut Vec<Netplay<rides::State>>,
    games: &mut Vec<Machine2>,
    lag: u32,
    rounds: std::ops::Range<u32>,
    pad: &dyn Fn(usize, usize) -> Report,
    mut each: impl FnMut(u32, &mut Vec<Netplay<rides::State>>, &mut Vec<Machine2>),
) {
    let mut wire: Vec<(String, u32, String, Vec<u8>)> = Vec::new();
    for round in rounds {
        each(round, nets, games);
        for (to, at, from, d) in std::mem::take(&mut wire) {
            if at > round {
                wire.push((to, at, from, d));
            } else if let Some(np) = nets.iter_mut().find(|n| n.me == to) {
                np.heard(&from, &d);
            }
        }
        for (np, game) in nets.iter_mut().zip(games.iter_mut()) {
            let local: Vec<Report> = np.lineup.mine(&np.me).into_iter().map(|s| pad(s, np.tick())).collect();
            game.frame(np, &local);
            for (to, d) in np.outgoing() {
                assert!(d.len() <= net::link::DATAGRAM_MAX, "a datagram of {} bytes", d.len());
                wire.push((to, round + lag, np.me.clone(), d));
            }
            np.said.clear();
        }
        ggrs::clock::advance(ROUND);
    }
    ggrs::clock::drive(false);
}

/// Every machine of `nets` ran the same pads and had the same state before every tick all of them
/// settled from `from`: how many ticks that is.
fn agree(nets: &[Netplay<rides::State>], from: usize) -> usize {
    let n = nets.iter().map(|np| np.settled().max(0) as usize).min().unwrap();
    for np in nets {
        let at = (from..n).find(|&t| np.hashes[t] != nets[0].hashes[t] || np.log[t] != nets[0].log[t]);
        assert_eq!(at, None, "{} parts from {} at tick {at:?}", np.me, nets[0].me);
    }
    n.saturating_sub(from)
}

fn online_cars(disc: &mut Disc) -> Vec<CarRide> {
    let machines = disc.res("machine.res").expect("the machines' archive");
    vehicle::car_kit(&mut |n| machines.read_suffix(n).ok())
}

/// A desync caught while the host's seat is out in the truck: g's state is put back from the host's
/// bytes, the truck in them with its collision's probe (`Rides::probe`), and from the resync on the
/// two machines agree again tick for tick with the truck still out and driven on — on both.
#[test]
fn a_resync_with_the_truck_out_carries_it_and_both_drive_on() {
    let Some(mut disc) = disc() else { return };
    let cars = online_cars(&mut disc);
    let lineup = Lineup::parse("host,g").unwrap();
    driven();
    let mut nets: Vec<Netplay<rides::State>> = ["host", "g"].iter().map(|me| Netplay::new(lineup.clone(), me, Timing::default(), dumps()).unwrap()).collect();
    let mut games: Vec<Machine2> = (0..2).map(|_| Machine2::new(2, &cars)).collect();
    games[1].glitch = true;
    let script = [(0, 30, Kind::Monster)];
    let mut out = Vec::new();
    rounds(&mut nets, &mut games, 3, 0..900, &script, |round, _, games| {
        if round == 300 {
            out = games.iter().map(|g| g.car_at(0)).collect();
        }
    });
    assert!(out.iter().all(|c| c.is_some_and(|(k, _)| k == Kind::Monster)), "the truck out before the glitch: {out:?}");
    let fresh = nets[1].resync.fresh as usize;
    for np in &nets {
        assert!(np.desyncs.first().is_some_and(|d| (320..400).contains(&d.0)), "{}: {:?}", np.me, np.desyncs);
        assert_eq!(np.resync.done, 1, "{}: resyncs", np.me);
        assert!(!np.desynced, "{} still out of sync", np.me);
        if let Some(report) = &np.report {
            let _ = std::fs::remove_file(report);
        }
    }
    assert!(fresh > 330, "taken at {fresh}");
    let n = agree(&nets, fresh);
    assert!(n > 300, "only {n} ticks agreed after the resync");
    // The truck still out on both and gone on from where it was before the glitch (the same truck
    // tick for tick is the hashes above, which hold it: the machines end on ticks of their own).
    for (who, g) in ["host", "g"].iter().zip(&games) {
        let (kind, far) = g.car_at(0).expect("the truck still out");
        assert_eq!(kind, Kind::Monster, "{who}");
        assert!(far > out[0].unwrap().1 + 30.0, "{who}: the truck went from {out:?} to {far} ft");
    }
    assert!(games.iter().all(|g| g.sims[0].car.as_ref().unwrap().struck.as_ref().is_some_and(|s| !s.rays.segments.is_empty())), "a car without its probe");
}

/// A freeride opened online with the host out in the baja: g joins mid-ride, its seat spawned from
/// the host's, and takes the ride's state with the baja in it; from the join on both machines agree,
/// the baja driven on both, and g's own pick of the buggy after it goes as any pick does — two cars
/// out at once, the same on both.
#[test]
fn a_join_with_the_baja_out_takes_it_and_a_second_car_follows() {
    let Some(mut disc) = disc() else { return };
    let cars = online_cars(&mut disc);
    let spawn = rides::seat_bytes(&Machine2::new(1, &cars).sims[0]);
    driven();
    let mut nets = vec![Netplay::new(Lineup::parse("host").unwrap(), "host", Timing::default(), dumps()).unwrap()];
    let mut games = vec![Machine2::new(1, &cars)];
    // The baja from tick 30; g joins at round 200 (asked 30 before); its buggy from its tick 500.
    let script = [(0, 30, Kind::Baja), (1, 500, Kind::Buggy)];
    let join = 200;
    let mut at_join = None;
    rounds(&mut nets, &mut games, 3, 0..1000, &script, |round, nets, games| {
        if round + 30 == join {
            nets[0].admit("g", &[1], rides::spawn_on(&spawn, &[])).unwrap();
        }
        if round == join {
            at_join = games[0].car_at(0);
            nets.push(Netplay::joining("host", "g", Timing::default(), dumps()).unwrap());
            games.push(Machine2::new(1, &cars));
        }
    });
    assert!(at_join.is_some_and(|(k, _)| k == Kind::Baja), "the baja out at the join: {at_join:?}");
    let t = nets[0].admit.joins.first().expect("g taken on").1 as usize;
    for np in &nets {
        assert_eq!(np.lineup.text(), "host,g", "{}", np.me);
        assert!(np.desyncs.is_empty() && !np.desynced, "{}: {:?}", np.me, np.desyncs);
    }
    let n = agree(&nets, t);
    assert!(n > 600, "host and g agree on {n} ticks from the join at {t}");
    for (who, g) in ["host", "g"].iter().zip(&games) {
        assert_eq!(g.sims.len(), 2, "{who}");
        let (baja, buggy) = (g.car_at(0).expect("the baja out"), g.car_at(1).expect("g's buggy out"));
        assert_eq!((baja.0, buggy.0), (Kind::Baja, Kind::Buggy), "{who}");
        assert!(baja.1 > at_join.unwrap().1 + 30.0, "{who}: the baja went from {at_join:?} to {baja:?}");
    }
}

/// F23: an online bike meets the other seat's car as split screen's does (F26). g's seat takes the
/// buggy where its bike stands, 25 ft up the host's way and facing along it, and leaves it parked,
/// its pad idle; at tick 60 the host's bike is set rolling into its tail at 30 ft/s. On both
/// machines its body rays strike the car's hull (`buggyf.col`): the bike stops short of the car,
/// where with no hull it rolls on through, and its resolve pushes the car along its way (faster in
/// the second after the shove, and further by the end), where the car with no hull only creeps; and
/// the two machines agree tick for tick throughout.
#[test]
fn an_online_bike_is_stopped_by_the_other_seats_parked_car() {
    use game_sim::bike_col::{self, BikeCol};
    use ps2_float::Vf;
    let Some(mut disc) = disc() else { return };
    let res = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let read = |name: String| res.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
    let col = std::sync::Arc::new(
        BikeCol::load(&read(bike_col::rays_file(bike_col::MODEL)), &read(bike_col::shape_file(bike_col::MODEL))).expect("the bike's collision files"),
    );
    let bikes: Vec<String> = disc.entries().iter().map(|e| e.name.clone()).filter(|n| n.to_ascii_lowercase().starts_with("bike\\")).collect();
    let phy = bikes.iter().find_map(|b| disc.res(b).ok()?.read_suffix("race125p.phy").ok()).expect("race125p.phy");
    let tables = game_sim::ride::Tables::build();
    let phy = game_sim::phy::Phy::read_bytes(&phy, &tables.curve);
    let cars = online_cars(&mut disc);
    assert!(cars.iter().find(|c| c.machine == Machine::Buggy).is_some_and(|c| c.hull.is_some()), "machine.res has no buggyf.col");
    let machine = |hull: bool| {
        let mut cars = cars.clone();
        if !hull {
            cars.iter_mut().for_each(|c| c.hull = None);
        }
        let sims = [0.0, 25.0]
            .into_iter()
            .map(|z| {
                let mut sim = Sim::new(0xf23);
                std::sync::Arc::make_mut(&mut sim.phy).collisions = phy.collisions;
                let mounts = game_sim::stock::mounts(&sim.phy);
                sim.rider = game_sim::rider::Rider::new(&sim.phy, mounts, Vf::new(0.0, 1.0, z, 1.0), game_sim::stock::FACING, Default::default());
                sim.rider.sense.wrecks_enabled = false;
                sim.rider.carries_bike_col(std::sync::Arc::clone(&col));
                sim
            })
            .collect();
        Machine2 { sims, world: Flat::default(), cars: vec![cars; net::SEATS], glitch: false, shove: Some((0, 60, Vf::new(0.0, 0.0, 30.0, 0.0))) }
    };
    // g's seat on the buggy from its first tick, never on the gas; the host's pad idle.
    let pad = |seat: usize, _: usize| {
        let mut r = Report::from_buttons(0);
        if seat == 1 {
            r.0[net::SWITCH] = vehicle::Wanted { to: Some(Kind::Buggy), dpad_rides: true, ..Default::default() }.byte();
        }
        r
    };
    let ride = |hull: bool| {
        let lineup = Lineup::parse("host,g").unwrap();
        driven();
        let mut nets: Vec<Netplay<rides::State>> =
            ["host", "g"].iter().map(|me| Netplay::new(lineup.clone(), me, Timing::default(), dumps()).unwrap()).collect();
        let mut games: Vec<Machine2> = (0..2).map(|_| machine(hull)).collect();
        // The car where it stood at the shove, and how fast it went in the second after.
        let mut parked: Vec<Option<f32>> = vec![None; 2];
        let mut fastest = [0.0f32; 2];
        rounds_on(&mut nets, &mut games, 3, 0..160, &pad, |_, _, games| {
            for (i, g) in games.iter().enumerate() {
                let Some(lane) = g.sims[1].car.as_ref() else { continue };
                match g.sims[0].ticks {
                    ..=60 => parked[i] = Some(lane.machine.body.position.z()),
                    61..=90 => fastest[i] = fastest[i].max(lane.machine.body.velocity.z()),
                    _ => {}
                }
            }
        });
        assert!(nets.iter().all(|np| np.desyncs.is_empty() && !np.desynced), "the machines parted");
        let n = agree(&nets, 0);
        assert!(n > 120, "only {n} ticks agreed");
        games
            .iter()
            .zip(parked)
            .zip(fastest)
            .map(|((g, parked), fastest)| {
                assert!(g.sims[0].ticks > 100, "the host's bike rode {} ticks", g.sims[0].ticks);
                let car = g.sims[1].car.as_ref().expect("the buggy is out").machine.body.position.z();
                (g.sims[0].rider.ride.position().z(), car - parked.expect("the buggy parked before the shove"), fastest)
            })
            .collect::<Vec<_>>()
    };
    let (met, through) = (ride(true), ride(false));
    for (who, (&(bike, pushed, fastest), &(past, still, idle))) in ["host", "g"].iter().zip(met.iter().zip(&through)) {
        println!(
            "{who}: bike at z {bike}, the car pushed {pushed} ft, at most {fastest} ft/s; with no hull the bike at z {past}, the car moved {still} ft, at most {idle} ft/s"
        );
        assert!(past > 30.0, "{who}: with no hull the bike stopped at z {past}, short of the car's far side");
        assert!(bike < 25.0, "{who}: the bike went on to z {bike}, through the car at z 25");
        assert!(fastest > idle + 1.0, "{who}: the car was pushed to {fastest} ft/s along the bike's way (with no hull {idle})");
        // Parked on an idle pad the buggy creeps on along its nose a little either way.
        assert!(pushed > still + 1.0, "{who}: the car moved {pushed} ft along the bike's way (with no hull {still})");
    }
}

/// T2: the pause of a rider online offers the cars. Set as the shell sets it in an online ride
/// (main.rs: `menu.cars` from the seat's garage, which has its copies of the disc's cars, and
/// `menu.session`), the switch row goes Buggy, Monster Truck, Baja and round again, the session's
/// Leave row beside it; Cross on each asks for that machine, whose word is a car this machine's
/// online lane can take (the copies `Lot::online_cars` hands the session are the same kit).
#[test]
fn the_online_pause_offers_the_cars() {
    use game_sim::input::Control;
    use menu::{Act, Controls, Machine as Pick, Menu, Reader, Screen, Settings};
    let Some(mut disc) = disc() else { return };
    let cars = online_cars(&mut disc);
    let mut garage = Garage::new(Kind::Bike, None).unwrap();
    garage.cars = cars.clone();

    let mut reader = Reader::new();
    let mut m = Menu::new(Settings::default(), Controls::default());
    let mut tap = |m: &mut Menu, c: Option<Control>| -> Vec<Act> {
        // A press, then three frames up so the next tap is not a bounce (P109).
        let press = c.map_or(Report::idle(), |c| Report::from_buttons(c.mask()));
        [press, Report::idle(), Report::idle(), Report::idle()].iter().map(|r| m.step(reader.read(r))).collect()
    };
    tap(&mut m, None);
    tap(&mut m, Some(Control::Cross));
    tap(&mut m, Some(Control::Cross));
    assert!(m.riding());
    m.riders = 1;
    m.session = true;
    m.up[0] = Pick::from_word(garage.up.name()).unwrap();
    m.switchable[0] = garage.plane.is_some();
    for (k, kind) in [Kind::Buggy, Kind::Monster, Kind::Baja].into_iter().enumerate() {
        m.cars[0][k] = garage.machines().contains(&kind);
    }
    assert_eq!(m.choices(0), [Pick::Buggy, Pick::Monster, Pick::Baja]);

    tap(&mut m, Some(Control::Start));
    assert_eq!(m.screen(), Some(Screen::Pause));
    let rows = m.pause_rows();
    let at = rows.iter().position(|r| r.starts_with("Switch To Machine")).expect("a switch row");
    assert!(rows.iter().any(|r| r == "Leave Session"), "the online pause: {rows:?}");
    for _ in 0..at {
        tap(&mut m, Some(Control::Down));
    }
    for want in [Pick::Buggy, Pick::Monster, Pick::Baja, Pick::Buggy] {
        assert_eq!(m.pause_rows()[at], format!("Switch To Machine: < {} >", want.label()));
        tap(&mut m, Some(Control::Right));
    }
    // On the truck (one Right on from the round), Cross asks for it.
    let acts = tap(&mut m, Some(Control::Cross));
    assert!(acts.contains(&Act::Switch(0, Pick::Monster)), "{acts:?}");
    let to = Kind::parse(Pick::Monster.word()).unwrap();
    assert!(cars.iter().any(|c| Some(c.machine) == to.car()), "the lane has no {to:?}");
    for pick in [Pick::Buggy, Pick::Baja] {
        let to = Kind::parse(pick.word()).unwrap();
        assert!(cars.iter().any(|c| Some(c.machine) == to.car()), "the lane has no {to:?}");
    }
}

/// P289: a seat's car online rides that seat's own car modifiers. Seat 1 switches into the buggy
/// and drives it on the gas for three seconds through [`rides::Modded`] on a set whose row 1 has the
/// cars' acceleration at 150% (row 0 at 50%, seat 0 empty): it drives tick for tick as [`Rides`] on
/// the cars [`rides::cars_on`] builds, and further than on the disc's cars. The host's change back to
/// the original puts every seat's cars back to the disc's.
#[test]
fn a_seats_car_online_rides_its_own_car_modifiers() {
    use game_sim::modifiers::{Knob, Mods};
    use net::Game;
    let Some(mut disc) = disc() else { return };
    let stock = vec![online_cars(&mut disc); net::SEATS];
    let mut mods = Mods::original();
    mods.set(0, Knob::CarAccel, 50);
    mods.set(1, Knob::CarAccel, 150);
    let mods = rides::ride_set(&mods);
    let modded = rides::cars_on(&stock, &mods);
    let car = |cars: &[CarRide]| cars.iter().find(|c| c.machine == Machine::Buggy).unwrap().car.clone();
    assert_ne!(car(&modded[1]), car(&stock[1]), "row 1's buggy is the disc's");
    assert_ne!(car(&modded[0]), car(&modded[1]), "seat 0's buggy is on seat 1's row");
    assert!(modded[2..].iter().zip(&stock[2..]).all(|(m, s)| car(m) == car(s)), "a seat on the original's has the disc's cars");
    assert!(rides::cars_on(&stock, &Mods::original()).iter().zip(&stock).all(|(m, s)| m.iter().zip(s).all(|(a, b)| a.car == b.car)));

    let pad = |tick: usize| [Report::missing(), scripted(1, tick, &[(1, 0, Kind::Buggy)])];
    let bike = Sim::new(0x289);
    let phys = vec![(*bike.phy).clone()];
    // Each run: the hashes after every tick, and where seat 1's car got to.
    let run = |how: u8| -> (Vec<u64>, f32) {
        let mut sims = vec![bike.clone(), bike.clone()];
        let mut world = Flat::default();
        let mut live = modded.clone();
        let mut hashes = Vec::new();
        let plain = if how == 1 { &modded[..] } else { &stock[..] };
        let mut ride = Rides { fliers: &mut [], sims: &mut sims, world: &mut world, after: None, time_scale: None, plane: None, cars: if how == 0 { &[] } else { plain } };
        let mut m = mods;
        let mut game = rides::Modded { rides: &mut ride, stock: &phys, mods: &mut m, stock_plane: None, cars: (how == 0).then_some((&stock[..], &mut live)) };
        for tick in 0..90 {
            game.step(tick, &pad(tick));
            hashes.push(game.hash());
        }
        let at = sims[1].car.as_ref().expect("seat 1 is in the buggy").machine.matrix.0[3];
        (hashes, at.x().hypot(at.z()))
    };
    let (online, far) = run(0);
    let (plain, _) = run(1);
    let (_, disc_far) = run(2);
    assert_eq!(online, plain, "the modified cars are not the ones the ride drove");
    assert!(far > disc_far * 1.1, "seat 1's buggy went {far} ft at 150% acceleration, {disc_far} ft on the disc's");

    // The host's change to the original's: every seat's cars the disc's again.
    let mut sims = vec![bike.clone(), bike.clone()];
    let mut world = Flat::default();
    let mut live = modded.clone();
    let mut ride = Rides { fliers: &mut [], sims: &mut sims, world: &mut world, after: None, time_scale: None, plane: None, cars: &[] };
    let mut m = mods;
    let mut game = rides::Modded { rides: &mut ride, stock: &phys, mods: &mut m, stock_plane: None, cars: Some((&stock[..], &mut live)) };
    game.remodify(&mods.text(), "").unwrap();
    drop(game);
    assert!(live.iter().zip(&stock).all(|(m, s)| m.iter().zip(s).all(|(a, b)| a.car == b.car)), "the change left a seat's cars modified");
}
