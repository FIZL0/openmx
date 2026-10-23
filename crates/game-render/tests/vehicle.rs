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
use game_render::vehicle::{BLUR_ALPHA, BLUR_ARC, BLUR_FROM, BLUR_FULL, SPIN_STEP_CAP, Spin, about};
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
/// and the *node* against the angle it is drawn at — a point at the top of the tire, taken through
/// the composed tree and back into the tire's parent frame, has to have gone round by exactly that
/// angle. Since V59 the drawn angle is [`Spin`]'s, the same steps held under [`SPIN_STEP_CAP`], so
/// that is integrated here too.
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
    let mut drawn = [0.0f32; 2];
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
            let step = (rate * FRAME.0).clamp(-SPIN_STEP_CAP, SPIN_STEP_CAP);
            drawn[w] = wrap(drawn[w] + step);
        }
        let got = [c.front_roll.angle, c.rear_roll.angle];
        let shown = [c.front_spin.angle, c.rear_spin.angle];
        for w in 0..2 {
            assert!(
                (got[w] - reference[w]).abs() < 1e-4,
                "wheel {w} is at {} on frame {n} and its own spin wants {}",
                got[w],
                reference[w]
            );
            assert!(
                wrap(shown[w] - drawn[w]).abs() < 1e-3,
                "wheel {w} is drawn at {} on frame {n} and its capped steps want {}",
                shown[w],
                drawn[w]
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
            let (s, cos) = shown[w].sin_cos();
            let want = v3(0.0, cos, s);
            assert!(
                dir.sub(want).len() < 1e-3,
                "frame {n}: the top of {tire} points {dir:?} and an angle of {} wants {want:?}",
                shown[w]
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
/// the body: between the two ends of a row, and between one row and the next. The one row the
/// fore/aft axis cannot move is the stopped one, whose three columns V14 found aliased to a single
/// `.MOT` on this disc; that is asserted too.
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

    // Across a row, measured on a row whose three columns really are three clips. The lowest
    // setting is not one: at a band of zero `w3` is one, so the grid plays the `turn_stop` row
    // alone — and V14 measured that the `.mcf` whose claims win on this disc aliases *both*
    // `turn_stop_Fwd` and `turn_stop_Bck` to `turn_stop_Mid.MOT`, so all three of that row's
    // columns are the same clip and the axis cannot move it. That is the original's own behaviour
    // (V14's variant B, which left those two columns spelled out, missed the capture by 3.6e-4 in
    // this exact arm), so it is asserted here rather than worked around.
    let across = furthest(&poses[&(3, 0)], &poses[&(3, 2)]);
    let across_riding = furthest(&poses[&(7, 0)], &poses[&(7, 2)]);
    let stopped = furthest(&poses[&(0, 0)], &poses[&(0, 2)]);
    let rows = furthest(&poses[&(0, 1)], &poses[&(3, 1)]);
    println!(
        "the grounded grid on the rider: {} settings drawn, blend modes {modes:?}, the furthest \
         joint moves {across:.3} across the standing-still row and {across_riding:.3} across the \
         riding row, {stopped:.2e} across the stopped row (whose three columns are one clip), and \
         {rows:.3} between the first band's row and the third's",
        poses.len()
    );
    assert_eq!(
        modes,
        [2u8, 4].into_iter().collect(),
        "the grid's two arms are the interpolating one and the single pair"
    );
    assert!(across > 0.01, "the fore/aft axis does not move the body: {across}");
    assert!(
        across_riding > 0.01,
        "the fore/aft axis does not move the body on the riding row: {across_riding}"
    );
    assert!(
        stopped < 1e-5,
        "the stopped row's three columns all resolve to turn_stop_Mid.MOT on this disc, so the \
         fore/aft axis has nothing to move the body between: {stopped}"
    );
    assert!(rows > 0.01, "the band does not move the body between rows: {rows}");
}

/// **P29.** A hands-off flight draws the bike as square as the solver flies it.
///
/// The ride is dropped from [`DROP`] with nobody on the controls, so the solver's chassis stays level
/// all the way down: the auto-level has nothing to correct. What the shell draws on top of that is the
/// chassis' pose, and in the air the airborne row plays the `bike_air_*` sweep at the request's phase
/// axis. This steps two chassis through the same flight — one from [`Chassis::request`], whose axes
/// are zero, and one from [`Chassis::riding`], whose axes are the rider's own — and measures how far
/// the `Bike` node is turned off its rest, which is the twist the screen shows on top of the solver.
///
/// The first is the shell before P29 and is asserted to twist, so that the second passing means
/// something: the same flight, the same frames, and only the two axes differ.
#[test]
fn a_hands_off_flight_draws_the_bike_square() {
    let Some(mut disc) = disc() else { return };
    let set = stunts(&mut disc);
    let tables = rider::Tables::build();
    let p = phy(&mut disc, &tables);
    let build = sub(&mut disc);
    let mut zeroed = chassis(&mut disc, &set, &build.mcf);
    let mut riding = chassis(&mut disc, &set, &build.mcf);
    let root = riding.stance.set.skeleton.find(ROOT).expect("the chassis has a Bike node");

    let mut r = ride(&p, &riding, &set);
    let mut world = Flat::default();
    // How far a node's axes are turned from the identity, in degrees: the rotation angle of its
    // upper 3x3, which counts a yaw and a roll alike.
    let turned = |m: &M4| -> f32 {
        let len = |i: usize| (m.0[i][0] * m.0[i][0] + m.0[i][1] * m.0[i][1] + m.0[i][2] * m.0[i][2]).sqrt();
        let trace = m.0[0][0] / len(0) + m.0[1][1] / len(1) + m.0[2][2] / len(2);
        ((trace - 1.0) / 2.0).clamp(-1.0, 1.0).acos().to_degrees()
    };
    let (mut air, mut most_zeroed, mut most_riding) = (0usize, 0f32, 0f32);
    for k in 0..240 {
        r.frame(&p, &tables, &Controls::default(), FRAME, &mut world);
        let want = Chassis::want(&r.ride, &r.tricks);
        let d = Drive::of(&r.ride);
        zeroed.frame(FRAME, want, &Chassis::request(&r.ride, &r.tricks), d);
        riding.frame(FRAME, want, &Chassis::riding(&r), d);
        if want != va::AIR {
            continue;
        }
        air += 1;
        let (a, b) = (turned(&zeroed.world[root]), turned(&riding.world[root]));
        most_zeroed = most_zeroed.max(a);
        most_riding = most_riding.max(b);
        if k % 10 == 0 {
            println!(
                "frame {k:3}: shown crossup {:.4} fore-aft {:.4}; the Bike node turned {a:6.2} deg \
                 at zero axes, {b:6.2} deg at the rider's",
                r.motion.pose.0, r.motion.fore_aft.0
            );
        }
    }
    println!(
        "a hands-off drop: {air} airborne frames; the Bike node turned at most {most_zeroed:.2} deg \
         from zero axes and {most_riding:.2} deg from the rider's"
    );
    assert!(air > 30, "the drop was not a flight: {air} airborne frames");
    // Measured: 61.78 deg at zero axes (the sweep's far end, reached about half a second into the
    // flight) against 1.56 deg at the rider's, which is `bike_air_Mid`'s own pose at its midpoint.
    assert!(
        most_zeroed > 30.0,
        "zero axes no longer cross the bike up, so this test no longer shows what P29 fixed: \
         {most_zeroed} deg"
    );
    assert!(
        most_riding < 3.0,
        "the chassis is drawn turned {most_riding} deg off a level hands-off flight"
    );
}

/// **S55.** A crossed-up flight lands and the chassis eases square rather than snapping. In the air the
/// `Bike` node is twisted by `bike_air`'s crossup sweep; on touching down the machine restarts into
/// a grounded state, whose blend has no clip for the chassis and fades out of the snapshot over the
/// restart's ramp. The port used to draw the rest shape at once and throw the fade away, so the bike
/// (and the rider on it) turned 45° in one frame on landing — the snap the user saw.
///
/// Matched by eye, not to a capture: the original's drawn chassis is not in RAM where P29 looked.
#[test]
fn a_crossed_up_landing_eases_the_bike_square() {
    let Some(mut disc) = disc() else { return };
    let set = stunts(&mut disc);
    let tables = rider::Tables::build();
    let p = phy(&mut disc, &tables);
    let build = sub(&mut disc);
    let mut riding = chassis(&mut disc, &set, &build.mcf);
    let root = riding.stance.set.skeleton.find(ROOT).expect("the chassis has a Bike node");
    let mut r = ride(&p, &riding, &set);
    let mut world = Flat::default();
    let turned = |m: &M4| -> f32 {
        let len = |i: usize| (m.0[i][0] * m.0[i][0] + m.0[i][1] * m.0[i][1] + m.0[i][2] * m.0[i][2]).sqrt();
        let trace = m.0[0][0] / len(0) + m.0[1][1] / len(1) + m.0[2][2] / len(2);
        ((trace - 1.0) / 2.0).clamp(-1.0, 1.0).acos().to_degrees()
    };
    // Thrown forward and up, the stick held across the whole flight to cross it up.
    let c = Controls { stick: (F(0.6), F::ZERO), ..Controls::default() };
    let fwd = r.ride.axes().0;
    r.ride.chassis.vel = Vf::new(fwd.x() * 50.0, 15.0, fwd.z() * 50.0, 0.0);
    let (mut flew, mut landed) = (0usize, None);
    let mut shown = Vec::new();
    for _ in 0..200 {
        r.frame(&p, &tables, &c, FRAME, &mut world);
        let want = Chassis::want(&r.ride, &r.tricks);
        riding.frame(FRAME, want, &Chassis::riding(&r), Drive::of(&r.ride));
        let on = r.ride.air.on_ground;
        if !on && landed.is_none() {
            flew += 1;
        }
        if on && flew > 30 && landed.is_none() {
            landed = Some(shown.len());
        }
        shown.push(turned(&riding.world[root]));
    }
    let landed = landed.expect("the throw never came down");
    let at = shown[landed - 1];
    let after = &shown[landed - 1..(landed + 30).min(shown.len())];
    let step = after.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0f32, f32::max);
    println!(
        "{flew} airborne frames; the Bike node turned {at:.2} deg on the last of them, then {:?}",
        after.iter().map(|a| format!("{a:.2}")).collect::<Vec<_>>()
    );
    // Measured: 45.16 deg, then 3.01 deg a frame down to 0 over the restart's 14-frame ramp.
    assert!(at > 30.0, "the flight was not crossed up, so this proves nothing: {at} deg");
    assert!(step < 5.0, "the chassis turned {step} deg in one frame after landing");
    assert!(*after.last().unwrap() < 1.0, "the chassis never came square: {after:?}");
}

/// An angle put back into `-pi..pi`.
fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// The turn from `a` to `b` about `axis`, signed the way [`about`] turns (`y` towards `z` about `x`).
fn turn_about(axis: V3, a: V3, b: V3) -> f32 {
    axis.dot(a.cross(b)).atan2(a.dot(b))
}

/// The chassis' finest triangles as the pose has them, kept with the wheel each was skinned from.
fn wheel_tris(geometry: &Model, c: &Chassis, pose: &[M4], wheel: u8) -> Vec<game_render::psg::Tri> {
    skin(geometry, pose, 0)
        .into_iter()
        .filter(|t| t.v.iter().all(|v| c.wheels.get(v.node) == Some(&wheel)))
        .collect()
}

fn area(t: &game_render::psg::Tri) -> f32 {
    t.v[1].pos.sub(t.v[0].pos).cross(t.v[2].pos.sub(t.v[0].pos)).len() * 0.5
}

/// **V59.** Past the alias rate a wheel is drawn turning forward by under half a turn a tick, and
/// blurred.
///
/// The ride is 30 ticks a second and the picture is blended between two of them the short way
/// round, so a wheel that turns more than half a turn in a tick is drawn turning backwards, and one
/// of thin spokes strobes long before that. Every rate either way, at the ride's tick and at the
/// window's: the drawn step keeps the rate's sign, is the rate's own under the cap and never more
/// than [`SPIN_STEP_CAP`] (a fifth of a turn) past it, and wherever the cap holds a step back the
/// wheel is wholly blurred.
#[test]
fn past_the_alias_rate_the_wheel_turns_forward_and_blurs() {
    assert!(SPIN_STEP_CAP < std::f32::consts::PI * 0.5, "the cap is under a quarter turn");
    for dt in [1.0 / 30.0, 1.0 / 60.0] {
        assert!(BLUR_FULL * dt < SPIN_STEP_CAP, "the blur is whole before the cap bites at dt {dt}");
        for k in -1200..=1200 {
            let rate = k as f32 * 0.25;
            let mut s = Spin { angle: 2.9, ..Spin::default() };
            let before = s.angle;
            let after = s.step(dt, rate);
            assert!((-std::f32::consts::PI..=std::f32::consts::PI).contains(&after), "{after} wrapped");
            let step = wrap(after - before);
            assert!(step.abs() <= SPIN_STEP_CAP + 1e-5, "rate {rate} at dt {dt} drew a step of {step}");
            assert!(step.abs() < std::f32::consts::PI, "half a turn or more at rate {rate}");
            if rate != 0.0 {
                assert_eq!(step.signum(), rate.signum(), "rate {rate} at dt {dt} turned the wheel backwards");
            }
            if (rate * dt).abs() <= SPIN_STEP_CAP {
                assert!((step - rate * dt).abs() < 1e-4, "rate {rate} at dt {dt}: {step}, not its own step");
            } else {
                assert_eq!(s.blur, 1.0, "rate {rate} is capped at dt {dt} and not wholly blurred");
            }
            if rate.abs() <= BLUR_FROM {
                assert_eq!(s.blur, 0.0, "a slow wheel at {rate} is blurred");
            }
            if rate.abs() >= BLUR_FULL {
                assert_eq!(s.blur, 1.0, "a fast wheel at {rate} is not wholly blurred");
            }
            assert_eq!(s.dir, if rate < 0.0 { -1.0 } else { 1.0 });
        }
    }
    let mut s = Spin::default();
    s.step(1.0 / 30.0, f32::NAN);
    assert_eq!(s, Spin::default(), "a rate that is not a number leaves the wheel where it was");
}

/// **V59.** The blur is the wheel alone, turned back along its trail.
///
/// On the disc's own chassis, spun past the blur: [`BLUR_ALPHA`]'s copies a wheel, at those alphas;
/// skinned, every triangle with any area in a copy is that wheel's; and that wheel's every vertex
/// is the drawn one's, turned about the axle (the tire node's own `x`) the way it came from by its
/// share of [`BLUR_ARC`]. A slow wheel and the original's wheels draw no copies, and the original's
/// are drawn at [`Roll`](game_render::vehicle::Roll)'s uncapped angle.
#[test]
fn a_spinning_wheel_blurs_alone_about_its_axle() {
    let Some(mut disc) = disc() else { return };
    let set = stunts(&mut disc);
    let tables = rider::Tables::build();
    let p = phy(&mut disc, &tables);
    let build = sub(&mut disc);
    let mut c = chassis(&mut disc, &set, &build.mcf);
    let res = bike_res(&mut disc);
    let geometry = Model::parse(&res.read_suffix(&format!("{}.psg", c.geometry)).expect("psg"))
        .expect("the chassis geometry parses");
    let front = c.stance.set.skeleton.find(FRONT).expect("a TireFront");
    let rear = c.stance.set.skeleton.find(REAR).expect("a TireRear");

    let mut r = ride(&p, &c, &set);
    let mut world = Flat::default();
    let hands = Controls { gas: F::ONE, ..Controls::default() };
    let spun = |c: &mut Chassis, r: &rider::Rider, front: f32, rear: f32| {
        let mut d = Drive::of(&r.ride);
        d.front_spin = front;
        d.rear_spin = rear;
        c.frame(FRAME, Chassis::want(&r.ride, &r.tricks), &Chassis::request(&r.ride, &r.tricks), d);
    };
    for _ in 0..20 {
        r.frame(&p, &tables, &hands, FRAME, &mut world);
        let _ = frame(&mut c, &r);
    }

    // Forward on the front and backward on the rear, so both trails are checked.
    spun(&mut c, &r, 100.0, -100.0);
    let base = c.world.clone();
    let copies = c.blur_copies(&base);
    let n = BLUR_ALPHA.len();
    assert_eq!(copies.len(), 2 * n, "{n} copies a wheel");
    for (i, copy) in copies.iter().enumerate() {
        let (wheel, node, dir) = if i < n { (1u8, front, 1.0) } else { (2u8, rear, -1.0) };
        let k = i % n;
        assert_eq!(copy.alpha, BLUR_ALPHA[k], "copy {i}'s alpha");
        let tris = skin(&geometry, &copy.pose, 0);
        let drawn: Vec<_> = tris.iter().filter(|t| area(t) > 1e-6).collect();
        assert!(!drawn.is_empty(), "copy {i} draws nothing");
        for t in &drawn {
            assert!(t.v.iter().all(|v| c.wheels[v.node] == wheel), "copy {i} draws a triangle off wheel {wheel}");
        }

        let tire = base[node];
        let centre = v3(tire.0[3][0], tire.0[3][1], tire.0[3][2]);
        let axis = tire.dir(v3(1.0, 0.0, 0.0));
        let axis = axis.scale(1.0 / axis.len());
        let want = -dir * BLUR_ARC * (k as f32 + 1.0) / n as f32;
        let (was, now) = (wheel_tris(&geometry, &c, &base, wheel), wheel_tris(&geometry, &c, &copy.pose, wheel));
        assert_eq!(was.len(), now.len());
        assert!(!was.is_empty(), "wheel {wheel} has triangles");
        for (a, b) in was.iter().zip(&now) {
            for (va, vb) in a.v.iter().zip(&b.v) {
                let (ra, rb) = (va.pos.sub(centre), vb.pos.sub(centre));
                let (ha, hb) = (ra.dot(axis), rb.dot(axis));
                assert!((ha - hb).abs() < 1e-3, "copy {i} moved a vertex along the axle: {ha} to {hb}");
                let (pa, pb) = (ra.sub(axis.scale(ha)), rb.sub(axis.scale(hb)));
                assert!((pa.len() - pb.len()).abs() < 1e-3, "copy {i} moved a vertex off its circle");
                if pa.len() > 0.05 {
                    let got = turn_about(axis, pa, pb);
                    assert!(
                        wrap(got - want).abs() < 2e-3,
                        "copy {i} of wheel {wheel} is turned {got} about its axle and wants {want}"
                    );
                }
            }
        }
    }

    // Half way into the blur, the alphas scale with it.
    spun(&mut c, &r, (BLUR_FROM + BLUR_FULL) * 0.5, BLUR_FULL);
    let copies = c.blur_copies(&c.world);
    assert_eq!(copies.len(), 2 * n);
    assert!((copies[0].alpha - BLUR_ALPHA[0] * 0.5).abs() < 1e-6, "half a blur, half the alpha");
    assert_eq!(copies[n].alpha, BLUR_ALPHA[0]);

    // Slow: nothing more is drawn.
    spun(&mut c, &r, BLUR_FROM, -BLUR_FROM);
    assert!(c.blur_copies(&c.world).is_empty(), "a slow wheel draws copies");
    assert!(c.blur_copies(&c.world[1..]).is_empty(), "a pose of another model draws copies");

    // The original's: no copies, and the node at the roll's own angle, however far it went.
    c.original_wheels = true;
    let shocks = c.stance.set.skeleton.find(SHOCKS).expect("Shocks");
    for _ in 0..7 {
        spun(&mut c, &r, 150.0, 150.0);
        assert!(c.blur_copies(&c.world).is_empty(), "the original's wheels are blurred");
        let up = c.world[front].dir(v3(0.0, 1.0, 0.0));
        let parent = c.world[shocks];
        let dir = v3(
            up.x * parent.0[0][0] + up.y * parent.0[0][1] + up.z * parent.0[0][2],
            up.x * parent.0[1][0] + up.y * parent.0[1][1] + up.z * parent.0[1][2],
            up.x * parent.0[2][0] + up.y * parent.0[2][1] + up.z * parent.0[2][2],
        );
        let got = dir.z.atan2(dir.y);
        assert!(
            wrap(got - c.front_roll.angle).abs() < 1e-3,
            "the original's front is drawn at {got} and rolled to {}",
            c.front_roll.angle
        );
    }
    let off = wrap(c.front_spin.angle - c.front_roll.angle).abs();
    println!("V59 blur: {} copies at 100 rad/s; at 150 rad/s the capped turn is {off:.3} rad off the roll", 2 * n);
}

/// **V59.** At full droop the tyres stay on their axles and clear of the fender.
///
/// Off the drop, with both wheels hanging, the front axle is where the fork puts it under the
/// `Shocks` node and the rear is its bind distance from the swingarm pivot; each tyre's triangles
/// are round its axle (the middle of them on it); and the front fender, placed at `Handlebars` the
/// way the shell places it, has no vertex inside the tyre — within the tyre's width, every one is
/// farther from the axle than the tyre's outer edge. Over the whole minute the least clearance is
/// printed, with the compression it came at.
#[test]
fn at_full_droop_the_tyres_stay_on_their_axles_and_clear_of_the_fender() {
    let Some(mut disc) = disc() else { return };
    let set = stunts(&mut disc);
    let tables = rider::Tables::build();
    let p = phy(&mut disc, &tables);
    let build = sub(&mut disc);
    let mut c = chassis(&mut disc, &set, &build.mcf);
    let res = bike_res(&mut disc);
    let geometry = Model::parse(&res.read_suffix(&format!("{}.psg", c.geometry)).expect("psg"))
        .expect("the chassis geometry parses");
    let part = build
        .parts
        .iter()
        .find(|part| part.model.to_ascii_lowercase().contains("front_fender"))
        .expect("the bike has a front fender part");
    assert!(part.placement.eq_ignore_ascii_case(BARS), "the fender hangs off {}", part.placement);
    let name = part.model.trim_end_matches(".psg");
    let fender = Model::parse(&res.read_suffix(&format!("{name}.psg")).expect("fender psg")).expect("the fender parses");
    let fender_pose: Vec<M4> = (0..fender.nodes.len()).map(|i| fender.world(i)).collect();
    let fender_tris = skin(&fender, &fender_pose, 0);
    assert!(!fender_tris.is_empty(), "the fender skinned to nothing");

    let sk = &c.stance.set.skeleton;
    let (front, rear) = (sk.find(FRONT).expect("TireFront"), sk.find(REAR).expect("TireRear"));
    let (shocks, swingarm) = (sk.find(SHOCKS).expect("Shocks"), sk.find(SWINGARM).expect("SwingArm"));
    let bars = sk.find(BARS).expect("Handlebars");
    let rest = |name: &str| c.node_rest(name).expect("a bind place");
    let fork_drop = rest(FRONT).sub(rest(SHOCKS)).len();
    let arm = rest(REAR).sub(rest(SWINGARM)).len();

    let mut r = ride(&p, &c, &set);
    let mut world = Flat::default();
    let hands = Controls { gas: F::ONE, ..Controls::default() };
    let mut droop = 0;
    let mut least = (f32::MAX, 0.0f32);
    let mut least_droop = f32::MAX;
    for k in 0..A_MINUTE {
        r.frame(&p, &tables, &hands, FRAME, &mut world);
        let compression = [r.ride.front.w.compression.0, r.ride.rear.w.compression.0];
        let on_ground = Drive::of(&r.ride).on_ground;
        let _ = frame(&mut c, &r);
        let at = |j: usize| v3(c.world[j].0[3][0], c.world[j].0[3][1], c.world[j].0[3][2]);

        // The front tyre against the fender, every frame.
        let axis = c.world[front].dir(v3(1.0, 0.0, 0.0));
        let axis = axis.scale(1.0 / axis.len());
        let centre = at(front);
        let tyre = wheel_tris(&geometry, &c, &c.world, 1);
        let (mut outer, mut width) = (0.0f32, 0.0f32);
        for v in tyre.iter().flat_map(|t| t.v.iter()) {
            let r = v.pos.sub(centre);
            let h = r.dot(axis);
            outer = outer.max(r.sub(axis.scale(h)).len());
            width = width.max(h.abs());
        }
        let bars = c.world[bars];
        let mut clearance = f32::MAX;
        for v in fender_tris.iter().flat_map(|t| t.v.iter()) {
            let r = bars.point(v.pos).sub(centre);
            let h = r.dot(axis);
            if h.abs() < width {
                clearance = clearance.min(r.sub(axis.scale(h)).len() - outer);
            }
        }
        if clearance < least.0 {
            least = (clearance, compression[0]);
        }

        if on_ground || compression[0] > 1e-4 || compression[1] > 1e-4 {
            continue;
        }
        droop += 1;
        least_droop = least_droop.min(clearance);
        let drop = at(front).sub(at(shocks)).len();
        assert!((drop - fork_drop).abs() < 1e-3, "frame {k}: the front axle is {drop} from Shocks, bound at {fork_drop}");
        let reach = at(rear).sub(at(swingarm)).len();
        assert!((reach - arm).abs() < 1e-3, "frame {k}: the rear axle is {reach} from the pivot, bound at {arm}");
        for (wheel, node) in [(1u8, front), (2, rear)] {
            let tris = wheel_tris(&geometry, &c, &c.world, wheel);
            let axis = c.world[node].dir(v3(1.0, 0.0, 0.0));
            let axis = axis.scale(1.0 / axis.len());
            let mut mid = v3(0.0, 0.0, 0.0);
            for v in tris.iter().flat_map(|t| t.v.iter()) {
                mid = mid.add(v.pos.scale(1.0 / (3 * tris.len()) as f32));
            }
            let off = mid.sub(at(node));
            let off = off.sub(axis.scale(off.dot(axis))).len();
            assert!(off < 0.05, "frame {k}: wheel {wheel}'s middle is {off} off its axle");
        }
        assert!(clearance > 0.0, "frame {k}: the fender is {clearance} into the front tyre at full droop");
    }
    assert!(droop > 0, "the drop never hung both wheels at full droop");
    println!(
        "V59 droop: {droop} frames hanging, the fender at least {least_droop:.3} clear of the front tyre \
         there; over the minute at least {:.3}, at a front compression of {:.3}",
        least.0, least.1
    );
}
