//! **MD41:** a trick in a flip moves the bike as it does out of one, and keeps the rider's hands on
//! the bars.
//!
//! The user's report: with MX vs. ATV's tricks on, a Tsunami or an Indian Air thrown in a front flip
//! or a backflip ended with the hands off the grips — "looks like the bike wont allow movement in
//! flips, we need it to". The rides here are `game-sim/tests/ride.rs`'s flips off the whip tests'
//! ramp, a trick thrown in each, run the way the shell runs them: the ride, the body off it, the bike
//! under the body ([`Chassis::want_under`]) and the body told whether the bike has a clip for its
//! trick ([`Body::bare`]). They skip without the pack in `mods/tricks` or the disc.

use game_render::anim::PackClips;
use game_render::chassis::grips;
use game_render::disc::Disc;
use game_render::mat::{M4, V3, v3};
use game_render::rider::{Body, HAND_ANCHORS, Sex};
use game_render::scene::Vehicle;
use game_render::vehicle::{Chassis, Drive, FRONT, REAR, params};
use game_render::ini;
use game_sim::ride::Controls;
use game_sim::trick::{Set as Stunts24, Stunts};
use game_sim::collide::{Bundle, Record};
use game_sim::world::World;
use game_sim::{phy, phy::Phy, rider, trick_pack};
use ps2_float::{F, Vf};
use std::path::PathBuf;

const A_RIDER: &str = "thq1";
const A_BIKE: &str = "250sx";
const STUNT_ARCHIVE: &str = "appstart.res";
const STUNT_TABLE: &str = "rider\\anim\\stunts.ini";
const FRAME: F = F(1.0 / 60.0);

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
        println!("SKIPPED: no trick pack in {} (openmx-mods' tools/import-tricks makes one)", dir.display());
    }
    found
}

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            println!("SKIPPED: {why}");
            None
        }
    }
}

fn member(disc: &Disc, ends: &str) -> String {
    let want = ends.to_ascii_lowercase();
    disc.entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(&want))
        .min_by_key(|e| e.name.len())
        .map(|e| e.name.clone())
        .unwrap_or_else(|| panic!("the disc has no {ends}"))
}

/// Flat ground with `game-sim/tests/ride.rs`'s flip ramp across +z: 108 feet of run-up, then 25
/// feet at a slope of a half.
struct Ramp;

impl World for Ramp {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let (start, slope, length) = (108.0, 0.5, 25.0);
        let t = at.z() - start;
        if !(0.0..=length).contains(&t) {
            return (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0));
        }
        let n = 1.0 / (1.0f32 + slope * slope).sqrt();
        (Vf::new(at.x(), t * slope, at.z(), 1.0), Vf::new(0.0, n, -slope * n, 0.0))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

struct Kit {
    tables: rider::Tables,
    phy: Phy,
    set: Stunts24,
    body: Body,
    chassis: Chassis,
    bike_lacks: Vec<String>,
}

fn kit() -> Option<Kit> {
    let folder = pack()?;
    let mut disc = disc()?;
    let tables = rider::Tables::build();
    let res = disc.res(&member(&disc, STUNT_ARCHIVE)).expect("the app archive");
    let set = Stunts24::new(Stunts::parse(&res.read_suffix(STUNT_TABLE).unwrap()).unwrap().selection(0));
    let rows = trick_pack::table(&std::fs::read(folder.join("Rider/Anim/MX_Rider_Stunts.ini")).unwrap()).unwrap();
    let extras = trick_pack::extras(&rows, &set);
    let set = set.with_extras(extras);

    let bike = disc.res(&member(&disc, &format!("{A_BIKE}.res"))).expect("the bike archive");
    let phy_entry = bike.find_suffix(".phy").expect("a .phy").name.clone();
    let mut p = Phy::read_bytes(&bike.read(&phy_entry).unwrap(), &tables.ride.curve);
    let pack_phy = std::fs::read_dir(folder.join("Veh"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|f| f.file_name().unwrap().to_string_lossy().to_ascii_lowercase() == trick_pack::phy_name_for(&phy_entry))
        .unwrap_or_else(|| panic!("the pack has no {}", trick_pack::phy_name_for(&phy_entry)));
    let ini = phy::Ini::parse(&std::fs::read_to_string(pack_phy).unwrap());
    let moves = trick_pack::moves(&ini, &rows).expect("the pack's moves");
    trick_pack::apply(&mut p, &ini, moves);

    let clips = PackClips::open(&folder, "MX").unwrap();
    let rider_res = disc.res(&member(&disc, &format!("{A_RIDER}.res"))).expect("the rider archive");
    let mut body = Body::load(vec![rider_res], A_RIDER, Sex::Male, "", &set.stunts, p.motion()).expect("the body");
    body.add_pack(&clips, &set.stunts);
    let sub = Vehicle::parse(&ini::text(&bike.read_suffix(&format!("{A_BIKE}.sub")).unwrap()));
    let mut chassis = Chassis::load(vec![bike], &sub.mcf, "bike_", &set.stunts, params()).expect("the chassis");
    let bike_lacks = chassis.add_pack(&clips, &set.stunts);
    Some(Kit { tables, phy: p, set, body, chassis, bike_lacks })
}

/// How far a drawn hand may be from its grip, in feet, on a tick the draw holds it there: the solve
/// lands it exactly, so this is rounding.
const ON_THE_GRIPS: f32 = 0.05;
/// How far a drawn hand may be from its grip on a tick nothing holds it (V61, the original's rule:
/// its riding ragdoll is not placed while a pose off the clip table is in charge, so the rider is
/// drawn as the clip wrote it): a backflip and the landing out of one, and the few ticks the body
/// fades out of a trick the bike has a clip for. The disc's backflip leaves them 0.17 ft off in the
/// flip and 0.28 ft in the landing, up to 0.34 ft on the fade back into it from a Tsunami, and the
/// fade out of a Tsunami into a front flip's lean 0.15 ft; this is the most of those and a little
/// over.
const AS_WRITTEN: f32 = 0.4;
/// Out of a flip that is not a backflip, the most ticks after a trick that nothing holds the hands:
/// the fade out of the trick (four on the Tsunami's).
const UNHELD_TICKS: usize = 8;
/// How far apart, in feet, a node of the bike drawn in a trick and the same node in a plain flip at
/// the same tick have to be for the bike to be said to move with the trick.
const POSE_APART: f32 = 0.1;

const TSUNAMI: usize = 16;
const INDIAN_AIR: usize = 9;

#[derive(Debug, Clone)]
struct Seen {
    on_ground: bool,
    body: i32,
    bike: i32,
    grips: [V3; 2],
    /// The hands where the clip put them, and where the draw did.
    clip: [V3; 2],
    drawn: [V3; 2],
    /// The draw holds both hands on their grips: the riding hold, or a trick the bike has no clip for.
    held: bool,
    /// The bike's nodes, where the chassis draws them in its own space.
    nodes: Vec<V3>,
}

impl Seen {
    fn clip_off(&self) -> f32 {
        (0..2).map(|i| self.clip[i].sub(self.grips[i]).len()).fold(0.0, f32::max)
    }
    fn drawn_off(&self) -> f32 {
        (0..2).map(|i| self.drawn[i].sub(self.grips[i]).len()).fold(0.0, f32::max)
    }
}

fn at(m: &M4) -> V3 {
    v3(m.0[3][0], m.0[3][1], m.0[3][2])
}

fn into(m: &M4, p: V3) -> V3 {
    let q = p.sub(at(m));
    v3(
        q.x * m.0[0][0] + q.y * m.0[0][1] + q.z * m.0[0][2],
        q.x * m.0[1][0] + q.y * m.0[1][1] + q.z * m.0[1][2],
        q.x * m.0[2][0] + q.y * m.0[2][1] + q.z * m.0[2][2],
    )
}

/// The shell's frame order for `frames` ticks of `script` (each control set from its tick on), the
/// throttle held.
fn ride(kit: &mut Kit, script: &[(usize, Controls)], frames: usize) -> Vec<Seen> {
    let p = &kit.phy;
    let node = |name: &str| kit.chassis.node_rest(name).map(|v| Vf::new(v.x, v.y, v.z, 1.0));
    let mounts = (node(FRONT).expect("a TireFront"), node(REAR).expect("a TireRear"));
    let mut r = rider::Rider::new(p, mounts, Vf::new(0.0, 2.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), kit.set.clone());
    r.sense.wrecks_enabled = false;
    r.body_rates = params();
    let mut world = Ramp;
    let mut now = Controls::default();
    let skel = kit.body.rig.set.skeleton.clone();
    let hands = HAND_ANCHORS.map(|n| skel.find(n).expect("a hand anchor"));
    let root = kit.chassis.nodes.root.expect("a chassis root");
    let mut out = Vec::new();
    for k in 0..frames {
        if let Some((_, c)) = script.iter().find(|(at, _)| *at == k) {
            now = c.clone();
        }
        let c = Controls { gas: F::ONE, ..now.clone() };
        r.frame(p, &kit.tables, &c, FRAME, &mut world);
        kit.body.ride(FRAME, &mut r, &c, rider::Gate::default());
        let want = kit.chassis.want_under(&r.ride, &r.tricks, r.body.veh.want);
        let request = kit.chassis.riding_under(&r, want);
        kit.chassis.frame(FRAME, want, &request, Drive::of(&r.ride));
        kit.body.bare = kit.chassis.bare_under(&r.body.veh);
        let g = grips(&kit.chassis);
        let (drawn, _) = kit.body.palette(Some(&g));
        let clip = kit.body.rig.world();
        let bike = kit.chassis.world[root];
        let grip = |i: usize| into(&bike, g[2 + i].expect("a grip"));
        out.push(Seen {
            on_ground: r.ride.air.on_ground,
            body: r.body.veh.state,
            bike: kit.chassis.stance.driver.state,
            grips: [grip(0), grip(1)],
            clip: hands.map(|j| into(&bike, at(&clip[j]))),
            drawn: hands.map(|j| into(&bike, at(&drawn[j]))),
            held: kit.body.hold == 1.0 || kit.body.bare,
            nodes: kit.chassis.world.iter().map(at).collect(),
        });
        assert!(r.state.wreck.is_none(), "tick {k}: the ride wrecked");
    }
    out
}

fn hands(trick: [bool; 3], stick: (f32, f32)) -> Controls {
    Controls { trick, stunt: trick.iter().any(|&b| b), stick: (F(stick.0), F(stick.1)), ..Controls::default() }
}

const FRAMES: usize = 420;
/// Triangle and circle, the stick up: slot 16 on the disc's table and MX vs. ATV's.
fn tsunami() -> Controls {
    hands([false, true, true], (0.0, -1.0))
}
/// Square and triangle, the stick down: slot 9.
fn indian_air() -> Controls {
    hands([true, true, false], (0.0, 1.0))
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Flip {
    None,
    Back,
    Front,
}

/// `ride.rs`'s flips: back is the stick pulled, pushed and pulled over the lip; front is the same
/// with R1, the pack's modifier, held from the run-up to past the lip.
fn flip(f: Flip) -> Vec<(usize, Controls)> {
    let none = [false; 3];
    let r1 = [false, false, true];
    match f {
        Flip::None => vec![],
        Flip::Back => vec![
            (140, hands(none, (0.0, 1.0))),
            (158, hands(none, (0.0, -1.0))),
            (166, hands(none, (0.0, 1.0))),
            (176, hands(none, (0.0, 0.0))),
        ],
        Flip::Front => vec![
            (130, hands(r1, (0.0, 0.0))),
            (140, hands(r1, (0.0, 1.0))),
            (158, hands(r1, (0.0, -1.0))),
            (176, hands(none, (0.0, 0.0))),
        ],
    }
}

/// `f` with `trick` thrown at tick `at` and let go two ticks later.
fn with_trick(f: Flip, trick: Controls, at: usize) -> Vec<(usize, Controls)> {
    let mut script = flip(f);
    script.retain(|(k, _)| *k != at && *k != at + 2);
    script.push((at, trick));
    script.push((at + 2, hands([false; 3], (0.0, 0.0))));
    script
}

/// The ticks the body ran `slot` on, first and last.
fn span(seen: &[Seen], slot: usize) -> (usize, usize) {
    let on: Vec<usize> = (0..seen.len()).filter(|&k| seen[k].body == slot as i32).collect();
    assert!(on.len() > 10, "the body hardly ran slot {slot} ({} ticks)", on.len());
    (on[0], *on.last().unwrap())
}

fn apart(a: &Seen, b: &Seen) -> f32 {
    a.nodes.iter().zip(&b.nodes).map(|(x, y)| x.sub(*y).len()).fold(0.0, f32::max)
}

/// One trick in one flip, asserted: the hands after the trick, and the bike during it.
/// Each ride on a fresh kit, so the body and the bike start where a ride does.
fn trick_in_a_flip(f: Flip, name: &str, slot: usize, trick: Controls, at: usize) {
    let plain = ride(&mut kit().unwrap(), &flip(f), FRAMES);
    let mut kit = kit().unwrap();
    let seen = ride(&mut kit, &with_trick(f, trick, at), FRAMES);
    let (first, last) = span(&seen, slot);
    let landed = (last..FRAMES).find(|&k| seen[k].on_ground).unwrap_or(FRAMES);
    let has_clip = kit.chassis.stance.lib.clip(slot as i32).is_some();
    let worst_after = seen[last + 1..].iter().map(Seen::drawn_off).fold(0.0, f32::max);
    let moved = (first..=last).map(|k| apart(&seen[k], &plain[k])).fold(0.0, f32::max);
    println!(
        "{name} in {f:?}: ticks {first}..={last}, landed {landed}, the bike {} (moved {moved:.2} ft off the plain flip's), \
         hands at most {worst_after:.2} ft off after, {} ticks unheld",
        if has_clip { "clipped" } else { "bare" },
        seen[last + 1..].iter().filter(|s| !s.held).count(),
    );
    for k in first..=last {
        let s = &seen[k];
        if has_clip {
            assert_eq!(s.bike, s.body, "tick {k}: the bike is not in the {name} the body is in");
        } else {
            assert!(!(0..24).contains(&s.bike), "tick {k}: the bike ran {name}'s empty slot {}", s.bike);
            assert!(
                s.held && s.drawn_off() < ON_THE_GRIPS,
                "tick {k}: {name}'s hands are {:.2} ft off a bike with no clip for it (the clip's {:.2}, bike {})",
                s.drawn_off(),
                s.clip_off(),
                s.bike
            );
        }
    }
    if has_clip {
        assert!(moved > POSE_APART, "{name} in {f:?}: the bike is drawn at most {moved:.3} ft off a plain flip's");
    }
    let unheld = seen[last + 1..].iter().filter(|s| !s.held).count();
    if f != Flip::Back {
        assert!(unheld <= UNHELD_TICKS, "{name} in {f:?}: {unheld} ticks after the trick hold nothing");
    }
    for k in last + 1..FRAMES {
        let s = &seen[k];
        let bound = if s.held { ON_THE_GRIPS } else { AS_WRITTEN };
        assert!(
            s.drawn_off() < bound,
            "{name} in {f:?}, tick {k} ({} after the trick, body {} bike {}, held {}): the hands are {:.2} ft off the grips",
            k - last,
            s.body,
            s.bike,
            s.held,
            s.drawn_off()
        );
    }
    assert!(seen[FRAMES - 1].on_ground && seen[FRAMES - 1].held, "{name} in {f:?}: not riding on at the end");
}

/// The report itself: a Tsunami off a front flip, and one alone, end with the hands on the grips.
#[test]
fn a_tsunami_ends_with_the_hands_on_the_grips() {
    if kit().is_none() {
        return;
    }
    trick_in_a_flip(Flip::Front, "Tsunami", TSUNAMI, tsunami(), 172);
    trick_in_a_flip(Flip::None, "Tsunami", TSUNAMI, tsunami(), 180);
}

/// The user's second: an Indian Air off a front flip, which no game ships a bike clip for.
#[test]
fn an_indian_air_off_a_front_flip_ends_with_the_hands_on_the_grips() {
    if kit().is_none() {
        return;
    }
    trick_in_a_flip(Flip::Front, "Indian Air", INDIAN_AIR, indian_air(), 200);
}

/// The user's third: both in a backflip.
#[test]
fn tricks_in_a_backflip_move_the_bike_and_keep_the_hands() {
    if kit().is_none() {
        return;
    }
    trick_in_a_flip(Flip::Back, "Tsunami", TSUNAMI, tsunami(), 172);
    trick_in_a_flip(Flip::Back, "Indian Air", INDIAN_AIR, indian_air(), 200);
}

/// The pack fills the bike's holes: the backflip from its `bike_backflip`, and the Indian Air not
/// at all — MX vs. ATV marks it not `UsedForBike` and ships no bike clip for it under either name —
/// so the shell's log names it.
#[test]
fn the_pack_fills_the_bikes_backflip_and_names_what_it_cannot() {
    let Some(kit) = kit() else { return };
    let bare = kit.chassis.bare_slots(&kit.set.stunts);
    println!("no bike clip for {bare:?}; the pack's extras lack {:?}", kit.bike_lacks);
    assert!(kit.chassis.stance.lib.clip(game_sim::vehicle_anim::BACKFLIP).is_some(), "no bike backflip from the pack");
    assert!(bare.iter().any(|n| n == "indianair"), "the Indian Air is not named bare: {bare:?}");
    assert!(!bare.iter().any(|n| n == "tsunami"), "the Tsunami is named bare: {bare:?}");
    assert!(kit.chassis.lacks(INDIAN_AIR as i32) && !kit.chassis.lacks(TSUNAMI as i32));
}
