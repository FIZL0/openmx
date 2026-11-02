//! N22a1: the two players' planes flown into each other in the original's split screen, against the
//! port's planes stepped together ([`flight::frames_together`]).
//!
//! No tape of two planes meeting exists, so the reference is the original's own code: S93's
//! `splitscreen.ram` has both players' planes parked on free09, 58 ft apart nose to tail along x,
//! and plane 0's body is given 40 ft/s along x ([`touching`]); the level's collision node's update is
//! run tick after tick until the planes part. Both planes' collision objects are records on the node
//! (`[plane 0, plane 1, P2, P1]`): every list emptied, every object's search in that order, then the
//! planes' dynamics world, four substeps each resolving plane 0 then plane 1 (oracle.txt N22a). A
//! plane's rays strike the other's hull (`biplanef.col`, category [`flight::COLLIDABLE`]): its entry
//! names the other's record, the mirror goes on the other's list — after its own entries when it
//! searched first, ahead of them when it searches after — and each resolve pushes the other's body.
//! So, on every tick of the touch:
//!
//! - [`the_original_planes_meet_and_part`]: the ticks the touch spans, who strikes on each;
//! - [`the_port_finds_both_planes_lists`]: [`flight::searches_together`] on both planes off RAM
//!   against the original's two searches, each list every field, in order;
//! - [`the_port_resolves_every_plane_call_as_the_original`]: each of the eight resolves, the port's
//!   ([`contact::resolve`] with the bikes as vehicles and the other plane's body as a
//!   [`Side::Body`]), both planes' bodies the same bits after it;
//! - [`the_ticks_with_the_ports_resolves_are_the_originals`]: the port's resolves standing in for
//!   the original's on every tick from the poke to after the touch, both planes' bodies every field
//!   the original's own run's after each.
//!
//! The image is game data, git-ignored: absent, the tests skip.

use game_sim::biplane::Body;
use game_sim::contact::{self, Collider, Contact, Node, Other, Side, Vehicle, Weighed};
use game_sim::flight::{self, Flight};
use game_sim::flight_search::{self, Rays};
use game_sim::phy::{self, Plane};
use game_sim::track;
use game_sim::archive::{Psr, Res};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::rc::Rc;

const NODE: u32 = 0x0061_e720;
const PLAYERS: [u32; 2] = [0x00c1_f810, 0x012e_3ad0];
const NODE_UPDATE: u32 = 0x0026_f800;
const RESOLVE: u32 = 0x0027_0dc0;
const BUDGET: u64 = 2_000_000_000;

/// The speed plane 0 is given along x, ft/s, and how many ticks are walked from it: the planes
/// touch on the 40th and have parted well before the last.
const SPEED: f32 = 40.0;
const TICKS: usize = 80;

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

/// S93's split-screen image, ready to call into.
fn image() -> Option<Ee> {
    let path = ctx("states/s93/splitscreen.ram").or_else(|| ctx("states/splitscreen.ram"))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// The image with plane 0 given [`SPEED`] along x: the planes' tick 0.
fn touching() -> Option<Ee> {
    let mut ee = image()?;
    let b = body_at(&ee, objects(&ee)[0]);
    let mut v = quad(&ee, b + body_at::VELOCITY);
    v.0[0] = SPEED;
    put(&mut ee, b + body_at::VELOCITY, v);
    Some(ee)
}

/// Another machine over the same memory, to run a tick on aside.
fn fork(ee: &Ee) -> Ee {
    let mut e = Ee::with_ram(ee.ram.clone());
    e.spr.clone_from(&ee.spr);
    e.stack = ee.stack;
    e.set(28, ee.r(28));
    e
}

/// The plane as the port builds it off `machine.res`, with its hull.
fn the_plane() -> Option<Flight> {
    let kit = Kit::load().ok()?;
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
    Some(Flight::new(&plane).with_rays(Rays::of(&rays)).with_hull(hull))
}

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
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)))
}

fn put(ee: &mut Ee, a: u32, v: Vf) {
    for (k, x) in v.0.iter().enumerate() {
        ee.set_f32(a + 4 * k as u32, *x);
    }
}

fn bits(v: Vf) -> [u32; 4] {
    v.to_bits()
}

fn body_of(ee: &Ee, b: u32) -> Body {
    use body_at::*;
    Body {
        enabled: ee.u32_at(b + ENABLED) != 0,
        inv_mass: F(ee.f32_at(b + INV_MASS)),
        damping: quad(ee, b + DAMPING),
        last_position: quad(ee, b + LAST_POSITION),
        position: quad(ee, b + POSITION),
        velocity: quad(ee, b + VELOCITY),
        last_force: quad(ee, b + LAST_FORCE),
        force: quad(ee, b + FORCE),
        accel: quad(ee, b + ACCEL),
        shock_peak: quad(ee, b + SHOCK_PEAK),
        shock: quad(ee, b + SHOCK),
        step: quad(ee, b + STEP),
        inv_inertia: quad(ee, b + INV_INERTIA),
        ang_damping: quad(ee, b + ANG_DAMPING),
        last_quat: quad(ee, b + LAST_QUAT),
        quat: quad(ee, b + QUAT),
        cofg: quad(ee, b + COFG),
        moment: quad(ee, b + MOMENT),
        last_moment: quad(ee, b + LAST_MOMENT),
        rates: quad(ee, b + RATES),
        ang_accel: quad(ee, b + ANG_ACCEL),
        twist_peak: quad(ee, b + TWIST_PEAK),
        twist: quad(ee, b + TWIST),
    }
}

/// What a resolve writes on a body.
fn body_fields(b: &Body) -> [(&'static str, Vf); 6] {
    [("velocity", b.velocity), ("shock", b.shock), ("rates", b.rates), ("twist", b.twist), ("force", b.force), ("moment", b.moment)]
}

fn put_body(ee: &mut Ee, at: u32, b: &Body) {
    use body_at::*;
    for (off, v) in [(VELOCITY, b.velocity), (SHOCK, b.shock), (RATES, b.rates), (TWIST, b.twist), (FORCE, b.force), (MOMENT, b.moment)] {
        put(ee, at + off, v);
    }
}

fn objects(ee: &Ee) -> Vec<u32> {
    let count = ee.u32_at(NODE + 0x30);
    (0..count).map(|k| ee.u32_at(ee.u32_at(NODE + 0x38) + 4 * k)).collect()
}

/// The records by the port's names for them: P1's and P2's, then plane 0's and plane 1's.
fn records(ee: &Ee) -> [u32; 4] {
    let o = objects(ee);
    let [p1, p2] = PLAYERS.map(|v| ee.u32_at(v + 0x7d0));
    [p1, p2, o[0], o[1]]
}

fn body_at(ee: &Ee, rec: u32) -> u32 {
    ee.u32_at(rec + 0xf4)
}

fn node_at(ee: &Ee, n: u32) -> Node {
    Node { rows: [0, 1, 2].map(|k| quad(ee, n + 0x80 + 16 * k)), origin: quad(ee, n + 0xb0) }
}

/// A list entry, its other side named by [`records`]' index (99: something else).
fn contact_at(ee: &Ee, e: u32, recs: &[u32; 4]) -> Contact {
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
        other: (other != 0).then(|| recs.iter().position(|&o| o == other).unwrap_or(99)),
        mover: (ee.u32_at(e + 0x78) != 0).then_some(99),
    }
}

fn list_at(ee: &Ee, r: u32, recs: &[u32; 4]) -> Vec<Contact> {
    let n = ee.u32_at(r + 0x100);
    let at = ee.u32_at(r + 0x108);
    (0..n).map(|k| contact_at(ee, at + 0x80 * k, recs)).collect()
}

/// Plane `k`'s list as the port's names it: the riders by player, the other plane after them
/// ([`flight::frames_together`]: two riders, then the one other plane).
fn as_port(list: Vec<Contact>, k: usize) -> Vec<Contact> {
    list.into_iter().map(|c| Contact { other: c.other.map(|o| if o == 2 + (1 - k) { 2 } else { o }), ..c }).collect()
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

fn differ(port: &Contact, orig: &Contact) -> Vec<&'static str> {
    let quads = [
        ("from", port.from, orig.from),
        ("on_body", port.on_body, orig.on_body),
        ("normal", port.normal, orig.normal),
        ("point", port.point, orig.point),
        ("arm", port.arm, orig.arm),
        ("other_arm", port.other_arm, orig.other_arm),
    ];
    let mut bad: Vec<&'static str> = quads.iter().filter(|(_, p, o)| bits(*p) != bits(*o)).map(|(n, _, _)| *n).collect();
    if (port.kind, port.surface, port.other, port.mover) != (orig.kind, orig.surface, orig.other, orig.mover) {
        bad.push("kind, surface, other or mover");
    }
    bad
}

fn run_original(ee: &mut Ee, r: u32, first: u32) {
    let (ra, sp, stack) = (ee.r(31), ee.r(29), ee.stack);
    ee.stack = (sp as u32 - 0x400) & !0xf;
    ee.call(RESOLVE, &[u64::from(r), u64::from(first)], &[], 50_000_000);
    ee.stack = stack;
    ee.set(29, sp);
    ee.set(31, ra);
    ee.pc = RESOLVE;
}

fn dt(ee: &Ee) -> f32 {
    ee.f32_at(addr("frame_dt").unwrap())
}

/// The original's first half of the node's update: every object emptied, then every one's search.
fn searched(ee: &mut Ee) {
    let dt = dt(ee);
    let objects = objects(ee);
    for &o in &objects {
        ee.call(ee.u32_at(ee.u32_at(o + 8) + 0x74), &[u64::from(o)], &[], 50_000_000);
    }
    for &o in &objects {
        ee.call(ee.u32_at(ee.u32_at(o + 8) + 0x7c), &[u64::from(o)], &[dt], 50_000_000);
    }
}

/// Which planes' own rays strike the other plane in this tick's searches.
fn strikers(ee: &Ee) -> [bool; 2] {
    let mut e = fork(ee);
    searched(&mut e);
    let recs = records(&e);
    [0, 1].map(|k| list_at(&e, recs[2 + k], &recs).iter().any(|c| c.kind == 0 && c.other == Some(2 + (1 - k))))
}

/// A plane's collision object as its resolve reads it: the port's tuning, and the scratch the last
/// resolve left in RAM.
fn collider_at(ee: &Ee, plane: &Flight, rec: u32) -> Collider {
    Collider { cofg_world: quad(ee, rec + 0xa0), normal_speed: F(ee.f32_at(rec + 0x1d8)), relative: quad(ee, rec + 0x1e0), ..plane.collider }
}

/// The port's resolve of plane `k`'s call, as [`flight::frames_together`]' force listener makes it:
/// its list against the riders as vehicles and the other plane's body, off RAM. Both bodies after it.
fn port_plane(ee: &Ee, plane: &Flight, recs: [u32; 4], k: usize) -> ([Body; 2], [Collider; 2], Vec<Contact>) {
    let mut bodies = [0, 1].map(|j| body_of(ee, body_at(ee, recs[2 + j])));
    let mut colliders = [0, 1].map(|j| collider_at(ee, plane, recs[2 + j]));
    let mut contacts = as_port(list_at(ee, recs[2 + k], &recs), k);
    // the riders are never named here: what they are tuned to does not matter
    let mut riders = vec![plane.collider; 2];
    let mut vehicles: Vec<Vehicle> = recs[..2].iter().map(|&r| vehicle_at(ee, r)).collect();
    let ([b0, b1], [c0, c1]) = (&mut bodies, &mut colliders);
    let ((me, mine), (other, theirs)) = if k == 0 { ((b0, c0), (b1, c1)) } else { ((b1, c1), (b0, c0)) };
    let mut pushed = Weighed { body: other, mass: plane.mass };
    let mut others: Vec<Other<'_>> = riders
        .iter_mut()
        .zip(vehicles.iter_mut())
        .map(|(collider, v)| Other { collider, side: Side::Vehicle(v) })
        .collect();
    others.push(Other { collider: theirs, side: Side::Body(&mut pushed) });
    contact::resolve(mine, &mut Weighed { body: me, mass: plane.mass }, &mut contacts, &mut others, &[], true);
    drop(others);
    (bodies, colliders, contacts)
}

#[test]
fn the_original_planes_meet_and_part() {
    let Some(mut ee) = touching() else { return };
    let o = objects(&ee);
    assert_eq!(o.len(), 4, "the node's objects {o:x?}");
    for &p in &o[..2] {
        assert!(ee.u32_at(p + 0xf0) == 0 && ee.u32_at(p + 0xf4) != 0, "{p:x} is no plane's object");
        assert_eq!(ee.u32_at(ee.u32_at(p + 0x1f8) + 0x84), flight::COLLIDABLE, "the plane's collidable's category");
    }
    let dt = dt(&ee);
    let mut seen = Vec::new();
    for _ in 0..TICKS {
        seen.push(strikers(&ee));
        ee.call(NODE_UPDATE, &[u64::from(NODE)], &[dt], BUDGET);
    }
    let first = seen.iter().position(|s| s.iter().any(|&b| b)).expect("the planes meet");
    let last = seen.iter().rposition(|s| s.iter().any(|&b| b)).expect("the planes meet");
    assert_eq!(first, 39, "the first tick of the touch");
    assert_eq!(seen[first], [true, false], "plane 0 strikes first");
    assert!(seen[first..=last].iter().any(|s| *s == [true, true]), "both strike on some tick: {:?}", &seen[first..=last]);
    assert!(seen[first..=last].iter().any(|s| *s == [false, true]), "plane 1 alone on some tick: {:?}", &seen[first..=last]);
    assert!(last + 5 < TICKS, "the planes have parted by the walk's end: last touch {last}");
}

#[test]
fn the_port_finds_both_planes_lists() {
    let (Some(mut ee), Some(plane)) = (touching(), the_plane()) else { return };
    let dt = dt(&ee);
    let (mut touched, mut mixed) = (0, [0, 0]);
    for t in 0..TICKS {
        let mut orig = fork(&ee);
        searched(&mut orig);
        let recs = records(&orig);
        let lists = [0, 1].map(|k| as_port(list_at(&orig, recs[2 + k], &recs), k));
        if lists.iter().any(|l| !l.is_empty()) {
            touched += 1;
            let recs = records(&ee);
            let mut planes = [0, 1].map(|k| {
                let mut p = plane.clone();
                p.body = body_of(&ee, body_at(&ee, recs[2 + k]));
                p
            });
            let mut mets = [0, 1].map(|_| flight::Met {
                colliders: vec![plane.collider; 2],
                vehicles: recs[..2].iter().map(|&r| vehicle_at(&ee, r)).collect(),
                ..flight::Met::default()
            });
            let [p0, p1] = &mut planes;
            let [m0, m1] = &mut mets;
            let mut fleet = [
                flight::Fly { flight: p0, pad: game_sim::biplane::Pad::default(), met: Some(m0), bikes: &[] },
                flight::Fly { flight: p1, pad: game_sim::biplane::Pad::default(), met: Some(m1), bikes: &[] },
            ];
            let mut ground = game_sim::world::Flat { y: F(-1.0e6), ..Default::default() };
            flight::searches_together(F(dt), &mut fleet, &mut ground);
            for k in 0..2 {
                let port = &planes[k].record.contacts;
                mixed[k] += usize::from(port.iter().any(|c| c.kind == 0) && port.iter().any(|c| c.kind == 1));
                assert_eq!(port.len(), lists[k].len(), "tick {t}: plane {k}'s list\nport {port:?}\noriginal {:?}", lists[k]);
                for (j, (a, b)) in port.iter().zip(&lists[k]).enumerate() {
                    assert_eq!(differ(a, b), Vec::<&str>::new(), "tick {t}: plane {k} entry {j}\nport {a:?}\noriginal {b:?}");
                }
                let r = &planes[k].record;
                assert!(r.touching && r.on_object && r.touching_other, "tick {t}: plane {k}'s flags");
                // what its rays struck last, which the crash test reads: the other plane, a vehicle
                if port.iter().any(|c| c.kind == 0) {
                    let struck = r.struck.expect("a struck object");
                    assert_eq!(flight_search::category(struck) & flight_search::VEHICLE, flight_search::VEHICLE, "tick {t}: plane {k} struck {struck:x}");
                }
            }
        }
        ee.call(NODE_UPDATE, &[u64::from(NODE)], &[dt], BUDGET);
    }
    assert!(touched > 10, "the touch spans {touched} ticks");
    // plane 0's own entries ahead of plane 1's mirrors, and plane 1's behind plane 0's, on some tick
    assert!(mixed[0] > 0 && mixed[1] > 0, "ticks with both kinds on a list: {mixed:?}");
}

/// Every plane resolve in a tick, the port's beside the original's: each call's misfits, in order.
fn resolve_log(ee: &mut Ee, plane: &Rc<Flight>, stand_in: bool) -> Rc<RefCell<Vec<(u32, u32, usize, Vec<String>)>>> {
    let recs = records(ee);
    let log = Rc::new(RefCell::new(Vec::new()));
    let (seen, plane) = (Rc::clone(&log), Rc::clone(plane));
    ee.hooks.insert(RESOLVE, Box::new(move |ee: &mut Ee| {
        let (r, first) = (ee.r(4) as u32, ee.r(5) as u32);
        let Some(k) = recs[2..].iter().position(|&x| x == r) else {
            run_original(ee, r, first);
            return;
        };
        let (bodies, colliders, contacts) = port_plane(ee, &plane, recs, k);
        let mut bad = vec![];
        if stand_in {
            for j in 0..2 {
                put_body(ee, body_at(ee, recs[2 + j]), &bodies[j]);
                let rec = recs[2 + j];
                put(ee, rec + 0xa0, colliders[j].cofg_world);
                ee.set_f32(rec + 0x1d8, colliders[j].normal_speed.0);
                put(ee, rec + 0x1e0, colliders[j].relative);
            }
            let at = ee.u32_at(r + 0x108);
            for (i, c) in contacts.iter().enumerate() {
                put(ee, at + 0x80 * i as u32 + 0x50, c.arm);
                put(ee, at + 0x80 * i as u32 + 0x60, c.other_arm);
            }
        } else {
            run_original(ee, r, first);
            for j in 0..2 {
                let want = body_of(ee, body_at(ee, recs[2 + j]));
                for ((what, p), (_, o)) in body_fields(&bodies[j]).iter().zip(body_fields(&want)) {
                    if bits(*p) != bits(o) {
                        bad.push(format!("plane {j}'s {what}: port {p:?} original {o:?}"));
                    }
                }
                let want = collider_at(ee, &plane, recs[2 + j]);
                if (bits(colliders[j].cofg_world), colliders[j].normal_speed.0.to_bits(), bits(colliders[j].relative))
                    != (bits(want.cofg_world), want.normal_speed.0.to_bits(), bits(want.relative))
                {
                    bad.push(format!("plane {j}'s collider scratch"));
                }
            }
            for (i, (p, o)) in contacts.iter().zip(&list_at(ee, r, &recs)).enumerate() {
                if bits(p.arm) != bits(o.arm) || bits(p.other_arm) != bits(o.other_arm) {
                    bad.push(format!("contact {i}'s arms"));
                }
            }
        }
        seen.borrow_mut().push((r, first, contacts.len(), bad));
    }));
    log
}

#[test]
fn the_port_resolves_every_plane_call_as_the_original() {
    let (Some(mut ee), Some(plane)) = (touching(), the_plane()) else { return };
    let plane = Rc::new(plane);
    let dt = dt(&ee);
    let recs = records(&ee);
    let mut both = 0;
    for t in 0..TICKS {
        let strikes = strikers(&ee);
        let log = resolve_log(&mut ee, &plane, false);
        ee.call(NODE_UPDATE, &[u64::from(NODE)], &[dt], BUDGET);
        ee.hooks.clear();
        let log = log.borrow();
        let order: Vec<(u32, u32)> = log.iter().map(|&(r, f, _, _)| (r, f)).collect();
        let want: Vec<(u32, u32)> = (0..4).flat_map(|_| [(recs[2], 1), (recs[3], 1)]).collect();
        assert_eq!(order, want, "tick {t}: the planes' resolves interleave, four substeps of plane 0 then plane 1");
        for (r, first, n, bad) in log.iter() {
            let k = recs[2..].iter().position(|x| x == r).unwrap();
            if strikes[k] {
                assert!(*n > 0, "tick {t}: plane {k} struck but resolved an empty list");
            }
            assert!(bad.is_empty(), "tick {t}: plane {k} substep {first}: {bad:#?}");
        }
        both += usize::from(strikes == [true, true]);
    }
    assert!(both > 0, "no tick had both planes striking");
}

#[test]
fn the_ticks_with_the_ports_resolves_are_the_originals() {
    let (Some(mut ee), Some(mut orig), Some(plane)) = (touching(), touching(), the_plane()) else { return };
    let plane = Rc::new(plane);
    let dt = dt(&ee);
    let recs = records(&ee);
    for t in 0..TICKS {
        let log = resolve_log(&mut ee, &plane, true);
        ee.call(NODE_UPDATE, &[u64::from(NODE)], &[dt], BUDGET);
        ee.hooks.clear();
        orig.call(NODE_UPDATE, &[u64::from(NODE)], &[dt], BUDGET);
        assert_eq!(log.borrow().len(), 8, "tick {t}: the eight plane calls stood in");
        for &p in &recs[2..] {
            let (got, want) = (body_of(&ee, body_at(&ee, p)), body_of(&orig, body_at(&orig, p)));
            assert_eq!(format!("{got:?}"), format!("{want:?}"), "tick {t}: the plane {p:x}'s body");
        }
    }
}
