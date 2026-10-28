//! S169 — a knockable prop as what a contact stands on, the port against the original.
//!
//! The `.toy`'s `HasPhysics` things are two: the cars (S127: a path car until struck, its mesh's
//! mover the car, so a [`game_sim::traffic::Moving`] like any path car's) and the physics
//! collections' props (S80–S198: hay bales, crates, barrels, signs, beach balls, each its own rigid
//! body). The second kind's collidable, the model's `f.col`, names **no mover** (`+0x7c` 0) but an
//! owner (`+0x80`): the prop's collision object, whose body is the prop's. What a wheel standing on
//! a prop reads is therefore the wheel contact-velocity step's owner branch on that object, which
//! [`game_sim::prop_world::Collection::under`] answers off the port's own loaded prop.
//!
//! On the first recording's save state (free09, every prop asleep where it was made):
//!
//! 1. every prop's `f.col` names its collision object and no mover, and the object names the prop's
//!    body and no chassis — the port's prop has a collision object for each;
//! 2. 10,000 wheels standing on a prop the generator picks, its body given a generated place, turn,
//!    velocity and spin in the state and in the port alike: the original's step
//!    (`wheel_contact_velocity`) run in the interpreter, against [`PropSet::under`] and the ride's
//!    wheel arithmetic, bit for bit.
//!
//! Skipped with a line where the disc, the kit or the state is not there.

use std::path::PathBuf;

use game_render::disc::{Disc, Res};
use game_sim::biplane::Area;
use game_sim::chassis::{self, Mat3};
use game_sim::phy::Ini;
use game_sim::prop_set::PropSet;
use game_sim::ride;
use game_sim::track::Scene;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "first_recording.p2m2_SaveState.eeMemory.bin";
const BUDGET: u64 = 50_000_000;

/// The `.toy` actor's collection list (see tests/props.rs).
const LIST: u32 = 0x011b_43f4;

/// free09's terrain: three root cells by three, nine feet a sample (as tests/prop_set.rs has it).
const FREE09: (f32, f32, f32, f32) = (3.0, 3.0, 9.0, 9.0);

/// How many wheels the second test stands on a prop.
const WHEELS: usize = 10_000;

/// The collection's fields.
mod coll {
    pub const NAME: u32 = 0x144;
    pub const BODIES: u32 = 0x320;
}

/// The physics object's.
mod obj {
    pub const BODY: u32 = 0x110;
    pub const COLLISION: u32 = 0x120;
    pub const NODE: u32 = 0x13c;
}

/// The collision object's.
mod rec {
    pub const COFG: u32 = 0x90;
    pub const COFG_WORLD: u32 = 0xa0;
    pub const CHASSIS: u32 = 0xf0;
    pub const BODY: u32 = 0xf4;
    pub const COLLIDABLE: u32 = 0x1f8;
}

/// A collidable's mover (a vehicle) and owner (a collision object).
mod col {
    pub const MOVER: u32 = 0x7c;
    pub const OWNER: u32 = 0x80;
}

/// The body's.
mod body_at {
    pub const POSITION: u32 = 0x60;
    pub const VELOCITY: u32 = 0x70;
    pub const QUAT: u32 = 0x170;
    pub const RATES: u32 = 0x1b0;
}

/// The wheel's, the chassis' and a node's (as tests/traffic.rs's owner-branch test has them).
mod wheel {
    pub const CONTACT: u32 = 0x120;
    pub const LOAD: u32 = 0x140;
    pub const CONTACT_VEL: u32 = 0x170;
    pub const CHASSIS_VEL: u32 = 0x180;
    pub const CHASSIS_LOCAL: u32 = 0x190;
    pub const DONE: u32 = 0x1a0;
    pub const UNDER_VEL: u32 = 0x1b0;
    pub const UNDER_AT: u32 = 0x1c0;
    pub const CHASSIS: u32 = 0x1e4;
    pub const OWNER: u32 = 0x1f0;
    pub const VEHICLE: u32 = 0x1f4;
    pub const SIZE: u32 = 0x220;
    pub const CHASSIS_NODE: u32 = 0x254;
    pub const CHASSIS_COFG: u32 = 0x60;
    pub const CHASSIS_VELOCITY: u32 = 0x260;
    pub const CHASSIS_SPIN: u32 = 0x280;
    pub const CHASSIS_SIZE: u32 = 0x2a0;
    pub const NODE_WORLD: u32 = 0x80;
}

/// Scratch the wheel and its chassis are built in: free in the state (checked).
const WHEEL: u32 = 0x01f8_0000;
const CHASSIS: u32 = 0x01f9_0000;

fn image() -> Option<Ee> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/states").join(STATE);
    let path = if here.exists() { here } else { kit.ctx("states").join(STATE) };
    match Ee::from_ram(&path) {
        Ok(mut ee) => {
            ee.set(28, u64::from(addr("boot_gp").unwrap()));
            Some(ee)
        }
        Err(why) => {
            skip(&format!("{why} — `./kit state ram context/recordings/first_recording.p2m2_SaveState.p2s` writes it"));
            None
        }
    }
}

fn text(ee: &Ee, at: u32) -> String {
    (0..256).map(|k| ee.u8_at(at + k)).take_while(|&b| b != 0).map(|b| b as char).collect()
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|k| ee.u32_at(a + k)))
}

fn put(ee: &mut Ee, a: u32, v: Vf) {
    for (k, x) in v.to_bits().into_iter().enumerate() {
        ee.set_u32(a + 4 * k as u32, x);
    }
}

/// The game's collections, in the order they were made.
fn collections(ee: &Ee) -> Vec<u32> {
    let mut out = Vec::new();
    let mut cell = ee.u32_at(LIST + 4);
    while cell != 0 {
        out.push(ee.u32_at(cell));
        cell = ee.u32_at(cell + 8);
    }
    out
}

/// Collection `c`'s physics object `k`.
fn object(ee: &Ee, c: u32, k: usize) -> u32 {
    ee.u32_at(ee.u32_at(c + coll::BODIES + 8) + 4 * k as u32)
}

/// free09's props as the ride loads them off the disc.
fn free09() -> Option<PropSet> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let name = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("Tracks\\free09.res")).map(|e| e.name.clone())?;
    let res: Res = disc.res(&name).expect("free09.res");
    let toy = Ini::parse_bytes(&res.read_suffix("free09.toy").expect("free09.toy"));
    let scene = Scene::parse(&res.read_suffix("free09.scn").expect("free09.scn")).expect("free09.scn parses");
    let (x, z, sx, sz) = FREE09;
    let (set, said) = PropSet::load(
        &toy,
        "Tracks\\Free09\\",
        &scene.models,
        &mut |p| res.read(p).ok(),
        &mut |p| res.read_suffix(p).ok(),
        Some(Area::new(F(x), F(z), F(sx), F(sz))),
    );
    assert!(said.is_empty(), "free09 loads whole: {said:?}");
    Some(set)
}

/// A small generator, for the cases below.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }
    /// A float in `[-r, r)`.
    fn f(&mut self, r: f32) -> f32 {
        (self.next() as f32 / 4_294_967_296.0 * 2.0 - 1.0) * r
    }
    /// A vector `r` around `c`, with `w` given.
    fn around(&mut self, c: Vf, r: f32, w: f32) -> Vf {
        Vf::new(c.x() + self.f(r), c.y() + self.f(r), c.z() + self.f(r), w)
    }
    /// A unit quaternion.
    fn quat(&mut self) -> Vf {
        let v = [self.f(1.0), self.f(1.0), self.f(1.0), self.f(1.0)];
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
        Vf::new(v[0] / n, v[1] / n, v[2] / n, v[3] / n)
    }
}

/// Every prop's `f.col` names its collision object as its owner and no mover, and the object names
/// the prop's body, no chassis; the port has the object, with the original's centre of mass.
#[test]
fn a_props_collidable_names_its_body_not_a_mover() {
    let Some(ee) = image() else { return };
    let Some(set) = free09() else { return };
    let game = collections(&ee);
    assert_eq!(set.collections.len(), game.len(), "the collection count");
    let mut objects = 0;
    for (port, &c) in set.collections.iter().zip(&game) {
        let name = text(&ee, c + coll::NAME);
        for (k, o) in port.objects.iter().enumerate() {
            let at = |what: &str| format!("{name} object {k}: {what}");
            let go = object(&ee, c, k);
            let r = ee.u32_at(go + obj::COLLISION);
            assert_ne!(r, 0, "{}", at("a collision object"));
            let mesh = ee.u32_at(r + rec::COLLIDABLE);
            assert_ne!(mesh, 0, "{}", at("the f.col"));
            assert_eq!(ee.u32_at(mesh + col::MOVER), 0, "{}", at("the f.col's mover"));
            assert_eq!(ee.u32_at(mesh + col::OWNER), r, "{}", at("the f.col's owner"));
            assert_eq!(ee.u32_at(r + rec::BODY), ee.u32_at(go + obj::BODY), "{}", at("the object's body"));
            assert_eq!(ee.u32_at(r + rec::CHASSIS), 0, "{}", at("the object's chassis"));
            let col = o.collision.as_ref().unwrap_or_else(|| panic!("{}", at("the port's collision object")));
            assert_eq!(col.collider.cofg.to_bits(), quad(&ee, r + rec::COFG).to_bits(), "{}", at("the centre of mass"));
            objects += 1;
        }
    }
    assert!(objects >= 62, "{objects} props");
}

/// A wheel standing on a moving prop reads what the original's wheel reads off it.
#[test]
fn a_wheel_on_a_moving_prop_reads_it_as_the_originals_does() {
    let Some(mut ee) = image() else { return };
    let Some(mut set) = free09() else { return };
    let game = collections(&ee);
    assert_eq!(set.collections.len(), game.len(), "the collection count");
    for a in (WHEEL..CHASSIS + wheel::CHASSIS_SIZE).step_by(4) {
        assert_eq!(ee.u32_at(a), 0, "{a:#x}: the scratch is in use in the state");
    }
    let props: Vec<(usize, usize)> =
        set.collections.iter().enumerate().flat_map(|(c, col)| (0..col.objects.len()).map(move |k| (c, k))).collect();
    let step = addr("wheel_contact_velocity").unwrap();
    let mut rng = Rng(0x5169_5169_5169_5169);
    let mut used = vec![false; props.len()];
    for case in 0..WHEELS {
        let pick = rng.next() as usize % props.len();
        used[pick] = true;
        let (ci, k) = props[pick];
        let go = object(&ee, game[ci], k);
        let (b, r, node) = (ee.u32_at(go + obj::BODY), ee.u32_at(go + obj::COLLISION), ee.u32_at(go + obj::NODE));
        // The prop's body somewhere near where it stands, tumbling.
        let home = quad(&ee, b + body_at::POSITION);
        let position = rng.around(home, 6.0, 1.0);
        let velocity = Vf::new(rng.f(40.0), rng.f(40.0), rng.f(40.0), rng.f(2.0));
        let rates = Vf::new(rng.f(8.0), rng.f(8.0), rng.f(8.0), rng.f(2.0));
        let quat = rng.quat();
        // The wheel on it, its chassis hung on the prop's model node.
        let w = rng.f(2.0);
        let contact_at = rng.around(position, 4.0, w);
        let load = rng.around(position, 6.0, 1.0);
        let (cofg, vel, spin) = (rng.around(position, 6.0, 1.0), rng.around(Vf::ZERO, 60.0, 0.0), rng.around(Vf::ZERO, 6.0, 0.0));

        put(&mut ee, b + body_at::POSITION, position);
        put(&mut ee, b + body_at::VELOCITY, velocity);
        put(&mut ee, b + body_at::QUAT, quat);
        put(&mut ee, b + body_at::RATES, rates);
        for o in (0..wheel::SIZE).step_by(4) {
            ee.set_u32(WHEEL + o, 0);
        }
        for o in (0..wheel::CHASSIS_SIZE).step_by(4) {
            ee.set_u32(CHASSIS + o, 0);
        }
        ee.set_u32(WHEEL + wheel::CHASSIS, CHASSIS);
        ee.set_u32(WHEEL + wheel::OWNER, r);
        ee.set_u32(WHEEL + wheel::VEHICLE, 0);
        put(&mut ee, WHEEL + wheel::CONTACT, contact_at);
        put(&mut ee, WHEEL + wheel::LOAD, load);
        ee.set_u32(CHASSIS + wheel::CHASSIS_NODE, node);
        put(&mut ee, CHASSIS + wheel::CHASSIS_COFG, cofg);
        put(&mut ee, CHASSIS + wheel::CHASSIS_VELOCITY, vel);
        put(&mut ee, CHASSIS + wheel::CHASSIS_SPIN, spin);
        ee.call(step, &[u64::from(WHEEL)], &[], BUDGET);

        let body = &mut set.collections[ci].bones[k].body;
        body.position = position;
        body.velocity = velocity;
        body.quat = quat;
        body.rates = rates;
        let under = set.under(ci, k, contact_at).expect("a prop with a collision object");
        let own = ride::contact_velocity(vel, spin, load, cofg);
        let (contact_vel, chassis_vel) = ride::wheel_contact_velocity(own, vel, Some(under));
        let m = [0, 1, 2].map(|i| quad(&ee, node + wheel::NODE_WORLD + 16 * i));
        let local = chassis::unrotate(&Mat3::new(m[0], m[1], m[2]), chassis_vel);
        let cofg_world = set.collections[ci].objects[k].collision.as_ref().unwrap().collider.cofg_world;

        let name = text(&ee, game[ci] + coll::NAME);
        let what = |field: &str| format!("case {case}: {name} object {k}: {field}");
        let same = |field: &str, got: Vf, at: u32| assert_eq!(got.to_bits(), quad(&ee, at).to_bits(), "{}", what(field));
        same("+0xa0 the object's centre in the world", cofg_world, r + rec::COFG_WORLD);
        same("+0x1b0 its velocity", under.0, WHEEL + wheel::UNDER_VEL);
        same("+0x1c0 its velocity at the contact", under.1, WHEEL + wheel::UNDER_AT);
        same("+0x170 the contact's velocity", contact_vel, WHEEL + wheel::CONTACT_VEL);
        same("+0x180 the chassis' velocity", chassis_vel, WHEEL + wheel::CHASSIS_VEL);
        same("+0x190 the chassis' velocity, its own way", local, WHEEL + wheel::CHASSIS_LOCAL);
        assert_eq!(ee.u32_at(WHEEL + wheel::DONE), 1, "{}", what("+0x1a0"));
    }
    let stood = used.iter().filter(|&&u| u).count();
    assert_eq!(stood, props.len(), "every prop stood on at least once");
}
