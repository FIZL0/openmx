//! S47 — the sweep probe's narrow phase against the original.
//!
//! On the save state beside C7's recording, with the plane's own sweep probe and the meshes and
//! hierarchies its nearby search gathered there (26 meshes, 8 hierarchies of two meshes each, wide
//! vertices, about half with per-vertex normals). Each case gives the probe 1 to 16 generated segments,
//! most of them aimed through a triangle of one of those meshes from either side, some with an end on a
//! transform node and some of no length, and fills the answer with leftovers from an imaginary earlier
//! search; then runs the original's clear and one to three of its pair tests against candidates of the
//! case, and the port's [`clear`] and [`pair`] on the same numbers. Compared, bit for bit: every pair's
//! return, the probe's world and local segments, its centre and radius (and the collidable's copies),
//! and the whole answer — both head quads, the nearest normal, point and push-out, the tally, the
//! nearest fraction, and per segment the fraction, point, normal, hit word, owner word and the
//! collidable struck.

use game_sim::collide::{MAX_PROBES, MeshRef, Record};
use game_sim::sweep_probe::{Answer, Placed, Segment, Shape, SweepProbe, Target, Transform, clear, pair};
use game_sim::track::{Child, Node, Points, Triangle};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";
const PLANE: u32 = 0x00c4_4a40;
const PLANE_COLLISION: u32 = 0x170;
const CASES: usize = 10_000;
const BUDGET: u64 = 20_000_000;

/// The collision object's fields: its nearby search and its sweep probe.
const NEARBY: u32 = 0x10c;
const SWEEP_PROBE: u32 = 0x1f4;

/// A collidable's fields, as the pair test reads them.
mod at {
    pub const NEARBY: u32 = 0x31;
    pub const CENTRE: u32 = 0x40;
    pub const RADIUS: u32 = 0x60;
    pub const KIND: u32 = 0x68;
    pub const FLAGS: u32 = 0x116;
    pub const TRANSFORM: u32 = 0x120;
    pub const PARTS: u32 = 0x200;
    pub const PART_LIST: u32 = 0x214;
    pub const VERTICES: u32 = 0x220;
    pub const PACKED: u32 = 0x228;
    pub const NODES: u32 = 0x234;
    pub const TRIANGLES: u32 = 0x238;
}

/// The sweep probe's own fields.
mod probe_at {
    pub const ANSWER: u32 = 0x6c;
    pub const COUNT: u32 = 0x120;
    pub const NODES: u32 = 0x124;
    pub const ENDS: u32 = 0x128;
    pub const LOCAL_ORIGINS: u32 = 0x12c;
    pub const LOCAL_DIRS: u32 = 0x130;
    pub const LENGTHS: u32 = 0x134;
    pub const ORIGINS: u32 = 0x138;
    pub const DIRS: u32 = 0x13c;
    pub const LOCAL_CENTRE: u32 = 0x140;
    pub const CENTRE: u32 = 0x150;
    pub const RADIUS: u32 = 0x160;
    pub const SMOOTH: u32 = 0x164;
}

/// The answer's fields.
mod answer_at {
    pub const NORMAL: u32 = 0x20;
    pub const POINT: u32 = 0x30;
    pub const PUSH: u32 = 0x40;
    pub const HITS: u32 = 0x8c;
    pub const NORMALS: u32 = 0x94;
    pub const POINTS: u32 = 0x98;
    pub const STRUCK: u32 = 0x9c;
    pub const OWNERS: u32 = 0xa0;
    pub const FLAGS: u32 = 0xa4;
    pub const FRACTIONS: u32 = 0xa8;
    pub const NEAREST: u32 = 0xb0;
}

fn plane() -> Option<Ee> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: C7's save state, `./kit state ram` of biplane.p2m2_SaveState.p2s", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// A deterministic generator for the cases.
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    fn one_in(&mut self, n: u64) -> bool {
        self.next() % n == 0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn get_v(ee: &Ee, a: u32) -> Vf {
    Vf([0, 1, 2, 3].map(|k| ee.f32_at(a + 4 * k)))
}
fn put_v(ee: &mut Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_f32(a + 4 * k as u32, v.0[k]);
    }
}
fn bits(v: Vf) -> [u32; 4] {
    v.to_bits()
}

/// A mesh read out of RAM: everything reachable from its tree's root.
struct Mesh {
    nodes: Vec<Node>,
    triangles: Vec<Triangle>,
    vertices: Points,
}

fn read_mesh(ee: &Ee, obj: u32) -> Mesh {
    let (nodes_at, tris_at) = (ee.u32_at(obj + at::NODES), ee.u32_at(obj + at::TRIANGLES));
    let node = |i: u32| {
        let a = nodes_at + 0x24 * i;
        let v3 = |o: u32| [0, 1, 2].map(|k| F(ee.f32_at(a + o + 4 * k)));
        Node { volume: F(ee.f32_at(a + 4)), centre: v3(0xc), half_extent: v3(0x18), children: [ee.u16_at(a + 8), ee.u16_at(a + 0xa)] }
    };
    let (mut n_nodes, mut n_tris) = (1u32, 0u32);
    let mut todo = vec![0u32];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(i) = todo.pop() {
        if !seen.insert(i) {
            continue;
        }
        let n = node(i);
        for c in 0..2 {
            match n.child(c) {
                Child::Node(j) => {
                    n_nodes = n_nodes.max(u32::from(j) + 1);
                    todo.push(u32::from(j));
                }
                Child::Leaf(t) => n_tris = n_tris.max(u32::from(t) + 1),
            }
        }
    }
    let nodes: Vec<Node> = (0..n_nodes).map(node).collect();
    let triangles: Vec<Triangle> = (0..n_tris)
        .map(|i| Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices: [0, 1, 2].map(|k| ee.u16_at(tris_at + 0x14 * i + 2 * k)) })
        .collect();
    let n_verts = triangles.iter().flat_map(|t| t.vertices).max().map_or(0, |m| u32::from(m) + 1);
    let va = ee.u32_at(obj + at::VERTICES);
    let vertices = if ee.u32_at(obj + at::PACKED) != 0 {
        Points::Packed((0..n_verts).map(|i| [0, 1, 2].map(|k| ee.u16_at(va + 6 * i + 2 * k) as i16)).collect())
    } else {
        Points::Wide((0..n_verts).map(|i| [0, 1, 2].map(|k| F(ee.f32_at(va + 12 * i + 4 * k)))).collect())
    };
    Mesh { nodes, triangles, vertices }
}

fn transform_of(ee: &Ee, obj: u32) -> Transform {
    [0, 1, 2, 3].map(|k| get_v(ee, obj + at::TRANSFORM + 0x10 * k))
}

/// The scene: the probe and its answer, the candidates and the hierarchies' parts, their meshes, and
/// the scratch the probe's and the answer's arrays are moved to (the plane's own are sized for one
/// segment).
struct Scene {
    probe: u32,
    answer: u32,
    /// The search's candidates, then every hierarchy part.
    objects: Vec<u32>,
    candidates: usize,
    meshes: Vec<Option<Mesh>>,
    /// Per candidate, its parts' indices in `objects` (a mesh is its own only part).
    parts: Vec<Vec<usize>>,
    ends: u32,
    node_ptrs: u32,
    node_mats: u32,
    local_origins: u32,
    local_dirs: u32,
    lengths: u32,
    origins: u32,
    dirs: u32,
    normals: u32,
    points: u32,
    struck: u32,
    owners: u32,
    flags: u32,
    fractions: u32,
}

fn scene(ee: &mut Ee) -> Scene {
    let rec = ee.u32_at(PLANE + PLANE_COLLISION);
    let near = ee.u32_at(rec + NEARBY);
    let probe = ee.u32_at(rec + SWEEP_PROBE);
    assert_eq!(ee.u32_at(probe + at::KIND), 3, "the plane's sweep probe is not a sweep probe");
    let answer = ee.u32_at(probe + probe_at::ANSWER);
    let (found, n) = (ee.u32_at(near), ee.u32_at(near + 4));
    let mut objects: Vec<u32> = (0..n).map(|i| ee.u32_at(found + 8 * i)).collect();
    let candidates = objects.len();
    let mut parts = Vec::new();
    for c in 0..candidates {
        let o = objects[c];
        match ee.u32_at(o + at::KIND) {
            0 => parts.push(vec![c]),
            1 => {
                let list = ee.u32_at(o + at::PART_LIST);
                let mut mine = Vec::new();
                for j in 0..ee.u32_at(o + at::PARTS) {
                    mine.push(objects.len());
                    objects.push(ee.u32_at(list + 4 * j));
                }
                parts.push(mine);
            }
            k => panic!("candidate {c} is of kind {k}, not a mesh or a hierarchy"),
        }
    }
    let meshes = objects.iter().map(|&o| (ee.u32_at(o + at::KIND) == 0).then(|| read_mesh(ee, o))).collect();
    // One contiguous block of scratch for the arrays.
    let mut next = ee.scratch(1);
    let mut take = |n: u32| {
        let a = next;
        next += n;
        a
    };
    let p = MAX_PROBES as u32;
    Scene {
        probe,
        answer,
        objects,
        candidates,
        meshes,
        parts,
        ends: take(0x20 * p),
        node_ptrs: take(8 * p),
        node_mats: take(0x80 * p),
        local_origins: take(0x10 * p),
        local_dirs: take(0x10 * p),
        lengths: take(4 * p),
        origins: take(0x10 * p),
        dirs: take(0x10 * p),
        normals: take(0x10 * p),
        points: take(0x10 * p),
        struck: take(4 * p),
        owners: take(4 * p),
        flags: take(4 * p),
        fractions: take(4 * p),
    }
}

fn index_of(s: &Scene, a: u32) -> Option<usize> {
    if a == 0 {
        return None;
    }
    Some(s.objects.iter().position(|&o| o == a).unwrap_or(1_000_000 + a as usize))
}

/// A world-space point through a transform, in host arithmetic: only for aiming the segments.
fn place(m: &Transform, p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|k| m[3].0[k] + m[0].0[k] * p[0] + m[1].0[k] * p[1] + m[2].0[k] * p[2])
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn unit3(p: &mut Pick) -> [f32; 3] {
    loop {
        let v = [p.range(-1.0, 1.0), p.range(-1.0, 1.0), p.range(-1.0, 1.0)];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if l > 0.1 && l <= 1.0 {
            return v.map(|x| x / l);
        }
    }
}

/// A random quad, mostly ordinary numbers.
fn junk(p: &mut Pick) -> Vf {
    Vf([0; 4].map(|_| p.range(-50.0, 50.0)))
}

/// One generated case: the segments and the leftover answer, in RAM and in the port; and the pairs to
/// test.
fn generate(ee: &mut Ee, s: &Scene, p: &mut Pick, port: &mut SweepProbe) -> Vec<usize> {
    let pairs: Vec<usize> = (0..1 + p.below(3)).map(|_| p.below(s.candidates as u64) as usize).collect();
    let aim = pairs[0];
    let count = match p.below(10) {
        0..=4 => 1,
        5..=7 => 2 + p.below(3) as usize,
        _ => 5 + p.below(12) as usize,
    };
    // Far from everything now and then, so the spheres reject.
    let away = if p.one_in(10) { [p.range(-200.0, 200.0), p.range(50.0, 200.0), p.range(-200.0, 200.0)] } else { [0.0; 3] };
    let mut segments = Vec::new();
    let mut last = [0.0f32; 3];
    for i in 0..count {
        let part = s.parts[aim][p.below(s.parts[aim].len() as u64) as usize];
        let mesh = s.meshes[part].as_ref().unwrap();
        let m = transform_of(ee, s.objects[part]);
        let target = if i > 0 && p.one_in(2) {
            last.map(|x| x + p.range(-0.3, 0.3))
        } else if mesh.triangles.is_empty() {
            place(&m, [0.0; 3])
        } else {
            let t = &mesh.triangles[p.below(mesh.triangles.len() as u64) as usize];
            let mr = MeshRef { nodes: &mesh.nodes, triangles: &mesh.triangles, vertices: &mesh.vertices };
            let c = mr.corners(t).map(|v| place(&m, [v.0[0], v.0[1], v.0[2]]));
            let (mut u, mut v) = (p.range(-0.15, 1.0), p.range(-0.15, 1.0));
            if u + v > 1.1 {
                (u, v) = (1.0 - u, 1.0 - v);
            }
            std::array::from_fn(|k| c[0][k] + (c[1][k] - c[0][k]) * u + (c[2][k] - c[0][k]) * v)
        };
        last = target;
        let dir = unit3(p);
        let length = if p.one_in(30) { 0.0 } else { p.range(0.005, 3.0) };
        let before = length * p.range(-0.2, 1.2);
        let a: [f32; 3] = std::array::from_fn(|k| target[k] - dir[k] * before + away[k]);
        let b: [f32; 3] = std::array::from_fn(|k| a[k] + dir[k] * length);
        let mut seg = Segment::default();
        for (k, e) in [a, b].into_iter().enumerate() {
            let w = if p.one_in(4) { p.range(-2.0, 2.0) } else { 1.0 };
            if p.one_in(5) {
                // On a node: nearly the identity, moved.
                let mut node: Transform = [Vf([1.0, 0.0, 0.0, 0.0]), Vf([0.0, 1.0, 0.0, 0.0]), Vf([0.0, 0.0, 1.0, 0.0]), Vf::ZERO];
                for r in 0..3 {
                    for c in 0..4 {
                        node[r].0[c] += p.range(-0.02, 0.02);
                    }
                }
                let t = [p.range(-2.0, 2.0), p.range(-2.0, 2.0), p.range(-2.0, 2.0)];
                node[3] = Vf([t[0], t[1], t[2], p.range(-1.0, 1.0)]);
                seg.nodes[k] = Some(node);
                let l = sub3(e, t);
                seg.ends[k] = Vf([l[0], l[1], l[2], w]);
            } else {
                seg.ends[k] = Vf([e[0], e[1], e[2], w]);
            }
        }
        if p.one_in(40) {
            seg.ends[1] = seg.ends[0];
            seg.nodes[1] = seg.nodes[0];
        }
        segments.push(seg);
    }

    // The segments into RAM.
    let pr = s.probe;
    ee.set_u32(pr + probe_at::COUNT, count as u32);
    for (key, a) in [
        (probe_at::NODES, s.node_ptrs),
        (probe_at::ENDS, s.ends),
        (probe_at::LOCAL_ORIGINS, s.local_origins),
        (probe_at::LOCAL_DIRS, s.local_dirs),
        (probe_at::LENGTHS, s.lengths),
        (probe_at::ORIGINS, s.origins),
        (probe_at::DIRS, s.dirs),
    ] {
        ee.set_u32(pr + key, a);
    }
    assert_eq!(ee.u32_at(pr + probe_at::SMOOTH), 0, "the probe asks for smoothed normals");
    for (i, seg) in segments.iter().enumerate() {
        let i = i as u32;
        for k in 0..2u32 {
            put_v(ee, s.ends + 0x20 * i + 0x10 * k, seg.ends[k as usize]);
            let slot = s.node_mats + 0x80 * i + 0x40 * k;
            match seg.nodes[k as usize] {
                Some(m) => {
                    for r in 0..4u32 {
                        put_v(ee, slot + 0x10 * r, m[r as usize]);
                    }
                    // The node's world transform is its +0x80.
                    ee.set_u32(s.node_ptrs + 8 * i + 4 * k, slot - 0x80);
                }
                None => ee.set_u32(s.node_ptrs + 8 * i + 4 * k, 0),
            }
        }
    }
    port.segments = segments;
    // Stale local and world forms, the same on both sides.
    for i in 0..MAX_PROBES {
        let (o, d, lo, ld) = (junk(p), junk(p), junk(p), junk(p));
        let i32_ = i as u32;
        put_v(ee, s.origins + 0x10 * i32_, o);
        put_v(ee, s.dirs + 0x10 * i32_, d);
        put_v(ee, s.local_origins + 0x10 * i32_, lo);
        put_v(ee, s.local_dirs + 0x10 * i32_, ld);
        port.world.probes[i].origin = o;
        port.world.probes[i].dir = d;
        port.local.probes[i].origin = lo;
        port.local.probes[i].dir = ld;
    }
    let lc = junk(p);
    put_v(ee, pr + probe_at::LOCAL_CENTRE, lc);
    port.local.centre = lc;

    // The answer, as an earlier search might have left it.
    let an = s.answer;
    for (key, a) in [
        (answer_at::NORMALS, s.normals),
        (answer_at::POINTS, s.points),
        (answer_at::STRUCK, s.struck),
        (answer_at::OWNERS, s.owners),
        (answer_at::FLAGS, s.flags),
        (answer_at::FRACTIONS, s.fractions),
    ] {
        ee.set_u32(an + key, a);
    }
    let mut ans = Answer { head: [junk(p), junk(p)], record: Record::new(), struck: [None; MAX_PROBES] };
    put_v(ee, an, ans.head[0]);
    put_v(ee, an + 0x10, ans.head[1]);
    let r = &mut ans.record;
    r.nearest_normal = junk(p);
    r.nearest_point = junk(p);
    r.push = junk(p);
    r.hits = p.below(5) as i32;
    r.nearest = F(p.range(0.0, 1.0));
    put_v(ee, an + answer_at::NORMAL, r.nearest_normal);
    put_v(ee, an + answer_at::POINT, r.nearest_point);
    put_v(ee, an + answer_at::PUSH, r.push);
    ee.set_u32(an + answer_at::HITS, r.hits as u32);
    ee.set_f32(an + answer_at::NEAREST, r.nearest.0);
    for i in 0..MAX_PROBES {
        let iu = i as u32;
        r.fraction[i] = F(if p.one_in(2) { -1.0 } else { p.range(0.0, 1.0) });
        r.point[i] = junk(p);
        r.normal[i] = junk(p);
        r.hit[i] = p.one_in(2);
        r.owner[i] = p.below(2) as i32;
        let struck = if p.one_in(3) { 0 } else { s.objects[p.below(s.objects.len() as u64) as usize] };
        ans.struck[i] = index_of(s, struck);
        ee.set_f32(s.fractions + 4 * iu, r.fraction[i].0);
        put_v(ee, s.points + 0x10 * iu, r.point[i]);
        put_v(ee, s.normals + 0x10 * iu, r.normal[i]);
        ee.set_u32(s.flags + 4 * iu, u32::from(r.hit[i]));
        ee.set_u32(s.owners + 4 * iu, r.owner[i] as u32);
        ee.set_u32(s.struck + 4 * iu, struck);
    }
    port.answer = ans;

    // The candidates: hierarchies are not struck by a nearby search in the save state; let them be.
    for &c in &pairs {
        let o = s.objects[c];
        let byte = ee.u8_at(o + at::NEARBY);
        let on = !p.one_in(12);
        ee.write(o + at::NEARBY, 1, u128::from(byte & !1 | u8::from(on)));
    }
    pairs
}

fn target<'a>(ee: &Ee, s: &Scene, placed: &'a [Placed<'a>], c: usize) -> Target<'a> {
    let o = s.objects[c];
    let shape = match ee.u32_at(o + at::KIND) {
        0 => Shape::Mesh(placed[c]),
        1 => {
            let first = s.parts[c][0];
            Shape::Hierarchy(&placed[first..first + s.parts[c].len()])
        }
        _ => Shape::Other,
    };
    Target { nearby: ee.u8_at(o + at::NEARBY) & 1 != 0, centre: get_v(ee, o + at::CENTRE), radius: F(ee.f32_at(o + at::RADIUS)), shape }
}

#[derive(Default)]
struct Reached {
    pairs: usize,
    struck: usize,
    rejected: usize,
    hierarchy_struck: usize,
    segments_hit: usize,
    on_nodes: usize,
    no_length: usize,
    sixteen: usize,
}

#[test]
fn the_sweep_probe_matches_the_original() {
    let Some(mut ee) = plane() else { return };
    let s = scene(&mut ee);
    let (clear_at, pair_at) = (addr("sphere_probe_clear").unwrap(), addr("sphere_probe_pair").unwrap());
    let empty = Points::Wide(Vec::new());
    let placed: Vec<Placed> = (0..s.objects.len())
        .map(|i| {
            let mesh = match &s.meshes[i] {
                Some(m) => MeshRef { nodes: &m.nodes, triangles: &m.triangles, vertices: &m.vertices },
                None => MeshRef { nodes: &[], triangles: &[], vertices: &empty },
            };
            Placed { transform: transform_of(&ee, s.objects[i]), mesh, object: i }
        })
        .collect();
    let mut p = Pick(0x5eed_0047_c0ff_ee11);
    let mut seen = Reached::default();
    for case in 0..CASES {
        let mut port = SweepProbe::default();
        let pairs = generate(&mut ee, &s, &mut p, &mut port);
        let count = port.segments.len();
        seen.on_nodes += port.segments.iter().filter(|g| g.nodes.iter().any(Option::is_some)).count();
        seen.no_length += port.segments.iter().filter(|g| g.ends[0] == g.ends[1] && g.nodes[0] == g.nodes[1]).count();
        seen.sixteen += usize::from(count == MAX_PROBES);
        let flags = ee.u8_at(s.probe + at::FLAGS);

        ee.call(clear_at, &[u64::from(s.probe)], &[], BUDGET);
        clear(&mut port);
        assert_eq!(ee.u8_at(s.probe + at::FLAGS), flags & !0x20, "case {case}: the clear's hit bit");
        let mut returns = Vec::new();
        for &c in &pairs {
            ee.call(pair_at, &[u64::from(s.probe), u64::from(s.objects[c])], &[], BUDGET);
            let theirs = ee.v0() != 0;
            let t = target(&ee, &s, &placed, c);
            let ours = pair(&mut port, Some(&t));
            returns.push((c, theirs, ours));
            seen.pairs += 1;
            seen.struck += usize::from(theirs);
            seen.rejected += usize::from(!t.nearby || !theirs && ee.u32_at(s.answer + answer_at::HITS) == 0);
            seen.hierarchy_struck += usize::from(theirs && matches!(t.shape, Shape::Hierarchy(_)));
        }
        let ctx = format!("case {case}: {count} segments, pairs {returns:?}");
        for (_, theirs, ours) in &returns {
            assert_eq!(theirs, ours, "{ctx}: the return");
        }

        // The probe.
        let pr = s.probe;
        assert_eq!(bits(get_v(&ee, pr + probe_at::CENTRE)), bits(port.world.centre), "{ctx}: the centre");
        assert_eq!(bits(get_v(&ee, pr + at::CENTRE)), bits(port.world.centre), "{ctx}: the collidable's centre");
        assert_eq!(ee.f32_at(pr + probe_at::RADIUS).to_bits(), port.world.radius.0.to_bits(), "{ctx}: the radius");
        assert_eq!(ee.f32_at(pr + at::RADIUS).to_bits(), port.world.radius.0.to_bits(), "{ctx}: the reach");
        assert_eq!(bits(get_v(&ee, pr + probe_at::LOCAL_CENTRE)), bits(port.local.centre), "{ctx}: the local centre");
        for i in 0..count {
            let iu = i as u32;
            let (w, l) = (port.world.probes[i], port.local.probes[i]);
            assert_eq!(bits(get_v(&ee, s.origins + 0x10 * iu)), bits(w.origin), "{ctx}: segment {i}'s start");
            assert_eq!(bits(get_v(&ee, s.dirs + 0x10 * iu)), bits(w.dir), "{ctx}: segment {i}'s direction");
            assert_eq!(ee.f32_at(s.lengths + 4 * iu).to_bits(), w.length.0.to_bits(), "{ctx}: segment {i}'s length");
            assert_eq!(bits(get_v(&ee, s.local_origins + 0x10 * iu)), bits(l.origin), "{ctx}: segment {i}'s local start");
            assert_eq!(bits(get_v(&ee, s.local_dirs + 0x10 * iu)), bits(l.dir), "{ctx}: segment {i}'s local direction");
        }

        // The answer.
        let (an, a) = (s.answer, &port.answer);
        let r = &a.record;
        assert_eq!(bits(get_v(&ee, an)), bits(a.head[0]), "{ctx}: +0");
        assert_eq!(bits(get_v(&ee, an + 0x10)), bits(a.head[1]), "{ctx}: +0x10");
        assert_eq!(bits(get_v(&ee, an + answer_at::NORMAL)), bits(r.nearest_normal), "{ctx}: the nearest normal");
        assert_eq!(bits(get_v(&ee, an + answer_at::POINT)), bits(r.nearest_point), "{ctx}: the nearest point");
        assert_eq!(bits(get_v(&ee, an + answer_at::PUSH)), bits(r.push), "{ctx}: the push-out");
        assert_eq!(ee.i32_at(an + answer_at::HITS), r.hits, "{ctx}: the tally");
        assert_eq!(ee.f32_at(an + answer_at::NEAREST).to_bits(), r.nearest.0.to_bits(), "{ctx}: the nearest fraction");
        for i in 0..MAX_PROBES {
            let iu = i as u32;
            assert_eq!(ee.f32_at(s.fractions + 4 * iu).to_bits(), r.fraction[i].0.to_bits(), "{ctx}: segment {i}'s fraction");
            assert_eq!(bits(get_v(&ee, s.points + 0x10 * iu)), bits(r.point[i]), "{ctx}: segment {i}'s point");
            assert_eq!(bits(get_v(&ee, s.normals + 0x10 * iu)), bits(r.normal[i]), "{ctx}: segment {i}'s normal");
            assert_eq!(ee.u32_at(s.flags + 4 * iu), u32::from(r.hit[i]), "{ctx}: segment {i}'s hit word");
            assert_eq!(ee.i32_at(s.owners + 4 * iu), r.owner[i], "{ctx}: segment {i}'s owner word");
            assert_eq!(index_of(&s, ee.u32_at(s.struck + 4 * iu)), a.struck[i], "{ctx}: segment {i}'s struck");
            seen.segments_hit += usize::from(i < count && r.hit[i]);
        }
    }
    eprintln!(
        "{} pairs: {} struck ({} hierarchies), {} refused or empty; {} segment hits; {} segments on nodes, {} of no length; {} cases of 16",
        seen.pairs, seen.struck, seen.hierarchy_struck, seen.rejected, seen.segments_hit, seen.on_nodes, seen.no_length, seen.sixteen
    );
    assert!(seen.struck > 2000 && seen.hierarchy_struck > 300 && seen.rejected > 1000, "the cases do not reach enough");
    assert!(seen.on_nodes > 1000 && seen.no_length > 100 && seen.sixteen > 100, "the segments do not vary enough");
}
