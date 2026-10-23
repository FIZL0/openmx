//! S69 — the wreck bike's collision, against the original's own objects and the s62 wrecks.
//!
//! The s62 captures (`research/s62_wreck.py`) carry the wreck bike's body (`bike_a`, from `+0x200`)
//! every tick, and their states the whole object. Two tests:
//!
//! * the collision object the port's loader builds from `Wreckbike.phy` on the disc is the one in the
//!   game's memory, field for field, and so are its ray probe (and the bike model's collision file it
//!   came from) and the terrain object's fields the port derives from the ground;
//! * each wreck's bike body, started from its first state and ticked by [`WreckBike::tick`] over the
//!   disc's free09 ground, is the captured body to the bit, every tick until the wreck ends.
//!
//! The captures, states and disc are game data, git-ignored: absent, the tests skip.

use game_sim::archive::{Psr, Res};
use game_sim::biplane::Body;
use game_sim::contact::{Collider, Node, Response};
use game_sim::contact_search::Heightfield;
use game_sim::phy::Part;
use game_sim::ride::WRECK_INERTIA;
use game_sim::track::{Rays, Scene, Terrain};
use game_sim::world::Ground;
use game_sim::wreck_bike::{self, NoObjects, WreckBike};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::collections::HashMap;
use std::path::PathBuf;

/// The rider body actor in every s62 state, and its riding request's pointer to the wreck bike.
const ACTOR: u32 = 0x017a_7700;
const REQUEST: u32 = 0x488;
const WRECK_BIKE: u32 = 0x1ac;
/// From what the request points at: the body and the collision object.
const BODY: u32 = 0x780;
const COLLISION: u32 = 0x8c0;

/// The five wrecks: the capture, its first state's frame, the last frame the bike moves.
const WRECKS: [(&str, u32, u32); 5] =
    [("front", 2602, 2680), ("side", 2607, 2685), ("back", 2477, 2555), ("tip", 2472, 2550), ("front3", 2630, 2680)];

/// The collision object's fields.
mod rec {
    pub const SEARCH_GROUND: u32 = 0x30;
    pub const SEARCH: u32 = 0x34;
    pub const SWEEP_BACK: u32 = 0x3c;
    pub const TOUCHING: u32 = 0x40;
    pub const TOUCHING_OTHER: u32 = 0x44;
    pub const ON_GROUND: u32 = 0x48;
    pub const ON_OBJECT: u32 = 0x4c;
    pub const DAMP: u32 = 0x50;
    pub const REST_LINEAR: u32 = 0x54;
    pub const DAMP_LINEAR: u32 = 0x58;
    pub const REST_ANGULAR: u32 = 0x5c;
    pub const DAMP_ANGULAR: u32 = 0x60;
    pub const MASS: u32 = 0x64;
    pub const INV_MASS: u32 = 0x68;
    pub const INV_INERTIA: u32 = 0x80;
    pub const COFG: u32 = 0x90;
    pub const COFG_WORLD: u32 = 0xa0;
    pub const OFFSET: u32 = 0xb0;
    pub const LEAD: u32 = 0xc0;
    pub const PUSH_OUT: u32 = 0xc4;
    pub const FACING: u32 = 0xc8;
    pub const SWEPT_FROM: u32 = 0xd0;
    pub const SWEPT_TO: u32 = 0xe0;
    pub const NODE: u32 = 0xf8;
    pub const TERRAIN: u32 = 0xfc;
    pub const CAPACITY: u32 = 0x104;
    pub const NEARBY: u32 = 0x10c;
    pub const ACTORS: u32 = 0x110;
    pub const COLLIDES: u32 = 0x114;
    pub const MAX_DEPTH: u32 = 0x118;
    pub const SCENERY: u32 = 0x124;
    pub const ACTOR: u32 = 0x13c;
    pub const ANGULAR: u32 = 0x148;
    pub const ACTOR_ANGULAR: u32 = 0x14c;
    pub const SPRING_MASK: u32 = 0x150;
    pub const FRICTION_MASK: u32 = 0x160;
    pub const ACTOR_SPRING_MASK: u32 = 0x170;
    pub const ACTOR_FRICTION_MASK: u32 = 0x180;
    pub const FRICTION: u32 = 0x190;
    pub const ACTOR_FRICTION: u32 = 0x194;
    pub const SURFACES: u32 = 0x198;
    pub const NORMAL_SPEED: u32 = 0x1d8;
    pub const RELATIVE: u32 = 0x1e0;
    pub const RAY_PROBE: u32 = 0x1f0;
    pub const SWEEP_PROBE: u32 = 0x1f4;
    pub const COLLIDABLE: u32 = 0x1f8;
    /// The collidable's flags; the ray probe's count and rays.
    pub const FLAGS: u32 = 0x84;
    pub const RAY_COUNT: u32 = 0x168;
    pub const RAYS: u32 = 0x170;
}

/// The rigid body's fields.
mod body_at {
    pub const ENABLED: u32 = 0x00;
    pub const MASS: u32 = 0x0c;
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

/// The terrain object's fields.
mod ter {
    pub const MIN: u32 = 0x30;
    pub const MAX: u32 = 0x40;
    pub const HEIGHT_SCALE: u32 = 0x80;
    pub const SPLIT: u32 = 0x760;
    pub const PER_SAMPLE: u32 = 0x778;
    pub const COARSE: u32 = 0x8a8;
}

// --- the data --------------------------------------------------------------------------------------

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

/// A capture: its header's columns and its rows by frame counter.
struct Capture {
    cols: HashMap<String, usize>,
    rows: HashMap<u32, Vec<String>>,
}

impl Capture {
    fn load(name: &str) -> Option<Capture> {
        let text = std::fs::read_to_string(ctx(&format!("capture/s62/{name}.csv"))?).expect("read the capture");
        let mut lines = text.lines();
        let cols: HashMap<String, usize> =
            lines.next().expect("a header").split(',').enumerate().map(|(i, c)| (c.to_string(), i)).collect();
        let fc = cols["frame_counter"];
        let rows = lines
            .map(|l| l.split(',').map(str::to_string).collect::<Vec<_>>())
            .map(|r| (r[fc].parse().expect("a frame counter"), r))
            .collect();
        Some(Capture { cols, rows })
    }

    fn text(&self, frame: u32, col: &str) -> &str {
        &self.rows[&frame][*self.cols.get(col).unwrap_or_else(|| panic!("no column {col}"))]
    }

    fn get(&self, frame: u32, col: &str) -> f32 {
        let s = self.text(frame, col);
        s.parse::<f64>().unwrap_or_else(|_| panic!("{col} at {frame}: {s:?}")) as f32
    }

    fn bytes(&self, frame: u32, col: &str) -> Vec<u8> {
        let s = self.text(frame, col);
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex")).collect()
    }
}

fn state(name: &str, frame: u32) -> Option<Ee> {
    let path = ctx(&format!("capture/s62/{name}-{frame}.ee.bin"))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    if let Some(spr) = ctx(&format!("capture/s62/{name}-{frame}.spr.bin")) {
        ee.spr = std::fs::read(spr).expect("the scratchpad");
    }
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), frame, "the state is not frame {frame}");
    Some(ee)
}

/// The disc's archive directory and one member's bytes.
struct Disc {
    file: std::fs::File,
    psr: Psr,
}

impl Disc {
    fn open() -> Option<Disc> {
        let at = ctx("iso/MX.PSR")?;
        let mut file = std::fs::File::open(at).expect("MX.PSR");
        let mut front = vec![0u8; 64 << 10];
        std::io::Read::read_exact(&mut file, &mut front).expect("the front of MX.PSR");
        let psr = Psr::parse(&front).expect("MX.PSR's directory");
        Some(Disc { file, psr })
    }

    fn member(&mut self, name: &str) -> Vec<u8> {
        let e = self.psr.find(name).unwrap_or_else(|| panic!("no {name} on the disc"));
        std::io::Seek::seek(&mut self.file, std::io::SeekFrom::Start(e.at)).expect("seek");
        let mut bytes = vec![0u8; e.size as usize];
        std::io::Read::read_exact(&mut self.file, &mut bytes).expect("read");
        bytes
    }
}

/// One file out of a `.res`.
fn inside(res_bytes: &[u8], name: &str) -> Vec<u8> {
    let res = Res::parse(res_bytes).expect("the archive");
    let e = res.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).unwrap_or_else(|| panic!("no {name}"));
    res.read(res_bytes, e).expect("the member")
}

/// The wreck bike's part and its model's collision rays, out of free09's second track archive.
fn wreck_files(disc: &mut Disc) -> (Part, Rays) {
    let res = disc.member("Tracks\\free09_ss_2.res");
    let part = Part::read_bytes(&inside(&res, "bike\\wreckbike.phy"), "PhysicsPart1");
    let rays = Rays::parse(&inside(&res, "bike\\col\\wreckbikem.col")).expect("the ray file");
    (part, rays)
}

/// free09's terrain bytes and the scene's layout of them.
fn free09(disc: &mut Disc) -> (Vec<u8>, Scene) {
    let res_bytes = disc.member("Tracks\\free09.res");
    let res = Res::parse(&res_bytes).expect("free09.res");
    let scene_entry = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(".scn")).expect("a scene");
    let scene = Scene::parse(&res.read(&res_bytes, scene_entry).expect("the scene")).expect("parse the scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let te = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(&leaf)).expect("the terrain");
    (res.read(&res_bytes, te).expect("the terrain bytes"), scene)
}

// --- the objects -----------------------------------------------------------------------------------

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)))
}

fn same(what: &str, got: Vf, want: Vf) {
    for k in 0..4 {
        assert_eq!(got[k].to_bits(), want[k].to_bits(), "{what}[{k}]: the game {want:?}, the port {got:?}");
    }
}

fn same_f(what: &str, got: F, want: F) {
    assert_eq!(got.0.to_bits(), want.0.to_bits(), "{what}: the game {want:?}, the port {got:?}");
}

/// The bike's pointers in a state: its body and its collision object.
fn bike_in(ee: &Ee) -> (u32, u32) {
    let b = ee.u32_at(ee.u32_at(ACTOR + REQUEST) + WRECK_BIKE);
    (b - BODY, b + COLLISION)
}

fn body_of(at: impl Fn(u32) -> f32, enabled: bool) -> Body {
    use body_at::*;
    let q = |a: u32| Vf([0, 4, 8, 12].map(|k| at(a + k)));
    Body {
        enabled,
        inv_mass: F(at(INV_MASS)),
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

fn ee_body(ee: &Ee, b: u32) -> Body {
    body_of(|a| ee.f32_at(b + a), ee.u32_at(b + body_at::ENABLED) != 0)
}

/// A captured body: `bike_a`'s bytes from `+0x200`.
fn captured_body(bytes: &[u8]) -> Body {
    let word = |a: u32| u32::from_le_bytes(bytes[a as usize..a as usize + 4].try_into().unwrap());
    body_of(|a| f32::from_bits(word(a)), word(body_at::ENABLED) != 0)
}

fn bodies_differ(got: &Body, want: &Body) -> Vec<&'static str> {
    let mut out = Vec::new();
    if got.enabled != want.enabled {
        out.push("enabled");
    }
    if got.inv_mass.0.to_bits() != want.inv_mass.0.to_bits() {
        out.push("inv_mass");
    }
    for (name, g, w) in [
        ("damping", got.damping, want.damping),
        ("last_position", got.last_position, want.last_position),
        ("position", got.position, want.position),
        ("velocity", got.velocity, want.velocity),
        ("last_force", got.last_force, want.last_force),
        ("force", got.force, want.force),
        ("accel", got.accel, want.accel),
        ("shock_peak", got.shock_peak, want.shock_peak),
        ("shock", got.shock, want.shock),
        ("step", got.step, want.step),
        ("inv_inertia", got.inv_inertia, want.inv_inertia),
        ("ang_damping", got.ang_damping, want.ang_damping),
        ("last_quat", got.last_quat, want.last_quat),
        ("quat", got.quat, want.quat),
        ("cofg", got.cofg, want.cofg),
        ("moment", got.moment, want.moment),
        ("last_moment", got.last_moment, want.last_moment),
        ("rates", got.rates, want.rates),
        ("ang_accel", got.ang_accel, want.ang_accel),
        ("twist_peak", got.twist_peak, want.twist_peak),
        ("twist", got.twist, want.twist),
    ] {
        if (0..4).any(|k| g[k].to_bits() != w[k].to_bits()) {
            out.push(name);
        }
    }
    out
}

fn response(ee: &Ee, r: u32, at: u32, angular: u32, spring: u32, friction_mask: u32, friction: u32) -> Response {
    Response {
        stiffness: F(ee.f32_at(r + at)),
        damping: F(ee.f32_at(r + at + 4)),
        restitution: F(ee.f32_at(r + at + 8)),
        angular: ee.u32_at(r + angular) != 0,
        friction: F(ee.f32_at(r + friction)),
        spring_mask: quad(ee, r + spring),
        friction_mask: quad(ee, r + friction_mask),
    }
}

fn collider_of(ee: &Ee, r: u32) -> Collider {
    use rec::*;
    let c = ee.u32_at(r + COLLIDABLE);
    Collider {
        damp: ee.u32_at(r + DAMP) != 0,
        damp_linear: F(ee.f32_at(r + DAMP_LINEAR)),
        damp_angular: F(ee.f32_at(r + DAMP_ANGULAR)),
        mass: F(ee.f32_at(r + MASS)),
        inv_mass: F(ee.f32_at(r + INV_MASS)),
        inv_inertia: quad(ee, r + INV_INERTIA),
        cofg: quad(ee, r + COFG),
        cofg_world: quad(ee, r + COFG_WORLD),
        actors: ee.u32_at(r + ACTORS),
        collides_with: ee.u32_at(r + COLLIDES),
        collidable: (c != 0).then(|| ee.u32_at(c + FLAGS)),
        max_depth: F(ee.f32_at(r + MAX_DEPTH)),
        scenery: response(ee, r, SCENERY, ANGULAR, SPRING_MASK, FRICTION_MASK, FRICTION),
        actor: response(ee, r, ACTOR, ACTOR_ANGULAR, ACTOR_SPRING_MASK, ACTOR_FRICTION_MASK, ACTOR_FRICTION),
        surfaces: core::array::from_fn(|k| F(ee.f32_at(r + SURFACES + 4 * k as u32))),
        normal_speed: F(ee.f32_at(r + NORMAL_SPEED)),
        relative: quad(ee, r + RELATIVE),
    }
}

fn node_of(ee: &Ee, n: u32) -> Node {
    Node { rows: [0, 1, 2].map(|k| quad(ee, n + 0x80 + 16 * k)), origin: quad(ee, n + 0xb0) }
}

/// The port's wreck bike as the state holds it: the part's loader for the tuning, the state for
/// everything that moves.
fn bike_from(ee: &Ee, part: &Part) -> WreckBike {
    let (b, c) = bike_in(ee);
    let body = ee_body(ee, b);
    let mut bike = WreckBike::new(part, WRECK_INERTIA, body.position, body.quat);
    bike.body = body;
    bike.collider.cofg_world = quad(ee, c + rec::COFG_WORLD);
    bike.collider.normal_speed = F(ee.f32_at(c + rec::NORMAL_SPEED));
    bike.collider.relative = quad(ee, c + rec::RELATIVE);
    let r = &mut bike.record;
    r.touching = ee.u32_at(c + rec::TOUCHING) != 0;
    r.touching_other = ee.u32_at(c + rec::TOUCHING_OTHER) != 0;
    r.on_ground = ee.u32_at(c + rec::ON_GROUND) != 0;
    r.on_object = ee.u32_at(c + rec::ON_OBJECT) != 0;
    r.swept_from = quad(ee, c + rec::SWEPT_FROM);
    r.swept_to = quad(ee, c + rec::SWEPT_TO);
    r.node = node_of(ee, ee.u32_at(c + rec::NODE));
    bike
}

// --- the tests -------------------------------------------------------------------------------------

/// The part's loader against the game's own objects: the collision object's tuning, the body's, the
/// ray probe and the model file it is read from, and the terrain object's fields.
#[test]
fn the_loader_builds_the_original_collision_object() {
    let (name, frame, _) = WRECKS[0];
    let Some(ee) = state(name, frame) else { return };
    let Some(mut disc) = Disc::open() else { return };
    let (part, col) = wreck_files(&mut disc);
    let (b, c) = bike_in(&ee);
    let body = ee_body(&ee, b);

    // The body: the part's mass, damping and centre of mass, the shipped inertia.
    same_f("the body's mass", part.mass, F(ee.f32_at(b + body_at::MASS)));
    let fresh = WreckBike::new(&part, WRECK_INERTIA, body.position, body.quat);
    for (what, g, w) in [
        ("the body's damping", fresh.body.damping, body.damping),
        ("the body's angular damping", fresh.body.ang_damping, body.ang_damping),
        ("the body's inverse inertia", fresh.body.inv_inertia, body.inv_inertia),
        ("the body's centre of mass", fresh.body.cofg, body.cofg),
    ] {
        same(what, g, w);
    }
    same_f("the body's inverse mass", fresh.body.inv_mass, body.inv_mass);

    // The collision object, every field the loader sets.
    let mut want = collider_of(&ee, c);
    want.cofg_world = fresh.collider.cofg_world;
    want.normal_speed = fresh.collider.normal_speed;
    want.relative = fresh.collider.relative;
    assert_eq!(fresh.collider, want, "the collision object's tuning");
    for (k, (g, w)) in [fresh.collider.scenery.stiffness, fresh.collider.scenery.damping]
        .into_iter()
        .zip([want.scenery.stiffness, want.scenery.damping])
        .enumerate()
    {
        same_f(["stiffness", "damping"][k], g, w);
    }
    same_f("the inverse mass", fresh.collider.inv_mass, want.inv_mass);
    same("the inverse inertia", fresh.collider.inv_inertia, want.inv_inertia);

    // The search side.
    let r = &fresh.record;
    let flag = |at: u32| ee.u32_at(c + at) != 0;
    assert_eq!(
        (r.search_ground, r.search, r.sweep_back, r.damp, r.nearby, r.sweeps),
        (
            flag(rec::SEARCH_GROUND),
            flag(rec::SEARCH),
            flag(rec::SWEEP_BACK),
            flag(rec::DAMP),
            flag(rec::NEARBY),
            flag(rec::SWEEP_PROBE)
        ),
        "the search's switches"
    );
    same_f("rest linear", r.rest_linear, F(ee.f32_at(c + rec::REST_LINEAR)));
    same_f("rest angular", r.rest_angular, F(ee.f32_at(c + rec::REST_ANGULAR)));
    same("the sweep's offset", r.offset, quad(&ee, c + rec::OFFSET));
    same_f("the sweep's lead", r.lead, F(ee.f32_at(c + rec::LEAD)));
    same_f("the push out", r.push_out, F(ee.f32_at(c + rec::PUSH_OUT)));
    same_f("facing", r.facing, F(ee.f32_at(c + rec::FACING)));
    assert_eq!(r.capacity as u32, ee.u32_at(c + rec::CAPACITY), "the contact capacity");

    // The ray probe: the game's, and the model's collision file.
    let probe = ee.u32_at(c + rec::RAY_PROBE);
    let (n, at) = (ee.u32_at(probe + rec::RAY_COUNT), ee.u32_at(probe + rec::RAYS));
    let rays = wreck_bike::rays();
    assert_eq!(rays.len() as u32, n, "the ray count");
    assert_eq!(col.segments.len(), rays.len(), "the collision file's rays");
    for (i, ray) in rays.iter().enumerate() {
        same(&format!("ray {i}'s start"), ray[0], quad(&ee, at + 0x20 * i as u32));
        same(&format!("ray {i}'s end"), ray[1], quad(&ee, at + 0x20 * i as u32 + 0x10));
        let s = &col.segments[i];
        for k in 0..3 {
            assert_eq!(ray[0][k].to_bits(), s.from[k].0.to_bits(), "ray {i}'s start against the file");
            assert_eq!(ray[1][k].to_bits(), s.to[k].0.to_bits(), "ray {i}'s end against the file");
        }
    }

    // The terrain object against the port's reading of the disc's ground.
    let (bytes, scene) = free09(&mut disc);
    let terrain = Terrain::parse(&bytes).expect("free09's terrain");
    let ground = Ground::new(&terrain, &scene.terrain);
    let field = ground.field();
    let t = ee.u32_at(c + rec::TERRAIN);
    same("the terrain's min", field.min(), quad(&ee, t + ter::MIN));
    let (got, want) = (field.max(), quad(&ee, t + ter::MAX));
    for k in [0, 2, 3] {
        assert_eq!(got[k].to_bits(), want[k].to_bits(), "the terrain's max[{k}]: {want:?} / {got:?}");
    }
    same_f("samples a unit", field.per_sample(), F(ee.f32_at(t + ter::PER_SAMPLE)));
    same_f("the height scale", field.height_scale(), F(ee.f32_at(t + ter::HEIGHT_SCALE)));
    assert_eq!(field.coarse(), ee.u32_at(t + ter::COARSE) != 0, "coarse");
    assert_eq!(field.split_one_way(), ee.u32_at(t + ter::SPLIT) == 1, "split one way");
}

/// Each wreck's bike, from its first state, ticked by the port over free09 against the captured
/// body, every tick until the wreck ends.
#[test]
fn the_wreck_bikes_replay_bit_exact() {
    let Some(mut disc) = Disc::open() else { return };
    let (part, _) = wreck_files(&mut disc);
    let (bytes, scene) = free09(&mut disc);
    let terrain = Terrain::parse(&bytes).expect("free09's terrain");
    let ground = Ground::new(&terrain, &scene.terrain);
    let field = ground.field();
    let mut ticks = 0;
    let mut failures = Vec::new();
    for (name, frame, last) in WRECKS {
        let Some(cap) = Capture::load(name) else { return };
        let Some(ee) = state(name, frame) else { return };
        let mut bike = bike_from(&ee, &part);
        assert!(
            bodies_differ(&bike.body, &captured_body(&cap.bytes(frame, "bike_a")[0x200..0x400])).is_empty(),
            "{name}: the state's body is not its row's"
        );
        for f in frame + 1..=last {
            bike.tick(F(cap.get(f, "dt")), Some(&field), &mut NoObjects);
            let want = captured_body(&cap.bytes(f, "bike_a")[0x200..0x400]);
            let differ = bodies_differ(&bike.body, &want);
            if !differ.is_empty() {
                failures.push(format!("{name}: tick {f} differs in {differ:?}"));
                break;
            }
            ticks += 1;
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    assert_eq!(ticks, 78 + 78 + 78 + 78 + 50, "ticks compared");
}

/// The ray probe's tree the trees are searched with (S102): [`wreck_bike::ray_tree`]'s nodes are
/// the game's, node for node and bit for bit, and the model's collision file's; its root is node 0.
#[test]
fn the_ray_probes_tree_is_the_games_and_the_files() {
    const ROOT: u32 = 0x160;
    const NODES: u32 = 0x16c;
    const NODE_SIZE: u32 = 0x24;
    let (name, frame, _) = WRECKS[0];
    let Some(ee) = state(name, frame) else { return };
    let Some(mut disc) = Disc::open() else { return };
    let (_, col) = wreck_files(&mut disc);
    let (_, c) = bike_in(&ee);
    let probe = ee.u32_at(c + rec::RAY_PROBE);
    let tree = wreck_bike::ray_tree();
    assert_eq!(u32::from(tree.root), ee.u32_at(probe + ROOT), "the root");
    assert_eq!(tree.nodes.len(), col.nodes.len(), "the collision file's nodes");
    let nodes = ee.u32_at(probe + NODES);
    let bits3 = |v: [F; 3]| v.map(F::to_bits);
    for (i, n) in tree.nodes.iter().enumerate() {
        let at = nodes + NODE_SIZE * i as u32;
        let words = |a: u32| [0, 4, 8].map(|k| ee.u32_at(a + k));
        let game = (ee.u32_at(at), words(at + 0xc), words(at + 0x18), [ee.u16_at(at + 8), ee.u16_at(at + 10)]);
        let ours = (n.volume.to_bits(), bits3(n.centre), bits3(n.half_extent), n.children);
        assert_eq!(ours, game, "node {i} against the game's");
        let f = &col.nodes[i];
        assert_eq!(ours, (f.volume.to_bits(), bits3(f.centre), bits3(f.half_extent), f.children), "node {i} against the file");
    }
    assert_eq!(tree.segments.len(), wreck_bike::rays().len(), "the probe's rays");
}

/// A wreck bike thrown at a free09 tree (S102): with one of the track's trunks put where its first
/// wreck's bike goes, [`WreckBike::tick_wooded`] takes the trunk's contacts and the bike goes
/// elsewhere; with the tree far away it is the bare tick to the bit.
#[test]
fn a_wreck_bike_strikes_a_free09_trunk() {
    use game_sim::camera::CurveTable;
    use game_sim::eco::{self, Ecosystem, Trees};
    let (name, frame, last) = WRECKS[0];
    let Some(ee) = state(name, frame) else { return };
    let Some(mut disc) = Disc::open() else { return };
    let (part, _) = wreck_files(&mut disc);
    let res_bytes = disc.member("Tracks\\free09.res");
    let res = Res::parse(&res_bytes).expect("free09.res");
    let esb = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with("free09.esb")).expect("free09.esb");
    let esb = Ecosystem::parse(&res.read(&res_bytes, esb).expect("the esb")).expect("free09.esb parses");
    let (bytes, scene) = free09(&mut disc);
    let terrain = Terrain::parse(&bytes).expect("free09's terrain");
    let ground = Ground::new(&terrain, &scene.terrain);
    let field = ground.field();
    let dt = F(game_sim::tick::DeltaTime::CAP_30);
    let ticks = (last - frame) as usize;

    // The bare ride, where its body goes each tick.
    let bare: Vec<Vf> = {
        let mut bike = bike_from(&ee, &part);
        (0..ticks)
            .map(|_| {
                bike.tick(dt, Some(&field), &mut NoObjects);
                bike.body.position
            })
            .collect()
    };
    let cell = eco::cell_size(F(6912.0));
    let tree_at = |at: Vf| {
        let mut eco = esb.clone();
        let mut plant = *esb.plants.iter().find(|p| p.species == 1).expect("a TreeB");
        let ground_y = game_sim::contact_search::ground(&field, at).point.y();
        plant.cell = [at.x(), ground_y, at.z()].map(|v| (v / cell.0) as u16);
        eco.plants = vec![plant];
        Trees::new(&eco, cell, &CurveTable::build()).0
    };
    let wooded = |trees: &Trees| -> Vec<Vf> {
        let mut bike = bike_from(&ee, &part);
        (0..ticks)
            .map(|_| {
                bike.tick_wooded(dt, Some(&field), trees);
                bike.body.position
            })
            .collect()
    };

    let far = tree_at(Vf::new(bare[0].x() + 500.0, 0.0, bare[0].z() + 500.0, 1.0));
    let far_ride = wooded(&far);
    for (t, (a, b)) in bare.iter().zip(&far_ride).enumerate() {
        assert_eq!(a.0.map(f32::to_bits), b.0.map(f32::to_bits), "tick {t}: a tree 700 feet off moved the bike");
    }

    let mid = bare[ticks / 3];
    let near = tree_at(mid);
    let trunk = near.props.props[0].centre;
    let ride = wooded(&near);
    let off = |p: &Vf| ((p.x() - trunk.x()).powi(2) + (p.z() - trunk.z()).powi(2)).sqrt();
    let radius = esb.species[1].as_ref().expect("TreeB").solids[0].radius.0;
    let parted = bare.iter().zip(&ride).position(|(a, b)| a.0.map(f32::to_bits) != b.0.map(f32::to_bits));
    let reaches = bare.iter().position(|p| off(p) < radius).expect("the bare ride goes through the trunk");
    eprintln!("trunk {radius} round at {trunk:?}: the bare ride reaches it at tick {reaches}, the wooded one parts at {parted:?}");
    // Only the rays search groups (the sweep probe's flags keep it off them, as the original's do), so
    // the bike's body is inside the trunk's reach a tick before a ray crosses its wall.
    let parted = parted.expect("the trunk moved the bike");
    assert!(reaches <= parted && parted <= reaches + 2, "the trunk struck at tick {parted}, the bike reached it at {reaches}");
    let (end_bare, end) = (off(&bare[ticks - 1]), off(&ride[ticks - 1]));
    assert!(end < end_bare - 3.0, "the struck bike is thrown back: it ends {end} ft off the trunk, the bare one {end_bare}");
}
