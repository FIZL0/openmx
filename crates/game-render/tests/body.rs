//! The body on the bike, against the real disc: the clips a rider's gesture slots and airborne
//! poses name, where a posed body stands, and a ride's worth of frames posed by the simulation's
//! own animation driving.
//!
//! Needs the user's own disc, so every test skips with the reason when it is not there — a fresh
//! checkout stays green.

use game_render::anim::Set;
use game_render::disc::{Disc, Res};
use game_render::psg::Model;
use game_render::rider::{
    AIR_POSES, Body, FOOT_ANCHORS, HAND_ANCHORS, HOLD_WITHIN, LET_GO_AT, LIMBS, Sex, Skeleton,
    anchors_of, hold_on, stunt_name,
};
use game_render::skin::skin;
use game_render::mat::v3;
use game_render::world::{Camera, vertical_fov};
use game_sim::ride::Controls;
use game_sim::trick::{Dir, SLOTS, Set as Stunts24, Stunts, Trick, slot_of};
use game_sim::world::Flat;
use game_sim::{motion, phy::Phy, rider};
use ps2_float::{F, Vf};

/// The stock gear: the rider every other one on the disc is a repaint of.
const A_RIDER: &str = "thq1";
/// The bike the port is built against — its chassis, and the `.phy` the ride runs on.
const A_BIKE: &str = "250sx";
/// Where the rider's stunt table lives, by the same two names the shell uses.
const STUNT_ARCHIVE: &str = "appstart.res";
const STUNT_TABLE: &str = "rider\\anim\\stunts.ini";

/// The frame the fixtures run at: the engine's own clamped step.
const FRAME: F = F(1.0 / 60.0);
/// A minute of it, which is what the task asks a ride to survive.
const A_MINUTE: usize = 60 * 60;
/// How high a stunt fixture drops the bike from, so a stunt has air to run in.
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

/// The member of the ISO whose name ends this way, which is how every archive on the disc is found:
/// the directory it sits in is the game's business. The shortest match, because the menus keep
/// their own copy of a bike one directory deeper.
fn member(disc: &Disc, ends: &str) -> Option<String> {
    let want = ends.to_ascii_lowercase();
    disc.entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(&want))
        .min_by_key(|e| e.name.len())
        .map(|e| e.name.clone())
}

/// One copy of the rider's archive. An animation set owns the archives it reads its `.mot` files
/// out of, so a test that both poses a body and loads its geometry opens the archive twice — the
/// same thing the shell does.
///
/// Past the disc itself nothing here is allowed to be missing: a disc with no rider, no stunt
/// table or no bike is a broken disc, and a test that quietly passed on one would prove nothing.
fn rider_res(disc: &mut Disc) -> Res {
    let name = member(disc, &format!("{A_RIDER}.res")).expect("the disc has no rider archive");
    disc.res(&name).expect("the rider archive does not open")
}

/// The 24 stunts this rider can do, off the disc's own table.
fn stunts(disc: &mut Disc) -> Stunts24 {
    let name = member(disc, STUNT_ARCHIVE).expect("the disc has no app archive");
    let res = disc.res(&name).expect("the app archive does not open");
    let bytes = res.read_suffix(STUNT_TABLE).expect("the app archive has no stunt table");
    Stunts24::new(Stunts::parse(&bytes).expect("the stunt table parses").selection(0))
}

/// The bike's own parameters, which the ride runs on and the animation's blend rate comes off.
///
/// A bike archive carries one `.phy` per class and difficulty (`race250a`, `pro250p`, …) and none
/// named after the bike, so this takes the first the archive holds — the same fallback the shell
/// takes, and which one it is does not matter here: what is being ridden is the animation.
fn phy(disc: &mut Disc, tables: &rider::Tables) -> Phy {
    let name = member(disc, &format!("{A_BIKE}.res")).expect("the disc has no bike archive");
    let res = disc.res(&name).expect("the bike archive does not open");
    let bytes = res.read_suffix(".phy").expect("the bike archive has no .phy");
    Phy::read_bytes(&bytes, &tables.ride.curve)
}

/// A body with every clip its gesture slots and airborne poses name.
fn body(disc: &mut Disc, set: &Stunts24, phy: &Phy) -> Body {
    let res = rider_res(disc);
    match Body::load(vec![res], A_RIDER, Sex::Male, "", &set.stunts, phy.motion()) {
        Ok(body) => body,
        Err(why) => panic!("the body did not load: {why}"),
    }
}

/// **V8.** Every gesture slot and every airborne pose of a real rider binds to a clip in the
/// rider's own set.
///
/// The names are not the bike's: `game_sim::motion::Library::stunt_name` asks for `bike_<stunt>`,
/// which is the vehicle's clip, and the rider's set holds `rider_<stunt>` for the same stunt — so
/// a body loaded with the bike's rule would bind nothing at all. The three airborne poses are the
/// set's own `air_Fwd` / `air_Mid` / `air_Bck`, which no prefix touches.
#[test]
fn every_gesture_slot_and_airborne_pose_of_a_rider_is_bound() {
    let Some(mut disc) = disc() else { return };
    let set = stunts(&mut disc);
    let tables = rider::Tables::build();
    let p = phy(&mut disc, &tables);
    let body = body(&mut disc, &set, &p);

    let named = set.stunts.iter().filter(|s| !s.name.is_empty()).count();
    assert_eq!(named, SLOTS, "the disc's first selection names all {SLOTS} slots");
    for (slot, stunt) in set.stunts.iter().enumerate() {
        let name = stunt_name("", &stunt.name);
        assert!(
            body.rig.lib.stunt[slot].is_some(),
            "slot {slot} ({:?}) has no clip: the set wanted {name}",
            stunt.name
        );
        let id = body.rig.lib.stunt[slot].expect("just checked");
        let bound = body.rig.set.get(id).expect("a bound id is in the set");
        assert!(bound.name.eq_ignore_ascii_case(&name), "slot {slot} bound {} not {name}", bound.name);
        assert!(bound.clip.timing.duration > F::ZERO, "{name} has no length");
        assert!(bound.named() > 0, "{name} reaches no bone of the rider's skeleton");
    }
    for (what, id) in
        AIR_POSES.iter().zip([body.rig.lib.air_fwd, body.rig.lib.air_mid, body.rig.lib.air_bck])
    {
        let id = id.unwrap_or_else(|| panic!("the airborne pose {what} is not bound"));
        let bound = body.rig.set.get(id).expect("a bound id is in the set");
        assert!(bound.name.eq_ignore_ascii_case(what), "{what} bound {}", bound.name);
        assert!(bound.named() > 0, "{what} reaches no bone of the rider's skeleton");
    }
    println!(
        "{A_RIDER}: {} clips bound, all {SLOTS} gesture slots and all 3 airborne poses",
        body.rig.set.clips.len()
    );
}

/// **V8.** A posed body is already in the bike's own space, so a drawn rider needs no placement.
///
/// V4 had to choose one ([`bolt_onto`] on the foot anchors) because the bind pose is modelled at
/// the origin and the game never draws it. This measures what the clips do instead: posed by the
/// first frame of a stunt, the four dummy anchors the two skeletons share land on the chassis's
/// four, with the feet on the pegs and the hands at the grips — no bolt, no rotation, nothing.
#[test]
fn a_posed_body_stands_on_the_bike_with_nothing_bolted() {
    let Some(mut disc) = disc() else { return };
    let res = rider_res(&mut disc);
    let bike = member(&disc, &format!("{A_BIKE}.res")).expect("the disc has no bike archive");
    let chassis = disc.res(&bike).and_then(|r| r.read_suffix("bike.psg")).expect("the chassis");
    let chassis = Skeleton::of(&Model::parse(&chassis).expect("the chassis parses"));

    let mut set = Set::load(vec![res], "rider\\anim\\mrider.mcf").expect("the rider's set");
    set.load_listed();
    let anchors: Vec<&str> = FOOT_ANCHORS.iter().chain(HAND_ANCHORS.iter()).copied().collect();

    // The bind pose first, as the control: its anchors miss the chassis's by five inches, which is
    // why V4 needed a placement at all — a body drawn in it sits through the seat with its arms
    // out.
    let bind = set.world();
    let apart = |world: &[game_render::mat::M4], name: &str| -> f32 {
        let i = set.skeleton.find(name).expect("the rider has this anchor");
        let at = world[i].0[3];
        let want = chassis.at(name).expect("the chassis has this anchor");
        ((at[0] - want.x).powi(2) + (at[1] - want.y).powi(2) + (at[2] - want.z).powi(2)).sqrt()
    };
    let worst_bind = anchors.iter().map(|n| apart(&bind, n)).fold(0.0, f32::max);
    println!("the bind pose: the furthest anchor is {worst_bind:.3} from the chassis's");
    assert!(worst_bind > 3.0, "the bind pose is meant to miss the bike, not by {worst_bind}");

    // And then posed. The pose a riding clip holds is the one to measure: the grounded idle plays
    // `air_Mid` at its exact midpoint (`game_sim::motion::Driver::select`), and every clip a rider
    // rides in puts all four anchors on the chassis's there. The first frame of a clip is not the
    // place to look — several of them start mid-transition out of whatever came before, and
    // `air_Fwd` opens with the hands two and a half inches off the grips before it settles.
    //
    // What does *not* land is a gesture: `hold_knee_left` takes a foot four inches off its peg and
    // `wreck_sep_over_bars` throws the whole body over the bars. That is the stunt, not a
    // placement error, and it is why a drawn rider is the clips' business and not this module's.
    for (name, phase) in [
        ("turn_still_Mid", 0.5),
        ("turn_riding_Mid", 0.5),
        ("air_Fwd", 0.5),
        ("air_Mid", 0.5),
        ("air_Bck", 0.5),
        ("backflip", 0.0),
    ] {
        let id = set.id(name).unwrap_or_else(|| panic!("no clip named {name}"));
        let bound = set.get(id).expect("a found id is in the set");
        let mut pose = game_render::skin::Pose::new(set.skeleton.len());
        bound.clip.pose(phase * bound.clip.timing.duration.0, &bound.joints, &mut pose);
        let world = set.skeleton.world(&pose);
        let worst = anchors.iter().map(|n| apart(&world, n)).fold(0.0, f32::max);
        println!("posed by {name} at {phase}: the furthest anchor is {worst:.3} from the chassis's");
        assert!(worst < 1.5, "posed by {name}, an anchor is {worst} from the chassis's");
        assert!(worst < worst_bind, "posed by {name} is no closer than the bind pose");
    }
}

/// **V8.** A minute of riding poses the body every frame, and the pose is drawable every frame.
///
/// The driving is the simulation's own — `game_sim::rider::Rider` runs one
/// `game_sim::motion::Driver` and keeps what it answered — so this is the shell's loop with the
/// window taken off: step the rider, hand the frame to [`Body::pose`], skin the geometry.
#[test]
fn a_minute_of_riding_poses_the_body_every_frame() {
    let Some(mut disc) = disc() else { return };
    let set = stunts(&mut disc);
    let tables = rider::Tables::build();
    let p = phy(&mut disc, &tables);
    let mut body = body(&mut disc, &set, &p);
    let res = rider_res(&mut disc);
    let geometry = Model::parse(&res.read_suffix(&format!("{}.psg", body.geometry())).expect("psg"))
        .expect("the body's geometry parses");

    let mounts = (p.front.body_tire_force_position, p.rear.body_tire_force_position);
    let mut r = rider::Rider::new(
        &p,
        mounts,
        Vf::new(0.0, DROP, 0.0, 1.0),
        Vf::new(0.0, 0.0, 1.0, 0.0),
        set.clone(),
    );
    let mut world = Flat::default();
    // Dropped in, so the minute has airborne frames in it as well as grounded ones, with a stunt
    // asked for on the way down: between them the selection's gesture arm, its airborne arm and
    // its grounded idle all pose the body at least once.
    let (slot, _) = shortest_stunt(&set);
    let gesture = gesture_for(slot);
    // Hands on for the rest of it: full throttle with the stick held over, so the ride state
    // machine and the animation driving both have something to do.
    let hands = Controls { gas: F::ONE, stick: (F(0.4), F(-0.2)), ..Controls::default() };
    let ask = Controls {
        trick: buttons_for(gesture.0),
        dpad: dpad_for(gesture.1),
        ..Controls::default()
    };

    let mut posed = 0usize;
    let mut blends = [0usize; 4];
    for k in 0..A_MINUTE {
        r.frame(&p, &tables, if k == 2 { &ask } else { &hands }, FRAME, &mut world);
        body.pose(FRAME, r.animation);
        blends[match r.animation.blend {
            motion::Blend::Free => 0,
            motion::Blend::One { .. } => 1,
            motion::Blend::Two { .. } => 2,
            motion::Blend::Three { .. } => 3,
        }] += 1;
        let joints = body.rig.world();
        assert_eq!(joints.len(), body.rig.set.skeleton.len(), "a transform per joint, frame {k}");
        for (j, m) in joints.iter().enumerate() {
            assert!(
                m.0.iter().flatten().all(|v| v.is_finite()),
                "joint {j} is not finite on frame {k}: {m:?}"
            );
        }
        if body.posed {
            posed += 1;
        }
        // Skinning every frame is the shell's job and is what the sixty seconds are about; a tenth
        // of a second apart is enough to prove the triangles come out of it without making the
        // test a renderer benchmark.
        if k % 6 == 0 {
            let tris = skin(&geometry, &joints, 0);
            assert!(!tris.is_empty(), "the body skinned to nothing on frame {k}");
        }
    }
    assert_eq!(posed, A_MINUTE, "every frame of the minute wrote a pose");
    println!("a minute of riding: {A_MINUTE} frames, blends free/one/two/three {blends:?}");
}

/// **V8.** A stunt looks like the stunt it is: its first frame and its last are different poses.
///
/// The gesture is made in the air, the stunt runs to its end, and the body is posed from the same
/// driving the simulation did — so what this compares is two frames of one stunt as the shell would
/// have drawn them.
#[test]
fn a_stunts_first_and_last_frames_are_different_poses() {
    let Some(mut disc) = disc() else { return };
    let set = stunts(&mut disc);
    let tables = rider::Tables::build();
    let p = phy(&mut disc, &tables);
    let mut body = body(&mut disc, &set, &p);

    let (slot, stunt) = shortest_stunt(&set);
    let gesture = gesture_for(slot);

    let mounts = (p.front.body_tire_force_position, p.rear.body_tire_force_position);
    let mut r = rider::Rider::new(
        &p,
        mounts,
        Vf::new(0.0, DROP, 0.0, 1.0),
        Vf::new(0.0, 0.0, 1.0, 0.0),
        set.clone(),
    );
    let mut world = Flat::default();

    let mut first: Option<Vec<game_render::mat::M4>> = None;
    let mut last: Option<Vec<game_render::mat::M4>> = None;
    let mut frames = 0usize;
    for k in 0..600 {
        // Asked for on frame 2, held for that frame only, so the stunt's clock runs free.
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
        body.pose(FRAME, r.animation);
        if said.started == Some(Trick(slot as u8)) {
            first = Some(body.rig.world());
        }
        if first.is_some() && r.tricks.in_progress {
            last = Some(body.rig.world());
            frames += 1;
        }
        if first.is_some() && !r.tricks.in_progress {
            break;
        }
    }
    let (first, last) = (first.expect("the gesture started the stunt"), last.expect("it ran"));
    let moved = |a: &[game_render::mat::M4], b: &[game_render::mat::M4]| -> f32 {
        a.iter()
            .zip(b)
            .map(|(x, y)| {
                let (x, y) = (x.0[3], y.0[3]);
                ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt()
            })
            .fold(0.0, f32::max)
    };
    let apart = moved(&first, &last);
    println!(
        "{:?} ({:.3}s, {frames} frames): the furthest joint moved {apart:.3} between its first \
         frame and its last",
        stunt.name, stunt.duration.0
    );
    assert!(apart > 1.0, "{:?} drew the same pose at both ends: {apart}", stunt.name);
}

/// The shortest stunt the rider has, so that it fits inside one drop with room either side.
/// Whichever one that is comes off the disc, not out of this file.
fn shortest_stunt(set: &Stunts24) -> (usize, game_sim::trick::Stunt) {
    set.stunts
        .iter()
        .enumerate()
        .filter(|(k, s)| *k != set.no_hander && *k != set.throwaway && s.duration > F::ZERO)
        .min_by(|a, b| a.1.duration.0.total_cmp(&b.1.duration.0))
        .map(|(k, s)| (k, s.clone()))
        .expect("a stunt to do")
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

/// **V8.** The first frame of a ride draws the bind pose, and that is the cross-fade doing its job.
///
/// `game_sim::motion::Driver::frame` restarts the motion on the first frame of a ride, and a
/// restart puts the fade ramp back to 1 — so the pose the first frame asks for is all of the pose
/// the rider was already in, which before anything has posed it is the pose the body was modelled
/// in. It fades out of it at the bike's own blend rate, a third of a second on this `.phy`, and
/// from then on the body rides.
///
/// This is here because the shell's `--shot` writes the first frame and a reader who sees a rider
/// with its arms out should be able to find out why without re-deriving it.
#[test]
fn the_first_frame_of_a_ride_fades_out_of_the_bind_pose() {
    let Some(mut disc) = disc() else { return };
    let set = stunts(&mut disc);
    let tables = rider::Tables::build();
    let p = phy(&mut disc, &tables);
    let mut body = body(&mut disc, &set, &p);
    let bind = body.rig.world();

    let mounts = (p.front.body_tire_force_position, p.rear.body_tire_force_position);
    let mut r = rider::Rider::new(
        &p,
        mounts,
        Vf::new(0.0, 0.0, 0.0, 1.0),
        Vf::new(0.0, 0.0, 1.0, 0.0),
        set.clone(),
    );
    let mut world = Flat::default();
    let hands = Controls { gas: F::ONE, ..Controls::default() };
    let furthest = |a: &[game_render::mat::M4], b: &[game_render::mat::M4]| -> f32 {
        a.iter()
            .zip(b)
            .map(|(x, y)| {
                let (x, y) = (x.0[3], y.0[3]);
                ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt()
            })
            .fold(0.0, f32::max)
    };

    // The ramp falls by the `.phy`'s blend rate per second, so the fade is over in that many
    // frames; a ride is sampled a little past it.
    let fade = (60.0 / p.motion().blend_rate.0).ceil() as usize;
    let mut at_first = 0.0;
    for k in 0..=fade {
        r.frame(&p, &tables, &hands, FRAME, &mut world);
        body.pose(FRAME, r.animation);
        if k == 0 {
            assert!(r.animation.restart, "the first frame of a ride restarts the motion");
            assert!(body.posed, "and it writes a pose, even though the pose is all of the old one");
            at_first = furthest(&body.rig.world(), &bind);
        }
    }
    let settled = furthest(&body.rig.world(), &bind);
    println!(
        "the first frame is {at_first:.3} from the bind pose and frame {fade} is {settled:.3}; \
         the fade is {:.3}s at a blend rate of {}",
        fade as f32 / 60.0,
        p.motion().blend_rate.0
    );
    assert!(at_first < 0.001, "the first frame is not the bind pose: {at_first} away");
    assert!(settled > 3.0, "the fade never got out of the bind pose: {settled} away");
}

/// **V10.** The body falls back through its own five levels of detail as the camera pulls away, and
/// the triangles it costs fall with it.
///
/// V8 drew the body from group 0 at every distance, so a rider two hundred feet away cost what the
/// one on your bike cost. The rule that picks between the five is the engine's own
/// (`Model::group_for`): the term it tests is a focal length over a distance — how big the model is
/// on screen — so the hand-over distances below are this camera's, and a narrower field of view or a
/// taller window moves all of them.
#[test]
fn the_level_of_detail_falls_back_as_the_camera_pulls_away() {
    let Some(mut disc) = disc() else { return };
    let res = rider_res(&mut disc);
    let body = Model::parse(&res.read_suffix("mrider.psg").expect("the body's geometry"))
        .expect("the body's geometry parses");
    assert_eq!(body.groups, 5, "the body's .psg holds five levels of detail");
    assert_eq!(body.group_distance.len(), body.groups, "a threshold per group");
    let last = body.group_distance[body.groups - 1];
    assert!(last > 1e30, "the coarsest group's threshold is the FLT_MAX sentinel, not {last}");

    // What each group costs, which is the point of having them.
    let cost: Vec<usize> = (0..body.groups).map(|g| body.triangles(g).len()).collect();
    println!("the body's five groups cost {cost:?} triangles");
    for g in 1..4 {
        assert!(cost[g] < cost[g - 1], "group {g} ({}) is no cheaper than {}", cost[g], cost[g - 1]);
    }
    // The coarsest group is the odd one out in this file: it is the catch-all past every threshold
    // and the artists left it dearer than the group before it, 417 triangles against 156. It is
    // still a fifth of the finest, which is what a group being drawn at all is for.
    assert!(cost[4] < cost[0] / 2, "even the catch-all is cheaper than the body: {cost:?}");

    // The camera the game rides with: `Preset1`'s 85 degrees across, in a 4:3 frame of the disc's
    // own height.
    let camera = Camera {
        eye: v3(0.0, 0.0, 0.0),
        at: v3(0.0, 0.0, 1.0),
        fov: vertical_fov(85.0, 4.0 / 3.0),
        near: 0.05,
        far: 4000.0,
    };
    let focal = camera.focal(448);
    let radius = body
        .triangles(0)
        .iter()
        .flat_map(|t| t.v.iter())
        .map(|v| v.pos.len())
        .fold(0.0f32, f32::max);
    println!(
        "a focal length of {focal:.1} pixels and a body reaching {radius:.2} feet: the groups hand \
         over at {:?} feet",
        body.group_distance
            .iter()
            .take(body.groups - 1)
            .map(|t| (t * focal + radius).round() as i32)
            .collect::<Vec<_>>()
    );

    // Pulling away, a foot at a time to a quarter of a mile: the group may only ever get coarser and
    // the cost may only ever fall.
    let mut was = (0usize, cost[0]);
    let mut falls = 0;
    for feet in 1..=1320 {
        let group = body.group_for(feet as f32, radius, focal);
        assert!(group < body.groups, "group {group} is not one of the five, at {feet} feet");
        assert!(group >= was.0, "the body got finer at {feet} feet: group {group} after {}", was.0);
        if group != was.0 {
            let now = cost[group];
            println!("at {feet} feet it falls to group {group}, {now} triangles");
            // Every hand-over but the last is cheaper than the group it came from. The last is the
            // catch-all this file left dearer than the group before it (above), and the camera does
            // not reach it until the rider is a hundred yards off and four pixels tall.
            if group + 1 < body.groups {
                assert!(now < was.1, "falling to group {group} at {feet} feet cost more: {now}");
            } else {
                assert!(feet > 300, "the catch-all group should not be drawn {feet} feet away");
                assert!(now < cost[0] / 4, "the catch-all costs {now} of the body's {}", cost[0]);
            }
            falls += 1;
            was = (group, now);
        }
    }
    assert_eq!(falls, body.groups - 1, "every group should get its turn inside a quarter mile");

    // The two ends the task asks about, and the one term the window has in it: the same rider at the
    // same distance is drawn finer into a taller frame, which is the rule's own shape rather than a
    // distance table.
    let close = body.group_for(8.0, radius, focal);
    let far = body.group_for(200.0, radius, focal);
    assert_eq!(close, 0, "the rider on your bike is drawn from the finest group");
    assert!(far > close, "a rider two hundred feet away is still group {far}");
    assert!(
        cost[far] * 4 < cost[close],
        "two hundred feet away still costs {} of {}",
        cost[far],
        cost[close]
    );
    let tall = body.group_for(200.0, radius, camera.focal(1792));
    assert!(tall < far, "a frame four times as tall drew the same rider from group {tall}");
}


/// A twelfth of a world unit. The disc's unit is a foot — a terrain quad is three of them, a bike's
/// wheelbase five and a quarter, this body five and a half tall — so an inch is a twelfth.
const AN_INCH: f32 = 1.0 / 12.0;

/// **V10.** A gesture that takes one limb off the bike leaves the others holding on: solved against
/// the chassis's own anchors, every hand that is on the bars is on the grip to within an inch and
/// every foot that is on the pegs is on the peg, while the limb the stunt lets go of is left exactly
/// where the clip put it.
///
/// This is what the four dummy anchors are for, and why the original keeps a controller per limb and
/// aims it at the vehicle's own anchors every frame. A clip is a recording made against one pose of
/// one bike: played on a bike whose bars are turned, at a lean the animator did not finish on, or
/// through a cross-fade out of something else, the hand drifts off the grip and the stunt reads as a
/// clip playing rather than a rider holding on. Every gesture this rider has, at nine phases of each,
/// and nothing excluded.
///
/// The weight each limb holds on with is [`game_render::rider::holding`], which is the port's own
/// rule and not the original's — see the module and the journal. What is being tested here is both
/// halves of it: the limbs it holds land on the bike, and the limbs it does not are untouched.
#[test]
fn a_solved_hand_stays_on_the_grip_through_a_gesture() {
    let Some(mut disc) = disc() else { return };
    let bike = member(&disc, &format!("{A_BIKE}.res")).expect("the disc has no bike archive");
    let chassis = disc.res(&bike).and_then(|r| r.read_suffix("bike.psg")).expect("the chassis");
    let chassis = Skeleton::of(&Model::parse(&chassis).expect("the chassis parses"));
    // The targets, read off the chassis the way the vehicle hands them to the rider.
    let targets = anchors_of(&chassis, &[]);
    assert!(targets.iter().all(|t| t.is_some()), "the chassis has all four of the rider's anchors");

    let table = stunts(&mut disc);
    let tables = rider::Tables::build();
    let p = phy(&mut disc, &tables);
    let body = body(&mut disc, &table, &p);
    let skel = body.rig.set.skeleton.clone();
    // The joints the solve has no business touching.
    let still: Vec<usize> = ["Pelvis", "LwrTorso", "UprTorso", "Head"]
        .iter()
        .map(|n| skel.find(n).unwrap_or_else(|| panic!("the body has no {n}")))
        .collect();
    let at = |world: &[game_render::mat::M4], j: usize| {
        let m = world[j].0[3];
        v3(m[0], m[1], m[2])
    };

    let mut held = [0usize; 4];
    let mut let_go = [0usize; 4];
    let mut stretched = 0usize;
    let mut worst_held = (0.0f32, String::new());
    let mut worst_fixed = (0.0f32, String::new());
    for stunt in table.stunts.iter().filter(|s| !s.name.is_empty()) {
        let name = stunt_name("", &stunt.name);
        let id = body.rig.set.id(&name).unwrap_or_else(|| panic!("no clip named {name}"));
        let bound = body.rig.set.get(id).expect("a found id is in the set");
        for step in 0..=8 {
            let phase = step as f32 / 8.0;
            let mut pose = game_render::skin::Pose::new(skel.len());
            bound.clip.pose(phase * bound.clip.timing.duration.0, &bound.joints, &mut pose);
            let before = skel.world(&pose);
            let mut after = before.clone();
            let ended = hold_on(&skel, &mut after, &targets, 1.0);

            for (i, limb) in LIMBS.iter().enumerate() {
                let target = targets[i].expect("every anchor has a target here");
                let (root, bend, anchor) = (
                    skel.find(limb.root).expect("the body has this joint"),
                    skel.find(limb.bend).expect("the body has this joint"),
                    skel.find(limb.anchor).expect("the body has this anchor"),
                );
                let was = at(&before, anchor).sub(target).len();
                let now = ended[i].unwrap_or_else(|| panic!("{} was not solved", limb.anchor));
                let where_it_is = at(&after, anchor).sub(target).len();
                assert!(
                    (where_it_is - now).abs() < 1e-3,
                    "the solve's answer for {} ({now:.4}) is not where it left the joint \
                     ({where_it_is:.4})",
                    limb.anchor
                );
                // What the limb is made of, and whether the anchor was ever within its reach: the
                // two lengths the solve works in, off the pose it started from.
                let (upper, lower) = (
                    at(&before, bend).sub(at(&before, root)).len(),
                    at(&before, anchor).sub(at(&before, bend)).len(),
                );
                let span = target.sub(at(&before, root)).len();
                let short = span - (upper + lower);
                let where_ = format!("{} in {name} at {phase}", limb.anchor);

                if was >= LET_GO_AT {
                    // The stunt let go. Nothing may move at all — not the limb, not its anchor.
                    let_go[i] += 1;
                    assert!(
                        (now - was).abs() < 1e-6,
                        "{where_}: the stunt let go {was:.2} feet back and the solve moved it to \
                         {now:.2}"
                    );
                    for j in [root, bend, anchor] {
                        assert!(
                            at(&after, j).sub(at(&before, j)).len() < 1e-6,
                            "{where_}: a let-go limb's joints moved"
                        );
                    }
                } else if was <= HOLD_WITHIN && short < -AN_INCH {
                    // Holding on, and the anchor is inside the limb's reach: it goes on the bike.
                    held[i] += 1;
                    assert!(now < AN_INCH, "{where_}: ended {now:.4} out, from {was:.4}");
                    if was > worst_fixed.0 {
                        worst_fixed = (was, where_.clone());
                    }
                } else if was <= HOLD_WITHIN {
                    // Holding on, but the clip has the limb at or past full stretch. The best it can
                    // do is straighten at the anchor, and it may not end further out than that.
                    stretched += 1;
                    assert!(
                        now <= was + 1e-3 && now < short.max(0.0) + AN_INCH,
                        "{where_}: {now:.4} out with {short:.4} of stretch to make up, from {was:.4}"
                    );
                } else {
                    // In between: the limb is faded on, so the anchor moves towards the bike without
                    // reaching it. The fade is the point — an edge here would pop the frame it
                    // crossed — so what is checked is the direction and the bound.
                    assert!(
                        now <= was + 1e-3,
                        "{where_}: a faded limb went further out, {was:.3} to {now:.3}"
                    );
                }
                if was > worst_held.0 {
                    worst_held = (was, where_);
                }
                // Turned, not stretched: both of the limb's lengths survive the solve.
                let (upper_now, lower_now) = (
                    at(&after, bend).sub(at(&after, root)).len(),
                    at(&after, anchor).sub(at(&after, bend)).len(),
                );
                assert!(
                    (upper_now - upper).abs() < 1e-3 && (lower_now - lower).abs() < 1e-3,
                    "{name} at {phase}: {} came out of the solve a different length",
                    limb.root
                );
            }
            for &j in &still {
                assert!(
                    at(&after, j).sub(at(&before, j)).len() < 1e-4,
                    "{name} at {phase}: the solve moved {}",
                    skel.bones[j].name
                );
            }
        }
    }
    println!(
        "{} gestures at nine phases: {} limbs held onto the bike, {} were let go of, {} were at \
         full stretch; the furthest an anchor started out was {:.2} feet ({}), and the furthest one \
         that was pulled back on was {:.2} feet ({})",
        table.stunts.iter().filter(|s| !s.name.is_empty()).count(),
        held.iter().sum::<usize>(),
        let_go.iter().sum::<usize>(),
        stretched,
        worst_held.0,
        worst_held.1,
        worst_fixed.0,
        worst_fixed.1
    );
    println!(
        "held: {held:?} and let go of: {let_go:?}, in the order of LIMBS — foot L, foot R, hand L,          hand R"
    );
    // The test proves nothing unless the gestures really do take limbs off the bike and put others
    // back on it.
    assert!(held.iter().sum::<usize>() > 400, "barely any limb was held: {held:?}");
    assert!(let_go[0] + let_go[1] > 0, "no gesture let go of a foot");
    assert!(let_go[2] + let_go[3] > 0, "no gesture let go of a hand");
    assert!(worst_fixed.0 > 0.5, "nothing more than half a foot out was ever pulled back on");
    assert!(held[2] + held[3] > 100, "hardly any hand was put back on the bars: {held:?}");
}

/// **V10.** The one-hander, by name: the hand the stunt keeps on the bars ends on the grip and the
/// hand it throws stays thrown.
///
/// The general test above sweeps every gesture and holds the rule to both halves of itself. This one
/// is the sentence the task asks for, on the three stunts that say it plainest — a one-hander, a
/// can-can and a superman — so that a later change to the rule has to argue with the stunts by name.
#[test]
fn a_one_hander_keeps_the_other_hand_on_the_bars() {
    let Some(mut disc) = disc() else { return };
    let bike = member(&disc, &format!("{A_BIKE}.res")).expect("the disc has no bike archive");
    let chassis = disc.res(&bike).and_then(|r| r.read_suffix("bike.psg")).expect("the chassis");
    let chassis = Skeleton::of(&Model::parse(&chassis).expect("the chassis parses"));
    let targets = anchors_of(&chassis, &[]);
    let table = stunts(&mut disc);
    let tables = rider::Tables::build();
    let p = phy(&mut disc, &tables);
    let body = body(&mut disc, &table, &p);
    let skel = body.rig.set.skeleton.clone();

    // Which limbs each of the three is still holding the bike with at the phase the stunt is being
    // held at — the hold time is the stunt table's own (`Stunt::hold_time`) — in the order of
    // `LIMBS`: left foot, right foot, left hand, right hand. The rest are thrown, and the rider is
    // not getting them back until the stunt is over: a one-handed indian air is both feet forward
    // off the pegs and the left hand off the bars, a can-can swings the right leg over the seat, and
    // a superman is both feet off with both hands on.
    for (stunt, holding) in [
        ("onehandindianair", &[3][..]),
        ("cancan", &[0, 2, 3][..]),
        ("superman", &[2, 3][..]),
    ] {
        let record = table
            .stunts
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(stunt))
            .unwrap_or_else(|| panic!("this rider cannot do {stunt}"));
        let name = stunt_name("", &record.name);
        let id = body.rig.set.id(&name).unwrap_or_else(|| panic!("no clip named {name}"));
        let bound = body.rig.set.get(id).expect("a found id is in the set");
        let mut pose = game_render::skin::Pose::new(skel.len());
        let hold = record.hold_time.0.min(bound.clip.timing.duration.0);
        bound.clip.pose(hold, &bound.joints, &mut pose);
        let before = skel.world(&pose);
        let mut after = before.clone();
        let ended = hold_on(&skel, &mut after, &targets, 1.0);

        for (i, limb) in LIMBS.iter().enumerate() {
            let target = targets[i].expect("the chassis has this anchor");
            let anchor = skel.find(limb.anchor).expect("the body has this anchor");
            let m = before[anchor].0[3];
            let was = v3(m[0], m[1], m[2]).sub(target).len();
            let now = ended[i].expect("every limb of this body is solved");
            // How much limb there is between the swing joint and the anchor, and how far the anchor
            // has to go: a holding limb the clip has left stretched past its own length cannot be put
            // on the bike, and what it can do instead is straighten at it.
            let g = |w: &[game_render::mat::M4], j: usize| {
                let m = w[j].0[3];
                v3(m[0], m[1], m[2])
            };
            let (ro, be) = (
                skel.find(limb.root).expect("the body has this joint"),
                skel.find(limb.bend).expect("the body has this joint"),
            );
            let reach = g(&before, be).sub(g(&before, ro)).len()
                + g(&before, anchor).sub(g(&before, be)).len();
            let short = target.sub(g(&before, ro)).len() - reach;
            println!(
                "{stunt} held at {hold:.2}s: {} {was:.3} -> {now:.3}                  (a reach of {reach:.3} and {short:+.3} to make up)",
                limb.anchor
            );
            if !holding.contains(&i) {
                assert!(
                    was > LET_GO_AT && (now - was).abs() < 1e-6,
                    "{stunt} throws {}, and the solve put it back: {was:.3} -> {now:.3}",
                    limb.anchor
                );
            } else if short < -AN_INCH {
                assert!(
                    now < AN_INCH,
                    "{stunt} holds on with {}, which ended {now:.3} out (from {was:.3})",
                    limb.anchor
                );
            } else {
                // `onehandindianair` is the one gesture of the twenty-four where this happens: at the
                // pose it is held in, the grip is two and a half inches past the end of the arm the
                // rider is holding on with, so the solve straightens the arm at it and the hand ends
                // three inches out instead of one. Whether the original reaches it — a third joint
                // at the collar bone, a wrist that turns — is unproved; see the journal.
                assert!(
                    now < short + AN_INCH && now <= was + 1e-3,
                    "{stunt} holds on with {} at full stretch, {short:+.3} short, and the solve                      left it {now:.3} out (from {was:.3})",
                    limb.anchor
                );
            }
        }
    }
}

/// **V18.** A frame of the shell's own driving puts the body on the bike: three seconds of riding,
/// posed by the rider's own sixty-state machine, and the four anchors land on the chassis's inside
/// the same tolerance the posed-rider test measures — not in the bind pose, and not standing up.
///
/// This is the test the task is about, and what it proves is the wiring and not the clips. V8's
/// `a_posed_body_stands_on_the_bike_with_nothing_bolted` already measures the clips: posed by
/// `turn_riding_Mid` the hands are on the grips and the feet on the pegs. What the shell drew instead
/// was a body posed from `Rider::animation` — the *bike's* 28-slot driver — whose grounded idle is
/// `air_Mid` held at its midpoint and whose pose grid a rider's set does not even name, so the rider
/// stood upright on a bike that was being ridden. So the measurement here is the same one, at the end
/// of a ride this port drove itself: `Body::ride` per frame, nothing else writing the pose.
///
/// The throttle is held because that is what makes a rider ride. The body's own start sequence is the
/// gate's — a get-ready idle, a breathe, a start pose — and the throttle opening is the only thing in
/// the machine that ends it (`game_sim::rider_anim`, the gate), so a ride with the throttle shut is a
/// body still waiting at a line, which is a pose and not this one.
#[test]
fn a_frame_of_the_shells_own_driving_poses_the_body_on_the_bike() {
    let Some(mut disc) = disc() else { return };
    // The chassis, for the four places it wants the body's hands and feet. Read before the rider's
    // archive, which takes the disc mutably.
    let bike = member(&disc, &format!("{A_BIKE}.res")).expect("the disc has no bike archive");
    let chassis = disc.res(&bike).and_then(|r| r.read_suffix("bike.psg")).expect("the chassis");
    let chassis = Skeleton::of(&Model::parse(&chassis).expect("the chassis parses"));

    let set = stunts(&mut disc);
    let tables = rider::Tables::build();
    let p = phy(&mut disc, &tables);
    let mut body = body(&mut disc, &set, &p);

    // Every one of the sixty states has a clip, which is the first half of being paired with the
    // right machine: twelve of them would be a bike's set under the body's table (V13).
    let bound = body.pose_lib.clip.iter().filter(|c| c.is_some()).count();
    assert_eq!(bound, game_sim::vehicle_anim::STATES, "the body's own clip table is {bound} of 60");

    // The rider the shell builds: a foot above a flat floor, pointed down +z, with the body's own
    // eight numbers filled in as V12 measured them. Wrecks off — a wrecked body is thrown off the
    // bike on purpose and its anchors are nowhere near it, and what is being measured here is a
    // riding pose.
    let mounts = (p.front.body_tire_force_position, p.rear.body_tire_force_position);
    let mut r = rider::Rider::new(
        &p,
        mounts,
        Vf::new(0.0, 1.0, 0.0, 1.0),
        Vf::new(0.0, 0.0, 1.0, 0.0),
        set.clone(),
    );
    r.sense.wrecks_enabled = false;
    r.body_rates = game_render::vehicle::params();
    let bind = body.rig.world();

    let mut floor = Flat::default();
    let hands = Controls { gas: F::ONE, ..Controls::default() };
    let mut states: Vec<i32> = vec![r.body.veh.want];
    // Three seconds of it, which is where the task measured the shell: the ride's own frame, and then
    // the body's own frame off what it wrote — the two calls the shell makes, in the shell's order.
    let mut last = None;
    for _ in 0..3 * 60 {
        r.frame(&p, &tables, &hands, FRAME, &mut floor);
        last = Some(body.ride(FRAME, &mut r, &hands, rider::Gate::default()));
        if states.last() != Some(&r.body.veh.want) {
            states.push(r.body.veh.want);
        }
    }
    let clips: Vec<String> = last
        .expect("a frame")
        .blend
        .clips()
        .iter()
        .map(|c| match c {
            Some(id) => body.rig.set.get(*id).map_or("?".into(), |b| b.name.clone()),
            None => "-".into(),
        })
        .collect();
    println!(
        "states {states:?}, blend {clips:?}, phase {:?} mix {:?} band {:?} speed {:?}",
        r.body.veh.phase.value, r.body.veh.mix.value, r.body_request.band, r.ride.chassis.speed
    );
    assert!(body.posed, "three seconds of riding never wrote a pose");
    assert!(r.state.wreck.is_none(), "the ride wrecked, so there is no riding pose to measure");
    assert!(r.ride.air.on_ground, "the bike left the ground, so this is not the grounded pose");
    assert_eq!(
        r.body.veh.want,
        game_sim::vehicle_anim::TURN_RIDING,
        "the body is in {:?} and not the pose grid",
        r.body.veh.want
    );

    // The same four anchors the posed test measures, against the same chassis, with the same rule:
    // the body is drawn at the bike's own transform, so the distance between the two skeletons'
    // anchors in the bike's space is the whole of what "the rider is on the bike" means.
    let anchors: Vec<&str> = FOOT_ANCHORS.iter().chain(HAND_ANCHORS.iter()).copied().collect();
    let apart = |world: &[game_render::mat::M4], name: &str| -> f32 {
        let i = body.rig.set.skeleton.find(name).expect("the rider has this anchor");
        let at = world[i].0[3];
        let want = chassis.at(name).expect("the chassis has this anchor");
        ((at[0] - want.x).powi(2) + (at[1] - want.y).powi(2) + (at[2] - want.z).powi(2)).sqrt()
    };
    let world = body.rig.world();
    let worst_bind = anchors.iter().map(|n| apart(&bind, n)).fold(0.0, f32::max);
    let worst = anchors.iter().map(|n| apart(&world, n)).fold(0.0, f32::max);
    for name in &anchors {
        println!("  {name}: {:.3} from the chassis's, bind {:.3}", apart(&world, name), apart(&bind, name));
    }
    // The tolerance is the posed test's own, and so is the comparison with the bind pose: a body
    // still in the pose it was modelled in misses the bike by five inches.
    assert!(worst < 1.5, "three seconds in, an anchor is {worst} from the chassis's");
    assert!(worst < worst_bind, "the ride is no closer to the bike than the bind pose");
    // And it is not the bind pose by any measure: every joint has moved.
    let moved = world
        .iter()
        .zip(bind.iter())
        .map(|(a, b)| {
            let (x, y) = (a.0[3], b.0[3]);
            ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt()
        })
        .fold(0.0, f32::max);
    assert!(moved > AN_INCH, "the pose is the bind pose: the furthest joint moved {moved}");
    println!(
        "three seconds of the shell's own driving: states {states:?}, {:.1} mph, \
         the furthest anchor {worst:.3} from the chassis's (bind {worst_bind:.3}), \
         the furthest joint {moved:.3} off its bind place",
        r.ride.chassis.speed.0 * 0.681_818
    );
}
