//! V36: **a stopped bike's rider puts a foot down**, and takes it back up moving off.
//!
//! The shell's own two calls a frame — the ride's [`rider::Rider::frame`], then the body's
//! [`Body::ride`] — and the pose the draw holds on to the bike ([`Body::palette`]): two seconds on
//! the throttle, the brakes on until the bike stands, three seconds standing, and the throttle again.
//!
//! The clip is the pose grid's stopped row, `turn_stop_*`, which the body's machine already blends in
//! under the grid's first band (7.3333 ft/s); its feet are a foot under the pegs. What kept them on
//! the pegs was the draw's hold. The original's riding ragdoll lets go of both feet on the tick the
//! request's speed drops under the same 7.3333 with the bike not yawing ([`Feet`], proved against the
//! body actor's switch in `game-sim/tests/foot_down.rs`, and that against the original's update in
//! S109's `ragdoll_riding.rs`), and takes them back on the tick it is over it again.
//!
//! Needs the user's own disc; skips with the reason when it is not there.

use game_render::disc::{Disc, Res};
use game_render::psg::Model;
use game_render::rider::{Body, FOOT_ANCHORS, HAND_ANCHORS, Sex, Skeleton, anchors_of};
use game_sim::foot_down::Feet;
use game_sim::ragdoll_riding::STAND_SPEED;
use game_sim::ride::Controls;
use game_sim::rider_anim::Mode;
use game_sim::trick::{Set as Stunts24, Stunts};
use game_sim::collide::{Bundle, Record};
use game_sim::world::World;
use game_sim::{phy::Phy, rider};
use ps2_float::{F, Vf};

/// The engine's own clamped step, the frame the shell rides at.
const FRAME: F = F(1.0 / 60.0);
/// An inch, in the disc's unit (a foot).
const AN_INCH: f32 = 1.0 / 12.0;

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

/// A flat floor at height 0 whose normal is a direction (w = 0), which a ride pulls away on.
struct Floor;

impl World for Floor {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// One tick of what the test watches.
struct Tick {
    speed: f32,
    yaw: f32,
    mode: Mode,
    feet: Feet,
    /// How far each foot's anchor is drawn under its peg, left then right, and how far each hand's
    /// is drawn from its grip.
    under: [f32; 2],
    hands: [f32; 2],
}

#[test]
fn a_stopped_bike_puts_a_foot_down_and_moving_off_lifts_it() {
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
    let chassis = Skeleton::of(&Model::parse(&bike.read_suffix("bike.psg").expect("the chassis")).expect("it parses"));
    let grips = anchors_of(&chassis, &[]);
    let mut body = Body::load(vec![res(&mut disc, "thq1.res")], "thq1", Sex::Male, "", &set.stunts, p.motion())
        .unwrap_or_else(|why| panic!("the body did not load: {why}"));
    let skel = body.rig.set.skeleton.clone();
    let joint = |name: &str| skel.find(name).unwrap_or_else(|| panic!("the rider has no {name}"));
    let (feet, hands) = (FOOT_ANCHORS.map(joint), HAND_ANCHORS.map(joint));

    let mounts = game_sim::stock::mounts(&p);
    let mut r = rider::Rider::new(&p, mounts, Vf::new(0.0, 1.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), set.clone());
    r.sense.wrecks_enabled = false;
    r.body_rates = game_render::vehicle::params();
    let mut floor = Floor;

    let gas = Controls { gas: F::ONE, ..Controls::default() };
    let brake = Controls { brake: F::ONE, ..Controls::default() };
    // Set down first: since S67's bottom-out shove, full gas held through the drop from the spawn
    // launches the 250 off the landing and loops it over backwards.
    for _ in 0..60 {
        r.frame(&p, &tables, &Controls::default(), FRAME, &mut floor);
        body.ride(FRAME, &mut r, &Controls::default(), rider::Gate::default());
    }
    let legs: [(&str, Controls, usize); 4] = [("ride", gas, 120), ("brake", brake, 180), ("stand", Controls::default(), 180), ("off", gas, 120)];
    let mut ticks: Vec<(&str, Tick)> = Vec::new();
    for (leg, c, n) in legs {
        for _ in 0..n {
            // The brakes only until the bike all but stands: held on a standing bike they back it up.
            let c = if leg == "brake" && r.ride.chassis.local_vel.z() < 0.5 { Controls::default() } else { c };
            r.frame(&p, &tables, &c, FRAME, &mut floor);
            body.ride(FRAME, &mut r, &c, rider::Gate::default());
            let (world, _) = body.palette(Some(&grips));
            let at = |j: usize| world[j].0[3];
            let under = [0, 1].map(|k| grips[k].expect("a peg").y - at(feet[k])[1]);
            let hands = [0, 1].map(|k| {
                let (a, g) = (at(hands[k]), grips[2 + k].expect("a grip"));
                ((a[0] - g.x).powi(2) + (a[1] - g.y).powi(2) + (a[2] - g.z).powi(2)).sqrt()
            });
            let c = &r.ride.chassis;
            let tick = Tick { speed: c.local_vel.z(), yaw: c.ang_vel.y(), mode: r.body.machine.mode, feet: body.feet, under, hands };
            ticks.push((leg, tick));
        }
    }
    assert!(r.state.wreck.is_none(), "the ride wrecked");

    // The switch's own rule, tick for tick, on every riding tick: both feet go together, on the tick
    // the speed is under the stand speed and the bike is not yawing, and nowhere else.
    let slow = |t: &Tick| t.speed < STAND_SPEED.0 && t.yaw.abs() < 1.5;
    let riding: Vec<&(&str, Tick)> = ticks.iter().filter(|(_, t)| t.mode == Mode::Riding).collect();
    for (i, (leg, t)) in riding.iter().enumerate() {
        assert_eq!(t.feet.held, [!slow(t); 2], "{leg} riding tick {i}: {:.3} ft/s, yaw {:.3}", t.speed, t.yaw);
    }

    // Standing: the feet are off the pegs and on the floor the stopped clip stands them on, and the
    // hands are still on the grips.
    let stood: Vec<&Tick> = ticks.iter().filter(|(l, t)| *l == "stand" && t.mode == Mode::Riding).map(|(_, t)| t).collect();
    assert!(stood.len() > 60, "the stopped bike was riding on only {} ticks", stood.len());
    let last = stood.last().expect("a standing tick");
    assert!(last.speed.abs() < 0.1, "the bike never stopped: {} ft/s", last.speed);
    for t in &stood[stood.len() - 30..] {
        assert!(t.under.iter().all(|&u| u > 0.9), "a stopped rider's foot is only {:?} under its peg", t.under);
        assert!(t.hands.iter().all(|&h| h < AN_INCH), "a stopped rider's hand is {:?} off its grip", t.hands);
    }

    // Braking to a stop: the feet come off on the tick the speed crosses the stand speed.
    assert!(!ticks[119].1.feet.down(), "two seconds on the throttle and a foot is still down");
    let first_down = ticks.iter().skip(120).position(|(_, t)| t.feet.down()).expect("no foot ever came down") + 120;
    let crossed = ticks.iter().position(|(l, t)| *l == "brake" && slow(t)).expect("the brakes never slowed the bike");
    assert_eq!(first_down, crossed, "the feet came down on tick {first_down}, the speed crossed on {crossed}");

    // Moving off: back on the tick the speed is over the stand speed, from three quarters, and on
    // the pegs within an inch once the fade has run.
    let off: Vec<&Tick> = ticks.iter().filter(|(l, _)| *l == "off").map(|(_, t)| t).collect();
    let up = off.iter().position(|t| !t.feet.down()).expect("moving off never lifted the feet");
    assert!(!slow(off[up]) && slow(off[up - 1]), "the feet went up on tick {up} of moving off, not where the speed crossed");
    let w = off[up].feet.weight[0].0;
    assert!((0.75..0.8).contains(&w), "the feet's hold came back at {w}, not from three quarters");
    let settled = off.iter().skip(up + 40).filter(|t| t.mode == Mode::Riding).collect::<Vec<_>>();
    assert!(!settled.is_empty() && settled.iter().all(|t| t.feet.weight == [F::ONE; 2]), "the hold never faded back to one");
    let worst = settled.iter().flat_map(|t| t.under).fold(0.0f32, |m, u| m.max(u.abs()));
    assert!(worst < AN_INCH, "moving off, a foot is still {worst} off its peg");
    println!(
        "stopped from {:.1} ft/s: feet down on tick {first_down}, {:.2}/{:.2} ft under the pegs standing; \
         up {up} ticks into moving off at {:.2} ft/s",
        ticks[119].1.speed, last.under[0], last.under[1], off[up].speed
    );
}
