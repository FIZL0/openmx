//! **MD21:** MX vs. ATV's trick clips, out of a trick pack (`mods/tricks/<id>/`), read, bound to MX
//! Unleashed's own rider and bike, and played under the pack's extras and 360s.
//!
//! Every test here needs the pack the user's own MX vs. ATV disc makes (openmx-mods'
//! `tools/import-tricks`), in this checkout's `mods/tricks/`. Without one each test says so and
//! passes; the ones that ride also need MX Unleashed's disc.

use game_render::anim::PackClips;
use game_render::disc::Disc;
use game_render::psg::Model;
use game_render::rider::{Body, FOOT_ANCHORS, HAND_ANCHORS, Sex};
use game_render::skin::{Clip, Pose, SKIP, Skeleton};
use game_render::vehicle::{Chassis, params};
use game_render::scene::Vehicle;
use game_render::ini;
use game_sim::ride::Controls;
use game_sim::trick::{EXTRA_SLOTS, SLOTS, Set as Stunts24, Stunts, Trick};
use game_sim::vehicle_anim::{self as va, PACK_EXTRA, SPIN_LEFT, SPIN_RIGHT};
use game_sim::world::Flat;
use game_sim::{phy::Phy, rider, trick_pack};
use ps2_float::{F, Vf};
use std::path::PathBuf;

const A_RIDER: &str = "thq1";
const A_BIKE: &str = "250sx";
const STUNT_ARCHIVE: &str = "appstart.res";
const STUNT_TABLE: &str = "rider\\anim\\stunts.ini";
const FRAME: F = F(1.0 / 60.0);
const DROP: f32 = 30.0;

/// The first trick pack in this checkout's `mods/tricks/`, or `None` having said so.
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

/// The disc's 24 for this rider, with the pack's extras after them: what the shell rides with
/// Tricks on MX vs. ATV.
fn stunts(disc: &mut Disc, pack: &std::path::Path) -> Stunts24 {
    let res = disc.res(&member(disc, STUNT_ARCHIVE)).expect("the app archive");
    let bytes = res.read_suffix(STUNT_TABLE).expect("the stunt table");
    let set = Stunts24::new(Stunts::parse(&bytes).expect("it parses").selection(0));
    let rows = trick_pack::table(&std::fs::read(pack.join("Rider/Anim/MX_Rider_Stunts.ini")).expect("the pack's table"))
        .expect("the pack's table reads");
    let extras = trick_pack::extras(&rows, &set);
    set.with_extras(extras)
}

fn phy(disc: &mut Disc, tables: &rider::Tables) -> Phy {
    let res = disc.res(&member(disc, &format!("{A_BIKE}.res"))).expect("the bike archive");
    Phy::read_bytes(&res.read_suffix(".phy").expect("a .phy"), &tables.ride.curve)
}

fn body(disc: &mut Disc, set: &Stunts24, phy: &Phy) -> Body {
    let res = disc.res(&member(disc, &format!("{A_RIDER}.res"))).expect("the rider archive");
    Body::load(vec![res], A_RIDER, Sex::Male, "", &set.stunts, phy.motion()).expect("the body loads")
}

fn chassis(disc: &mut Disc, set: &Stunts24) -> Chassis {
    let res = disc.res(&member(disc, &format!("{A_BIKE}.res"))).expect("the bike archive");
    let sub = Vehicle::parse(&ini::text(&res.read_suffix(&format!("{A_BIKE}.sub")).expect("the .sub")));
    Chassis::load(vec![res], &sub.mcf, "bike_", &set.stunts, params()).expect("the chassis loads")
}

/// Every `.mot` in the pack parses — v3, v6 and v7 alike, to the end of the file — and its channels
/// keep the disc's own invariants: times increasing from zero, every name index inside the table.
#[test]
fn every_pack_clip_parses() {
    let Some(folder) = pack() else { return };
    let clips = PackClips::open(&folder, "MX").expect("the pack opens");
    let mut versions = std::collections::BTreeMap::<i32, u32>::new();
    for m in clips.members() {
        let bytes = clips.read(&m).unwrap();
        let clip = Clip::parse(&bytes).unwrap_or_else(|e| panic!("{m}: {e}"));
        *versions.entry(clip.version).or_default() += 1;
        assert!(clip.duration() > 0.0, "{m}: a duration of {}", clip.duration());
        assert!(!clip.names.is_empty(), "{m}: no names");
        for c in &clip.rotation {
            assert!(c.keys.len() >= 1, "{m}: a rotation channel with no keys");
            assert_eq!(c.keys.time(0), 0.0, "{m}");
            assert!((1..c.keys.len()).all(|i| c.keys.time(i - 1) < c.keys.time(i)), "{m}: times");
            assert!((0..clip.names.len() as i32).contains(&c.name), "{m}: name {}", c.name);
        }
        for c in &clip.position {
            assert_eq!(c.keys[0].t, 0.0, "{m}");
            assert!((0..clip.names.len() as i32).contains(&c.name), "{m}: name {}", c.name);
        }
    }
    let total: u32 = versions.values().sum();
    println!("{total} pack clips parse, by version {versions:?}");
    assert!(total > 100, "only {total} .mot files in {}", folder.display());
    assert!(versions.contains_key(&6) && versions.contains_key(&7), "no v6 or v7 clip was read: {versions:?}");
}

/// The pack's clips bound to MX Unleashed's rider and bike by channel name: which channels find no
/// joint, per skeleton. The rider's are MX vs. ATV's eighteen body joints; the bike's its six chassis
/// nodes. Printed, and held to: every rider channel binds, and every bike channel but the ones named.
#[test]
fn the_pack_clips_bind_to_the_riders_and_the_bikes_skeletons() {
    let Some(folder) = pack() else { return };
    let Some(mut disc) = disc() else { return };
    let clips = PackClips::open(&folder, "MX").unwrap();
    let rider = disc.res(&member(&disc, &format!("{A_RIDER}.res"))).unwrap();
    let rider = Skeleton::of(&Model::parse(&rider.read_suffix("mrider.psg").unwrap()).unwrap());
    let bike = disc.res(&member(&disc, &format!("{A_BIKE}.res"))).unwrap();
    let bike = Skeleton::of(&Model::parse(&bike.read_suffix("bike.psg").unwrap()).unwrap());
    // The channels that name no joint, and the joints no channel names, over every clip of a kind.
    let unbound = |skel: &Skeleton, want: &str| {
        let (mut left, mut named, mut n) = (std::collections::BTreeSet::new(), std::collections::BTreeSet::new(), 0);
        for m in clips.members().iter().filter(|m| m.to_ascii_lowercase().contains(want)) {
            let clip = Clip::parse(&clips.read(m).unwrap()).unwrap();
            let map = clip.joints(skel);
            for (k, &j) in map.iter().enumerate() {
                match j {
                    SKIP => left.insert(clip.names[k].clone()),
                    j => named.insert(j as usize),
                };
            }
            n += 1;
        }
        let still: Vec<&str> = (0..skel.len()).filter(|j| !named.contains(j)).map(|j| skel.bones[j].name.as_str()).collect();
        (n, left, still.join(" "))
    };
    // The motocross rider's own clips (`Rider\Anim\MX_*`) and bike's (`Veh\Anim\bike_*`).
    let (riders, rider_left, rider_still) = unbound(&rider, "rider\\anim\\mx_");
    let (bikes, bike_left, bike_still) = unbound(&bike, "veh\\anim\\bike_");
    println!("{riders} rider clips: channels with no joint of mrider.psg {rider_left:?}; joints none animates: {rider_still}");
    println!("{bikes} bike clips: channels with no node of {A_BIKE}'s bike.psg {bike_left:?}; nodes none animates: {bike_still}");
    assert!(riders >= 40 && bikes >= 10, "{riders} rider and {bikes} bike clips found");
    // And bound is not the same as in the right place: where each rider clip's first frame puts the
    // hands and feet against the bike's own grips and pegs, as `tests/body.rs` measures the disc's.
    let anchors: Vec<&str> = FOOT_ANCHORS.iter().chain(HAND_ANCHORS.iter()).copied().collect();
    let worst = |clip: &Clip| -> f32 {
        let map = clip.joints(&rider);
        let mut pose = Pose::new(rider.len());
        clip.pose(0.0, &map, &mut pose);
        let world = rider.world(&pose);
        anchors
            .iter()
            .map(|n| {
                let at = world[rider.find(n).unwrap()].0[3];
                let want = bike.at(n).unwrap();
                ((at[0] - want.x).powi(2) + (at[1] - want.y).powi(2) + (at[2] - want.z).powi(2)).sqrt()
            })
            .fold(0.0, f32::max)
    };
    let mut seated: Vec<(f32, String)> = clips
        .members()
        .into_iter()
        .filter(|m| m.to_ascii_lowercase().starts_with("rider\\anim\\mx_"))
        .map(|m| (worst(&Clip::parse(&clips.read(&m).unwrap()).unwrap()), m))
        .collect();
    seated.sort_by(|a, b| a.0.total_cmp(&b.0));
    let disc_rider = disc.res(&member(&disc, &format!("{A_RIDER}.res"))).unwrap();
    let air = worst(&Clip::parse(&disc_rider.read_suffix("air_mid.mot").unwrap()).unwrap());
    println!("first frames, the furthest hand or foot from the bike's (ft): MX Unleashed's air_Mid {air:.2}");
    for (d, m) in &seated {
        println!("  {d:5.2} {m}");
    }
    assert!(seated.len() >= 40, "only {} motocross rider clips found", seated.len());
    // A clip whose frame were MX vs. ATV's and not this bike's would miss by feet; every one of them
    // starts within a third of a foot (MD21's journal has the list).
    assert!(seated.iter().all(|(d, _)| *d < 0.5), "a clip starts off the bike: {:?}", seated.last());
    assert!(rider_left.is_empty(), "rider channels unbound: {rider_left:?}");
    assert!(bike_left.is_empty(), "bike channels unbound: {bike_left:?}");
}

/// The rider and the bike find the pack's extras and the two 360s, and a body without the pack has
/// none of the new states — the sixty are what they were.
#[test]
fn the_body_and_the_chassis_take_the_packs_clips() {
    let Some(folder) = pack() else { return };
    let Some(mut disc) = disc() else { return };
    let tables = rider::Tables::build();
    let set = stunts(&mut disc, &folder);
    let p = phy(&mut disc, &tables);
    let clips = PackClips::open(&folder, "MX").unwrap();

    let plain = body(&mut disc, &set, &p);
    assert!(plain.pose_lib.pack.iter().all(Option::is_none), "a body without the pack has pack clips");
    let mut with = body(&mut disc, &set, &p);
    assert_eq!(with.pose_lib.clip, plain.pose_lib.clip, "the sixty differ before the pack is added");
    let missing = with.add_pack(&clips, &set.stunts);
    assert_eq!(with.pose_lib.clip, plain.pose_lib.clip, "adding the pack moved one of the sixty");
    let extras: Vec<&str> = set.stunts[SLOTS..].iter().map(|s| s.name.as_str()).collect();
    println!("extras {extras:?}; the rider has no clip for {missing:?}");
    assert!(missing.is_empty(), "the rider is missing {missing:?}");
    let member_of = |b: &Body, state: i32| b.rig.set.get(b.pose_lib.clip(state).unwrap()).unwrap().member.clone();
    assert_eq!(extras[0].to_ascii_lowercase(), "9oclockindianair", "Cross up is 9oclockindianair");
    assert!(member_of(&with, PACK_EXTRA).eq_ignore_ascii_case("Rider\\Anim\\MX_9oclockindianair.mot"));
    assert!(member_of(&with, SPIN_LEFT).eq_ignore_ascii_case("Rider\\Anim\\MX_ThreeSixtyL.mot"));
    assert!(member_of(&with, SPIN_RIGHT).eq_ignore_ascii_case("Rider\\Anim\\MX_ThreeSixtyR.mot"));

    let mut bike = chassis(&mut disc, &set);
    let before = bike.stance.lib.clip;
    let missing = bike.add_pack(&clips, &set.stunts);
    assert_eq!(bike.stance.lib.clip, before);
    println!("the bike has no clip for {missing:?}");
    for state in [SPIN_LEFT, SPIN_RIGHT, PACK_EXTRA] {
        assert!(bike.stance.lib.clip(state).is_some(), "the bike has no clip in state {state}");
    }
    assert!(missing.len() < EXTRA_SLOTS, "the bike found almost nothing: {missing:?}");
}

/// What a ride draws: the body's state and the clips of its blend, per frame, and the pose.
struct Drawn {
    states: Vec<i32>,
    clips: Vec<Vec<Option<game_sim::motion::ClipId>>>,
    worlds: Vec<Vec<game_render::mat::M4>>,
}

/// A drop from [`DROP`] feet on the gas — which is what lets the body off the start line — with
/// `hands` held from frame 2 to 20, the rider's body ridden every frame.
fn ride(body: &mut Body, p: &Phy, tables: &rider::Tables, set: &Stunts24, hands: Controls, frames: usize) -> Drawn {
    let mounts = (p.front.body_tire_force_position, p.rear.body_tire_force_position);
    let mut r = rider::Rider::new(p, mounts, Vf::new(0.0, DROP, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), set.clone());
    r.body_rates = params();
    let mut world = Flat::default();
    let mut out = Drawn { states: Vec::new(), clips: Vec::new(), worlds: Vec::new() };
    for k in 0..frames {
        let c = if (2..20).contains(&k) { hands.clone() } else { Controls::default() };
        let c = Controls { gas: F::ONE, ..c };
        r.frame(p, tables, &c, FRAME, &mut world);
        let frame = body.ride(FRAME, &mut r, &c, rider::Gate::default());
        out.states.push(r.body.veh.state);
        out.clips.push(frame.blend.clips());
        out.worlds.push(body.rig.world());
    }
    out
}

/// **The brief's ride:** Cross up in the air poses the rider off `9oclockindianair`'s clip — its
/// own state past the sixty, its clip the blend's — where without the pack's clips the same hands
/// draw the airborne lean.
#[test]
fn cross_up_in_the_air_plays_9oclockindianair() {
    let Some(folder) = pack() else { return };
    let Some(mut disc) = disc() else { return };
    let tables = rider::Tables::build();
    let set = stunts(&mut disc, &folder);
    let p = phy(&mut disc, &tables);
    let clips = PackClips::open(&folder, "MX").unwrap();
    let cross_up = Controls { extra: [true, false], stick: (F::ZERO, F(-1.0)), ..Controls::default() };

    let mut lean = body(&mut disc, &set, &p);
    let without = ride(&mut lean, &p, &tables, &set, cross_up.clone(), 120);
    let mut posed = body(&mut disc, &set, &p);
    posed.add_pack(&clips, &set.stunts);
    let with = ride(&mut posed, &p, &tables, &set, cross_up, 120);
    let id = posed.pose_lib.clip(PACK_EXTRA).unwrap();

    let playing: Vec<usize> = (0..with.states.len()).filter(|&k| with.states[k] == PACK_EXTRA).collect();
    assert!(!playing.is_empty(), "the body never played the extra: states {:?}", with.states);
    assert!(playing.iter().all(|&k| with.clips[k].first() == Some(&Some(id))),
            "the extra's frames blend something else");
    assert!(without.states.iter().all(|&s| s < va::STATES as i32), "a ride without the clips left the sixty");
    assert!(playing.iter().all(|&k| without.states[k] == va::AIR), "without them it is not the airborne lean");
    let k = *playing.last().unwrap();
    let apart = with.worlds[k]
        .iter()
        .zip(&without.worlds[k])
        .map(|(a, b)| (0..3).map(|i| (a.0[3][i] - b.0[3][i]).powi(2)).sum::<f32>().sqrt())
        .fold(0.0, f32::max);
    println!("9oclockindianair: {} frames in state {PACK_EXTRA}, the furthest joint {apart:.2} ft off the lean", playing.len());
    assert!(apart > 0.5, "the clip draws the airborne lean: {apart}");
}

/// The trick ids the pack's two 360s score under are not gesture slots, so nothing but the spin
/// state reaches their clips: a body that is asked for no trick never plays them.
#[test]
fn no_trick_plays_no_pack_clip() {
    let Some(folder) = pack() else { return };
    let Some(mut disc) = disc() else { return };
    let tables = rider::Tables::build();
    let set = stunts(&mut disc, &folder);
    let p = phy(&mut disc, &tables);
    let clips = PackClips::open(&folder, "MX").unwrap();
    let mut posed = body(&mut disc, &set, &p);
    posed.add_pack(&clips, &set.stunts);
    let quiet = ride(&mut posed, &p, &tables, &set, Controls::default(), 120);
    assert!(quiet.states.iter().all(|&s| s < va::STATES as i32), "{:?}", quiet.states);
    assert_eq!(Trick::THREE_SIXTY_RIGHT.slot(), None);
}
