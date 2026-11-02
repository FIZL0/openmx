//! S241 — a free06 EyeBall struck by the biplane in PCSX2, and the port's strike set against it.
//!
//! research/s241/aim.py puts biplane.p2m2's plane on free06's ground 25 ft short of ball 0 and rolls it
//! at the ball at 20 ft/s with no pad; research/s241/strike.py records watch/s241-strike.toml and saves
//! states around the strike (context/capture/s241/hit0.csv, context/s241/hit0-*.p2s, their RAM in
//! context/states/s241-hit0-*.eeMemory.bin). The ball wakes on 11046: the plane's collision object, filed
//! first in free06's world node, has its own rays' hit on the ball's `f.col` (kind 0, naming the ball's
//! record) and the ball's list the mirror (kind 1, naming the plane's) at its head, ahead of its own
//! ground entry; the plane's world steps first, its resolve pushing the ball, then the ball's.
//!
//! - [`the_planes_rays_strike_the_ball_as_the_originals`]: from 11046, 11047 and 11048, the original's
//!   search (every filed object's list emptied, then every object's search) against the port's
//!   ([`Flight::search_props`] over the state's props): the plane's entries on the ball and the ball's
//!   mirrors, every field the search writes;
//! - [`the_plane_and_the_ball_push_each_other_as_the_original`]: 11046's node ticked by the original,
//!   the plane's first-substep resolve and the ball's hooked: the port's [`contact::resolve`] on the
//!   list, the plane's body and the ball's as the call finds them (the other as a body,
//!   [`Side::Body`]), against what the original's leaves, both bodies and the arms.
//!
//! - [`the_balls_rays_strike_the_planes_hull_as_the_originals`]: 11061, the balls woken and filed, the
//!   original's search against the port's hull pass ([`prop_wreck::struck_on`] on the plane's hull where
//!   its node stands): each ball's entries on the hull and the mirrors on the plane's list.
//!
//! Skipped with a line where the disc, the kit, MX.PSR or the states are not there (s2's context only).

use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::rc::Rc;

use game_render::disc::Disc;
use game_sim::archive::{Psr, Res};
use game_sim::biplane::Body;
use game_sim::contact::{self, Collider, Contact, Other, Side, Weighed};
use game_sim::flight::{self, Flight};
use game_sim::flight_search::{self, Rays};
use game_sim::phy::{self, Ini, Plane};
use game_sim::prop_set::PropSet;
use game_sim::prop_wreck;
use game_sim::track::{self, Scene, Terrain};
use game_sim::world::{Flat, Ground, World};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

/// The biplane (MXBiplane) in every state made from biplane.p2m2's: `+0x160` its body, `+0x170` its
/// collision record.
const PLANE: u32 = 0x00c4_4a40;
/// free06's world node the plane's collision object and the woken props' are filed on.
const NODE: u32 = 0x0060_b200;
/// The node's update (node, dt) and a collision object's resolve (object, first substep).
const NODE_UPDATE: u32 = 0x0026_f800;
const RESOLVE: u32 = 0x0027_0dc0;
/// The six EyeBalls' physics objects (`+0x110` body, `+0x120` collision record, `+0x13c` node).
const BALLS: [u32; 6] = [0x00d3_f510, 0x00d4_77a0, 0x00d4_a290, 0x00d4_cd80, 0x00d4_f870, 0x00d5_2360];

mod body_at {
    pub const ENABLED: u32 = 0x00;
    pub const INV_MASS: u32 = 0x10;
    pub const DAMPING: u32 = 0x20;
    pub const LAST_POSITION: u32 = 0x50;
    pub const POSITION: u32 = 0x60;
    pub const VELOCITY: u32 = 0x70;
    pub const LAST_FORCE: u32 = 0x80;
    pub const FORCE: u32 = 0x90;
    pub const ACCEL: u32 = 0xa0;
    pub const SHOCK_PEAK: u32 = 0xb0;
    pub const SHOCK: u32 = 0xc0;
    pub const STEP: u32 = 0xf0;
    pub const INV_INERTIA: u32 = 0x120;
    pub const ANG_DAMPING: u32 = 0x130;
    pub const LAST_QUAT: u32 = 0x150;
    pub const QUAT: u32 = 0x170;
    pub const COFG: u32 = 0x180;
    pub const MOMENT: u32 = 0x190;
    pub const LAST_MOMENT: u32 = 0x1a0;
    pub const RATES: u32 = 0x1b0;
    pub const ANG_ACCEL: u32 = 0x1c0;
    pub const TWIST_PEAK: u32 = 0x1d0;
    pub const TWIST: u32 = 0x1e0;
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|k| ee.u32_at(a + k)))
}

fn body_of(ee: &Ee, b: u32) -> Body {
    use body_at::*;
    let q = |a: u32| quad(ee, b + a);
    Body {
        enabled: ee.u32_at(b + ENABLED) != 0,
        inv_mass: F(ee.f32_at(b + INV_MASS)),
        damping: q(DAMPING),
        last_position: q(LAST_POSITION),
        position: q(POSITION),
        velocity: q(VELOCITY),
        last_force: q(LAST_FORCE),
        force: q(FORCE),
        accel: q(ACCEL),
        shock_peak: q(SHOCK_PEAK),
        shock: q(SHOCK),
        step: q(STEP),
        inv_inertia: q(INV_INERTIA),
        ang_damping: q(ANG_DAMPING),
        last_quat: q(LAST_QUAT),
        quat: q(QUAT),
        cofg: q(COFG),
        moment: q(MOMENT),
        last_moment: q(LAST_MOMENT),
        rates: q(RATES),
        ang_accel: q(ANG_ACCEL),
        twist_peak: q(TWIST_PEAK),
        twist: q(TWIST),
    }
}

/// A list entry in the port's terms, its other side as the record it names (`Some(record)`).
fn entry_at(ee: &Ee, e: u32) -> Contact {
    let other = ee.u32_at(e + 0x74);
    Contact {
        kind: ee.u32_at(e),
        from: quad(ee, e + 0x10),
        on_body: quad(ee, e + 0x20),
        normal: quad(ee, e + 0x30),
        point: quad(ee, e + 0x40),
        arm: quad(ee, e + 0x50),
        other_arm: quad(ee, e + 0x60),
        surface: ee.u32_at(e + 0x70) as i32,
        other: (other != 0).then_some(other as usize),
        mover: (ee.u32_at(e + 0x78) != 0).then_some(99),
    }
}

fn list_at(ee: &Ee, r: u32) -> Vec<Contact> {
    let (n, at) = (ee.u32_at(r + 0x100), ee.u32_at(r + 0x108));
    (0..n).map(|k| entry_at(ee, at + 0x80 * k)).collect()
}

/// The contact fields a search writes (the arms are the resolve's), by name, that differ.
fn searched_differ(port: &Contact, orig: &Contact) -> Vec<&'static str> {
    let mut out = Vec::new();
    for (what, p, o) in [("from", port.from, orig.from), ("on_body", port.on_body, orig.on_body), ("normal", port.normal, orig.normal), ("point", port.point, orig.point)] {
        if p.to_bits() != o.to_bits() {
            out.push(what);
        }
    }
    if port.kind != orig.kind {
        out.push("kind");
    }
    if port.surface != orig.surface {
        out.push("surface");
    }
    out
}

/// A state's RAM, its gp set; `None` with a skip line where it is not there.
fn image(kit: &Kit, name: &str) -> Option<Ee> {
    let rel = format!("states/s241-{name}.eeMemory.bin");
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context").join(&rel);
    let path = if here.exists() { here } else { kit.ctx(&rel) };
    let mut ee = Ee::from_ram(&path).map_err(|why| skip(&format!("{why} — research/s241/slots.py ram {name} writes it"))).ok()?;
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// The plane as the port builds it off `machine.res`, with its hull (tests/plane_contact_capture.rs's).
fn the_plane(kit: &Kit) -> Option<(Flight, track::Mesh)> {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/iso/MX.PSR");
    let path = if here.exists() { here } else { kit.ctx("iso").join("MX.PSR") };
    let Ok(mut f) = File::open(&path) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", path.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("machine.res").expect("machine.res is in MX.PSR");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("machine.res");
    let member = |name: &str| {
        let e = res.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).unwrap_or_else(|| panic!("machine.res has {name}"));
        res.read(&bytes, e).expect("its bytes")
    };
    let plane = Plane::read_bytes(&member(phy::BIPLANE_PHY));
    let rays = track::Rays::parse(&member(flight_search::BIPLANE_RAYS)).expect("biplanem.col");
    let hull = track::Mesh::parse(&member(flight::HULL)).expect("biplanef.col");
    Some((Flight::new(&plane).with_rays(Rays::of(&rays)), hull))
}

/// free06's knockable props as the ride loads them, and its terrain, off the disc.
fn free06() -> Option<(PropSet, Ground<'static>)> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let member = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("Tracks\\free06.res")).expect("free06.res").name.clone();
    let res = disc.res(&member).expect("free06's archive");
    let res = &res;
    let toy = Ini::parse_bytes(&res.read_suffix("free06.toy").expect("free06.toy"));
    let scene = Scene::parse(&res.read_suffix("free06.scn").expect("free06.scn")).expect("free06.scn parses");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    // The test's lifetime: the ground borrows them.
    let bytes: &'static [u8] = Box::leak(res.read_suffix(&leaf).expect("free06's terrain").into_boxed_slice());
    let terrain: &'static Terrain = Box::leak(Box::new(Terrain::parse(bytes).expect("free06's terrain parses")));
    let ground = Ground::new(terrain, &scene.terrain);
    let area = ground.area();
    let (set, said) =
        PropSet::load(&toy, "Tracks\\Free06\\", &scene.models, &mut |p| res.read(p).ok(), &mut |p| res.read_suffix(p).ok(), area);
    assert!(said.is_empty(), "free06 loads whole: {said:?}");
    Some((set, ground))
}

/// The state's six balls in the port's set: each RAM ball's (collection, object), the one whose node
/// stands nearest the ball's node in RAM, with its body and node the state's.
fn seed(ee: &Ee, set: &mut PropSet) -> Vec<(usize, usize)> {
    BALLS
        .iter()
        .map(|&o| {
            let node = ee.u32_at(o + 0x13c);
            let origin = quad(ee, node + 0xb0);
            let mut best = None;
            for (c, coll) in set.collections.iter().enumerate() {
                for (k, ob) in coll.objects.iter().enumerate() {
                    let d = (ob.node.origin - origin).0[..3].iter().map(|v| v * v).sum::<f32>();
                    if ob.collision.is_some() && best.is_none_or(|(_, _, b)| d < b) {
                        best = Some((c, k, d));
                    }
                }
            }
            let (c, k, d) = best.expect("a prop with a collision object");
            assert!(d < 2500.0, "the ball at {:?} is {d:?} from the nearest prop", origin.0);
            let coll = &mut set.collections[c];
            coll.bones[k].body = body_of(ee, ee.u32_at(o + 0x110));
            let ob = &mut coll.objects[k];
            ob.node.rows = [0, 1, 2].map(|r| quad(ee, node + 0x80 + 16 * r));
            ob.node.origin = origin;
            (c, k)
        })
        .collect()
}

fn the_record(ee: &Ee) -> u32 {
    ee.u32_at(PLANE + 0x170)
}

/// **The plane's rays strike the ball as the original's** (S241): from each state, the original's
/// node has every filed object's list emptied and then searched; the port's plane, its body the
/// state's, searches free06's props, the balls where the state has them. The plane's entries on a
/// ball and the mirrors on that ball's list are the same, every field the search writes, in order;
/// the port's entry names the ball the original's names. By 11048 the ball is knocked clear: neither
/// strikes it.
#[test]
fn the_planes_rays_strike_the_ball_as_the_originals() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let Some((base, _)) = the_plane(&kit) else { return };
    let Some((set0, _)) = free06() else { return };
    let mut struck = 0;
    for name in ["hit0-11046", "hit0-11047", "hit0-11048"] {
        let Some(mut ee) = image(&kit, name) else { return };
        let dt = ee.f32_at(addr("frame_dt").unwrap());
        let mut set = set0.clone();
        let balls = seed(&ee, &mut set);
        let records: Vec<u32> = BALLS.iter().map(|&o| ee.u32_at(o + 0x120)).collect();
        let plane_rec = the_record(&ee);

        let mut plane = base.clone();
        plane.body = body_of(&ee, ee.u32_at(PLANE + 0x160));
        let mut ground = Flat { y: F(-1.0e6), ..Default::default() };
        let knocked = plane.search_props(F(dt), &mut ground, &set);

        let filed: Vec<u32> = (0..ee.u32_at(NODE + 0x30)).map(|k| ee.u32_at(ee.u32_at(NODE + 0x38) + 4 * k)).collect();
        assert_eq!(filed.first(), Some(&plane_rec), "{name}: the plane's object is filed first {filed:x?}");
        for &o in &filed {
            ee.call(ee.u32_at(ee.u32_at(o + 8) + 0x74), &[u64::from(o)], &[], 50_000_000);
        }
        for &o in &filed {
            ee.call(ee.u32_at(ee.u32_at(o + 8) + 0x7c), &[u64::from(o)], &[dt], 50_000_000);
        }

        let orig: Vec<Contact> = list_at(&ee, plane_rec).into_iter().filter(|c| c.kind == 0 && records.contains(&(c.other.unwrap_or(0) as u32))).collect();
        struck += orig.len();
        let port: Vec<&Contact> = plane.record.contacts.iter().filter(|c| c.other.is_some_and(|o| o >= flight::KNOCK_FROM)).collect();
        eprintln!("{name}: the plane's entries on the balls, port {} original {}", port.len(), orig.len());
        assert_eq!(port.len(), orig.len(), "{name}: the plane's entries on the balls\nport {port:?}\noriginal {orig:?}");
        for (k, (p, o)) in port.iter().zip(&orig).enumerate() {
            assert_eq!(searched_differ(p, o), Vec::<&str>::new(), "{name}: plane entry {k}\nport {p:?}\noriginal {o:?}");
            let (c, ob, _) = &knocked[p.other.unwrap() - flight::KNOCK_FROM];
            let ball = balls.iter().position(|b| *b == (*c, *ob)).expect("the port's entry names a ball");
            assert_eq!(records[ball] as usize, o.other.unwrap(), "{name}: plane entry {k} names ball {ball}");
        }
        for (ball, &r) in records.iter().enumerate() {
            let orig: Vec<Contact> = list_at(&ee, r).into_iter().filter(|c| c.kind == 1 && c.other == Some(plane_rec as usize)).collect();
            let port = knocked.iter().find(|(c, o, _)| (*c, *o) == balls[ball]).map_or(&[][..], |(_, _, m)| &m[..]);
            assert_eq!(port.len(), orig.len(), "{name}: ball {ball}'s mirrors\nport {port:?}\noriginal {orig:?}");
            for (k, (p, o)) in port.iter().zip(&orig).enumerate() {
                assert_eq!(searched_differ(p, o), Vec::<&str>::new(), "{name}: ball {ball}'s mirror {k}\nport {p:?}\noriginal {o:?}");
            }
        }
    }
    assert!(struck > 0, "the original's plane strikes no ball from any state");
}

/// Run the original's resolve from inside a hook on it (tests/prop_knock.rs's).
fn run_original(ee: &mut Ee, r: u32, first: u32) {
    let (ra, sp, stack) = (ee.r(31), ee.r(29), ee.stack);
    ee.stack = (sp as u32 - 0x400) & !0xf;
    ee.call(RESOLVE, &[u64::from(r), u64::from(first)], &[], 50_000_000);
    ee.stack = stack;
    ee.set(29, sp);
    ee.set(31, ra);
    ee.pc = RESOLVE;
}

/// What a resolve writes on a body, by name.
fn written(b: &Body) -> [(&'static str, Vf); 6] {
    [("velocity", b.velocity), ("rates", b.rates), ("shock", b.shock), ("twist", b.twist), ("force", b.force), ("moment", b.moment)]
}

/// **The push both ways** (S241): 11046's node ticked by the original, the plane's first-substep
/// resolve (its world steps first) and then the ball's hooked — the port's [`contact::resolve`] on the
/// list as the call finds it, its own body and the other's (the plane's force listener has the ball's
/// body among its others, the ball's step the plane's as its striker), against what the original's
/// resolve leaves: both bodies and the entries' arms.
#[test]
fn the_plane_and_the_ball_push_each_other_as_the_original() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let Some((base, _)) = the_plane(&kit) else { return };
    let Some((mut set, _)) = free06() else { return };
    let Some(mut ee) = image(&kit, "hit0-11046") else { return };
    let balls = seed(&ee, &mut set);
    let plane_rec = the_record(&ee);
    let plane_body = ee.u32_at(PLANE + 0x160);
    let (bc, bo) = balls[0];
    let ball_rec = ee.u32_at(BALLS[0] + 0x120);
    let ball_body = ee.u32_at(BALLS[0] + 0x110);
    let ball_collider: Collider = set.collections[bc].objects[bo].collision.as_ref().unwrap().collider;
    let ball_mass = set.collections[bc].bones[bo].mass;
    let (plane_collider, plane_mass) = (base.collider, base.mass);

    let bad: Rc<RefCell<Vec<String>>> = Rc::default();
    let done: Rc<RefCell<[bool; 2]>> = Rc::default();
    let (seen, ran) = (Rc::clone(&bad), Rc::clone(&done));
    ee.hooks.insert(
        RESOLVE,
        Box::new(move |ee: &mut Ee| {
            let (r, first) = (ee.r(4) as u32, ee.r(5) as u32);
            let which = [plane_rec, ball_rec].iter().position(|&x| x == r);
            let Some(which) = which.filter(|&w| first != 0 && !ran.borrow()[w]) else {
                run_original(ee, r, first);
                return;
            };
            ran.borrow_mut()[which] = true;
            let (me, other) = if which == 0 { (plane_body, ball_body) } else { (ball_body, plane_body) };
            let (mut own_c, own_m, mut other_c, other_m) =
                if which == 0 { (plane_collider, plane_mass, ball_collider, ball_mass) } else { (ball_collider, ball_mass, plane_collider, plane_mass) };
            let named = if which == 0 { ball_rec } else { plane_rec };
            let mut list: Vec<Contact> = list_at(ee, r).into_iter().map(|c| Contact { other: c.other.map(|o| if o == named as usize { 0 } else { 99 }), ..c }).collect();
            let mut own = body_of(ee, me);
            let mut theirs = body_of(ee, other);
            {
                let mut weighed = Weighed { body: &mut theirs, mass: other_m };
                let mut others = [Other { collider: &mut other_c, side: Side::Body(&mut weighed) }];
                contact::resolve(&mut own_c, &mut Weighed { body: &mut own, mass: own_m }, &mut list, &mut others, &[], true);
            }
            run_original(ee, r, first);
            let what = ["plane", "ball"][which];
            let mut bad = seen.borrow_mut();
            bad.push(format!("{what}'s resolve: {} entries", list.len()));
            for (whose, port, orig) in [("own", &own, body_of(ee, me)), ("other", &theirs, body_of(ee, other))] {
                for ((field, p), (_, o)) in written(port).into_iter().zip(written(&orig)) {
                    if whose == "other" && (field == "force" || field == "moment") {
                        continue;
                    }
                    if p.to_bits() != o.to_bits() {
                        bad.push(format!("{what}'s resolve, {whose} body {field}: port {:?} original {:?}", p.0, o.0));
                    }
                }
            }
            for (k, (p, o)) in list.iter().zip(&list_at(ee, r)).enumerate() {
                if p.arm.to_bits() != o.arm.to_bits() || p.other_arm.to_bits() != o.other_arm.to_bits() {
                    bad.push(format!("{what}'s entry {k}'s arms: port {:?} {:?} original {:?} {:?}", p.arm, p.other_arm, o.arm, o.other_arm));
                }
            }
        }),
    );
    let dt = ee.f32_at(addr("frame_dt").unwrap());
    ee.call(NODE_UPDATE, &[u64::from(NODE)], &[dt], 2_000_000_000);
    assert_eq!(*done.borrow(), [true, true], "both first-substep resolves ran in the node's tick");
    let bad = bad.borrow();
    eprintln!("{bad:#?}");
    assert_eq!(bad.len(), 2, "{bad:#?}");
}

/// **The balls' rays strike the plane's hull as the original's** (S241, as a wreck bike's frame,
/// S197b): in 11061 the knocked balls are filed on the node after the plane, and two of them (balls 0
/// and 4) have their rays on its hull: their entries (kind 0) naming the plane's record after their
/// own, the plane's list the mirrors. The original's node re-searched from the state against the
/// port's hull pass, the filed balls where the state has them and the hull at the plane's node: each
/// ball's entries on the hull and the plane's mirrors, every field the search writes, in order.
#[test]
fn the_balls_rays_strike_the_planes_hull_as_the_originals() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let Some((_, hull)) = the_plane(&kit) else { return };
    let Some((mut set, _)) = free06() else { return };
    let Some(mut ee) = image(&kit, "hit0-11061") else { return };
    let dt = ee.f32_at(addr("frame_dt").unwrap());
    let balls = seed(&ee, &mut set);
    let records: Vec<u32> = BALLS.iter().map(|&o| ee.u32_at(o + 0x120)).collect();
    let plane_rec = the_record(&ee);
    let filed: Vec<u32> = (0..ee.u32_at(NODE + 0x30)).map(|k| ee.u32_at(ee.u32_at(NODE + 0x38) + 4 * k)).collect();
    for (b, &(c, o)) in balls.iter().enumerate() {
        set.collections[c].objects[o].filed = filed.contains(&records[b]);
    }
    let node = ee.u32_at(plane_rec + 0xf8);
    let at = [0, 1, 2].map(|r| quad(&ee, node + 0x80 + 16 * r));
    let at = [at[0], at[1], at[2], quad(&ee, node + 0xb0)];
    let strike = prop_wreck::struck_on(&mut set, &hull, at, flight::COLLIDABLE, flight::COLLIDES_WITH, 0);

    for &o in &filed {
        ee.call(ee.u32_at(ee.u32_at(o + 8) + 0x74), &[u64::from(o)], &[], 50_000_000);
    }
    for &o in &filed {
        ee.call(ee.u32_at(ee.u32_at(o + 8) + 0x7c), &[u64::from(o)], &[dt], 50_000_000);
    }
    let mut struck = 0;
    for (b, &(c, o)) in balls.iter().enumerate() {
        let orig: Vec<Contact> = list_at(&ee, records[b]).into_iter().filter(|e| e.kind == 0 && e.other == Some(plane_rec as usize)).collect();
        let port = &set.collections[c].objects[o].collision.as_ref().unwrap().record.contacts;
        eprintln!("ball {b}: entries on the hull, port {} original {}", port.len(), orig.len());
        assert_eq!(port.len(), orig.len(), "ball {b}'s entries on the hull\nport {port:?}\noriginal {orig:?}");
        for (k, (p, e)) in port.iter().zip(&orig).enumerate() {
            assert_eq!(searched_differ(p, e), Vec::<&str>::new(), "ball {b}'s entry {k}\nport {p:?}\noriginal {e:?}");
        }
        struck += orig.len();
    }
    assert!(struck > 0, "no ball's rays strike the plane in 11061");
    let orig: Vec<Contact> = list_at(&ee, plane_rec).into_iter().filter(|e| e.kind == 1 && records.contains(&(e.other.unwrap_or(0) as u32))).collect();
    assert_eq!(strike.mirrors.len(), orig.len(), "the plane's mirrors\nport {:?}\noriginal {orig:?}", strike.mirrors);
    for (k, (p, e)) in strike.mirrors.iter().zip(&orig).enumerate() {
        assert_eq!(searched_differ(p, e), Vec::<&str>::new(), "the plane's mirror {k}\nport {p:?}\noriginal {e:?}");
        let pushed = &strike.struck[p.other.expect("a mirror names its prop")];
        let ball = balls.iter().position(|&b| b == (pushed.collection, pushed.object)).expect("a ball");
        assert_eq!(records[ball] as usize, e.other.unwrap(), "the plane's mirror {k} names ball {ball}");
    }
}

/// **The ride's plane knocks the ball** (S241, [`game_sim::Sim::step`]'s plane lane): hit0-11042's
/// plane (its body the state's, rolling at ball 0 at 19 ft/s, the throttle shut) and free06's props,
/// the balls where the state has them, over free06's terrain. The state's body is the capture's
/// 11043 row (the save lands a tick after it was asked for), and the original's ball wakes on 11046:
/// three ticks on, the port's plane's rays strike ball 0 too, and it wakes and is thrown off along
/// the plane's way. The rest of the plane's state (its tires, surfaces) is the port's fresh one, so
/// this is the lane's wiring and the tick it strikes on, not the capture's rows.
#[test]
fn the_rides_plane_knocks_the_ball() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let Some((plane, hull)) = the_plane(&kit) else { return };
    let Some((mut set, mut ground)) = free06() else { return };
    let Some(ee) = image(&kit, "hit0-11042") else { return };
    let balls = seed(&ee, &mut set);
    let (c, o) = balls[0];
    assert!(!set.collections[c].bones[o].body.enabled, "ball 0 is asleep in 11042");
    let mut sim = game_sim::Sim::new(0x5241);
    let mut plane = plane.with_hull(hull);
    plane.body = body_of(&ee, ee.u32_at(PLANE + 0x160));
    let before = plane.body.velocity;
    sim.plane = Some(plane);
    sim.traffic.props = set;
    let dt = F(ee.f32_at(addr("frame_dt").unwrap()));
    let mut woke = None;
    for tick in 1..=12 {
        sim.step(dt, &Default::default(), &mut ground);
        if woke.is_none() && sim.traffic.props.collections[c].bones[o].body.enabled {
            woke = Some(tick);
        }
    }
    let ball = sim.traffic.props.collections[c].bones[o].body;
    let after = sim.plane.as_ref().unwrap().body.velocity;
    eprintln!("ball 0 woke on tick {woke:?}, velocity {:?}; the plane's {:?} -> {:?}", ball.velocity.0, before.0, after.0);
    assert_eq!(woke, Some(3), "ball 0 wakes on the original's tick, 11046");
    let along = |v: Vf| v.0[0] * before.0[0] + v.0[2] * before.0[2];
    assert!(along(ball.velocity) > 0.0, "ball 0 is thrown along the plane's way");
}
