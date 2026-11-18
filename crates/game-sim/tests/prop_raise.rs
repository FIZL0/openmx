//! V134: the race's bale raiser — its groups picked off the props by name, its spots measured to the
//! line, each group's 50 ft move tick by tick and the bodies placed with it, in a ride and in a
//! snapshot; and sx01's spots measured as the original's image holds them.

use game_sim::biplane::Area;
use game_sim::phy::Ini;
use game_sim::prop_raise::{Due, LIFT, Raiser, Words, gathered, trigger};
use game_sim::prop_set::PropSet;
use game_sim::race::{self, Entry, LapCard, Laps, PathNode, RaceCard, RacePath, Spot, TrackPaths};
use game_sim::track::Placement;
use game_sim::world::Flat;
use game_sim::{Sim, hash, snapshot};
use ps2_float::{F, Vf, fpu};
use std::sync::Arc;

/// A sign on a pin (tests/prop_set.rs's), one body to a collection.
const PCO: &str = "[PhysicsParts]
NumberOfPhysicsParts = 1
NumberOfConstraints = 1
NumberOfIterations = 2
SimulationHz = 60
[PhysicsPart1]
Type = RigidBody
Name = Sign
Weight = 20
GravityScale = 1
ResetTime = 5
SpeedThreshold = 0.5
AngularRateThreshold = 0.5
[Constraint1]
Type = Pin
Name = Sign
PinPosition = 0, 4, 0
BodyPinPosition = 0, 4, 0
ConstrainAngles = 1
MinAngles = -60, -5, -60
MaxAngles = 60, 5, 60
";

const TOY: &str = "[PhysicsCollections]
NumberOfPhysicsCollections = 1
[Collection1]
Name = Sign
";

/// A one-node `.psg` with a box of half extent `(1, 2, 0.1)` about `(0, 2, 0)`.
fn psg() -> Vec<u8> {
    let mut b = vec![0u8; 12 + 224];
    b[..4].copy_from_slice(b"psg\0");
    b[8..12].copy_from_slice(&1u32.to_le_bytes());
    for (k, v) in [0.0f32, 2.0, 0.0, 1.0, 2.0, 0.1].iter().enumerate() {
        let at = 12 + 192 + 4 * k;
        b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    b
}

fn sign(y: f32) -> PropSet {
    let model = Placement { file: "Sign".into(), position: [F(100.0), F(y), F(100.0)], ..Placement::default() };
    let (set, said) = PropSet::load(
        &Ini::parse(TOY),
        "Tracks\\Test\\",
        &[model],
        &mut |p| (p == "Tracks\\Test\\Sign.pco").then(|| PCO.as_bytes().to_vec()),
        &mut |p| (p == "Sign.psg").then(psg),
        Some(Area::new(F(1000.0), F(1000.0), F(1.0), F(1.0))),
    );
    assert!(said.is_empty(), "{said:?}");
    set
}

/// sx01's four groups' kind of names, a sign each: two under the floor, one on it, one neither.
fn bales() -> PropSet {
    let mut set = sign(40.0);
    set.join(sign(41.0), 1);
    set.join(sign(90.0), 2);
    set.join(sign(90.0), 3);
    set.names = Arc::new(["up_bale_single_01", "UP_BALE_triple", "down_bale_triple_01", "bale_single_02"].map(String::from).to_vec());
    set
}

/// One path along +z, a node every 10 ft from 0 to 100, the line at the node at 90 (tests/drop_gate.rs's).
fn straight_laps() -> Laps {
    let nodes = (0..=10)
        .map(|k| {
            let z = 10.0 * k as f32;
            let p = |x: f32| Vf([x, 0.0, z, 0.0]);
            PathNode { id: k, word_04: 0, points: [p(0.0), p(-10.0), p(10.0)], span: if k < 10 { F(10.0) } else { F::ZERO }, word_44: 0, word_48: 0, word_4c: 0 }
        })
        .collect();
    let track = TrackPaths {
        paths: vec![RacePath { flags: 0x41, length: F(100.0), word_0c: 0, nodes, links: vec![0], dist: F::ZERO, best: None }],
        free_routes: 10,
        free_parents: 10,
        free_candidates: 10,
        ..TrackPaths::default()
    };
    let course = race::Course { line: race::Gate::new(), line_spot: Spot::on(0, 9, F::ZERO), lap_length: F(100.0) };
    let card = |player| Entry { player, card: RaceCard { lap: LapCard::default(), ..RaceCard::default() }, stats: race::Tracker::default() };
    Laps { rules: race::supercross_rules(2), course, lists: race::LineLists::default(), cards: vec![card(0), card(1)], track: Arc::new(track) }
}

fn at(e: &mut Entry, kept: f32, now: f32) {
    e.card.lap.to_line_kept = F(kept);
    e.card.lap.to_line = F(now);
}

fn words(z: f32, time: f32, substring: &str) -> Words {
    Words { trigger_at: Vf::new(3.0, 7.0, z, 1.0), move_time: F(time), substring: substring.into() }
}

#[test]
fn the_groups_are_every_body_of_each_collection_whose_name_holds_the_substring() {
    let set = bales();
    assert_eq!(gathered(&set, "up_bale"), [(0, 0)], "case counts: UP_BALE is not up_bale");
    assert_eq!(gathered(&set, "down_bale"), [(2, 0)]);
    assert_eq!(gathered(&set, "bale"), [(0, 0), (2, 0), (3, 0)]);
    assert!(gathered(&set, "crowd").is_empty());
    // A set loaded off a `.toy` names its collections as it made them.
    assert_eq!(sign(0.0).names.as_slice(), ["Sign"]);
    let mut laps = straight_laps();
    assert!(Raiser::bind(&mut laps, Spot::on(0, 9, F::ZERO), &words(35.0, 1.0, "crowd"), &words(35.0, 1.0, "fans"), &set).is_none(), "no body: no raiser");
}

#[test]
fn a_spot_is_put_on_the_paths_and_measured_to_the_line() {
    let mut laps = straight_laps();
    let line = laps.course.line_spot;
    let mut p = Vf::new(3.0, 7.0, 35.0, 1.0);
    // Halfway from the node at 30 to the next: 55 ft on to the line at 90, the path's point written over it.
    assert_eq!(trigger(&mut laps, line, &mut p), F(55.0));
    assert_eq!((p.x(), p.y(), p.z()), (0.0, 0.0, 35.0));
    // Past the path's end: the spot is held to it.
    let mut p = Vf::new(0.0, 0.0, 130.0, 1.0);
    trigger(&mut laps, line, &mut p);
    assert_eq!(p.z(), 100.0);
    let mut bare = Laps { track: Arc::new(TrackPaths::default()), ..straight_laps() };
    assert_eq!(trigger(&mut bare, line, &mut Vf::new(0.0, 0.0, 35.0, 1.0)), F::ZERO, "no paths: 0");
}

#[test]
fn each_group_moves_50_ft_over_its_time_from_the_first_card_past_its_spot() {
    let mut laps = straight_laps();
    let mut set = bales();
    let line = laps.course.line_spot;
    // The up spot 55 ft from the line, the down one 35.
    let mut r = Raiser::bind(&mut laps, line, &words(35.0, 0.1, "up_bale"), &words(55.0, 0.2, "down_bale"), &set).unwrap();
    assert_eq!((r.up.trigger, r.down.trigger), (F(55.0), F(35.0)));
    assert!(r.up.armed && r.down.armed && !r.up.on && !r.down.on);
    let dt = F(1.0 / 30.0);
    let (up0, down0) = (set.collections[0].objects[0].clone(), set.collections[2].objects[0].clone());
    // On the spot, or a kept spot no farther: nothing.
    at(&mut laps.cards[0], 60.0, 55.0);
    at(&mut laps.cards[1], 55.0, 50.0);
    r.tick(dt, &laps.cards);
    assert!(r.due.is_empty() && r.up.armed);
    // The second card past the up spot: the up group starts, placing where it stands on this tick.
    at(&mut laps.cards[1], 56.0, 54.0);
    r.tick(dt, &laps.cards);
    assert!(!r.up.armed && r.up.on && r.down.armed);
    assert_eq!((r.up.count, r.up.length, r.up.t), (1, F(0.1), dt));
    assert_eq!(r.due, [(true, Due::Share(F::ZERO))]);
    r.place(&mut set);
    assert!(r.due.is_empty());
    let body_y = |set: &PropSet, c: usize| set.collections[c].bones[0].body.position.y();
    let stood = body_y(&set, 0);
    // The object's own place, look and up stay: the move is from where it is put back to.
    assert_eq!(set.collections[0].objects[0].place, up0.place);
    assert_eq!((set.collections[0].objects[0].look, set.collections[0].objects[0].up), (up0.look, up0.up));
    // A card past both: the down group starts too, and places first.
    at(&mut laps.cards[0], 40.0, 30.0);
    r.tick(dt, &laps.cards);
    assert_eq!(r.due, [(false, Due::Share(F::ZERO)), (true, Due::Share(F(fpu::div(dt.0, 0.1))))]);
    r.place(&mut set);
    let q = fpu::div(dt.0, 0.1);
    let want = fpu::add(40.0, fpu::mul(fpu::sub(fpu::add(40.0, LIFT.0), 40.0), q));
    assert_eq!(body_y(&set, 0), fpu::add(stood, fpu::sub(want, 40.0)), "a third of the way up");
    // On to the end of the up group's time: it is placed 50 ft up and put back there from then on.
    let mut ups = 0;
    while r.up.on {
        r.tick(dt, &laps.cards);
        ups += r.due.iter().filter(|d| d.0).count();
        r.place(&mut set);
    }
    // Three steps of the f32 1/30 are not short of the f32 0.1.
    assert_eq!(ups, 2, "one more share, then the end");
    assert_eq!(set.collections[0].objects[0].place.y(), 90.0);
    assert_eq!(body_y(&set, 0), fpu::add(stood, 50.0));
    while r.down.on {
        r.tick(dt, &laps.cards);
        r.place(&mut set);
    }
    assert_eq!(set.collections[2].objects[0].place.y(), 40.0, "50 ft down from {:?}", down0.place);
    // Spent: no card sets either off again.
    at(&mut laps.cards[0], 60.0, 10.0);
    r.tick(dt, &laps.cards);
    assert!(r.due.is_empty());
    // The collections the groups do not name are as they were made.
    assert_eq!(set.collections[1], bales().collections[1]);
    assert_eq!(set.collections[3], bales().collections[3]);
}

#[test]
fn a_raiser_rides_in_the_sims_race_and_its_snapshot() {
    let mut laps = straight_laps();
    let set = bales();
    let line = laps.course.line_spot;
    let raiser = Raiser::bind(&mut laps, line, &words(35.0, 0.5, "up_bale"), &words(55.0, 0.5, "down_bale"), &set).unwrap();
    // A card already past both spots: they start on the first tick.
    at(&mut laps.cards[0], 60.0, 30.0);
    let mut sim = Sim::new(7);
    sim.traffic.props = set;
    let mut r = race::Race::new(race::Gate::new(), &[], F(3.0), game_sim::rng::Rng::new());
    r.laps = Some(Box::new(laps));
    let quiet = {
        let mut s = sim.clone();
        s.race = Some(Box::new(r.clone()));
        hash::of(&s)
    };
    r.raiser = Some(Box::new(raiser));
    sim.race = Some(Box::new(r));
    assert_ne!(hash::of(&sim), quiet, "the raiser is in the walk");
    let mut world = Flat::default();
    let dt = F(1.0 / 30.0);
    let y0 = sim.traffic.props.collections[0].bones[0].body.position.y();
    let mut saved = None;
    let mut hashes = Vec::new();
    for t in 0..30 {
        if t == 5 {
            saved = Some(snapshot::encode(&sim));
        }
        sim.step(dt, &Default::default(), &mut world);
        if t >= 5 {
            hashes.push(hash::of(&sim));
        }
    }
    let r = sim.race.as_ref().unwrap().raiser.as_deref().unwrap();
    assert!(!r.up.on && !r.down.on && r.due.is_empty(), "both done in half a second");
    assert_eq!(sim.traffic.props.collections[0].objects[0].place.y(), 90.0);
    assert!(sim.traffic.props.collections[0].bones[0].body.position.y() > y0 + 40.0);
    // From the snapshot's bytes the ride goes on to the same hashes.
    let mut back = Sim::new(7);
    back.traffic.props = bales();
    snapshot::decode(&mut back, &saved.unwrap()).expect("the snapshot decodes");
    for (t, want) in hashes.iter().enumerate() {
        back.step(dt, &Default::default(), &mut world);
        assert_eq!(hash::of(&back), *want, "tick {} after the snapshot", t + 5);
    }
}

fn context(rel: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context").join(rel)
}

/// sx01's race file off the disc (tests/drop_gate.rs's sx17 reader).
fn sx01_race_file() -> Option<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let kit = match ps2_oracle::Kit::load() {
        Ok(k) => k,
        Err(why) => {
            ps2_oracle::skip(&why);
            return None;
        }
    };
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = std::fs::File::open(&at) else {
        ps2_oracle::skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = game_sim::archive::Psr::parse(&front).expect("MX.PSR's directory");
    for e in psr.entries.iter().filter(|e| e.name.to_ascii_lowercase().starts_with("tracks\\sx01") && e.name.ends_with(".res")) {
        f.seek(SeekFrom::Start(e.at)).expect("seek");
        let mut bytes = vec![0u8; e.size as usize];
        f.read_exact(&mut bytes).expect("read");
        let res = game_sim::archive::Res::parse(&bytes).expect("the archive's layout");
        if let Some(m) = res.find("tracks\\sx01\\sx01.tdf") {
            return Some(res.read(&bytes, m).expect("inflate the race file"));
        }
    }
    panic!("no tracks\\sx01\\sx01.tdf in MX.PSR");
}

/// The path spot's node index from the loaded track object's node list (tests/drop_gate.rs's).
fn loaded_spot(ee: &ps2_oracle::Ee, slots: &[u32], at: u32) -> Spot {
    let (path, node) = (ee.u32_at(at), ee.u32_at(at + 4));
    let along = F(f32::from_bits(ee.u32_at(at + 8)));
    let i = slots.iter().position(|&s| s == path && s != 0).expect("the spot's path is a loaded slot");
    let (mut n, mut k) = (ee.u32_at(path + 0x10), 0u32);
    while n != 0 && n != node && k < 10_000 {
        n = ee.u32_at(n + 0x54);
        k += 1;
    }
    assert_eq!(n, node, "the spot's node is on its path");
    Spot { path: i as u32 + 1, piece: if node == 0 { 0 } else { k + 1 }, along }
}

/// C31's sx01 race (four AI riders): the race activity's raiser at `+0x3a8` holds both spots'
/// distances. [`trigger`] on sx01's race file, from `[Funnel_Up]`'s `TriggerPosition` to the image's
/// line — the supercross loader's, moved onto the finish gate — measures the same bits, and writes
/// the same path point over the position; to the race file's own line it would be 1.45 ft longer.
#[test]
fn sx01s_spots_are_measured_as_the_image_holds_them() {
    let Some(file) = sx01_race_file() else { return };
    let ee = match ps2_oracle::Ee::from_ram(&context("states/c31-sx01-ai4.ee.bin")) {
        Ok(ee) => ee,
        Err(why) => return ps2_oracle::skip(&format!("{why} — C31's sx01 race image")),
    };
    let manager = match ps2_oracle::addr("activity_manager") {
        Ok(a) => a,
        Err(why) => return ps2_oracle::skip(&why),
    };
    let act = ee.u32_at(ee.u32_at(ee.u32_at(manager) + 0x58));
    assert_eq!(ee.u32_at(act + 0x3a4), 1, "the race asks for a raiser");
    let raiser = ee.u32_at(act + 0x3a8);
    assert_ne!(raiser, 0, "sx01 has one");
    let f = |a: u32| f32::from_bits(ee.u32_at(a));
    let (up_words, down_words) = (ee.u32_at(raiser + 0x54), ee.u32_at(raiser + 0x40));
    // Both sections have sx01's `MoveTime=1`, and the groups' counts are the shell's (10 up, 17 down).
    assert_eq!((f(up_words + 0x38), f(down_words + 0x38)), (1.0, 1.0));
    assert_eq!((ee.u32_at(raiser + 0x34), ee.u32_at(raiser + 0x3c)), (10, 17));
    let track = ee.u32_at(ee.u32_at(act + 0x160) + 0x68);
    let slots: Vec<u32> = (0..ee.u32_at(track + 0x40)).map(|k| ee.u32_at(ee.u32_at(track + 0x50) + 4 * k)).collect();
    let moved = loaded_spot(&ee, &slots, ee.u32_at(track + 0x5c) + 0x94);
    let t = race::TrackFile::parse(&file).expect("sx01's race file reads");
    let (paths, spots) = TrackPaths::load(&t).expect("sx01's track loads");
    let file_line = spots[1].expect("sx01's race file has a line");
    let mut laps = straight_laps();
    laps.track = Arc::new(paths);
    // sx01's `[Funnel_up]` and `[Funnel_Down]` both say `TriggerPosition=562,120,706`.
    let text = Vf::new(562.0, 120.0, 706.0, 1.0);
    let mut p = text;
    let got = trigger(&mut laps, moved, &mut p);
    for (name, off, words) in [("up", 0x80, up_words), ("down", 0x84, down_words)] {
        assert_eq!(got.0.to_bits(), ee.u32_at(raiser + off), "the {name} spot: {got:?} vs {}", f(raiser + off));
        // The file's nodes have no height until the shell grounds them (there the y is the image's
        // 91.2977 too): x and z.
        assert_eq!([p.x(), p.z()].map(f32::to_bits), [0, 8].map(|k| ee.u32_at(words + 0x10 + k)), "the {name} spot's point {p:?}");
    }
    assert_eq!(got.0, 3369.9512);
    let to_file = trigger(&mut laps, file_line, &mut text.clone());
    assert_eq!(to_file.0, 3371.399, "the race file's own line");
}
