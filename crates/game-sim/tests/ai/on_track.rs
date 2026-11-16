// ---------------------------------------------------------------------------------------------------
// S331c: the path nets under an AI rider — the route node off the brain's spot on the AI net, and the
// avoidance's edge and the habits' spot off the card's spot on the race track — on sx01's race image.
// ---------------------------------------------------------------------------------------------------

use super::path_branch::{s309_paths, s309_spot};
use super::path_nets::{S282C_RIDERS, S282C_STATE};
use super::*;
use game_sim::ai::{HabitNode, Jump, OnTrack, route_node};
use game_sim::race::Spot;

/// Free RAM in the image for the oracle's point, out and io quads.
const SCRATCH: u32 = 0x01f8_0000;

/// What the three Around hooks read, against sx01's race image (S282c's, three AI riders on the race
/// driver), through the original's own path functions on the image's real nets:
///
/// - **The route node.** Each brain's spot on its AI net (`+0x220`, the net its `+0xdc`) names, through
///   [`route_node`], the very node its `+0x224` points at, and [`Jump::of`] and [`HabitNode::of`] are
///   that node's `+4`, `+0x48` and `+0x44`, `+0x4c` — the words `ai_trick_plan` and `ai_habits` read.
///   Then every node of the AI net the same way, by its spot.
/// - **The edge.** Each brain's card (`+0xd4`) has its `+0x88` path and `+0x8c` node on the race file
///   (the brain's race `+0xd8`, its `+0x68`). Then 10,000 asks of [`OnTrack::edge`] at random spots on
///   that net and random points, against `ai_update_mx`'s keep-on-the-track stretch (`./kit dis
///   0x00387f48 0x00388000`) replayed call for call: `path_locate` (−1, the net, no hint, the point, the
///   path, out), the located segment's next and the one after, `path_segment_holds` on both, then
///   `path_node_direction` at the card's node into a copy of the point.
/// - **The spot.** 10,000 asks of [`OnTrack::spot_point`] at random spots (the along word in and out
///   of 0..1, last nodes among them) against `path_spot_point` on the race file into a vf0 quad, as
///   `ai_habits` hands it; then each card's own spot.
#[test]
fn s331c_the_path_hooks_read_the_nets_as_the_race_image_does() {
    let kit = match ps2_oracle::Kit::load() {
        Ok(k) => k,
        Err(why) => return skip(&why),
    };
    let image = kit.ctx("states").join(S282C_STATE);
    if !image.exists() {
        return skip(&format!("{} is not there (research/c31's race start, S282c)", image.display()));
    }
    let names = ["activity_manager", "boot_gp", "path_locate", "path_segment_holds", "path_node_direction", "path_spot_point"];
    let Some(at) = named(&names) else { return };
    let [manager, gp, locate, holds, direction, spot_point] = at[..] else { unreachable!() };
    let mut ee = ps2_oracle::Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(gp));
    assert!((0..0x40).step_by(4).all(|k| ee.u32_at(SCRATCH + k) == 0), "{SCRATCH:#x} is free in the image");
    let act = ee.u32_at(ee.u32_at(ee.u32_at(manager) + 0x58));
    let (ai_at, race_at) = (ee.u32_at(act + 0xbc), ee.u32_at(act + 0xb8));
    let (ai, ai_recs, ai_nodes) = s309_paths(&ee, ai_at);
    let (race, recs, nodes) = s309_paths(&ee, race_at);
    let net = Net::from_track(&race);

    // The route node: each brain's, then every node of the AI net.
    let words = |ee: &ps2_oracle::Ee, n: u32| [4, 0x44, 0x48, 0x4c].map(|o| ee.u32_at(n + o));
    let mut on_node = 0;
    for b in S282C_RIDERS {
        assert_eq!(ee.u32_at(b + 0xdc), ai_at, "brain {b:#x}: the follower's net");
        let spot = s309_spot(&ee, b + 0x220, &ai_recs, &ai_nodes).expect("the brain's spot is on its net");
        let at = ee.u32_at(b + 0x224);
        let node = route_node(Some(&ai), spot);
        assert_eq!(node.is_some(), at != 0, "brain {b:#x}: a route node");
        if let Some(n) = node {
            on_node += 1;
            assert_eq!([n.word_04, n.word_44, n.word_48, n.word_4c], words(&ee, at), "brain {b:#x}: the route node's words");
            let (j, h) = (Jump::of(n), HabitNode::of(n));
            assert_eq!(
                [j.flags, j.airtime.0.to_bits(), h.flags, h.speed.0.to_bits(), h.gain.0.to_bits()],
                [ee.u32_at(at + 4), ee.u32_at(at + 0x48), ee.u32_at(at + 4), ee.u32_at(at + 0x44), ee.u32_at(at + 0x4c)],
                "brain {b:#x}: the trick plan's and the habits' node"
            );
        }
    }
    assert!(on_node > 0, "no brain is on a node of its net");
    for (p, addrs) in ai_nodes.iter().enumerate() {
        for (n, &a) in addrs.iter().enumerate() {
            let node = route_node(Some(&ai), Spot::on(p, n, F(0.5))).expect("a node by its spot");
            assert_eq!([node.word_04, node.word_44, node.word_48, node.word_4c], words(&ee, a), "AI net path {p} node {n}");
        }
    }
    assert!(route_node(Some(&ai), Spot::default()).is_none() && route_node(None, Spot::on(0, 0, F(0.0))).is_none());

    // Each card's spot on the race file is the edge's path and node.
    for b in S282C_RIDERS {
        let card = ee.u32_at(b + 0xd4);
        assert_eq!(ee.u32_at(ee.u32_at(b + 0xd8) + 0x68), race_at, "brain {b:#x}: the edge's net is the race file");
        let spot = s309_spot(&ee, card + 0x88, &recs, &nodes).expect("the card's spot is on the race file");
        let (p, n) = spot.at().expect("the card names a path and a node");
        assert_eq!(ee.u32_at(ee.u32_at(card + 0x88) + 0x10), nodes[p][0], "brain {b:#x}: the card's path's first node");
        assert_eq!(ee.u32_at(card + 0x8c), nodes[p][n], "brain {b:#x}: the card's node");
        assert!(OnTrack { paths: &race, net: &net, spot }.edge().is_some(), "brain {b:#x}: an edge");
    }

    let (point, out, io) = (SCRATCH, SCRATCH + 0x10, SCRATCH + 0x20);
    let original_edge = |ee: &mut ps2_oracle::Ee, ahead: Vf, path: u32, node: u32| -> Option<[u32; 4]> {
        put(ee, point, ahead);
        put(ee, out, Vf::new(0.0, 0.0, 0.0, 0.0));
        ee.call(locate, &[u64::from(race_at), 0, u64::from(point), u64::from(path), u64::from(out), 0, 0, 0], &[-1.0], BUDGET);
        if ee.v0() == 0 {
            return None;
        }
        let seg = ee.u32_at(out + 4);
        let next = if seg == 0 { 0 } else { ee.u32_at(seg + 0x54) };
        let after = if next == 0 { 0 } else { ee.u32_at(next + 0x54) };
        if after == 0 {
            return None;
        }
        for (a, b) in [(seg, next), (next, after)] {
            ee.call(holds, &[u64::from(race_at), u64::from(point), u64::from(a), u64::from(b)], &[], BUDGET);
            if ee.v0() != 0 {
                return None;
            }
        }
        put(ee, io, ahead);
        ee.call(direction, &[u64::from(race_at), u64::from(node), u64::from(io)], &[], BUDGET);
        Some(got(ee, io))
    };
    let mut pick = Pick(0x5331_c0ed_9e7a_0001);
    let unit = |p: &mut Pick| (p.next() >> 40) as f32 / (1u64 << 24) as f32;
    let (mut pushed, mut held, mut spot_last, mut spot_clamped) = (0, 0, 0, 0);
    for case in 0..10_000 {
        let p = pick.below(race.paths.len() as u32) as usize;
        let n = pick.below(race.paths[p].nodes.len() as u32) as usize;
        let near = race.paths[pick.below(race.paths.len() as u32) as usize].nodes.clone();
        let c = near[pick.below(near.len() as u32) as usize].points[pick.below(3) as usize];
        let (dx, dz) = (unit(&mut pick) * 120.0 - 60.0, unit(&mut pick) * 120.0 - 60.0);
        let ahead = Vf::new(c.x() + dx, c.y() + unit(&mut pick) * 10.0, c.z() + dz, unit(&mut pick));
        let along = F(unit(&mut pick) * 1.4 - 0.2);
        let spot = Spot::on(p, n, along);
        let on = OnTrack { paths: &race, net: &net, spot };
        let what = format!("case {case}: path {p} node {n}, ahead {ahead:?}");

        let mine = on.edge().expect("a spot on a node has an edge")(ahead).map(|v| v.to_bits());
        let theirs = original_edge(&mut ee, ahead, recs[p], nodes[p][n]);
        assert_eq!(mine, theirs, "{what}: the edge");
        if mine.is_some() { pushed += 1 } else { held += 1 }

        let vf0 = Vf::new(0.0, 0.0, 0.0, 1.0);
        let mut mine = vf0;
        on.spot_point()(&mut mine);
        put(&mut ee, out, vf0);
        ee.set_u32(io, recs[p]);
        ee.set_u32(io + 4, nodes[p][n]);
        ee.set_f32(io + 8, along.0);
        ee.call(spot_point, &[u64::from(race_at), u64::from(io), u64::from(out)], &[], BUDGET);
        assert_eq!(mine.to_bits(), got(&ee, out), "{what}: along {along:?}: the spot's point");
        if n + 1 == race.paths[p].nodes.len() { spot_last += 1 }
        if !(0.0..=1.0).contains(&along.0) { spot_clamped += 1 }
    }
    eprintln!("s331c: edge pushed {pushed}, held {held}; spot on a last node {spot_last}, along clamped {spot_clamped}");
    assert!(pushed > 1000 && held > 1000, "both edge answers reached: {pushed} pushed, {held} held");
    assert!(spot_last > 10 && spot_clamped > 1000, "the spot's branches reached: {spot_last} last, {spot_clamped} clamped");

    // Each card's own spot, as the habits ask it.
    for b in S282C_RIDERS {
        let card = ee.u32_at(b + 0xd4);
        let spot = s309_spot(&ee, card + 0x88, &recs, &nodes).expect("the card's spot is on the race file");
        let vf0 = Vf::new(0.0, 0.0, 0.0, 1.0);
        let mut mine = vf0;
        OnTrack { paths: &race, net: &net, spot }.spot_point()(&mut mine);
        put(&mut ee, out, vf0);
        ee.call(spot_point, &[u64::from(race_at), u64::from(card + 0x88), u64::from(out)], &[], BUDGET);
        assert_eq!(mine.to_bits(), got(&ee, out), "brain {b:#x}: the card's spot's point");
    }
}
