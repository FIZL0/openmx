//! The bike that moves, against the real disc: the chassis skeleton posed from the vehicle's own
//! animation machine, and the four nodes the simulation drives instead of a clip.
//!
//! Needs the user's own disc, so every test skips with the reason when it is not there — a fresh
//! checkout stays green.

use game_render::anim::{Set, Stance};
use game_render::disc::{Disc, Res};
use game_render::ini;
use game_render::mat::{M4, V3, v3};
use game_render::psg::Model;
use game_render::rider as gear;
use game_render::scene::Vehicle;
use game_render::skin::skin;
use game_render::vehicle::{BARS, Chassis, Drive, FRONT, REAR, ROOT, SHOCKS, SWINGARM, params};
use game_sim::ride::Controls;
use game_sim::trick::{Dir, SLOTS, Set as Stunts24, Stunts, Trick, slot_of};
use game_sim::world::Flat;
use game_sim::{phy::Phy, rider, vehicle_anim as va};
use ps2_float::{F, Vf};

/// The bike the port is built against: its `.sub`, its chassis and the `.phy` the ride runs on.
const A_BIKE: &str = "250sx";
/// Where the rider's stunt table lives, by the same two names the shell uses.
const STUNT_ARCHIVE: &str = "appstart.res";
const STUNT_TABLE: &str = "rider\\anim\\stunts.ini";
/// What the vehicle's own clip loader puts in front of a name, which the shell spells the same way.
const PREFIX: &str = "bike_";

/// The frame the fixtures run at: the engine's own clamped step.
const FRAME: F = F(1.0 / 60.0);
/// A minute of it, which is what the task asks a ride to survive.
const A_MINUTE: usize = 60 * 60;
/// How high a fixture drops the bike from, so the suspension has a landing to work through.
const DROP: f32 = 30.0;

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            println!("SKIPPED: {why}");
            None
        }
    }
}

/// The member of the ISO whose name ends this way — the shortest match, because the menus keep
/// their own copy of a bike one directory deeper.
fn member(disc: &Disc, ends: &str) -> Option<String> {
    let want = ends.to_ascii_lowercase();
    disc.entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(&want))
        .min_by_key(|e| e.name.len())
        .map(|e| e.name.clone())
}

/// One copy of the bike's archive. The animation set owns the archives it reads its `.mot` files
/// out of, so a test that both poses a chassis and loads its geometry opens the archive twice —
/// the same thing the shell does.
fn bike_res(disc: &mut Disc) -> Res {
    let name = member(disc, &format!("{A_BIKE}.res")).expect("the disc has no bike archive");
    disc.res(&name).expect("the bike archive does not open")
}

/// The 24 stunts the rider can do, off the disc's own table — the same table the vehicle's gesture
/// slots are named from.
fn stunts(disc: &mut Disc) -> Stunts24 {
    let name = member(disc, STUNT_ARCHIVE).expect("the disc has no app archive");
    let res = disc.res(&name).expect("the app archive does not open");
    let bytes = res.read_suffix(STUNT_TABLE).expect("the app archive has no stunt table");
    Stunts24::new(Stunts::parse(&bytes).expect("the stunt table parses").selection(0))
}

fn phy(disc: &mut Disc, tables: &rider::Tables) -> Phy {
    let res = bike_res(disc);
    let bytes = res.read_suffix(".phy").expect("the bike archive has no .phy");
    Phy::read_bytes(&bytes, &tables.ride.curve)
}

/// The bike's build, which is where the animation set's name comes from.
fn sub(disc: &mut Disc) -> Vehicle {
    let res = bike_res(disc);
    let bytes = res.read_suffix(&format!("{A_BIKE}.sub")).expect("the bike archive has no .sub");
    Vehicle::parse(&ini::text(&bytes))
}

/// A chassis with every clip its sixty states name.
fn chassis(disc: &mut Disc, set: &Stunts24, mcf: &str) -> Chassis {
    let res = bike_res(disc);
    match Chassis::load(vec![res], mcf, PREFIX, &set.stunts, params()) {
        Ok(c) => c,
        Err(why) => panic!("the chassis did not load: {why}"),
    }
}

/// A rider on that bike, dropped in from `DROP` so a minute has air and a landing in it.
///
/// The mounts are the **chassis model's own wheel nodes**, which is what the shell hands the
/// simulation (`bike_attach_nodes` looks `TireFront` and `TireRear` up by name and writes each one's
/// place straight into the wheel's `+0xa0`; S22). It matters here more than anywhere: the `.phy`'s
/// `BodyTireForcePosition` is in a different frame and a different scale from the model's nodes, so
/// a fixture that used it would hand the drives a compression and a mount that do not belong to the
/// skeleton they are written onto.
fn ride(p: &Phy, c: &Chassis, set: &Stunts24) -> rider::Rider {
    let at = |name: &str, fallback: Vf| -> Vf {
        match c.node_rest(name) {
            Some(v) => Vf::new(v.x, v.y, v.z, 1.0),
            None => fallback,
        }
    };
    let mounts = (
        at(FRONT, p.front.body_tire_force_position),
        at(REAR, p.rear.body_tire_force_position),
    );
    rider::Rider::new(
        p,
        mounts,
        Vf::new(0.0, DROP, 0.0, 1.0),
        Vf::new(0.0, 0.0, 1.0, 0.0),
        set.clone(),
    )
}

/// One frame of the shell's own loop with the window taken off: step the rider, hand the ride and
/// the trick machine to the chassis.
fn frame(c: &mut Chassis, r: &rider::Rider) -> va::Frame {
    let want = Chassis::want(&r.ride, &r.tricks);
    let request = Chassis::request(&r.ride, &r.tricks);
    c.frame(FRAME, want, &request, Drive::of(&r.ride))
}

/// A node's transform in the `Bike` frame: what the drives are written in, with the root's own
/// transform taken out. Rigid, so the inverse is a transpose.
fn in_root(world: &[M4], root: usize, j: usize) -> M4 {
    let (r, m) = (world[root], world[j]);
    let t = v3(m.0[3][0] - r.0[3][0], m.0[3][1] - r.0[3][1], m.0[3][2] - r.0[3][2]);
    let un = |v: V3| {
        v3(
            v.x * r.0[0][0] + v.y * r.0[0][1] + v.z * r.0[0][2],
            v.x * r.0[1][0] + v.y * r.0[1][1] + v.z * r.0[1][2],
            v.x * r.0[2][0] + v.y * r.0[2][1] + v.z * r.0[2][2],
        )
    };
    let (x, y, z) = (
        un(v3(m.0[0][0], m.0[0][1], m.0[0][2])),
        un(v3(m.0[1][0], m.0[1][1], m.0[1][2])),
        un(v3(m.0[2][0], m.0[2][1], m.0[2][2])),
    );
    let t = un(t);
    M4([
        [x.x, x.y, x.z, 0.0],
        [y.x, y.y, y.z, 0.0],
        [z.x, z.y, z.z, 0.0],
        [t.x, t.y, t.z, 1.0],
    ])
}

/// **V9.** A minute of riding poses the chassis every frame, with a finite transform on every node,
/// and the pose is drawable every frame.
#[test]
fn a_minute_of_riding_poses_the_chassis_every_frame() {
    let Some(mut disc) = disc() else { return };
    let set = stunts(&mut disc);
    let tables = rider::Tables::build();
    let p = phy(&mut disc, &tables);
    let build = sub(&mut disc);
    assert!(!build.mcf.is_empty(), "{A_BIKE}.sub names no animation set");
    let mut c = chassis(&mut disc, &set, &build.mcf);
    let res = bike_res(&mut disc);
    let geometry = Model::parse(&res.read_suffix(&format!("{}.psg", c.geometry)).expect("psg"))
        .expect("the chassis geometry parses");

    let mut r = ride(&p, &c, &set);
    let mut world = Flat::default();
    // A stunt on the way down, then hands on for the rest: between them the machine's gesture arm,
    // its airborne row and its grounded grid are all asked for at least once.
    let (slot, _) = shortest_bike_stunt(&c, &set);
    let gesture = gesture_for(slot);
    let hands = Controls { gas: F::ONE, stick: (F(0.4), F(-0.2)), ..Controls::default() };
    let ask = Controls {
        trick: buttons_for(gesture.0),
        dpad: dpad_for(gesture.1),
        ..Controls::default()
    };

    let joints = c.stance.set.skeleton.len();
    let mut states = std::collections::BTreeSet::new();
    let mut modes = [0usize; 6];
    for k in 0..A_MINUTE {
        r.frame(&p, &tables, if k == 2 { &ask } else { &hands }, FRAME, &mut world);
        let f = frame(&mut c, &r);
        states.insert(c.stance.driver.state);
        modes[f.blend.mode() as usize] += 1;
        assert_eq!(c.world.len(), joints, "a transform per node, frame {k}");
        for (j, m) in c.world.iter().enumerate() {
            assert!(
                m.0.iter().flatten().all(|v| v.is_finite()),
                "node {j} ({}) is not finite on frame {k}: {m:?}",
                c.stance.set.skeleton.bones[j].name
            );
        }
        // Skinning every frame is the shell's job; a tenth of a second apart proves the triangles
        // come out of it without making the test a renderer benchmark.
        if k % 6 == 0 {
            let tris = skin(&geometry, &c.world, 0);
            assert!(!tris.is_empty(), "the chassis skinned to nothing on frame {k}");
        }
    }
    assert!(c.stance.posed, "no state of the minute found a clip to pose the chassis with");
    println!(
        "a minute of riding: {A_MINUTE} frames, states {states:?}, blend modes \
         free/one/two/_/four/pair {modes:?}"
    );
}

/// **V9.** The fork node travels with the ported compression.
///
/// The drive is `shocks_rest + fork * (inv_sin * compression)` plus a fixed nudge, worked out in the
/// `Bike` frame and then re-expressed in the steering head's own frame, because that is the node's
/// parent. So what this checks is the round trip: the `Shocks` node's place in the `Bike` frame, as
/// the composed tree leaves it, against the expression the drive is written from — through a drop and
/// a landing, which is where the suspension actually works.
#[test]
fn the_fork_node_travels_with_the_ported_compression() {
    let Some(mut disc) = disc() else { return };
    let set = stunts(&mut disc);
    let tables = rider::Tables::build();
    let p = phy(&mut disc, &tables);
    let build = sub(&mut disc);
    let mut c = chassis(&mut disc, &set, &build.mcf);

    let root = c.stance.set.skeleton.find(ROOT).expect("the chassis has a Bike node");
    let shocks = c.stance.set.skeleton.find(SHOCKS).expect("the chassis has a Shocks node");
    let bars = c.stance.set.skeleton.find(BARS).expect("the chassis has a Handlebars node");
    assert_eq!(c.stance.set.skeleton.bones[shocks].parent, Some(bars), "Shocks hangs off Handlebars");

    let mut r = ride(&p, &c, &set);
    let mut world = Flat::default();
    let hands = Controls { gas: F::ONE, ..Controls::default() };
    let (mut lowest, mut highest) = (f32::MAX, f32::MIN);
    let mut worst = 0.0f32;
    for k in 0..A_MINUTE {
        r.frame(&p, &tables, &hands, FRAME, &mut world);
        let compression = r.ride.front.w.compression.0;
        let _ = frame(&mut c, &r);
        let at = in_root(&c.world, root, shocks);
        let got = v3(at.0[3][0], at.0[3][1], at.0[3][2]);
        // The drive's own expression, spelled out here rather than called, so that this is a check
        // and not a tautology.
        let want = c
            .shape
            .shocks_rest
            .add(c.shape.fork.scale(c.shape.inv_sin * compression))
            .add(v3(-0.03, 0.0, 0.02));
        worst = worst.max(got.sub(want).len());
        let along = got.sub(c.shape.shocks_rest).dot(c.shape.fork);
        lowest = lowest.min(along);
        highest = highest.max(along);
        assert!(
            got.sub(want).len() < 1e-3,
            "frame {k}: the fork is at {got:?} and the compression of {compression} wants {want:?}"
        );
    }
    let travel = highest - lowest;
    println!(
        "the fork: inv_sin {:.4} on an axis of {:?}, {travel:.4} of travel along it over the \
         minute (model units, the frame the mounts are in), worst round trip {worst:.2e}",
        c.shape.inv_sin, c.shape.fork
    );
    assert!(travel > 0.01, "the suspension never moved: {travel} of travel");
}

/// **V9.** The wheel node's angle integrates its own spin.
///
/// Neither half of this is in a clip. The rate is the wheel's own (`+0x1d4`, which S16's
/// integration writes) and the angle is the renderer's integral of it, because nothing in the
/// simulation reads one. Two things are checked: the integral against an independent one run here,
/// and the *node* against the integral — a point at the top of the tire, taken through the composed
/// tree and back into the tire's parent frame, has to have gone round by exactly that angle.
#[test]
fn the_wheel_node_angle_integrates_its_own_spin() {
    let Some(mut disc) = disc() else { return };
    let set = stunts(&mut disc);
    let tables = rider::Tables::build();
    let p = phy(&mut disc, &tables);
    let build = sub(&mut disc);
    let mut c = chassis(&mut disc, &set, &build.mcf);

    let wheels = [
        (FRONT, SHOCKS, c.stance.set.skeleton.find(FRONT), c.stance.set.skeleton.find(SHOCKS)),
        (REAR, SWINGARM, c.stance.set.skeleton.find(REAR), c.stance.set.skeleton.find(SWINGARM)),
    ];
    for (tire, parent, j, k) in wheels {
        assert!(j.is_some() && k.is_some(), "the chassis has a {tire} under a {parent}");
        assert_eq!(c.stance.set.skeleton.bones[j.unwrap()].parent, k, "{tire} hangs off {parent}");
    }

    let mut r = ride(&p, &c, &set);
    let mut world = Flat::default();
    let hands = Controls { gas: F::ONE, ..Controls::default() };
    // The reference integration: the original's own wrap, run here off the same rate.
    let mut reference = [0.0f32; 2];
    let mut turned = [0.0f32; 2];
    for n in 0..A_MINUTE {
        r.frame(&p, &tables, &hands, FRAME, &mut world);
        let spin = [r.ride.front.mv.spin.0, r.ride.rear.mv.spin.0];
        let _ = frame(&mut c, &r);
        for (w, rate) in spin.iter().enumerate() {
            turned[w] += rate.abs() * FRAME.0;
            let mut a = reference[w] + rate * FRAME.0;
            a -= (a * 0.15915494).trunc() * 6.2831855;
            if a > std::f32::consts::PI {
                a -= 6.2831855;
            } else if a < -std::f32::consts::PI {
                a += 6.2831855;
            }
            reference[w] = a;
        }
        let got = [c.front_roll.angle, c.rear_roll.angle];
        for w in 0..2 {
            assert!(
                (got[w] - reference[w]).abs() < 1e-4,
                "wheel {w} is at {} on frame {n} and its own spin wants {}",
                got[w],
                reference[w]
            );
        }
        // And the node itself: the top of the tire, in the frame of the node it hangs off.
        for (w, (tire, _, j, k)) in wheels.iter().enumerate() {
            let (j, k) = (j.unwrap(), k.unwrap());
            let up = c.world[j].dir(v3(0.0, 1.0, 0.0));
            let parent = c.world[k];
            let dir = v3(
                up.x * parent.0[0][0] + up.y * parent.0[0][1] + up.z * parent.0[0][2],
                up.x * parent.0[1][0] + up.y * parent.0[1][1] + up.z * parent.0[1][2],
                up.x * parent.0[2][0] + up.y * parent.0[2][1] + up.z * parent.0[2][2],
            );
            let (s, cos) = got[w].sin_cos();
            let want = v3(0.0, cos, s);
            assert!(
                dir.sub(want).len() < 1e-3,
                "frame {n}: the top of {tire} points {dir:?} and an angle of {} wants {want:?}",
                got[w]
            );
        }
    }
    println!(
        "the wheels: {:.1} and {:.1} turns over the minute, ending at {:.3} and {:.3} rad",
        turned[0] / 6.2831855,
        turned[1] / 6.2831855,
        c.front_roll.angle,
        c.rear_roll.angle
    );
    assert!(turned[0] > 6.2831855, "the front wheel never turned: {} rad", turned[0]);
    assert!(turned[1] > 6.2831855, "the rear wheel never turned: {} rad", turned[1]);
}

/// **V9.** A stunt looks like the stunt it is on the bike too: its first frame and its last are
/// different poses of the chassis.
///
/// What is compared is the *clip's* pose — the chassis skeleton under
/// `game_render::anim::Set::pose` alone, with the drives left out — because the drives move the
/// suspension and the wheels every frame regardless and would make this pass on a bike whose every
/// clip was missing.
#[test]
fn a_stunts_first_and_last_frames_differ_in_the_bikes_pose() {
    let Some(mut disc) = disc() else { return };
    let set = stunts(&mut disc);
    let tables = rider::Tables::build();
    let p = phy(&mut disc, &tables);
    let build = sub(&mut disc);
    let mut c = chassis(&mut disc, &set, &build.mcf);

    let (slot, stunt) = shortest_bike_stunt(&c, &set);
    let gesture = gesture_for(slot);
    let mut r = ride(&p, &c, &set);
    let mut world = Flat::default();

    let posed = |c: &Chassis| c.stance.set.skeleton.world(&c.stance.set.pose);
    let (mut first, mut last) = (None, None);
    let mut frames = 0usize;
    let mut modes = std::collections::BTreeSet::new();
    let mut states = std::collections::BTreeSet::new();
    for k in 0..600 {
        let hands = if k == 2 {
            Controls {
                trick: buttons_for(gesture.0),
                dpad: dpad_for(gesture.1),
                ..Controls::default()
            }
        } else {
            Controls::default()
        };
        let said = r.frame(&p, &tables, &hands, FRAME, &mut world);
        let f = frame(&mut c, &r);
        if said.started == Some(Trick(slot as u8)) {
            first = Some(posed(&c));
        }
        if first.is_some() && r.tricks.in_progress {
            modes.insert(f.blend.mode());
            states.insert(c.stance.driver.state);
            last = Some(posed(&c));
            frames += 1;
        }
        if first.is_some() && !r.tricks.in_progress {
            break;
        }
    }
    let (first, last) = (first.expect("the gesture started the stunt"), last.expect("it ran"));
    let apart = furthest(&first, &last);
    println!(
        "{:?} on the bike ({:.3}s, {frames} frames, states {states:?}, blend modes {modes:?}): \
         the furthest node moved {apart:.3} between its first frame and its last",
        stunt.name, stunt.duration.0
    );
    assert_eq!(
        states,
        [slot as i32].into_iter().collect(),
        "the machine ran the gesture slot the trick is in and nothing else"
    );
    assert!(apart > 0.01, "{:?} drew the same chassis pose at both ends: {apart}", stunt.name);
}

/// The furthest any node moved between two frames.
fn furthest(a: &[M4], b: &[M4]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| {
            let (x, y) = (x.0[3], y.0[3]);
            ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt()
        })
        .fold(0.0, f32::max)
}

/// The shortest stunt the *bike* has a clip for, so that the gesture arm has something to play.
/// `UsedForBike` is the index of the unlisted set and this asks the bound table instead, which is
/// the same thing from the other side.
fn shortest_bike_stunt(c: &Chassis, set: &Stunts24) -> (usize, game_sim::trick::Stunt) {
    set.stunts
        .iter()
        .enumerate()
        .filter(|(k, s)| {
            *k != set.no_hander
                && *k != set.throwaway
                && s.duration > F::ZERO
                && *k < SLOTS
                && c.stance.lib.clip[*k].is_some()
        })
        .min_by(|a, b| a.1.duration.0.total_cmp(&b.1.duration.0))
        .map(|(k, s)| (k, s.clone()))
        .expect("the bike has a stunt clip")
}

/// The buttons that make a group, in the order the trick decoder reads them.
fn buttons_for(group: u8) -> [bool; 3] {
    match group {
        1 => [true, false, false],
        2 => [false, true, false],
        3 => [true, true, false],
        4 => [true, false, true],
        5 => [false, true, true],
        6 => [true, true, true],
        _ => [false, false, false],
    }
}

/// The d-pad that names a direction, in the order the decoder polls it: left, right, up, down.
fn dpad_for(dir: Dir) -> [bool; 4] {
    let mut out = [false; 4];
    match dir {
        Dir::Left => out[0] = true,
        Dir::Right => out[1] = true,
        Dir::Up => out[2] = true,
        Dir::Down => out[3] = true,
        Dir::None => {}
    }
    out
}

/// The gesture that names a slot: the first group-and-direction pair that maps to it.
fn gesture_for(slot: usize) -> (u8, Dir) {
    for group in 1..=6u8 {
        for dir in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
            if slot_of(group, dir) == Some(slot) {
                return (group, dir);
            }
        }
    }
    panic!("no gesture names slot {slot}");
}

/// A compile-time reminder that the machine's states are the ones the port reads.
const _: () = {
    assert!(va::AIR == 35 && va::TURN_RIDING == 30);
};

// ---------------------------------------------------------------------------------------------
// V13: the other side of the pairing — the sixty-state machine's own skeleton.
// ---------------------------------------------------------------------------------------------

/// The stock gear, and the body V12's capture watched the sixty-state step run on.
const A_RIDER: &str = "thq1";
/// What the *body's* loader puts in front of a gesture slot's stunt name. The race copy of the set;
/// the freestyle one is `ff_rider_` out of the same archive.
const RIDER_PREFIX: &str = "rider_";
/// The directory a rider's `.mcf` names, which is where the sixty clips come from.
const RIDER_ANIM: &str = "rider\\anim\\";
/// How many joints the body has.
const RIDER_JOINTS: usize = 22;

/// One copy of the rider's archive, opened the same way the bike's is.
fn rider_res(disc: &mut Disc) -> Res {
    let name = member(disc, &format!("{A_RIDER}.res")).expect("the disc has no rider archive");
    disc.res(&name).expect("the rider archive does not open")
}

/// The rider's body with the sixty-state machine bound to it: the pairing V12's capture proves,
/// built out of the same pieces [`Chassis::load`] builds the bike's from.
fn rider_stance(disc: &mut Disc, set: &Stunts24) -> Stance {
    let chain = rider_res(disc);
    let gear = gear::Rider::load(&[chain], A_RIDER, gear::Sex::Male).expect("the rider loads");
    let res = rider_res(disc);
    let anim = match Set::load(vec![res], &gear.mcf) {
        Ok(s) => s,
        Err(why) => panic!("the rider's animation set did not load: {why}"),
    };
    Stance::new(anim, RIDER_PREFIX, &set.stunts, params())
}

/// **V13.** The sixty-state machine's clip table is the *rider's*: every one of its sixty slots
/// binds against the body's set, and the bike's set — paired with it until now — has twelve.
///
/// This is V12's capture restated as a test. The 24 gesture slots come out prefixed and the 36 pose
/// names bare, which is the loader's own split, and the clips land on a 22-joint skeleton out of
/// `Rider\Anim\`.
#[test]
fn the_sixty_state_tables_clips_are_the_riders() {
    let Some(mut disc) = disc() else { return };
    let set = stunts(&mut disc);
    let body = rider_stance(&mut disc, &set);

    assert_eq!(body.set.skeleton.len(), RIDER_JOINTS, "the body's joints");
    assert_eq!(
        body.set.set.directory.to_ascii_lowercase(),
        RIDER_ANIM,
        "the set's clips come out of the rider's own directory"
    );
    assert_eq!(body.bound_slots(), va::STATES, "every state of the machine found its clip");

    // The loader's split, slot by slot: prefixed stunt names in the gesture slots, bare pose names
    // after them, and each clip's channels landing on joints of the skeleton it was bound to.
    for (slot, id) in body.lib.clip.iter().enumerate() {
        let id = id.unwrap_or_else(|| panic!("state {slot} bound no clip"));
        let bound = body.set.get(id).expect("a bound clip is in the set");
        let want = if slot < SLOTS {
            va::Library::stunt_name(RIDER_PREFIX, &set.stunts[slot].name)
        } else {
            va::POSE_CLIPS[slot - SLOTS].to_string()
        };
        assert_eq!(bound.name, want, "state {slot}");
        assert!(bound.named() > 0, "state {slot} ({want}) is a clip for another skeleton");
    }

    // And the other side of the swap: the bike's set, asked for the same sixty, answers for the
    // twelve V9 found — its 9 `UsedForBike` gestures and the three airborne poses — and for none of
    // the nine rows of the grounded grid, which is why V9's grid drew nothing.
    let build = sub(&mut disc);
    let bike = chassis(&mut disc, &set, &build.mcf);
    let grid: Vec<usize> = (va::TURN_STOP..va::TURN_RIDING + 3).map(|s| s as usize).collect();
    assert!(
        grid.iter().all(|&s| bike.stance.lib.clip[s].is_none()),
        "a bike archive has none of the grounded grid"
    );
    assert!(
        grid.iter().all(|&s| body.lib.clip[s].is_some()),
        "the rider's set has every row of the grounded grid"
    );
    // What the bike's set *does* answer for, by name, so the twelve clips V9 found are accounted
    // for against the sixty slots asked of them.
    let on_the_bike: Vec<String> = bike
        .stance
        .lib
        .clip
        .iter()
        .enumerate()
        .filter_map(|(slot, id)| id.map(|id| (slot, id)))
        .map(|(slot, id)| format!("{slot}:{}", bike.stance.set.get(id).unwrap().name))
        .collect();
    println!(
        "the sixty-state table: {} of {} slots on the rider's {} joints out of {:?}; {} of {} on \
         the bike's {} joints out of {:?} — {}",
        body.bound_slots(),
        va::STATES,
        body.set.skeleton.len(),
        body.set.set.directory,
        bike.stance.bound_slots(),
        va::STATES,
        bike.stance.set.skeleton.len(),
        bike.stance.set.set.directory,
        on_the_bike.join(" ")
    );
    println!(
        "the bike's set holds {} clips: {}",
        bike.stance.set.clips.len(),
        bike.stance.set.clips.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(" ")
    );
}

/// **V13.** A minute of riding poses the rider's own skeleton from the machine, every frame — and
/// the same minute on the bike's set leaves frames with nothing to draw.
///
/// The wants and the request block are the shell's own reading off the ride ([`Chassis::want`]), so
/// both sets are asked for exactly the same states in the same order: the only difference is which
/// skeleton and which clips are on the other end of the machine.
#[test]
fn a_minute_of_riding_poses_the_riders_own_skeleton() {
    let Some(mut disc) = disc() else { return };
    let set = stunts(&mut disc);
    let tables = rider::Tables::build();
    let p = phy(&mut disc, &tables);
    let build = sub(&mut disc);
    let mut c = chassis(&mut disc, &set, &build.mcf);
    let mut body = rider_stance(&mut disc, &set);

    let mut r = ride(&p, &c, &set);
    let mut world = Flat::default();
    // The same fixture V9's minute rides: a stunt on the way down, then hands on, so the gesture
    // arm, the airborne row and the grounded grid are all asked for.
    let (slot, _) = shortest_bike_stunt(&c, &set);
    let gesture = gesture_for(slot);
    let hands = Controls { gas: F::ONE, stick: (F(0.4), F(-0.2)), ..Controls::default() };
    let ask = Controls {
        trick: buttons_for(gesture.0),
        dpad: dpad_for(gesture.1),
        ..Controls::default()
    };

    let joints = body.set.skeleton.len();
    let mut states = std::collections::BTreeSet::new();
    let mut modes = [0usize; 6];
    let (mut rider_blank, mut bike_blank) = (0usize, 0usize);
    // The number the grid cuts up, in feet per second: the thresholds are 5, 10 and 15 mph.
    let mut fastest = f32::MIN;
    let mut first = Vec::new();
    let mut last = Vec::new();
    for k in 0..A_MINUTE {
        r.frame(&p, &tables, if k == 2 { &ask } else { &hands }, FRAME, &mut world);
        let want = Chassis::want(&r.ride, &r.tricks);
        let request = Chassis::request(&r.ride, &r.tricks);
        fastest = fastest.max(request.band.0);
        let f = body.frame(FRAME, want, &request);
        let _ = c.frame(FRAME, want, &request, Drive::of(&r.ride));
        states.insert(body.driver.state);
        modes[f.blend.mode() as usize] += 1;
        rider_blank += usize::from(!body.drawn);
        bike_blank += usize::from(!c.stance.drawn);

        let posed = body.world();
        assert_eq!(posed.len(), joints, "a transform per joint, frame {k}");
        for (j, m) in posed.iter().enumerate() {
            assert!(
                m.0.iter().flatten().all(|v| v.is_finite()),
                "joint {j} ({}) is not finite on frame {k}: {m:?}",
                body.set.skeleton.bones[j].name
            );
        }
        if k == 0 {
            first = posed.clone();
        }
        last = posed;
    }

    println!(
        "a minute on the rider: {A_MINUTE} frames, states {states:?}, blend modes \
         free/one/two/_/four/pair {modes:?}, {rider_blank} frames with nothing to draw on the \
         rider's set and {bike_blank} on the bike's, fastest band {fastest:.3} ft/s"
    );
    assert!(body.posed, "the machine never posed the body");
    assert_eq!(rider_blank, 0, "the rider's set had nothing to draw on {rider_blank} frames");
    assert!(
        bike_blank > 0,
        "the bike's set drew every frame of the minute, so this proves nothing about the pairing"
    );
    assert!(states.contains(&va::TURN_RIDING), "the grounded grid never ran");
    assert!(modes[4] > 0, "the grid's interpolating bands never ran");
    let apart = furthest(&first, &last);
    assert!(apart > 0.01, "the body held one pose for the whole minute: {apart}");
}

/// **V13.** Every row of the grounded pose grid draws on the rider's set, across all four regions
/// the three thresholds cut the band into and from one end of the fore/aft axis to the other.
///
/// V9's minute could not reach this: a bike archive has none of the nine `turn_*` clips, and the
/// one fixture that rides never leaves the lowest band anyway (5.3 ft/s of 7.3333). So the band and
/// the mix axis are driven here instead of ridden — the request block is the step's only input for
/// both — and what is checked is that each setting finds its clips and that the grid actually moves
/// the body: between the two ends of a row, and between one row and the next.
#[test]
fn the_grounded_pose_grid_draws_across_every_band() {
    let Some(mut disc) = disc() else { return };
    let set = stunts(&mut disc);
    let mut body = rider_stance(&mut disc, &set);

    // Two in each of the four regions the thresholds make: under 5 mph, 5 to 10, 10 to 15, over 15.
    let bands = [F(0.0), F(5.0), F(7.3333), F(12.0), F(14.6666), F(18.0), F(22.0), F(30.0)];
    let ends = [F::ZERO, F(0.5), F::ONE];
    // Half a second at each setting, which is four time constants of the slower axis.
    const SETTLE: usize = 30;

    let mut modes = std::collections::BTreeSet::new();
    let mut poses = std::collections::BTreeMap::new();
    for (b, band) in bands.iter().enumerate() {
        for (e, axis) in ends.iter().enumerate() {
            let r = va::Request {
                band: *band,
                mix_axis: *axis,
                phase_axis: F(0.5),
                seq: 1,
                ..va::Request::default()
            };
            for _ in 0..SETTLE {
                let f = body.frame(FRAME, va::TURN_RIDING, &r);
                modes.insert(f.blend.mode());
                assert!(
                    body.drawn,
                    "the grid drew nothing at {} ft/s with the axis at {}",
                    band.0, axis.0
                );
            }
            poses.insert((b, e), body.world());
        }
    }

    let across = furthest(&poses[&(0, 0)], &poses[&(0, 2)]);
    let rows = furthest(&poses[&(0, 1)], &poses[&(3, 1)]);
    println!(
        "the grounded grid on the rider: {} settings drawn, blend modes {modes:?}, the furthest \
         joint moves {across:.3} across a row and {rows:.3} between the first band's row and the \
         third's",
        poses.len()
    );
    assert_eq!(
        modes,
        [2u8, 4].into_iter().collect(),
        "the grid's two arms are the interpolating one and the single pair"
    );
    assert!(across > 0.01, "the fore/aft axis does not move the body: {across}");
    assert!(rows > 0.01, "the band does not move the body between rows: {rows}");
}
