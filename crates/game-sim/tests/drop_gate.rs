//! P313c: the drop gate's sink — its pieces' names, its trigger off the race's cards, the group's
//! height tick by tick, and the collision pieces moved to the identity.

use game_sim::drop_gate::{DOWN, Sink, collision_names, gathered, soultree_names};
use game_sim::math::Mat4;
use game_sim::race::{self, Entry, LapCard, Laps, PathNode, RaceCard, RacePath, Spot, TrackPaths};
use game_sim::track::{Mesh, Node, Placement, Points, Triangle};
use game_sim::world::{Prop, Props};
use ps2_float::{F, Vf};

#[test]
fn the_pieces_are_named_off_the_gate_name() {
    // nat19 (and nat12, nat16's names): two scene pieces, the collision one numbered after them.
    assert_eq!(soultree_names("Nat19_Dropgate01", 2), ["Nat19_Dropgate01", "Nat19_Dropgate02"]);
    assert_eq!(collision_names("Nat19_Dropgate01", 2, 1), ["Nat19_Dropgate03.col"]);
    // sx17 and sx18: two of each.
    assert_eq!(collision_names("THQsxdropgate01", 2, 2), ["THQsxdropgate03.col", "THQsxdropgate04.col"]);
    // nat13: one scene piece, the whole name, and no cut before the collision number either.
    assert_eq!(soultree_names("Nat03dropgate", 1), ["Nat03dropgate"]);
    assert_eq!(collision_names("Nat03dropgate", 1, 1), ["Nat03dropgate02.col"]);
    // nat09: no collision pieces.
    assert!(collision_names("Nat09_dropgate01", 1, 0).is_empty());
    // From ten the number has no leading zero; a name of two characters is not cut.
    assert_eq!(collision_names("gate01", 9, 2), ["gate10.col", "gate11.col"]);
    assert_eq!(soultree_names("ab", 2), ["ab01", "ab02"]);
}

#[test]
fn the_collision_pieces_are_gathered_by_file_name_ignoring_case() {
    let files = ["fence.col", "nat19_dropgate03.col", "Nat19_Dropgate02.col", "NAT19_DROPGATE03.COL"];
    assert_eq!(gathered(files, &collision_names("Nat19_Dropgate01", 2, 1)), [1, 3]);
    // nat12 and nat19 want piece 03, which their scenes do not have: nothing is gathered.
    assert!(gathered(["Nat19_Dropgate02.col"], &collision_names("Nat19_Dropgate01", 2, 1)).is_empty());
}

/// One path along +z, a node every 10 ft from 0 to 100, the line at the node at 90.
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
    let course = race::Course { line: race::Gate::new(), line_spot: line(), lap_length: F(100.0) };
    Laps { rules: race::supercross_rules(2), course, lists: race::LineLists::default(), cards: vec![card(0), card(1)], track: std::sync::Arc::new(track) }
}

/// The straight track's line spot, the course's.
fn line() -> Spot {
    Spot::on(0, 9, F::ZERO)
}

fn card(player: usize) -> Entry {
    Entry { player, card: RaceCard { lap: LapCard::default(), ..RaceCard::default() }, stats: race::Tracker::default() }
}

fn at(e: &mut Entry, kept: f32, now: f32) {
    e.card.lap.to_line_kept = F(kept);
    e.card.lap.to_line = F(now);
}

#[test]
fn the_trigger_is_the_lower_gates_spot_measured_to_the_line() {
    let mut laps = straight_laps();
    // Nearest the point is halfway from the node at 30 to the next: 55 ft on to the line at 90.
    let sink = Sink::bind(&mut laps, line(), Vf::new(3.0, 0.0, 35.0, 1.0), true, F(0.25), vec![]);
    assert_eq!(sink.trigger, F(55.0));
    assert!(sink.armed && !sink.on);
    assert_eq!(laps.track.free_routes, 10, "the search gives its pool entries back");
    // No paths: the port measures 0.
    let mut bare = Laps { track: std::sync::Arc::new(TrackPaths::default()), ..straight_laps() };
    assert_eq!(Sink::bind(&mut bare, line(), Vf::new(3.0, 0.0, 35.0, 1.0), true, F(0.25), vec![]).trigger, F::ZERO);
}

#[test]
fn the_first_card_past_the_trigger_sinks_the_gate_toward_minus_100() {
    let mut laps = straight_laps();
    let mut sink = Sink::bind(&mut laps, line(), Vf::new(0.0, 0.0, 35.0, 1.0), true, F(0.25), vec![]);
    let dt = F(1.0 / 30.0);
    // Short of the trigger, and on it (not nearer than it): nothing.
    at(&mut laps.cards[0], 60.0, 56.0);
    at(&mut laps.cards[1], 60.0, 55.0);
    assert!(!sink.tick(dt, &laps.cards));
    assert!(sink.armed);
    // A kept spot no farther than the trigger does not count either.
    at(&mut laps.cards[1], 55.0, 50.0);
    assert!(!sink.tick(dt, &laps.cards));
    // The second card past it: the sink starts, and steps on the same tick from a clock of 0.
    at(&mut laps.cards[1], 60.0, 54.0);
    assert!(sink.tick(dt, &laps.cards));
    assert!(!sink.armed && sink.on);
    assert_eq!((sink.count, sink.length), (1, F(0.25)));
    assert_eq!(sink.y.0.to_bits(), (-0.0f32).to_bits(), "-100 × 0 is a negative zero");
    // Each tick steps from the height it holds: about −50 at the end of its time, then −100.
    let mut ys = vec![sink.y.0];
    while sink.tick(dt, &laps.cards) {
        ys.push(sink.y.0);
    }
    let want = [0.0, -13.333, -23.111, -30.756, -36.930, -42.046, -46.363, -50.061, -100.0];
    assert_eq!(ys.len(), want.len(), "{ys:?}");
    for (got, want) in ys.iter().zip(want) {
        assert!((got - want).abs() < 2e-3, "{ys:?}");
    }
    assert_eq!(sink.y, DOWN);
    assert!(!sink.on);
    // Disarmed: another card past the trigger starts nothing.
    at(&mut laps.cards[0], 60.0, 10.0);
    assert!(!sink.tick(dt, &laps.cards));
    assert_eq!(sink.count, 1);
}

#[test]
fn a_gate_that_does_not_move_down_only_disarms() {
    let mut laps = straight_laps();
    let mut sink = Sink::bind(&mut laps, line(), Vf::new(0.0, 0.0, 35.0, 1.0), false, F(0.25), vec![]);
    at(&mut laps.cards[0], 60.0, 50.0);
    assert!(!sink.tick(F(1.0 / 30.0), &laps.cards));
    assert!(!sink.armed && !sink.on);
    assert_eq!(sink.y, F::ZERO);
}

/// A plank 20 ft wide, standing at `z`.
fn plank(z: f32) -> Prop {
    let v = |x: f32, z: f32| [F(x), F::ZERO, F(z)];
    let mesh = Mesh {
        version: 2,
        surface: None,
        transform: [[F::ONE, F::ZERO, F::ZERO], [F::ZERO, F::ONE, F::ZERO], [F::ZERO, F::ZERO, F::ONE], [F::ZERO; 3]],
        vertices: Points::Wide(vec![v(-10.0, -1.0), v(10.0, -1.0), v(10.0, 1.0), v(-10.0, 1.0)]),
        normals: None,
        nodes: vec![Node { volume: F::ZERO, centre: [F::ZERO; 3], half_extent: [F(10.0), F(0.5), F(1.0)], children: [0x8000, 0x8001] }],
        triangles: vec![
            Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices: [0, 2, 1] },
            Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices: [0, 3, 2] },
        ],
    };
    let place = Placement {
        file: "x.col".into(),
        position: [F(5.0), F(90.0), F(z)],
        look: [F::ZERO, F::ZERO, F::ONE],
        up: [F::ZERO, F::ONE, F::ZERO],
        kind: None,
        ref_name: None,
    };
    Prop::place(mesh, &place)
}

#[test]
fn a_sink_tick_stands_its_collision_pieces_at_the_identity() {
    let mut props = Props { props: vec![plank(10.0), plank(20.0), plank(30.0)] };
    let kept = props.props[0].to_world;
    let mut laps = straight_laps();
    let mut sink = Sink::bind(&mut laps, line(), Vf::new(0.0, 0.0, 35.0, 1.0), true, F(0.25), vec![1, 2, 7]);
    at(&mut laps.cards[0], 60.0, 50.0);
    assert!(sink.tick(F(1.0 / 30.0), &laps.cards));
    sink.stand(&mut props);
    assert_eq!(props.props[0].to_world, kept, "a piece not gathered stays");
    for p in &props.props[1..] {
        assert_eq!(p.to_world, Mat4::IDENTITY);
        assert_eq!(p.from_world, Mat4::IDENTITY);
        assert_eq!((p.centre.x(), p.centre.y(), p.centre.z()), (0.0, 0.0, 0.0));
    }
}

// ---------------------------------------------------------------------------------------------------
// C40: live proofs on sx17 (research/c40, journal 2026-11-17-c40-drop-gate-live). The capture and the
// start state are git-ignored; each test skips without its file.

fn context(rel: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context").join(rel)
}

/// The capture's rows as name → text.
fn capture_rows(rel: &str) -> Option<Vec<std::collections::HashMap<String, String>>> {
    let Ok(text) = std::fs::read_to_string(context(rel)) else {
        ps2_oracle::skip(&format!("no context/{rel} — research/c40/capture.sh records it"));
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(str::to_string).collect();
    Some(lines.map(|l| head.iter().cloned().zip(l.split(',').map(str::to_string)).collect()).collect())
}

fn bits(row: &std::collections::HashMap<String, String>, name: &str) -> u32 {
    let v = &row[name];
    v.parse::<u32>().or_else(|_| v.parse::<i32>().map(|i| i as u32)).unwrap_or_else(|_| v.parse::<f32>().unwrap().to_bits())
}

fn float(row: &std::collections::HashMap<String, String>, name: &str) -> F {
    F(row[name].parse::<f32>().unwrap())
}

/// C40: sx17's AI-ridden race, every tick from the start until the gate is down and a while after,
/// replayed through [`Sink::tick`] on the captured cards (list order) and dt: the counter, armed, the
/// clock and the height match bit for bit on every row. The first card past the trigger fires the
/// sink on the very row its pair crosses, so the gate runs after the score manager's card work in a
/// frame. The two collision pieces' matrices (`+0x1a0`) go from their placement to the identity on
/// that row.
#[test]
fn sx17s_live_sink_replays_tick_for_tick() {
    let Some(rows) = capture_rows("capture/c40/sx17.csv") else { return };
    let first = &rows[0];
    let mut laps = straight_laps();
    laps.cards = (0..4).map(card).collect();
    let mut sink = Sink::bind(&mut laps, line(), Vf::new(0.0, 0.0, 35.0, 1.0), true, F(f32::from_bits(0x3e80_0000)), vec![]);
    sink.trigger = float(first, "trigger");
    assert_eq!(float(first, "trigger").0, 2475.7615, "sx17's trigger, the bind's measure (see the image test)");
    let mut fired = None;
    for (i, r) in rows.iter().enumerate() {
        if bits(r, "in_game") != 1 || bits(r, "gate") == 0 {
            continue;
        }
        for k in 0..4 {
            at(&mut laps.cards[k], float(r, &format!("card{k}_kept")).0, float(r, &format!("card{k}_now")).0);
        }
        let was = sink.armed;
        sink.tick(float(r, "dt"), &laps.cards);
        let want = (bits(r, "count"), bits(r, "armed"), bits(r, "sink_on"), bits(r, "sink_t"), bits(r, "gate_y_bits"));
        let got = (sink.count as u32, sink.armed as u32, sink.on as u32, sink.t.0.to_bits(), sink.y.0.to_bits());
        assert_eq!(got, want, "row {i} (frame {}): count, armed, on, t, y", r["frame_counter"]);
        if was && !sink.armed {
            fired = Some(i);
        }
    }
    let fired = fired.expect("the capture runs past the trigger");
    // The heights from the firing row on: -0, then the steps, -100 on the ninth.
    let ys: Vec<u32> = rows[fired..fired + 10].iter().map(|r| bits(r, "gate_y_bits")).collect();
    assert_eq!(ys, [0x8000_0000, 0xc155_4fdf, 0xc1b8_df8d, 0xc1f6_06af, 0xc213_b62b, 0xc228_2cea, 0xc239_70eb, 0xc248_3c25, 0xc2c8_0000, 0xc2c8_0000]);
    // The collision pieces: their placement before, the identity from the firing row.
    let matrix = |r: &std::collections::HashMap<String, String>, k: usize| -> Vec<f32> {
        let raw: Vec<u8> = (0..r[&format!("col{k}_raw")].len() / 2).map(|j| u8::from_str_radix(&r[&format!("col{k}_raw")][2 * j..2 * j + 2], 16).unwrap()).collect();
        raw[0x80..0xc0].chunks(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect()
    };
    let identity: Vec<f32> = (0..16).map(|j| if j % 5 == 0 { 1.0 } else { 0.0 }).collect();
    for k in 0..2 {
        assert_eq!(bits(&rows[fired], &format!("col{k}_node")), 0, "a static collision object has no node");
        let before = matrix(&rows[fired - 1], k);
        assert_ne!(before, identity);
        assert!(before[12..15].iter().zip([424.0525, 90.055, 91.0698]).all(|(a, b)| (a - b).abs() < 1e-3), "col{k}'s placement before the sink: {before:?}");
        for r in &rows[fired..] {
            assert_eq!(matrix(r, k), identity, "col{k} at frame {}", r["frame_counter"]);
        }
    }
}

/// The path spot's node index from the loaded track object's node list (path `+0x10`, next `+0x54`).
fn loaded_spot(ee: &ps2_oracle::Ee, slots: &[u32], at: u32) -> Spot {
    let (path, node) = (ee.u32_at(at), ee.u32_at(at + 4));
    let along = F(f32::from_bits(ee.u32_at(at + 8)));
    let i = slots.iter().position(|&s| s == path && s != 0).expect("the line's path is a loaded slot");
    let (mut n, mut k) = (ee.u32_at(path + 0x10), 0u32);
    while n != 0 && n != node && k < 10_000 {
        n = ee.u32_at(n + 0x54);
        k += 1;
    }
    assert_eq!(n, node, "the line's node is on its path");
    Spot { path: i as u32 + 1, piece: if node == 0 { 0 } else { k + 1 }, along }
}

/// sx17's race file off the disc (the `.res` under `Tracks\` that holds `tracks\sx17\sx17.tdf`).
fn sx17_race_file() -> Option<Vec<u8>> {
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
    for e in psr.entries.iter().filter(|e| e.name.to_ascii_lowercase().starts_with("tracks\\sx17") && e.name.ends_with(".res")) {
        f.seek(SeekFrom::Start(e.at)).expect("seek");
        let mut bytes = vec![0u8; e.size as usize];
        f.read_exact(&mut bytes).expect("read");
        let res = game_sim::archive::Res::parse(&bytes).expect("the archive's layout");
        if let Some(m) = res.find("tracks\\sx17\\sx17.tdf") {
            return Some(res.read(&bytes, m).expect("inflate the race file"));
        }
    }
    panic!("no tracks\\sx17\\sx17.tdf in MX.PSR");
}

/// C40: sx17's start state (copy 4 slot 36, the moment the world runs): the gate object at activity
/// `+0x3ac` holds the trigger its bind measured. [`Sink::bind`] on sx17's race file, from the image's
/// `[LowerGate]` spot to the image's line spot, measures the same bits. Its two gathered collision
/// objects are the pieces [`collision_names`] names.
#[test]
fn sx17s_trigger_is_bound_as_the_image_holds_it() {
    let Some(file) = sx17_race_file() else { return };
    let ee = match ps2_oracle::Ee::from_ram(&context("states/c40-sx17-race-start.ee.bin")) {
        Ok(ee) => ee,
        Err(why) => return ps2_oracle::skip(&format!("{why} — `./kit state ram` of copy 4's slot 36 (research/c40)")),
    };
    let manager = match ps2_oracle::addr("activity_manager") {
        Ok(a) => a,
        Err(why) => return ps2_oracle::skip(&why),
    };
    let act = ee.u32_at(ee.u32_at(ee.u32_at(manager) + 0x58));
    let gate = ee.u32_at(act + 0x3ac);
    let f = |a: u32| F(f32::from_bits(ee.u32_at(a)));
    let words = ee.u32_at(gate + 0xa8);
    let trigger_at = Vf::new(f(words + 0x10).0, f(words + 0x14).0, f(words + 0x18).0, f(words + 0x1c).0);
    // sx17's `TriggerPosition=604.6,93,704.9`, read as the game reads text: strtod, then the truncating
    // double-to-float (704.9 is the f32 below, not the nearest).
    let text = ["604.6", "93", "704.9"].map(|t| game_sim::phy::number(t).0.to_bits());
    assert_eq!([0, 1, 2].map(|k| trigger_at.0[k].to_bits()), text, "sx17's TriggerPosition {trigger_at:?}");
    assert_ne!(trigger_at.0[2], 704.9, "the rounded read is one bit above the game's");
    let track = ee.u32_at(ee.u32_at(gate + 0xc8) + 0x68);
    let slots: Vec<u32> = (0..ee.u32_at(track + 0x40)).map(|k| ee.u32_at(ee.u32_at(track + 0x50) + 4 * k)).collect();
    // The image's line: the supercross loader's, moved onto the finish gate after the bind.
    let moved = loaded_spot(&ee, &slots, ee.u32_at(track + 0x5c) + 0x94);
    let t = race::TrackFile::parse(&file).expect("sx17's race file reads");
    let (paths, spots) = TrackPaths::load(&t).expect("sx17's track loads");
    let file_line = spots[1].expect("sx17's race file has a line");
    let mut laps = straight_laps();
    laps.track = std::sync::Arc::new(paths);
    laps.course.line_spot = moved;
    let sink = Sink::bind(&mut laps, file_line, trigger_at, true, F(0.25), vec![]);
    // The trigger's spot is the image's (the `[LowerGate]` words keep it at +0).
    let mut spot = Spot::default();
    game_sim::ai::follow::spot(std::sync::Arc::make_mut(&mut laps.track), trigger_at, false, &mut spot, None);
    assert_eq!(spot, loaded_spot(&ee, &slots, words), "the trigger's path spot");
    assert_eq!(sink.trigger.0.to_bits(), ee.u32_at(gate + 0xb0), "the trigger ({:?} vs the image's {:?})", sink.trigger, f(gate + 0xb0));
    assert_eq!(sink.trigger.0, 2475.7615);
    // Measured to the moved line it would be 2.2 ft longer: a tick early at a rider's pace.
    let to_moved = Sink::bind(&mut laps, moved, trigger_at, true, F(0.25), vec![]);
    assert_eq!(to_moved.trigger.0, 2477.9702);
    // The rounded 704.9 lands on the same spot here: the same trigger.
    let rounded = Sink::bind(&mut laps, file_line, Vf::new(604.6, 93.0, 704.9, 1.0), true, F(0.25), vec![]);
    assert_eq!(rounded.trigger.0.to_bits(), sink.trigger.0.to_bits());
    // The gathered collision objects, by the name each one's collidable (+0x34) keeps at +0x8a.
    let list = ee.u32_at(gate + 0x68);
    let names: Vec<String> = (0..ee.u32_at(gate + 0x60))
        .map(|k| {
            let obj = ee.u32_at(ee.u32_at(list + 4 * k) + 0x34);
            (0..64).map(|j| ee.u8_at(obj + 0x8a + j)).take_while(|&b| b != 0).map(char::from).collect()
        })
        .collect();
    let want = collision_names("THQsxdropgate01", 2, 2);
    assert_eq!(names.len(), want.len(), "{names:?}");
    for (got, want) in names.iter().zip(&want) {
        assert!(got.to_ascii_lowercase().ends_with(&want.to_ascii_lowercase()), "{got} vs {want}");
    }
}
