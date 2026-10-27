//! S157: the bike against the world in the user's 2p recording, both riders.
//!
//! C10 indexed four world contacts in the 2p ride (research/c10/index.md, "World contacts"), every
//! one a body ray of the riding bike on the scenery, and nothing else: no crate, sign, beach ball or
//! wall is touched in it. P2's nose ray meets a tree's trunk at 130413 and its shoulder ray the same
//! trunk at 130417, which wrecks it on the scenery (reason 6); its shoulder ray meets another trunk at
//! 130896 (6 again); P1's nose ray is in the ground with the bike on its side at 131483 (ridden
//! out). C10 saved the original's RAM at the ticks around each (`context/states/c10/c10-<frame>.ram`).
//! From each image the interpreter runs the level's collision node one tick, as S88's
//! `rider_rider_capture.rs` does for the riders' collisions, and:
//!
//! - [`the_original_ticks_each_image_into_the_next`]: the tick lands on the next image, both riders'
//!   lists and wreck reasons and the hit rider's chassis bit for bit: the images are one ride;
//! - [`the_original_meets_the_world_where_c10_indexed_it`]: the contact appears on its tick, on the
//!   rider C10 named, against the scenery, and the wreck follows where the original wrecked;
//! - [`the_port_finds_the_world_contacts_the_original_does`]: the port's body rays through its own
//!   free09_ss — terrain, props and trunks — put the original's entries on the list, bit for bit;
//! - [`the_port_resolves_the_world_contacts_as_the_original`]: the impulse, the shock and twist, the
//!   force and torque of the port's resolve are the original's;
//! - [`the_tick_with_the_ports_resolve_wrecks_as_the_original`]: the tick with the port's resolve in
//!   the original's place is the original's tick, the wreck reasons with it;
//! - [`the_ports_wreck_test_wrecks_where_the_original_does`]: the port's hit clause on the port's
//!   contacts wrecks where the original does — a Race500P takes the 50cc reach (4), so a shoulder ray
//!   wrecks it.
//!
//! The images and the disc are game data, git-ignored: absent, the tests skip.

use game_render::disc::Disc;
use game_sim::air::{self, Air, Wreck};
use game_sim::bike_col::{self, BikeCol};
use game_sim::camera::CurveTable;
use game_sim::contact::{Collider, Contact, Node, Ridden, Vehicle};
use game_sim::eco::{Trees, Wooded};
use game_sim::math::Mat4;
use game_sim::phy::Phy;
use game_sim::ride::Tables;
use game_sim::rider_contact::{self, Bump};
use game_sim::track::{self, Scene, Terrain};
use game_sim::world::{Ground, Place, Props};
use game_sim::mover_contact;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

fn ctx(rel: &str) -> Option<PathBuf> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    if !path.exists() {
        skip(&format!("{} is not there", path.display()));
        return None;
    }
    Some(path)
}

/// C10's RAM image at `frame`, ready to call into.
fn image(frame: u32) -> Option<Ee> {
    let path = ctx(&format!("states/c10/c10-{frame}.ram"))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// The game manager's slot and the frame counter (kit.toml's `@game_manager`, `@frame_counter`).
const GAME_MANAGER: u32 = 0x004e_1e98;
const FRAME_COUNTER: u32 = 0x0047_9d04;
/// The level's collision node in the 2p state (oracle.txt, S157): its objects are two others, then
/// the two riders' records in either order.
const NODE: u32 = 0x0061_e630;
/// The node's update (node, dt) and a collision object's resolve (object, first substep).
const NODE_UPDATE: u32 = 0x0026_f800;
const RESOLVE: u32 = 0x0027_0dc0;
const BUDGET: u64 = 2_000_000_000;

/// A captured chassis quantity: its column and its offset on the chassis.
const CHASSIS: [(&str, u32); 7] =
    [("centre", 0x60), ("force", 0x130), ("moment", 0x140), ("shock", 0x170), ("twist", 0x180), ("vel", 0x260), ("rates", 0x290)];

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)))
}

/// P1's and P2's ridden vehicles: the actor list's fourth and third entries (watch/2p.toml), each a
/// ridden vehicle (type 2) whose player index says which.
fn players(ee: &Ee) -> [u32; 2] {
    let down = |hops: &[u32]| hops.iter().fold(ee.u32_at(GAME_MANAGER), |a, &h| ee.u32_at(a + h));
    let out = [down(&[0x34, 0x34, 0x8, 0x8, 0x8, 0x0]), down(&[0x34, 0x34, 0x8, 0x8, 0x0])];
    for (p, &v) in out.iter().enumerate() {
        assert_eq!(ee.u32_at(v), 2, "p{}'s list entry is not a ridden vehicle", p + 1);
        assert_eq!(ee.u32_at(ee.u32_at(v + 0x20) + 0x30), p as u32, "the list's p{} is another player", p + 1);
    }
    out
}

fn records(ee: &Ee) -> [u32; 2] {
    players(ee).map(|v| ee.u32_at(v + 0x7d0))
}

fn wrecks(ee: &Ee) -> [u32; 2] {
    players(ee).map(|v| ee.u32_at(v + 0x748))
}

/// A rider's chassis quantities as RAM holds them, in [`CHASSIS`]'s order, as bits.
type Quantities = [[u32; 3]; 7];

fn quantities(ee: &Ee, p: usize) -> Quantities {
    let ch = ee.u32_at(players(ee)[p] + 0x7a4);
    CHASSIS.map(|(_, at)| {
        let b = quad(ee, ch + at).to_bits();
        [b[0], b[1], b[2]]
    })
}

/// One entry of a record's list as the capture keeps it: kind, the ray's end on the body, the normal,
/// the point, and the other side's record (0 the scenery).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Entry {
    kind: u32,
    end: [u32; 3],
    normal: [u32; 3],
    point: [u32; 3],
    other: u32,
}

fn three(v: Vf) -> [u32; 3] {
    let b = v.to_bits();
    [b[0], b[1], b[2]]
}

fn list(ee: &Ee, rec: u32) -> Vec<Entry> {
    let (n, at) = (ee.u32_at(rec + 0x100), ee.u32_at(rec + 0x108));
    (0..n)
        .map(|k| {
            let e = at + 0x80 * k;
            Entry {
                kind: ee.u32_at(e),
                end: three(quad(ee, e + 0x20)),
                normal: three(quad(ee, e + 0x30)),
                point: three(quad(ee, e + 0x40)),
                other: ee.u32_at(e + 0x74),
            }
        })
        .collect()
}

/// The node's objects; every rider still on the bike has its record among them (a wrecked one's
/// is the wreck bike's object instead).
fn the_node(ee: &Ee) -> Vec<u32> {
    let n = ee.u32_at(NODE + 0x30);
    let objects: Vec<u32> = (0..n).map(|k| ee.u32_at(ee.u32_at(NODE + 0x38) + 4 * k)).collect();
    for (p, (r, w)) in records(ee).into_iter().zip(wrecks(ee)).enumerate() {
        assert!(w != 0 || objects.contains(&r), "p{}'s record {r:#x} is not on the node: {objects:x?}", p + 1);
    }
    objects
}

/// One tick of the level's collision node from `ee`, as the original runs it.
fn tick(ee: &mut Ee) {
    the_node(ee);
    let dt = ee.f32_at(addr("frame_dt").unwrap());
    ee.call(NODE_UPDATE, &[u64::from(NODE)], &[dt], BUDGET);
}

/// Everything [`the_original_ticks_each_image_into_the_next`] holds of a tick: both riders' chassis,
/// lists and wreck reasons.
fn seen(ee: &Ee) -> ([Quantities; 2], [Vec<Entry>; 2], [u32; 2]) {
    let recs = records(ee);
    ([quantities(ee, 0), quantities(ee, 1)], recs.map(|r| list(ee, r)), wrecks(ee))
}


fn put(ee: &mut Ee, a: u32, v: Vf) {
    for (k, x) in v.0.iter().enumerate() {
        ee.set_f32(a + 4 * k as u32, *x);
    }
}

fn dt(ee: &Ee) -> f32 {
    ee.f32_at(addr("frame_dt").unwrap())
}

/// A world contact C10 indexed: the image of the tick before it, the rider whose rays found it, how
/// high the ray reaches in the bike's model space, and both riders' wreck reasons after the tick.
struct Hit {
    image: u32,
    p: usize,
    reach: f32,
    wreck: [u32; 2],
}

/// The four, in frame order: P2's nose ray on a trunk (130413), its shoulder ray on the same trunk
/// four ticks on, which wrecks it on the scenery (6), its shoulder ray on another trunk (6), and P1's
/// nose ray in the ground with the bike on its side (131483).
const HITS: [Hit; 4] = [
    Hit { image: 130412, p: 1, reach: 1.858_302_1, wreck: [0, 0] },
    Hit { image: 130416, p: 1, reach: 4.580_028, wreck: [0, 6] },
    Hit { image: 130895, p: 1, reach: 4.580_028, wreck: [0, 6] },
    Hit { image: 131482, p: 0, reach: 1.858_302_1, wreck: [0, 0] },
];

fn node_at(ee: &Ee, n: u32) -> Node {
    Node { rows: [0, 1, 2].map(|k| quad(ee, n + 0x80 + 16 * k)), origin: quad(ee, n + 0xb0) }
}

fn mat_at(ee: &Ee, n: u32) -> Mat4 {
    let node = node_at(ee, n);
    Mat4([node.rows[0], node.rows[1], node.rows[2], node.origin])
}

/// A list entry in the port's terms; the other side and the mover only as there or not (99), which
/// is all a world contact has to say: neither is there.
fn contact_at(ee: &Ee, e: u32) -> Contact {
    Contact {
        kind: ee.u32_at(e),
        from: quad(ee, e + 0x10),
        on_body: quad(ee, e + 0x20),
        normal: quad(ee, e + 0x30),
        point: quad(ee, e + 0x40),
        arm: quad(ee, e + 0x50),
        other_arm: quad(ee, e + 0x60),
        surface: ee.u32_at(e + 0x70) as i32,
        other: (ee.u32_at(e + 0x74) != 0).then_some(99),
        mover: (ee.u32_at(e + 0x78) != 0).then_some(99),
    }
}

fn list_at(ee: &Ee, r: u32) -> Vec<Contact> {
    let (n, at) = (ee.u32_at(r + 0x100), ee.u32_at(r + 0x108));
    (0..n).map(|k| contact_at(ee, at + 0x80 * k)).collect()
}

fn vehicle_at(ee: &Ee, rec: u32) -> Vehicle {
    let v = ee.u32_at(rec + 0xf0);
    Vehicle {
        node: node_at(ee, ee.u32_at(rec + 0xf8)),
        cofg: quad(ee, v + 0x190),
        mass: F(ee.f32_at(v + 0x210)),
        inv_mass: F(ee.f32_at(v + 0x214)),
        inv_inertia: [0xf0, 0x104, 0x118].map(|at| F(ee.f32_at(v + at))),
        velocity: quad(ee, v + 0x260),
        rates: quad(ee, v + 0x290),
        shock: quad(ee, v + 0x170),
        twist: quad(ee, v + 0x180),
    }
}

fn ridden_at(ee: &Ee, rec: u32) -> Ridden {
    let v = ee.u32_at(rec + 0xf0);
    Ridden { chassis: vehicle_at(ee, rec), position: quad(ee, v + 0x60), force: quad(ee, v + 0x130), torque: quad(ee, v + 0x140) }
}

/// Every field of two contacts that differs, by name.
fn differ(port: &Contact, orig: &Contact) -> Vec<&'static str> {
    let quads = [
        ("from", port.from, orig.from),
        ("on_body", port.on_body, orig.on_body),
        ("normal", port.normal, orig.normal),
        ("point", port.point, orig.point),
        ("arm", port.arm, orig.arm),
        ("other_arm", port.other_arm, orig.other_arm),
    ];
    let mut bad: Vec<&'static str> = quads.iter().filter(|(_, p, o)| p.to_bits() != o.to_bits()).map(|(n, _, _)| *n).collect();
    if port.kind != orig.kind {
        bad.push("kind");
    }
    if port.surface != orig.surface {
        bad.push("surface");
    }
    if port.other != orig.other {
        bad.push("other");
    }
    if port.mover != orig.mover {
        bad.push("mover");
    }
    bad
}

/// Run the original's resolve from inside a hook on it, the hooked call's registers and stack put
/// back so the hook returns to its caller as the call would have (S88's).
fn run_original(ee: &mut Ee, r: u32, first: u32) {
    let (ra, sp, stack) = (ee.r(31), ee.r(29), ee.stack);
    ee.stack = (sp as u32 - 0x400) & !0xf;
    ee.call(RESOLVE, &[u64::from(r), u64::from(first)], &[], 50_000_000);
    ee.stack = stack;
    ee.set(29, sp);
    ee.set(31, ra);
    ee.pc = RESOLVE;
}

/// One tick of the node from `ee` with `on` handed every rider's resolve (machine, rider, record,
/// first substep) in place of the original's; other objects resolve as the original does.
fn tick_with(ee: &mut Ee, mut on: impl FnMut(&mut Ee, usize, u32, u32) + 'static) {
    let recs = records(ee);
    ee.hooks.insert(RESOLVE, Box::new(move |ee: &mut Ee| {
        let (r, first) = (ee.r(4) as u32, ee.r(5) as u32);
        match recs.iter().position(|&x| x == r) {
            Some(p) => on(ee, p, r, first),
            None => run_original(ee, r, first),
        }
    }));
    tick(ee);
}

/// Both riders' bikes, `Race500P.phy` (the 2p state's, oracle.txt S157), and the bike's collision
/// files out of the split track's archive.
fn bike(tables: &Tables) -> Option<(Phy, BikeCol)> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let res = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let read = |name: String| res.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
    let col = BikeCol::load(&read(bike_col::rays_file(bike_col::MODEL)), &read(bike_col::shape_file(bike_col::MODEL)))
        .expect("the bike's collision files");
    let bikes: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("bike\\") && n.ends_with(".res")
        })
        .collect();
    let phy = bikes
        .iter()
        .find_map(|b| disc.res(b).ok()?.read_suffix("race500p.phy").ok())
        .expect("race500p.phy in a bike archive");
    Some((Phy::read_bytes(&phy, &tables.curve), col))
}

/// The 2p ride's world, the split free09 (`Tracks\Free09_SS`, whose names the images hold): its
/// terrain, its scene's props out of both archives and its ecosystem's trunks, as the shell reads
/// them, handed to `then`.
fn in_free09_ss<R>(then: impl FnOnce(&mut Wooded<Place<'_>>) -> R) -> Option<R> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let parts = ["Tracks\\free09_ss.res", "Tracks\\free09_ss_2.res"].map(|n| disc.res(n).unwrap_or_else(|e| panic!("{n}: {e}")));
    let read = |want: &str| parts.iter().find_map(|r| r.read_suffix(want).ok()).ok_or_else(|| format!("no archive of this track has {want}"));
    let scene = Scene::parse(&read(".scn").unwrap()).expect("parse the scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let bytes = read(leaf.rsplit('\\').next().unwrap()).unwrap();
    let terrain = Terrain::parse(&bytes).expect("parse the terrain");
    let ground = Ground::new(&terrain, &scene.terrain);
    let (props, missed) = Props::load(&scene, read);
    // the shell skips it too: its one hierarchy file is not a mesh the port reads
    assert_eq!(missed, ["UTILITYSHACK1.col: this is a Hierarchy collision file, read as Mesh"], "free09_ss's props the port lacks");
    let (trees, missed) = Trees::on_ground(&read(".esb").unwrap(), &ground, &CurveTable::build()).expect("the ecosystem");
    assert!(missed.is_empty(), "free09_ss's trunks the port lacks: {missed:?}");
    Some(then(&mut Wooded { world: Place { ground, props }, trees }))
}

/// Pairs of consecutive images, and which riders the node's tick alone carries from one to the
/// next bit for bit. The node is not the whole frame: P1, riding on behind P2's wreck, has its held
/// controls moved by the frame's input in the ticks the node alone runs on stale ones.
const PAIRS: [(u32, [bool; 2]); 6] = [
    (130416, [true, true]),
    (130417, [false, true]),
    (130418, [false, true]),
    (130895, [true, true]),
    (130896, [false, true]),
    (131482, [true, true]),
];

#[test]
fn the_original_ticks_each_image_into_the_next() {
    let mut ran = 0;
    for (from, exact) in PAIRS {
        let (Some(mut ee), Some(next)) = (image(from), image(from + 1)) else { return };
        assert_eq!(ee.u32_at(FRAME_COUNTER), from, "c10-{from}'s frame counter");
        tick(&mut ee);
        let (got, want) = (seen(&ee), seen(&next));
        for p in 0..2 {
            let bad: Vec<&str> = (0..CHASSIS.len()).filter(|&k| got.0[p][k] != want.0[p][k]).map(|k| CHASSIS[k].0).collect();
            if exact[p] {
                assert!(bad.is_empty(), "{from} -> {}: p{}'s {bad:?}", from + 1, p + 1);
            } else {
                // stale controls move the drive, not the contacts: the push and the twist agree
                assert!(bad.iter().all(|b| ["centre", "vel", "rates", "force", "moment"].contains(b)), "{from}: p{}'s {bad:?}", p + 1);
            }
        }
        assert_eq!(got.1, want.1, "{from} -> {}: the contact lists", from + 1);
        assert_eq!(got.2, want.2, "{from} -> {}: the wreck reasons", from + 1);
        ran += 1;
    }
    assert_eq!(ran, PAIRS.len());
}

/// What a rider's record held when the node resolved it: (rider, first substep, its list).
type Resolved = Rc<RefCell<Vec<(usize, u32, Vec<Contact>)>>>;

#[test]
fn the_original_meets_the_world_where_c10_indexed_it() {
    for hit in &HITS {
        let Some(mut ee) = image(hit.image) else { return };
        // both bikes are Race500P, whose name holds "Race50": the constructor's flag is set (S157)
        for v in players(&ee) {
            assert_eq!(ee.u32_at(v + 0x588), 1, "{}: the 50cc flag", hit.image);
        }
        let log: Resolved = Rc::default();
        let seen = Rc::clone(&log);
        tick_with(&mut ee, move |ee, p, r, first| {
            seen.borrow_mut().push((p, first, list_at(ee, r)));
            run_original(ee, r, first);
        });
        let log = log.borrow();
        // a 60 Hz tick is one substep: each rider still on its bike resolves once
        let mine: Vec<_> = log.iter().filter(|(p, _, _)| *p == hit.p).collect();
        assert_eq!(mine.len(), 1, "{}: p{}'s resolves {log:?}", hit.image, hit.p + 1);
        let (_, first, list) = mine[0];
        assert_eq!(*first, 1, "{}", hit.image);
        assert_eq!(list.len(), 1, "{}: p{}'s list {list:?}", hit.image, hit.p + 1);
        let c = &list[0];
        assert_eq!((c.kind, c.other, c.mover), (0, None, None), "{}: its own ray, on the scenery", hit.image);
        assert_eq!(c.on_body.y().to_bits(), hit.reach.to_bits(), "{}: the ray's reach", hit.image);
        for (p, _, l) in log.iter().filter(|(p, _, _)| *p != hit.p) {
            assert!(l.is_empty(), "{}: p{} touched {l:?}", hit.image, p + 1);
        }
        assert_eq!(wrecks(&ee), hit.wreck, "{}: the wreck reasons after the tick", hit.image);
    }
}

/// The port's body rays from where the image left the bike, through the port's free09_ss: the ground
/// first when the bike is tipped past its up limit (P147), then the props (S138), as the rider
/// gathers them; against what the original's record held when the node resolved it.
#[test]
fn the_port_finds_the_world_contacts_the_original_does() {
    let tables = Tables::build();
    let Some((_, col)) = bike(&tables) else { return };
    in_free09_ss(|world| {
        for hit in &HITS {
            let Some(mut ee) = image(hit.image) else { return };
            let rec = records(&ee)[hit.p];
            let at = mat_at(&ee, ee.u32_at(rec + 0xf8));
            let dt = F(dt(&ee));
            let mut port = rider_contact::ground(&col, &at, world, dt);
            port.extend(rider_contact::scenery(&col, &at, world, dt).map(|(f, _)| f).unwrap_or_default());
            // P2's three are trunks: the scene's props alone have nothing there
            if hit.p == 1 {
                let bare = rider_contact::scenery(&col, &at, &mut world.world, dt).map(|(f, _)| f).unwrap_or_default();
                assert!(bare.is_empty(), "{}: the props without the trees {bare:?}", hit.image);
            }
            let orig: Rc<RefCell<Vec<Contact>>> = Rc::default();
            let seen = Rc::clone(&orig);
            let me = hit.p;
            tick_with(&mut ee, move |ee, p, r, first| {
                if p == me {
                    *seen.borrow_mut() = list_at(ee, r);
                }
                run_original(ee, r, first);
            });
            let orig = orig.borrow();
            assert_eq!(port.len(), orig.len(), "{}: p{}\nport {port:?}\noriginal {orig:?}", hit.image, hit.p + 1);
            for (k, (a, b)) in port.iter().zip(orig.iter()).enumerate() {
                assert_eq!(differ(a, b), Vec::<&str>::new(), "{}: contact {k}\nport {a:?}\noriginal {b:?}", hit.image);
            }
        }
    });
}

/// The port's resolve of the rider's world contact, alone on its list as the rider builds it
/// ([`rider_contact::alone`]): the original's list, the bike's chassis off RAM.
fn port_resolve(ee: &Ee, rec: u32, collider: Collider) -> (Bump, Ridden) {
    let mut bump = Bump { me: 0, contacts: list_at(ee, rec), colliders: vec![collider], vehicles: vec![vehicle_at(ee, rec)], movers: Vec::new() };
    let mut own = ridden_at(ee, rec);
    bump.resolve(&mut own, true);
    (bump, own)
}

#[test]
fn the_port_resolves_the_world_contacts_as_the_original() {
    let tables = Tables::build();
    let Some((phy, _)) = bike(&tables) else { return };
    let collider = rider_contact::collider(&phy.collisions, phy.mass.mass);
    for hit in &HITS {
        let Some(mut ee) = image(hit.image) else { return };
        let bad: Rc<RefCell<Option<Vec<String>>>> = Rc::default();
        let seen = Rc::clone(&bad);
        let me = hit.p;
        tick_with(&mut ee, move |ee, p, r, first| {
            let port = (p == me).then(|| port_resolve(ee, r, collider));
            run_original(ee, r, first);
            let Some((bump, own)) = port else { return };
            let got = ridden_at(ee, r);
            let mut bad = vec![];
            for (what, p, o) in [
                ("velocity", own.chassis.velocity, got.chassis.velocity),
                ("rates", own.chassis.rates, got.chassis.rates),
                ("shock", own.chassis.shock, got.chassis.shock),
                ("twist", own.chassis.twist, got.chassis.twist),
                ("force", own.force, got.force),
                ("torque", own.torque, got.torque),
            ] {
                if p.to_bits() != o.to_bits() {
                    bad.push(format!("{what}: port {p:?} original {o:?}"));
                }
            }
            for (k, (p, o)) in bump.contacts.iter().zip(&list_at(ee, r)).enumerate() {
                if p.arm.to_bits() != o.arm.to_bits() || p.other_arm.to_bits() != o.other_arm.to_bits() {
                    bad.push(format!("contact {k}'s arms: port {:?} {:?} original {:?} {:?}", p.arm, p.other_arm, o.arm, o.other_arm));
                }
            }
            *seen.borrow_mut() = Some(bad);
        });
        let bad = bad.borrow();
        let bad = bad.as_ref().unwrap_or_else(|| panic!("{}: p{} never resolved", hit.image, hit.p + 1));
        assert!(bad.is_empty(), "{}: p{} {bad:#?}", hit.image, hit.p + 1);
    }
}

/// The node's tick with the port's resolve standing in for the hit rider's: the rider lands where
/// the original's own tick does — on the next image where C10 saved one — and wrecks as it does.
#[test]
fn the_tick_with_the_ports_resolve_wrecks_as_the_original() {
    let tables = Tables::build();
    let Some((phy, _)) = bike(&tables) else { return };
    let collider = rider_contact::collider(&phy.collisions, phy.mass.mass);
    for hit in &HITS {
        let (Some(mut ee), Some(mut want)) = (image(hit.image), image(hit.image)) else { return };
        tick(&mut want);
        let stood_in = Rc::new(RefCell::new(0));
        let count = Rc::clone(&stood_in);
        let me = hit.p;
        tick_with(&mut ee, move |ee, p, r, first| {
            if p != me {
                run_original(ee, r, first);
                return;
            }
            // the port in the original's place: what it computes, written where the original would
            let (bump, own) = port_resolve(ee, r, collider);
            let ch = ee.u32_at(r + 0xf0);
            put(ee, ch + 0x260, own.chassis.velocity);
            put(ee, ch + 0x290, own.chassis.rates);
            put(ee, ch + 0x170, own.chassis.shock);
            put(ee, ch + 0x180, own.chassis.twist);
            put(ee, ch + 0x130, own.force);
            put(ee, ch + 0x140, own.torque);
            let c = &bump.colliders[0];
            put(ee, r + 0xa0, c.cofg_world);
            ee.set_f32(r + 0x1d8, c.normal_speed.0);
            put(ee, r + 0x1e0, c.relative);
            let at = ee.u32_at(r + 0x108);
            for (k, c) in bump.contacts.iter().enumerate() {
                put(ee, at + 0x80 * k as u32 + 0x50, c.arm);
                put(ee, at + 0x80 * k as u32 + 0x60, c.other_arm);
            }
            *count.borrow_mut() += 1;
        });
        assert_eq!(*stood_in.borrow(), 1, "{}", hit.image);
        assert_eq!(seen(&ee), seen(&want), "{}: with the port's resolve", hit.image);
        assert_eq!(wrecks(&ee), hit.wreck, "{}: with the port's resolve", hit.image);
        if let Some(next) = image(hit.image + 1) {
            assert_eq!(quantities(&ee, hit.p), quantities(&next, hit.p), "{}: p{} against the next image", hit.image, hit.p + 1);
        }
    }
}

/// The port's wreck test on the port's own contacts: the hit clause wrecks a Race500P on the
/// scenery from a shoulder ray (4.58) as the original does, and not from the nose ray (1.86). With
/// the 125's reach it would not wreck at all: only the head ray reaches above 6.
#[test]
fn the_ports_wreck_test_wrecks_where_the_original_does() {
    let tables = Tables::build();
    let Some((phy, col)) = bike(&tables) else { return };
    let fifty = track::fifty_reach(&track::physics_file(track::Engine::Cc500, false, track::Tuning::P));
    assert!(fifty, "Race500P's name holds Race50");
    in_free09_ss(|world| {
        for hit in &HITS {
            let Some(ee) = image(hit.image) else { return };
            let rec = records(&ee)[hit.p];
            let at = mat_at(&ee, ee.u32_at(rec + 0xf8));
            let dt = F(dt(&ee));
            let ground = rider_contact::ground(&col, &at, world, dt);
            let mut hits = rider_contact::ground_hits(&ground);
            let found = rider_contact::scenery(&col, &at, world, dt).map(|(f, _)| f).unwrap_or_default();
            hits.extend(mover_contact::wreck_contacts(&found));
            assert!(!hits.is_empty(), "{}: the port's rays found nothing", hit.image);
            let verdict = |fifty| air::hit_wreck(&phy.air, &mut Air::default(), true, fifty, F::ONE, F::ZERO, &hits);
            let want = (hit.wreck[hit.p] == Wreck::Scenery as u32).then_some(Wreck::Scenery);
            assert_eq!(verdict(fifty), want, "{}: p{}'s hit clause on {hits:?}", hit.image, hit.p + 1);
            assert_eq!(verdict(false), None, "{}: no ray but the head's reaches the 125's limit", hit.image);
        }
    });
}
