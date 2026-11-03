//! N22a: a riding bike into a player's biplane in the original's split screen, replayed in the port.
//!
//! S93 saved the original's RAM at the tick before each of its split ride's three biplane wrecks: the
//! long ride's 4662 (P1 into the plane), 5075 and 5506 (P2 into it). Both players' planes'
//! collision objects are in the level's collision node, ahead of the riders' (`[plane 0, plane 1,
//! P2, P1]`), and the node's update is S88's: every object's
//! list emptied, every object's search, then the updates — the planes' dynamics world first, four
//! substeps each resolving plane 0 then plane 1 with impulses on, then the riders, the last first,
//! two substeps each. A bike's body rays strike the plane's hull (`biplanef.col`, its collidable
//! category the plane's), the bike's list takes the entry and the plane's the mirror, and the bike's
//! resolve pushes the plane's body as it pushes another bike's chassis. So:
//!
//! - [`the_original_tick_from_each_image_is_the_wreck_row`]: the image, the tick and the wreck;
//! - [`the_port_finds_the_originals_contacts_with_the_plane`]: [`rider_contact::gather_met`] on the
//!   two frames and the planes' hulls against the original's search, the bike's list and the
//!   plane's mirrors, every field;
//! - [`the_port_resolves_every_call_with_the_plane_as_the_original`]: every resolve in the tick, the
//!   bike's ([`Bump::resolve`] with the plane's body among its others) and the plane's (its mirrors
//!   against the bikes, [`contact::resolve`] as the plane's force listener calls it), the same bits;
//! - [`the_tick_with_the_ports_resolve_is_the_wreck_row`]: the port's resolves standing in for the
//!   original's, the tick still lands on the row, the same rider wrecks, and the plane's body is
//!   the original's.
//!
//! 5075's plane also struck P2's bike with its own rays (N22b): the plane's entries name P2's record
//! and P2's list takes their mirrors first. [`the_port_finds_the_originals_contacts_with_the_plane`]
//! compares only the bike-side entries and mirrors there;
//! [`the_planes_own_rays_strike_the_bike_as_the_originals`] compares the rest, the port's plane
//! search against the original's; and the two resolve tests take all three images, P2's chassis
//! after the plane's push and its own resolve against the capture's row. The images and the capture
//! are game data, git-ignored: absent, the tests skip.

use game_render::disc::Disc;
use game_sim::biplane::Body;
use game_sim::bike_col::{self, BikeCol};
use game_sim::contact::{self, Collider, Contact, Node, Other, Ridden, Side, Vehicle, Weighed};
use game_sim::flight::{self, Flight};
use game_sim::flight_search::{self, Rays};
use game_sim::math::Mat4;
use game_sim::phy::{self, Phy, Plane};
use game_sim::ride::Tables;
use game_sim::rider_contact::{self, Bump, Flown, Hull, Seat};
use game_sim::track;
use game_sim::archive::{Psr, Res};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
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
    let path = ctx(&format!("states/s93/{name}.ram")).or_else(|| ctx(&format!("states/{name}.ram")))?;
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

/// The plane as the port builds it off `machine.res` (its tuning `tests/flight_collision.rs` proves),
/// with its hull.
fn the_plane() -> Option<(Flight, track::Mesh)> {
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
    Some((Flight::new(&plane).with_rays(Rays::of(&rays)), hull))
}

/// The split capture's rows by column (S88's reader, cut to what this needs).
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

/// A biplane wreck S93 named: the image of the tick before, the contact's frame, the striker, and
/// each rider's wreck reason after the tick. `own`: the plane's own rays struck the bike too (N22b).
struct Named {
    image: &'static str,
    frame: u32,
    striker: usize,
    wreck: [u32; 2],
    own: bool,
}

const NAMED: [Named; 3] = [
    Named { image: "s93-long-4661", frame: 4662, striker: 0, wreck: [1, 0], own: false },
    Named { image: "s93-long-5074", frame: 5075, striker: 1, wreck: [0, 1], own: true },
    Named { image: "s93-long-5505", frame: 5506, striker: 1, wreck: [0, 1], own: false },
];

/// The level's collision node and P1's and P2's ridden vehicles (oracle.txt, S88); the node's update
/// and a collision object's resolve.
const NODE: u32 = 0x0061_e720;
const PLAYERS: [u32; 2] = [0x00c1_f810, 0x012e_3ad0];
const NODE_UPDATE: u32 = 0x0026_f800;
const RESOLVE: u32 = 0x0027_0dc0;
const BUDGET: u64 = 2_000_000_000;

/// A captured chassis quantity: its column and its offset on the chassis.
const CHASSIS: [(&str, u32); 7] =
    [("centre", 0x60), ("force", 0x130), ("moment", 0x140), ("shock", 0x170), ("twist", 0x180), ("vel", 0x260), ("rates", 0x290)];

/// The body's fields (`tests/biplane.rs`'s `body_at`).
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

/// What a resolve writes on a body, by name: the plane's own side writes all six, a pushed body the
/// four speeds.
fn body_fields(b: &Body) -> [(&'static str, Vf); 6] {
    [("velocity", b.velocity), ("shock", b.shock), ("rates", b.rates), ("twist", b.twist), ("force", b.force), ("moment", b.moment)]
}

fn put_body(ee: &mut Ee, at: u32, b: &Body) {
    use body_at::*;
    for (off, v) in [(VELOCITY, b.velocity), (SHOCK, b.shock), (RATES, b.rates), (TWIST, b.twist), (FORCE, b.force), (MOMENT, b.moment)] {
        put(ee, at + off, v);
    }
}

/// The node's objects: the two planes', then the riders'.
fn objects(ee: &Ee) -> Vec<u32> {
    let count = ee.u32_at(NODE + 0x30);
    (0..count).map(|k| ee.u32_at(ee.u32_at(NODE + 0x38) + 4 * k)).collect()
}

/// The records the port names by index: P1's and P2's, then plane 0's and plane 1's (`players + k`).
fn records(ee: &Ee) -> [u32; 4] {
    let o = objects(ee);
    let [p1, p2] = PLAYERS.map(|v| ee.u32_at(v + 0x7d0));
    [p1, p2, o[0], o[1]]
}

/// A plane's collision object's body.
fn body_at(ee: &Ee, rec: u32) -> u32 {
    ee.u32_at(rec + 0xf4)
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

/// Run the original's resolve from inside a hook on it (S88's).
fn run_original(ee: &mut Ee, r: u32, first: u32) {
    let (ra, sp, stack) = (ee.r(31), ee.r(29), ee.stack);
    ee.stack = (sp as u32 - 0x400) & !0xf;
    ee.call(RESOLVE, &[u64::from(r), u64::from(first)], &[], 50_000_000);
    ee.stack = stack;
    ee.set(29, sp);
    ee.set(31, ra);
    ee.pc = RESOLVE;
}

type Quantities = [[u32; 3]; 7];

fn quantities(ee: &Ee, p: usize) -> Quantities {
    let ch = chassis(ee, p);
    CHASSIS.map(|(_, at)| {
        let b = quad(ee, ch + at).to_bits();
        [b[0], b[1], b[2]]
    })
}

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

/// What the port needs of the ride: the bike's tuning and collision files, the plane and its hull.
struct Kit2 {
    phy: Phy,
    col: BikeCol,
    plane: Flight,
    hull: track::Mesh,
}

fn kit() -> Option<Kit2> {
    let tables = Tables::build();
    let (phy, col) = disc(&tables)?;
    let (plane, hull) = the_plane()?;
    Some(Kit2 { phy, col, plane, hull })
}

impl Kit2 {
    fn colliders(&self) -> Vec<Collider> {
        vec![rider_contact::collider(&self.phy.collisions, self.phy.mass.mass); 2]
    }

    /// Both planes' bodies as the bike's resolve finds them, in RAM.
    fn flown(&self, ee: &Ee, recs: &[u32; 4], plane_colliders: &[Collider; 2]) -> Vec<Flown> {
        (0..2)
            .map(|k| Flown { seat: k, collider: plane_colliders[k], body: body_of(ee, body_at(ee, recs[2 + k])), mass: self.plane.mass })
            .collect()
    }
}

/// The port's resolve of a bike's call: its list, both riders' objects and chassis and both planes'
/// bodies off RAM as the call finds them.
fn port_bike(ee: &Ee, kit: &Kit2, recs: [u32; 4], me: usize, first: bool, planes: &[Collider; 2]) -> (Bump, Ridden) {
    let mut bump = Bump {
        me,
        contacts: list_at(ee, recs[me], &recs),
        colliders: kit.colliders(),
        vehicles: recs[..2].iter().map(|&r| vehicle_at(ee, r)).collect(),
        planes: kit.flown(ee, &recs, planes),
        ..Bump::default()
    };
    let mut own = ridden_at(ee, recs[me]);
    bump.resolve(&mut own, first);
    (bump, own)
}

/// The port's resolve of a plane's call, as its force listener makes it: its list against the
/// riders' objects and chassis off RAM, impulses on. With the riders' chassis after it (N22b: what
/// the plane's own entries on a bike push).
fn port_plane(ee: &Ee, kit: &Kit2, recs: [u32; 4], k: usize, collider: &mut Collider) -> (Body, Vec<Contact>, Vec<Vehicle>) {
    let mut body = body_of(ee, body_at(ee, recs[2 + k]));
    let mut contacts = list_at(ee, recs[2 + k], &recs);
    let mut colliders = kit.colliders();
    let mut vehicles: Vec<Vehicle> = recs[..2].iter().map(|&r| vehicle_at(ee, r)).collect();
    let mut others: Vec<Other<'_>> = colliders
        .iter_mut()
        .zip(vehicles.iter_mut())
        .map(|(collider, v)| Other { collider, side: Side::Vehicle(v) })
        .collect();
    contact::resolve(collider, &mut Weighed { body: &mut body, mass: kit.plane.mass }, &mut contacts, &mut others, &[], true);
    drop(others);
    (body, contacts, vehicles)
}

#[test]
fn the_original_tick_from_each_image_is_the_wreck_row() {
    for named in &NAMED {
        let (Some(mut ee), Some(cap)) = (image(named.image), Split::load("splitscreen")) else { return };
        let row = cap.row_at(named.frame);
        assert_eq!(against_row([quantities(&ee, 0), quantities(&ee, 1)], &cap, row - 1), Vec::<String>::new(), "{} is not the row before", named.image);
        assert_eq!([wreck(&ee, 0), wreck(&ee, 1)], [0, 0], "{}: a rider is already down", named.image);
        let o = objects(&ee);
        assert_eq!(o.len(), 4, "{}: the node's objects {o:x?}", named.image);
        for &p in &o[..2] {
            assert!(ee.u32_at(p + 0xf0) == 0 && ee.u32_at(p + 0xf4) != 0, "{}: {p:x} is no plane's object", named.image);
            assert_eq!(ee.u32_at(ee.u32_at(p + 0x1f8) + 0x84), flight::COLLIDABLE, "{}: the plane's collidable's category", named.image);
        }
        let dt = dt(&ee);
        ee.call(NODE_UPDATE, &[u64::from(NODE)], &[dt], BUDGET);
        assert_eq!(against_row([quantities(&ee, 0), quantities(&ee, 1)], &cap, row), Vec::<String>::new(), "{} ticked is not {}", named.image, named.frame);
        assert_eq!([wreck(&ee, 0), wreck(&ee, 1)], named.wreck, "{}: who wrecks", named.image);
    }
}

#[test]
fn the_port_finds_the_originals_contacts_with_the_plane() {
    let Some(kit) = kit() else { return };
    for named in &NAMED {
        let Some(mut ee) = image(named.image) else { return };
        let dt = dt(&ee);
        let recs = records(&ee);
        let seats: Vec<Seat<'_>> =
            recs[..2].iter().map(|&r| Seat { col: Some(&kit.col), at: mat_at(&ee, ee.u32_at(r + 0xf8)), ghost: false, riding: true }).collect();
        let hulls: Vec<Hull<'_>> = (0..2)
            .map(|k| Hull { seat: k, mesh: &kit.hull, at: mat_at(&ee, ee.u32_at(recs[2 + k] + 0xf8)), capacity: kit.plane.record.capacity, category: game_sim::flight::COLLIDABLE })
            .collect();
        let (bikes, mirrors) = rider_contact::gather_met(&seats, &hulls, F(dt), &|_, at| at);

        // the original's first half of the node's update: every object emptied, then every one searches
        let objects = objects(&ee);
        for &o in &objects {
            ee.call(ee.u32_at(ee.u32_at(o + 8) + 0x74), &[u64::from(o)], &[], 50_000_000);
        }
        for &o in &objects {
            ee.call(ee.u32_at(ee.u32_at(o + 8) + 0x7c), &[u64::from(o)], &[dt], 50_000_000);
        }

        // the bikes' lists: only their own entries where the plane's rays struck the bike (N22b)
        for p in 0..2 {
            let orig: Vec<Contact> = list_at(&ee, recs[p], &recs).into_iter().filter(|c| !named.own || c.kind == 0).collect();
            assert_eq!(bikes[p].len(), orig.len(), "{}: p{}'s contacts\nport {:?}\noriginal {orig:?}", named.image, p + 1, bikes[p]);
            for (k, (a, b)) in bikes[p].iter().zip(&orig).enumerate() {
                assert_eq!(differ(a, b), Vec::<&str>::new(), "{}: p{} contact {k}\nport {a:?}\noriginal {b:?}", named.image, p + 1);
            }
        }
        assert!(!bikes[named.striker].is_empty(), "{}: the striker found nothing", named.image);
        assert!(bikes[named.striker].iter().all(|c| c.other == Some(2)), "{}: the striker's entries name plane 0", named.image);
        // and the planes' mirrors, which is all of each plane's list but the plane's own entries
        for k in 0..2 {
            let orig: Vec<Contact> = list_at(&ee, recs[2 + k], &recs).into_iter().filter(|c| c.kind == 1).collect();
            assert_eq!(mirrors[k].len(), orig.len(), "{}: plane {k}'s mirrors", named.image);
            for (j, (a, b)) in mirrors[k].iter().zip(&orig).enumerate() {
                assert_eq!(differ(a, b), Vec::<&str>::new(), "{}: plane {k} mirror {j}\nport {a:?}\noriginal {b:?}", named.image);
            }
        }
        assert!(!mirrors[0].is_empty() && mirrors[1].is_empty(), "{}: only plane 0 is struck", named.image);
    }
}

#[test]
fn the_port_resolves_every_call_with_the_plane_as_the_original() {
    let Some(kit) = kit() else { return };
    let kit = Rc::new(kit);
    for named in NAMED.iter() {
        let Some(mut ee) = image(named.image) else { return };
        let dt = dt(&ee);
        let recs = records(&ee);
        // the planes' collision objects carry their resolve's scratch from call to call
        let planes = Rc::new(RefCell::new([kit.plane.collider; 2]));
        let log = Rc::new(RefCell::new(Vec::<(u32, u32, usize, Vec<String>)>::new()));
        let (seen, kit) = (Rc::clone(&log), Rc::clone(&kit));
        ee.hooks.insert(RESOLVE, Box::new(move |ee: &mut Ee| {
            let (r, first) = (ee.r(4) as u32, ee.r(5) as u32);
            let Some(who) = recs.iter().position(|&x| x == r) else {
                run_original(ee, r, first);
                return;
            };
            let mut bad = vec![];
            if who < 2 {
                let (bump, own) = port_bike(ee, &kit, recs, who, first != 0, &planes.borrow());
                run_original(ee, r, first);
                let got = ridden_at(ee, r);
                for (what, p, o) in [
                    ("velocity", own.chassis.velocity, got.chassis.velocity),
                    ("rates", own.chassis.rates, got.chassis.rates),
                    ("shock", own.chassis.shock, got.chassis.shock),
                    ("twist", own.chassis.twist, got.chassis.twist),
                    ("force", own.force, got.force),
                    ("torque", own.torque, got.torque),
                ] {
                    if bits(p) != bits(o) {
                        bad.push(format!("{what}: port {p:?} original {o:?}"));
                    }
                }
                for (k, f) in bump.planes.iter().enumerate() {
                    let want = body_of(ee, body_at(ee, recs[2 + k]));
                    for ((what, p), (_, o)) in body_fields(&f.body).iter().zip(body_fields(&want)) {
                        if bits(*p) != bits(o) {
                            bad.push(format!("plane {k}'s {what}: port {p:?} original {o:?}"));
                        }
                    }
                }
                planes.borrow_mut().iter_mut().zip(&bump.planes).for_each(|(c, f)| *c = f.collider);
                seen.borrow_mut().push((r, first, bump.contacts.len(), bad));
            } else {
                let k = who - 2;
                let mut collider = planes.borrow()[k];
                let (body, contacts, vehicles) = port_plane(ee, &kit, recs, k, &mut collider);
                run_original(ee, r, first);
                let want = body_of(ee, body_at(ee, r));
                for ((what, p), (_, o)) in body_fields(&body).iter().zip(body_fields(&want)) {
                    if bits(*p) != bits(o) {
                        bad.push(format!("{what}: port {p:?} original {o:?}"));
                    }
                }
                // the riders' chassis, which the plane's own entries on a bike would push (N22b)
                for (i, v) in vehicles.iter().enumerate() {
                    let got = vehicle_at(ee, recs[i]);
                    for (what, p, o) in [("velocity", v.velocity, got.velocity), ("rates", v.rates, got.rates), ("shock", v.shock, got.shock), ("twist", v.twist, got.twist)] {
                        if bits(p) != bits(o) {
                            bad.push(format!("p{}'s {what}: port {p:?} original {o:?}", i + 1));
                        }
                    }
                }
                for (j, (p, o)) in contacts.iter().zip(&list_at(ee, r, &recs)).enumerate() {
                    if bits(p.arm) != bits(o.arm) || bits(p.other_arm) != bits(o.other_arm) {
                        bad.push(format!("contact {j}'s arms"));
                    }
                }
                planes.borrow_mut()[k] = collider;
                seen.borrow_mut().push((r, first, contacts.len(), bad));
            }
        }));
        ee.call(NODE_UPDATE, &[u64::from(NODE)], &[dt], BUDGET);
        let log = log.borrow();
        let order: Vec<(u32, u32)> = log.iter().map(|&(r, f, _, _)| (r, f)).collect();
        // the planes' world first, four substeps of both with impulses on; then the riders, last first
        let mut want: Vec<(u32, u32)> = (0..4).flat_map(|_| [(recs[2], 1), (recs[3], 1)]).collect();
        want.extend([recs[1], recs[0]].iter().flat_map(|&r| [(r, 1), (r, 0)]));
        assert_eq!(order, want, "{}: the resolves' order (object, first substep)", named.image);
        for (r, first, n, bad) in log.iter() {
            let who = recs.iter().position(|x| x == r).unwrap();
            if who == 2 || who == named.striker {
                assert!(*n > 0, "{}: {r:x} resolved an empty list", named.image);
            }
            assert!(bad.is_empty(), "{}: {r:x} substep {first}: {bad:#?}", named.image);
        }
    }
}

#[test]
fn the_tick_with_the_ports_resolve_is_the_wreck_row() {
    let Some(kit) = kit() else { return };
    let kit = Rc::new(kit);
    for named in NAMED.iter() {
        let (Some(mut ee), Some(cap)) = (image(named.image), Split::load("splitscreen")) else { return };
        let mut orig = image(named.image).expect("the image again");
        let dt = dt(&ee);
        let recs = records(&ee);
        let planes = Rc::new(RefCell::new([kit.plane.collider; 2]));
        let stood_in = Rc::new(RefCell::new(0));
        let (count, kit) = (Rc::clone(&stood_in), Rc::clone(&kit));
        ee.hooks.insert(RESOLVE, Box::new(move |ee: &mut Ee| {
            let (r, first) = (ee.r(4) as u32, ee.r(5) as u32);
            let Some(who) = recs.iter().position(|&x| x == r) else {
                run_original(ee, r, first);
                return;
            };
            // the port in the original's place: what it computes, written where the original would
            let (collider, contacts) = if who < 2 {
                let (bump, own) = port_bike(ee, &kit, recs, who, first != 0, &planes.borrow());
                for (i, &rec) in recs[..2].iter().enumerate() {
                    let ch = ee.u32_at(rec + 0xf0);
                    let v = if i == who { &own.chassis } else { &bump.vehicles[i] };
                    put(ee, ch + 0x260, v.velocity);
                    put(ee, ch + 0x290, v.rates);
                    put(ee, ch + 0x170, v.shock);
                    put(ee, ch + 0x180, v.twist);
                }
                let ch = ee.u32_at(r + 0xf0);
                put(ee, ch + 0x130, own.force);
                put(ee, ch + 0x140, own.torque);
                for (k, f) in bump.planes.iter().enumerate() {
                    put_body(ee, body_at(ee, recs[2 + k]), &f.body);
                    planes.borrow_mut()[k] = f.collider;
                }
                (bump.colliders[who], bump.contacts)
            } else {
                let k = who - 2;
                let mut collider = planes.borrow()[k];
                let (body, contacts, vehicles) = port_plane(ee, &kit, recs, k, &mut collider);
                put_body(ee, body_at(ee, r), &body);
                for (&rec, v) in recs[..2].iter().zip(&vehicles) {
                    let ch = ee.u32_at(rec + 0xf0);
                    put(ee, ch + 0x260, v.velocity);
                    put(ee, ch + 0x290, v.rates);
                    put(ee, ch + 0x170, v.shock);
                    put(ee, ch + 0x180, v.twist);
                }
                planes.borrow_mut()[k] = collider;
                (collider, contacts)
            };
            put(ee, r + 0xa0, collider.cofg_world);
            ee.set_f32(r + 0x1d8, collider.normal_speed.0);
            put(ee, r + 0x1e0, collider.relative);
            let at = ee.u32_at(r + 0x108);
            for (k, c) in contacts.iter().enumerate() {
                put(ee, at + 0x80 * k as u32 + 0x50, c.arm);
                put(ee, at + 0x80 * k as u32 + 0x60, c.other_arm);
            }
            *count.borrow_mut() += 1;
        }));
        ee.call(NODE_UPDATE, &[u64::from(NODE)], &[dt], BUDGET);
        orig.call(NODE_UPDATE, &[u64::from(NODE)], &[dt], BUDGET);
        assert_eq!(*stood_in.borrow(), 12, "{}: eight plane calls and four riders'", named.image);
        let row = cap.row_at(named.frame);
        assert_eq!(against_row([quantities(&ee, 0), quantities(&ee, 1)], &cap, row), Vec::<String>::new(), "{} with the port's resolve", named.image);
        assert_eq!([wreck(&ee, 0), wreck(&ee, 1)], named.wreck, "{} with the port's resolve: who wrecks", named.image);
        // and both planes' bodies are the original's own tick's, every field
        for &p in &recs[2..] {
            let (got, want) = (body_of(&ee, body_at(&ee, p)), body_of(&orig, body_at(&orig, p)));
            assert_eq!(format!("{got:?}"), format!("{want:?}"), "{}: the plane {p:x}'s body", named.image);
        }
    }
}


/// `bikef.col` as the plane's world places it on each bike (S95's), out of the split track's archive.
fn bike_frame() -> Option<track::Mesh> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let res = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let bytes = res.read_suffix(flight_search::BIKE_COLLISION).unwrap_or_else(|e| panic!("{e}"));
    Some(track::Mesh::parse(&bytes).expect("bikef.col parses"))
}

/// A bike's frame collidable (its record's `+0x1f8`) and the chassis transform it is placed on.
fn frame_collidable(ee: &Ee, rec: u32) -> (u32, Mat4) {
    let c = ee.u32_at(rec + 0x1f8);
    (c, Mat4([0, 1, 2, 3].map(|r| quad(ee, c + 0x120 + 16 * r))))
}

/// N22b: 5075's plane strikes P2's bike with its own rays. The bike's frame collidable is its
/// collision record's (the collidable's owner, `+0x80`, is the record, category [`flight_search::BIKE`]),
/// so the plane's entries (kind 0) name P2's record and P2's list takes the mirrors (kind 1, naming
/// the plane) ahead of its own entries. The port's plane search ([`Flight::search_alone`]) with the
/// bikes named by player ([`flight::Met::bikes`]) gives the same entries on the plane's list and the
/// same mirrors on P2's, every field; and [`rider_contact::struck_first`] puts them ahead of the
/// bike's own gather as the original's list has them.
#[test]
fn the_planes_own_rays_strike_the_bike_as_the_originals() {
    let (Some(kit), Some(frame)) = (kit(), bike_frame()) else { return };
    let named = &NAMED[1];
    assert!(named.own);
    let Some(mut ee) = image(named.image) else { return };
    let dt = dt(&ee);
    let recs = records(&ee);
    let bikes: Vec<flight_search::Bike> = recs[..2]
        .iter()
        .map(|&r| {
            let (c, transform) = frame_collidable(&ee, r);
            assert_eq!((ee.u32_at(c + 0x84), ee.u32_at(c + 0x80)), (flight_search::BIKE, r), "{r:x}'s frame: category and owner");
            flight_search::Bike { transform, in_world: true }
        })
        .collect();

    // the port: plane 0 from its body in RAM, the two bikes named P1 and P2, the plane named 2
    let mut plane = kit.plane.clone();
    plane.body = body_of(&ee, body_at(&ee, recs[2]));
    let mut met = flight::Met {
        colliders: kit.colliders(),
        vehicles: recs[..2].iter().map(|&r| vehicle_at(&ee, r)).collect(),
        bikes: vec![0, 1],
        named: Some(2),
        ..flight::Met::default()
    };
    let mut ground = game_sim::world::Flat { y: F(-1.0e6), ..Default::default() };
    plane.search_alone(F(dt), &mut ground, &frame, &bikes, Some(&mut met));
    let seats: Vec<Seat<'_>> =
        recs[..2].iter().map(|&r| Seat { col: Some(&kit.col), at: mat_at(&ee, ee.u32_at(r + 0xf8)), ghost: false, riding: true }).collect();
    let hulls: Vec<Hull<'_>> = (0..2)
        .map(|k| Hull { seat: k, mesh: &kit.hull, at: mat_at(&ee, ee.u32_at(recs[2 + k] + 0xf8)), capacity: kit.plane.record.capacity, category: game_sim::flight::COLLIDABLE })
        .collect();
    let (own, _) = rider_contact::gather_met(&seats, &hulls, F(dt), &|_, at| at);

    // the original: every list emptied, then every object's search
    let objects = objects(&ee);
    for &o in &objects {
        ee.call(ee.u32_at(ee.u32_at(o + 8) + 0x74), &[u64::from(o)], &[], 50_000_000);
    }
    for &o in &objects {
        ee.call(ee.u32_at(ee.u32_at(o + 8) + 0x7c), &[u64::from(o)], &[dt], 50_000_000);
    }
    let orig: Vec<Contact> = list_at(&ee, recs[2], &recs).into_iter().filter(|c| c.kind == 0).collect();
    assert!(!orig.is_empty() && orig.iter().all(|c| c.other == Some(1)), "{}: the plane's own entries name P2 {orig:?}", named.image);
    assert_eq!(plane.record.contacts.len(), orig.len(), "the plane's own entries\nport {:?}\noriginal {orig:?}", plane.record.contacts);
    for (k, (a, b)) in plane.record.contacts.iter().zip(&orig).enumerate() {
        assert_eq!(differ(a, b), Vec::<&str>::new(), "plane entry {k}\nport {a:?}\noriginal {b:?}");
    }
    assert_eq!(met.pushed, [false, true], "the bikes the plane's entries name");
    for p in 0..2 {
        let list = rider_contact::struck_first(&met.struck[p..=p], own[p].clone(), Vf::ZERO, &ground);
        let orig = list_at(&ee, recs[p], &recs);
        assert_eq!(list.len(), orig.len(), "p{}'s list\nport {list:?}\noriginal {orig:?}", p + 1);
        for (k, (a, b)) in list.iter().zip(&orig).enumerate() {
            assert_eq!(differ(a, b), Vec::<&str>::new(), "p{} entry {k}\nport {a:?}\noriginal {b:?}", p + 1);
        }
    }
    assert!(met.struck[1].len() == 2 && met.struck[0].is_empty(), "the mirrors {:?}", met.struck);
}
