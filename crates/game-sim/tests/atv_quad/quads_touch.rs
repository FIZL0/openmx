//! MD98a — two quads' collision records resolving their contacts with each other, against C38's image.
//!
//! C38's `c38-37523.ee.bin` is an ATV race (Desert Oasis, SLUS-21104) on the frame two AI quads touch:
//! each one's record holds four contacts naming the other's record (research/oracle.txt `# C38`), its
//! own (kind 0) and the mirrors of the other's (kind 1). The port refills each record's list from those
//! contacts with the other quad's record and body held ([`Held::refill_among`]) and resolves it as the
//! gravity's call does ([`Held::resolve`]); the original runs the record's vt `+0x88` handed 1. First
//! the one record's call, then the other's on what the first left: after each, every word the port
//! writes (both records' centres in the world, normal speed and relative velocity, the caller's entries'
//! arms and relative velocities, both bodies whole) is the original's, and the original writes nothing
//! else outside its stack.

use super::*;
use game_sim::atv_contact::{Held, Knocked};
use game_sim::handling::record::{self, Contact as Filed};

/// The touching pair's records (research/oracle.txt `# C38`), and SLUS-21104's `$gp`.
const RECORDS: [u32; 2] = [0x01a5_11b0, 0x01aa_5020];
const GP: u64 = 0x003d_baf0;

fn c38_image() -> Option<Ee> {
    c38("c38-37523")
}

/// C38's image `name` (`c38-37523` the touch, `c38-37521` the frame before it).
fn c38(name: &str) -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2/c38").join(format!("{name}.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!("{} is not there (C38's images, research/c38)", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, GP);
    Some(ee)
}

fn v(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| f32::from_bits(ee.u32_at(a + k))))
}

/// The record at `rec` as [`record::held`] reads it, its list left out.
fn held_bare(ee: &Ee, rec: u32) -> Held {
    let w = |a: u32| if a == rec + 0xf0 { 0 } else { ee.u32_at(a) };
    record::held(&w, rec).expect("a list left out")
}

/// The record at `rec` and its body as another quad's resolution pushes them.
fn knocked(ee: &Ee, rec: u32) -> Knocked {
    let h = held_bare(ee, rec);
    Knocked { collider: h.record.collider, body: h.body.expect("a body"), mass: h.mass, cofg: v(ee, ee.u32_at(rec + 0xe4) + 0x1a0) }
}

/// The contacts in the record's list at `rec` as its pass filed them.
fn filed(ee: &Ee, rec: u32) -> Vec<Filed> {
    let list = ee.u32_at(rec + 0xf8);
    (0..ee.u32_at(rec + 0xf0))
        .map(|k| {
            let e = list + record::CONTACT * k;
            Filed {
                kind: ee.u32_at(e),
                from: v(ee, e + 0x10),
                to: v(ee, e + 0x20),
                normal: v(ee, e + 0x30),
                point: v(ee, e + 0x40),
                surface: ee.u32_at(e + 0x80),
                other: ee.u32_at(e + 0x84),
                mover: ee.u32_at(e + 0x88),
            }
        })
        .collect()
}

/// The record at `rec` as the gravity's call holds it, refilled from its list with `other` (the other
/// quad's record, as the port last left it) held; each entry's relative velocity the list slot's.
fn held(ee: &Ee, rec: u32, other_rec: u32, other: Knocked) -> Held {
    let mut h = held_bare(ee, rec);
    let list = ee.u32_at(rec + 0xf8);
    h.entries = (0..ee.u32_at(rec + 0xf0))
        .map(|k| game_sim::atv_contact::Entry { relative: v(ee, list + record::CONTACT * k + 0x70), ..Default::default() })
        .collect();
    assert!(h.refill_among(&filed(ee, rec), vec![other], &[other_rec]), "the list naming the other quad was refused");
    h
}

/// **Two AI quads' records resolve their contacts with each other as the original's** (C38's touch).
#[test]
fn two_quads_records_resolve_their_touch_as_the_originals() {
    let Some(mut ee) = c38_image() else { return };
    let resolve_at = ps2_oracle::addr("atv_record_resolve").unwrap();
    for (k, &rec) in RECORDS.iter().enumerate() {
        let other = RECORDS[1 - k];
        assert_eq!(ee.u32_at(ee.u32_at(rec + 4) + 0x88), resolve_at, "{rec:#x}'s vt +0x88");
        assert_eq!(ee.u32_at(ee.u32_at(rec + 0xe4) + 0xe8), 0, "{rec:#x}'s body has listeners");
        let f = filed(&ee, rec);
        assert_eq!(f.len(), 4, "{rec:#x}'s list");
        assert!(f.iter().all(|c| c.other == other && c.mover == 0), "{rec:#x}'s contacts are not all with {other:#x}");
    }
    let kinds = |rec: u32| filed(&ee, rec).iter().map(|c| c.kind).collect::<Vec<_>>();
    assert_eq!((kinds(RECORDS[0]), kinds(RECORDS[1])), (vec![0, 1, 1, 1], vec![1, 0, 0, 0]), "the lists' kinds");

    // The port carries the other quad's record and body from one call to the next.
    let mut theirs = knocked(&ee, RECORDS[1]);
    for (k, &rec) in RECORDS.iter().enumerate() {
        let other = RECORDS[1 - k];
        let mut port = held(&ee, rec, other, theirs);
        assert!(port.entries.iter().all(|e| e.contact.other == Some(0)), "{rec:#x}: an entry not held with the other quad");
        let cofg = v(&ee, ee.u32_at(rec + 0xe4) + 0x1a0);
        let before = (port.body, port.bodies[0].body);
        assert!(port.resolve(cofg), "{rec:#x}: the port did not resolve");
        assert_ne!((port.body, port.bodies[0].body), before, "{rec:#x}: neither body was pushed");

        ee.written.clear();
        ee.call(resolve_at, &[u64::from(rec), 1], &[], 400_000);

        let mut compared = std::collections::BTreeSet::new();
        let pushed = port.bodies[0];
        let other_held = Held { record: game_sim::atv_contact::Record { collider: pushed.collider, ..Default::default() }, body: Some(pushed.body), ..Default::default() };
        let words = |a: u32| ee.u32_at(a);
        let mine: Vec<(u32, u32)> = record::held_words(&port, rec, &words).into_iter().chain(record::held_words(&other_held, other, &words)).collect();
        for (a, w) in mine {
            assert_eq!(w, ee.u32_at(a), "{rec:#x}'s call: the port wrote {w:#010x} at {a:#x}, the original {:#010x}", ee.u32_at(a));
            compared.extend(a..a + 4);
        }
        let stray: Vec<u32> = ee.written.iter().copied().filter(|&a| !(ee.stack - 0x4000..ee.stack).contains(&a) && !compared.contains(&a)).collect();
        assert!(stray.is_empty(), "{rec:#x}'s call: the original wrote what the port does not: {:x?}", &stray[..stray.len().min(16)]);

        // The next call's other is this one's record, as the port left it.
        theirs = Knocked { collider: port.record.collider, body: port.body.expect("the body"), mass: port.mass, cofg };
    }
}


// --- S481: the passes' hits on the other quad's hull ---------------------------------------------------

/// A collidable's kind word (`+0x78`: 0 a mesh, 2 a ray probe), its record (`+0x90`) and category
/// (`+0x94`), its frame (`+0x130`); a ray probe's node (`+0x174`), a mesh's (`+0x240`), and a node's
/// world (`+0x70`). The record's part (`+0x200`, the rays it runs on), its hull (`+0x208`) and the
/// category mask its hits are tested against (`+0x108`). A part's record (`+0x7c`): its hits' count
/// (`+0x8c`), rays (`+0x90`, two quadwords each), normals (`+0x94`), points (`+0x98`), struck (`+0x9c`).
mod at {
    pub const KIND: u32 = 0x78;
    pub const RECORD: u32 = 0x90;
    pub const CATEGORY: u32 = 0x94;
    pub const FRAME: u32 = 0x130;
    pub const PROBE_NODE: u32 = 0x174;
    pub const MESH_NODE: u32 = 0x240;
    pub const NODE_WORLD: u32 = 0x70;
    pub const PART: u32 = 0x200;
    pub const HULL: u32 = 0x208;
    pub const MASK: u32 = 0x108;
    pub const HITS: u32 = 0x7c;
    pub const HIT_COUNT: u32 = 0x8c;
    pub const HIT_RAYS: u32 = 0x90;
    pub const HIT_NORMALS: u32 = 0x94;
    pub const HIT_POINTS: u32 = 0x98;
    pub const HIT_STRUCK: u32 = 0x9c;
}

fn frame_at(ee: &Ee, a: u32) -> [Vf; 4] {
    [0, 1, 2, 3].map(|k| v(ee, a + 0x10 * k))
}

fn bits(v: Vf) -> [u32; 4] {
    v.0.map(f32::to_bits)
}

/// A filed contact as words, so two compare bit for bit.
fn contact_words(c: &Filed) -> (u32, [[u32; 4]; 4], u32, u32, u32) {
    (c.kind, [bits(c.from), bits(c.to), bits(c.normal), bits(c.point)], c.surface, c.other, c.mover)
}

/// A tree as it sits in RAM, 36 bytes a node, read from `root` down; and one past its highest leaf.
fn tree_at(ee: &Ee, nodes: u32, root: u16) -> (Vec<game_sim::track::Node>, usize) {
    let blank = game_sim::track::Node { volume: F::ZERO, centre: [F::ZERO; 3], half_extent: [F::ZERO; 3], children: [0, 0] };
    let (mut out, mut leaves, mut todo) = (Vec::new(), 0, vec![root]);
    while let Some(w) = todo.pop() {
        if w & 0x8000 != 0 {
            leaves = leaves.max(usize::from(w & 0x7fff) + 1);
            continue;
        }
        let n = nodes + 36 * u32::from(w);
        if out.len() <= usize::from(w) {
            out.resize(usize::from(w) + 1, blank);
        }
        let f = |a: u32| F(f32::from_bits(ee.u32_at(a)));
        let three = |a: u32| [0, 4, 8].map(|k| f(a + k));
        let children = [ee.u16_at(n + 8), ee.u16_at(n + 10)];
        assert!(out[usize::from(w)] == blank, "node {w} at {nodes:#x} reached twice");
        out[usize::from(w)] = game_sim::track::Node { volume: f(n), centre: three(n + 12), half_extent: three(n + 24), children };
        todo.extend(children);
    }
    (out, leaves)
}

/// The ray probe `part`'s segments, in its own frame (`+0x178` count, `+0x180` the pairs).
fn part_rays(ee: &Ee, part: u32) -> Vec<[Vf; 2]> {
    let segs = ee.u32_at(part + 0x180);
    (0..ee.u32_at(part + 0x178)).map(|i| [v(ee, segs + 0x20 * i), v(ee, segs + 0x20 * i + 0x10)]).collect()
}

/// **The hull mesh the port strikes is the collidable the original's pass strikes**: the quad's
/// `ATV_R_ChasisF.col` against the hull collidable `c` in RAM (its tree from `+0x23c`/`+0x244`, its
/// triangles `+0x248`, its vertices through `+0x230`), every word.
fn hull_is_the_collidables(ee: &Ee, c: u32, hull: &game_sim::track::Mesh) {
    assert_eq!(ee.u16_at(c + 0x23c), 0, "{c:#x}'s tree does not start at node 0, as the port's does");
    let (nodes, count) = tree_at(ee, ee.u32_at(c + 0x244), 0);
    assert_eq!(nodes, hull.nodes, "{c:#x}'s tree");
    let facets = game_sim::flight_search::facets(hull);
    let at = ee.u32_at(c + 0x248);
    let ram: Vec<game_sim::ray_probe::Facet> = (0..count as u32)
        .map(|i| {
            let f = at + 0x14 * i;
            game_sim::ray_probe::Facet { vertices: [0, 2, 4].map(|k| ee.u16_at(f + k)), normal: [0, 2, 4].map(|k| ee.u16_at(f + 0xc + k) as i16) }
        })
        .collect();
    assert_eq!(ram, facets, "{c:#x}'s triangles");
    assert_eq!(ee.u32_at(c + 0x230 + 8), 0, "{c:#x}'s vertices are packed");
    let array = ee.u32_at(c + 0x230);
    let corners = facets.iter().flat_map(|f| f.vertices).max().map_or(0, |m| u32::from(m) + 1);
    let ram = game_sim::track::Points::Wide((0..corners).map(|i| [0, 4, 8].map(|k| F(f32::from_bits(ee.u32_at(array + 12 * i + k))))).collect());
    assert_eq!(ram, hull.vertices, "{c:#x}'s vertices");
}

/// The record at `rec` as its pass starts on C38's frame before the touch (its list empty), with
/// the sweep back off (see the test).
fn record_before(ee: &Ee, rec: u32) -> record::Record {
    let w = |a: u32| ee.u32_at(a);
    let mut r = record::read(&w, rec);
    assert!(r.contacts.is_empty(), "{rec:#x}'s list is not empty before the touch");
    r.sweep = false;
    r
}

/// **S481 — each quad's record pass strikes the other quad's hull as the original's** (C38).
///
/// On C38's frame before the touch (`c38-37521`, every record empty, every part and hull where the
/// touch frame's passes find them), each record's pass is the port's ([`quad::RecordAmong`] over
/// [`record::pass`]): its rays (the record's part's, in its scene node's frame) struck against the
/// other quad's hull ([`quad::Knockable`]: the `ATV_R_ChasisF.col` mesh standing at its node, in the
/// category [`quad::RECORD_CLASS`], its record the other's). The passes run as
/// [`quad::step_together`] runs them, record 0x01a511b0's then 0x01aa5020's, each one's mirrors on
/// the other's record. Both lists then equal the touch image's (`c38-37523`) word for word: the
/// rays, normals, points, kinds, surfaces and names of all four entries of each, in their order.
///
/// Off the touch image, what the original struck with: each record's part (`+0x200`) is a ray probe
/// (kind 2) and its result record holds exactly the pass's own hits, every one on the other quad's
/// hull (`+0x208`), a mesh (kind 0) whose record is the other's and whose category is
/// [`quad::RECORD_CLASS`], not in the striking record's mask. The port strikes it with the ray
/// probe's pair ([`game_sim::ray_probe::pair`], the part search's, F46d), the part's tree the riding
/// `.col`'s as stored (the part's tree in RAM is that file's, node for node; a tree built off the rays
/// files the mirrors in another order). The other order (0x01aa5020's pass first) files different
/// lists: the order is what makes them the original's.
#[test]
fn the_passes_strike_the_other_quads_hull_as_the_originals() {
    let (Some(before), Some(touch), Some(hull), Some((riding, _))) = (c38("c38-37521"), c38("c38-37523"), super::mxu::quad_hull(), super::mxu::quad_cols()) else { return };
    let tree = game_sim::flight_search::Rays::of(&riding);
    // What the original's pass struck, and with what.
    for (k, &rec) in RECORDS.iter().enumerate() {
        let other = RECORDS[1 - k];
        let part = touch.u32_at(rec + at::PART);
        assert_eq!(part, before.u32_at(rec + at::PART), "{rec:#x} swapped its rays");
        assert_eq!(touch.u32_at(part + at::KIND), 2, "{rec:#x}'s part is not a ray probe");
        let hits = touch.u32_at(part + at::HITS);
        let n = touch.u32_at(hits + at::HIT_COUNT);
        let own: Vec<Filed> = filed(&touch, rec).into_iter().filter(|c| c.kind == 0).collect();
        assert_eq!(n as usize, own.len(), "{rec:#x}'s part's hits are not its own entries");
        for (i, c) in (0..n).zip(&own) {
            let struck = touch.u32_at(touch.u32_at(hits + at::HIT_STRUCK) + 4 * i);
            assert_eq!(struck, touch.u32_at(other + at::HULL), "{rec:#x}'s hit {i} is not on {other:#x}'s hull");
            assert_eq!(touch.u32_at(struck + at::KIND), 0, "{other:#x}'s hull is not a mesh");
            assert_eq!(touch.u32_at(struck + at::RECORD), other, "{other:#x}'s hull's record");
            assert_eq!(touch.u32_at(struck + at::CATEGORY), quad::RECORD_CLASS, "{other:#x}'s hull's category");
            assert_eq!(touch.u32_at(rec + at::MASK) & quad::RECORD_CLASS, 0, "{rec:#x}'s mask");
            let ray = touch.u32_at(hits + at::HIT_RAYS) + 0x20 * i;
            let got = (v(&touch, ray), v(&touch, ray + 0x10), v(&touch, touch.u32_at(hits + at::HIT_NORMALS) + 0x10 * i), v(&touch, touch.u32_at(hits + at::HIT_POINTS) + 0x10 * i));
            assert_eq!([got.0, got.1, got.2, got.3].map(bits), [c.from, c.to, c.normal, c.point].map(bits), "{rec:#x}'s hit {i} against its entry");
        }
    }
    // The port's hull is that collidable, and the frames the passes stand at are the nodes'.
    let mut stand = Vec::new();
    for &rec in &RECORDS {
        let (part, h) = (before.u32_at(rec + at::PART), before.u32_at(rec + at::HULL));
        hull_is_the_collidables(&before, h, &hull);
        let rays_at = frame_at(&before, part + at::FRAME);
        let hull_at = frame_at(&before, h + at::FRAME);
        assert_eq!(rays_at.map(bits), frame_at(&before, before.u32_at(part + at::PROBE_NODE) + at::NODE_WORLD).map(bits), "{rec:#x}'s rays are not at their node");
        assert_eq!(hull_at.map(bits), frame_at(&before, before.u32_at(h + at::MESH_NODE) + at::NODE_WORLD).map(bits), "{rec:#x}'s hull is not at its node");
        assert_eq!(rays_at.map(bits), hull_at.map(bits), "{rec:#x}'s rays and hull stand apart");
        let root = before.u16_at(part + 0x170);
        assert_eq!(root, tree.root, "{rec:#x}'s part's tree's root");
        assert_eq!(tree_at(&before, before.u32_at(part + 0x17c), root).0, tree.nodes, "{rec:#x}'s part's tree is not the riding file's");
        assert_eq!(part_rays(&before, part), tree.pairs(), "{rec:#x}'s part's rays are not the riding file's");
        stand.push((rays_at, tree.pairs()));
    }
    let dt = F::from_bits(0x3d08_850a);
    let passes = |order: [usize; 2]| {
        let mut records = RECORDS.map(|rec| record_before(&before, rec));
        for i in order {
            let j = 1 - i;
            let mut scene = game_sim::atv_moto::Scene::default();
            scene.nodes[0].local = stand[i].0;
            let mut flat = game_sim::world::Flat { y: F(-1.0e4), ..Default::default() };
            let on = quad::RecordOn { world: &mut flat, scene: &mut scene, rays: &stand[i].1, damped: None, category: 0, nobody: Default::default(), stood: None };
            let theirs = std::mem::take(&mut records[j]);
            let mut knocked = [quad::Knockable::new(RECORDS[j], hull.clone(), stand[j].0, theirs).with_category(quad::RECORD_CLASS)];
            let mut among = quad::RecordAmong { on, knocked: &mut knocked, tree: Some(tree.clone()) };
            record::pass(&mut records[i], RECORDS[i], None, dt, &mut among);
            let [k] = knocked;
            records[j] = k.record;
        }
        records
    };
    let records = passes([0, 1]);
    for (k, &rec) in RECORDS.iter().enumerate() {
        let port: Vec<_> = records[k].contacts.iter().map(contact_words).collect();
        let original: Vec<_> = filed(&touch, rec).iter().map(contact_words).collect();
        assert_eq!(port, original, "{rec:#x}'s list");
        assert!(records[k].touching && records[k].touching_other && records[k].on_object, "{rec:#x}'s touch flags");
    }
    let swapped = passes([1, 0]);
    assert_ne!(swapped[0].contacts, records[0].contacts, "the other order files the same list");
}


