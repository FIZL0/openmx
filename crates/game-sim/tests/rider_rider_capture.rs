//! S88: the two riders' collisions in the original's split screen, replayed in the port bit for bit.
//!
//! S93 captured two rides with both riders in every row (`tests/split.rs`) and saved the original's
//! RAM at the tick before each named contact. The rider-against-rider ones are the long ride's 3403
//! (P1 at speed into P2 standing: P1 goes over the bars) and the short ride's 5998 (P2 into P1: P2 goes
//! over). Each image is the row before its contact, and the interpreter running the level's collision
//! node from it (every object's list emptied, every object's search, then the bikes' updates, each
//! resolving its own list at the top of its substeps) lands on the contact row for both chassis. So:
//!
//! - [`the_original_tick_from_each_image_is_the_contact_row`]: the image, the tick and the wreck;
//! - [`the_port_finds_the_originals_contacts`]: [`rider_contact::gather`] on the two frames against
//!   the original's search, every field, and the captured entries;
//! - [`the_port_resolves_every_rider_call_as_the_original`]: every resolve the original makes for a
//!   rider in the tick, the port's [`Bump::resolve`] on the same RAM gives the same bits, for both
//!   chassis;
//! - [`the_tick_with_the_ports_resolve_is_the_contact_row`]: the port's resolve standing in for the
//!   original's, the tick still lands on the row and the same rider wrecks;
//! - [`the_original_steps_the_last_player_first`]: the order (S78's open question), which the port's
//!   `Sim::step_all` follows.
//!
//! The long ride's 3414 (P2 sliding into P1's wreck) is the original's side only
//! ([`the_original_tick_into_the_wreck_is_row_3414`]); the biplane wrecks are S87's. The images and
//! captures are game data, git-ignored (`s93-long-3413.ram` is S88's own, made the way S93 made the
//! others): absent, the tests skip.

use game_render::disc::Disc;
use game_sim::bike_col::{self, BikeCol};
use game_sim::contact::{Collider, Contact, Node, Ridden, Vehicle};
use game_sim::math::Mat4;
use game_sim::phy::Phy;
use game_sim::ride::Tables;
use game_sim::rider_contact::{self, Bump, Seat};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::cell::RefCell;
use std::collections::HashMap;
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

/// A RAM image S93 saved, ready to call into.
fn image(name: &str) -> Option<Ee> {
    let path = ctx(&format!("states/{name}.ram"))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// The split track's bike: race125p.phy and the bike's collision files.
fn disc(tables: &Tables) -> Option<(Phy, BikeCol)> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
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
        .find_map(|b| disc.res(b).ok()?.read_suffix("race125p.phy").ok())
        .expect("race125p.phy in a bike archive");
    Some((Phy::read_bytes(&phy, &tables.curve), col))
}

/// One split capture's rows by column (the reader of `tests/split.rs`, cut to what this needs).
struct Split {
    cols: HashMap<String, usize>,
    rows: Vec<Vec<String>>,
}

impl Split {
    fn load(name: &str) -> Option<Split> {
        let text = std::fs::read_to_string(ctx(&format!("capture/split/{name}.csv"))?).expect("read the capture");
        let mut lines = text.lines();
        let cols = lines.next().expect("a header").split(',').enumerate().map(|(i, c)| (c.to_string(), i)).collect();
        let rows = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
        Some(Split { cols, rows })
    }

    fn text(&self, row: usize, col: &str) -> &str {
        &self.rows[row][*self.cols.get(col).unwrap_or_else(|| panic!("no column {col}"))]
    }

    fn u32(&self, row: usize, col: &str) -> u32 {
        let s = self.text(row, col);
        s.parse().unwrap_or_else(|_| panic!("{col} in row {row}: {s:?}"))
    }

    /// The captured float, exact: the capture prints an f32 widened to f64.
    fn f32(&self, row: usize, col: &str) -> f32 {
        let s = self.text(row, col);
        let v = s.parse::<f64>().unwrap_or_else(|_| panic!("{col} in row {row}: {s:?}"));
        assert_eq!(f64::from(v as f32), v, "{col} in row {row} is not an f32");
        v as f32
    }

    fn row_at(&self, frame: u32) -> usize {
        (0..self.rows.len()).find(|&r| self.u32(r, "frame_counter") == frame).unwrap_or_else(|| panic!("no row for frame {frame}"))
    }

    fn vec3(&self, row: usize, col: &str) -> [u32; 3] {
        ["x", "y", "z"].map(|c| self.f32(row, &format!("{col}.{c}")).to_bits())
    }
}

/// A named rider-against-rider contact: the image of the tick before, its capture, the contact's
/// frame, and each rider's wreck reason after the tick (the one who strikes at speed goes over the
/// bars, 1; the one standing is shoved and stays up).
///
/// `p1_torn`: the row caught P1 between P2's update and its own. The capture is read while the game
/// runs, and 5998's row has P1 exactly as P2's push left it — its centre is 5997's to the bit, and
/// 5999's has moved by two ticks' worth — so P1 is compared there, P2 at the tick's end. (3403's row
/// is torn later on: both chassis are the tick's end, but P1's wreck shows a row late, at 3404.)
struct Named {
    image: &'static str,
    capture: &'static str,
    frame: u32,
    wreck: [u32; 2],
    p1_torn: bool,
}

const NAMED: [Named; 2] = [
    Named { image: "s93-long-3402", capture: "splitscreen", frame: 3403, wreck: [1, 0], p1_torn: false },
    Named { image: "s93-short-5997", capture: "splitscreen2", frame: 5998, wreck: [0, 1], p1_torn: true },
];

/// Every image S93 saved in the split rides: the two above, the three biplane ones and the two the
/// rides were recorded from.
const IMAGES: [&str; 7] =
    ["s93-long-3402", "s93-long-4661", "s93-long-5074", "s93-long-5505", "s93-short-5997", "splitscreen", "splitscreen2_shortcolision"];

/// The level's collision node (oracle.txt, S88), and P1's and P2's ridden vehicles.
const NODE: u32 = 0x0061_e720;
const PLAYERS: [u32; 2] = [0x00c1_f810, 0x012e_3ad0];
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

fn put(ee: &mut Ee, a: u32, v: Vf) {
    for (k, x) in v.0.iter().enumerate() {
        ee.set_f32(a + 4 * k as u32, *x);
    }
}

fn bits(v: Vf) -> [u32; 4] {
    v.to_bits()
}

/// P1's and P2's collision objects.
fn records(ee: &Ee) -> [u32; 2] {
    PLAYERS.map(|v| ee.u32_at(v + 0x7d0))
}

fn chassis(ee: &Ee, p: usize) -> u32 {
    ee.u32_at(PLAYERS[p] + 0x7a4)
}

fn wreck(ee: &Ee, p: usize) -> u32 {
    ee.u32_at(PLAYERS[p] + 0x748)
}

fn node_at(ee: &Ee, n: u32) -> Node {
    Node { rows: [0, 1, 2].map(|k| quad(ee, n + 0x80 + 16 * k)), origin: quad(ee, n + 0xb0) }
}

fn mat_at(ee: &Ee, n: u32) -> Mat4 {
    let node = node_at(ee, n);
    Mat4([node.rows[0], node.rows[1], node.rows[2], node.origin])
}

/// A list entry, its other side named by player (99: something not a rider).
fn contact_at(ee: &Ee, e: u32, recs: &[u32; 2]) -> Contact {
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

fn list_at(ee: &Ee, r: u32, recs: &[u32; 2]) -> Vec<Contact> {
    let n = ee.u32_at(r + 0x100);
    let at = ee.u32_at(r + 0x108);
    (0..n).map(|k| contact_at(ee, at + 0x80 * k, recs)).collect()
}

/// An object's chassis as the other side of a contact.
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

/// An object's chassis as the own side.
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
    let mut bad: Vec<&'static str> = quads.iter().filter(|(_, p, o)| bits(*p) != bits(*o)).map(|(n, _, _)| *n).collect();
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

/// The port's resolve of one call the original makes: the object's list, both players' objects and
/// chassis, read off RAM as the call finds them.
fn port_resolve(ee: &Ee, recs: [u32; 2], me: usize, first: bool, colliders: &[Collider]) -> (Bump, Ridden) {
    let mut bump = Bump {
        me,
        contacts: list_at(ee, recs[me], &recs),
        colliders: colliders.to_vec(),
        vehicles: recs.iter().map(|&r| vehicle_at(ee, r)).collect(),
        movers: Vec::new(),
    };
    let mut own = ridden_at(ee, recs[me]);
    bump.resolve(&mut own, first);
    (bump, own)
}

/// Run the original's resolve from inside a hook on it: the hooked call's registers and stack are put
/// back, so the hook returns to its caller as the call would have.
fn run_original(ee: &mut Ee, r: u32, first: u32) {
    let (ra, sp, stack) = (ee.r(31), ee.r(29), ee.stack);
    ee.stack = (sp as u32 - 0x400) & !0xf;
    ee.call(RESOLVE, &[u64::from(r), u64::from(first)], &[], 50_000_000);
    ee.stack = stack;
    ee.set(29, sp);
    ee.set(31, ra);
    ee.pc = RESOLVE;
}

/// A rider's captured chassis quantities as RAM holds them, in [`CHASSIS`]'s order.
type Quantities = [[u32; 3]; 7];

fn quantities(ee: &Ee, p: usize) -> Quantities {
    let ch = chassis(ee, p);
    CHASSIS.map(|(_, at)| {
        let b = quad(ee, ch + at).to_bits();
        [b[0], b[1], b[2]]
    })
}

/// Both riders' quantities against a capture row: every one whose bits differ.
fn against_row(got: [Quantities; 2], cap: &Split, row: usize) -> Vec<String> {
    let mut bad = vec![];
    for p in 0..2 {
        for (k, (name, _)) in CHASSIS.iter().enumerate() {
            let want = cap.vec3(row, &format!("p{}_ch_{name}", p + 1));
            if got[p][k] != want {
                bad.push(format!("p{} {name}: {:?} vs captured {:?}", p + 1, got[p][k], want));
            }
        }
    }
    bad
}

fn dt(ee: &Ee) -> f32 {
    ee.f32_at(addr("frame_dt").unwrap())
}

/// A resolve, the original's or a stand-in: (machine, object, first substep).
type Resolve = Box<dyn FnMut(&mut Ee, u32, u32)>;

/// One tick of the node from the image with `resolve` answering every object's resolve call; then
/// the riders as the contact's row should hold them (see [`Named::p1_torn`]) and their wreck reasons.
fn tick(mut ee: Ee, named: &Named, mut resolve: Resolve) -> ([Quantities; 2], [u32; 2]) {
    let p2 = records(&ee)[1];
    let torn = Rc::new(RefCell::new(None));
    let seen = Rc::clone(&torn);
    ee.hooks.insert(RESOLVE, Box::new(move |ee: &mut Ee| {
        let (r, first) = (ee.r(4) as u32, ee.r(5) as u32);
        resolve(ee, r, first);
        // P2's second substep's resolve is its last: P1 as the rest of P2's update leaves it
        if r == p2 && first == 0 {
            *seen.borrow_mut() = Some(quantities(ee, 0));
        }
    }));
    let dt = dt(&ee);
    ee.call(NODE_UPDATE, &[u64::from(NODE)], &[dt], BUDGET);
    let p1 = if named.p1_torn { torn.borrow().expect("P2 resolved") } else { quantities(&ee, 0) };
    ([p1, quantities(&ee, 1)], [wreck(&ee, 0), wreck(&ee, 1)])
}

#[test]
fn the_original_tick_from_each_image_is_the_contact_row() {
    for named in &NAMED {
        let (Some(ee), Some(cap)) = (image(named.image), Split::load(named.capture)) else { return };
        let row = cap.row_at(named.frame);
        let before = [quantities(&ee, 0), quantities(&ee, 1)];
        assert_eq!(against_row(before, &cap, row - 1), Vec::<String>::new(), "{} is not the row before", named.image);
        assert_eq!([wreck(&ee, 0), wreck(&ee, 1)], [0, 0], "{}: a rider is already down", named.image);
        let (riders, wrecks) = tick(ee, named, Box::new(run_original));
        assert_eq!(against_row(riders, &cap, row), Vec::<String>::new(), "{} ticked is not {}", named.image, named.frame);
        assert_eq!(wrecks, named.wreck, "{}: who wrecks", named.image);
    }
}

#[test]
fn the_port_finds_the_originals_contacts() {
    let tables = Tables::build();
    let Some((_, col)) = disc(&tables) else { return };
    for named in &NAMED {
        let (Some(mut ee), Some(cap)) = (image(named.image), Split::load(named.capture)) else { return };
        let dt = dt(&ee);
        let recs = records(&ee);
        let seats: Vec<Seat<'_>> = recs
            .iter()
            .map(|&r| Seat { col: Some(&col), at: mat_at(&ee, ee.u32_at(r + 0xf8)), ghost: false, riding: true })
            .collect();
        let port = rider_contact::gather(&seats, F(dt));

        // the original's first half of the node's update: every object emptied, then every one searches
        let count = ee.u32_at(NODE + 0x30);
        let objects: Vec<u32> = (0..count).map(|k| ee.u32_at(ee.u32_at(NODE + 0x38) + 4 * k)).collect();
        assert!(objects.ends_with(&recs), "{}: the riders' objects are not the node's last two", named.image);
        for &o in &objects {
            ee.call(ee.u32_at(ee.u32_at(o + 8) + 0x74), &[u64::from(o)], &[], 50_000_000);
        }
        for &o in &objects {
            ee.call(ee.u32_at(ee.u32_at(o + 8) + 0x7c), &[u64::from(o)], &[dt], 50_000_000);
        }

        let row = cap.row_at(named.frame);
        for p in 0..2 {
            let orig = list_at(&ee, recs[p], &recs);
            assert!(!orig.is_empty(), "{}: p{} found nothing", named.image, p + 1);
            assert_eq!(port[p].len(), orig.len(), "{}: p{}'s contacts", named.image, p + 1);
            for (k, (a, b)) in port[p].iter().zip(&orig).enumerate() {
                assert_eq!(differ(a, b), Vec::<&str>::new(), "{}: p{} contact {k}\nport {a:?}\noriginal {b:?}", named.image, p + 1);
                // and what the capture kept of it at the end of the tick
                let at = |f: &str| format!("p{}_c{k}.{f}", p + 1);
                assert_eq!(cap.u32(row, &at("kind")), a.kind);
                assert_eq!(cap.u32(row, &at("other")), recs[a.other.expect("a rider on the other side")]);
                for (name, v) in [("end", a.on_body), ("normal", a.normal), ("point", a.point)] {
                    assert_eq!(cap.vec3(row, &at(name)), <[u32; 3]>::try_from(&bits(v)[..3]).unwrap(), "{}: {}", named.image, at(name));
                }
            }
        }
    }
}

/// The riders' tuning, which is the port's [`rider_contact::collider`] for race125p
/// (`tests/rider_contact.rs` proves it against both objects).
fn colliders(phy: &Phy) -> Vec<Collider> {
    vec![rider_contact::collider(&phy.collisions, phy.mass.mass); 2]
}

#[test]
fn the_port_resolves_every_rider_call_as_the_original() {
    let tables = Tables::build();
    let Some((phy, _)) = disc(&tables) else { return };
    for named in &NAMED {
        let Some(mut ee) = image(named.image) else { return };
        let dt = dt(&ee);
        let recs = records(&ee);
        let colliders = colliders(&phy);
        let log = Rc::new(RefCell::new(Vec::<(usize, u32, usize, Vec<String>)>::new()));
        let seen = Rc::clone(&log);
        ee.hooks.insert(RESOLVE, Box::new(move |ee: &mut Ee| {
            let (r, first) = (ee.r(4) as u32, ee.r(5) as u32);
            let port = recs.iter().position(|&x| x == r).map(|me| port_resolve(ee, recs, me, first != 0, &colliders));
            run_original(ee, r, first);
            let Some((bump, own)) = port else { return };
            let me = bump.me;
            let got = ridden_at(ee, r);
            let mut bad = vec![];
            let own_pairs = [
                ("velocity", own.chassis.velocity, got.chassis.velocity),
                ("rates", own.chassis.rates, got.chassis.rates),
                ("shock", own.chassis.shock, got.chassis.shock),
                ("twist", own.chassis.twist, got.chassis.twist),
                ("force", own.force, got.force),
                ("torque", own.torque, got.torque),
            ];
            let them = vehicle_at(ee, recs[1 - me]);
            let pushed = &bump.vehicles[1 - me];
            let other_pairs = [
                ("other's velocity", pushed.velocity, them.velocity),
                ("other's rates", pushed.rates, them.rates),
                ("other's shock", pushed.shock, them.shock),
                ("other's twist", pushed.twist, them.twist),
            ];
            for (what, p, o) in own_pairs.iter().chain(&other_pairs) {
                if bits(*p) != bits(*o) {
                    bad.push(format!("{what}: port {p:?} original {o:?}"));
                }
            }
            for (k, (p, o)) in bump.contacts.iter().zip(&list_at(ee, r, &recs)).enumerate() {
                if bits(p.arm) != bits(o.arm) || bits(p.other_arm) != bits(o.other_arm) {
                    bad.push(format!("contact {k}'s arms"));
                }
            }
            seen.borrow_mut().push((me, first, bump.contacts.len(), bad));
        }));
        ee.call(NODE_UPDATE, &[u64::from(NODE)], &[dt], BUDGET);
        let log = log.borrow();
        // both riders, both substeps, with their contacts
        assert_eq!(log.len(), 4, "{}: {log:?}", named.image);
        for (me, first, n, bad) in log.iter() {
            assert!(*n > 0, "{}: p{} resolved an empty list", named.image, me + 1);
            assert!(bad.is_empty(), "{}: p{} substep {first}: {bad:#?}", named.image, me + 1);
        }
    }
}

#[test]
fn the_tick_with_the_ports_resolve_is_the_contact_row() {
    let tables = Tables::build();
    let Some((phy, _)) = disc(&tables) else { return };
    for named in &NAMED {
        let (Some(ee), Some(cap)) = (image(named.image), Split::load(named.capture)) else { return };
        let recs = records(&ee);
        let colliders = colliders(&phy);
        let stood_in = Rc::new(RefCell::new(0));
        let count = Rc::clone(&stood_in);
        let port: Resolve = Box::new(move |ee: &mut Ee, r, first| {
            let Some(me) = recs.iter().position(|&x| x == r) else {
                run_original(ee, r, first);
                return;
            };
            // the port in the original's place: what it computes, written where the original would
            let (bump, own) = port_resolve(ee, recs, me, first != 0, &colliders);
            for (i, &rec) in recs.iter().enumerate() {
                let ch = ee.u32_at(rec + 0xf0);
                let v = if i == me { &own.chassis } else { &bump.vehicles[i] };
                put(ee, ch + 0x260, v.velocity);
                put(ee, ch + 0x290, v.rates);
                put(ee, ch + 0x170, v.shock);
                put(ee, ch + 0x180, v.twist);
            }
            let c = &bump.colliders[me];
            put(ee, r + 0xa0, c.cofg_world);
            ee.set_f32(r + 0x1d8, c.normal_speed.0);
            put(ee, r + 0x1e0, c.relative);
            let ch = ee.u32_at(r + 0xf0);
            put(ee, ch + 0x130, own.force);
            put(ee, ch + 0x140, own.torque);
            let at = ee.u32_at(r + 0x108);
            for (k, c) in bump.contacts.iter().enumerate() {
                put(ee, at + 0x80 * k as u32 + 0x50, c.arm);
                put(ee, at + 0x80 * k as u32 + 0x60, c.other_arm);
            }
            *count.borrow_mut() += 1;
            // and the hook returns to the caller, as the call would have
        });
        let (riders, wrecks) = tick(ee, named, port);
        assert_eq!(*stood_in.borrow(), 4, "{}", named.image);
        let row = cap.row_at(named.frame);
        assert_eq!(against_row(riders, &cap, row), Vec::<String>::new(), "{} with the port's resolve", named.image);
        assert_eq!(wrecks, named.wreck, "{} with the port's resolve: who wrecks", named.image);
    }
}

/// S78 left it open which chassis resolves first; the images settle it: the node updates P2's
/// vehicle, both substeps, before P1's, so P1 meets P2's chassis already pushed. The port's
/// `Sim::step_all` steps the last player first for it.
#[test]
fn the_original_steps_the_last_player_first() {
    let mut ran = 0;
    for name in IMAGES {
        let Some(mut ee) = image(name) else { continue };
        let dt = dt(&ee);
        let recs = records(&ee);
        let log = Rc::new(RefCell::new(Vec::<(usize, u32)>::new()));
        let seen = Rc::clone(&log);
        ee.hooks.insert(RESOLVE, Box::new(move |ee: &mut Ee| {
            let (r, first) = (ee.r(4) as u32, ee.r(5) as u32);
            if let Some(p) = recs.iter().position(|&x| x == r) {
                seen.borrow_mut().push((p + 1, first));
            }
            run_original(ee, r, first);
        }));
        ee.call(NODE_UPDATE, &[u64::from(NODE)], &[dt], BUDGET);
        assert_eq!(*log.borrow(), [(2, 1), (2, 0), (1, 1), (1, 0)], "{name}: the riders' resolves in order (player, first substep)");
        ran += 1;
    }
    assert!(ran == 0 || ran == IMAGES.len(), "only {ran} of the images are there");
}

/// 3414: P2 slides back into P1's wreck (S93's image of 3413, saved for S88). The original's tick from
/// the image is the row for both riders, P1 still down. P1's riding object has left the node's list
/// (the wreck bike's is in its place) and is not resolved, yet P2's own rays strike its frozen frame
/// and the mirror goes into it, and P2 also resolves the wreck bike's mirrored hit. The port has
/// neither — a rider who is down is in no list and struck by nobody, and the ride has no wreck bike
/// (S105) — so only the original's side is proved here.
#[test]
fn the_original_tick_into_the_wreck_is_row_3414() {
    let (Some(mut ee), Some(cap)) = (image("s93-long-3413"), Split::load("splitscreen")) else { return };
    let recs = records(&ee);
    let row = cap.row_at(3414);
    let before = [quantities(&ee, 0), quantities(&ee, 1)];
    assert_eq!(against_row(before, &cap, row - 1), Vec::<String>::new(), "the image is not 3413");
    assert_eq!([wreck(&ee, 0), wreck(&ee, 1)], [1, 0], "P1 is down from 3403's contact");
    let count = ee.u32_at(NODE + 0x30);
    let objects: Vec<u32> = (0..count).map(|k| ee.u32_at(ee.u32_at(NODE + 0x38) + 4 * k)).collect();
    assert!(!objects.contains(&recs[0]) && objects.contains(&recs[1]), "P1's riding object is still listed: {objects:x?}");
    let wreck_bike = objects[2];
    assert_eq!(objects[3], recs[1]);
    // the wreck bike's object: on a dynamics body, not a chassis
    assert!(ee.u32_at(wreck_bike + 0xf0) == 0 && ee.u32_at(wreck_bike + 0xf4) != 0, "{wreck_bike:x} is no body's object");

    let lists = Rc::new(RefCell::new(Vec::<(u32, u32, Vec<(u32, u32)>)>::new()));
    let seen = Rc::clone(&lists);
    ee.hooks.insert(RESOLVE, Box::new(move |ee: &mut Ee| {
        let (r, first) = (ee.r(4) as u32, ee.r(5) as u32);
        if recs.contains(&r) {
            let (n, at) = (ee.u32_at(r + 0x100), ee.u32_at(r + 0x108));
            let list = (0..n).map(|k| (ee.u32_at(at + 0x80 * k), ee.u32_at(at + 0x80 * k + 0x74))).collect();
            seen.borrow_mut().push((r, first, list));
        }
        run_original(ee, r, first);
    }));
    let dt = dt(&ee);
    ee.call(NODE_UPDATE, &[u64::from(NODE)], &[dt], BUDGET);
    assert_eq!(against_row([quantities(&ee, 0), quantities(&ee, 1)], &cap, row), Vec::<String>::new(), "3413 ticked is not 3414");
    assert_eq!([wreck(&ee, 0), wreck(&ee, 1)], [1, 0], "P2 stays up");
    // only P2 resolves, both substeps: the wreck bike's mirror, then its own two rays into P1's frame
    let p2 = vec![(1, wreck_bike), (0, recs[0]), (0, recs[0])];
    assert_eq!(*lists.borrow(), [(recs[1], 1, p2.clone()), (recs[1], 0, p2)]);
    // and the capture has them, P1's frozen object holding the mirrors of P2's rays
    for (k, &(kind, other)) in [(1, wreck_bike), (0, recs[0]), (0, recs[0])].iter().enumerate() {
        assert_eq!((cap.u32(row, &format!("p2_c{k}.kind")), cap.u32(row, &format!("p2_c{k}.other"))), (kind, other), "P2's entry {k}");
    }
    for k in 0..2 {
        assert_eq!((cap.u32(row, &format!("p1_c{k}.kind")), cap.u32(row, &format!("p1_c{k}.other"))), (1, recs[1]), "P1's entry {k}");
    }
}
