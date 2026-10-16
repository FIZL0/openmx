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
    AIR_POSES, Body, FOOT_ANCHORS, HAND_ANCHORS, Sex, Skeleton, stunt_name,
};
use game_render::skin::skin;
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
