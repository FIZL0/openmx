// ---------------------------------------------------------------------------------------------------
// S282 — the track path's queries, and the avoidance's keep-on-the-track push on them
// ---------------------------------------------------------------------------------------------------
//
// Generated paths are laid into RAM as the original keeps them: a node every 0x60 bytes with its
// centre at +0x10, its two edge points at +0x20/+0x30, the length on at +0x40 and the next node at
// +0x54, the words between filled with junk the queries must not read; the path object (the rider's
// own +0x88) holds the first node at +0x10 and no branches.

use super::*;

const NET_NODES: u32 = 0x019a_0000;
const NODE_SIZE: u32 = 0x60;
const PATH_OBJ: u32 = 0x019b_0000;
pub(crate) const PATH_OUT: u32 = 0x019b_1000; // locate's place: +0 the path, +4 the node, +8 how far along
pub(crate) const PATH_DIST: u32 = 0x019b_1100;
pub(crate) const PATH_POINT: u32 = 0x019b_1200;
const PATH_IO: u32 = 0x019b_1300;
/// What `locate` leaves in a word it does not write.
pub(crate) const UNTOUCHED: u32 = 0x5eed_f00d;

fn node_at(i: usize) -> u32 {
    NET_NODES + i as u32 * NODE_SIZE
}

fn index_of(a: u32) -> Option<usize> {
    (a != 0).then(|| ((a - NET_NODES) / NODE_SIZE) as usize)
}

fn write_net(ee: &mut ps2_oracle::Ee, net: &Net, junk: &mut Pick) {
    for (i, n) in net.nodes.iter().enumerate() {
        let a = node_at(i);
        for o in (0..NODE_SIZE).step_by(4) {
            ee.set_u32(a + o, junk.next() as u32);
        }
        put(ee, a + 0x10, n.centre);
        put(ee, a + 0x20, n.edges[0]);
        put(ee, a + 0x30, n.edges[1]);
        ee.set_f32(a + 0x40, n.length.0);
        ee.set_u32(a + 0x54, n.next.map_or(0, node_at));
    }
}

/// A coordinate, now and then snapped to a whole number so sides line up and points land on them.
fn coord(p: &mut Pick, lo: f32, hi: f32, snap: bool) -> f32 {
    let x = p.range(lo, hi);
    if snap { x.round() } else { x }
}

/// A chain of one to twelve cross-sections wandering across the ground, most a true length apart,
/// some with a length that is nothing, negative or wrong; now and then on whole numbers, with edge
/// points out of order or folded back onto the centre.
fn gen_net(p: &mut Pick) -> Net {
    let count = 1 + p.below(12) as usize;
    let snap = p.chance(4);
    let mut nodes: Vec<Node> = Vec::new();
    let (mut x, mut z, mut yaw) = (coord(p, -60.0, 60.0, snap), coord(p, -60.0, 60.0, snap), p.range(-7.0, 7.0));
    for i in 0..count {
        let w = p.range(2.0, 15.0);
        let (s, c) = (yaw.sin(), yaw.cos());
        let centre = Vf::new(x, p.range(-20.0, 20.0), z, p.range(-1.0, 1.0));
        let mut side = |k: f32| {
            let jx = if p.chance(3) { 0.0 } else { p.range(-3.0, 3.0) };
            let jz = if p.chance(3) { 0.0 } else { p.range(-3.0, 3.0) };
            let e = Vf::new(x + k * c * w + jx, p.range(-20.0, 20.0), z - k * s * w + jz, p.range(-1.0, 1.0));
            if snap { Vf::new(e.x().round(), e.y(), e.z().round(), e.w()) } else { e }
        };
        let mut edges = [side(1.0), side(-1.0)];
        if p.chance(8) {
            edges.swap(0, 1);
        }
        if p.chance(16) {
            edges[p.below(2) as usize] = centre;
        }
        nodes.push(Node { centre, edges, length: F(0.0), next: (i + 1 < count).then_some(i + 1) });
        if p.chance(6) {
            yaw += p.range(-1.5, 1.5);
        }
        let step = if p.chance(12) { 0.0 } else { p.range(2.0, 30.0) };
        x += yaw.sin() * step;
        z += yaw.cos() * step;
        if snap {
            (x, z) = (x.round(), z.round());
        }
    }
    for i in 0..count {
        let length = match nodes[i].next {
            Some(j) => {
                let (a, b) = (nodes[i].centre, nodes[j].centre);
                let true_len = ((b.x() - a.x()).powi(2) + (b.z() - a.z()).powi(2)).sqrt();
                match p.below(10) {
                    0 => 0.0,
                    1 => -p.range(0.0, 10.0),
                    2 => true_len * p.range(0.5, 2.0),
                    _ => true_len,
                }
            }
            None => p.range(-5.0, 30.0),
        };
        nodes[i].length = F(length);
    }
    Net { nodes, ..Net::default() }
}

/// A point near the net: on a node or edge point, between two, or anywhere around.
fn near(p: &mut Pick, net: &Net) -> Vf {
    let n = &net.nodes[p.below(net.nodes.len() as u32) as usize];
    let base = match p.below(4) {
        0 => n.centre,
        1 => n.edges[p.below(2) as usize],
        _ => Vf::new(
            n.centre.x() + p.range(-25.0, 25.0),
            n.centre.y(),
            n.centre.z() + p.range(-25.0, 25.0),
            p.range(-1.0, 1.0),
        ),
    };
    if p.chance(4) { base } else { Vf::new(base.x() + p.range(-2.0, 2.0), base.y(), base.z() + p.range(-2.0, 2.0), base.w()) }
}

/// The three path queries against the original over ten thousand generated paths: `path_locate`
/// (no hint, with and without a distance limit, the holding-only filter and the distance out),
/// `path_segment_holds` on a stretch and on any two nodes, and `path_node_direction` on any node,
/// none, or the last. Every word each one writes is compared, and the words it must not write.
#[test]
fn the_path_queries_match_over_ten_thousand_paths() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["path_locate", "path_segment_holds", "path_node_direction"]) else { return };
    let [locate, holds, direction] = at[..] else { unreachable!() };
    let mut pick = Pick(0x5282_9a7b_0c1d_e2f3);
    let mut junk = Pick(0x0bad_cafe_1234_5678);
    let mut seen = [0u32; 8];
    for case in 0..10_000 {
        let p = &mut pick;
        let net = gen_net(p);
        write_net(&mut ee, &net, &mut junk);
        let count = net.nodes.len();
        let first = p.below(count as u32) as usize;
        ee.set_u32(PATH_OBJ, junk.next() as u32);
        ee.set_u32(PATH_OBJ + 0x10, node_at(first));
        ee.set_u32(PATH_OBJ + 0x18, 0);
        let point = near(p, &net);
        put(&mut ee, PATH_POINT, point);
        let what = format!("case {case}: {count} nodes from {first}, point {point:?}");

        // locate
        let limit = match p.below(3) {
            0 => p.range(0.0, 120.0),
            1 => -p.range(0.0, 5.0),
            _ => -1.0,
        };
        let holding_only = p.chance(3);
        let with_distance = !p.chance(5);
        for o in [0, 4, 8] {
            ee.set_u32(PATH_OUT + o, UNTOUCHED);
        }
        ee.set_u32(PATH_DIST, UNTOUCHED);
        ee.call(
            locate,
            &[
                0,
                0,
                u64::from(PATH_POINT),
                u64::from(PATH_OBJ),
                u64::from(PATH_OUT),
                if with_distance { u64::from(PATH_DIST) } else { 0 },
                0,
                u64::from(holding_only),
            ],
            &[limit],
            BUDGET,
        );
        let ours = path::locate(&net, first, point, F(limit), holding_only);
        let w = format!("{what}, limit {limit}, holding only {holding_only}: {ours:?}");
        assert_eq!(ee.v0(), 1, "{w}: locate's answer");
        assert_eq!(ee.u32_at(PATH_OUT), PATH_OBJ, "{w}: the place's path");
        match ours.place {
            Some(pl) => {
                assert_eq!(index_of(ee.u32_at(PATH_OUT + 4)), Some(pl.node), "{w}: the node");
                assert_eq!(ee.u32_at(PATH_OUT + 8), pl.along.to_bits(), "{w}: how far along");
                seen[0] += u32::from(pl.along > F(0.0) && pl.along < F(1.0));
            }
            None => {
                assert_eq!(ee.u32_at(PATH_OUT + 4), UNTOUCHED, "{w}: a node written");
                assert_eq!(ee.u32_at(PATH_OUT + 8), UNTOUCHED, "{w}: a fraction written");
                seen[1] += 1;
            }
        }
        let want_dist = if with_distance { ours.distance.to_bits() } else { UNTOUCHED };
        assert_eq!(ee.u32_at(PATH_DIST), want_dist, "{w}: the distance");
        seen[2] += u32::from(limit >= 0.0 && ours.place.is_some_and(|pl| pl.node != first));

        // holds, on a stretch or any two nodes
        let a = p.below(count as u32) as usize;
        let b = match net.nodes[a].next {
            Some(j) if !p.chance(4) => j,
            _ => p.below(count as u32) as usize,
        };
        ee.call(holds, &[0, u64::from(PATH_POINT), u64::from(node_at(a)), u64::from(node_at(b))], &[], BUDGET);
        let held = path::segment_holds(&net, point, a, b);
        assert_eq!(ee.v0() != 0, held, "{what}: holds {a}..{b}");
        assert!(ee.v0() <= 1, "{what}: holds answered {}", ee.v0());
        seen[3] += u32::from(held);

        // the direction at a node
        let node = if p.chance(10) { None } else { Some(p.below(count as u32) as usize) };
        let io_w = p.range(-1.0, 1.0);
        let io_in = p.vec(5.0, io_w);
        put(&mut ee, PATH_IO, io_in);
        ee.call(direction, &[0, node.map_or(0, |i| u64::from(node_at(i))), u64::from(PATH_IO)], &[], BUDGET);
        let mut io = io_in;
        let gave = path::node_direction(&net, node, &mut io);
        assert_eq!(ee.v0() != 0, gave, "{what}: direction at {node:?}");
        assert_eq!(got(&ee, PATH_IO), io.to_bits(), "{what}: the direction at {node:?}");
        seen[4] += u32::from(gave);
        seen[5] += u32::from(!gave && node.is_some());
        seen[6] += u32::from(count == 1);
        seen[7] += u32::from(holding_only && ours.place.is_some());
    }
    eprintln!(
        "path queries: {} inside a stretch, {} unplaced, {} limited, {} held, {} directions, {} refused, {} lone, {} held places",
        seen[0], seen[1], seen[2], seen[3], seen[4], seen[5], seen[6], seen[7]
    );
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 100, "branch {k} reached only {n} times: {seen:?}");
    }
}

/// The keep-on-the-track push end to end: the original's motocross avoidance pass (`ai_update_mx`)
/// on generated paths with the three path queries running as the original's own code, against
/// [`ai::avoid`] with [`path::off_track`] as its edge. No other riders and no feeler fan (flags 0x80
/// and 0x40 both up), so the only thing bending the steer is the push. Ten thousand passes.
#[test]
fn the_keep_on_track_push_matches_over_ten_thousand_passes() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_update_mx", "curve_table_build", "mem_alloc"]) else { return };
    let [update, curve_build, mem_alloc] = at[..] else { unreachable!() };
    ee.hooks.insert(mem_alloc, Box::new(|ee| ee.set(2, 0x0100_0000)));
    ee.call(curve_build, &[], &[], 2_000_000_000);
    ee.hooks.remove(&mem_alloc);
    let curve = CurveTable::build();
    // The feeler bundle the pass keeps (it would allocate one were there none), never filled here.
    ee.set_u32(AI + 0x2a0, BUNDLE);
    ee.set_u32(BUNDLE + 0x6c, FEELER_RECORD);
    ee.set_u32(BUNDLE + 0x134, LENGTHS);
    ee.set_u32(BUNDLE + 0x138, ORIGINS);
    ee.set_u32(BUNDLE + 0x13c, DIRS);
    ee.hooks.insert(mem_alloc, Box::new(|_| panic!("the pass allocated")));
    let mut pick = Pick(0x0ff7_7ac4_5282_0001);
    let mut junk = Pick(0x7e57_0000_aaaa_5555);
    ee.set_u32(AI + 0x40, SCENE);
    ee.set_u32(AI + 0xd0, 2);
    ee.set_u32(AI + 0xd4, OWN);
    ee.set_u32(AI + 0xd8, RACE);
    ee.set_u32(OWN + 0x40, 1);
    ee.set_u32(RACE + 0x34, 0);
    ee.set_u32(RACE + 0x68, PATH_OBJ + 0x800);
    let (mut pushed, mut held, mut short, mut no_path, mut flat, mut lengthless) = (0, 0, 0, 0, 0, 0);
    for case in 0..10_000 {
        let p = &mut pick;
        let net = gen_net(p);
        write_net(&mut ee, &net, &mut junk);
        let count = net.nodes.len();
        let first = if p.chance(20) { None } else { Some(p.below(count as u32) as usize) };
        let node = if p.chance(10) { None } else { Some(p.below(count as u32) as usize) };
        ee.set_u32(PATH_OBJ, junk.next() as u32);
        ee.set_u32(PATH_OBJ + 0x10, first.map_or(0, node_at));
        ee.set_u32(PATH_OBJ + 0x18, 0);
        ee.set_u32(OWN + 0x88, if first.is_some() { PATH_OBJ } else { 0 });
        ee.set_u32(OWN + 0x8c, node.map_or(0, node_at));

        let t = AvoidTuning { edge_look_ahead: F(p.range(0.0, 20.0)), edge_push: F(p.range(0.0, 2.0)), ..AvoidTuning::default() };
        let yaw = p.range(-7.0, 7.0);
        let me = Me {
            position: near(p, &net),
            heading: Vf::new(yaw.sin(), p.range(-0.2, 0.2), yaw.cos(), p.range(-1.0, 1.0)),
            velocity: p.vec(30.0, 0.0),
            speed: F(p.range(0.01, 60.0)),
            held_speed: F(p.range(-5.0, 60.0)),
            progress: F(p.range(0.0, 100.0)),
        };
        let flags = (p.next() & 0xffff_ffff_0000_0000 | p.next() & 0x0000_0000_fffe_00ff) & !(flag::SKIP_MARKED | flag::AVOID_DUE)
            | flag::NO_RIDER_SWEEP
            | flag::QUIET_AREA
            | flag::KEEP_ON_TRACK
            | if p.chance(2) { flag::AVOID_DUE } else { 0 };
        let steer_w = p.range(-1.0, 1.0);
        let steer_in = Steer { direction: p.vec(1.0, steer_w), speed: F(p.range(0.0, 60.0)) };

        ee.write(AI + 0x30, 8, u128::from(flags));
        put(&mut ee, AI + 0x50, me.velocity);
        ee.set_f32(AI + 0x80, me.held_speed.0);
        ee.set_f32(OWN + 0xbc, me.progress.0);
        ee.set_f32(AI + 0x288, t.edge_look_ahead.0);
        ee.set_f32(AI + 0x3c8, t.edge_push.0);
        put(&mut ee, ARGS, me.position);
        put(&mut ee, ARGS + 0x10, me.heading);
        put(&mut ee, ARGS + 0x20, steer_in.direction);
        ee.set_f32(ARGS + 0x30, me.speed.0);
        ee.set_f32(ARGS + 0x34, steer_in.speed.0);
        ee.call(
            update,
            &[
                u64::from(AI),
                0,
                u64::from(IGNORE),
                u64::from(ARGS),
                u64::from(ARGS + 0x10),
                u64::from(ARGS + 0x20),
                u64::from(ARGS + 0x30),
                u64::from(ARGS + 0x34),
            ],
            &[],
            BUDGET,
        );

        let mut port_flags = flags;
        let mut steer = steer_in;
        let mut answer = None;
        let mut edge = |ahead: Vf| {
            let a = path::off_track(&net, first.unwrap(), node.unwrap(), ahead);
            answer = Some(a);
            a
        };
        let edge: Option<&mut dyn FnMut(Vf) -> Option<Vf>> =
            if first.is_some() && node.is_some() { Some(&mut edge) } else { None };
        let fan = ai::avoid(&t, &me, &mut port_flags, &[], edge, &mut steer, &curve);
        let what = format!("case {case}: {count} nodes, first {first:?}, node {node:?}, answer {answer:?}");
        assert!(fan.is_none(), "{what}: a fan built");
        assert_eq!(got(&ee, ARGS + 0x20), steer.direction.to_bits(), "{what}: the steer direction");
        assert_eq!(ee.u32_at(ARGS + 0x34), steer.speed.to_bits(), "{what}: the aimed-for speed");
        assert_eq!(ee.read(AI + 0x30, 8) as u64, port_flags, "{what}: the flags");
        match answer {
            Some(Some(q)) => {
                let across = me.heading.x() * q.z() - me.heading.z() * q.x();
                if across > 0.0 { pushed += 1 } else { flat += 1 }
                lengthless += u32::from(q == ahead_of(&me, &t));
            }
            None => no_path += 1,
            Some(None) if count < 3 => short += 1,
            Some(None) => held += 1,
        }
    }
    eprintln!(
        "keep on track: {pushed} pushed, {flat} pointed away, {held} on the track, {short} short paths, \
         {no_path} no path or node, {lengthless} pushed along the point itself"
    );
    assert!(pushed > 1000 && flat > 1000 && held > 1000 && short > 100 && no_path > 100 && lengthless > 100);
}

/// The point the pass looks at, the way it builds it (`vmulx.xyz` then `vadd.xyz`).
fn ahead_of(me: &Me, t: &AvoidTuning) -> Vf {
    let s = me.heading.mul(Vf::new(t.edge_look_ahead.0, 0.0, 0.0, 0.0).bc(ps2_float::Field::X));
    me.position.merge(ps2_float::Dest::XYZ, me.position.add(s))
}
