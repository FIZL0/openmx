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
//! S76 adds the scene's props: the wreck bike's probes are the plane's and leave the scenery in, and
//! every wreck — the s62 ones and two into free09's garage (`research/s76_tele.py`) — replays bit-exact
//! through [`WreckBike::tick_among`] over free09's ground and all its collision meshes.
//!
//! The captures, states and disc are game data, git-ignored: absent, the tests skip.

use game_sim::archive::{Psr, Res};
use game_sim::biplane::Body;
use game_sim::contact::{Collider, Node, Response, Vehicle};
use game_sim::contact_search::Heightfield;
use game_sim::flight_search::{self, Bikes};
use game_sim::math::Mat4;
use game_sim::phy::Part;
use game_sim::ride::WRECK_INERTIA;
use game_sim::track::{self, Rays, Scene, Terrain};
use game_sim::world::{Ground, Props};
use game_sim::wreck_bike::{self, NoObjects, Riders, WreckBike};
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
        Capture::read(&format!("capture/s62/{name}.csv"))
    }

    /// A capture by its path under `context/`.
    fn read(rel: &str) -> Option<Capture> {
        let text = std::fs::read_to_string(ctx(rel)?).expect("read the capture");
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
    bike_at(ee, part, b, c)
}

/// [`bike_from`] with the body and the collision object found already.
fn bike_at(ee: &Ee, part: &Part, b: u32, c: u32) -> WreckBike {
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

/// free09's scene props, read as the shell reads them, out of its two track archives.
fn free09_props(disc: &mut Disc) -> Props {
    let by_suffix = |res: &Res, want: &str| {
        let want = want.to_ascii_lowercase();
        res.entries.iter().position(|e| {
            let name = e.name.to_ascii_lowercase();
            name == want || name.ends_with(&format!("\\{want}"))
        })
    };
    let (_, scene) = free09(disc);
    let parts: Vec<(Vec<u8>, Res)> = ["Tracks\\free09.res", "Tracks\\free09_2.res"]
        .into_iter()
        .filter(|n| disc.psr.find(n).is_some())
        .collect::<Vec<_>>()
        .into_iter()
        .map(|n| {
            let bytes = disc.member(n);
            let res = Res::parse(&bytes).expect("a track archive");
            (bytes, res)
        })
        .collect();
    let (props, missing) = Props::load(&scene, |want| {
        parts
            .iter()
            .find_map(|(bytes, res)| by_suffix(res, want).and_then(|e| res.read(bytes, &res.entries[e]).ok()))
            .ok_or_else(|| "no archive of this track has it".to_string())
    });
    // The shell skips it too: its one hierarchy file is not a mesh the port reads.
    assert_eq!(missing, ["UTILITYSHACK1.col: this is a Hierarchy collision file, read as Mesh"], "free09's props the port lacks");
    props
}

/// One wreck's bike, from its first state, ticked by `tick` against the captured body every tick
/// until the wreck ends: the ticks that agree, or the first that does not.
fn replay(part: &Part, (name, frame, last): (&str, u32, u32), mut tick: impl FnMut(&mut WreckBike, F)) -> Option<Result<u32, String>> {
    let cap = Capture::load(name)?;
    let ee = state(name, frame)?;
    let mut bike = bike_from(&ee, part);
    assert!(
        bodies_differ(&bike.body, &captured_body(&cap.bytes(frame, "bike_a")[0x200..0x400])).is_empty(),
        "{name}: the state's body is not its row's"
    );
    for f in frame + 1..=last {
        tick(&mut bike, F(cap.get(f, "dt")));
        let differ = bodies_differ(&bike.body, &captured_body(&cap.bytes(f, "bike_a")[0x200..0x400]));
        if !differ.is_empty() {
            return Some(Err(format!("{name}: tick {f} differs in {differ:?}")));
        }
    }
    Some(Ok(last - frame))
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
    for wreck in WRECKS {
        let Some(run) = replay(&part, wreck, |bike, dt| bike.tick(dt, Some(&field), &mut NoObjects)) else { return };
        match run {
            Ok(n) => ticks += n,
            Err(e) => failures.push(e),
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    assert_eq!(ticks, 78 + 78 + 78 + 78 + 50, "ticks compared");
}

/// S76's wrecks into free09's garage (`research/s76_tele.py` puts recording2's bike in the farmyard,
/// `research/s62_wreck.py` with `research/s76/gas.pad` rides it into the back wall): the capture, its
/// first state's frame, the last frame of the wreck. The first is the bike that struck the back wall
/// at 75 ft/s and bounces off it; the second is the respawned bike driven into the garage's side.
const GARAGE: [(&str, u32, u32); 2] = [("garage", 2882, 2960), ("garage", 3002, 3079)];

/// S76: the wreck bike among the scene's props. Every wreck — the five s62 ones out in the open and
/// the two into free09's garage — ticked by [`WreckBike::tick_among`] over free09's ground and all its
/// collision meshes is the captured body to the bit until the wreck ends; and the garage's first,
/// ticked against the terrain alone, parts from the capture, so the wall is what the replay met.
#[test]
fn the_wreck_bikes_replay_among_free09s_props() {
    let Some(mut disc) = Disc::open() else { return };
    let (part, _) = wreck_files(&mut disc);
    let props = free09_props(&mut disc);
    let (bytes, scene) = free09(&mut disc);
    let terrain = Terrain::parse(&bytes).expect("free09's terrain");
    let ground = Ground::new(&terrain, &scene.terrain);
    let field = ground.field();
    let mut ticks = 0;
    let mut failures = Vec::new();
    // The ticks on which the search left the bike touching an object, wreck by wreck.
    let mut touched = Vec::new();
    for wreck in WRECKS.into_iter().chain(GARAGE) {
        let mut on_object = 0;
        let run = replay(&part, wreck, |bike, dt| {
            bike.tick_among(dt, Some(&field), Some(&props), None);
            on_object += u32::from(bike.record.on_object);
        });
        let Some(run) = run else { return };
        match run {
            Ok(n) => ticks += n,
            Err(e) => failures.push(e),
        }
        touched.push(on_object);
    }
    assert!(failures.is_empty(), "{failures:#?}");
    assert_eq!(ticks, 78 + 78 + 78 + 78 + 50 + 78 + 77, "ticks compared");
    eprintln!("ticks touching a prop, wreck by wreck: {touched:?}");
    assert_eq!(touched[..WRECKS.len()], [0; WRECKS.len()], "an s62 wreck met a prop");
    assert!(touched[WRECKS.len()..].iter().all(|&n| n > 0), "a garage wreck never touched the garage: {touched:?}");

    for wreck in GARAGE {
        let bare = replay(&part, wreck, |bike, dt| bike.tick(dt, Some(&field), &mut NoObjects)).expect("the garage capture");
        let parted = bare.expect_err("a garage wreck replays without the garage");
        eprintln!("against the terrain alone: {parted}");
    }
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

/// S76: the wreck bike's two probes, as every s62 state and the garage's hold them, are the plane's
/// ([`flight_search::PROBE`], every category, the ray probe's flags, no category left out on the ray
/// probe), and what they leave out — four collidables on the ray probe and two on the sweep, by
/// pointer, and on the sweep the categories 0x8 and 0x800 — is never a scenery mesh, so the
/// world's props are searched as the plane searches them.
#[test]
fn the_wreck_bikes_probes_search_the_scenery() {
    use game_sim::flight_search::{PROBE, PROBE_COLLIDES_WITH, SCENERY};
    const CATEGORY: u32 = 0x84;
    const COLLIDES_WITH: u32 = 0xcc;
    const OBJECTS: u32 = 0xd0;
    const CATEGORIES: u32 = 0xe8;
    const FLAGS: u32 = 0x116;
    let mut seen = 0;
    let mut hits = Vec::new();
    for (name, frame, _) in WRECKS.into_iter().chain(GARAGE) {
        let Some(ee) = state(name, frame) else { return };
        let (_, c) = bike_in(&ee);
        let (ray, sweep) = (ee.u32_at(c + rec::RAY_PROBE), ee.u32_at(c + rec::SWEEP_PROBE));
        let list = |o: u32, at: u32| -> (u32, Vec<u32>) {
            let (mode, arr, n) = (ee.u32_at(o + at), ee.u32_at(o + at + 4), ee.u32_at(o + at + 8));
            (mode, (0..n).map(|k| ee.u32_at(arr + 4 * k)).collect())
        };
        for (what, o) in [("ray", ray), ("sweep", sweep)] {
            assert_eq!((ee.u32_at(o + CATEGORY), ee.u32_at(o + COLLIDES_WITH)), (PROBE, PROBE_COLLIDES_WITH), "{name}: the {what} probe's masks");
            // Without the per-search and the hit bits, which the state may have caught raised.
            assert_eq!(ee.u8_at(o + FLAGS) & !0x30, if what == "ray" { 0x02 } else { 0x03 }, "{name}: the {what} probe's flags");
            let (mode, left_out) = list(o, OBJECTS);
            assert_eq!(mode, 1, "{name}: the {what} probe's object list leaves out");
            for x in &left_out {
                assert_ne!(ee.u32_at(x + CATEGORY), SCENERY, "{name}: the {what} probe leaves out a scenery mesh");
            }
            assert_eq!(left_out.len(), if what == "ray" { 4 } else { 2 }, "{name}: what the {what} probe leaves out");
        }
        assert_eq!(list(ray, CATEGORIES), (1, vec![]), "{name}: the ray probe's category list");
        let (mode, cats) = list(sweep, CATEGORIES);
        assert_eq!((mode, &cats[..]), (1, &[0x8, 0x800][..]), "{name}: the sweep probe's category list");
        assert!(cats.iter().all(|&k| k & SCENERY == 0), "{name}: the sweep leaves the scenery out");
        hits.push(ee.u8_at(ray + FLAGS) & 0x20 != 0);
        seen += 1;
    }
    assert_eq!(seen, WRECKS.len() + GARAGE.len(), "states read");
    // The ray probe's hit bit, which only a struck object raises (the terrain is the heightfield
    // rays'), is down in every s62 state and in the garage's first, and up where its second starts:
    // the respawned bike already against the garage's side.
    assert_eq!(hits, [false, false, false, false, false, false, true], "the ray probe's hit bits");
}

/// P186: the crash body's velocity the shell feeds `Vehicle Dust` and `Vehicle Chunks` is
/// [`wreck_bike::WreckBike`]'s `collider.relative`, the relative velocity at the last resolved contact,
/// which `CollisionModel` reads at the collision object's `+0x1e0`. P153's capture of animandpump's
/// wreck (`context/capture/p153/wreck.csv`, journal `wreck-particle-contacts`) has the original's
/// `+0x1e0` on every row, and its state at 3790 (`wreck-3790.p2s`, its RAM image and scratchpad taken
/// out as `wreck-3790.ee.bin`/`.spr.bin`) the whole wreck bike: the port's bike from that state,
/// ticked over free09's ground and props, has the row's relative velocity to the bit on every tick
/// until the respawn.
#[test]
fn the_wreck_bike_feeds_the_dust_the_originals_contact_velocity() {
    /// The state's frame, and the last before the respawn (3918, journal P153).
    const FROM: u32 = 3790;
    const LAST: u32 = 3917;
    let Some(mut disc) = Disc::open() else { return };
    let Some(cap) = Capture::read("capture/p153/wreck.csv") else { return };
    let Some(path) = ctx(&format!("capture/p153/wreck-{FROM}.ee.bin")) else { return };
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    if let Some(spr) = ctx(&format!("capture/p153/wreck-{FROM}.spr.bin")) {
        ee.spr = std::fs::read(spr).expect("the scratchpad");
    }
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), FROM, "the state is not frame {FROM}");
    // The collision object as the capture reached it: the human's rider (the game manager's first
    // actor), its particle manager's Vehicle Dust entry, and the entry's object; and the object's own
    // body (its `+0xf4`).
    let head = ee.u32_at(ee.u32_at(ee.u32_at(addr("game_manager").unwrap()) + 0x34) + 0x34);
    let rider = ee.u32_at(ee.u32_at(head + 0x8));
    let dust = ee.u32_at(ee.u32_at(ee.u32_at(rider + 0x1490) + 0x3c));
    let c = ee.u32_at(ee.u32_at(dust + 0x28));
    let b = ee.u32_at(c + 0xf4);
    let (part, _) = wreck_files(&mut disc);
    same_f("the body's inverse mass", F(ee.f32_at(b + body_at::INV_MASS)), WreckBike::new(&part, WRECK_INERTIA, Vf::default(), Vf::new(0.0, 0.0, 0.0, 1.0)).body.inv_mass);
    let props = free09_props(&mut disc);
    let (bytes, scene) = free09(&mut disc);
    let terrain = Terrain::parse(&bytes).expect("free09's terrain");
    let ground = Ground::new(&terrain, &scene.terrain);
    let field = ground.field();

    let row = |f: u32| Vf([0, 1, 2, 3].map(|k| cap.get(f, &format!("body_vel.{}", ["x", "y", "z", "w"][k]))));
    let mut bike = bike_at(&ee, &part, b, c);
    same(&format!("{FROM}: the state's relative velocity against its row"), bike.collider.relative, row(FROM));
    let mut moved = 0;
    for f in FROM + 1..=LAST {
        let before = bike.collider.relative;
        bike.tick_among(F(cap.get(f, "dt")), Some(&field), Some(&props), None);
        same(&format!("{f}: the relative velocity"), bike.collider.relative, row(f));
        moved += usize::from((0..4).any(|k| before[k].to_bits() != bike.collider.relative[k].to_bits()));
    }
    eprintln!("{} ticks, the relative velocity moved on {moved}", LAST - FROM);
    assert!(moved > 20, "the bike's contact velocity moved on only {moved} ticks");
}

/// P186: the zone byte the rider's dust and chunks read off each of the ragdoll's spheres
/// (`ParticleCollisionConstraint`, sphere `+0x3c`) is the one the sphere's own ground query writes:
/// [`World::wheel_surface`] at the sphere's centre, which `ragdoll_contact::Contact::solve` keeps as
/// `surface`. P153's capture holds every sphere's byte and point on every row; with no scene contact
/// in it, the point is the query's own (its `x` and `z` the bone's centre), so free09's terrain at the
/// point has to answer the byte. A sphere never solved (the ragdoll's seven before the wreck) holds 0
/// and no ground and is not asked.
#[test]
fn the_ragdolls_sphere_zone_is_its_own_ground_querys() {
    use game_sim::world::{Place, World};
    let Some(mut disc) = Disc::open() else { return };
    let Some(cap) = Capture::read("capture/p153/wreck.csv") else { return };
    let (bytes, scene) = free09(&mut disc);
    let terrain = Terrain::parse(&bytes).expect("free09's terrain");
    let mut place = Place { ground: Ground::new(&terrain, &scene.terrain), props: Props::default() };
    let (mut asked, mut touching, mut by_ground_at) = (0, 0, 0);
    let mut frames: Vec<u32> = cap.rows.keys().copied().collect();
    frames.sort();
    for f in frames {
        for s in 0..9 {
            let word = |k: &str| cap.text(f, &format!("r{s}_{k}")).parse::<u32>().expect("a word");
            let (zone, ground) = (word("zone"), word("ground") != 0);
            assert_eq!(word("object"), 0, "{f} sphere {s}: a scene contact moves the point off the query's");
            if zone == 0 && !ground {
                continue;
            }
            let at = Vf([0, 1, 2, 3].map(|k| cap.get(f, &format!("r{s}_at.{}", ["x", "y", "z", "w"][k]))));
            assert_eq!(place.wheel_surface(at), Some(zone as u8), "{f} sphere {s} at {at:?}: the sphere's zone byte");
            asked += 1;
            if ground {
                touching += 1;
                // What the shell fed before P186: the terrain's surface under the point, on a touching
                // sphere only.
                by_ground_at += usize::from(u32::from(place.surface(at)) != zone);
            }
        }
    }
    eprintln!("{asked} sphere rows asked, {touching} touching; the ground's own surface differs on {by_ground_at}");
    assert!(asked > 1500 && touching > 400, "{asked} asked, {touching} touching");
}

// --- S134: a wreck bike into another rider's bike ---------------------------------------------------

/// libpad's pad buffers: port 0's two halves (+0x80 the second, each half's stamp at +0x58), +0x400
/// port 1's.
const PAD_BUFFERS: u32 = 0x0047_a980;
const PAD_COLUMNS: [(&str, &str); 4] = [("pad0_a", "stamp0_a"), ("pad0_b", "stamp0_b"), ("pad1_a", "stamp1_a"), ("pad1_b", "stamp1_b")];
/// `sceSifSetDma`, `sceSifDmaStat` and the data cache's write-back over a range.
const SIF_SET_DMA: u32 = 0x003f_c3a0;
const SIF_DMA_STAT: u32 = 0x003f_c380;
const DCACHE_RANGE: u32 = 0x003f_c8d8;
/// The `lui $r, 0x1000` instructions that form Timer 0's register address, and its stand-in.
const TIMER_READS: [u32; 2] = [0x0016_7dc0, 0x0013_21fc];
const TIMER_COUNT: u32 = 0;
const IOP_RPC_SEND_LEN: u32 = 344;

/// Run the hooked function from inside its hook, the registers and stack put back so the hook returns
/// to its caller as the call would have.
fn through(ee: &mut Ee) {
    let (pc, ra, sp, steps, stack) = (ee.pc, ee.r(31), ee.r(29), ee.steps, ee.stack);
    let args: Vec<u64> = (4..12).map(|k| ee.r(k)).collect();
    ee.stack = (sp as u32 - 0x800) & !0xf;
    ee.call(pc, &args, &[], 2_000_000_000);
    ee.set(31, ra);
    ee.set(29, sp);
    (ee.steps, ee.stack, ee.pc) = (steps, stack, pc);
}

/// Frame `f` of a split capture, whole, as tests/ragdoll_wrecks.rs's S176 runs a 2p one: row `f − 1`'s
/// pads laid in, row `f`'s dt, the device update, the markers and the scene root's simulate.
fn whole_frame(ee: &mut Ee, cap: &Capture, f: u32, clock: &mut f64) {
    let dt = cap.get(f, "dt");
    for (k, (pad, stamp)) in PAD_COLUMNS.into_iter().enumerate() {
        let at = PAD_BUFFERS + [0u32, 0x80, 0x400, 0x480][k];
        for (i, b) in cap.bytes(f - 1, pad).into_iter().enumerate() {
            ee.ram[((at & 0x01ff_ffff) as usize) + i] = b;
        }
        ee.set_u32(at + 0x58, cap.text(f - 1, stamp).parse().expect("a stamp"));
    }
    let stubs = [addr("interrupts_off").unwrap(), addr("interrupts_on").unwrap(), SIF_SET_DMA, SIF_DMA_STAT, DCACHE_RANGE, addr("sceSifCheckStatRpc").unwrap()];
    ee.hooks.insert(stubs[0], Box::new(|ee: &mut Ee| ee.set(2, 1)));
    ee.hooks.insert(stubs[1], Box::new(|_: &mut Ee| {}));
    ee.hooks.insert(SIF_SET_DMA, Box::new(|ee: &mut Ee| ee.set(2, 1)));
    ee.hooks.insert(SIF_DMA_STAT, Box::new(|ee: &mut Ee| ee.set(2, u64::MAX)));
    ee.hooks.insert(DCACHE_RANGE, Box::new(|_: &mut Ee| {}));
    let (check, send) = (addr("sceSifCheckStatRpc").unwrap(), addr("iop_rpc_send").unwrap());
    let send_end = send + IOP_RPC_SEND_LEN;
    ee.hooks.insert(
        check,
        Box::new(move |ee: &mut Ee| {
            let ra = ee.r(31) as u32;
            if (send..send_end).contains(&ra) { ee.set(2, 0) } else { through(ee) }
        }),
    );
    for at in TIMER_READS {
        let word = ee.u32_at(at);
        assert!(word & 0xffe0_ffff == 0x3c00_1000 || word & 0xffe0_ffff == 0x3c00_0000, "{at:#x} is not a `lui $r, 0x1000`: a different build");
        ee.set_u32(at, word & 0xffff_0000);
    }
    *clock += f64::from(dt) * 1000.0;
    let wall = (*clock / 100.0).floor();
    ee.set_u32(addr("wall_ticks").unwrap(), wall as u32);
    ee.set_u32(TIMER_COUNT, ((*clock - wall * 100.0) * 576.0) as u32);
    let app = ee.u32_at(addr("app_pointer").unwrap());
    let virt = |ee: &Ee, obj: u32, table: u32, slot: u32| ee.u32_at(ee.u32_at(obj + table) + slot);
    let dev = ee.u32_at(app + 0x104);
    ee.call(virt(ee, dev, 0x65c, 0x10), &[u64::from(dev), 1], &[dt], 4_000_000_000);
    let marker = virt(ee, app, 0x3c4, 0x80);
    ee.call(marker, &[u64::from(app), 1], &[], 4_000_000_000);
    ee.set_u32(app + 0x32c, 2);
    ee.set_f32(app + 0x344, dt);
    ee.set_f32(app + 0x348, ps2_float::fpu::div(1.0, dt));
    ee.set_u32(app + 0x374, ee.u32_at(app + 0x374) + 1);
    assert_eq!(ee.u32_at(app + 0x374), f, "the image's frame counter is not the frame before {f}");
    let root = ee.u32_at(app + 0x378);
    ee.call(virt(ee, root, 0x8, 0x38), &[u64::from(root)], &[dt], 4_000_000_000);
    assert_ne!(ee.v0(), 0, "frame {f}: the simulation asked to stop");
    ee.set_u32(app + 0x32c, 0);
    ee.call(marker, &[u64::from(app), 2], &[], 4_000_000_000);
    for at in stubs {
        ee.hooks.remove(&at);
    }
}

/// Rider `p`'s ridden vehicle: the game manager's actor list, the riders' vehicles its third and fourth.
fn rider(ee: &Ee, p: u32) -> u32 {
    let mut node = ee.u32_at(ee.u32_at(ee.u32_at(addr("game_manager").unwrap()) + 0x34) + 0x34);
    for _ in 0..2 + p {
        node = ee.u32_at(node + 8);
    }
    ee.u32_at(node)
}

/// S93's short split-screen ride (`splitscreen2_shortcolision.p2m2`): P2 T-bones P1 and wrecks at
/// 5998; its wreck bike tumbles on into P1's bike, its rays on P1's on 5999 to 6001. The frame the
/// port's bike is seeded from, P2's wreck bike's collision object in the image, the last tick
/// compared.
const SPLIT_WRECK: (u32, u32, u32) = (5998, 0x012a_4b30, 6040);

fn matrix(ee: &Ee, at: u32) -> Mat4 {
    Mat4([0, 1, 2, 3].map(|r| quad(ee, at + 16 * r)))
}

/// A rider's chassis as the other side of a contact, through its collision record (`+0xf0` the
/// chassis, `+0xf8` its transform).
fn vehicle_at(ee: &Ee, r: u32) -> Vehicle {
    let v = ee.u32_at(r + 0xf0);
    Vehicle {
        node: node_of(ee, ee.u32_at(r + 0xf8)),
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

/// S134: **a wreck bike into another rider's bike replays bit-exact.** The original runs S93's
/// short ride whole from its image (every rider's chassis the capture's, every frame); P2's wreck
/// bike, seeded from it the frame it wrecks and ticked by [`WreckBike::tick_ridden`] with both
/// riders' bikes handed in, is the original's body to the bit every tick: its entries name P1's
/// record where the original's do, P1's record takes the mirrors the original filed, and P1's chassis
/// is left as it was. Ticked by [`WreckBike::tick_among`], without the bikes, it parts on the first.
#[test]
fn a_wreck_bike_into_a_riders_bike_replays_bit_exact() {
    let Some(path) = ctx("states/s93/s93-short-5997.ram") else { return };
    let Some(cap) = Capture::read("capture/split/splitscreen2.csv") else { return };
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let mut clock = f64::from(ee.u32_at(addr("wall_ticks").unwrap())) * 100.0;
    let Some(mut disc) = Disc::open() else { return };
    let (part, _) = wreck_files(&mut disc);
    let props = free09_props(&mut disc);
    let mesh = track::Mesh::parse(&inside(&disc.member("Tracks\\free09_ss_2.res"), flight_search::BIKE_COLLISION)).expect("bikef.col");
    let (bytes, scene) = free09(&mut disc);
    let terrain = Terrain::parse(&bytes).expect("free09's terrain");
    let ground = Ground::new(&terrain, &scene.terrain);
    let field = ground.field();
    let records = [0, 1].map(|p| ee.u32_at(rider(&ee, p) + 0x7d0));
    whole_frame(&mut ee, &cap, SPLIT_WRECK.0, &mut clock);
    let body = ee.u32_at(SPLIT_WRECK.1 + 0xf4);
    let seed = bike_at(&ee, &part, body, SPLIT_WRECK.1);
    let (mut ridden, mut alone) = (seed.clone(), seed);
    let mut alone_parted = None;
    let (mut struck, mut mirrored) = (0, 0);
    for f in SPLIT_WRECK.0 + 1..=SPLIT_WRECK.2 {
        // The riders as the frame before left them: the wreck bike's world steps first.
        let bikes: Vec<flight_search::Bike> = records
            .map(|r| {
                let c = ee.u32_at(r + rec::COLLIDABLE);
                flight_search::Bike { transform: matrix(&ee, c + 0x120), in_world: ee.u8_at(c + 0x116) & game_sim::world_search::flags::ENABLED != 0 }
            })
            .to_vec();
        let mut chassis = records.map(|r| vehicle_at(&ee, r));
        let mut colliders = records.map(|r| collider_of(&ee, r));
        let before = chassis;
        whole_frame(&mut ee, &cap, f, &mut clock);
        for p in 0..2 {
            let ch = ee.u32_at(rider(&ee, p) + 0x7a4);
            let col = |a: &str| cap.get(f, &format!("p{}_ch_centre.{a}", p + 1)).to_bits();
            assert_eq!([0, 4, 8].map(|k| ee.u32_at(ch + 0x60 + k)), ["x", "y", "z"].map(col), "{f}: the interpreter's rider {p} is not the capture's");
        }
        let dt = F(cap.get(f, "dt"));
        let riders = Riders { bikes: Bikes { mesh: &mesh, bikes: &bikes }, chassis: &mut chassis, colliders: &mut colliders };
        assert!(ridden.struck.struck.is_empty(), "tick {f}: a prop struck the wreck bike; the riders are not first among its others");
        let mirrors = ridden.tick_ridden(dt, Some(&field), Some(&props), None, riders);
        let want = ee_body(&ee, body);
        let differ = bodies_differ(&ridden.body, &want);
        assert!(differ.is_empty(), "tick {f}: the wreck bike among the riders' bikes differs in {differ:?}");
        assert_eq!(chassis, before, "tick {f}: the wreck bike's resolve moved a rider's chassis; the original's does not");
        // Its list names the riders' records where the original's does, and the mirrors are the ones the
        // original filed on them.
        let list = ee.u32_at(SPLIT_WRECK.1 + 0x108);
        let named: Vec<Option<u32>> = (0..ee.u32_at(SPLIT_WRECK.1 + 0x100))
            .map(|k| ee.u32_at(list + 0x80 * k + 0x74))
            .map(|o| (o != 0).then(|| records.iter().position(|&r| r == o).expect("a rider's record") as u32))
            .collect();
        let got: Vec<Option<u32>> = ridden.record.contacts.iter().map(|e| e.other.map(|k| k as u32)).collect();
        assert_eq!(got, named, "tick {f}: the riders its entries name");
        for (p, m) in mirrors.iter().enumerate() {
            let r = records[p];
            let at = ee.u32_at(r + 0x108);
            let theirs: Vec<[[u32; 4]; 2]> = (0..ee.u32_at(r + 0x100))
                .map(|k| at + 0x80 * k)
                .filter(|&e| ee.u32_at(e) == 1 && ee.u32_at(e + 0x74) == SPLIT_WRECK.1)
                .map(|e| [quad(&ee, e + 0x30).to_bits(), quad(&ee, e + 0x40).to_bits()])
                .collect();
            let ours: Vec<[[u32; 4]; 2]> = m.iter().map(|e| [e.normal.to_bits(), e.point.to_bits()]).collect();
            assert_eq!(ours, theirs, "tick {f}: the mirrors (normal, point) on rider {p}'s record");
            mirrored += m.len();
        }
        struck += usize::from(got.iter().any(Option::is_some));
        alone.tick_among(dt, Some(&field), Some(&props), None);
        if alone_parted.is_none() && !bodies_differ(&alone.body, &want).is_empty() {
            alone_parted = Some(f);
        }
    }
    eprintln!("ticks striking a rider's bike: {struck}, mirrors filed: {mirrored}; without the bikes it parts at {alone_parted:?}");
    assert!(struck >= 3 && mirrored >= 3, "the wreck bike never met P1's bike: {struck} ticks, {mirrored} mirrors");
    assert_eq!(alone_parted, Some(SPLIT_WRECK.0 + 1), "the wreck bike replays without the riders' bikes");
}

/// The side wreck's state's collision node, whose update (`0x0026_f800`) is the wreck bike's tick
/// (its objects: one more, an AI rider's record, the wreck bike's, two more riders'), and the race
/// gate whose sphere the wreck bike lies 15 ft off.
const SIDE_NODE: u32 = 0x00e1_7ef0;
const SIDE_GATE: u32 = 0x016a_f9c0;

/// The race gate's collision file, out of the first of free09's archives that holds it.
fn race_gate_file(disc: &mut Disc) -> track::Hierarchy {
    for n in ["Tracks\\free09.res", "Tracks\\free09_2.res", "Tracks\\free09_ss_2.res"] {
        if disc.psr.find(n).is_none() {
            continue;
        }
        let bytes = disc.member(n);
        let res = Res::parse(&bytes).expect("a track archive");
        if let Some(e) = res.find("col\\machineracegate.col") {
            return track::Hierarchy::parse(&res.read(&bytes, e).expect("the member")).expect("machineracegate.col");
        }
    }
    panic!("no free09 archive holds machineracegate.col")
}

/// A gate collidable's parts as the image stands them (`+0x200` count, `+0x204` switches, `+0x214`
/// list; each part's owner at `+0x78`, its transform at `+0x120`), their meshes the file's in file
/// order. A gate has no skeleton, so its parts are bound to no bone.
fn gate_parts<'m>(ee: &Ee, g: u32, file: &'m track::Hierarchy) -> Vec<flight_search::GatePart<'m>> {
    let (n, on, list) = (ee.u32_at(g + 0x200), ee.u32_at(g + 0x204), ee.u32_at(g + 0x214));
    assert_eq!(n as usize, file.parts.len(), "{g:#x}: the gate's parts against the file's");
    (0..n)
        .map(|j| {
            let p = ee.u32_at(list + 4 * j);
            let part = &file.parts[j as usize];
            assert_eq!(ee.u32_at(p + 0x78), g, "{g:#x}: part {j}'s owner");
            flight_search::GatePart { mesh: &part.mesh, transform: matrix(ee, p + 0x120), enabled: ee.u32_at(on + 4 * j) != 0 }
        })
        .collect()
}

/// A gate collidable as the image holds it: its sphere, its box in its frame, its `+0x31` bit.
fn gate_at<'a>(ee: &Ee, g: u32, parts: &'a [flight_search::GatePart<'a>]) -> flight_search::Gate<'a> {
    flight_search::Gate {
        centre: quad(ee, g + 0x40),
        radius: F(ee.f32_at(g + 0x60)),
        box_centre: quad(ee, g + 0x120),
        half: quad(ee, g + 0x130),
        frame: matrix(ee, g + 0x1a0),
        parts,
        nearby: ee.u8_at(g + 0x31) & 1 != 0,
    }
}

/// S503: **the wreck bike among the toys, the riders' bikes and the race gates replays bit-exact.**
/// The side wreck's state, its bike thrown at 60 ft/s at the gate beside it, is run by the original's
/// node update; the port's bike, seeded from it and ticked by [`WreckBike::tick_gated`] with free09's
/// props, the three AI riders' bikes and the twelve gates of its nearby list all handed in, is the
/// original's body to the bit every tick to the wreck's end. As the original holds them, the gates'
/// nearby bit is clear and the bike flies through; with the bit set on both sides the bike strikes the
/// gate, still bit-exact, and parts from the first ride.
#[test]
fn a_wreck_bike_meeting_a_gate_replays_bit_exact() {
    let (name, frame, last) = WRECKS[1];
    let Some(cap) = Capture::load(name) else { return };
    let Some(mut disc) = Disc::open() else { return };
    let (part, _) = wreck_files(&mut disc);
    let props = free09_props(&mut disc);
    let mesh = track::Mesh::parse(&inside(&disc.member("Tracks\\free09_ss_2.res"), flight_search::BIKE_COLLISION)).expect("bikef.col");
    let file = race_gate_file(&mut disc);
    let (bytes, scene) = free09(&mut disc);
    let terrain = Terrain::parse(&bytes).expect("free09's terrain");
    let ground = Ground::new(&terrain, &scene.terrain);
    let field = ground.field();
    let mut rides = Vec::new();
    for nearby in [false, true] {
        let Some(mut ee) = state(name, frame) else { return };
        let (b, c) = bike_in(&ee);
        let objects = ee.u32_at(SIDE_NODE + 0x38);
        assert_eq!(ee.u32_at(objects + 8), c, "the node's third object is the wreck bike's");
        // Thrown at the gate's near post: its box's centre, out along the frame's first row to a foot
        // inside the box's side.
        let (at, centre) = (quad(&ee, b + body_at::POSITION), quad(&ee, SIDE_GATE + 0x140));
        let (side, reach) = (quad(&ee, SIDE_GATE + 0x1a0), ee.f32_at(SIDE_GATE + 0x130) - 1.0);
        let sign = if (0..3).map(|k| (at[k] - centre[k]) * side[k]).sum::<f32>() < 0.0 { -1.0 } else { 1.0 };
        let to = [0, 1, 2].map(|k| centre[k] + sign * reach * side[k] - at[k]);
        let len = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
        for k in 0..3 {
            ee.set_f32(b + body_at::VELOCITY + 4 * k as u32, to[k] / len * 60.0);
        }
        if nearby {
            ee.set_u32(SIDE_GATE + 0x30, ee.u32_at(SIDE_GATE + 0x30) | 0x100);
        }
        let search = ee.u32_at(c + rec::NEARBY);
        let listed: Vec<u32> = (0..ee.u32_at(search + 4)).map(|k| ee.u32_at(ee.u32_at(search) + 8 * k)).collect();
        let gate_ids: Vec<u32> = listed.iter().copied().filter(|&o| ee.u32_at(o + 0x84) == flight_search::GATE).collect();
        assert!(gate_ids.contains(&SIDE_GATE) && gate_ids.len() == 12, "the list's gates: {gate_ids:x?}");
        let node: Vec<u32> = (0..ee.u32_at(SIDE_NODE + 0x30)).map(|k| ee.u32_at(objects + 4 * k)).collect();
        let records: Vec<u32> = listed
            .iter()
            .filter(|&&o| ee.u32_at(o + 0x84) == 0x0009_a048)
            .map(|&o| *node.iter().find(|&&r| ee.u32_at(r + rec::COLLIDABLE) == o).expect("each listed bike's record is in the node"))
            .collect();
        assert_eq!(records.len(), 3, "the AI riders' bikes in the list");
        let parts: Vec<Vec<flight_search::GatePart>> = gate_ids.iter().map(|&g| gate_parts(&ee, g, &file)).collect();
        let gates: Vec<flight_search::Gate> = gate_ids.iter().zip(&parts).map(|(&g, p)| gate_at(&ee, g, p)).collect();
        assert_eq!(gates.iter().filter(|g| g.nearby).count(), usize::from(nearby), "the gates' nearby bits");
        let mut bike = bike_at(&ee, &part, b, c);
        let mut ride = Vec::new();
        let mut on_object = 0;
        for f in frame + 1..=last {
            let bikes: Vec<flight_search::Bike> = records
                .iter()
                .map(|&r| {
                    let c = ee.u32_at(r + rec::COLLIDABLE);
                    flight_search::Bike { transform: matrix(&ee, c + 0x120), in_world: ee.u8_at(c + 0x116) & game_sim::world_search::flags::ENABLED != 0 }
                })
                .collect();
            let mut chassis: Vec<Vehicle> = records.iter().map(|&r| vehicle_at(&ee, r)).collect();
            let mut colliders: Vec<Collider> = records.iter().map(|&r| collider_of(&ee, r)).collect();
            let before = chassis.clone();
            let dt = cap.get(f, "dt");
            ee.call(0x0026_f800, &[u64::from(SIDE_NODE)], &[dt], 2_000_000_000);
            let riders = Riders { bikes: Bikes { mesh: &mesh, bikes: &bikes }, chassis: &mut chassis, colliders: &mut colliders };
            bike.tick_gated(F(dt), Some(&field), Some(&props), None, riders, &gates);
            let differ = bodies_differ(&bike.body, &ee_body(&ee, b));
            assert!(differ.is_empty(), "nearby {nearby}, tick {f}: the wreck bike among the toys, bikes and gates differs in {differ:?}");
            assert_eq!(chassis, before, "tick {f}: the wreck bike's resolve moved a rider's chassis");
            on_object += u32::from(bike.record.on_object);
            ride.push(bike.body.position.0.map(f32::to_bits));
        }
        eprintln!("gates' nearby bit {nearby}: ticks on an object {on_object}");
        rides.push((ride, on_object));
    }
    let parted = rides[0].0.iter().zip(&rides[1].0).position(|(a, b)| a != b);
    eprintln!("with the gate's bit set the ride parts at tick {:?}", parted.map(|t| frame + 1 + t as u32));
    assert_eq!(rides[0].1, 0, "the bike touched an object with every gate's bit clear");
    assert!(rides[1].1 > 0 && parted.is_some(), "the thrown bike never met the gate");
}
