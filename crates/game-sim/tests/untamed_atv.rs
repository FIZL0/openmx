//! MD81b — MX vs. ATV Untamed's (SLUS-21701) ATV step against MX vs. ATV Unleashed's (SLUS-21104),
//! proved where it differs.
//!
//! The quad's ground step, its air state, its wheels and its engine are SLUS-21104's with the fields
//! moved (research/untamed-machines.md, "SLUS-21701's ATV step against SLUS-21104's"). What the quad
//! does differently in Untamed is in the code it shares with the bike: the reworked preload, backflip
//! and 360 (MD93, MD93a, MD93b), and the vehicle's hazard clock, which Untamed hands a third word,
//! whether any wheel is sunk. With no wheel sunk Untamed passes over the free reset, so a quad off the
//! ground is not free and its clock runs by the wheels' surface time.
//!
//! The oracle is SLUS-21701's clock off a RAM image of a Free Ride in it (`context/untamed/ram/nat_or.bin`,
//! the 32 MB eeMemory of an Untamed save state; the GAME_REL.BIN overlay is in it), run in the
//! interpreter on a vehicle laid out at Untamed's offsets, against
//! [`game_sim::handling::untamed::stuck`] on the same state. Without the image the test skips (and says
//! so).
//!
//! The other change on the quad's path is the scene graph's (the bike's too): Untamed's node world
//! update makes the topmost node's new world frame orthonormal again
//! ([`game_sim::atv_moto::Scene::update_untamed`]). It is proved the same way, on chains of three nodes
//! laid at Untamed's node offsets.

use game_sim::atv_moto::{self, Scene};
use game_sim::handling::tail::{self, NO_STUCK, Out, Vehicle, Vehicles};
use game_sim::handling::untamed;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Where the harness puts the vehicle, its vtable and its run: the top of RAM, under the stack.
const OBJ: u32 = 0x01c0_0000;
const VTABLE: u32 = 0x01c0_2000;
const RUN: u32 = 0x01c0_2800;
/// The address the put-back slot (vt `+0x3a8`) is pointed at, hooked.
const PUT_BACK: u32 = 0x01c0_2900;

/// SLUS-21701's hazard clock (the quad's update calls it straight, SLUS-21104 through vt `+0x45c`), and
/// the words it reads to check the image: its first two instructions.
const CLOCK: u32 = 0x004c_e3a0;
const CLOCK_HEAD: [u32; 2] = [0x27bd_ffe0, 0xffbf_0010];

/// The vehicle's clock words at Untamed's offsets (SLUS-21104's less 0x10): stuck, free, how long; and
/// the run's pointer.
const STUCK: u32 = 0xc00;
const FREE: u32 = 0xc04;
const TIME: u32 = 0xde0;
const RUN_AT: u32 = 0x6a4;

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("untamed/ram").join(format!("nat_or{}", ".bin"));
    if !path.exists() {
        skip(&format!(
            "{} is not there: unzip eeMemory.bin from an MX vs. ATV Untamed (SLUS-21701) Free Ride save state \
             (ps2-decomp-kit-slots/pcsx2/s9, SLUS-21701 (08FE7DFD).05.p2s) to it (s1 has it)",
            path.display()
        ));
        return None;
    }
    let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    if [ee.u32_at(CLOCK), ee.u32_at(CLOCK + 4)] != CLOCK_HEAD {
        panic!("{} is not the MX vs. ATV Untamed image these addresses are from", path.display());
    }
    Some(ee)
}

struct Gen(u64);

#[allow(clippy::float_arithmetic)]
impl Gen {
    fn bits(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
    fn unit(&mut self) -> f32 {
        self.bits() as f32 / 4294967296.0
    }
    fn f(&mut self, lo: f32, hi: f32) -> F {
        F(lo + (hi - lo) * self.unit())
    }
    fn b(&mut self) -> bool {
        self.bits() & 0x10000 != 0
    }
    fn pick(&mut self, from: &[f32]) -> F {
        F(from[self.bits() as usize % from.len()])
    }
}

/// The port's side: the vehicle's clock words, the put-backs counted.
struct Clock {
    v: Vehicle,
    put_backs: u32,
}

impl Vehicles for Clock {
    fn vehicle(&mut self) -> &mut Vehicle {
        &mut self.v
    }
    fn skipped(&mut self) -> bool {
        false
    }
    fn paused(&mut self) -> bool {
        false
    }
    fn rumble_ready(&mut self) -> bool {
        false
    }
    fn word(&mut self, _at: u32) -> u32 {
        0
    }
    fn out(&mut self, c: Out, _dt: F) {
        assert_eq!(c, Out::PutBack, "the hazard clock asks only for the put-back");
        self.put_backs += 1;
    }
}

#[derive(Clone, Copy, Debug)]
struct State {
    on: bool,
    grounded: bool,
    dt: F,
    limit: F,
    free: u32,
    stuck: u32,
    time: F,
    run_flags: u32,
}

fn state(g: &mut Gen) -> State {
    let grounded = g.b();
    State {
        // `on` is the hazard surface and `grounded` together on the quad; the clock is handed it
        // as it comes, so both are drawn apart (the bike's and the car's callers too).
        on: g.b(),
        grounded,
        dt: g.pick(&[1.0 / 60.0, 1.0 / 30.0, 0.0, 0.25]),
        limit: if g.b() { g.pick(&[-1.0, 0.0, -0.0, -0.5, 0.5, 1.0, 3.0]) } else { g.f(-2.0, 4.0) },
        free: g.bits() & 1,
        stuck: g.bits() & 1,
        time: if g.b() { g.f(0.0, 3.5) } else { g.pick(&[0.0, 0.49, 0.98, 2.99]) },
        run_flags: if g.b() { NO_STUCK } else { 0 } | (g.bits() & 0x0f),
    }
}

fn lay(ee: &mut Ee, s: &State) {
    for o in (0..0x1800).step_by(4) {
        ee.set_u32(OBJ + o, 0);
    }
    ee.set_u32(OBJ + 4, VTABLE);
    ee.set_u32(VTABLE + 0x3a8, PUT_BACK);
    ee.set_u32(OBJ + RUN_AT, RUN);
    ee.set_u32(RUN + 8, s.run_flags);
    ee.set_u32(RUN + 12, 0);
    ee.set_u32(OBJ + STUCK, s.stuck);
    ee.set_u32(OBJ + FREE, s.free);
    ee.set_f32(OBJ + TIME, s.time.0);
}

#[test]
fn untamed_hazard_clock_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let put_backs = Rc::new(Cell::new(0u32));
    let count = put_backs.clone();
    ee.hooks.insert(PUT_BACK, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, OBJ, "the put-back is called on the vehicle");
        assert_eq!((ee.r(5), ee.r(6)), (1, 1), "the put-back is asked to place and keep");
        count.set(count.get() + 1);
    }));
    let mut g = Gen(0x4d44_3831_6200);
    let mut tally = BTreeMap::<&str, u32>::new();
    for k in 0..4000 {
        let s = state(&mut g);
        lay(&mut ee, &s);
        put_backs.set(0);
        ee.call(CLOCK, &[u64::from(OBJ), u64::from(s.on), u64::from(s.grounded)], &[s.dt.0, s.limit.0], 10_000);

        let mut c = Clock { v: Vehicle { free: s.free, stuck: s.stuck, stuck_time: s.time, run_flags: s.run_flags, ..Vehicle::default() }, put_backs: 0 };
        untamed::stuck(&mut c, s.on, s.grounded, s.dt, s.limit);
        let want = (ee.u32_at(OBJ + FREE), ee.u32_at(OBJ + STUCK), ee.u32_at(OBJ + TIME), put_backs.get());
        let got = (c.v.free, c.v.stuck, c.v.stuck_time.0.to_bits(), c.put_backs);
        assert_eq!(got, want, "state {k}: {s:?}");

        // SLUS-21104's clock, on the same state: the same whenever a wheel is sunk.
        let mut old = Clock { v: Vehicle { free: s.free, stuck: s.stuck, stuck_time: s.time, run_flags: s.run_flags, ..Vehicle::default() }, put_backs: 0 };
        tail::stuck(&mut old, s.on, s.dt, s.limit);
        let same = (old.v.free, old.v.stuck, old.v.stuck_time.0.to_bits(), old.put_backs) == got;
        if s.grounded {
            assert!(same, "state {k}: a wheel sunk, Untamed's clock is SLUS-21104's: {s:?}");
        }
        let path = match (s.grounded, same, got.3) {
            (true, _, _) => "grounded",
            (false, true, _) => "not grounded, as SLUS-21104",
            (false, false, 0) => "not grounded, kept stuck",
            (false, false, _) => "not grounded, put back",
        };
        *tally.entry(path).or_default() += 1;
    }
    eprintln!("MD81b hazard clock: {tally:?}");
    for want in ["grounded", "not grounded, as SLUS-21104", "not grounded, kept stuck", "not grounded, put back"] {
        assert!(tally.get(want).copied().unwrap_or(0) >= 50, "too few states went {want:?}: {tally:?}");
    }
}

// --- the node world update ------------------------------------------------------------------------

/// SLUS-21701's node world update and the orthonormaliser it calls (its first instructions checked).
const NODE_UPDATE: u32 = 0x0018_f720;
const NODE_UPDATE_HEAD: u32 = 0x27bd_fed0;
const ORTHONORMAL: u32 = 0x0013_1aa0;
const ORTHONORMAL_HEAD: u32 = 0x27bd_ffd0;
/// A node's words at Untamed's offsets (SLUS-21104's plus 0x30): its frame, its world frame, its
/// parent and its flag byte.
const LOCAL: u32 = 0x60;
const WORLD: u32 = 0xa0;
const PARENT: u32 = 0x120;
const FLAGS: u32 = 0x130;
/// Where the three nodes go (each 0x200 long), and the orthonormaliser's output.
const NODE: u32 = 0x01c0_4000;
const OUT: u32 = 0x01c0_4800;

fn put_v(ee: &mut Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_f32(a + 4 * k as u32, v.0[k]);
    }
}
fn get_v(ee: &Ee, a: u32) -> [u32; 4] {
    std::array::from_fn(|k| ee.u32_at(a + 4 * k as u32))
}
fn bits(m: &[Vf; 4]) -> [[u32; 4]; 4] {
    m.map(|v| v.0.map(f32::to_bits))
}

/// A frame: a turn (a quaternion's, as the scene places nodes) with its rows sometimes nudged off
/// orthonormal, and a position; now and then one of all zeros.
fn frame(g: &mut Gen) -> [Vf; 4] {
    if g.bits() % 40 == 0 {
        return [Vf::ZERO; 4];
    }
    let q = Vf::new(g.f(-1.0, 1.0).0, g.f(-1.0, 1.0).0, g.f(-1.0, 1.0).0, g.f(-1.0, 1.0).0);
    let pos = Vf::new(g.f(-900.0, 900.0).0, g.f(-50.0, 200.0).0, g.f(-900.0, 900.0).0, g.f(-1.0, 1.0).0);
    let mut m = atv_moto::quat_frame(q, pos);
    if g.b() {
        for r in &mut m[..3] {
            for k in 0..3 {
                r.0[k] = ps2_float::vu::add(r.0[k], g.f(-0.01, 0.01).0);
            }
        }
    }
    m
}

#[test]
fn untamed_orthonormaliser_is_the_originals() {
    let Some(mut ee) = image() else { return };
    assert_eq!(ee.u32_at(ORTHONORMAL), ORTHONORMAL_HEAD, "not the Untamed image's orthonormaliser");
    let mut g = Gen(0x4d44_3831_6201);
    for k in 0..3000 {
        let m = frame(&mut g);
        for (r, v) in m.iter().enumerate() {
            put_v(&mut ee, NODE + 0x10 * r as u32, *v);
        }
        // a0 out, a1 the position, a2 the third row, a3 the second; t0 and t1 1, as the update asks.
        let ints = [OUT, NODE + 0x30, NODE + 0x20, NODE + 0x10, 1, 1].map(u64::from);
        ee.call(ORTHONORMAL, &ints, &[], 10_000);
        let want: [[u32; 4]; 4] = std::array::from_fn(|r| get_v(&ee, OUT + 0x10 * r as u32));
        assert_eq!(bits(&atv_moto::untamed_orthonormal(&m)), want, "frame {k}: {m:?}");
    }
}

#[test]
fn untamed_node_world_update_is_the_originals() {
    let Some(mut ee) = image() else { return };
    assert_eq!(ee.u32_at(NODE_UPDATE), NODE_UPDATE_HEAD, "not the Untamed image's node update");
    let mut g = Gen(0x4d44_3831_6202);
    let mut tally = BTreeMap::<&str, u32>::new();
    for k in 0..2000 {
        // Three nodes, 0 the root, 1 under it, 2 under 1; each current or not, at random.
        let mut scene = Scene::default();
        for n in 0..3 {
            let at = NODE + 0x200 * n as u32;
            for o in (0..0x200).step_by(4) {
                ee.set_u32(at + o, 0);
            }
            let node = &mut scene.nodes[n];
            node.local = frame(&mut g);
            node.world = frame(&mut g);
            node.parent = n.checked_sub(1);
            node.flags = (g.bits() & 0x81) as u8;
            for r in 0..4 {
                put_v(&mut ee, at + LOCAL + 0x10 * r as u32, node.local[r]);
                put_v(&mut ee, at + WORLD + 0x10 * r as u32, node.world[r]);
            }
            ee.set_u32(at + PARENT, node.parent.map_or(0, |p| NODE + 0x200 * p as u32));
            ee.set_u32(at + FLAGS, u32::from(node.flags));
        }
        let asked = (g.bits() % 3) as usize;
        let before = scene;
        ee.call(NODE_UPDATE, &[u64::from(NODE + 0x200 * asked as u32)], &[], 100_000);
        scene.update_untamed(asked);
        for n in 0..3 {
            let at = NODE + 0x200 * n as u32;
            let want: [[u32; 4]; 4] = std::array::from_fn(|r| get_v(&ee, at + WORLD + 0x10 * r as u32));
            assert_eq!(bits(&scene.nodes[n].world), want, "case {k}, node {n} of node {asked} updated: {before:?}");
            assert_eq!(u32::from(scene.nodes[n].flags), ee.u32_at(at + FLAGS) & 0xff, "case {k}, node {n}'s flags");
        }
        // How the update went: nothing (current), the root alone, or from a node with a parent (made
        // orthonormal), and whether SLUS-21104's update would have come out the same.
        let mut old = before;
        old.update(asked);
        let path = match (before.nodes[asked].flags & 1, old == scene) {
            (1, _) => "current",
            (_, true) => "as SLUS-21104",
            (_, false) => "made orthonormal",
        };
        *tally.entry(path).or_default() += 1;
    }
    eprintln!("MD81b node update: {tally:?}");
    for want in ["current", "as SLUS-21104", "made orthonormal"] {
        assert!(tally.get(want).copied().unwrap_or(0) >= 100, "too few cases went {want:?}: {tally:?}");
    }
}
