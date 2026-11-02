//! S195 — free09's beach balls knocked by the bike in PCSX2, and the port's props tick replayed
//! against the capture.
//!
//! research/s195/aim.py puts the human's bike (first_recording's state, free09) in line with a beach
//! ball and sets it rolling at it with no pad; research/s195/knock.py records watch/s195-knock.toml (the
//! five balls' bodies, node origins, record flags and countdowns, a row a tick) and saves states around
//! the strike. The replay starts from a state saved after the strike, the woken balls' collections
//! read out of the state's RAM into the port's [`PropSet`], and from there every tick the port's
//! [`PropSet::frame_among`] over the disc's terrain and scene is compared bit for bit with the
//! original's rows: the flight, the landing, the bounces and the roll, asleep, the countdown, and the
//! put-back.
//!
//! S197's strike, the bike's body rays on ball 2's `f.col` ([`prop_contact`]), is proved on the tick
//! from knock-2882 to knock-2883: the bike's entry and the ball's mirror, the push both ways in the
//! bike's first substep (the original's props node ticked with its resolve hooked), and the ball's
//! body after the props' step.
//!
//! Skipped with a line where the disc, the kit, the state or the capture is not there (they are in s3's
//! context only).

use std::collections::HashMap;
use std::path::PathBuf;

use game_render::disc::{Disc, Res};
use game_sim::bike_col::{self, BikeCol};
use game_sim::biplane::{Area, Body};
use game_sim::contact::{Contact, Node, Other, Ridden, Side, Vehicle};
use game_sim::math::Mat4;
use game_sim::phy::Phy;
use game_sim::prop_contact;
use game_sim::ride::Tables;
use game_sim::rider_contact;
use game_sim::phy::Ini;
use game_sim::prop_set::PropSet;
use game_sim::prop_world::Listener;
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Ground, Props};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

/// The `.toy` actor's collection list (tests/prop_collision.rs).
const LIST: u32 = 0x011b_43f4;

/// free09's terrain: three root cells by three, nine feet a sample.
const FREE09: (f32, f32, f32, f32) = (3.0, 3.0, 9.0, 9.0);

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
    pub const BODY: u32 = 0x110;
    pub const COLLISION: u32 = 0x120;
    pub const NODE: u32 = 0x13c;
    pub const FRESH: u32 = 0x1b0;
    pub const COUNTDOWN: u32 = 0x1bc;
}

mod rec {
    pub const TOUCHING: u32 = 0x40;
    pub const TOUCHING_OTHER: u32 = 0x44;
    pub const ON_GROUND: u32 = 0x48;
    pub const ON_OBJECT: u32 = 0x4c;
    pub const COFG_WORLD: u32 = 0xa0;
    pub const SWEPT_FROM: u32 = 0xd0;
    pub const SWEPT_TO: u32 = 0xe0;
    pub const NORMAL_SPEED: u32 = 0x1d8;
    pub const RELATIVE: u32 = 0x1e0;
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

fn context(kit: &Kit, rel: &str) -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context").join(rel);
    if here.exists() { here } else { kit.ctx(rel) }
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|k| ee.u32_at(a + k)))
}

fn text(ee: &Ee, at: u32) -> String {
    (0..256).map(|k| ee.u8_at(at + k)).take_while(|&b| b != 0).map(|b| b as char).collect()
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

fn collections(ee: &Ee) -> Vec<u32> {
    let mut out = Vec::new();
    let mut cell = ee.u32_at(LIST + 4);
    while cell != 0 {
        out.push(ee.u32_at(cell));
        cell = ee.u32_at(cell + 8);
    }
    out
}

/// A capture's rows, column by name. Floats are written as the shortest decimal of the f32, so they
/// parse back to its bits.
struct Rows {
    names: HashMap<String, usize>,
    rows: Vec<Vec<String>>,
}

impl Rows {
    fn load(path: &PathBuf) -> Option<Rows> {
        let text = std::fs::read_to_string(path).ok()?;
        let mut lines = text.lines();
        let names = lines.next()?.split(',').enumerate().map(|(k, n)| (n.to_string(), k)).collect();
        let rows = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
        Some(Rows { names, rows })
    }

    fn get(&self, r: usize, name: &str) -> &str {
        &self.rows[r][*self.names.get(name).unwrap_or_else(|| panic!("no column {name}"))]
    }

    fn u32(&self, r: usize, name: &str) -> u32 {
        self.get(r, name).parse().unwrap()
    }

    fn f32(&self, r: usize, name: &str) -> f32 {
        self.get(r, name).parse().unwrap()
    }

    fn quad(&self, r: usize, name: &str) -> Vf {
        Vf::new(self.f32(r, &format!("{name}.x")), self.f32(r, &format!("{name}.y")), self.f32(r, &format!("{name}.z")), self.f32(r, &format!("{name}.w")))
    }

    /// The record's four flags, `bytes:16` in hex: touching, touching another, on the ground, on an
    /// object.
    fn flags(&self, r: usize, name: &str) -> [bool; 4] {
        let h = self.get(r, name);
        core::array::from_fn(|k| &h[8 * k..8 * k + 2] != "00")
    }
}

/// free09 off the disc: the props as the ride loads them, the terrain's bytes and the scene.
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
    let (props, _) =
        Props::load(&scene, |want| archives.iter().find_map(|r| r.read_suffix(want).ok()).ok_or_else(|| "no archive of this track has it".to_string()));
    Some(Free09 { set, terrain, scene, props })
}

fn node_origin(n: &Node) -> Vf {
    n.origin
}

/// The knocked collections as the state left them, read into the port's: every collection whose
/// active bit is set is woken the port's way (its settle listener and its body's object listener on,
/// the collision object filed), then the body, the node, the countdowns, the fresh flag and the
/// collision record's moving parts are the state's. Answers the woken collections' indices.
fn seed(ee: &Ee, set: &mut PropSet) -> Vec<usize> {
    let game = collections(ee);
    assert_eq!(game.len(), set.collections.len(), "the state's collections");
    let mut woken = Vec::new();
    for (ci, &c) in game.iter().enumerate() {
        if ee.u32_at(c + coll::FLAGS) & coll::ACTIVE == 0 {
            continue;
        }
        let name = text(ee, c + coll::NAME);
        assert_eq!(ee.u32_at(c + coll::BODIES), 1, "{name}: one body");
        let o = ee.u32_at(ee.u32_at(c + coll::BODIES + 8));
        let (b, node, r) = (ee.u32_at(o + obj::BODY), ee.u32_at(o + obj::NODE), ee.u32_at(o + obj::COLLISION));
        set.wake(ci, 0);
        let port = &mut set.collections[ci];
        // The node's listeners as the state has them: the collection's settle and the object's own.
        let world_node = ee.u32_at(c + coll::NODE);
        let n = ee.u32_at(world_node + 0x48);
        let at = ee.u32_at(world_node + 0x48 + 8);
        let on: Vec<u32> = (0..n).map(|k| ee.u32_at(at + 4 * k)).collect();
        port.listeners = on
            .iter()
            .filter_map(|&l| {
                if l == c + coll::LISTENER {
                    Some(Listener::Settle)
                } else if l == o + obj::LISTENER {
                    Some(Listener::Object(0))
                } else {
                    None
                }
            })
            .collect();
        eprintln!("{name}: the node's listeners {:?}", port.listeners);
        assert!(port.listeners.contains(&Listener::Settle), "{name}: the settle listener is on");
        let body = ee_body(ee, b);
        if !body.enabled {
            port.disable(0);
        }
        port.bones[0].body = body;
        let ob = &mut port.objects[0];
        ob.node.rows = [0, 1, 2].map(|k| quad(ee, node + 0x80 + 16 * k));
        ob.node.origin = quad(ee, node + 0xb0);
        ob.countdown = F(ee.f32_at(o + obj::COUNTDOWN));
        ob.fresh = ee.u32_at(o + obj::FRESH) != 0;
        let col = ob.collision.as_mut().expect("a ball has a collision object");
        col.collider.cofg_world = quad(ee, r + rec::COFG_WORLD);
        col.collider.normal_speed = F(ee.f32_at(r + rec::NORMAL_SPEED));
        col.collider.relative = quad(ee, r + rec::RELATIVE);
        let rc = &mut col.record;
        rc.touching = ee.u32_at(r + rec::TOUCHING) != 0;
        rc.touching_other = ee.u32_at(r + rec::TOUCHING_OTHER) != 0;
        rc.on_ground = ee.u32_at(r + rec::ON_GROUND) != 0;
        rc.on_object = ee.u32_at(r + rec::ON_OBJECT) != 0;
        rc.swept_from = quad(ee, r + rec::SWEPT_FROM);
        rc.swept_to = quad(ee, r + rec::SWEPT_TO);
        woken.push(ci);
    }
    woken
}

/// What one ball's replay came to.
#[derive(Debug, Default)]
struct Run {
    /// The capture's row the state is (its body equal bit for bit), or the row it was set from.
    from: usize,
    /// The last row the port agrees with on every compared lane, and the first lane that parts after it.
    agreed: usize,
    parted: Option<String>,
    /// Rows the game's record reads all clear and the port's does not, the bodies equal (below).
    clear: usize,
    /// Rows that read the ball a tick ahead ([`ahead`]), skipped.
    ahead: usize,
    landed: Option<usize>,
    slept: Option<usize>,
    put_back: Option<usize>,
    /// The run before a [`resync`], if there was one.
    before: Option<Box<Run>>,
}

/// The rows' ball `k` against the port's collection `ci`: the body's lanes, then the record's flags,
/// what differs (empty when nothing does).
fn differ(rows: &Rows, r: usize, k: usize, set: &PropSet, ci: usize) -> (Vec<String>, Option<String>) {
    let c = &set.collections[ci];
    let b = &c.bones[0].body;
    let o = &c.objects[0];
    let p = |n: &str| format!("b{k}_{n}");
    let mut out = Vec::new();
    for (name, got) in [
        ("last_pos", b.last_position),
        ("pos", b.position),
        ("vel", b.velocity),
        ("last_quat", b.last_quat),
        ("quat", b.quat),
        ("rates", b.rates),
        ("origin", node_origin(&o.node)),
    ] {
        let want = rows.quad(r, &p(name));
        if got.to_bits() != want.to_bits() {
            out.push(format!("{name}: the game {:?}, the port {:?}", want.0, got.0));
        }
    }
    if b.enabled != (rows.u32(r, &p("en")) != 0) {
        out.push(format!("enabled: the game {}, the port {}", rows.u32(r, &p("en")), b.enabled));
    }
    if c.active != (rows.u32(r, &p("active")) & 2 != 0) {
        out.push(format!("active: the game {}, the port {}", rows.u32(r, &p("active")), c.active));
    }
    if o.fresh != (rows.u32(r, &p("fresh")) != 0) {
        out.push(format!("fresh: the game {}, the port {}", rows.u32(r, &p("fresh")), o.fresh));
    }
    if o.countdown.0.to_bits() != rows.f32(r, &p("countdown")).to_bits() {
        out.push(format!("countdown: the game {}, the port {}", rows.f32(r, &p("countdown")), o.countdown.0));
    }
    let rc = &o.collision.as_ref().unwrap().record;
    let flags = [rc.touching, rc.touching_other, rc.on_ground, rc.on_object];
    let want = rows.flags(r, &p("touch"));
    let flags = (flags != want).then(|| format!("touching, another, ground, object: the game {want:?}, the port {flags:?}"));
    (out, flags)
}

/// Ball `k` set from the capture's row `r`, where the game's has just gone to sleep: its body's lanes,
/// asleep, the object's countdown and the collection's active bit. For a ball the port has lost (the
/// bike struck it again, which the port's props tick does not see), to replay the countdown and the
/// put-back from the game's own resting place.
fn resync(rows: &Rows, r: usize, k: usize, set: &mut PropSet) {
    let p = |n: &str| format!("b{k}_{n}");
    assert_eq!(rows.u32(r, &p("en")), 0, "ball {k} is asleep in row {r}");
    let c = &mut set.collections[k];
    c.disable(0);
    let b = &mut c.bones[0].body;
    b.last_position = rows.quad(r, &p("last_pos"));
    b.position = rows.quad(r, &p("pos"));
    b.velocity = rows.quad(r, &p("vel"));
    b.last_quat = rows.quad(r, &p("last_quat"));
    b.quat = rows.quad(r, &p("quat"));
    b.rates = rows.quad(r, &p("rates"));
    c.objects[0].node.origin = rows.quad(r, &p("origin"));
    c.objects[0].countdown = F(rows.f32(r, &p("countdown")));
    c.objects[0].fresh = rows.u32(r, &p("fresh")) != 0;
    c.active = rows.u32(r, &p("active")) & 2 != 0;
    // Still active on the row it slept on, the collection's settle ran before the object's listener.
    c.listeners = if c.active { vec![Listener::Settle, Listener::Object(0)] } else { vec![Listener::Object(0)] };
    let rc = &mut c.objects[0].collision.as_mut().unwrap().record;
    [rc.touching, rc.touching_other, rc.on_ground, rc.on_object] = rows.flags(r, &p("touch"));
}

/// Whether row `r` holds ball `k` a tick ahead: every one of its columns as the next row has them
/// (the row read after the next frame had run, as a few frame_counter-stamped rows are).
fn ahead(rows: &Rows, r: usize, k: usize) -> bool {
    let pre = format!("b{k}_");
    r + 1 < rows.rows.len() && rows.names.iter().filter(|(n, _)| n.starts_with(&pre)).all(|(_, &i)| rows.rows[r][i] == rows.rows[r + 1][i])
}

/// The capture `name` replayed from its state saved at `frame`: the woken balls read out of the
/// state, then the port ticked a row at a time against the rows that follow, each ball until the
/// first row it parts on (or the capture ends). `balls` are the ball columns (0..4, the first five
/// collections, the five BeachBalls) to compare; each must be woken in the state. `again` names
/// (ball, frame) pairs to [`resync`] before that frame's tick, and compare again from.
///
/// The record's flags: a row is read once the game's frame has run, but a few rows read the record
/// after the next tick's clear (the collision pass is clear, search, step; the rows that read all
/// clear come in runs while a ball is pressed hard into the ground, e.g. the edge's), so where the
/// game's four read clear and the bodies agree bit for bit the row counts as agreed (`clear`).
fn replay(name: &str, frame: u32, balls: &[usize], again: &[(usize, u32)]) -> Option<(Rows, Vec<Run>)> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let state = context(&kit, &format!("states/s195-{name}-{frame}.eeMemory.bin"));
    let csv = context(&kit, &format!("capture/s195/{name}.csv"));
    let Some(rows) = Rows::load(&csv) else {
        skip(&format!("no {} (S195's capture, s3's context only)", csv.display()));
        return None;
    };
    let ee = match Ee::from_ram(&state) {
        Ok(ee) => ee,
        Err(why) => {
            skip(&format!("{why} — research/s195/slots.py ram {name}-{frame} writes it"));
            return None;
        }
    };
    let mut free = free09()?;
    let terrain = Terrain::parse(&free.terrain).expect("free09's terrain");
    let ground = Ground::new(&terrain, &free.scene.terrain);
    let field = ground.field();
    let woken = seed(&ee, &mut free.set);
    for &k in balls {
        assert!(woken.contains(&k), "ball {k} is not awake in {name}-{frame} (woken: {woken:?})");
        assert_eq!(free.set.collections[k].objects.len(), 1);
    }

    // The row the state is: the first ball's body, bit for bit.
    let k0 = balls[0];
    let from = (0..rows.rows.len())
        .find(|&r| differ(&rows, r, k0, &free.set, k0).0.is_empty())
        .unwrap_or_else(|| panic!("no row of {name} is the state's ball {k0}"));
    eprintln!("{name}-{frame}: the state is row {from} (frame {}); woken {woken:?}", rows.u32(from, "frame_counter"));
    let mut runs: Vec<Run> = balls.iter().map(|_| Run { from, agreed: from, ..Run::default() }).collect();
    let mut airborne = vec![false; balls.len()];
    for r in from + 1..rows.rows.len() {
        for &(k, f) in again {
            if rows.u32(r, "frame_counter") == f {
                // The tick into row r is the one the state after row r - 1 makes.
                resync(&rows, r - 1, k, &mut free.set);
                let i = balls.iter().position(|&b| b == k).expect("a resynced ball is compared");
                let before = std::mem::take(&mut runs[i]);
                runs[i] = Run { from: r - 1, agreed: r - 1, before: Some(Box::new(before)), ..Run::default() };
            }
        }
        let dt = F(rows.f32(r, "dt"));
        free.set.frame_among(dt, Some(&field), Some(&free.props), None);
        for (i, &k) in balls.iter().enumerate() {
            let run = &mut runs[i];
            if run.parted.is_some() {
                continue;
            }
            let (d, flags) = differ(&rows, r, k, &free.set, k);
            if d.is_empty() && flags.is_some() && rows.flags(r, &format!("b{k}_touch")) == [false; 4] {
                run.clear += 1;
            } else if (!d.is_empty() || flags.is_some()) && ahead(&rows, r, k) {
                run.ahead += 1;
                continue;
            } else if !d.is_empty() || flags.is_some() {
                let d: Vec<String> = d.into_iter().chain(flags).collect();
                run.parted = Some(format!("row {r} (frame {}): {d:#?}", rows.u32(r, "frame_counter")));
                continue;
            }
            run.agreed = r;
            let c = &free.set.collections[k];
            let rc = &c.objects[0].collision.as_ref().unwrap().record;
            if !rc.touching {
                airborne[i] = true;
            }
            if rc.on_ground && airborne[i] && run.landed.is_none() {
                run.landed = Some(r);
            }
            let b = &c.bones[0].body;
            if !b.enabled && run.slept.is_none() {
                run.slept = Some(r);
            }
            let home = c.objects[0].place;
            if !b.enabled && b.position.to_bits()[..3] == home.to_bits()[..3] && run.put_back.is_none() {
                run.put_back = Some(r);
            }
        }
    }
    for (run, k) in runs.iter().zip(balls) {
        eprintln!("ball {k}: {run:#?}");
    }
    Some((rows, runs))
}

/// The row of `frame`.
fn row_of(rows: &Rows, frame: u32) -> usize {
    (0..rows.rows.len()).find(|&r| rows.u32(r, "frame_counter") == frame).unwrap_or_else(|| panic!("no row of frame {frame}"))
}

/// The parting line of a run: the bike struck the ball again (the record touching another
/// object), which the port's props tick does not see.
fn struck_again(run: &Run, rows: &Rows, frame: u32) {
    let parted = run.parted.as_deref().unwrap_or_else(|| panic!("the run never parts: {run:#?}"));
    assert!(parted.starts_with(&format!("row {} (frame {frame})", row_of(rows, frame))), "{run:#?}");
    assert!(parted.contains("touching, another, ground, object: the game [true, true,"), "the bike's contact: {parted}");
}

/// The strike wakes the ball through its object's struck listener: the object's listener goes on the
/// world node's list first, then the collection's settle (the port's PropSet::wake files them the
/// other way; S197 brings the strike). Ball 2 asleep in the capture's first row, woken in the first
/// state knock.py saved (asked for with the bike 8 ft off, it landed on the strike's tick); ball 1,
/// nudged by the bike passing earlier, went on the same way.
#[test]
fn the_strike_puts_the_objects_listener_on_first() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let csv = context(&kit, "capture/s195/knock.csv");
    let Some(rows) = Rows::load(&csv) else {
        skip(&format!("no {} (S195's capture, s3's context only)", csv.display()));
        return;
    };
    let state = context(&kit, "states/s195-knock-2881.eeMemory.bin");
    let ee = match Ee::from_ram(&state) {
        Ok(ee) => ee,
        Err(why) => {
            skip(&format!("{why} — research/s195/slots.py ram knock-2881 writes it"));
            return;
        }
    };
    assert_eq!((rows.u32(0, "b2_en"), rows.u32(0, "b2_active")), (0, 0), "ball 2 asleep before the strike");
    let game = collections(&ee);
    for k in [1, 2] {
        let c = game[k];
        assert_eq!(text(&ee, c + coll::NAME), "BeachBall");
        assert!(ee.u32_at(c + coll::FLAGS) & coll::ACTIVE != 0, "ball {k} woken");
        let o = ee.u32_at(ee.u32_at(c + coll::BODIES + 8));
        let node = ee.u32_at(c + coll::NODE);
        let on: Vec<u32> = (0..ee.u32_at(node + 0x48)).map(|i| ee.u32_at(ee.u32_at(node + 0x50) + 4 * i)).collect();
        let at = |l: u32| on.iter().position(|&x| x == l).unwrap_or_else(|| panic!("ball {k}: {l:x} not on {on:x?}"));
        assert!(at(o + obj::LISTENER) < at(c + coll::LISTENER), "ball {k}: the object's listener before the settle: {on:x?}");
    }
}

/// Ball 2, struck at frame 2882, from the state two ticks after: its flight, the landing, the bounces
/// and the roll down the slope, bit for bit, until the bike touches it again at frame 3093. Ball 1,
/// nudged by the bike passing at frame 2865: its roll and its sleep at frame 3010 (the countdown
/// started, the collection put to sleep on the same tick), until the bike wakes it again at 3322; then,
/// set from the game's row where it sleeps again (frame 3380), the sixty seconds' countdown and the
/// put-back at frame 5181, and asleep at its place to the capture's end.
#[test]
fn the_knocked_balls_fly_land_sleep_and_go_back() {
    let Some((rows, runs)) = replay("knock", 2884, &[2, 1], &[(1, 3381)]) else { return };
    let (two, one) = (&runs[0], &runs[1]);
    assert_eq!(two.landed, Some(row_of(&rows, 2912)), "{two:#?}");
    assert_eq!(two.agreed, row_of(&rows, 3092), "{two:#?}");
    struck_again(two, &rows, 3093);

    let first = one.before.as_deref().expect("ball 1 set again");
    assert_eq!(first.slept, Some(row_of(&rows, 3010)), "{first:#?}");
    struck_again(first, &rows, 3322);
    assert_eq!(one.from, row_of(&rows, 3380));
    assert_eq!(one.put_back, Some(row_of(&rows, 5181)), "{one:#?}");
    assert!(one.parted.is_none(), "{one:#?}");
    assert_eq!(one.agreed, rows.rows.len() - 1);
}

/// The off-the-map edge reset: ball 2, knocked and then poked to x 6909.4 (research/s195/edge.py; no
/// free09 prop reaches the edge in play), pushed out of the edge's ground and over x 6912 at frame
/// 2939. From the state the tick before, the port disables it and puts it back at its place on that
/// tick, asleep with no countdown and its collection put to sleep, as the game does; and it stays
/// there with the game's until the bike, parked by its place, touches it at frame 2991.
#[test]
fn a_ball_over_the_edge_goes_back_on_the_tick() {
    let Some((rows, runs)) = replay("edge", 2937, &[2], &[]) else { return };
    let run = &runs[0];
    assert_eq!(run.from, row_of(&rows, 2938));
    assert!(rows.f32(run.from, "b2_pos.x") > 6911.0, "the ball at the edge");
    assert_eq!(run.put_back, Some(row_of(&rows, 2939)), "{run:#?}");
    assert_eq!(rows.f32(row_of(&rows, 2939), "b2_countdown"), -1.0);
    struck_again(run, &rows, 2991);
}

/// The human bike's ridden vehicle in a free09 state (watch/s195-knock.toml's chain off the game
/// manager): its collision object is `+0x7d0`, its chassis `+0x7a4`.
fn the_bike(ee: &Ee) -> u32 {
    [0x34, 0x34, 0x8, 0x8, 0x0].iter().fold(ee.u32_at(0x004e_1e98), |a, &h| ee.u32_at(a + h))
}

fn node_at(ee: &Ee, n: u32) -> Node {
    Node { rows: [0, 1, 2].map(|k| quad(ee, n + 0x80 + 16 * k)), origin: quad(ee, n + 0xb0) }
}

/// A list entry in the port's terms; the other side only as there or not (99).
fn entry_at(ee: &Ee, e: u32) -> Contact {
    Contact {
        kind: ee.u32_at(e),
        from: quad(ee, e + 0x10),
        on_body: quad(ee, e + 0x20),
        normal: quad(ee, e + 0x30),
        point: quad(ee, e + 0x40),
        arm: quad(ee, e + 0x50),
        other_arm: quad(ee, e + 0x60),
        surface: ee.u32_at(e + 0x70) as i32,
        other: (ee.u32_at(e + 0x74) != 0).then_some(99),
        mover: (ee.u32_at(e + 0x78) != 0).then_some(99),
    }
}

fn list_at(ee: &Ee, r: u32) -> Vec<Contact> {
    let (n, at) = (ee.u32_at(r + 0x100), ee.u32_at(r + 0x108));
    (0..n).map(|k| entry_at(ee, at + 0x80 * k)).collect()
}

/// A bike record's chassis as the other side of a contact (`+0xf0` the chassis, `+0xf8` its node).
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

/// The contact fields a search writes (the arms are the resolve's), by name, that differ.
fn searched_differ(port: &Contact, orig: &Contact) -> Vec<&'static str> {
    let mut out = Vec::new();
    for (what, p, o) in [("from", port.from, orig.from), ("on_body", port.on_body, orig.on_body), ("normal", port.normal, orig.normal), ("point", port.point, orig.point)] {
        if p.to_bits() != o.to_bits() {
            out.push(what);
        }
    }
    if port.kind != orig.kind {
        out.push("kind");
    }
    if port.surface != orig.surface {
        out.push("surface");
    }
    if port.other.is_some() != orig.other.is_some() {
        out.push("other");
    }
    out
}

/// The human's bike (`Race125P.phy`) and its two collision files, off the disc.
fn bike_files(tables: &Tables) -> Option<(Phy, BikeCol)> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
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
    let phy = bikes.iter().find_map(|b| disc.res(b).ok()?.read_suffix("race125p.phy").ok()).expect("race125p.phy in a bike archive");
    Some((Phy::read_bytes(&phy, &tables.curve), col))
}

/// The bike strikes ball 2 (S197): from knock-2882, one tick into knock-2883 the port's way — the
/// props' collide pass, the bike's body rays against the balls' `f.col` ([`prop_contact::strike`]),
/// the bike's resolve pushing the ball ([`rider_contact::Bump::resolve`], first substep), and the
/// props' step with the bike as the mirror's other side — against the state.
#[test]
fn the_bike_strikes_the_ball() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let load = |frame: u32| {
        let state = context(&kit, &format!("states/s195-knock-{frame}.eeMemory.bin"));
        Ee::from_ram(&state).map_err(|why| skip(&format!("{why} — research/s195/slots.py ram knock-{frame} writes it"))).ok()
    };
    let (Some(before), Some(after)) = (load(2882), load(2883)) else { return };
    let tables = Tables::build();
    let Some((phy, col)) = bike_files(&tables) else { return };
    let Some(mut free) = free09() else { return };
    let terrain = Terrain::parse(&free.terrain).expect("free09's terrain");
    let ground = Ground::new(&terrain, &free.scene.terrain);
    let field = ground.field();
    let woken = seed(&before, &mut free.set);
    assert!(woken.contains(&2), "ball 2 woken in knock-2882: {woken:?}");
    let rec = before.u32_at(the_bike(&before) + 0x7d0);
    assert_eq!(rec, after.u32_at(the_bike(&after) + 0x7d0));
    let at = {
        let n = node_at(&before, before.u32_at(rec + 0xf8));
        Mat4([n.rows[0], n.rows[1], n.rows[2], n.origin])
    };
    let dt = F(after.f32_at(addr("frame_dt").unwrap()));
    eprintln!("dt {:?} (before's {:?})", dt, before.f32_at(addr("frame_dt").unwrap()));

    let movers = free.set.search(dt, Some(&field), Some(&free.props), None);
    let strike = prop_contact::strike(&col, &at, &free.set, 0, 1, dt);
    let ball = |ee: &Ee| {
        let c = collections(ee)[2];
        ee.u32_at(ee.u32_at(c + coll::BODIES + 8))
    };
    let (want_bike, want_ball) = (list_at(&after, rec), list_at(&after, after.u32_at(ball(&after) + obj::COLLISION)));
    // The bike's one entry, on ball 2, and the ball's one, its mirror: the search's fields bit for bit.
    assert_eq!(strike.contacts.len(), want_bike.len(), "the bike's list\nport {:?}\nstate {want_bike:?}", strike.contacts);
    assert_eq!(strike.contacts.len(), 1);
    assert_eq!(strike.contacts[0].other, Some(1), "the entry names the first struck body, after the one player");
    assert_eq!(searched_differ(&strike.contacts[0], &want_bike[0]), Vec::<&str>::new());
    assert_eq!(strike.struck.len(), 1, "{:?}", strike.struck);
    let (c, o, mirrors) = &strike.struck[0];
    assert_eq!((*c, *o), (2, 0), "ball 2 struck");
    assert_eq!(mirrors.len(), want_ball.len(), "the ball's list");
    assert_eq!(mirrors[0].other, Some(0), "the mirror names the striker, player 1");
    assert_eq!(searched_differ(&mirrors[0], &want_ball[0]), Vec::<&str>::new());

    let pushed = free.set.strike(&strike);
    let collider = rider_contact::collider(&phy.collisions, phy.mass.mass);
    let mut bump = rider_contact::Bump { me: 0, contacts: strike.contacts.clone(), colliders: vec![collider], vehicles: vec![vehicle_at(&before, rec)], movers: Vec::new(), props: pushed };
    let mut own = ridden_at(&before, rec);
    bump.resolve(&mut own, true);
    free.set.pushed(&bump.props);
    let (mut bike_collider, mut bike) = (collider, vehicle_at(&after, rec));
    free.set.step(dt, &movers, &mut [Other { collider: &mut bike_collider, side: Side::Vehicle(&mut bike) }]);
    let got = &free.set.collections[2].bones[0].body;
    let want = ee_body(&after, after.u32_at(ball(&after) + obj::BODY));
    let mut bad = Vec::new();
    for (what, p, o) in [
        ("last position", got.last_position, want.last_position),
        ("position", got.position, want.position),
        ("velocity", got.velocity, want.velocity),
        ("last quat", got.last_quat, want.last_quat),
        ("quat", got.quat, want.quat),
        ("rates", got.rates, want.rates),
        ("shock", got.shock, want.shock),
        ("twist", got.twist, want.twist),
    ] {
        if p.to_bits() != o.to_bits() {
            bad.push(format!("{what}: port {:?} state {:?}", p.0, o.0));
        }
    }
    let r = after.u32_at(ball(&after) + obj::COLLISION);
    let rc = &free.set.collections[2].objects[0].collision.as_ref().unwrap().record;
    let flags = [rc.touching, rc.touching_other, rc.on_ground, rc.on_object];
    let want_flags = [rec::TOUCHING, rec::TOUCHING_OTHER, rec::ON_GROUND, rec::ON_OBJECT].map(|a| after.u32_at(r + a) != 0);
    if flags != want_flags {
        bad.push(format!("the record's flags: port {flags:?} state {want_flags:?}"));
    }
    // The mirror's arms are the ball's force listener's, its last substep's.
    let (p, o) = (&rc.contacts[0], &want_ball[0]);
    if p.arm.to_bits() != o.arm.to_bits() || p.other_arm.to_bits() != o.other_arm.to_bits() {
        bad.push(format!("the mirror's arms: port {:?} {:?} state {:?} {:?}", p.arm, p.other_arm, o.arm, o.other_arm));
    }
    assert!(bad.is_empty(), "ball 2 after the tick: {bad:#?}");
}

/// A strike wakes a sleeping body the way S195 read off the world node: the object's listener on
/// first, then the collection's settle ([`PropSet::struck`]); [`PropSet::wake`] files them the other
/// way round.
#[test]
fn a_strike_files_the_objects_listener_first() {
    let Some(mut free) = free09() else { return };
    let c = &free.set.collections[0];
    assert!(!c.active && !c.bones[0].body.enabled, "ball 0 asleep as loaded");
    free.set.struck(0, 0);
    let c = &free.set.collections[0];
    assert!(c.active && c.bones[0].body.enabled && c.objects[0].filed);
    assert_eq!(c.listeners, vec![Listener::Object(0), Listener::Settle]);
    free.set.wake(1, 0);
    assert_eq!(free.set.collections[1].listeners, vec![Listener::Settle, Listener::Object(0)]);
}

/// free09's props' world node in the knock states: its filed objects are the bikes' and the woken
/// props' collision objects, the human bike's ahead of the balls (oracle.txt, S197).
const PROPS_NODE: u32 = 0x00e1_7ef0;
/// The node's update (node, dt) and a collision object's resolve (object, first substep).
const NODE_UPDATE: u32 = 0x0026_f800;
const RESOLVE: u32 = 0x0027_0dc0;

/// Run the original's resolve from inside a hook on it, the hooked call's registers and stack put
/// back so the hook returns to its caller as the call would have (tests/two_p_world.rs's).
fn run_original(ee: &mut Ee, r: u32, first: u32) {
    let (ra, sp, stack) = (ee.r(31), ee.r(29), ee.stack);
    ee.stack = (sp as u32 - 0x400) & !0xf;
    ee.call(RESOLVE, &[u64::from(r), u64::from(first)], &[], 50_000_000);
    ee.stack = stack;
    ee.set(29, sp);
    ee.set(31, ra);
    ee.pc = RESOLVE;
}

/// The push both ways (S197): knock-2882's image ticked by the original's props node, the bike's
/// first-substep resolve hooked — the port's [`rider_contact::Bump::resolve`] on the list, the chassis
/// and ball 2's body as the call finds them, against what the original's resolve leaves: the bike's
/// chassis (velocity, rates, shock, twist, force, torque), the ball's body, and the entry's arms.
#[test]
fn the_bike_and_the_ball_push_each_other_as_the_original() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let state = context(&kit, "states/s195-knock-2882.eeMemory.bin");
    let Ok(mut ee) = Ee::from_ram(&state).map_err(|why| skip(&format!("{why} — research/s195/slots.py ram knock-2882 writes it"))) else { return };
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let tables = Tables::build();
    let Some((phy, _)) = bike_files(&tables) else { return };
    let Some(mut free) = free09() else { return };
    seed(&ee, &mut free.set);
    let collider = rider_contact::collider(&phy.collisions, phy.mass.mass);
    let ball = &free.set.collections[2];
    let (ball_collider, mass) = (ball.objects[0].collision.as_ref().unwrap().collider, ball.bones[0].mass);
    let rec = ee.u32_at(the_bike(&ee) + 0x7d0);
    let o = ee.u32_at(ee.u32_at(collections(&ee)[2] + coll::BODIES + 8));
    let body = ee.u32_at(o + obj::BODY);
    let filed: Vec<u32> = (0..ee.u32_at(PROPS_NODE + 0x30)).map(|k| ee.u32_at(ee.u32_at(PROPS_NODE + 0x38) + 4 * k)).collect();
    assert!(filed.contains(&rec) && filed.contains(&ee.u32_at(o + obj::COLLISION)), "the bike and ball 2 on the node: {filed:x?}");

    let bad: Rc<RefCell<Option<Vec<String>>>> = Rc::default();
    let seen = Rc::clone(&bad);
    ee.hooks.insert(
        RESOLVE,
        Box::new(move |ee: &mut Ee| {
            let (r, first) = (ee.r(4) as u32, ee.r(5) as u32);
            if r != rec || first == 0 || seen.borrow().is_some() {
                run_original(ee, r, first);
                return;
            }
            let list: Vec<Contact> = list_at(ee, rec).into_iter().map(|c| Contact { other: c.other.map(|_| 1), ..c }).collect();
            let pushed = prop_contact::Pushed { collection: 2, object: 0, collider: ball_collider, body: ee_body(ee, body), mass };
            let mut bump = rider_contact::Bump { me: 0, contacts: list, colliders: vec![collider], vehicles: vec![vehicle_at(ee, rec)], movers: Vec::new(), props: vec![pushed] };
            let mut own = ridden_at(ee, rec);
            bump.resolve(&mut own, true);
            run_original(ee, r, first);
            let got = ridden_at(ee, rec);
            let ball = ee_body(ee, body);
            let p = &bump.props[0].body;
            let mut bad = vec![format!("{} entries", bump.contacts.len())];
            for (what, port, orig) in [
                ("chassis velocity", own.chassis.velocity, got.chassis.velocity),
                ("chassis rates", own.chassis.rates, got.chassis.rates),
                ("chassis shock", own.chassis.shock, got.chassis.shock),
                ("chassis twist", own.chassis.twist, got.chassis.twist),
                ("chassis force", own.force, got.force),
                ("chassis torque", own.torque, got.torque),
                ("ball velocity", p.velocity, ball.velocity),
                ("ball rates", p.rates, ball.rates),
                ("ball shock", p.shock, ball.shock),
                ("ball twist", p.twist, ball.twist),
            ] {
                if port.to_bits() != orig.to_bits() {
                    bad.push(format!("{what}: port {:?} original {:?}", port.0, orig.0));
                }
            }
            for (k, (p, o)) in bump.contacts.iter().zip(&list_at(ee, rec)).enumerate() {
                if p.arm.to_bits() != o.arm.to_bits() || p.other_arm.to_bits() != o.other_arm.to_bits() {
                    bad.push(format!("entry {k}'s arms: port {:?} {:?} original {:?} {:?}", p.arm, p.other_arm, o.arm, o.other_arm));
                }
            }
            *seen.borrow_mut() = Some(bad);
        }),
    );
    let dt = ee.f32_at(addr("frame_dt").unwrap());
    ee.call(NODE_UPDATE, &[u64::from(PROPS_NODE)], &[dt], 2_000_000_000);
    let bad = bad.borrow();
    let bad = bad.as_ref().expect("the bike's first-substep resolve ran in the node's tick");
    eprintln!("{bad:#?}");
    assert_eq!(bad.len(), 1, "{bad:#?}");
}
