//! MD65c2 — the quad's net interpolation (vt `+0x3ec`), [`quad::net_interpolate`] against the
//! original's.
//!
//! The original runs as it is, the biplane's interpolation it calls last (atv_biplane_net_interpolate)
//! and the slerp (atv_quat_slerp, on the image's arc cosine table) unhooked; only the put-back a far snapshot
//! asks for (vt `+0x3a8`) is hooked, logged as [`Call::NetPutBack`]. Every word the two can write is
//! read through the quad's own pointers (the snapshot's nine at `*(+0xbcc)`, the eight lags at
//! `*(+0xd14)`, `*(+0xd18)`, `+0x1790`, `+0x178c`, and `+0xd40`, `+0x32c`) and compared by bits, and
//! the original may write nothing else but its stack. The generated test lays its own pointers; the
//! spawned quad's are the image's, its lags and the world's area too.

use super::*;
use game_sim::quad::{Anim, Lag, NetSmooth, Remote, Snapshot};

const NET_INTERPOLATE: u32 = 0x004e_7bc0;
const BIPLANE_INTERPOLATE: u32 = 0x004a_87c0;
/// The quad's put-back, which vt `+0x3a8` is.
const PUT_BACK_SLOT: u32 = 0x3a8;

/// The generated cases' stand-ins: the snapshot's pointer table and its nine places, the two lag
/// tables and the eight lags, the two snapshots, and the world (`+0x3b0`) whose `+0x3b4` holds the area.
const LAYOUT: u32 = 0x01d8_0000;
const SNAP_TABLE: u32 = LAYOUT;
const SNAP_PLACES: u32 = LAYOUT + 0x100;
const POS_LAGS: u32 = LAYOUT + 0x200;
const VEL_LAGS: u32 = LAYOUT + 0x210;
const LAGS: u32 = LAYOUT + 0x300;
const OLDER: u32 = LAYOUT + 0x400;
const NEWER: u32 = LAYOUT + 0x480;
const WORLD_OBJ: u32 = LAYOUT + 0x500;
const AREA_OBJ: u32 = LAYOUT + 0x600;

/// Where everything the interpolation reads and writes is, off the quad's pointers.
#[derive(Clone, Copy, Debug)]
struct Places {
    snap: [u32; 9],
    /// Position x, y, z; velocity x, y, z; crossup; fore and aft.
    lags: [u32; 8],
    area: u32,
}

impl Places {
    fn of(ee: &Ee, quad: u32) -> Places {
        let table = ee.u32_at(quad + 0xbcc);
        let pos = ee.u32_at(quad + 0xd14);
        let vel = ee.u32_at(quad + 0xd18);
        let mut lags = [0; 8];
        for k in 0..3 {
            lags[k] = ee.u32_at(pos + 4 * k as u32);
            lags[3 + k] = ee.u32_at(vel + 4 * k as u32);
        }
        lags[6] = ee.u32_at(quad + 0x1790);
        lags[7] = ee.u32_at(quad + 0x178c);
        Places {
            snap: std::array::from_fn(|k| ee.u32_at(table + 4 * k as u32)),
            lags,
            area: ee.u32_at(ee.u32_at(quad + 0x3b0) + 0x3b4) + 0x550,
        }
    }
}

fn v_at(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

fn read_snapshot(ee: &Ee, a: u32) -> Snapshot {
    Snapshot {
        time: F(ee.f32_at(a)),
        position: v_at(ee, a + 0x10),
        velocity: v_at(ee, a + 0x20),
        orientation: v_at(ee, a + 0x30),
        crossup_blend: F(ee.f32_at(a + 0x40)),
        fore_aft_blend: F(ee.f32_at(a + 0x44)),
        wreck: ee.u32_at(a + 0x48),
        stunt: ee.u32_at(a + 0x4c) as i32,
        word: ee.u32_at(a + 0x50),
    }
}

fn put_snapshot(ee: &mut Ee, a: u32, s: &Snapshot) {
    put_f(ee, a, s.time);
    put_v(ee, a + 0x10, s.position);
    put_v(ee, a + 0x20, s.velocity);
    put_v(ee, a + 0x30, s.orientation);
    put_f(ee, a + 0x40, s.crossup_blend);
    put_f(ee, a + 0x44, s.fore_aft_blend);
    ee.set_u32(a + 0x48, s.wreck);
    ee.set_u32(a + 0x4c, s.stunt as u32);
    ee.set_u32(a + 0x50, s.word);
}

fn read_lag(ee: &Ee, a: u32) -> Lag {
    Lag { value: F(ee.f32_at(a)), time: F(ee.f32_at(a + 4)), fraction: F(ee.f32_at(a + 8)) }
}

/// What the port is handed, read off the quad.
#[derive(Clone, Debug)]
struct In {
    older: Snapshot,
    newer: Snapshot,
    area: [F; 4],
    s: NetSmooth,
    r: Remote,
    a: Anim,
    q: Quad,
}

fn read_in(ee: &Ee, quad: u32, p: &Places, older: u32, newer: u32) -> In {
    let lag = |k: usize| read_lag(ee, p.lags[k]);
    let s = NetSmooth {
        position: [lag(0), lag(1), lag(2)],
        velocity: [lag(3), lag(4), lag(5)],
        crossup: lag(6),
        fore_aft: lag(7),
        put_back_at: v_at(ee, quad + 0xd40),
        velocity_change: v_at(ee, p.snap[3]),
        wreck: ee.u32_at(p.snap[6]),
        word: ee.u32_at(p.snap[8]),
    };
    let r = Remote { position: v_at(ee, p.snap[0]), orientation: v_at(ee, p.snap[1]), velocity: v_at(ee, p.snap[2]), ..Remote::default() };
    let a = Anim {
        crossup_blend: F(ee.f32_at(p.snap[4])),
        fore_aft_blend: F(ee.f32_at(p.snap[5])),
        next_stunt: ee.u32_at(p.snap[7]) as i32,
        ..Anim::default()
    };
    let q = Quad { speed_len: F(ee.f32_at(quad + 0x32c)), ..Quad::default() };
    In {
        older: read_snapshot(ee, older),
        newer: read_snapshot(ee, newer),
        area: [0, 4, 8, 12].map(|o| F(ee.f32_at(p.area + o))),
        s,
        r,
        a,
        q,
    }
}

/// Every word either may write, as `(what, address, the port's word)`.
fn port_words(quad: u32, p: &Places, d: &In) -> Vec<(String, u32, u32)> {
    let mut w = Vec::new();
    let mut v = |what: &str, a: u32, x: Vf| {
        for (k, b) in x.to_bits().into_iter().enumerate() {
            w.push((format!("{what}[{k}]"), a + 4 * k as u32, b));
        }
    };
    v("snapshot position", p.snap[0], d.r.position);
    v("snapshot orientation", p.snap[1], d.r.orientation);
    v("snapshot velocity", p.snap[2], d.r.velocity);
    v("snapshot velocity change", p.snap[3], d.s.velocity_change);
    v("put back at", quad + 0xd40, d.s.put_back_at);
    let lags = [d.s.position[0], d.s.position[1], d.s.position[2], d.s.velocity[0], d.s.velocity[1], d.s.velocity[2], d.s.crossup, d.s.fore_aft];
    for (k, l) in lags.iter().enumerate() {
        let a = p.lags[k];
        w.push((format!("lag {k} value"), a, l.value.0.to_bits()));
        w.push((format!("lag {k} time"), a + 4, l.time.0.to_bits()));
        w.push((format!("lag {k} fraction"), a + 8, l.fraction.0.to_bits()));
    }
    w.push(("crossup blend".into(), p.snap[4], d.a.crossup_blend.0.to_bits()));
    w.push(("fore-aft blend".into(), p.snap[5], d.a.fore_aft_blend.0.to_bits()));
    w.push(("wreck".into(), p.snap[6], d.s.wreck));
    w.push(("stunt".into(), p.snap[7], d.a.next_stunt as u32));
    w.push(("word".into(), p.snap[8], d.s.word));
    w.push(("speed".into(), quad + 0x32c, d.q.speed_len.0.to_bits()));
    w
}

fn hook_put_back(ee: &mut Ee, quad: &Rc<Cell<u32>>) -> Log<Call> {
    let log: Log<Call> = Rc::default();
    let at = ee.u32_at(QUAD_VTABLE + PUT_BACK_SLOT);
    assert_eq!(at, PUT_BACK, "vt +0x3a8 is the quad's put-back");
    let (l, q) = (log.clone(), quad.clone());
    ee.hooks.insert(at, Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5), ee.r(6)), (q.get(), 0, 0), "the put-back's arguments");
        l.borrow_mut().push(Call::NetPutBack);
    }));
    log
}

/// Runs both on the quad at `quad` with the snapshots at `older` and `newer` and compares every word.
fn compare(ee: &mut Ee, log: &Log<Call>, table: &AcosTable, quad: u32, older: u32, newer: u32, t: F, dt: F, label: &str) -> In {
    let p = Places::of(ee, quad);
    let start = read_in(ee, quad, &p, older, newer);
    log.borrow_mut().clear();
    ee.call(NET_INTERPOLATE, &[u64::from(quad), u64::from(older), u64::from(newer)], &[dt.0, t.0], 50_000);
    let mut d = start.clone();
    let mut calls = Vec::new();
    let mut body = Body::default();
    quad::net_interpolate(&d.older, &d.newer, t, dt, d.area, &mut d.s, &mut d.r, &mut d.a, &mut d.q, &mut body, table, &mut calls);
    let words = port_words(quad, &p, &d);
    let frame = (ee.stack - 0x2000, ee.stack + 0x40);
    for (lo, hi) in ee.written_ranges() {
        for x in lo..hi {
            let ours = words.iter().any(|&(_, a, _)| (a..a + 4).contains(&x));
            assert!(ours || (frame.0..frame.1).contains(&x), "{label}: the original wrote {x:#010x} (in {lo:#010x}..{hi:#010x})\n{start:#?}");
        }
    }
    for (what, a, want) in &words {
        let got = ee.u32_at(*a);
        assert_eq!(
            got,
            *want,
            "{label}: {what} is {got:#010x} ({}) in the original and {want:#010x} ({}) in the port (t {t:?}, dt {dt:?})\n{start:#?}",
            f32::from_bits(got),
            f32::from_bits(*want)
        );
    }
    assert_eq!(*log.borrow(), calls, "{label}: the calls\n{start:#?}");
    start
}

/// The paths a case takes, read off what it starts from (plain floats: only for the tally).
#[allow(clippy::float_arithmetic)]
fn paths(i: &In, t: F) -> Vec<&'static str> {
    let mut p = vec![if t.0 <= 0.0 { "t at or before the older" } else if t.0 < 1.0 { "t between" } else { "t at or past the newer" }];
    let lerp = |a: f32, b: f32| a + t.0 * (b - a);
    let (o, n) = (i.older.position, i.newer.position);
    let mut at = [lerp(o.x(), n.x()), lerp(o.y(), n.y()), lerp(o.z(), n.z())];
    for (k, lo, hi) in [(0, i.area[0].0 + 25.0, i.area[1].0 - 25.0), (2, i.area[2].0 + 25.0, i.area[3].0 - 25.0)] {
        if at[k] < lo || at[k] > hi {
            p.push(if k == 0 { "x held inside" } else { "z held inside" });
            at[k] = at[k].clamp(lo, hi);
        }
    }
    let r = i.r.position;
    let far = ((r.x() - at[0]).powi(2) + (r.y() - at[1]).powi(2) + (r.z() - at[2]).powi(2)).sqrt();
    p.push(if far <= 8.0 { "near" } else if far <= 16.0 { "lags set" } else { "put back" });
    if far > 8.0 && (at[1] == f32::MAX || [0, 1, 2].iter().any(|&k| i.older.velocity.0[k] == f32::MAX && i.newer.velocity.0[k] == f32::MAX)) {
        p.push("FLT_MAX lane not set");
    }
    let (a, b) = (i.older.orientation, i.newer.orientation);
    let dot = a.x() * b.x() + a.y() * b.y() + a.z() * b.z() + a.w() * b.w();
    p.push(if dot.abs() >= 1.0 { "orientations parallel" } else if dot < 0.0 { "orientation the short way" } else { "orientation slerped" });
    p
}

impl Gen {
    fn lag(&mut self, around: f32) -> Lag {
        let time = if self.one_in(30) { F::ZERO } else { self.near(0.05, 0.005, 0.5) };
        Lag { value: self.f(-around, around), time, fraction: self.f(0.0, 1.0) }
    }

    fn net_t(&mut self) -> F {
        match self.bits() % 9 {
            0 => F::ZERO,
            1 => F::ONE,
            2 => F(-0.9),
            3 => F(1.9),
            4 => self.f(-0.9, 0.0),
            5 => self.f(1.0, 1.9),
            _ => self.f(0.0, 1.0),
        }
    }

    fn near_quaternion(&mut self, q: Vf) -> Vf {
        let v: [f32; 4] = std::array::from_fn(|k| q.0[k] + self.f(-0.05, 0.05).0);
        #[allow(clippy::float_arithmetic)]
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2] + v[3] * v[3]).sqrt();
        #[allow(clippy::float_arithmetic)]
        Vf::new(v[0] / n, v[1] / n, v[2] / n, v[3] / n)
    }

    /// Two snapshots, newest second: the newer a step from the older, now and then a long way off.
    #[allow(clippy::float_arithmetic)]
    fn snapshots(&mut self, area: [F; 4]) -> (Snapshot, Snapshot) {
        let pos = Vf::new(
            self.f(area[0].0 - 100.0, area[1].0 + 100.0).0,
            if self.one_in(40) { f32::MAX } else { self.f(-200.0, 500.0).0 },
            self.f(area[2].0 - 100.0, area[3].0 + 100.0).0,
            self.f(-1.0, 1.0).0,
        );
        let step = if self.one_in(5) { self.vec(-200.0, 200.0) } else { self.vec(-10.0, 10.0) };
        let next = Vf::new(pos.x() + step.x(), if pos.y() == f32::MAX { f32::MAX } else { pos.y() + step.y() }, pos.z() + step.z(), step.w());
        let mut vel = || {
            let mut v = self.vec(-60.0, 60.0);
            if self.one_in(30) {
                v.0[(self.bits() % 3) as usize] = f32::MAX;
            }
            v
        };
        let (v0, mut v1) = (vel(), vel());
        for k in 0..3 {
            if v0.0[k] == f32::MAX {
                v1.0[k] = f32::MAX;
            }
        }
        let o0 = self.quaternion();
        let o1 = match self.bits() % 6 {
            0 => o0,
            1 => Vf::new(-o0.x(), -o0.y(), -o0.z(), -o0.w()),
            2 => {
                let n = self.near_quaternion(o0);
                Vf::new(-n.x(), -n.y(), -n.z(), -n.w())
            }
            3 => self.quaternion(),
            _ => self.near_quaternion(o0),
        };
        let mut snap = |position: Vf, velocity: Vf, orientation: Vf| Snapshot {
            time: self.f(0.0, 500.0),
            position,
            velocity,
            orientation,
            crossup_blend: self.f(-0.5, 1.5),
            fore_aft_blend: self.near(0.5, -0.5, 1.5),
            wreck: if self.b() { 0 } else { self.bits() % 16 },
            stunt: if self.b() { 46 } else { self.bits() as i32 },
            word: self.bits(),
        };
        let older = snap(pos, v0, o0);
        let newer = snap(next, v1, o1);
        (older, newer)
    }
}

/// The generated layout: the quad's pointers, its lags, the world's area, the snapshots, and where
/// the snapshot last put the quad (near, a little way off or far from the new place).
#[allow(clippy::float_arithmetic)]
fn lay(ee: &mut Ee, g: &mut Gen) {
    for o in (0..0x1800).step_by(4) {
        ee.set_u32(OBJ + o, 0);
    }
    for o in (0..0x1000).step_by(4) {
        ee.set_u32(LAYOUT + o, 0);
    }
    ee.set_u32(OBJ + 0x4, QUAD_VTABLE);
    ee.set_u32(OBJ + 0xbcc, SNAP_TABLE);
    for k in 0..9 {
        ee.set_u32(SNAP_TABLE + 4 * k, SNAP_PLACES + 0x10 * k);
    }
    ee.set_u32(OBJ + 0xd14, POS_LAGS);
    ee.set_u32(OBJ + 0xd18, VEL_LAGS);
    for k in 0..3 {
        ee.set_u32(POS_LAGS + 4 * k, LAGS + 0x10 * k);
        ee.set_u32(VEL_LAGS + 4 * k, LAGS + 0x10 * (3 + k));
    }
    ee.set_u32(OBJ + 0x1790, LAGS + 0x60);
    ee.set_u32(OBJ + 0x178c, LAGS + 0x70);
    ee.set_u32(OBJ + 0x3b0, WORLD_OBJ);
    ee.set_u32(WORLD_OBJ + 0x3b4, AREA_OBJ);
    let area = [g.f(-3000.0, -400.0), g.f(400.0, 3000.0), g.f(-3000.0, -400.0), g.f(400.0, 3000.0)];
    for (k, &a) in area.iter().enumerate() {
        put_f(ee, AREA_OBJ + 0x550 + 4 * k as u32, a);
    }
    let (older, newer) = g.snapshots(area);
    put_snapshot(ee, OLDER, &older);
    put_snapshot(ee, NEWER, &newer);
    let p = Places::of(ee, OBJ);
    for (k, &a) in p.lags.iter().enumerate() {
        let l = g.lag(if k < 3 { 3000.0 } else if k < 6 { 60.0 } else { 1.5 });
        put_f(ee, a, l.value);
        put_f(ee, a + 4, l.time);
        put_f(ee, a + 8, l.fraction);
    }
    // Where the snapshot last put the quad: about where this one will.
    let o = older.position;
    let off = match g.bits() % 3 {
        0 => g.vec(-3.0, 3.0),
        1 => g.vec(-12.0, 12.0),
        _ => g.vec(-100.0, 100.0),
    };
    let y = if o.y() == f32::MAX { g.f(-200.0, 500.0).0 } else { o.y() + off.y() };
    put_v(ee, p.snap[0], Vf::new(o.x() + off.x(), y, o.z() + off.z(), off.w()));
    put_v(ee, p.snap[1], g.quaternion());
    put_v(ee, p.snap[2], g.vec(-60.0, 60.0));
    put_v(ee, p.snap[3], g.vec(-60.0, 60.0));
    put_f(ee, p.snap[4], g.f(-1.0, 1.0));
    put_f(ee, p.snap[5], g.f(-1.0, 1.0));
    for k in 6..9 {
        ee.set_u32(p.snap[k], g.bits());
    }
    put_v(ee, OBJ + 0xd40, g.vec(-3000.0, 3000.0));
    put_f(ee, OBJ + 0x32c, g.f(0.0, 80.0));
}

fn dt_of(g: &mut Gen) -> F {
    if g.one_in(4) { F(1.0 / 60.0) } else { g.f(0.0, 0.2) }
}

const PATHS: [&str; 13] = [
    "t at or before the older",
    "t between",
    "t at or past the newer",
    "x held inside",
    "z held inside",
    "near",
    "lags set",
    "put back",
    "FLT_MAX lane not set",
    "orientations parallel",
    "orientation the short way",
    "orientation slerped",
    "one of each",
];

#[test]
fn the_quads_net_interpolation_is_the_originals() {
    let Some(mut ee) = image() else { return };
    assert_eq!(ee.u32_at(QUAD_VTABLE + 0x3ec), NET_INTERPOLATE, "the quad's net interpolation slot");
    assert_eq!(ee.u32_at(NET_INTERPOLATE + 0x108) >> 26, 3, "the quad's interpolation ends in a jal");
    assert_eq!(ee.u32_at(NET_INTERPOLATE + 0x108) & 0x03ff_ffff, BIPLANE_INTERPOLATE >> 2, "to the biplane's");
    let table = AcosTable::build();
    let at = ee.u32_at(ACOS_PTR);
    for (i, &e) in table.entries().iter().enumerate() {
        assert_eq!(ee.f32_at(at + 4 * i as u32).to_bits(), e.to_bits(), "arc cosine entry {i}");
    }
    let quad = Rc::new(Cell::new(OBJ));
    let log = hook_put_back(&mut ee, &quad);
    let mut g = Gen(0x0065_c002);
    let mut t = BTreeMap::new();
    for k in 0..20_000 {
        lay(&mut ee, &mut g);
        let (tt, dt) = (g.net_t(), dt_of(&mut g));
        let start = compare(&mut ee, &log, &table, OBJ, OLDER, NEWER, tt, dt, &format!("case {k}"));
        let p = paths(&start, tt);
        for &x in &p {
            tally(&mut t, x);
        }
        if p.len() >= 4 {
            tally(&mut t, "one of each");
        }
    }
    each_path_seen(&t, &PATHS, 200);
}

/// The spawned quad's own pointers, lags and world area, handed generated snapshot pairs; each case
/// starts from the image's words again.
#[test]
fn the_spawned_quads_net_interpolation_is_the_ports() {
    let Some(mut ee) = image() else { return };
    let table = AcosTable::build();
    let q = image::IMAGE_QUAD;
    let quad = Rc::new(Cell::new(q));
    let log = hook_put_back(&mut ee, &quad);
    let p = Places::of(&ee, q);
    let mut keep: Vec<(u32, u32)> = (0..0x1800).step_by(4).map(|o| (q + o, ee.u32_at(q + o))).collect();
    for &a in p.lags.iter() {
        keep.extend((0..3).map(|k| (a + 4 * k, ee.u32_at(a + 4 * k))));
    }
    for (k, &a) in p.snap.iter().enumerate() {
        let n = if k < 4 { 4 } else { 1 };
        keep.extend((0..n).map(|j| (a + 4 * j, ee.u32_at(a + 4 * j))));
    }
    let area: [F; 4] = [0, 4, 8, 12].map(|o| F(ee.f32_at(p.area + o)));
    let lags = read_lag(&ee, p.lags[0]);
    eprintln!("the spawned quad: snapshot places {:x?}, lags {:x?} (position x {lags:?}), area {area:?}", p.snap, p.lags);
    let mut g = Gen(0x0065_c003);
    let mut t = BTreeMap::new();
    for k in 0..2_000 {
        for &(a, w) in &keep {
            ee.set_u32(a, w);
        }
        let (mut older, mut newer) = g.snapshots(area);
        // About where the image's quad is, now and then far off.
        if !g.one_in(4) {
            let at = v_at(&ee, p.snap[0]);
            #[allow(clippy::float_arithmetic)]
            let o = Vf::new(at.x() + g.f(-10.0, 10.0).0, at.y() + g.f(-10.0, 10.0).0, at.z() + g.f(-10.0, 10.0).0, 1.0);
            #[allow(clippy::float_arithmetic)]
            let n = Vf::new(o.x() + g.f(-5.0, 5.0).0, o.y() + g.f(-5.0, 5.0).0, o.z() + g.f(-5.0, 5.0).0, 1.0);
            older.position = o;
            newer.position = n;
        }
        if g.one_in(3) {
            older.orientation = v_at(&ee, p.snap[1]);
        }
        put_snapshot(&mut ee, OLDER, &older);
        put_snapshot(&mut ee, NEWER, &newer);
        let (tt, dt) = (g.net_t(), dt_of(&mut g));
        let start = compare(&mut ee, &log, &table, q, OLDER, NEWER, tt, dt, &format!("spawned case {k}"));
        for x in paths(&start, tt) {
            tally(&mut t, x);
        }
    }
    for &(a, w) in &keep {
        ee.set_u32(a, w);
    }
    eprintln!("the spawned quad's paths: {t:?}");
    each_path_seen(&t, &["t between", "near", "lags set", "put back", "orientation slerped"], 10);
}
