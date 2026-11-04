//! MD75e — **the quad's whole physics frame off the image**: SLUS-21104's world step
//! (`atv_world_step`) over the game's own physics world in MD37a's `atv-r-spawn.ee.bin`, with nothing
//! hooked but the ground query (`atv_ground_query`), against the port's [`quad::frame`] run on the
//! [`image::read_ride`] read of the same image, every word of the quad, its body and its wheels equal.
//!
//! The ground query is answered on both sides by [`atv_ground::Terrain::ground`] over Desert Oasis's
//! terrain file (MD76a proves it against the original's query). The port's [`quad::Steps`] answers
//! every call itself: the bike steps, the quad's own calls, the vehicle post and its callees, the
//! collision sync and the gather ([`atv_search::Oasis`]), and the distributor's sweep through MD86's
//! search. What it hands on (`rest`) is listed and checked against what may go there.

use super::*;
use game_sim::atv_contact;
use game_sim::atv_ground;
use game_sim::atv_search;
use game_sim::handling::seed;

/// The game's physics world (`research/oracle.txt` `atv_world`), the terrain object the wheels stand
/// on, the nearby list the quad keeps (`+0xc4`), and SLUS-21104's `$gp`.
const IMAGE_WORLD: u32 = 0x0065_bc0c;
const TERRAIN: u32 = 0x0184_c7d0;
const IMAGE_LIST: u32 = 0x0162_7a40;
const GP: u64 = 0x003d_baf0;

/// Desert Oasis's terrain file, out of `mc02.res`.
fn terrain_file() -> Option<Vec<u8>> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let res = kit.ctx("mxvatv-ps2/md37a").join("mc02.res");
    if !res.exists() {
        skip(&format!("{} is not there: Desert Oasis's mc02.res off MX vs. ATV's disc (Env/MC02)", res.display()));
        return None;
    }
    let file = std::fs::read(&res).unwrap();
    let archive = game_sim::archive::Res::parse(&file).unwrap();
    Some(archive.read(&file, archive.find("Env\\mc02\\MC02.ps2.trn").expect("mc02.res holds MC02.ps2.trn")).unwrap())
}

/// **The spawned quad's whole frame is the original's**: SLUS-21104's world step over the image's
/// world, at 1/30 s (one iteration of four substeps), with only the ground query hooked, against
/// [`quad::frame`] on the image's ride; every word [`poke_ride`] lays of the quad, its body, wheels,
/// scene nodes and collidables equals the image's after the step, and so does the whole rigid body.
/// Every word the step changed in those blocks is one the comparison reads, both sides ask the
/// ground the same number of times, and what the port hands on is what the frame may hand on. Then a
/// word of the port's result is changed, to see the comparison go red there.
#[test]
fn the_spawned_quads_whole_frame_is_the_originals() {
    whole_frame(false);
}

/// MD75e1 — **the same frame with the quad touching something**: two contacts laid into the
/// image's collision record (count `+0xf0`, list `+0xf8`), on both sides, the port holding the
/// record ([`quad::Bike::contacts`], read by [`record_of`]) so that [`quad::Steps`] resolves them at
/// each of the four gravity calls ([`atv_contact::Held::resolve`]) where the original's record's vt
/// `+0x88` does. Besides every word of the first test, the record's written fields and each entry's
/// arms and relative velocity equal the original's after the frame.
#[test]
fn the_spawned_quads_frame_touching_something_is_the_originals() {
    whole_frame(true);
}

/// Two contacts as a collide might leave them on the spawned quad: the left rear tyre's side on a
/// rock, and the nose on a wall ahead, about the body's centre `c`. The entry's arms and relative
/// velocity are left as garbage, which the resolution writes before it reads.
fn lay_contacts(ee: &mut Ee, record: u32, c: Vf) {
    let list = ee.u32_at(record + 0xf8);
    assert!(ee.u32_at(record + 0xf4) >= 2, "the record's list holds fewer than two");
    let at = |x: f32, y: f32, z: f32| Vf::new(c.x() + x, c.y() + y, c.z() + z, 1.0);
    let contacts = [
        (at(-0.55, -0.35, -0.6), Vf::new(0.6, 0.8, 0.0, 0.0), 0.04, 3),
        (at(0.1, 0.2, 1.1), Vf::new(0.0, 0.0, -1.0, 0.0), 0.02, 7),
    ];
    for (k, (point, normal, depth, surface)) in contacts.into_iter().enumerate() {
        let e = list + 0x90 * k as u32;
        for a in (e..e + 0x90).step_by(4) {
            ee.set_u32(a, 0x3f80_0000 + a % 0x100);
        }
        ee.set_u32(e, 0);
        let off = |s: f32| Vf::new(point.x() - normal.x() * s, point.y() - normal.y() * s, point.z() - normal.z() * s, 1.0);
        put_v(ee, e + 0x10, off(depth));
        put_v(ee, e + 0x20, off(-depth));
        put_v(ee, e + 0x30, normal);
        put_v(ee, e + 0x40, point);
        ee.set_u32(e + 0x80, surface);
        ee.set_u32(e + 0x84, 0);
        ee.set_u32(e + 0x88, 0);
    }
    ee.set_u32(record + 0xf0, 2);
}

/// The record and its list as [`atv_contact`] holds it (tests/atv_contact.rs's reading), the body's
/// mass beside it. Its contacts are with nothing that moves.
fn record_of(ee: &Ee, r: u32, body: u32) -> atv_contact::Held {
    use game_sim::contact::{Collider, Contact, Response};
    let fl = |a: u32| F(ee.f32_at(a));
    let response = |at: u32, angular: u32, spring: u32, friction_mask: u32, friction: u32| Response {
        stiffness: fl(r + at),
        damping: fl(r + at + 4),
        restitution: fl(r + at + 8),
        angular: ee.u32_at(r + angular) != 0,
        friction: fl(r + friction),
        spring_mask: get_v(ee, r + spring),
        friction_mask: get_v(ee, r + friction_mask),
    };
    let c = ee.u32_at(r + 0x208);
    assert_eq!(ee.u32_at(r + 0xe4), body, "the record's body");
    let collider = Collider {
        damp: ee.u32_at(r + 0x44) != 0,
        damp_linear: fl(r + 0x4c),
        damp_angular: fl(r + 0x54),
        mass: fl(r + 0x58),
        inv_mass: fl(r + 0x5c),
        inv_inertia: get_v(ee, r + 0x70),
        cofg: get_v(ee, r + 0x80),
        cofg_world: get_v(ee, r + 0x90),
        actors: ee.u32_at(r + 0x100),
        collides_with: ee.u32_at(r + 0x104),
        collidable: (c != 0).then(|| ee.u32_at(c + 0x94)),
        max_depth: fl(r + 0x10c),
        scenery: response(0x118, 0x13c, 0x160, 0x170, 0x1a0),
        actor: response(0x130, 0x140, 0x180, 0x190, 0x1a4),
        surfaces: core::array::from_fn(|k| fl(r + 0x1a8 + 4 * k as u32)),
        normal_speed: fl(r + 0x1e8),
        relative: get_v(ee, r + 0x1f0),
    };
    let list = ee.u32_at(r + 0xf8);
    let entries = (0..ee.u32_at(r + 0xf0))
        .map(|k| {
            let e = list + 0x90 * k;
            assert_eq!([ee.u32_at(e + 0x84), ee.u32_at(e + 0x88)], [0, 0], "entry {k} is with another record or a mover");
            let contact = Contact {
                kind: ee.u32_at(e),
                from: get_v(ee, e + 0x10),
                on_body: get_v(ee, e + 0x20),
                normal: get_v(ee, e + 0x30),
                point: get_v(ee, e + 0x40),
                arm: get_v(ee, e + 0x50),
                other_arm: get_v(ee, e + 0x60),
                surface: ee.u32_at(e + 0x80) as i32,
                other: None,
                mover: None,
            };
            atv_contact::Entry { contact, relative: get_v(ee, e + 0x70) }
        })
        .collect();
    atv_contact::Held {
        record: atv_contact::Record { collider, impulse_mask: get_v(ee, r + 0x150) },
        entries,
        movers: Vec::new(),
        mass: fl(body + 0x18),
        body: None,
    }
}

fn whole_frame(touching: bool) {
    let Some(mut pre) = image() else { return };
    let Some(mut ee) = image() else { return };
    let Some(trn) = terrain_file() else { return };
    let Some(d) = oasis::desert_world(&pre) else { return };
    let trn: &'static [u8] = Box::leak(trn.into_boxed_slice());
    let map: &'static game_sim::track::Terrain<'static> = Box::leak(Box::new(game_sim::track::Terrain::parse(trn).unwrap()));
    let terrain = atv_ground::Terrain::of(map);
    let dt = F(1.0 / 30.0);
    let (mut r, at) = image::read_ride(&pre, image::IMAGE_QUAD, dt).unwrap_or_else(|e| panic!("{e}"));
    // The quad's record has no contacts: its vt +0x88 (the gravity record) loops over them, and the
    // port hands that call on. MD75e1: or it is given two, and the port resolves them.
    assert_eq!(pre.u32_at(at.record + 0xf0), 0, "the image's collision record holds contacts");
    if touching {
        let centre = get_v(&pre, at.body + 0x70);
        lay_contacts(&mut pre, at.record, centre);
        lay_contacts(&mut ee, at.record, centre);
        r.bike.contacts = Some(record_of(&pre, at.record, at.body));
    }
    assert_eq!(pre.u32_at(IMAGE_LIST + 4), 0, "the image's nearby list is empty");

    // The original: the world's frame, the ground query answered by the port's terrain.
    let asked = Rc::new(Cell::new(0u32));
    let (a, t) = (asked.clone(), terrain.clone());
    ee.hooks.insert(GROUND_QUERY, Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(7)), (TERRAIN, 0), "the ground query's world and fourth argument");
        let (p, n, rec) = (ee.r(5) as u32, ee.r(6) as u32, ee.r(8) as u32);
        let mut record = ee.u8_at(rec);
        let (point, normal) = t.ground(get_v(ee, p), Vf::ZERO, &mut record);
        put_v(ee, p, point);
        put_v(ee, n, normal);
        ee.write(rec, 1, u128::from(record));
        a.set(a.get() + 1);
    }));
    ee.set(28, GP);
    ee.call(WORLD_STEP, &[u64::from(IMAGE_WORLD)], &[dt.0], 50_000_000);

    // The port, from the image's ride.
    let mut body = seed::body(&|a| pre.u32_at(a), at.body);
    let oasis::Desert { world, props, eco, trees, rays, search, first_part, .. } = d;
    let mut oasis = atv_search::Oasis::new(world, &props, &eco, &trees, rays, search, first_part);
    oasis.gathered = true;
    let mut port_asked = 0;
    let ground = |q: Vf, record: &mut u8| {
        port_asked += 1;
        terrain.ground(q, Vf::ZERO, record)
    };
    let table = AcosTable::build();
    let mut m = quad::Steps { bike: r.bike.clone(), rest: Vec::<Call>::new(), ground, sweep: &mut oasis };
    quad::frame(&GAME_WORLD, &r.s.p, &mut r.s.q, &mut body, dt, &table, &mut m);
    r.bike = m.bike;
    let rest = m.rest;
    if touching {
        // The record's and its entries' words the resolution writes, as the original left them.
        let h = r.bike.contacts.as_ref().unwrap();
        let c = &h.record.collider;
        let mut bad = Vec::new();
        let mut same = |what: String, got: Vf, at: u32| {
            if (0..4).any(|k| got[k].to_bits() != ee.u32_at(at + 4 * k as u32)) {
                bad.push(format!("  {what}: port {got:?}, original {:?}", get_v(&ee, at)));
            }
        };
        same("the record's centre in the world".into(), c.cofg_world, at.record + 0x90);
        same("the record's relative velocity".into(), c.relative, at.record + 0x1f0);
        let list = ee.u32_at(at.record + 0xf8);
        for (k, e) in h.entries.iter().enumerate() {
            let a = list + 0x90 * k as u32;
            same(format!("entry {k}'s arm"), e.contact.arm, a + 0x50);
            same(format!("entry {k}'s other arm"), e.contact.other_arm, a + 0x60);
            same(format!("entry {k}'s relative velocity"), e.relative, a + 0x70);
        }
        assert!(bad.is_empty(), "the record differs:\n{}", bad.join("\n"));
        assert_eq!(c.normal_speed.to_bits(), ee.u32_at(at.record + 0x1e8), "the record's normal speed");
        eprintln!("MD75e1: normal speed {:?}, relative {:?}; force {:?}", c.normal_speed, c.relative, body.last_force);
    }
    r.s.body = body.view();
    assert_eq!(port_asked, asked.get(), "the ground queries: port {port_asked}, original {}", asked.get());
    assert_eq!(asked.get(), 16, "four wheels over four substeps");
    // Handed on: the gravity record (unless the port holds it, MD75e1), and the two collidables'
    // database moves — the world's database is the image's own (MD75c1/MD89 move it), not the quad's.
    let named: Vec<String> = rest.iter().map(|c| format!("{c:?}").split([' ', '(']).next().unwrap().to_string()).collect();
    let told = if touching { 0 } else { 4 };
    assert_eq!(named, ["GravityRecord"; 4][..told].iter().copied().chain(["PartMove"; 2]).collect::<Vec<_>>(), "handed on: {rest:?}");

    let mut fresh = image().unwrap();
    let (bad, n) = image::compare(&ee, &mut fresh, &r, &at);
    let show = |bad: &[(u32, u32, u32, u32)]| {
        bad.iter().take(20).map(|&(t, ia, p, w)| format!("  test {t:#010x} = image {ia:#010x}: port {p:#010x}, original {w:#010x}")).collect::<Vec<_>>().join("\n")
    };
    assert!(bad.is_empty(), "{} of {n} words differ from the original's frame:\n{}", bad.len(), show(&bad));
    let body_bad: Vec<String> = seed::body_words(&body, at.body)
        .into_iter()
        .filter(|&(a, v)| ee.u32_at(a) != v)
        .map(|(a, v)| format!("  body +{:#x}: port {v:#010x}, original {:#010x}", a - at.body, ee.u32_at(a)))
        .collect();
    assert!(body_bad.is_empty(), "the rigid body differs:\n{}", body_bad.join("\n"));

    // Every word the step changed in the blocks the ride stands for is one the comparison read, but
    // for the trigger search's transient searched bit (0x10 of the flag byte `+0x126`), which it sets
    // on the quad's own collidables in its reach before their filter leaves them out: the port's
    // world does not hold the quad's own collidables as candidates, nor its collidables a flag byte.
    let blocks = image::blocks(&at);
    let mut read: std::collections::BTreeSet<u32> = fresh.written.iter().filter_map(|&t| image::image_at(&blocks, t & !3)).collect();
    read.extend(seed::body_words(&body, at.body).into_iter().map(|(a, _)| a));
    if touching {
        // MD75e1: the record's centre in the world, normal speed and relative velocity, compared above.
        read.extend([0x90, 0x94, 0x98, 0x9c, 0x1e8, 0x1f0, 0x1f4, 0x1f8, 0x1fc].map(|o| at.record + o));
    }
    let mut changed = 0;
    let mut searched = Vec::new();
    let mut unread = Vec::new();
    for &(_, ib, len) in &blocks {
        for a in (ib..ib + len).step_by(4) {
            let (was, is) = (pre.u32_at(a), ee.u32_at(a));
            if was == is {
                continue;
            }
            changed += 1;
            if read.contains(&a) {
                continue;
            }
            match at.parts.iter().position(|&p| p != 0 && a == p + 0x124) {
                Some(k) if was ^ is == 0x0010_0000 => searched.push(COLLIDABLES[k]),
                _ => unread.push(format!("  {a:#010x}: {was:#010x} -> {is:#010x}")),
            }
        }
    }
    assert!(unread.is_empty(), "{} words the step changed are not compared:\n{}", unread.len(), unread.join("\n"));
    assert_eq!(searched, [quad::Part::Riding, quad::Part::Sweep], "the quad's own collidables the trigger's search marked");
    // The scene past the nodes the ride holds: the step only marks their world frames stale (bit 0
    // of `+0x100` down), as placing the quad's node marks its subtree; the port's scene holds
    // [`atv_moto::NODES`] nodes and none of those.
    let mut scene = at.nodes.clone();
    let mut k = 0;
    while k < scene.len() && scene.len() < 4096 {
        for o in [0xf0, 0xf4, 0xf8] {
            let n = pre.u32_at(scene[k] + o);
            if n != 0 && !scene.contains(&n) {
                scene.push(n);
            }
        }
        k += 1;
    }
    let mut stale = 0;
    let mut past = Vec::new();
    for &n in &scene[at.nodes.len()..] {
        for a in (n..n + 0x148).step_by(4) {
            let (was, is) = (pre.u32_at(a), ee.u32_at(a));
            if a == n + 0x100 && was ^ is == 1 && was & 1 == 1 {
                stale += 1;
            } else if was != is {
                past.push(format!("  {a:#010x} (node {n:#010x}): {was:#010x} -> {is:#010x}"));
            }
        }
    }
    assert!(past.is_empty(), "the step moved nodes past the ride's:\n{}", past.join("\n"));
    eprintln!("MD75e: the scene holds {} nodes, the ride {}; {stale} past them marked stale", scene.len(), at.nodes.len());
    assert!(changed > 300, "only {changed} words changed: the step did not run the quad");
    eprintln!("MD75e: {} instructions, {} ground queries; {n} words equal, {changed} of them changed", ee.steps, asked.get());

    // A word changed in the port's result goes red at its place, and only there.
    let mut wrong = r.clone();
    wrong.s.q.lean = F(f32::from_bits(r.s.q.lean.to_bits() ^ 1));
    let (bad, _) = image::compare(&ee, &mut fresh, &wrong, &at);
    assert_eq!(bad.len(), 1, "one word changed, {} red:\n{}", bad.len(), show(&bad));
}
