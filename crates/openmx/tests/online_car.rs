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
//! Needs the disc's `machine.res` cars and skips without the disc, saying so.

#[allow(unused)]
#[path = "../src/thread.rs"]
mod thread;
#[allow(dead_code)]
#[path = "../src/audio.rs"]
mod audio;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use game_render::Disc;
use game_sim::input::{PADS, Report};
use game_sim::world::Flat;
use game_sim::Sim;
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

/// Two shells riding `track` online for `frames` ticks, the host's on `script`: each one's output.
fn ride_online(track: &str, frames: u32, script: &str) -> [String; 2] {
    // Two ports the system has free now (machines_everywhere's way).
    let port = || std::net::UdpSocket::bind("127.0.0.1:0").and_then(|s| s.local_addr()).expect("a free port").port();
    let (hp, mut gp) = (port(), port());
    while gp == hp {
        gp = port();
    }
    let (h, g) = (format!("host@127.0.0.1:{hp}"), format!("g@127.0.0.1:{gp}"));
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
        shell.spawn().expect("the shell starts")
    };
    // A shell that fails takes its peer down with it: the peer would wait for its pads for ever.
    let mut kids = [ride(&h, &g, Some(script)), ride(&g, &h, None)];
    let readers = kids.each_mut().map(|c| {
        let mut out = c.stdout.take().expect("piped stdout");
        std::thread::spawn(move || {
            let mut said = String::new();
            std::io::Read::read_to_string(&mut out, &mut said).ok();
            said
        })
    });
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
    let said = ride_online("free09", 420, "40:monster,41:gas,200:lift,330:bike");
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
