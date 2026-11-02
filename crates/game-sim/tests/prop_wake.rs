//! S200 — a prop's hit listener and its asleep probe, the port against the original's run in the
//! interpreter.
//!
//! A prop's ray probe carries a category list, which the gather's and the search's pair filter read:
//! every category as the prop's init leaves it, the wake mask alone once a body that detects collisions
//! asleep (`DetectCollisionsWhenDisabled`) is put to sleep, and every category again when a wake or the
//! countdown's put-back takes it back. First every free09 prop's list, through the enable, the
//! disable and the put-back, against the state's.
//!
//! Then the egg on its sculpture (`LargeEgg.col`) is woken and put straight back to sleep where it
//! lies, so its collision object stays filed with its rays searching among the wake mask's categories
//! only. The sculpture is a scenery mesh (category 0x220): frame after frame its rays meet nothing, and
//! it sleeps on. Then the sculpture is made a wake-category object — the original's mesh given the
//! toys' category (0x48) in RAM, the port's made a toy (a still one: [`Prop::moving`] at rest), which is
//! what a toy is in the port's prop world — and on the next frame the egg's rays strike it: the hit
//! listener wakes the body and the collection's listener activates the collection. The egg settles on
//! the sculpture and goes to sleep, and from then on the sculpture under it wakes it in every frame's
//! search and its listener puts it back to sleep in the same frame: it creeps on, and its countdown,
//! set afresh at each sleep, never runs down to the put-back. Every frame the body,
//! the record's flags, the probe's category list, the collection's active bit and the node's
//! listeners are compared, bit for bit.
//!
//! Skipped with a line where the disc, the kit or the state is not there.

use std::path::PathBuf;

use game_render::disc::{Disc, Res};
use game_sim::biplane::{Area, Body};
use game_sim::contact::Node;
use game_sim::phy::Ini;
use game_sim::prop_set::PropSet;
use game_sim::prop_world::Listener;
use game_sim::track::{Scene, Terrain};
use game_sim::traffic::Moving;
use game_sim::world::{Ground, Props};
use game_sim::world_search::Categories;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "first_recording.p2m2_SaveState.eeMemory.bin";
const BUDGET: u64 = 50_000_000;
/// The `.toy` actor's collection list {count, head}; cells {data, +4, +8 next}.
const LIST: u32 = 0x011b_43f4;
/// The terrain's area, as `free09.scn` lays it.
const FREE09: (f32, f32, f32, f32) = (3.0, 3.0, 9.0, 9.0);
/// Where the egg lies on its sculpture.
const EGG: [f32; 3] = [3244.70, 143.67, 3977.15];
/// The sculpture's mesh collidable in the state: the egg's nearest candidate once it searches awake.
const SCULPTURE: u32 = 0x013b_7110;
/// The toys' category (the trailers' meshes in this state), which has the wake mask's bit.
const TOY: u32 = 0x48;

mod coll {
    pub const FLAGS: u32 = 0x20;
    pub const ACTIVE: u32 = 2;
    pub const NAME: u32 = 0x144;
    pub const LISTENER: u32 = 0x34;
    pub const NODE: u32 = 0x248;
    pub const BODIES: u32 = 0x320;
}

mod obj {
    pub const LISTENER: u32 = 0x50;
    pub const DETECT: u32 = 0x6c;
    pub const BODY: u32 = 0x110;
    pub const COLLISION: u32 = 0x120;
    pub const RAY_PROBE: u32 = 0x124;
    pub const FRESH: u32 = 0x1b0;
    pub const COUNTDOWN: u32 = 0x1bc;
}

/// The ray probe's category list.
mod probe {
    pub const MODE: u32 = 0xe8;
    pub const LIST: u32 = 0xec;
    pub const COUNT: u32 = 0xf0;
}

/// A collidable's category, and where it is.
const CATEGORY: u32 = 0x84;
const CENTRE: u32 = 0x40;

mod rec {
    pub const VTABLE: u32 = 0x08;
    pub const TOUCHING: u32 = 0x40;
    pub const TOUCHING_OTHER: u32 = 0x44;
    pub const ON_GROUND: u32 = 0x48;
    pub const ON_OBJECT: u32 = 0x4c;
    pub const CLEAR: u32 = 0x74;
    pub const COLLIDE: u32 = 0x7c;
}

/// The world node's listener list {count, +8 items}.
const NODE_LISTENERS: u32 = 0x48;

mod body_at {
    pub const ENABLED: u32 = 0x00;
    pub const POSITION: u32 = 0x60;
    pub const VELOCITY: u32 = 0x70;
    pub const QUAT: u32 = 0x170;
    pub const RATES: u32 = 0x1b0;
}

/// The original's functions.
mod f {
    pub const UPDATE: u32 = 0x003b_28a0;
    pub const SETTLE: u32 = 0x003b_2440;
    pub const ENABLE: u32 = 0x0015_b790;
    pub const DISABLE: u32 = 0x0015_b8c0;
    pub const LISTEN: u32 = 0x0015_b2a0;
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

/// A ray probe's category list as the state has it.
fn ee_categories(ee: &Ee, p: u32) -> Categories {
    let (list, n) = (ee.u32_at(p + probe::LIST), ee.u32_at(p + probe::COUNT));
    Categories { mode: ee.u32_at(p + probe::MODE), list: (0..n).map(|i| ee.u32_at(list + 4 * i)).collect() }
}

fn port_categories(set: &PropSet, c: usize, k: usize) -> Categories {
    set.collections[c].objects[k].collision.as_ref().expect("a collision object").categories.clone()
}

/// free09 off the disc: the props as the ride loads them, the terrain's bytes and the scene's
/// collision meshes.
struct Free09 {
    set: PropSet,
    terrain: Vec<u8>,
    scene: Scene,
    props: Props,
}

fn free09() -> Option<Free09> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let names: Vec<String> = ["Tracks\\free09.res", "Tracks\\free09_2.res"]
        .iter()
        .filter_map(|want| disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case(want)).map(|e| e.name.clone()))
        .collect();
    let archives: Vec<Res> = names.iter().map(|n| disc.res(n).expect("a free09 archive")).collect();
    let res = &archives[0];
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
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain = res.read_suffix(&leaf).expect("free09's terrain");
    let (props, _) = Props::load(&scene, |want| archives.iter().find_map(|r| r.read_suffix(want).ok()).ok_or_else(|| "no archive of this track has it".to_string()));
    Some(Free09 { set, terrain, scene, props })
}

/// Every free09 prop's ray probe's category list as the loader leaves it, then through the enable,
/// the disable and the countdown's put-back, against the original's.
#[test]
fn every_props_probe_category_list_follows_the_originals() {
    let Some(mut ee) = image() else { return };
    let Some(mut free) = free09() else { return };
    let game = collections(&ee);
    assert_eq!(free.set.collections.len(), game.len(), "the collection count");
    let dt = 1.0f32 / 30.0;
    let (mut probed, mut detecting) = (0, 0);
    for (ci, &c) in game.iter().enumerate() {
        let name = text(&ee, c + coll::NAME);
        let n = free.set.collections[ci].objects.len() as u32;
        for k in 0..n {
            let at = |what: &str| format!("{name} object {k}: {what}");
            let o = ee.u32_at(ee.u32_at(c + coll::BODIES + 8) + 4 * k);
            let p = ee.u32_at(o + obj::RAY_PROBE);
            let detect = ee.u32_at(o + obj::DETECT) != 0;
            let ki = k as usize;
            assert_eq!(free.set.collections[ci].objects[ki].detect_when_disabled, detect, "{}", at("DetectCollisionsWhenDisabled"));
            let has_probe = free.set.collections[ci].objects[ki].collision.as_ref().is_some_and(|col| col.record.rays.is_some());
            assert_eq!(has_probe, p != 0, "{}", at("a ray probe"));
            let check = |ee: &Ee, set: &PropSet, when: &str| {
                if p != 0 {
                    assert_eq!(port_categories(set, ci, ki), ee_categories(ee, p), "{}", at(when));
                }
            };
            check(&ee, &free.set, "as loaded");
            ee.call(f::ENABLE, &[u64::from(o)], &[], BUDGET);
            free.set.collections[ci].enable(ki);
            check(&ee, &free.set, "enabled");
            ee.call(f::DISABLE, &[u64::from(o)], &[], BUDGET);
            free.set.collections[ci].disable(ki);
            check(&ee, &free.set, "disabled");
            if p != 0 && detect {
                assert_eq!(port_categories(&free.set, ci, ki), Categories { mode: 0, list: vec![0x40] }, "{}", at("asleep, the wake mask"));
            }
            // The countdown runs out: the object listener puts it back and unfiles it.
            ee.set_f32(o + obj::COUNTDOWN, 0.0);
            free.set.collections[ci].objects[ki].countdown = F(0.0);
            ee.call(f::LISTEN, &[u64::from(o)], &[dt], BUDGET);
            assert_ne!(ee.v0() as u32, 0, "{}", at("the game's listener is done"));
            free.set.collections[ci].step(F(dt), free.set.tables.as_ref().expect("the tables"), &[]);
            assert!(!free.set.collections[ci].listeners.contains(&Listener::Object(ki)), "{}", at("the port's listener is done"));
            assert!(!free.set.collections[ci].objects[ki].filed, "{}", at("put back, unfiled"));
            check(&ee, &free.set, "put back");
            if p != 0 {
                probed += 1;
                detecting += usize::from(detect);
            }
        }
    }
    assert!(probed >= 30 && detecting >= 1, "{probed} ray probes, {detecting} detecting asleep");
}

/// The node's listeners that are this collection's (`c`) and its object's (`o`), in the node's order.
fn ee_listeners(ee: &Ee, node: u32, c: u32, o: u32) -> Vec<Listener> {
    let n = ee.u32_at(node + NODE_LISTENERS);
    let items = ee.u32_at(node + NODE_LISTENERS + 8);
    (0..n)
        .map(|i| ee.u32_at(items + 4 * i))
        .filter_map(|l| {
            if l == c + coll::LISTENER {
                Some(Listener::Settle)
            } else if l == o + obj::LISTENER {
                Some(Listener::Object(0))
            } else {
                None
            }
        })
        .collect()
}

/// The node's listeners run, as its update runs them: each in order, taken off when it says so.
fn run_listeners(ee: &mut Ee, node: u32, c: u32, o: u32, dt: f32) {
    let mut i = 0;
    while i < ee.u32_at(node + NODE_LISTENERS) {
        let items = ee.u32_at(node + NODE_LISTENERS + 8);
        let l = ee.u32_at(items + 4 * i);
        let call = if l == c + coll::LISTENER {
            Some((f::SETTLE, c))
        } else if l == o + obj::LISTENER {
            Some((f::LISTEN, o))
        } else {
            None
        };
        let done = call.is_some_and(|(fun, this)| {
            ee.call(fun, &[u64::from(this)], &[dt], BUDGET);
            ee.v0() as u32 != 0
        });
        if done {
            let n = ee.u32_at(node + NODE_LISTENERS);
            for j in i..n - 1 {
                let next = ee.u32_at(items + 4 * (j + 1));
                ee.set_u32(items + 4 * j, next);
            }
            ee.set_u32(node + NODE_LISTENERS, n - 1);
        } else {
            i += 1;
        }
    }
}

fn node_rows(n: &Node) -> [[u32; 4]; 4] {
    [n.rows[0].to_bits(), n.rows[1].to_bits(), n.rows[2].to_bits(), n.origin.to_bits()]
}

fn ee_rows(ee: &Ee, node: u32) -> [[u32; 4]; 4] {
    [0, 1, 2, 3].map(|k| quad(ee, node + 0x80 + 16 * k).to_bits())
}

fn same_body(at: &str, got: &Body, ee: &Ee, b: u32) {
    assert_eq!(got.enabled, ee.u32_at(b + body_at::ENABLED) != 0, "{at}: enabled");
    for (name, g, a) in [
        ("position", got.position, body_at::POSITION),
        ("velocity", got.velocity, body_at::VELOCITY),
        ("quat", got.quat, body_at::QUAT),
        ("rates", got.rates, body_at::RATES),
    ] {
        let w = quad(ee, b + a);
        assert_eq!(g.to_bits(), w.to_bits(), "{at}: {name}: the game {:?}, the port {:?}", w.0, g.0);
    }
}

/// The egg put to sleep on its sculpture, its rays searching among the wake mask's categories: on a
/// scenery mesh it sleeps on; on a wake-category one its hit listener wakes it, and it keeps waking.
#[test]
fn a_wake_category_object_under_an_asleep_egg_wakes_it_as_the_originals() {
    let Some(mut ee) = image() else { return };
    let Some(mut free) = free09() else { return };
    let terrain = Terrain::parse(&free.terrain).expect("free09's terrain");
    let ground = Ground::new(&terrain, &free.scene.terrain);
    let field = ground.field();
    let game = collections(&ee);
    let pos = |ee: &Ee, c: u32| {
        let o = ee.u32_at(ee.u32_at(c + coll::BODIES + 8));
        quad(ee, ee.u32_at(o + obj::BODY) + body_at::POSITION)
    };
    let dist = |v: Vf, to: [f32; 3]| (0..3).map(|k| (v[k] - to[k]).powi(2)).sum::<f32>().sqrt();
    let (ci, &c) = game.iter().enumerate().min_by(|a, b| dist(pos(&ee, *a.1), EGG).total_cmp(&dist(pos(&ee, *b.1), EGG))).unwrap();
    let name = text(&ee, c + coll::NAME);
    assert!(dist(pos(&ee, c), EGG) < 1.0, "{name} is not the egg");
    let o = ee.u32_at(ee.u32_at(c + coll::BODIES + 8));
    let (b, r, p) = (ee.u32_at(o + obj::BODY), ee.u32_at(o + obj::COLLISION), ee.u32_at(o + obj::RAY_PROBE));
    let node = ee.u32_at(c + coll::NODE);
    let model = ee.u32_at(o + 0x13c);
    assert_ne!(ee.u32_at(o + obj::DETECT), 0, "{name}: DetectCollisionsWhenDisabled");
    assert_eq!(ee.u32_at(SCULPTURE + CATEGORY), 0x220, "the sculpture's mesh is scenery");
    let sculpture = quad(&ee, SCULPTURE + CENTRE);
    assert!(dist(sculpture, EGG) < 25.0, "the sculpture is under the egg");

    // Woken and put to sleep where it lies: filed, its rays among the wake mask's categories.
    ee.call(f::ENABLE, &[u64::from(o)], &[], BUDGET);
    ee.call(f::DISABLE, &[u64::from(o)], &[], BUDGET);
    free.set.collections[ci].enable(0);
    free.set.collections[ci].disable(0);
    assert_eq!(port_categories(&free.set, ci, 0), ee_categories(&ee, p), "{name}: asleep");
    assert!(free.set.collections[ci].objects[0].filed, "{name}: asleep, filed");

    let vt = ee.u32_at(r + rec::VTABLE);
    let (clear, collide) = (ee.u32_at(vt + rec::CLEAR), ee.u32_at(vt + rec::COLLIDE));
    let dt = 1.0f32 / 30.0;
    let (mut wakes, mut sleeps, mut was_awake, mut active_frames) = (0, 0, false, 0);
    for frame in 1..=600 {
        if frame == 31 {
            // The sculpture made a wake-category object: the toys' category in the game, a still toy in
            // the port.
            ee.set_u32(SCULPTURE + CATEGORY, TOY);
            let k = (0..free.props.props.len())
                .min_by(|&i, &j| dist(free.props.props[i].centre, [sculpture[0], sculpture[1], sculpture[2]]).total_cmp(&dist(free.props.props[j].centre, [sculpture[0], sculpture[1], sculpture[2]])))
                .unwrap();
            let prop = &mut free.props.props[k];
            prop.moving = Some(Moving { pose: prop.to_world, velocity: Vf::ZERO, spin: Vf::ZERO });
        }
        ee.call(clear, &[u64::from(r)], &[], BUDGET);
        ee.call(collide, &[u64::from(r)], &[dt], BUDGET);
        ee.call(f::UPDATE, &[u64::from(c)], &[dt], BUDGET);
        run_listeners(&mut ee, node, c, o, dt);
        free.set.frame_among(F(dt), Some(&field), Some(&free.props), None);

        let at = format!("{name}, frame {frame}");
        let port = &free.set.collections[ci];
        let ob = &port.objects[0];
        let col = ob.collision.as_ref().unwrap();
        same_body(&at, &port.bones[0].body, &ee, b);
        assert_eq!(node_rows(&ob.node), ee_rows(&ee, model), "{at}: the node");
        let flag = |a: u32| ee.u32_at(r + a) != 0;
        let rc = &col.record;
        assert_eq!(
            [rc.touching, rc.touching_other, rc.on_ground, rc.on_object],
            [flag(rec::TOUCHING), flag(rec::TOUCHING_OTHER), flag(rec::ON_GROUND), flag(rec::ON_OBJECT)],
            "{at}: touching, touching another, on the ground, on an object"
        );
        assert_eq!(col.categories, ee_categories(&ee, p), "{at}: the probe's category list");
        assert_eq!(ob.countdown.to_bits(), ee.u32_at(o + obj::COUNTDOWN), "{at}: the countdown");
        assert_eq!(ob.fresh, ee.u32_at(o + obj::FRESH) != 0, "{at}: fresh");
        assert_eq!(port.active, ee.u32_at(c + coll::FLAGS) & coll::ACTIVE != 0, "{at}: active");
        assert_eq!(port.listeners, ee_listeners(&ee, node, c, o), "{at}: the node's listeners");
        let awake = port.bones[0].body.enabled;
        if frame <= 30 {
            assert!(!awake && !port.active, "{at}: on a scenery mesh it sleeps on");
        }
        if awake && !was_awake {
            wakes += 1;
        }
        if !awake && was_awake {
            sleeps += 1;
        }
        active_frames += usize::from(port.active);
        was_awake = awake;
    }
    eprintln!("{name}: awake at the end of a frame {wakes} times, asleep again {sleeps}, active {active_frames} frames");
    assert!(wakes >= 2 && sleeps >= 1, "the sculpture wakes the egg, and wakes it again once it sleeps: {wakes} wakes, {sleeps} sleeps");
    let ob = &free.set.collections[ci].objects[0];
    assert!(!was_awake && ob.filed, "{name}: asleep at the end, still searching");
    assert_eq!(ob.countdown, ob.reset_time, "{name}: woken and put to sleep every frame, its countdown never runs");
}
