//! S4 — collision against the world, proved against the original.
//!
//! Four proofs. The big one is the sweep: the harness builds a whole little collision mesh in the
//! interpreter's RAM — vertices, triangles, a bounding-volume tree over them, a bundle of probe
//! segments and the record they report into — runs the original's sweep over it, then runs the port
//! over the same numbers and compares every field the original wrote. Ten thousand of those, each with
//! its own shape of tree and its own probes, and each one reported with the seed that made it so a
//! failure is reproducible on its own.
//!
//! The other three are the pieces the sweep is built out of and that a caller has to get right before
//! the sweep is even reached: the vertex fetch, in both of the widths a mesh is stored in, and the
//! derivation of a probe from two points. The vector maths inside the sweep — the cross product, the
//! VU's normalise, two kinds of dot product, the divides — is proved *by* the sweep: every one of them
//! is on the path of every hit, so a single wrong rounding anywhere in there shows up as a wrong
//! fraction.
//!
//! The comparison leaves nothing out: all four lanes of every vector, every fraction, every flag. The
//! `w` lanes are part of it on purpose, because they are not zero and not junk — the game's vector
//! helpers are `.xyz` instructions over a register loaded from their first source, so a `w` lane
//! travels sideways through the whole sweep, from a vertex into a normal and from a probe's direction
//! into a contact point. Getting one wrong is harmless until a record is copied or hashed, and then it
//! is a divergence with no arithmetic behind it at all.

use game_sim::collide::{self, Bundle, MAX_PROBES, MeshRef, Probe, Record};
use game_sim::track::{Child, Node, Points, Triangle};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, addr, game_or_skip};

// --- where the harness puts things in RAM -----------------------------------------------------------
// Past anything the executable uses, and well below the stack a call starts with.
const NODES: u32 = 0x0110_0000;
const TRIS: u32 = 0x0112_0000;
const VERTS: u32 = 0x0114_0000;
/// The vertex descriptor the sweep is handed: the array, the per-vertex normals (none here, which is
/// what switches smoothed contact normals off), and which width the vertices are in.
const DESC: u32 = 0x0116_0000;
/// The pair of pointers the sweep walks the tree through: the nodes, then the triangles.
const TREES: u32 = 0x0116_0100;
const OBJ: u32 = 0x0118_0000;
const ORIGINS: u32 = 0x0119_0000;
const DIRS: u32 = 0x0119_1000;
const LENGTHS: u32 = 0x0119_2000;
const REC: u32 = 0x011a_0000;
const FRACTIONS: u32 = 0x011b_0000;
const POINTS: u32 = 0x011b_1000;
const NORMALS: u32 = 0x011b_2000;
const FLAGS: u32 = 0x011b_3000;
const OWNERS: u32 = 0x011b_4000;

/// How many scenes the sweep proof generates.
const SCENES: u32 = 10_000;
/// How many samples each of the two helper proofs generates.
const SAMPLES: u32 = 10_000;

// --- the generator ----------------------------------------------------------------------------------

/// A plain 64-bit LCG, so a seed names a scene and nothing in the generated data depends on the host.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 32) as u32
    }
    /// A number in `0..n`.
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    /// A coordinate: a multiple of 1/64 in ±`range`, which is exact in binary and so identical on every
    /// host. Generated values are written into RAM as bits and handed to the port as the same bits, so
    /// the generator's own arithmetic is never part of what is being proved.
    #[allow(clippy::float_arithmetic)]
    fn coord(&mut self, range: i32) -> f32 {
        let steps = range * 64;
        (self.below(2 * steps as u32) as i32 - steps) as f32 * 0.015_625
    }
    fn point(&mut self, range: i32) -> Vf {
        Vf([self.coord(range), self.coord(range), self.coord(range), 1.0])
    }
}

/// One generated scene: a mesh and a bundle of probes to sweep through it.
struct Scene {
    verts: Points,
    tris: Vec<Triangle>,
    nodes: Vec<Node>,
    bundle: Bundle,
}

#[allow(clippy::float_arithmetic)] // the generator's own arithmetic: it only decides what to prove
fn scene(seed: u64) -> Scene {
    let mut rng = Rng(seed);
    let packed = rng.below(4) == 0;

    // Vertices, in a box a few metres across. Packed meshes hold shorts, so those are whole numbers.
    let count = 3 + rng.below(10) as usize;
    let mut positions = Vec::with_capacity(count);
    let mut shorts = Vec::with_capacity(count);
    for _ in 0..count {
        if packed {
            let s = [rng.below(21) as i16 - 10, rng.below(21) as i16 - 10, rng.below(21) as i16 - 10];
            shorts.push(s);
            // What the original's fetch makes of those shorts, which is what the tree has to be built
            // over. Proved separately by `the_vertex_fetch_matches_the_original`.
            positions.push([0, 1, 2].map(|k| (3.051_851e-5f32 * s[k] as f32) * 32767.0));
        } else {
            let p = [rng.coord(5), rng.coord(5), rng.coord(5)];
            positions.push(p);
        }
    }
    let verts = if packed {
        Points::Packed(shorts)
    } else {
        Points::Wide(positions.iter().map(|p| [F(p[0]), F(p[1]), F(p[2])]).collect())
    };

    // Triangles over them, two at least so the tree's root is a node — which is what the game's own
    // loader always produces, and what the sweep's entry assumes.
    let tri_count = 2 + rng.below(7) as usize;
    let mut tris = Vec::with_capacity(tri_count);
    for _ in 0..tri_count {
        let a = rng.below(count as u32) as u16;
        let b = rng.below(count as u32) as u16;
        let c = rng.below(count as u32) as u16;
        tris.push(Triangle { distance: F(0.0), normal: [F(0.0); 3], vertices: [a, b, c] });
    }

    // The tree: split the triangle list in half each time, box each range. Every fourth scene gets its
    // boxes shrunk, which is not something a loader would produce but is what makes the sweep reject
    // and prune — the paths a tree that fits its geometry never reaches.
    let shrink = if rng.below(4) == 0 { 0.5 } else { 1.0 };
    let order: Vec<usize> = (0..tri_count).collect();
    let mut nodes = Vec::new();
    build(&mut nodes, &order, &tris, &positions, shrink);

    // The probes, as short segments through the same space, derived the way the original derives them.
    let mut bundle = Bundle { count: 1 + rng.below(MAX_PROBES as u32) as usize, ..Bundle::default() };
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for i in 0..bundle.count {
        // Half the probes go somewhere at random, which is mostly how a probe misses. The other half
        // are aimed through a point on a triangle — sometimes just off its edge, sometimes stopping
        // short of it, sometimes from behind, which is how the parts of the test that matter get
        // reached at all. Aiming them is the generator's arithmetic and proves nothing by itself; what
        // it buys is that the containment test, the backface cull and the length cut-off are each
        // exercised thousands of times rather than a handful.
        let (from, to) = if rng.below(2) == 0 {
            let t = &tris[rng.below(tri_count as u32) as usize];
            let [a, b, c] = t.vertices.map(|v| positions[v as usize]);
            let (u, w) = (rng.below(69) as f32 * 0.015_625, rng.below(69) as f32 * 0.015_625);
            let on = [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * u + (c[k] - a[k]) * w);
            let off = [rng.coord(2), rng.coord(2), rng.coord(2)];
            let back = rng.below(5) as f32 * 0.5;
            (Vf([on[0] + off[0], on[1] + off[1], on[2] + off[2], 1.0]),
             Vf([on[0] - off[0] * back, on[1] - off[1] * back, on[2] - off[2] * back, 1.0]))
        } else {
            let from = rng.point(5);
            (from, Vf([from.x() + rng.coord(2), from.y() + rng.coord(2), from.z() + rng.coord(2), 1.0]))
        };
        bundle.probes[i] = Probe::between(from, to);
        for k in 0..3 {
            lo[k] = lo[k].min(from.0[k]).min(to.0[k]);
            hi[k] = hi[k].max(from.0[k]).max(to.0[k]);
        }
    }
    let centre = [0, 1, 2].map(|k| (lo[k] + hi[k]) * 0.5);
    let radius = (0..3).map(|k| (hi[k] - lo[k]) * 0.5).fold(0.0f32, f32::max);
    bundle.centre = Vf([centre[0], centre[1], centre[2], 0.0]);
    // Every fifth bundle gets a radius too small to cover itself, which is how the whole-bundle
    // rejection at a node gets exercised.
    bundle.radius = F(if rng.below(5) == 0 { radius * 0.25 } else { radius });

    Scene { verts, tris, nodes, bundle }
}

/// Build the tree over a range of triangles, root first.
#[allow(clippy::float_arithmetic)]
fn build(nodes: &mut Vec<Node>, items: &[usize], tris: &[Triangle], positions: &[[f32; 3]], shrink: f32) -> Child {
    if items.len() == 1 {
        return Child::Leaf(items[0] as u16);
    }
    let mine = nodes.len();
    nodes.push(Node { volume: F(0.0), centre: [F(0.0); 3], half_extent: [F(0.0); 3], children: [0, 0] });
    let mid = items.len() / 2;
    let left = build(nodes, &items[..mid], tris, positions, shrink);
    let right = build(nodes, &items[mid..], tris, positions, shrink);

    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for &t in items {
        for v in tris[t].vertices {
            let p = positions[v as usize];
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
    }
    nodes[mine] = Node {
        volume: F(0.0),
        centre: [0, 1, 2].map(|k| F((lo[k] + hi[k]) * 0.5)),
        half_extent: [0, 1, 2].map(|k| F((hi[k] - lo[k]) * 0.5 * shrink)),
        children: [word(left), word(right)],
    };
    Child::Node(mine as u16)
}

/// A child as the tree stores it: the top bit marks a leaf.
fn word(c: Child) -> u16 {
    match c {
        Child::Node(i) => i,
        Child::Leaf(i) => i | 0x8000,
    }
}

// --- writing a scene into the interpreter -----------------------------------------------------------

fn put_v(ee: &mut Ee, at: u32, v: Vf) {
    for (k, x) in v.0.iter().enumerate() {
        ee.set_f32(at + 4 * k as u32, *x);
    }
}

fn get_v(ee: &Ee, at: u32) -> [u32; 4] {
    [0, 1, 2, 3].map(|k| ee.u32_at(at + 4 * k))
}

fn install(ee: &mut Ee, s: &Scene) {
    for (i, n) in s.nodes.iter().enumerate() {
        let at = NODES + 0x24 * i as u32;
        // The node as the loader leaves it in memory: the key the dual-tree descent sorts on, the two
        // child words, then the box. (The file's own node is a few bytes shorter and in another order;
        // that is the loader's business, not the sweep's.)
        ee.set_u32(at, 0);
        ee.set_u32(at + 4, 0);
        ee.write(at + 8, 2, n.children[0] as u128);
        ee.write(at + 0xa, 2, n.children[1] as u128);
        for k in 0..3 {
            ee.set_f32(at + 0xc + 4 * k, n.centre[k as usize].0);
            ee.set_f32(at + 0x18 + 4 * k, n.half_extent[k as usize].0);
        }
    }
    for (i, t) in s.tris.iter().enumerate() {
        let at = TRIS + 0x14 * i as u32;
        for k in 0..5 {
            ee.set_u32(at + 4 * k, 0);
        }
        for k in 0..3 {
            ee.write(at + 2 * k, 2, t.vertices[k as usize] as u128);
        }
    }
    match &s.verts {
        Points::Wide(v) => {
            for (i, p) in v.iter().enumerate() {
                for k in 0..3 {
                    ee.set_f32(VERTS + 12 * i as u32 + 4 * k, p[k as usize].0);
                }
            }
        }
        Points::Packed(v) => {
            for (i, p) in v.iter().enumerate() {
                for k in 0..3 {
                    ee.write(VERTS + 6 * i as u32 + 2 * k, 2, p[k as usize] as u16 as u128);
                }
            }
        }
    }
    ee.set_u32(DESC, VERTS);
    ee.set_u32(DESC + 4, 0); // no per-vertex normals: contact normals stay the triangle's own
    ee.set_u32(DESC + 8, matches!(s.verts, Points::Packed(_)) as u32);
    ee.set_u32(TREES, NODES);
    ee.set_u32(TREES + 4, TRIS);

    ee.set_u32(OBJ + 0x120, s.bundle.count as u32);
    ee.set_u32(OBJ + 0x12c, ORIGINS);
    ee.set_u32(OBJ + 0x130, DIRS);
    ee.set_u32(OBJ + 0x134, LENGTHS);
    put_v(ee, OBJ + 0x140, s.bundle.centre);
    ee.set_f32(OBJ + 0x160, s.bundle.radius.0);
    for i in 0..MAX_PROBES as u32 {
        let p = s.bundle.probes[i as usize];
        put_v(ee, ORIGINS + 16 * i, p.origin);
        put_v(ee, DIRS + 16 * i, p.dir);
        ee.set_f32(LENGTHS + 4 * i, p.length.0);
    }

    // The record, as the caller's own clear step leaves it.
    for k in 0..0xc0 / 4 {
        ee.set_u32(REC + 4 * k, 0);
    }
    ee.set_u32(REC + 0x94, NORMALS);
    ee.set_u32(REC + 0x98, POINTS);
    ee.set_u32(REC + 0xa0, OWNERS);
    ee.set_u32(REC + 0xa4, FLAGS);
    ee.set_u32(REC + 0xa8, FRACTIONS);
    ee.set_f32(REC + 0xb0, 1.0);
    for i in 0..MAX_PROBES as u32 {
        ee.set_f32(FRACTIONS + 4 * i, -1.0);
        ee.set_u32(FLAGS + 4 * i, 0);
        ee.set_u32(OWNERS + 4 * i, 0);
        for k in 0..4 {
            ee.set_u32(POINTS + 16 * i + 4 * k, 0);
            ee.set_u32(NORMALS + 16 * i + 4 * k, 0);
        }
    }
}

/// Every difference between the original's record and the port's, as sentences.
fn differences(ee: &Ee, s: &Scene, got: &Record) -> Vec<String> {
    let mut out = Vec::new();
    let mut say = |what: String| out.push(what);
    for i in 0..s.bundle.count {
        let k = i as u32;
        let theirs = ee.u32_at(FRACTIONS + 4 * k);
        if theirs != got.fraction[i].to_bits() {
            say(format!("probe {i} fraction: theirs {theirs:#010x}, ours {:#010x}", got.fraction[i].to_bits()));
        }
        let flag = ee.u32_at(FLAGS + 4 * k);
        if (flag != 0) != got.hit[i] {
            say(format!("probe {i} hit flag: theirs {flag}, ours {}", got.hit[i]));
        }
        let owner = ee.i32_at(OWNERS + 4 * k);
        if owner != got.owner[i] {
            say(format!("probe {i} owner: theirs {owner}, ours {}", got.owner[i]));
        }
        for (name, at, ours) in [("point", POINTS, got.point[i].to_bits()),
                                 ("normal", NORMALS, got.normal[i].to_bits())] {
            let theirs = get_v(ee, at + 16 * k);
            if theirs != ours {
                say(format!("probe {i} {name}: theirs {theirs:08x?}, ours {ours:08x?}"));
            }
        }
    }
    let hits = ee.i32_at(REC + 0x8c);
    if hits != got.hits {
        say(format!("hit tally: theirs {hits}, ours {}", got.hits));
    }
    let nearest = ee.u32_at(REC + 0xb0);
    if nearest != got.nearest.to_bits() {
        say(format!("nearest fraction: theirs {nearest:#010x}, ours {:#010x}", got.nearest.to_bits()));
    }
    if got.nearest_probe.is_some() {
        for (name, at, ours) in [
            ("nearest normal", REC + 0x20, got.nearest_normal.to_bits()),
            ("nearest point", REC + 0x30, got.nearest_point.to_bits()),
            ("push-out", REC + 0x40, got.push.to_bits()),
        ] {
            let theirs = get_v(ee, at);
            if theirs != ours {
                say(format!("{name}: theirs {theirs:08x?}, ours {ours:08x?}"));
            }
        }
    }
    out
}

/// The sweep, over ten thousand generated scenes.
#[test]
fn the_mesh_sweep_matches_the_original() {
    let Some(mut ee) = game_or_skip() else { return };
    let sweep = addr("mesh_sweep").unwrap();
    let (mut hits, mut swept, mut pushes, mut steps) = (0u64, 0u64, 0u64, 0u64);
    for seed in 1..=SCENES as u64 {
        let s = scene(seed);
        install(&mut ee, &s);
        ee.call(sweep, &[OBJ as u64, 0, TREES as u64, DESC as u64, REC as u64, 1], &[], 4_000_000);
        steps += ee.steps;

        let mut got = Record::new();
        let mesh = MeshRef { nodes: &s.nodes, triangles: &s.tris, vertices: &s.verts };
        collide::sweep(&s.bundle, &mesh, &mut got);

        let bad = differences(&ee, &s, &got);
        assert!(bad.is_empty(), "scene {seed} ({} probes, {} triangles, {} nodes):\n  {}",
                s.bundle.count, s.tris.len(), s.nodes.len(), bad.join("\n  "));
        swept += s.bundle.count as u64;
        hits += got.hits as u64;
        pushes += got.nearest_probe.is_some() as u64;
    }
    println!("{SCENES} scenes, {swept} probes swept, {hits} triangle hits, {pushes} of them resolved to \
              a push-out, {steps} instructions of the original");
    assert!(pushes > SCENES as u64 / 50, "only {pushes} scenes hit anything: the generator stopped generating hits");
}

/// The vertex fetch, in both of the widths a collision mesh is stored in.
#[test]
fn the_vertex_fetch_matches_the_original() {
    let Some(mut ee) = game_or_skip() else { return };
    let fetch = addr("mesh_triangle_vertices").unwrap();
    let out = [ee.scratch(0), ee.scratch(1), ee.scratch(2)];
    let mut rng = Rng(0x5eed);
    for packed in [false, true] {
        ee.set_u32(DESC, VERTS);
        ee.set_u32(DESC + 4, 0);
        ee.set_u32(DESC + 8, packed as u32);
        for _ in 0..SAMPLES / 2 {
            // Three vertices, and a triangle that indexes them in a random order.
            let mut positions = Vec::new();
            let mut shorts = Vec::new();
            for i in 0..3u32 {
                if packed {
                    let s = [rng.next() as i16, rng.next() as i16, rng.next() as i16];
                    for (k, x) in s.iter().enumerate() {
                        ee.write(VERTS + 6 * i + 2 * k as u32, 2, *x as u16 as u128);
                    }
                    shorts.push(s);
                } else {
                    let p = [rng.coord(500), rng.coord(500), rng.coord(500)];
                    for (k, x) in p.iter().enumerate() {
                        ee.set_f32(VERTS + 12 * i + 4 * k as u32, *x);
                    }
                    positions.push([F(p[0]), F(p[1]), F(p[2])]);
                }
            }
            let verts = if packed { Points::Packed(shorts) } else { Points::Wide(positions) };
            let tri = Triangle {
                distance: F(0.0),
                normal: [F(0.0); 3],
                vertices: [rng.below(3) as u16, rng.below(3) as u16, rng.below(3) as u16],
            };
            for k in 0..3 {
                ee.write(TRIS + 2 * k, 2, tri.vertices[k as usize] as u128);
            }
            ee.call(fetch, &[out[0] as u64, out[1] as u64, out[2] as u64, TRIS as u64, DESC as u64], &[], 10_000);

            let mesh = MeshRef { nodes: &[], triangles: &[], vertices: &verts };
            for (k, &at) in out.iter().enumerate() {
                let theirs = get_v(&ee, at);
                let ours = mesh.vertex(tri.vertices[k]).to_bits();
                assert_eq!(theirs, ours, "vertex {} of {tri:?} ({}): theirs {theirs:08x?}, ours {ours:08x?}",
                           tri.vertices[k], if packed { "packed" } else { "wide" });
            }
        }
    }
    println!("{SAMPLES} vertex fetches, both widths");
}

/// A probe from two points: the length and the unit direction, as the original's own helpers produce
/// them. These are what a caller runs once per frame per probe, before any mesh is swept.
#[test]
fn a_probe_is_derived_the_way_the_original_derives_one() {
    let Some(mut ee) = game_or_skip() else { return };
    let (length, div) = (addr("vec3_length").unwrap(), addr("vec3_div_scalar").unwrap());
    let v = ee.scratch(0);
    let mut rng = Rng(0xc0ffee);
    for _ in 0..SAMPLES {
        let from = rng.point(500);
        let to = rng.point(500);
        let ours = Probe::between(from, to);

        let delta = to.sub(from);
        put_v(&mut ee, v, delta);
        ee.call(length, &[v as u64], &[], 10_000);
        let theirs = ee.f0();
        assert_eq!(theirs.to_bits(), ours.length.to_bits(),
                   "length of {delta:?}: theirs {:#010x}, ours {:#010x}", theirs.to_bits(), ours.length.to_bits());
        ee.call(div, &[v as u64], &[theirs], 10_000);
        let theirs = get_v(&ee, v);
        assert_eq!(theirs, ours.dir.to_bits(), "direction of {delta:?} over {theirs:?}");
    }
    println!("{SAMPLES} probe derivations");
}

/// The three rules of the shape test that a "better" collision would quietly drop, as plain assertions
/// that need no disc: a triangle is solid from one side only, a hit resolves to the travel that is
/// left over along the normal, and the bundle carries sixteen probes.
#[test]
fn the_shape_tests_own_rules() {
    // A metre-square floor at y = 0, wound so its normal points up — which is the winding whose two
    // edges cross to +y, and not the other one: the triangle is solid from this side only.
    let floor = [Vf([-1.0, 0.0, -1.0, 1.0]), Vf([-1.0, 0.0, 1.0, 1.0]), Vf([1.0, 0.0, -1.0, 1.0])];
    let down = Probe::between(Vf([0.0, 1.0, 0.0, 1.0]), Vf([0.0, -1.0, 0.0, 1.0]));
    let (fraction, point, normal) = collide::test(&down, floor).expect("a probe straight down hits the floor");
    assert_eq!(fraction, F(0.5), "half way down the two-unit probe");
    assert_eq!(point.0[..3], [0.0, 0.0, 0.0], "at the origin");
    assert!(normal.y() > 0.0, "the floor faces up, not down: {normal:?}");

    let up = Probe::between(Vf([0.0, -1.0, 0.0, 1.0]), Vf([0.0, 1.0, 0.0, 1.0]));
    assert!(collide::test(&up, floor).is_none(), "the underside of a triangle is not solid");

    // Through a corner, which the flattened containment test must still call inside, and past one,
    // which it must not.
    let near = Probe::between(Vf([-0.99, 1.0, -0.99, 1.0]), Vf([-0.99, -1.0, -0.99, 1.0]));
    assert!(collide::test(&near, floor).is_some(), "just inside a corner is inside");
    let past = Probe::between(Vf([0.6, 1.0, 0.6, 1.0]), Vf([0.6, -1.0, 0.6, 1.0]));
    assert!(collide::test(&past, floor).is_none(), "beyond the diagonal is outside");

    // The resolution: a probe stopped half way has half a unit of travel left, all of it along the
    // normal, so the blocked travel is half a unit *downwards* — the direction the probe was going, of
    // one unit of travel and not of the probe's two. It is what a caller takes away from the motion,
    // not a vector that lifts anything out of anything.
    let mut bundle = Bundle { count: 1, centre: Vf([0.0, 0.0, 0.0, 0.0]), radius: F(2.0), ..Bundle::default() };
    bundle.probes[0] = down;
    let tris = [Triangle { distance: F(0.0), normal: [F(0.0); 3], vertices: [0, 1, 2] },
                Triangle { distance: F(0.0), normal: [F(0.0); 3], vertices: [0, 1, 2] }];
    let nodes = vec![Node {
        volume: F(0.0),
        centre: [F(0.0); 3],
        half_extent: [F(1.0), F(0.1), F(1.0)],
        children: [0x8000, 0x8001],
    }];
    let verts = Points::Wide(floor.iter().map(|v| [F(v.x()), F(v.y()), F(v.z())]).collect());
    let mesh = MeshRef { nodes: &nodes, triangles: &tris, vertices: &verts };
    let mut rec = Record::new();
    assert!(collide::sweep(&bundle, &mesh, &mut rec));
    assert_eq!(rec.hits, 2, "both leaves are the same triangle, so the probe hits twice");
    assert_eq!(rec.nearest, F(0.5));
    assert_eq!(rec.push.0[..3], [0.0, -0.5, 0.0], "half a unit of leftover travel, along the normal");
    assert_eq!(rec.nearest_probe, Some(0));
    assert_eq!(MAX_PROBES, 16, "the mask the sweep keeps per level is sixteen words");
}
