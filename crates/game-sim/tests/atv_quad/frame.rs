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
    let Some(pre) = image() else { return };
    let Some(mut ee) = image() else { return };
    let Some(trn) = terrain_file() else { return };
    let Some(d) = oasis::desert_world(&pre) else { return };
    let trn: &'static [u8] = Box::leak(trn.into_boxed_slice());
    let map: &'static game_sim::track::Terrain<'static> = Box::leak(Box::new(game_sim::track::Terrain::parse(trn).unwrap()));
    let terrain = atv_ground::Terrain::of(map);
    let dt = F(1.0 / 30.0);
    let (mut r, at) = image::read_ride(&pre, image::IMAGE_QUAD, dt).unwrap_or_else(|e| panic!("{e}"));
    // The quad's record has no contacts: its vt +0x88 (the gravity record) loops over them, and the
    // port hands that call on.
    assert_eq!(pre.u32_at(at.record + 0xf0), 0, "the image's collision record holds contacts");
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
    r.s.body = body.view();
    assert_eq!(port_asked, asked.get(), "the ground queries: port {port_asked}, original {}", asked.get());
    assert_eq!(asked.get(), 16, "four wheels over four substeps");
    // Handed on: the gravity record (no contacts), and the two collidables' database moves — the
    // world's database is the image's own (MD75c1/MD89 move it), not the quad's.
    let named: Vec<String> = rest.iter().map(|c| format!("{c:?}").split([' ', '(']).next().unwrap().to_string()).collect();
    assert_eq!(named, ["GravityRecord"; 4].into_iter().chain(["PartMove"; 2]).collect::<Vec<_>>(), "handed on: {rest:?}");

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
