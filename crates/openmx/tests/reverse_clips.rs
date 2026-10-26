//! P72: **backing up, the drawn rider plays the reverse pair**, `reverse_l` and `reverse_r`.
//!
//! The shell's own two calls a frame — the ride's [`rider::Rider::frame`], then the body's
//! [`Body::ride`] — on the disc's 250 and the disc's rider: set down, then the gas and the brake held
//! together at a standstill, which is the original's only reverse (`Controls::reverse_rule`).
//!
//! What kept the rider out of the pair before: the body's machine picks it off the riding request's
//! reverse command (`+0x16c`, a copy of the pad read's `+0x648`), and the port's request read
//! `Controls::reverse`, which no pad ever set. Now it reads the command the ride's pad read left
//! ([`game_sim::ride::Held::reverse`]), so the pair is selected; the clips are the body's own, loaded
//! by name with the rest of the pose table, and the frame hands both to the draw at the pair's own
//! clock (vehicle_anim's arm for states 33/34).
//!
//! Needs the user's own disc; skips with the reason when it is not there.

use game_render::disc::{Disc, Res};
use game_render::rider::{Body, FOOT_ANCHORS, Sex};
use game_sim::collide::{Bundle, Record};
use game_sim::ride::{Controls, REVERSE_BELOW};
use game_sim::rider_anim::Mode;
use game_sim::trick::{Set as Stunts24, Stunts};
use game_sim::vehicle_anim::{Blend, REVERSE_L, REVERSE_R};
use game_sim::world::World;
use game_sim::{phy::Phy, rider};
use ps2_float::{F, Vf};

/// The engine's own clamped step, the frame the shell rides at.
const FRAME: F = F(1.0 / 60.0);

fn member(disc: &Disc, ends: &str) -> Option<String> {
    let want = ends.to_ascii_lowercase();
    disc.entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(&want))
        .min_by_key(|e| e.name.len())
        .map(|e| e.name.clone())
}

fn res(disc: &mut Disc, ends: &str) -> Res {
    let name = member(disc, ends).unwrap_or_else(|| panic!("the disc has no {ends}"));
    disc.res(&name).unwrap_or_else(|why| panic!("{name} does not open: {why}"))
}

/// A flat floor at height 0 whose normal is a direction (w = 0).
struct Floor;

impl World for Floor {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

#[test]
fn backing_up_the_rider_plays_the_reverse_pair() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            println!("SKIPPED: {why}");
            return;
        }
    };
    let app = res(&mut disc, "appstart.res");
    let table = app.read_suffix("rider\\anim\\stunts.ini").expect("the app archive has no stunt table");
    let set = Stunts24::new(Stunts::parse(&table).expect("the stunt table parses").selection(0));
    let tables = rider::Tables::build();
    let bike = res(&mut disc, "250sx.res");
    let p = Phy::read_bytes(&bike.read_suffix(".phy").expect("the bike has a .phy"), &tables.ride.curve);
    let mut body = Body::load(vec![res(&mut disc, "thq1.res")], "thq1", Sex::Male, "", &set.stunts, p.motion())
        .unwrap_or_else(|why| panic!("the body did not load: {why}"));
    // Loaded: the pair is in the body's pose table, bound to its skeleton.
    let (left, right) = (body.pose_lib.clip(REVERSE_L), body.pose_lib.clip(REVERSE_R));
    assert!(left.is_some() && right.is_some(), "the rider has no reverse pair: {left:?} {right:?}");
    let foot = body.rig.set.skeleton.find(FOOT_ANCHORS[0]).expect("the rider has a left foot");

    let mounts = game_sim::stock::mounts(&p);
    let mut r = rider::Rider::new(&p, mounts, Vf::new(0.0, 1.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), set.clone());
    r.sense.wrecks_enabled = false;
    r.body_rates = game_render::vehicle::params();
    let mut floor = Floor;
    for _ in 0..60 {
        r.frame(&p, &tables, &Controls::default(), FRAME, &mut floor);
        body.ride(FRAME, &mut r, &Controls::default(), rider::Gate::default());
    }
    // A blip of the throttle opens the start gate, which only the throttle does; then let it roll.
    let gas = Controls { gas: F::ONE, ..Controls::default() };
    for n in 0..120 {
        let c = if n < 10 { gas } else { Controls::default() };
        r.frame(&p, &tables, &c, FRAME, &mut floor);
        body.ride(FRAME, &mut r, &c, rider::Gate::default());
    }
    assert_eq!(r.body.machine.mode, Mode::Riding, "the throttle did not open the gate");
    let stood = body.palette(None).0[foot].0[3];

    let both = Controls { gas: F::ONE, brake: F::ONE, ..Controls::default() };
    let mut reversing = 0;
    let mut feet = Vec::new();
    let mut slowest = 0.0f32;
    for i in 0..240 {
        let seen = r.ride.air.contact_vel.z();
        r.frame(&p, &tables, &both, FRAME, &mut floor);
        let frame = body.ride(FRAME, &mut r, &both, rider::Gate::default());
        assert!(r.state.wreck.is_none() && r.body.machine.mode == Mode::Riding, "tick {i}: not riding but {:?}", r.body.machine.mode);
        assert_eq!(r.ride.held.reverse, if seen < REVERSE_BELOW { F::ONE } else { F::ZERO }, "tick {i}");
        slowest = slowest.min(r.ride.chassis.local_vel.z());
        // Asked for on the tick the command is up; running from the next, as the step latches it.
        // With the bars centred, the right one: the original's state 34 all the way back (P72's capture).
        assert_eq!(r.body.veh.want, REVERSE_R, "tick {i}: the body asked for state {}", r.body.veh.want);
        if r.body.veh.state != REVERSE_R {
            assert!(i < 2, "tick {i}: backing up, the body runs state {}", r.body.veh.state);
            continue;
        }
        reversing += 1;
        // Handed to the draw: the pair, right then left, mixed by its own clock.
        match frame.blend {
            Blend::Two { a, b, .. } => assert_eq!((a, b), (right, left), "tick {i}: the frame drew other clips"),
            other => panic!("tick {i}: backing up, the frame is {other:?}"),
        }
        assert!(body.posed, "tick {i}: nothing posed the body");
        feet.push(body.palette(None).0[foot].0[3]);
    }
    assert!(slowest < -10.0, "gas and brake at a standstill backed the bike up to only {slowest} ft/s");
    assert!(reversing > 200, "the body played the pair on only {reversing} of 240 ticks");
    // The pair moves the body: the foot is not where standing left it, and it moves with the clock.
    let dist = |a: [f32; 4], b: [f32; 4]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    let off = feet.iter().map(|f| dist(*f, stood)).fold(0.0f32, f32::max);
    let swing = feet.iter().map(|f| dist(*f, feet[0])).fold(0.0f32, f32::max);
    println!("backing up to {slowest:.2} ft/s: the pair on {reversing} ticks, the left foot up to {off:.3} ft off standing, swinging {swing:.3} ft");
    assert!(off > 0.1, "backing up, the left foot stays where standing put it ({off} ft)");
    assert!(swing > 0.05, "backing up, the left foot never moves ({swing} ft): the pair's clock does not run");
}
