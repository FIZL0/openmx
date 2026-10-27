//! S48: the biplane's collision object against the original's, in C7's save state of the plane in
//! flight — its tuning, its search's record and transform, the ray probe out of `biplanem.col`, the
//! plane's own collision mesh as the loader packs it — and the plane meeting the world with it.

use game_sim::archive::{Psr, Res};
use game_sim::biplane::{Body, CRASHED};
use game_sim::contact::{Collider, Node, Response};
use game_sim::flight::Flight;
use game_sim::flight_search::{self, PROBE, Rays, VEHICLE, WAKE_MASK, pack_normal};
use game_sim::phy::{self, Plane};
use game_sim::track;
use game_sim::world::Flat;
use game_sim::wreck_bike::node_of;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

/// The save state beside C7's recording, and the plane in it (`tests/flight.rs`'s).
const STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";
const PLANE: u32 = 0x00c4_4a40;
/// The plane's body and its collision object.
const BODY: u32 = 0x160;
const COLLISION: u32 = 0x170;
/// The plane's crash mask (the categories the speed rule spares).
const CRASH_MASK: u32 = 0x580;
/// The game's global mask the ray probe's hit listener tests.
const GLOBAL_MASK: u32 = 0x0046_8f68;

/// The collision object's fields (`tests/wreck_bike.rs`'s, the same class).
mod rec {
    pub const SEARCH_GROUND: u32 = 0x30;
    pub const SEARCH: u32 = 0x34;
    pub const SWEEP_BACK: u32 = 0x3c;
    pub const DAMP: u32 = 0x50;
    pub const REST_LINEAR: u32 = 0x54;
    pub const DAMP_LINEAR: u32 = 0x58;
    pub const REST_ANGULAR: u32 = 0x5c;
    pub const DAMP_ANGULAR: u32 = 0x60;
    pub const MASS: u32 = 0x64;
    pub const INV_MASS: u32 = 0x68;
    pub const INV_INERTIA: u32 = 0x80;
    pub const COFG: u32 = 0x90;
    pub const OFFSET: u32 = 0xb0;
    pub const LEAD: u32 = 0xc0;
    pub const PUSH_OUT: u32 = 0xc4;
    pub const FACING: u32 = 0xc8;
    pub const NODE: u32 = 0xf8;
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
    pub const RAY_PROBE: u32 = 0x1f0;
    pub const SWEEP_PROBE: u32 = 0x1f4;
    pub const COLLIDABLE: u32 = 0x1f8;
}

/// A collidable's fields, and the ray probe's and the mesh's own.
mod obj {
    pub const CENTRE: u32 = 0x40;
    pub const RADIUS: u32 = 0x60;
    pub const KIND: u32 = 0x68;
    pub const CATEGORY: u32 = 0x84;
    pub const FLAGS: u32 = 0x116;
    pub const COLLIDES: u32 = 0xcc;
    pub const TRANSFORM: u32 = 0x120;
    pub const ROOT: u32 = 0x160;
    pub const RAY_COUNT: u32 = 0x168;
    pub const RAYS: u32 = 0x170;
    pub const MESH_FACETS: u32 = 0x238;
    pub const MESH_FACET_COUNT: u32 = 0x23e;
}

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/states").join(STATE);
    let path = if here.exists() { here } else { kit.ctx("states").join(STATE) };
    match Ee::from_ram(&path) {
        Ok(ee) => Some(ee),
        Err(why) => {
            skip(&format!("{why} — `./kit state ram context/recordings/biplane.p2m2_SaveState.p2s` writes it"));
            None
        }
    }
}

/// `machine.res` out of the disc's `MX.PSR`.
fn machines() -> Option<(Res, Vec<u8>)> {
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
    Some((Res::parse(&bytes).expect("machine.res"), bytes))
}

fn member(res: &Res, bytes: &[u8], name: &str) -> Vec<u8> {
    let e = res.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).unwrap_or_else(|| panic!("machine.res has {name}"));
    res.read(bytes, e).expect("its bytes")
}

fn quad(ee: &Ee, at: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(at + k)))
}

fn same(what: &str, got: Vf, want: Vf) {
    for k in 0..4 {
        assert_eq!(got[k].to_bits(), want[k].to_bits(), "{what}[{k}]: the game {want:?}, the port {got:?}");
    }
}

fn same_f(what: &str, got: F, want: F) {
    assert_eq!(got.0.to_bits(), want.0.to_bits(), "{what}: the game {want:?}, the port {got:?}");
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

/// Every field of the collision object the loader sets (the three a resolve writes are the port's).
fn collider_of(ee: &Ee, r: u32, port: &Collider) -> Collider {
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
        cofg_world: port.cofg_world,
        actors: ee.u32_at(r + ACTORS),
        collides_with: ee.u32_at(r + COLLIDES),
        collidable: (c != 0).then(|| ee.u32_at(c + obj::CATEGORY)),
        max_depth: F(ee.f32_at(r + MAX_DEPTH)),
        scenery: response(ee, r, SCENERY, ANGULAR, SPRING_MASK, FRICTION_MASK, FRICTION),
        actor: response(ee, r, ACTOR, ACTOR_ANGULAR, ACTOR_SPRING_MASK, ACTOR_FRICTION_MASK, ACTOR_FRICTION),
        surfaces: core::array::from_fn(|k| F(ee.f32_at(r + SURFACES + 4 * k as u32))),
        normal_speed: port.normal_speed,
        relative: port.relative,
    }
}

/// The body the post listener placed the transform from: its pose fields as the state holds them.
fn body_of(ee: &Ee, b: u32) -> Body {
    Body {
        enabled: ee.u32_at(b) != 0,
        last_position: quad(ee, b + 0x50),
        position: quad(ee, b + 0x60),
        last_quat: quad(ee, b + 0x150),
        quat: quad(ee, b + 0x170),
        cofg: quad(ee, b + 0x180),
        ..Body::default()
    }
}

fn the_plane() -> Option<(Ee, Flight, Res, Vec<u8>)> {
    let ee = image()?;
    let (res, bytes) = machines()?;
    let plane = Plane::read_bytes(&member(&res, &bytes, phy::BIPLANE_PHY));
    let rays = track::Rays::parse(&member(&res, &bytes, flight_search::BIPLANE_RAYS)).expect("biplanem.col");
    let f = Flight::new(&plane).with_rays(Rays::of(&rays));
    Some((ee, f, res, bytes))
}

/// **The collision object is built as the original builds it**: every field of its tuning off
/// `biplane.phy`'s mass properties and the body's inertia, its search's switches, the sweep's
/// tuning, the contact capacity (one a ray), and its transform where the body is.
#[test]
fn the_planes_collision_object_is_the_originals() {
    let Some((ee, f, _, _)) = the_plane() else { return };
    let r = ee.u32_at(PLANE + COLLISION);
    let want = collider_of(&ee, r, &f.collider);
    assert_eq!(f.collider, want, "the collision object's tuning");
    for (what, g, w) in [
        ("the mass", f.collider.mass, want.mass),
        ("the inverse mass", f.collider.inv_mass, want.inv_mass),
        ("the stiffness", f.collider.scenery.stiffness, want.scenery.stiffness),
        ("the damping", f.collider.scenery.damping, want.scenery.damping),
    ] {
        same_f(what, g, w);
    }
    same("the inverse inertia", f.collider.inv_inertia, want.inv_inertia);

    let s = &f.record;
    let flag = |at: u32| ee.u32_at(r + at) != 0;
    assert_eq!(
        (s.search_ground, s.search, s.sweep_back, s.damp, s.nearby, s.sweeps),
        (flag(rec::SEARCH_GROUND), flag(rec::SEARCH), flag(rec::SWEEP_BACK), flag(rec::DAMP), flag(rec::NEARBY), flag(rec::SWEEP_PROBE)),
        "the search's switches"
    );
    same_f("rest linear", s.rest_linear, F(ee.f32_at(r + rec::REST_LINEAR)));
    same_f("rest angular", s.rest_angular, F(ee.f32_at(r + rec::REST_ANGULAR)));
    same("the sweep's offset", s.offset, quad(&ee, r + rec::OFFSET));
    same_f("the sweep's lead", s.lead, F(ee.f32_at(r + rec::LEAD)));
    same_f("the push out", s.push_out, F(ee.f32_at(r + rec::PUSH_OUT)));
    same_f("facing", s.facing, F(ee.f32_at(r + rec::FACING)));
    assert_eq!(s.capacity as u32, ee.u32_at(r + rec::CAPACITY), "the contact capacity");

    // The transform the post listener left: where the body's pose is.
    let n = ee.u32_at(r + rec::NODE);
    let node = Node { rows: [0, 1, 2].map(|k| quad(&ee, n + 0x80 + 16 * k)), origin: quad(&ee, n + 0xb0) };
    let port = node_of(&body_of(&ee, ee.u32_at(PLANE + BODY)));
    for k in 0..3 {
        same(&format!("the transform's row {k}"), port.rows[k], node.rows[k]);
    }
    same("the transform's origin", port.origin, node.origin);
}

/// **The ray probe is `biplanem.col`'s**: its 36 segments in its order, `w` 1, its tree's root;
/// the probe placed by the collision object's transform; its reach the root box's half diagonal
/// about the box's centre; its categories and flags the port's constants.
#[test]
fn the_ray_probe_is_the_col_files() {
    let Some((ee, f, _, _)) = the_plane() else { return };
    let r = ee.u32_at(PLANE + COLLISION);
    let probe = ee.u32_at(r + rec::RAY_PROBE);
    let rays = f.rays.as_deref().expect("rays");
    let (n, at) = (ee.u32_at(probe + obj::RAY_COUNT), ee.u32_at(probe + obj::RAYS));
    assert_eq!(rays.segments.len() as u32, n, "the ray count");
    for (i, s) in rays.segments.iter().enumerate() {
        same(&format!("ray {i}'s start"), s.from, quad(&ee, at + 0x20 * i as u32));
        same(&format!("ray {i}'s end"), s.to, quad(&ee, at + 0x20 * i as u32 + 0x10));
    }
    assert_eq!(rays.pairs(), f.record.rays.clone().unwrap(), "the record's rays are the probe's");
    assert_eq!(u32::from(rays.root), u32::from(ee.u16_at(probe + obj::ROOT)), "the tree's root");

    let t = [0, 1, 2, 3].map(|k| quad(&ee, probe + obj::TRANSFORM + 16 * k));
    let n = ee.u32_at(r + rec::NODE);
    for k in 0..4 {
        same(&format!("the probe's frame row {k}"), t[k], quad(&ee, n + 0x80 + 16 * k as u32));
    }
    let (centre, radius) = flight_search::reach(rays, &t);
    same_f("the probe's reach", radius, F(ee.f32_at(probe + obj::RADIUS)));
    let c = quad(&ee, probe + obj::CENTRE);
    for k in 0..3 {
        assert_eq!(centre[k].to_bits(), c[k].to_bits(), "the probe's centre[{k}]: the game {c:?}, the port {centre:?}");
    }
    assert_eq!(ee.u32_at(probe + obj::CATEGORY), PROBE, "the probe's category");
    assert_eq!(ee.u32_at(probe + obj::COLLIDES), flight_search::PROBE_COLLIDES_WITH, "what the probe collides with");
    assert_eq!(ee.u8_at(probe + obj::FLAGS) & !0x30, 0x12 & !0x30, "the probe's flags (less searched and hit)");
}

/// **A collision mesh's face normals are packed as the loader packs them**: the plane's own mesh
/// (`biplanef.col`, the collidable the other probes strike) facet for facet against the state —
/// vertex indices and the three shorts, including the −1 where a lane is a hair under 0.
#[test]
fn face_normals_pack_as_the_loader_packs_them() {
    let Some((ee, _, res, bytes)) = the_plane() else { return };
    let mesh = track::Mesh::parse(&member(&res, &bytes, "Biplane\\col\\biplanef.col")).expect("biplanef.col");
    let c = ee.u32_at(ee.u32_at(PLANE + COLLISION) + rec::COLLIDABLE);
    assert_eq!(ee.u32_at(c + obj::KIND), 0, "the plane's collidable is a mesh");
    let (at, n) = (ee.u32_at(c + obj::MESH_FACETS), u32::from(ee.u16_at(c + obj::MESH_FACET_COUNT)));
    let facets = flight_search::facets(&mesh);
    assert_eq!(facets.len() as u32, n, "the facet count");
    for (i, f) in facets.iter().enumerate() {
        let a = at + 0x14 * i as u32;
        let v = [0, 2, 4].map(|k| ee.u16_at(a + k));
        let normal = [0xc, 0xe, 0x10].map(|k| ee.u16_at(a + k) as i16);
        assert_eq!(f.vertices, v, "facet {i}'s vertices");
        assert_eq!(f.normal, normal, "facet {i}'s normal, from {:?}", mesh.triangles[i].normal);
    }
    assert_eq!(pack_normal([F(1.0), F(0.0), F(-1.0)]), [16384, 0, -16384]);
}

/// **The masks the listeners and the crash test read**: the hit listener's global mask, and the
/// plane's crash mask, which spares vehicles — a scenery mesh is in neither (S96: the meshes S48 read
/// as scenery, in the first, are the track's toys).
#[test]
fn the_masks_are_the_originals() {
    let Some(ee) = image() else { return };
    assert_eq!(ee.u32_at(GLOBAL_MASK), WAKE_MASK, "the global mask");
    assert_eq!(ee.u32_at(PLANE + CRASH_MASK), VEHICLE, "the plane's crash mask");
    assert_eq!(flight_search::SCENERY & WAKE_MASK, 0);
    assert_eq!(flight_search::SCENERY & VEHICLE, 0);
}

/// **The plane meets the ground with its collision object**: flown nose-down into flat ground
/// fast, its rays reach the ground, the contacts push it back out, and the crash test wrecks it —
/// which, before S48, it never did: it flew on through.
#[test]
fn a_plane_flown_into_the_ground_crashes() {
    let Some((_, f, _, _)) = the_plane() else { return };
    let tables = game_sim::ride::Tables::build();
    let mut f = f;
    let mut ground = Flat::default();
    f.attitude.flown = true;
    f.place(Vf::new(0.0, 30.0, 0.0, 1.0), Vf::new(0.0, -1.0, 1.0, 0.0), &tables.acos);
    f.body.velocity = Vf::new(0.0, -80.0, 80.0, 1.0);
    let dt = F(1.0 / 30.0);
    let pad = game_sim::biplane::Pad::default();
    let mut touched = None;
    for tick in 0..60 {
        f.frame(dt, pad, &mut ground, &tables);
        if f.record.touching && touched.is_none() {
            touched = Some(tick);
        }
        if f.crash.reason == CRASHED {
            break;
        }
        assert!(f.body.position.y() > -20.0, "tick {tick}: the plane went through the ground: {:?}", f.body.position);
    }
    assert!(touched.is_some(), "the collision object never touched the ground");
    assert_eq!(f.crash.reason, CRASHED, "the plane did not crash");
}

/// **The plane's crash is heard** (P164): what the shell's wreck clank reads — the wreck word, the
/// collision record touching and the body's shock peak — passes its gate (the peak at least
/// `[biplaneWreck]`'s `MinImpulse` of 5) within a second of the crash test wrecking a plane flown
/// into the ground, and never before it. Not on the wreck tick itself, as the original's nose-in
/// was: dived in at 45°, the tires take the hit the tick before (a peak of 91) and the collision
/// object, touching the next tick, adds no impulse until the tumble's first bounce.
#[test]
fn a_plane_crash_is_heard() {
    let Some((_, f, _, _)) = the_plane() else { return };
    let tables = game_sim::ride::Tables::build();
    let mut f = f;
    let mut ground = Flat::default();
    f.attitude.flown = true;
    f.place(Vf::new(0.0, 30.0, 0.0, 1.0), Vf::new(0.0, -1.0, 1.0, 0.0), &tables.acos);
    f.body.velocity = Vf::new(0.0, -80.0, 80.0, 1.0);
    let dt = F(1.0 / 30.0);
    let pad = game_sim::biplane::Pad::default();
    let peak = |f: &Flight| {
        let p = f.body.shock_peak;
        (p.z() * p.z() + p.y() * p.y() + p.x() * p.x()).sqrt()
    };
    let mut wrecked = None;
    for tick in 0..90 {
        f.frame(dt, pad, &mut ground, &tables);
        if f.crash.reason == 0 {
            assert!(wrecked.is_none(), "tick {tick}: put back before it was heard");
            continue;
        }
        assert_eq!(f.crash.reason, CRASHED, "tick {tick}");
        let at = *wrecked.get_or_insert(tick);
        if f.record.touching && peak(&f) >= 5.0 {
            println!("wrecked at tick {at}, heard at {tick}: a shock peak of {}", peak(&f));
            assert!(tick - at < 30, "first heard {} ticks after the wreck", tick - at);
            return;
        }
    }
    panic!("the plane's crash was never heard (wrecked at {wrecked:?})");
}

/// **A touchdown draws one `rand()`** (P146): the original's touchdown sound picks from its list of
/// one with a `rand()` all the same, in the measure before the crash test. A plane dropped level onto
/// flat ground from 6 ft touches down, and on each tick its wheels first touch (and on no other, the
/// engine's noise once built) the plane's generator moves on exactly one draw.
#[test]
fn a_touchdown_draws_one_rand() {
    let Some((_, f, _, _)) = the_plane() else { return };
    let tables = game_sim::ride::Tables::build();
    let mut f = f;
    f.attitude.flown = true;
    let lowest = f.tires.iter().map(|t| F(t.center.y()) - t.radius).fold(F(f32::MAX), |a, b| a.min(b));
    f.place(Vf::new(0.0, (F(6.0) - lowest).to_f32(), 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), &tables.acos);
    let mut ground = Flat::default();
    let dt = F(1.0 / 30.0);
    f.frame(dt, Default::default(), &mut ground, &tables);
    assert!(!f.measure.grounded, "on the ground from 6 ft up");
    let mut touchdowns = 0;
    for tick in 1..90 {
        let before = f.rng.state();
        f.frame(dt, Default::default(), &mut ground, &tables);
        let touchdown = f.measure.grounded && !f.measure.was_grounded;
        let mut once = game_sim::rng::Rng::from_state(before);
        once.rand();
        let want = if touchdown { once.state() } else { before };
        assert_eq!(f.rng.state(), want, "tick {tick}: touchdown {touchdown}");
        touchdowns += touchdown as u32;
        assert_eq!(f.crash.reason, 0, "tick {tick}: a 6 ft drop wrecked the plane");
    }
    assert!(touchdowns >= 1, "never touched down");
}

/// **On its wheels, the collision object is clear of the ground**: a plane at rest and then taking
/// off at full throttle over flat ground rides its tires, and its rays touch nothing — so the
/// take-off of `tests/flight.rs` still leaves the ground, with the collision object on.
#[test]
fn a_plane_on_its_wheels_takes_off_untouched() {
    let Some((_, f, _, _)) = the_plane() else { return };
    let tables = game_sim::ride::Tables::build();
    let mut f = f;
    f.attitude.flown = true;
    let lowest = f.tires.iter().map(|t| F(t.center.y()) - t.radius).fold(F(f32::MAX), |a, b| a.min(b));
    f.place(Vf::new(0.0, (-lowest).to_f32(), 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), &tables.acos);
    let mut ground = Flat::default();
    let dt = F(1.0 / 30.0);
    let gas = game_sim::biplane::Pad { forward: F::ONE, ..Default::default() };
    for tick in 0..30 * 40 {
        let pad = if tick < 90 { Default::default() } else { gas };
        f.frame(dt, pad, &mut ground, &tables);
        assert!(!f.record.touching, "tick {tick}: the collision object touched the ground on its wheels");
        assert_eq!(f.crash.reason, 0, "tick {tick}: the plane crashed taking off");
    }
    assert!(!f.measure.grounded && f.body.position.y() > 100.0, "the plane never took off: {:?}", f.body.position);
}

/// Flat ground with props standing on it: the world a plane can fly into something in.
struct Standing {
    flat: Flat,
    props: game_sim::world::Props,
}

impl game_sim::world::World for Standing {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.flat.ground(at)
    }
    fn solid(&mut self, bundle: &game_sim::collide::Bundle, record: &mut game_sim::collide::Record) -> bool {
        self.props.sweep(bundle, record)
    }
    fn heightfield(&mut self) -> Option<Box<dyn game_sim::contact_search::Heightfield + '_>> {
        self.flat.heightfield()
    }
    fn collision(
        &mut self,
    ) -> (Option<Box<dyn game_sim::contact_search::Heightfield + '_>>, Option<&game_sim::world::Props>) {
        (self.flat.heightfield(), Some(&self.props))
    }
}

/// **The plane meets a track's meshes with its collision object**: flown level into a column of
/// collision meshes standing in the air (the plane's own hull, `biplanef.col`, stacked), its rays
/// strike them through S46's narrow phase, the struck mesh is what the record keeps, and the crash
/// test wrecks it — a mesh is scenery, not a vehicle the crash mask spares.
#[test]
fn a_plane_flown_into_a_mesh_crashes() {
    let Some((_, f, res, bytes)) = the_plane() else { return };
    let tables = game_sim::ride::Tables::build();
    let hull = track::Mesh::parse(&member(&res, &bytes, "Biplane\\col\\biplanef.col")).expect("biplanef.col");
    let at = |y: f32| track::Placement {
        position: [F(0.0), F(y), F(60.0)],
        look: [F(1.0), F(0.0), F(0.0)],
        ..track::Placement::default()
    };
    let props = game_sim::world::Props { props: (0..9).map(|k| game_sim::world::Prop::place(hull.clone(), &at(176.0 + 6.0 * k as f32))).collect() };
    let mut world = Standing { flat: Flat::default(), props };
    let mut f = f;
    f.attitude.flown = true;
    f.place(Vf::new(0.0, 200.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), &tables.acos);
    f.body.velocity = Vf::new(0.0, 0.0, 120.0, 1.0);
    let dt = F(1.0 / 30.0);
    let gas = game_sim::biplane::Pad { forward: F::ONE, ..Default::default() };
    for tick in 0..60 {
        f.frame(dt, gas, &mut world, &tables);
        if f.crash.reason != 0 {
            break;
        }
        assert!(f.body.position.z() < 70.0, "tick {tick}: the plane flew through the meshes: {:?}", f.body.position);
    }
    assert!(f.record.struck.is_some(), "the rays never struck a mesh");
    assert_eq!(f.crash.reason, CRASHED, "the plane did not crash into the meshes");
}

/// **The plane meets a tree** (S102): flown level at twenty feet into one of free09's trunks standing
/// on a floor ([`game_sim::eco::Wooded`]), its rays strike the trunk through the placed group's part
/// and it crashes; with no tree it flies on past where the trunk would be.
#[test]
fn a_plane_flown_into_a_tree_crashes() {
    use game_sim::eco::{self, Ecosystem, Trees, Wooded};
    let Some((_, f, _, _)) = the_plane() else { return };
    let Some(kit) = Kit::load().ok() else { return };
    let Ok(mut file) = File::open(kit.ctx("iso").join("MX.PSR")) else { return };
    let mut front = vec![0u8; 64 << 10];
    file.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("Tracks\\free09.res").expect("free09.res is in MX.PSR");
    file.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    file.read_exact(&mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("free09.res");
    let esb = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with("free09.esb")).expect("free09.esb");
    let mut eco = Ecosystem::parse(&res.read(&bytes, esb).expect("its bytes")).expect("free09.esb parses");
    let cell = eco::cell_size(F(6912.0));
    // A TreeB (the file's slot 1) sixty feet ahead on the floor.
    let mut plant = *eco.plants.iter().find(|p| p.species == 1).expect("a TreeB");
    plant.cell = [0, 0, (60.0 / cell.0) as u16];
    eco.plants = vec![plant];
    let (trees, missed) = Trees::new(&eco, cell, &game_sim::camera::CurveTable::build());
    assert!(missed.is_empty(), "{missed:?}");

    let tables = game_sim::ride::Tables::build();
    let fly = |world: &mut dyn game_sim::world::World| {
        let mut f = f.clone();
        f.attitude.flown = true;
        f.place(Vf::new(0.0, 20.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), &tables.acos);
        f.body.velocity = Vf::new(0.0, 0.0, 120.0, 1.0);
        let gas = game_sim::biplane::Pad { forward: F::ONE, ..Default::default() };
        for _ in 0..30 {
            f.frame(F(1.0 / 30.0), gas, world, &tables);
            if f.crash.reason != 0 {
                break;
            }
        }
        f
    };
    let open = fly(&mut Flat::default());
    assert_eq!(open.crash.reason, 0, "the plane crashed over the bare floor");
    assert!(open.body.position.z() > 70.0, "the plane did not fly past the tree's spot: {:?}", open.body.position);

    let struck = fly(&mut Wooded { world: Flat::default(), trees });
    assert!(struck.record.struck.is_some(), "the rays never struck the trunk");
    assert_eq!(struck.crash.reason, CRASHED, "the plane did not crash into the tree");
    assert!(struck.body.position.z() < 62.0, "the plane flew through the trunk: {:?}", struck.body.position);
}
