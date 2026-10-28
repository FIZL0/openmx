//! S130 — a knocked free09 prop, the port's collection world against the original's run in the
//! interpreter.
//!
//! The first recording's save state is free09 with every prop asleep where it was made. A road sign
//! (one body, a `Pin` with `ConstrainAngles`) is woken in the state the way a strike wakes it — its
//! collection activated, its object enabled — and given a spin. The original's own code then runs
//! frame by frame in the interpreter: the collection's update (its world step: the preforce, force
//! and post listeners, the pin), and the two listeners the wake put on the world node, the
//! collection's and the object's. The port's [`prop_world::Collection`] runs beside it from the same
//! state, and every frame the body, the node's rows, the countdowns and the lifecycle flags are
//! compared bit for bit: through the swing against the angle limits, the body going to sleep, the
//! object's five-second countdown, the put-back, and the collection deactivating.
//!
//! The collision object is left out on both sides (the node's collide pass is not run, and the sign
//! never had a contact): the props' contacts are S131's.
//!
//! Skipped with a line where the disc, the kit or the state is not there.

use std::path::PathBuf;

use game_render::disc::Disc;
use game_sim::biplane::Body;
use game_sim::contact::Node;
use game_sim::math::Mat4;
use game_sim::phy::Part;
use game_sim::prop_world::{self, BoundsNode, Collection, Edge, Listener};
use game_sim::props::{NodeBox, Pco};
use game_sim::ragdoll::{Bone, Tables};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "first_recording.p2m2_SaveState.eeMemory.bin";
const BUDGET: u64 = 50_000_000;

/// The `.toy` actor's collection list (see tests/props.rs).
const LIST: u32 = 0x011b_43f4;

/// The collection's fields.
mod coll {
    pub const FLAGS: u32 = 0x20;
    pub const ACTIVE: u32 = 2;
    pub const PCO_PATH: u32 = 0x40;
    pub const NAME: u32 = 0x144;
    pub const LISTENER: u32 = 0x34;
    pub const NODE: u32 = 0x248;
    pub const CONSTRAINTS: u32 = 0x288; // count, then the array's pointer at +8
    pub const COUNTDOWN: u32 = 0x338;
    pub const RESET_TIME: u32 = 0x33c;
    pub const BODIES: u32 = 0x320;
}

/// The physics object's.
mod obj {
    pub const LISTENER: u32 = 0x50;
    pub const COLLISION: u32 = 0x54;
    pub const LISTENS: u32 = 0x60;
    pub const DETECT: u32 = 0x6c;
    pub const OFF_EDGE: u32 = 0x70;
    pub const PLACE: u32 = 0x90;
    pub const LOOK: u32 = 0xa0;
    pub const UP: u32 = 0xb0;
    pub const GRAVITY: u32 = 0xf0;
    pub const EDGE: u32 = 0x100;
    pub const BODY: u32 = 0x110;
    pub const NODE: u32 = 0x13c;
    pub const FRESH: u32 = 0x1b0;
    pub const COUNTDOWN: u32 = 0x1bc;
    pub const RESET_TIME: u32 = 0x1c0;
    pub const THRESHOLDS: u32 = 0x1c4;
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
    pub const STEP: u32 = 0xf0;
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

/// The pin's.
mod pin_at {
    pub const ON: u32 = 0x04;
    pub const ANCHOR1: u32 = 0x20;
    pub const ANCHOR2: u32 = 0x30;
    pub const LIMITED: u32 = 0x70;
    pub const LOWER: u32 = 0x80;
    pub const UPPER: u32 = 0x90;
}

/// The node's: the world rows, the box, its flags, the first child and the next sibling.
mod node_at {
    pub const WORLD: u32 = 0x80;
    pub const CENTRE: u32 = 0xc0;
    pub const HALF: u32 = 0xd0;
    pub const CHILDREN: u32 = 0x104;
    pub const NEXT: u32 = 0x108;
    pub const FLAGS: u32 = 0x110;
    pub const HAS_BOX: u8 = 4;
    pub const LISTENERS: u32 = 0x48; // count, then the array's pointer at +8
}

/// The edge object's bounds.
const EDGE_MIN: u32 = 0x30;
const EDGE_MAX: u32 = 0x40;

/// The original's functions.
mod f {
    pub const UPDATE: u32 = 0x003b_28a0;
    pub const SETTLE: u32 = 0x003b_2440;
    pub const ACTIVATE: u32 = 0x003b_27a0;
    pub const ENABLE: u32 = 0x0015_b790;
    pub const LISTEN: u32 = 0x0015_b2a0;
    pub const NODE_BOUNDS: u32 = 0x001b_3f50;
}

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

fn mat(ee: &Ee, a: u32) -> Mat4 {
    Mat4([0, 1, 2, 3].map(|k| quad(ee, a + 16 * k)))
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
    [0, 1, 2, 3].map(|k| quad(ee, node + node_at::WORLD + 16 * k).to_bits())
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

/// The world node's listener list, as `this` pointers.
fn node_listeners(ee: &Ee, node: u32) -> Vec<u32> {
    let n = ee.u32_at(node + node_at::LISTENERS);
    let at = ee.u32_at(node + node_at::LISTENERS + 8);
    (0..n).map(|k| ee.u32_at(at + 4 * k)).collect()
}

#[test]
fn a_knocked_sign_swings_sleeps_counts_down_and_is_put_back() {
    let Some(mut ee) = image() else { return };
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => return skip(&why),
    };
    let member = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("Tracks\\free09.res")).expect("free09.res").name.clone();
    let res = disc.res(&member).expect("free09's archive");

    // The first sign: one body, one pin.
    let c = *collections(&ee)
        .iter()
        .find(|&&c| text(&ee, c + coll::NAME).to_ascii_lowercase().starts_with("sign_"))
        .expect("a sign on free09");
    let name = text(&ee, c + coll::NAME);
    assert_eq!(ee.u32_at(c + coll::BODIES), 1, "{name}: one body");
    let o = ee.u32_at(ee.u32_at(c + coll::BODIES + 8));
    let b = ee.u32_at(o + obj::BODY);
    let node = ee.u32_at(o + obj::NODE);
    let world_node = ee.u32_at(c + coll::NODE);
    assert_eq!(ee.u32_at(c + coll::FLAGS) & coll::ACTIVE, 0, "{name} asleep in the state");
    assert!(!ee_body(&ee, b).enabled, "{name}'s body asleep in the state");

    // The port's collection, made by the loader off the state's node.
    let pco = Pco::read_bytes(&res.read(&text(&ee, c + coll::PCO_PATH)).expect("the sign's .pco"));
    let part: &Part = &pco.parts[0].part;
    let tables = Tables::build();
    let mut body = ee_body(&ee, b);
    body.quat = Vf::VF0;
    body.last_quat = Vf::VF0;
    let bone = Bone { body, mass: F(ee.f32_at(b + body_at::MASS)), inertia: Vf::ZERO, reference: Vf::VF0 };
    let frame = mat(&ee, node + node_at::WORLD);
    let (mut port, unmade) = Collection::new(&pco, &[(part, &pco.parts[0].name)], vec![bone], &[frame], &tables.curve);
    assert!(unmade.is_empty(), "{name}: {unmade:?}");

    // What the loader made, against the state.
    let same = |what: &str, got: Vf, at: u32| assert_eq!(got.to_bits(), quad(&ee, at).to_bits(), "{name}: {what}");
    let ob = &port.objects[0];
    same("gravity", ob.gravity, o + obj::GRAVITY);
    assert_eq!(ob.collision_enabled, ee.u32_at(o + obj::COLLISION) != 0, "{name}: CollisionEnabled");
    assert_eq!(ob.detect_when_disabled, ee.u32_at(o + obj::DETECT) != 0, "{name}: DetectCollisionsWhenDisabled");
    assert_eq!(ob.reset_off_edge, ee.u32_at(o + obj::OFF_EDGE) != 0, "{name}: ResetOffEdgeOfWorld");
    assert_eq!(ob.listens, ee.u32_at(o + obj::LISTENS) != 0, "{name}: the object listener");
    assert_eq!(ob.reset_time.to_bits(), ee.u32_at(o + obj::RESET_TIME), "{name}: ResetTime");
    assert_eq!(ob.countdown.to_bits(), ee.u32_at(o + obj::COUNTDOWN), "{name}: no countdown");
    for k in 0..6 {
        assert_eq!(ob.thresholds[k].to_bits(), ee.u32_at(o + obj::THRESHOLDS + 4 * k as u32), "{name}: threshold {k}");
    }
    assert_eq!(port.reset_time.to_bits(), ee.u32_at(c + coll::RESET_TIME), "{name}: the collection's ResetTime");
    assert_eq!(port.countdown.to_bits(), ee.u32_at(c + coll::COUNTDOWN), "{name}: the collection's countdown");
    assert_eq!(port.world.iterations, ee.i32_at(c + 0x250), "{name}: iterations");
    assert!(!port.pin_to_body, "{name}: no PinToBody");
    // The placement off the state's node: the same body and node the original's left.
    same("place", ob.place, o + obj::PLACE);
    same("look", ob.look.merge(ps2_float::Dest::W, quad(&ee, o + obj::LOOK)), o + obj::LOOK);
    same("up", ob.up.merge(ps2_float::Dest::W, quad(&ee, o + obj::UP)), o + obj::UP);
    same("start turn", port.bones[0].reference, b + body_at::START_TURN);
    let d = bodies_differ(&port.bones[0].body, &ee_body(&ee, b));
    assert!(d.is_empty(), "{name}: the placed body: {d:#?}");
    assert_eq!(node_rows(&port.objects[0].node), ee_rows(&ee, node), "{name}: the placed node");
    // The pin.
    assert_eq!(ee.u32_at(c + coll::CONSTRAINTS), 1, "{name}: one constraint");
    let p = ee.u32_at(ee.u32_at(c + coll::CONSTRAINTS + 8));
    let j = &port.joints[0];
    assert_eq!(j.on, ee.u32_at(p + pin_at::ON) != 0, "{name}: the pin's switch");
    assert_eq!(j.limited, ee.u32_at(p + pin_at::LIMITED) != 0, "{name}: ConstrainAngles");
    same("the body's anchor", j.pin.anchors[0], p + pin_at::ANCHOR1);
    same("the world's anchor", j.pin.anchors[1], p + pin_at::ANCHOR2);
    same("the lower limit", j.pin.lower, p + pin_at::LOWER);
    same("the upper limit", j.pin.upper, p + pin_at::UPPER);
    let edge = ee.u32_at(o + obj::EDGE);
    if edge != 0 {
        let lane = |a: u32, k: u32| F(ee.f32_at(edge + a + 4 * k));
        port.edge = Some(Edge { min: [lane(EDGE_MIN, 0), lane(EDGE_MIN, 2)], max: [lane(EDGE_MAX, 0), lane(EDGE_MAX, 2)] });
    }

    // The wake: the collection activated, the object enabled, and a spin.
    let before = node_listeners(&ee, world_node);
    ee.call(f::ACTIVATE, &[u64::from(c)], &[], BUDGET);
    ee.call(f::ENABLE, &[u64::from(o)], &[], BUDGET);
    port.activate();
    port.enable(0);
    let spin = Vf::new(2.5, 0.0, -3.0, 1.0);
    for (k, v) in spin.0.iter().enumerate() {
        ee.set_f32(b + body_at::RATES + 4 * k as u32, *v);
    }
    port.bones[0].body.rates = spin;
    let after = node_listeners(&ee, world_node);
    assert_eq!(after[..before.len()], before[..], "the node's listeners before the wake");
    assert_eq!(after[before.len()..], [c + coll::LISTENER, o + obj::LISTENER], "the wake's two listeners, in order");
    assert_eq!(port.listeners, [Listener::Settle, Listener::Object(0)], "the port's listeners");
    let d = bodies_differ(&port.bones[0].body, &ee_body(&ee, b));
    assert!(d.is_empty(), "{name}: the woken body: {d:#?}");
    assert_eq!(port.objects[0].fresh, ee.u32_at(o + obj::FRESH) != 0, "{name}: just woken");

    // Frame by frame.
    let dt = 1.0f32 / 30.0;
    let mut game_listeners = vec![(f::SETTLE, c), (f::LISTEN, o)];
    let (mut slept, mut counted, mut put_back, mut limit_hits) = (None, None, None, 0);
    let mut frame = 0;
    while frame < 3000 && !(game_listeners.is_empty() && port.listeners.is_empty()) {
        frame += 1;
        ee.call(f::UPDATE, &[u64::from(c)], &[dt], BUDGET);
        let mut i = 0;
        while i < game_listeners.len() {
            let (fun, this) = game_listeners[i];
            ee.call(fun, &[u64::from(this)], &[dt], BUDGET);
            if ee.v0() as u32 != 0 {
                game_listeners.remove(i);
            } else {
                i += 1;
            }
        }
        port.tick(F(dt), &tables);

        let at = |what: &str| format!("{name}, frame {frame}: {what}");
        let game = ee_body(&ee, b);
        let d = bodies_differ(&port.bones[0].body, &game);
        assert!(d.is_empty(), "{}: {d:#?}", at("the body"));
        assert_eq!(node_rows(&port.objects[0].node), ee_rows(&ee, node), "{}", at("the node"));
        assert_eq!(port.objects[0].countdown.to_bits(), ee.u32_at(o + obj::COUNTDOWN), "{}", at("the object's countdown"));
        assert_eq!(port.countdown.to_bits(), ee.u32_at(c + coll::COUNTDOWN), "{}", at("the collection's countdown"));
        assert_eq!(port.active, ee.u32_at(c + coll::FLAGS) & coll::ACTIVE != 0, "{}", at("active"));
        assert_eq!(port.objects[0].fresh, ee.u32_at(o + obj::FRESH) != 0, "{}", at("fresh"));
        let left: Vec<u32> = game_listeners.iter().map(|&(fun, _)| fun).collect();
        let ported: Vec<u32> =
            port.listeners.iter().map(|l| if *l == Listener::Settle { f::SETTLE } else { f::LISTEN }).collect();
        assert_eq!(ported, left, "{}", at("the listeners left"));
        if game.enabled && (0..3).any(|k| ee.f32_at(p + 0xa0 + 4 * k) != 0.0) {
            limit_hits += 1;
        }
        if !game.enabled && slept.is_none() {
            slept = Some(frame);
        }
        if slept.is_some() && counted.is_none() && ee.f32_at(o + obj::COUNTDOWN) < 5.0 {
            counted = Some(frame);
        }
        if put_back.is_none() && slept.is_some() && !game_listeners.iter().any(|&(fun, _)| fun == f::LISTEN) {
            put_back = Some(frame);
        }
    }
    assert!(game_listeners.is_empty(), "{name}: both listeners done in 3000 frames");
    let (slept, put_back) = (slept.expect("the sign went to sleep"), put_back.expect("the sign was put back"));
    assert!(counted.is_some(), "the countdown ran");
    assert!(limit_hits > 0, "the spin reached an angle limit");
    assert!(put_back > slept + 140, "{name}: put back at frame {put_back}, five seconds after sleeping at {slept}");
    assert!(!port.active, "{name}: the collection asleep again");
    eprintln!("{name}: limits hit on {limit_hits} frames, asleep at frame {slept}, put back at {put_back}, done at {frame}");
}

/// The node's bounds against the original's, over every node in the state's scene with children and
/// boxes under it: each node's children carried into its frame and merged.
#[test]
fn node_bounds_merges_the_childrens_boxes() {
    let Some(mut ee) = image() else { return };
    // The world node's own children, walked: every node with at least one child.
    fn tree(ee: &Ee, n: u32, depth: u32) -> BoundsNode {
        let own = (ee.u8_at(n + node_at::FLAGS) & node_at::HAS_BOX != 0)
            .then(|| NodeBox { centre: quad(ee, n + node_at::CENTRE), half_extent: quad(ee, n + node_at::HALF) });
        let mut children = Vec::new();
        let mut k = ee.u32_at(n + node_at::CHILDREN);
        while k != 0 && depth < 16 {
            children.push(tree(ee, k, depth + 1));
            k = ee.u32_at(k + node_at::NEXT);
        }
        BoundsNode { own, world: mat(ee, n + node_at::WORLD), children }
    }
    // Every scene node in the state with a child: the class the prop nodes are (its table at +0x10).
    let class = ee.u32_at(ee.u32_at(ee.u32_at(ee.u32_at(collections(&ee)[0] + coll::BODIES + 8)) + obj::NODE) + 0x10);
    let nodes: Vec<u32> = (0..0x0200_0000u32 - 0x200)
        .step_by(16)
        .filter(|&a| ee.u32_at(a + 0x10) == class && ee.u32_at(a + node_at::CHILDREN) != 0)
        .collect();
    let out = 0x7000_3000; // a stretch of the scratchpad the query does not use, for the two results
    let (mut merged, mut boxed) = (0, 0);
    for &n in &nodes {
        // The rows as the original's lazy rebuild leaves them, which is what its query reads.
        ee.call(f::NODE_BOUNDS, &[u64::from(n), u64::from(out), u64::from(out + 16)], &[], BUDGET);
        let game = NodeBox { centre: quad(&ee, out), half_extent: quad(&ee, out + 16) };
        let t = tree(&ee, n, 0);
        let port = prop_world::node_bounds(&t);
        assert_eq!(
            (port.centre.to_bits(), port.half_extent.to_bits()),
            (game.centre.to_bits(), game.half_extent.to_bits()),
            "node {n:#x}: the game {game:?}, the port {port:?}"
        );
        merged += 1;
        if t.children.iter().any(|c| c.own.is_some()) {
            boxed += 1;
        }
    }
    assert!(boxed > 0, "some node has a boxed child ({merged} with children)");
    eprintln!("node bounds: {merged} nodes with children, {boxed} with a boxed child");
}
