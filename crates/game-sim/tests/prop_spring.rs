//! S196 — the supercross banners' `BodyTorsionSpring`, and a collection's own `ResetTime`, against
//! the original's run in the interpreter.
//!
//! - **The load**: sx01's props, made by [`PropSet::load`] off the disc alone, hold the springs the
//!   original read for its nine banners: the stiffness and damping off each body's inertia, the body
//!   it pulls, its switch; each filed first on its collection world's force list, a listener whose
//!   force is the spring's pull.
//! - **The swing**: a banner woken in a fresh sx01 state (S133's scripted boot) and spun about its
//!   pin, the original's collection update and node listeners frame by frame against
//!   [`Collection::tick`]: the spring pulling it back through the angle limits, the body going to
//!   sleep, its five-second countdown and the put-back, bit for bit.
//! - **The collection's countdown**: free11's ten bowling pins (one collection) are one of the two kinds of prop on the disc whose
//!   `.pco` gives the collection a `ResetTime` (5) and its parts none (the spare tyres are the
//!   other). One pin woken with a small spin and its gravity zeroed in both (no collide pass runs
//!   here, so a pin with gravity falls for ever) sleeps, the collection counts its five seconds down,
//!   puts the pins back and deactivates. The loader had left every collection's `ResetTime` at -1.
//!
//! No `.pco` on the disc has a `PinToBody`, so that constraint's run has nothing to be proved on.
//!
//! The states are `context/states/s196-sx01.ee.bin` and `s196-free11.ee.bin` (`./kit state ram` of a
//! save made right after the scripted boot). Skipped with a line where the disc, the kit or a state is
//! not there.

use std::path::PathBuf;

use game_render::disc::{Disc, Res};
use game_sim::biplane::Body;
use game_sim::contact::Node;
use game_sim::math::Mat4;
use game_sim::phy::{Ini, Part};
use game_sim::prop_set::PropSet;
use game_sim::prop_world::{Collection, Edge, Listener};
use game_sim::props::Pco;
use game_sim::ragdoll::{Bone, Tables};
use game_sim::track::Scene;
use ps2_float::{Dest, F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const BUDGET: u64 = 50_000_000;

/// The collection's vtable: how its objects are found in a state.
const COLLECTION_VTABLE: u32 = 0x0045_da90;

/// The collection's fields.
mod coll {
    pub const VTABLE: u32 = 0x08;
    pub const FLAGS: u32 = 0x20;
    pub const ACTIVE: u32 = 2;
    pub const LISTENER: u32 = 0x34;
    pub const PCO_PATH: u32 = 0x40;
    pub const NAME: u32 = 0x144;
    pub const NODE: u32 = 0x248;
    /// The collection's own list of the pins its constraint reader made.
    pub const PINS: u32 = 0x288;
    /// The world's force list: a torsion spring is filed there at load, nothing else is.
    pub const FORCES: u32 = 0x2b8;
    pub const BODIES: u32 = 0x320;
    pub const COUNTDOWN: u32 = 0x338;
    pub const RESET_TIME: u32 = 0x33c;
}

/// The physics object's.
mod obj {
    pub const LISTENER: u32 = 0x50;
    pub const GRAVITY: u32 = 0xf0;
    pub const EDGE: u32 = 0x100;
    pub const PLACE: u32 = 0x90;
    pub const BODY: u32 = 0x110;
    pub const NODE: u32 = 0x13c;
    pub const FRESH: u32 = 0x1b0;
    pub const COUNTDOWN: u32 = 0x1bc;
}

/// The body's.
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
    /// The body's own listener list (count first), which a moment added is told to.
    pub const LISTENERS: u32 = 0xd0;
    pub const STEP: u32 = 0xf0;
    pub const INERTIA: u32 = 0x110;
    pub const INV_INERTIA: u32 = 0x120;
    pub const ANG_DAMPING: u32 = 0x130;
    pub const LAST_QUAT: u32 = 0x150;
    pub const START_TURN: u32 = 0x160;
    pub const QUAT: u32 = 0x170;
    pub const COFG: u32 = 0x180;
    pub const MOMENT: u32 = 0x190;
    pub const LAST_MOMENT: u32 = 0x1a0;
    pub const RATES: u32 = 0x1b0;
    pub const ANG_ACCEL: u32 = 0x1c0;
    pub const TWIST_PEAK: u32 = 0x1d0;
    pub const TWIST: u32 = 0x1e0;
}

/// The torsion spring's.
mod spring_at {
    pub const ON: u32 = 0x00;
    pub const VTABLE: u32 = 0x08;
    pub const STIFFNESS: u32 = 0x10;
    pub const DAMPING: u32 = 0x20;
    pub const BODY: u32 = 0x30;
    /// The force listener's slot in its vtable.
    pub const FORCE_SLOT: u32 = 0x0c;
}

/// The pin's.
mod pin_at {
    pub const LOWER: u32 = 0x80;
    pub const UPPER: u32 = 0x90;
}

const NODE_WORLD: u32 = 0x80;
const NODE_LISTENERS: u32 = 0x48;
const EDGE_MIN: u32 = 0x30;
const EDGE_MAX: u32 = 0x40;

/// The original's functions.
mod f {
    pub const UPDATE: u32 = 0x003b_28a0;
    pub const SETTLE: u32 = 0x003b_2440;
    pub const ACTIVATE: u32 = 0x003b_27a0;
    pub const ENABLE: u32 = 0x0015_b790;
    pub const LISTEN: u32 = 0x0015_b2a0;
    pub const SPRING_VTABLE: u32 = 0x0045_1ea0;
    pub const SPRING_FORCE: u32 = 0x001f_3180;
}

fn image(name: &str) -> Option<Ee> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/states").join(name);
    let path = if here.exists() { here } else { kit.ctx("states").join(name) };
    match Ee::from_ram(&path) {
        Ok(mut ee) => {
            ee.set(28, u64::from(addr("boot_gp").unwrap()));
            Some(ee)
        }
        Err(why) => {
            skip(&format!(
                "{why} — S196's journal: boot the track with S133's boot.py, then `./kit state ram <slot> --out context/states/{name}`"
            ));
            None
        }
    }
}

fn archive(track: &str) -> Option<Res> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let want = format!("Tracks\\{track}.res");
    let member = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case(&want)).expect("the track's archive").name.clone();
    Some(disc.res(&member).expect("the track's archive"))
}

/// The track's props, loaded the way the shell loads them (no area: the tests set the edge off the
/// state).
fn props(track: &str, res: &Res) -> PropSet {
    let toy = Ini::parse_bytes(&res.read_suffix(&format!("{track}.toy")).expect("the .toy"));
    let scene = Scene::parse(&res.read_suffix(&format!("{track}.scn")).expect("the .scn")).expect("the .scn parses");
    let (set, said) =
        PropSet::load(&toy, &format!("Tracks\\{track}\\"), &scene.models, &mut |p| res.read(p).ok(), &mut |p| res.read_suffix(p).ok(), None);
    assert!(said.is_empty(), "{track} loads whole: {said:?}");
    set
}

fn text(ee: &Ee, at: u32) -> String {
    (0..256).map(|k| ee.u8_at(at + k)).take_while(|&b| b != 0).map(|b| b as char).collect()
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|k| ee.u32_at(a + k)))
}

fn mat(ee: &Ee, a: u32) -> Mat4 {
    Mat4([0, 1, 2, 3].map(|k| quad(ee, a + 16 * k)))
}

/// A `{count, capacity, items}` list's items.
fn list(ee: &Ee, at: u32) -> Vec<u32> {
    let at_items = ee.u32_at(at + 8);
    (0..ee.u32_at(at)).map(|k| ee.u32_at(at_items + 4 * k)).collect()
}

/// Every collection in the state, by its vtable.
fn collections(ee: &Ee) -> Vec<u32> {
    (0x0010_0000u32..0x0200_0000).step_by(4).filter(|&a| ee.u32_at(a) == COLLECTION_VTABLE).map(|a| a - coll::VTABLE).collect()
}

fn ee_body(ee: &Ee, b: u32) -> Body {
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

fn bodies_differ(got: &Body, want: &Body) -> Vec<String> {
    let mut out = Vec::new();
    if got.enabled != want.enabled {
        out.push(format!("enabled: the game {}, the port {}", want.enabled, got.enabled));
    }
    for (name, g, w) in [
        ("last_position", got.last_position, want.last_position),
        ("position", got.position, want.position),
        ("velocity", got.velocity, want.velocity),
        ("last_force", got.last_force, want.last_force),
        ("force", got.force, want.force),
        ("shock_peak", got.shock_peak, want.shock_peak),
        ("shock", got.shock, want.shock),
        ("step", got.step, want.step),
        ("last_quat", got.last_quat, want.last_quat),
        ("quat", got.quat, want.quat),
        ("moment", got.moment, want.moment),
        ("last_moment", got.last_moment, want.last_moment),
        ("rates", got.rates, want.rates),
        ("ang_accel", got.ang_accel, want.ang_accel),
        ("twist_peak", got.twist_peak, want.twist_peak),
        ("twist", got.twist, want.twist),
    ] {
        if g.to_bits() != w.to_bits() {
            out.push(format!("{name}: the game {:?}, the port {:?}", w.0, g.0));
        }
    }
    // The linear acceleration's `w` is whatever a vector register held (see `Body::integrate`).
    if got.accel.to_bits()[..3] != want.accel.to_bits()[..3] {
        out.push(format!("accel: the game {:?}, the port {:?}", want.accel.0, got.accel.0));
    }
    out
}

fn node_rows(n: &Node) -> [[u32; 4]; 4] {
    [n.rows[0].to_bits(), n.rows[1].to_bits(), n.rows[2].to_bits(), n.origin.to_bits()]
}

fn ee_rows(ee: &Ee, node: u32) -> [[u32; 4]; 4] {
    [0, 1, 2, 3].map(|k| quad(ee, node + NODE_WORLD + 16 * k).to_bits())
}

/// The port's collection that stands where the game's collection `c` does: its bodies' places.
fn port_of<'a>(set: &'a PropSet, ee: &Ee, c: u32) -> &'a Collection {
    let places: Vec<[u32; 4]> = list(ee, c + coll::BODIES).iter().map(|&o| quad(ee, o + obj::PLACE).to_bits()).collect();
    let mut at = set.collections.iter().filter(|p| p.objects.iter().map(|o| o.place.to_bits()).eq(places.iter().copied()));
    let port = at.next().expect("a port collection at the game's places");
    assert!(at.next().is_none(), "one port collection at {places:x?}");
    port
}

/// The game's collection `c` as the port makes it, off the state's node frames and bodies (the
/// loader's own placement is S131's proof, and a few of its turns drift an ulp off a state's), with
/// the loader's bone inertia; its edge off the state's edge object.
fn rebuilt(ee: &Ee, res: &Res, c: u32, loaded: &Collection) -> Collection {
    let name = text(ee, c + coll::NAME);
    let objects = list(ee, c + coll::BODIES);
    let pco = Pco::read_bytes(&res.read(&text(ee, c + coll::PCO_PATH)).expect("the .pco"));
    // A body for each part, or a group's one part for each of its models.
    assert!(pco.parts.len() == objects.len() || pco.parts.len() == 1, "{name}: {} parts, {} bodies", pco.parts.len(), objects.len());
    let parts: Vec<(&Part, &str)> = (0..objects.len()).map(|k| &pco.parts[k % pco.parts.len()]).map(|p| (&p.part, p.name.as_str())).collect();
    let mut bones = Vec::new();
    let mut frames = Vec::new();
    for (k, &o) in objects.iter().enumerate() {
        let b = ee.u32_at(o + obj::BODY);
        let mut body = ee_body(ee, b);
        body.quat = Vf::VF0;
        body.last_quat = Vf::VF0;
        bones.push(Bone { body, mass: F(ee.f32_at(b + body_at::MASS)), inertia: loaded.bones[k].inertia, reference: Vf::VF0 });
        frames.push(mat(ee, ee.u32_at(o + obj::NODE) + NODE_WORLD));
    }
    let tables = Tables::build();
    let (mut port, unmade) = Collection::new(&pco, &parts, bones, &frames, &tables.curve);
    assert!(unmade.is_empty(), "{name}: {unmade:?}");
    assert_eq!(port.springs, loaded.springs, "{name}: the springs as the loader made them");
    assert_eq!(port.reset_time.to_bits(), loaded.reset_time.to_bits(), "{name}: the ResetTime as the loader read it");
    for (k, &o) in objects.iter().enumerate() {
        let b = ee.u32_at(o + obj::BODY);
        let d = bodies_differ(&port.bones[k].body, &ee_body(ee, b));
        assert!(d.is_empty(), "{name}: body {k} placed: {d:#?}");
        assert_eq!(port.bones[k].reference.to_bits(), quad(ee, b + body_at::START_TURN).to_bits(), "{name}: body {k}'s start turn");
    }
    let edge = ee.u32_at(objects[0] + obj::EDGE);
    if edge != 0 {
        let lane = |a: u32, k: u32| F(ee.f32_at(edge + a + 4 * k));
        port.edge = Some(Edge { min: [lane(EDGE_MIN, 0), lane(EDGE_MIN, 2)], max: [lane(EDGE_MAX, 0), lane(EDGE_MAX, 2)] });
    }
    port
}

/// What a run saw.
#[derive(Debug, Default)]
struct Run {
    frames: u32,
    slept: Option<u32>,
    put_back: Option<u32>,
    /// The woken body's furthest turn from its start, in degrees.
    peak: f32,
    /// Frames the collection's own countdown was running on.
    counting: u32,
    /// The woken body's turn, the game's, frame by frame.
    quats: Vec<[u32; 4]>,
}

/// Wakes the game's collection `c` and the port's, enables its first object and spins it by `spin`
/// (body-frame rates), then runs the original's update and node listeners beside
/// [`Collection::tick`] frame by frame, comparing every body and node, both countdowns and the
/// lifecycle, until both have no listener left or `limit` frames.
fn run(ee: &mut Ee, c: u32, port: &mut Collection, spin: Vf, limit: u32) -> Run {
    let name = text(ee, c + coll::NAME);
    let objects = list(ee, c + coll::BODIES);
    let o = objects[0];
    let b = ee.u32_at(o + obj::BODY);
    let world_node = ee.u32_at(c + coll::NODE);
    let start = quad(ee, b + body_at::START_TURN);
    assert_eq!(ee.u32_at(c + coll::FLAGS) & coll::ACTIVE, 0, "{name} asleep in the state");
    let tables = Tables::build();

    let before = list(ee, world_node + NODE_LISTENERS);
    ee.call(f::ACTIVATE, &[u64::from(c)], &[], BUDGET);
    ee.call(f::ENABLE, &[u64::from(o)], &[], BUDGET);
    port.activate();
    port.enable(0);
    for (k, v) in spin.0.iter().enumerate() {
        ee.set_f32(b + body_at::RATES + 4 * k as u32, *v);
    }
    port.bones[0].body.rates = spin;
    let after = list(ee, world_node + NODE_LISTENERS);
    assert_eq!(after[before.len()..], [c + coll::LISTENER, o + obj::LISTENER], "{name}: the wake's two listeners, in order");
    assert_eq!(port.listeners, [Listener::Settle, Listener::Object(0)], "{name}: the port's listeners");

    let dt = 1.0f32 / 30.0;
    let mut game = vec![(f::SETTLE, c), (f::LISTEN, o)];
    let mut seen = Run::default();
    while seen.frames < limit && !(game.is_empty() && port.listeners.is_empty()) {
        seen.frames += 1;
        let frame = seen.frames;
        ee.call(f::UPDATE, &[u64::from(c)], &[dt], BUDGET);
        let mut i = 0;
        while i < game.len() {
            let (fun, this) = game[i];
            ee.call(fun, &[u64::from(this)], &[dt], BUDGET);
            if ee.v0() as u32 != 0 {
                game.remove(i);
            } else {
                i += 1;
            }
        }
        port.tick(F(dt), &tables);

        let at = |what: &str| format!("{name}, frame {frame}: {what}");
        for (k, &ok) in objects.iter().enumerate() {
            let d = bodies_differ(&port.bones[k].body, &ee_body(ee, ee.u32_at(ok + obj::BODY)));
            assert!(d.is_empty(), "{}: {d:#?}", at(&format!("body {k}")));
            assert_eq!(node_rows(&port.objects[k].node), ee_rows(ee, ee.u32_at(ok + obj::NODE)), "{}", at(&format!("node {k}")));
            assert_eq!(port.objects[k].countdown.to_bits(), ee.u32_at(ok + obj::COUNTDOWN), "{}", at(&format!("object {k}'s countdown")));
            assert_eq!(port.objects[k].fresh, ee.u32_at(ok + obj::FRESH) != 0, "{}", at(&format!("object {k} fresh")));
        }
        assert_eq!(port.countdown.to_bits(), ee.u32_at(c + coll::COUNTDOWN), "{}", at("the collection's countdown"));
        assert_eq!(port.active, ee.u32_at(c + coll::FLAGS) & coll::ACTIVE != 0, "{}", at("active"));
        let left: Vec<u32> = game.iter().map(|&(fun, _)| fun).collect();
        let ported: Vec<u32> = port.listeners.iter().map(|l| if *l == Listener::Settle { f::SETTLE } else { f::LISTEN }).collect();
        assert_eq!(ported, left, "{}", at("the listeners left"));
        let body = ee_body(ee, b);
        seen.quats.push(body.quat.to_bits());
        let dot: f32 = (0..4).map(|k| body.quat.0[k] * start.0[k]).sum();
        seen.peak = seen.peak.max(2.0 * dot.abs().min(1.0).acos().to_degrees());
        if !body.enabled && seen.slept.is_none() {
            seen.slept = Some(frame);
        }
        if port.countdown >= F::ZERO {
            seen.counting += 1;
        }
        // The countdown that ran out, the object's or the collection's, put the body back.
        let running = port.objects[0].countdown >= F::ZERO || port.countdown >= F::ZERO;
        if seen.put_back.is_none() && seen.slept.is_some() && !running {
            seen.put_back = Some(frame);
        }
    }
    seen
}

#[test]
fn sx01s_banners_load_the_springs_the_original_read() {
    let Some(ee) = image("s196-sx01.ee.bin") else { return };
    let Some(res) = archive("sx01") else { return };
    let set = props("sx01", &res);
    assert_eq!(set.collections.len(), collections(&ee).len(), "sx01's collections");

    // The spring's force listener is the pull, at its vtable's force slot.
    assert_eq!(ee.u32_at(f::SPRING_VTABLE + spring_at::FORCE_SLOT), f::SPRING_FORCE, "the spring's force slot");
    let banners: Vec<u32> = collections(&ee).into_iter().filter(|&c| !list(&ee, c + coll::FORCES).is_empty()).collect();
    assert_eq!(banners.len(), 9, "sx01's banners");
    assert_eq!(set.collections.iter().filter(|c| !c.springs.is_empty()).count(), banners.len(), "the port's banners");
    for &c in &banners {
        let name = text(&ee, c + coll::NAME);
        assert!(text(&ee, c + coll::PCO_PATH).to_ascii_lowercase().ends_with("banner.pco"), "{name}: a banner");
        let port = port_of(&set, &ee, c);
        // Filed on the world's force list at load: ahead of any object's force listener.
        let forces = list(&ee, c + coll::FORCES);
        assert_eq!(forces.len(), 1, "{name}: one spring");
        assert_eq!(port.springs.len(), 1, "{name}: the port's one spring");
        let s = forces[0];
        let ps = port.springs[0];
        assert_eq!(ee.u32_at(s + spring_at::VTABLE), f::SPRING_VTABLE, "{name}: a torsion spring");
        let o = list(&ee, c + coll::BODIES)[0];
        let b = ee.u32_at(o + obj::BODY);
        assert_eq!(ee.u32_at(s + spring_at::BODY), b, "{name}: the spring's body");
        assert_eq!(ps.body, 0, "{name}: the port spring's body");
        assert_eq!(ps.on, ee.u32_at(s + spring_at::ON) != 0, "{name}: the switch");
        assert_eq!(port.bones[0].inertia.to_bits(), quad(&ee, b + body_at::INERTIA).to_bits(), "{name}: the inertia");
        assert_eq!(ps.stiffness.to_bits(), quad(&ee, s + spring_at::STIFFNESS).to_bits(), "{name}: the stiffness");
        assert_eq!(ps.damping.to_bits(), quad(&ee, s + spring_at::DAMPING).to_bits(), "{name}: the damping");
        // A moment added to the body is told to no one.
        assert_eq!(ee.u32_at(b + body_at::LISTENERS), 0, "{name}: the body's listeners");
        // The pin, its limits.
        let p = list(&ee, c + coll::PINS)[0];
        assert_eq!(port.joints[0].pin.lower.to_bits(), quad(&ee, p + pin_at::LOWER).to_bits(), "{name}: the lower limit");
        assert_eq!(port.joints[0].pin.upper.to_bits(), quad(&ee, p + pin_at::UPPER).to_bits(), "{name}: the upper limit");
    }
    eprintln!("sx01: {} banners, stiffness {:?}", banners.len(), set.collections.iter().find(|c| !c.springs.is_empty()).unwrap().springs[0].stiffness.0);
}

#[test]
fn a_swung_banner_springs_back_sleeps_and_is_put_back() {
    let Some(mut ee) = image("s196-sx01.ee.bin") else { return };
    let Some(res) = archive("sx01") else { return };
    let set = props("sx01", &res);
    let c = *collections(&ee).iter().find(|&&c| !list(&ee, c + coll::FORCES).is_empty()).expect("a banner");
    let name = text(&ee, c + coll::NAME);
    let mut port = rebuilt(&ee, &res, c, port_of(&set, &ee, c));
    let tables = Tables::build();

    // The same swing with the spring taken off: the spring must change the run.
    let mut loose = port.clone();
    loose.springs.clear();
    loose.activate();
    loose.enable(0);
    let spin = Vf::new(8.0, 0.0, 0.0, 1.0);
    loose.bones[0].body.rates = spin;

    let seen = run(&mut ee, c, &mut port, spin, 3000);
    let mut parted = None;
    for (k, want) in seen.quats.iter().enumerate() {
        loose.tick(F(1.0 / 30.0), &tables);
        if loose.bones[0].body.quat.to_bits() != *want {
            parted = Some(k + 1);
            break;
        }
    }
    let parted = parted.expect("the spring changes the swing");
    assert!(parted <= 3, "{name}: without its spring the swing parts from the game's at frame {parted}");
    let (slept, put_back) = (seen.slept.expect("the banner went to sleep"), seen.put_back.expect("the banner was put back"));
    // Up against the pin's 70° limit, the spring pulling it back the other way.
    assert!(seen.peak > 65.0, "{name}: swung {:.1}°", seen.peak);
    assert!(put_back > slept + 140, "{name}: put back at frame {put_back}, five seconds after sleeping at {slept}");
    assert!(!port.active, "{name}: the collection asleep again");
    eprintln!(
        "{name}: swung {:.1}°, asleep at frame {slept}, put back at {put_back}, done at {}, spring-less parts at frame {parted}",
        seen.peak, seen.frames
    );
}

#[test]
fn a_bowling_pin_waits_out_its_collections_reset_time() {
    let Some(mut ee) = image("s196-free11.ee.bin") else { return };
    let Some(res) = archive("free11") else { return };
    let set = props("free11", &res);
    let c = *collections(&ee)
        .iter()
        .find(|&&c| text(&ee, c + coll::PCO_PATH).to_ascii_lowercase().ends_with("bowlingpin.pco"))
        .expect("a bowling pin on free11");
    let name = text(&ee, c + coll::NAME);
    // Every collection's ResetTime as the original read it: free11's bowling pins' and spare tyres'
    // `[PhysicsParts]` give one.
    for k in collections(&ee) {
        let port = port_of(&set, &ee, k);
        assert_eq!(port.reset_time.to_bits(), ee.u32_at(k + coll::RESET_TIME), "{}: the collection's ResetTime", text(&ee, k + coll::NAME));
    }
    let loaded = port_of(&set, &ee, c);
    assert_eq!(loaded.reset_time, F(5.0), "{name}: five seconds");
    assert!(loaded.objects.iter().all(|o| o.reset_time < F::ZERO), "{name}: no ResetTime of the pins' own");
    let mut port = rebuilt(&ee, &res, c, loaded);

    // No ground under it here: no gravity on either side.
    let o = list(&ee, c + coll::BODIES)[0];
    for k in 0..3 {
        ee.set_f32(o + obj::GRAVITY + 4 * k, 0.0);
    }
    port.objects[0].gravity = port.objects[0].gravity.merge(Dest::XYZ, Vf::ZERO);
    // Under its AngularRateThreshold (0.5): it sleeps as soon as it is no longer fresh.
    let seen = run(&mut ee, c, &mut port, Vf::new(0.3, 0.0, -0.2, 1.0), 400);
    let slept = seen.slept.expect("the pin went to sleep");
    let put_back = seen.put_back.expect("the collection put the pin back");
    assert!(seen.counting > 140, "{name}: the collection counted down on {} frames", seen.counting);
    assert!(put_back > slept + 140, "{name}: put back at frame {put_back}, five seconds after sleeping at {slept}");
    assert!(!port.active, "{name}: the collection asleep again");
    eprintln!("{name}: asleep at frame {slept}, counted {} frames, put back at {put_back}", seen.counting);
}
