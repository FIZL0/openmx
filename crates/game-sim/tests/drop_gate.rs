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
    let course = race::Course { line: race::Gate::new(), line_spot: Spot::on(0, 9, F::ZERO), lap_length: F(100.0) };
    Laps { rules: race::supercross_rules(2), course, lists: race::LineLists::default(), cards: vec![card(0), card(1)], track: std::sync::Arc::new(track) }
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
    let sink = Sink::bind(&mut laps, Vf::new(3.0, 0.0, 35.0, 1.0), true, F(0.25), vec![]);
    assert_eq!(sink.trigger, F(55.0));
    assert!(sink.armed && !sink.on);
    assert_eq!(laps.track.free_routes, 10, "the search gives its pool entries back");
    // No paths: the port measures 0.
    let mut bare = Laps { track: std::sync::Arc::new(TrackPaths::default()), ..straight_laps() };
    assert_eq!(Sink::bind(&mut bare, Vf::new(3.0, 0.0, 35.0, 1.0), true, F(0.25), vec![]).trigger, F::ZERO);
}

#[test]
fn the_first_card_past_the_trigger_sinks_the_gate_toward_minus_100() {
    let mut laps = straight_laps();
    let mut sink = Sink::bind(&mut laps, Vf::new(0.0, 0.0, 35.0, 1.0), true, F(0.25), vec![]);
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
    let mut sink = Sink::bind(&mut laps, Vf::new(0.0, 0.0, 35.0, 1.0), false, F(0.25), vec![]);
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
    let mut sink = Sink::bind(&mut laps, Vf::new(0.0, 0.0, 35.0, 1.0), true, F(0.25), vec![1, 2, 7]);
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
