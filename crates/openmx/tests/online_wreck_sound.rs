//! P184 — an online ride hears its riders' wrecks.
//!
//! Online the session runs the riders' own frames, some of them twice when a late pad rolls the ride
//! back, and the shell sees only the ride as it is drawn. [`audio::Said`] reads a wreck entry off
//! that: the wreck reason going from none to some between two drawn frames. Proved here on the real
//! session ([`Netplay`] over [`Rides`], the shell's online step) between two machines in this thread
//! on a link three rounds late, so the host guesses the guest's pad, guesses wrong and re-runs: both
//! seats are `game_sim`'s drop-and-wreck bike (P180's, `tests/audio.rs`), and the host's sound,
//! handed each drawn frame's seats, plays each rider's wreck cry and its bike's clank once, on the
//! frame the wreck is first drawn.

#[allow(dead_code)]
#[path = "../src/net/mod.rs"]
mod net;
#[allow(dead_code)]
#[path = "../src/peer.rs"]
mod peer;
#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;
#[allow(unused)]
#[path = "../src/thread.rs"]
mod thread;
#[allow(dead_code)]
#[path = "../src/audio.rs"]
mod audio;

use std::sync::{Arc, Mutex};

use audio::{BikeSounds, Clip, List, Mixer, Music};
use game_sim::input::Report;
use game_sim::phy::{self, Phy};
use game_sim::rider::Tables;
use game_sim::trick::Set;
use game_sim::world::Flat;
use game_sim::{Sim, stock};
use net::rides::{self, Rides};
use net::{Lineup, Netplay, Timing};
use ps2_float::{F, Vf};

/// A round: one tick of the ride's 30 Hz clock (`pace::TICK`).
const ROUND: std::time::Duration = std::time::Duration::from_nanos(1_001_000_000_000 / 30_000);
/// The rounds a datagram takes.
const LAG: u32 = 3;

// `tests/audio.rs`' wreck fixture (P180): game-sim's ridden test bike with a vertical limit low enough
// that a drop onto flat ground wrecks, a wreck body and the rider's half of the wreck.
const WRECK_BIKE: &str = include_str!("../../game-sim/tests/freeride-bike.phy.txt");
const DROP_LIMITS: &str = "
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 400
BackWreckSpeedLimit = 400
SideWreckSpeedLimit = 400
RacingDownWreckSpeedLimit = 400
FreestyleDownWreckSpeedLimit = 5
GroundPitchDownWreckAngle = 85
GroundPitchUpWreckAngle = 85
AirbornePitchDownWreckAngle = 175
AirbornePitchUpWreckAngle = 175
AirborneRollWreckAngle = 175
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
";
const DROP_PART: &str = "
[PhysicsPart1]
Type = RigidBody
Name = wreck_physics
Weight = 250.0
CollisionWeight = 0.0001
GravityScale = 1.0
InertiaScale = 1.0, 1.0, 1.0
SpringStiffness = 5.0
SpringDamping = 0.4
MaxPenetration = 0.5
CofG = 0.0, 1.5, 0.0
SpringMomentScale = 1.0, 1.0, 1.0
FrictionMomentScale = 1.0, 1.0, 1.0
FrictionZone1 = 0.5
Radius = 1.0
AccelerationThreshold = 0.01
AngularAccelerationThreshold = 0.01
";
const DROP_RIDER: &str = "
[Wrecks]
HitBottomForceTime = 0.1
HitBottomHandHoldTime = 0.3
HitBottomForce = 0.0, 800.0, 0.0
GroundAngularRateDamping = 1.0
RiderFlailHeight = 3.0
RiderRollTime = 0.75
RiderRollSpeed = 0.0
";

/// Every seat a bike dropped 30 ft onto flat ground, a seat's width apart.
fn drops(seats: usize) -> Vec<Sim> {
    let tables = Arc::new(Tables::build());
    (0..seats)
        .map(|s| {
            let p = Phy::read_bytes(format!("{WRECK_BIKE}{DROP_LIMITS}").as_bytes(), &tables.ride.curve);
            let at = Vf::new(20.0 * s as f32, 30.0, 0.0, 1.0);
            let mut r = game_sim::rider::Rider::new(&p, stock::MOUNTS, at, Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
            let part = phy::Part::read_bytes(DROP_PART.as_bytes(), "PhysicsPart1");
            r.carries_wreck(part, phy::Wrecks::read_bytes(DROP_RIDER.as_bytes()), part.inertia(Vf::new(2.5, 4.0, 7.0, 1.0)));
            Sim::with_rider(1 + s as u32, tables.clone(), p, r)
        })
        .collect()
}

/// The guest's pad: the stick swung side to side every few ticks, so the host's guess of it — the
/// last pad heard — is wrong on every swing, and the host re-runs the ticks since.
fn swing(tick: usize) -> Report {
    let mut r = Report::from_buttons(0);
    r.set_axis(2, if (tick / 4) % 2 == 0 { 0x10 } else { 0xf0 });
    r
}

fn tone(amplitude: f32, seconds: f32) -> Arc<Clip> {
    let rate = 22050;
    let pcm = (0..(seconds * rate as f32) as usize).map(|i| ((i as f32 * 0.07).sin() * amplitude) as i16).collect();
    Arc::new(Clip { rate, channels: 1, pcm })
}

/// One sound the host's frames started: the round, whose, and which.
#[derive(Debug, PartialEq)]
struct Heard {
    round: u32,
    seat: usize,
    what: &'static str,
}

/// **An online wreck plays its voice and clank** (P184): each seat's cry and its bike's clank on the
/// frame its wreck is first drawn, and the wreck's entry once, though the host ran its tick again.
#[test]
fn a_headless_online_wreck_plays_its_voice_and_clank() {
    let lineup = Lineup::parse("host,g").unwrap();
    let names = lineup.peers();
    let dumps = std::env::temp_dir().join(format!("openmx-online-wreck-{}", std::process::id()));
    ggrs::clock::drive(true);
    let mut nets: Vec<Netplay<rides::State>> =
        names.iter().map(|me| Netplay::new(lineup.clone(), me, Timing::default(), dumps.clone()).unwrap()).collect();
    let mut sims: Vec<Vec<Sim>> = names.iter().map(|_| drops(2)).collect();
    let mut worlds: Vec<Flat> = names.iter().map(|_| Flat::default()).collect();

    // The host's sound: [`tests/audio.rs`]' tone rider, and a bike a seat whose `[Wreck]` is one clank
    // of its own, so that what is heard says whose it is.
    let mixer = Arc::new(Mutex::new(Mixer::new(Music::new(Vec::new(), 1))));
    let mut sound = audio::Sound::new(mixer.clone());
    sound.rider = audio::RiderSounds {
        fall: List { sounds: vec![(tone(5000.0, 0.3), false)] },
        impact: List { sounds: vec![(tone(5000.0, 0.2), false), (tone(5000.0, 0.25), false)] },
        near: 30.0,
        far: 150.0,
        min_impulse: 12.8,
    };
    let clanks: Vec<Arc<Clip>> = (0..2).map(|_| tone(7000.0, 1.3)).collect();
    for c in &clanks {
        let wreck = audio::MachineWreck { list: List { sounds: vec![(c.clone(), false)] }, near: 30.0, far: 150.0, min_impulse: 0.0, max_impulse: 1.0 };
        sound.seats.push(audio::SeatSound::new(Arc::new(BikeSounds { wreck, ..BikeSounds::default() }), 1));
    }
    let mut said = audio::Said::default();
    let mut out = vec![0i16; audio::RATE as usize / 30 * 2];
    let mut heard: Vec<Heard> = Vec::new();
    let mut drawn_wrecks: Vec<Option<u32>> = vec![None; 2];
    let mut voices: Vec<Option<audio::VoiceId>> = vec![None; 2];
    let mut sounding: Vec<Arc<Clip>> = Vec::new();

    let mut wire: Vec<(usize, u32, String, Vec<u8>)> = Vec::new();
    for round in 0..150u32 {
        for (to, at, from, d) in std::mem::take(&mut wire) {
            if at > round {
                wire.push((to, at, from, d));
            } else {
                nets[to].heard(&from, &d);
            }
        }
        for i in 0..names.len() {
            let np = &mut nets[i];
            let local: Vec<Report> =
                np.lineup.mine(&np.me).into_iter().map(|_| if i == 0 { Report::idle() } else { swing(np.tick()) }).collect();
            let mut game = Rides { sims: &mut sims[i], world: &mut worlds[i], after: None, time_scale: None, plane: None };
            np.frame(&mut game, &local, ROUND.as_secs_f32());
            for (to, d) in np.outgoing() {
                let j = names.iter().position(|n| *n == to).unwrap();
                wire.push((j, round + LAG, names[i].clone(), d));
            }
            np.said.clear();
        }
        // The host's drawn frame, as the shell's online tick hands it to the sound.
        let frames: Vec<audio::SeatFrame> = sims[0]
            .iter()
            .enumerate()
            .map(|(s, sim)| {
                let r = &sim.rider;
                let at = r.ride.position();
                let f = said.of(s, r);
                if r.state.wreck.is_some() && drawn_wrecks[s].is_none() {
                    drawn_wrecks[s] = Some(round);
                }
                audio::SeatFrame {
                    hands: audio::Hands { grounded: r.ride.air.on_ground, at: [at.x(), at.y(), at.z()], ..audio::Hands::default() },
                    wreck: f.wreck.map(|w| w.0 as i32),
                    landed_trick: f.landed.is_some(),
                    bike: audio::BikeCrash::of(r),
                    body: audio::RiderBody::of(r),
                    ..Default::default()
                }
            })
            .collect();
        sound.frame(&frames, 1.0 / 30.0);
        for (s, f) in frames.iter().enumerate() {
            let v = sound.seats[s].voice.voice();
            if f.wreck.is_some() && v.is_some() && v != voices[s] {
                heard.push(Heard { round, seat: s, what: "entry cry" });
            }
            voices[s] = v;
        }
        let mut m = mixer.lock().unwrap();
        let now: Vec<Arc<Clip>> = m.clips().into_iter().map(|c| c.0).collect();
        for clip in &now {
            if let Some(s) = clanks.iter().position(|c| Arc::ptr_eq(c, clip))
                && !sounding.iter().any(|c| Arc::ptr_eq(c, clip))
            {
                heard.push(Heard { round, seat: s, what: "clank" });
            }
        }
        sounding = now;
        m.render(&mut out);
        drop(m);
        ggrs::clock::advance(ROUND);
    }
    ggrs::clock::drive(false);
    let _ = std::fs::remove_dir_all(&dumps);

    let host = &nets[0];
    println!("host: {} ticks, {} re-run; wrecks drawn at {drawn_wrecks:?}; heard {heard:?}", host.tick(), host.rb.reruns);
    assert!(host.rb.reruns > 0, "the host never re-ran a tick: nothing here was rolled back");
    assert!(host.desyncs.is_empty() && !host.desynced, "{:?}", host.desyncs);
    for s in 0..2 {
        let at = drawn_wrecks[s].unwrap_or_else(|| panic!("seat {s} never wrecked"));
        let cries: Vec<&Heard> = heard.iter().filter(|h| h.seat == s && h.what == "entry cry").collect();
        assert_eq!(cries, [&Heard { round: at, seat: s, what: "entry cry" }], "seat {s}'s wreck cry: once, on the frame it is first drawn");
        let first_clank = heard.iter().find(|h| h.seat == s && h.what == "clank");
        assert_eq!(first_clank, Some(&Heard { round: at, seat: s, what: "clank" }), "seat {s}'s bike clanks with the cry");
    }
}
