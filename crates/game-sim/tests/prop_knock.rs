//! S195 — free09's beach balls knocked by the bike in PCSX2, and the port's props tick replayed
//! against the capture.
//!
//! research/s195/aim.py puts the human's bike (first_recording's state, free09) in line with a beach
//! ball and sets it rolling at it with no pad; research/s195/knock.py records watch/s195-knock.toml (the
//! five balls' bodies, node origins, record flags and countdowns, a row a tick) and saves states around
//! the strike. The strike itself goes through the original's struck listener with the contact's push,
//! which the port does not have yet (S197): the replay starts from a state saved after it, the woken
//! balls' collections read out of the state's RAM into the port's [`PropSet`], and from there every
//! tick the port's [`PropSet::frame_among`] over the disc's terrain and scene is compared bit for bit
//! with the original's rows: the flight, the landing, the bounces and the roll, asleep, the countdown,
//! and the put-back.
//!
//! Skipped with a line where the disc, the kit, the state or the capture is not there (they are in s3's
//! context only).

use std::collections::HashMap;
use std::path::PathBuf;

use game_render::disc::{Disc, Res};
use game_sim::biplane::{Area, Body};
use game_sim::contact::Node;
use game_sim::phy::Ini;
use game_sim::prop_set::PropSet;
use game_sim::prop_world::Listener;
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Ground, Props};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};

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
