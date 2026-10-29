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
