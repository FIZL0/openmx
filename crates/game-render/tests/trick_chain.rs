//! **S214:** a trick started straight out of another fades into it, rider and bike alike.
//!
//! The user, 2026-10-28: "when doing a trick and another one immediately the back snaps to its
//! position for the new trick instead of doing it smoothly like the original". A gesture made while
//! a trick runs is queued and starts as the first one winds down ([`game_sim::trick::Tricks`]), and
//! the body crosses from one clip into the other (the step's stunt cross-fade). The body did; the
//! bike under it did not. Its machine never took the update's latch, so the trick count read as new
//! on every frame of the queued trick and restarted the bike's clip each frame, and the cross-fade
//! started the bike's outgoing clip at phase 0. For a trick the bike has a clip of its own
//! (flatliner, kod, tsunami, ...) the bike, and the rider holding it, jumped 4.5 ft on the switch.
//!
//! Each trick is chained into the next one along, in the shell's own order (body, then the bike
//! under it, then the bike's bare hands), and the frame-to-frame step of the rider's bone rotations
//! and of every drawn joint (rider and bike) over the switch is held to the larger of two steps: the
//! first trick's own over the same frames, played alone, and the cross-fade's, read off the chain a
//! few frames on. A fade between two far-apart poses moves a foot 0.45 ft a frame, frame after
//! frame (Switchblade into No Footer); a snap is one frame's jump with the fade's step either side. Needs MX Unleashed's disc; a trick pack in this checkout's `mods/tricks/`
//! adds its extras to the chain, and without one the chain is the disc's alone.

use game_render::anim::PackClips;
use game_render::disc::{Disc, Res};
use game_render::mat::M4;
use game_render::rider::{Body, Sex};
use game_render::scene::Vehicle;
use game_render::vehicle::{Chassis, Drive, params};
use game_render::{chassis, ini};
use game_sim::replay::Capture;
use game_sim::ride::Controls;
use game_sim::trick::{Dir, Set as Stunts24, Stunts, extra_of, slot_of};
use game_sim::vehicle_anim::PACK_EXTRA;
use game_sim::world::Flat;
use game_sim::{phy::Phy, rider, trick_pack};
use ps2_float::{F, Vf};
use std::path::PathBuf;

const A_RIDER: &str = "thq1";
const A_BIKE: &str = "250sx";
const FRAME: F = F(1.0 / 60.0);
/// The first trick's press, and the second's: well inside the first, so the second is queued.
const FIRST: usize = 10;
const SECOND: usize = 20;
/// How many frames before the switch the first trick played alone is read from, how many from the
/// switch the chain's step is, and how many after those the cross-fade's own step is read over.
const BEFORE: usize = 20;
const AFTER: usize = 3;
const STEADY: usize = 5;
/// How far over the step it is held to the switch may move: the frame after a restart reads two
/// frames of the old clip's advance (the outgoing clip's phase is held on the restart frame, which
/// is the original's arithmetic), so twice, and a little more.
const RATIO: f32 = 2.5;
const SLACK_DEG: f32 = 2.0;
const SLACK_FT: f32 = 0.1;

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            println!("SKIPPED: {why}");
            None
        }
    }
}

/// The first trick pack in this checkout's `mods/tricks/`, if there is one.
fn pack() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../mods/tricks");
    let mut folders: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    folders.sort();
    let found = folders.into_iter().find(|f| {
        std::fs::read_to_string(f.join("mod.json")).is_ok_and(|t| t.contains("\"kind\": \"tricks\""))
    });
    if found.is_none() {
        println!("no trick pack in {}: the disc's tricks alone", dir.display());
    }
    found
}

fn res(disc: &mut Disc, ends: &str) -> Res {
    let want = ends.to_ascii_lowercase();
    let name = disc
        .entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(&want))
        .min_by_key(|e| e.name.len())
        .map(|e| e.name.clone())
        .unwrap_or_else(|| panic!("no {ends} on the disc"));
    disc.res(&name).unwrap()
}

fn buttons_for(group: u8) -> [bool; 3] {
    match group {
        1 => [true, false, false],
        2 => [false, true, false],
        3 => [true, true, false],
        4 => [true, false, true],
        5 => [false, true, true],
        6 => [true, true, true],
        _ => [false; 3],
    }
}

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

/// The button group and direction that make slot `slot`: groups 1..=6 the disc's 24, the two extra
/// buttons (7, 8) a pack's.
fn gesture_for(slot: usize) -> (u8, Dir) {
    let dirs = [Dir::Up, Dir::Down, Dir::Left, Dir::Right];
    for group in 1..=6u8 {
        for dir in dirs {
            if slot_of(group, dir) == Some(slot) {
                return (group, dir);
            }
        }
    }
    for group in 7..=8u8 {
        for dir in dirs {
            if slot >= 24 && extra_of(group, dir) == Some(slot - 24) {
                return (group, dir);
            }
        }
    }
    panic!("no gesture makes slot {slot}")
}

fn state_of(slot: usize) -> i32 {
    if slot < 24 { slot as i32 } else { PACK_EXTRA + (slot as i32 - 24) }
}

fn hands(gesture: Option<(u8, Dir)>) -> Controls {
    match gesture {
        Some((group, dir)) => Controls {
            gas: F::ONE,
            trick: buttons_for(group),
            extra: [group == 7, group == 8],
            dpad: dpad_for(dir),
            ..Controls::default()
        },
        None => Controls { gas: F::ONE, ..Controls::default() },
    }
}

/// The angle between two unit quaternions, in degrees.
fn degrees(a: [f32; 4], b: [f32; 4]) -> f32 {
    let d = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]).abs().min(1.0);
    (2.0 * d.acos()).to_degrees()
}

fn moved(a: &M4, b: &M4) -> f32 {
    let (a, b) = (a.0[3], b.0[3]);
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn a_trick_out_of_another_fades_into_it() {
    let Some(mut disc) = disc() else { return };
    let app = res(&mut disc, "appstart.res");
    let mut set = Stunts24::new(
        Stunts::parse(&app.read_suffix("rider\\anim\\stunts.ini").unwrap()).unwrap().selection(0),
    );
    let clips = pack().map(|folder| {
        let rows = trick_pack::table(&std::fs::read(folder.join("Rider/Anim/MX_Rider_Stunts.ini")).unwrap()).unwrap();
        set = set.clone().with_extras(trick_pack::extras(&rows, &set));
        PackClips::open(&folder, "MX").unwrap()
    });
    let tables = rider::Tables::build();
    let bike = res(&mut disc, &format!("{A_BIKE}.res"));
    let phy = Phy::read_bytes(&bike.read_suffix(".phy").unwrap(), &tables.ride.curve);
    let sub = Vehicle::parse(&ini::text(&bike.read_suffix(&format!("{A_BIKE}.sub")).unwrap()));
    let rider_res = res(&mut disc, &format!("{A_RIDER}.res"));
    let mut body = Body::load(vec![rider_res], A_RIDER, Sex::Male, "", &set.stunts, phy.motion()).unwrap();
    if let Some(clips) = &clips {
        body.add_pack(clips, &set.stunts);
    }

    let chain: Vec<usize> = (0..set.stunts.len())
        .filter(|&k| !set.stunts[k].name.is_empty() && k != set.no_hander && k != set.throwaway)
        .collect();
    // One ride: `a` pressed at FIRST and, given one, `b` at SECOND. The frame-to-frame step of every
    // frame (the largest bone turn, the largest drawn joint's move) and the step the second trick's
    // state came on.
    let mut ride = |a: usize, b: Option<usize>, frames: usize| {
        let mounts = (phy.front.body_tire_force_position, phy.rear.body_tire_force_position);
        let mut r = rider::Rider::new(&phy, mounts, Vf::new(0.0, 400.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), set.clone());
        r.sense.wrecks_enabled = false;
        r.body_rates = params();
        let mut bike = Chassis::load(vec![res(&mut disc, &format!("{A_BIKE}.res"))], &sub.mcf, "bike_", &set.stunts, params()).unwrap();
        if let Some(clips) = &clips {
            bike.add_pack(clips, &set.stunts);
        }
        let mut world = Flat::default();
        let mut last: Option<(Vec<[f32; 4]>, Vec<M4>)> = None;
        let mut steps: Vec<(f32, f32)> = Vec::new();
        let mut switch = None;
        for k in 0..frames {
            let pad = hands(match (k, b) {
                (FIRST, _) => Some(gesture_for(a)),
                (SECOND, Some(b)) => Some(gesture_for(b)),
                _ => None,
            });
            r.frame(&phy, &tables, &pad, FRAME, &mut world);
            body.ride(FRAME, &mut r, &pad, rider::Gate::default());
            let want = bike.want_under(&r.ride, &r.tricks, r.body.veh.want);
            let request = bike.riding_under(&r, want);
            bike.frame(FRAME, want, &request, Drive::of(&r.ride));
            body.bare = bike.bare_under(&r.body.veh);
            let rot = body.rig.set.pose.rot.clone();
            let mut drawn = body.palette(Some(&chassis::grips(&bike))).0;
            drawn.extend(bike.world.iter().cloned());
            if let Some((rot0, drawn0)) = &last {
                let turn = rot0.iter().zip(&rot).map(|(x, y)| degrees(*x, *y)).fold(0.0, f32::max);
                let shift = drawn0.iter().zip(&drawn).map(|(x, y)| moved(x, y)).fold(0.0, f32::max);
                steps.push((turn, shift));
                if switch.is_none() && b.is_some_and(|b| r.body.veh.state == state_of(b)) {
                    switch = Some(steps.len() - 1);
                }
            }
            last = Some((rot, drawn));
            if switch.is_some_and(|s| steps.len() >= s + AFTER + STEADY) {
                break;
            }
        }
        (steps, switch)
    };
    let max = |w: &[(f32, f32)]| w.iter().fold((0.0f32, 0.0f32), |m, d| (m.0.max(d.0), m.1.max(d.1)));
    let mut failed = Vec::new();
    let mut worst = (0.0f32, 0.0f32);
    for (i, &a) in chain.iter().enumerate() {
        let b = chain[(i + 1) % chain.len()];
        let (na, nb) = (&set.stunts[a].name, &set.stunts[b].name);
        let (steps, switch) = ride(a, Some(b), 600);
        let Some(s) = switch else {
            failed.push(format!("{na} -> {nb}: the second trick never started"));
            continue;
        };
        assert!(s >= BEFORE, "{na} -> {nb}: switched {s} frames in");
        // The first trick alone over the same frames, a few more for the outgoing clip running on at
        // the new one's clock: what the cross-fade starts from moves this much on its own, and the
        // fade itself this much once the switch is behind it.
        let (alone, _) = ride(a, None, s + AFTER + 4);
        let first = max(&alone[s - BEFORE..]);
        let fade = max(&steps[s + AFTER..]);
        let own = (first.0.max(fade.0), first.1.max(fade.1));
        let at = max(&steps[s..s + AFTER]);
        let ratio = (at.0 / (own.0 * RATIO + SLACK_DEG), at.1 / (own.1 * RATIO + SLACK_FT));
        worst = (worst.0.max(ratio.0), worst.1.max(ratio.1));
        if ratio.0 > 1.0 || ratio.1 > 1.0 {
            failed.push(format!(
                "{na} -> {nb}: the switch turned a bone {:.1}° and moved a joint {:.2} ft in a frame, \
                 against {:.1}° and {:.2} ft a frame in {na} alone or in the fade after",
                at.0, at.1, own.0, own.1
            ));
        }
    }
    println!("{} chained pairs, worst against the bound: {:.2} turn, {:.2} shift", chain.len(), worst.0, worst.1);
    assert!(failed.is_empty(), "{} of {} chains snap:\n{}", failed.len(), chain.len(), failed.join("\n"));
}

/// How far a joint of either skeleton may be from the original's. The port's are its own clips
/// sampled at the original's clock, so this is the sampling's float difference; the worst seen is
/// 1.5°.
const JOINT_DEG: f32 = 2.0;
/// How many ticks into flatliner each skeleton is held from: the rider's fade-in runs 7 ticks, and the
/// bike's 10 in the original.
const BODY_IN: usize = 7;
const BIKE_IN: usize = 10;
/// The body's stunt cross after the switch, in ticks. By its end the bike is back on the original's.
const BODY_CROSS: usize = 24;

/// **S217:** the same switch, filmed in the original: flatliner, and the Taunt queued while it runs.
///
/// `context/capture/s217/run1.csv` is a playback of a generated tape (the S217 journal's
/// `capture/make_tape.py`, which is C8's free09 tape with one jump changed): Triangle with the stick
/// right as the bike leaves the lip, then Circle with the stick up fourteen ticks later. Its rows hold
/// the body's state and trick clock and both skeletons' joints, the rider's and the bike's own. The
/// port rides the same two gestures at the capture's dt, fourteen ticks apart, and each of its ticks
/// is laid on the capture's from the tick flatliner's state came on.
///
/// Held to the original: the state and the trick's elapsed on every tick (the switch comes on the same
/// tick, with flatliner at the same point); the rider's 22 joints within [`JOINT_DEG`] from the end of
/// the fade-in to the landing, across the switch; the bike's within it from the end of its fade-in to
/// the switch, and again from the end of the body's cross on. Not held: the bike over the 24 ticks
/// after the switch. There the original restarts the bike's clip and fades from where the bike was
/// over 10 ticks, and the port crosses it with the body's stunt cross over 24 (B10). The test asserts
/// that the gap is still there, so whoever closes it tightens the window here. The first ticks of
/// flatliner are not held either: the port's rider falls from 400 ft with no jump behind it, so the
/// pose a fade-in starts from is not the original's.
#[test]
fn a_queued_pair_matches_the_original_across_the_switch() {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/capture/s217/run1.csv");
    let Ok(text) = std::fs::read_to_string(&file) else {
        println!("SKIPPED: no {} (S217's capture; the journal's capture/play.sh makes it)", file.display());
        return;
    };
    let Some(mut disc) = disc() else { return };
    let cap = Capture::parse(&text).unwrap();
    let col = |n: &str| cap.columns.iter().position(|c| c == n).unwrap_or_else(|| panic!("no {n} column"));
    let num = |row: usize, n: &str| -> f32 { cap.rows[row][col(n)].parse().unwrap() };
    let quat = |row: usize, joints: &str, j: usize| -> [f32; 4] {
        ["x", "y", "z", "w"].map(|c| num(row, &format!("{joints}{j}.q.{c}")))
    };
    let (a, b) = (7, 0); // flatliner, the Taunt
    let state = |i: usize| num(i, "state") as i32;
    let start = (0..cap.rows.len()).find(|&i| state(i) == state_of(a)).expect("flatliner never started");
    let switch = (start..cap.rows.len()).find(|&i| state(i) == state_of(b)).expect("the Taunt never started");
    let land = (start..cap.rows.len()).find(|&i| num(i, "airborne") == 0.0).expect("never landed");
    assert_eq!(switch - start, 26, "not S217's capture: the switch is {} ticks in", switch - start);
    let dt = F(num(start, "dt"));

    let app = res(&mut disc, "appstart.res");
    let set = Stunts24::new(Stunts::parse(&app.read_suffix("rider\\anim\\stunts.ini").unwrap()).unwrap().selection(0));
    let tables = rider::Tables::build();
    let bike_res = res(&mut disc, &format!("{A_BIKE}.res"));
    let phy = Phy::read_bytes(&bike_res.read_suffix(".phy").unwrap(), &tables.ride.curve);
    let sub = Vehicle::parse(&ini::text(&bike_res.read_suffix(&format!("{A_BIKE}.sub")).unwrap()));
    let rider_res = res(&mut disc, &format!("{A_RIDER}.res"));
    let mut body = Body::load(vec![rider_res], A_RIDER, Sex::Male, "", &set.stunts, phy.motion()).unwrap();
    let mut bike = Chassis::load(vec![bike_res], &sub.mcf, "bike_", &set.stunts, params()).unwrap();
    let mounts = (phy.front.body_tire_force_position, phy.rear.body_tire_force_position);
    let mut r = rider::Rider::new(&phy, mounts, Vf::new(0.0, 400.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), set.clone());
    r.sense.wrecks_enabled = false;
    r.body_rates = params();
    let mut world = Flat::default();

    // Per tick from flatliner's first: the worst rider joint and the worst bike joint against the
    // capture's.
    let mut worst: Vec<(f32, f32)> = Vec::new();
    let mut failed = Vec::new();
    for k in 0..FIRST + (land - start) {
        let pad = hands(match k {
            FIRST => Some(gesture_for(a)),
            k if k == FIRST + 14 => Some(gesture_for(b)),
            _ => None,
        });
        r.frame(&phy, &tables, &pad, dt, &mut world);
        body.ride(dt, &mut r, &pad, rider::Gate::default());
        let want = bike.want_under(&r.ride, &r.tricks, r.body.veh.want);
        let request = bike.riding_under(&r, want);
        bike.frame(dt, want, &request, Drive::of(&r.ride));
        body.bare = bike.bare_under(&r.body.veh);
        if k < FIRST {
            continue;
        }
        let (t, row) = (k - FIRST, start + k - FIRST);
        if r.body.veh.state != state(row) {
            failed.push(format!("tick {t}: state {} against the original's {}", r.body.veh.state, state(row)));
        }
        if (r.tricks.elapsed.0 - num(row, "trick_elapsed")).abs() > 1e-4 {
            failed.push(format!("tick {t}: elapsed {:?} against the original's {}", r.tricks.elapsed, num(row, "trick_elapsed")));
        }
        let off = |joints: &str, mine: &[[f32; 4]]| (0..22).map(|j| degrees(quat(row, joints, j), mine[j])).fold(0.0, f32::max);
        worst.push((off("j", &body.rig.set.pose.rot), off("bj", &bike.stance.set.pose.rot)));
    }
    let (s, l) = (switch - start, land - start);
    let mut held = |from: usize, to: usize, bike: bool| {
        for (t, w) in worst.iter().enumerate().take(to).skip(from) {
            let (off, what) = if bike { (w.1, "bike") } else { (w.0, "rider") };
            if off > JOINT_DEG {
                failed.push(format!("tick {t}: a {what} joint {off:.1}° off the original's"));
            }
        }
    };
    held(BODY_IN, l, false);
    held(BIKE_IN, s, true);
    held(s + BODY_CROSS, l, true);
    let rider_worst = worst[BODY_IN..l].iter().map(|w| w.0).fold(0.0, f32::max);
    let gap = worst[s..s + BODY_CROSS].iter().map(|w| w.1).fold(0.0, f32::max);
    println!("switch {s} ticks in, landing {l}: the rider within {rider_worst:.2}°, the bike {gap:.1}° off after the switch (B10)");
    assert!(failed.is_empty(), "{} misses against the original:\n{}", failed.len(), failed.join("\n"));
    assert!(
        gap > 20.0,
        "the bike after the switch is within {gap:.1}° of the original's: B10's gap is closed, so hold the \
         bike from the switch on"
    );
}
