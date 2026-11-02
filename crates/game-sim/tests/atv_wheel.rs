//! MD37b — the ATV's wheel suspension and tire forces, proved against MX vs. ATV's own code.
//!
//! The image is `tests/atv_quad.rs`'s (MD37a's `atv-r-spawn.ee.bin`): one wheel and the quad's rigid
//! body (with the body's own vtable, so its add-force, add-moment and add-force-at run as they are)
//! are laid out in RAM, a generated state is written into them, SLUS-21104's function runs, the same
//! state is written again with the port's answer over it, and the two regions are compared word for
//! word. The original may write nothing else. What the steps call (the ground query, the contact
//! velocity, the hinge) is hooked: the hooks check the arguments, log the call and serve generated
//! answers, which the port's closures serve too.
//!
//! Without the image every test here skips (and says so).

use game_sim::atv_wheel::{self, Body, More};
use game_sim::wheel::{Call, Wheel};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

const WHEEL: u32 = 0x01c0_8000;
const BODY: u32 = 0x01c0_2000;
/// The world the wheel asks for the ground; never read, only handed on.
const WORLD: u32 = 0x01c0_2400;
const REGIONS: [(&str, u32, u32); 2] = [("wheel", WHEEL, 0x8b0), ("body", BODY, 0x200)];

const BODY_VTABLE: u32 = 0x003c_d200;
const BODY_WORLD_MOMENT: u32 = 0x0049_c1a0;
const BODY_ADD_FORCE_AT: u32 = 0x0049_c0b0;
const SUSPENSION: u32 = 0x004a_0cb0;
const TIRE_FORCES: u32 = 0x004a_16f0;
const GROUND_QUERY: u32 = 0x0048_33c0;
const CONTACT_VELOCITY: u32 = 0x004a_1150;
const HINGE_FORCE: u32 = 0x004a_2aa0;

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2/md37a").join(format!("atv-r-spawn.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!(
            "{} is not there: it is MD37a's (eeMemory.bin of atv-r-spawn.p2s; memory: md37a ATV states live in s2)",
            path.display()
        ));
        return None;
    }
    let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    if ee.u32_at(BODY_VTABLE + 0x88) != BODY_WORLD_MOMENT || ee.u32_at(BODY_VTABLE + 0x84) != BODY_ADD_FORCE_AT {
        panic!("{} is not the MX vs. ATV quad image these addresses are from", path.display());
    }
    Some(ee)
}

/// One generated wheel and body, with the answers the hooked callees give, in call order.
#[derive(Clone, Debug, Default)]
struct State {
    w: Wheel,
    x: More,
    b: Body,
    dt: F,
    /// The ground query's answers: the point (whole quadword) and the unnormalised normal.
    ground: Vec<(Vf, Vf)>,
    /// The ground query hands the point back as it was given, which puts it exactly level with the
    /// probe's end.
    echo: bool,
    /// The contact-velocity step's answers: the contact's and the chassis' velocities.
    velocity: Vec<(Vf, Vf)>,
}

fn put_f(ee: &mut Ee, a: u32, v: F) {
    ee.set_f32(a, v.0);
}
fn put_i(ee: &mut Ee, a: u32, v: i32) {
    ee.set_u32(a, v as u32);
}
fn put_b(ee: &mut Ee, a: u32, v: bool) {
    ee.set_u32(a, u32::from(v));
}
fn put_v(ee: &mut Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_f32(a + 4 * k as u32, v.0[k]);
    }
}
fn get_v(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

fn reset(ee: &mut Ee) {
    for (_, base, len) in REGIONS {
        for o in (0..len).step_by(4) {
            ee.set_u32(base + o, 0);
        }
    }
}

/// The wheel and body at SLUS-21104's offsets (see `game_sim::atv_wheel`'s table).
fn poke(ee: &mut Ee, s: &State) {
    let (w, x, b) = (&s.w, &s.x, &s.b);
    ee.set_u32(WHEEL + 0x1e0, if w.has_ground { WORLD } else { 0 });
    ee.set_u32(WHEEL + 0x1e4, BODY);
    put_b(ee, WHEEL + 0x2c, w.steered);
    put_b(ee, WHEEL + 0x30, w.balancing);
    put_f(ee, WHEEL + 0x3c, w.max_compression);
    put_f(ee, WHEEL + 0x40, w.max_damper_speed);
    put_f(ee, WHEEL + 0x44, w.extension_rate);
    put_f(ee, WHEEL + 0x48, w.spring_rate);
    put_f(ee, WHEEL + 0x4c, w.damping_out);
    put_f(ee, WHEEL + 0x50, w.damping_in);
    put_v(ee, WHEEL + 0x80, w.probe);
    put_v(ee, WHEEL + 0x90, w.mount);
    put_v(ee, WHEEL + 0xc0, x.kept);
    put_i(ee, WHEEL + 0xd0, w.ray_hit);
    put_i(ee, WHEEL + 0xd4, w.swept_contact);
    put_i(ee, WHEEL + 0xd8, w.terrain_wins);
    put_b(ee, WHEEL + 0xdc, w.contact);
    put_i(ee, WHEEL + 0xec, w.ray_steps);
    put_f(ee, WHEEL + 0xf0, w.compression);
    put_f(ee, WHEEL + 0xf4, w.last_compression);
    put_f(ee, WHEEL + 0xf8, w.spring_force);
    put_v(ee, WHEEL + 0x100, w.probe_end);
    ee.set_u32(WHEEL + 0x110, u32::from(x.record) | 0x5a5a_5a00);
    put_v(ee, WHEEL + 0x120, w.contact_point);
    put_v(ee, WHEEL + 0x130, w.contact_normal);
    put_v(ee, WHEEL + 0x140, w.load_point);
    put_v(ee, WHEEL + 0x150, w.mount_world);
    put_v(ee, WHEEL + 0x160, w.probe_end_world);
    put_v(ee, WHEEL + 0x170, w.contact_velocity);
    put_v(ee, WHEEL + 0x180, w.chassis_velocity);
    put_v(ee, WHEEL + 0x200, w.ground_normal);
    put_f(ee, WHEEL + 0x214, w.probe_length);

    ee.set_u32(BODY + 0x110, BODY_VTABLE);
    ee.set_u32(BODY + 0xe8, 0); // no force listeners on the body
    put_v(ee, BODY + 0x70, b.position);
    put_v(ee, BODY + 0xa0, b.force);
    put_v(ee, BODY + 0xd0, b.correction);
    put_v(ee, BODY + 0x190, b.orientation);
    put_v(ee, BODY + 0x1a0, b.cofg);
    put_v(ee, BODY + 0x1b0, b.moment);
}

fn snap(ee: &Ee) -> Vec<u32> {
    REGIONS.iter().flat_map(|&(_, base, len)| (0..len).step_by(4).map(move |o| base + o)).map(|a| ee.u32_at(a)).collect()
}

fn where_is(i: usize) -> String {
    let mut i = i as u32 * 4;
    for (name, _, len) in REGIONS {
        if i < len {
            return format!("{name} + {i:#x}");
        }
        i -= len;
    }
    "past the end".into()
}

fn only_touched_the_regions(ee: &Ee) -> Result<(), String> {
    let frame = (ee.stack - 0x2000, ee.stack + 0x40);
    for (a, b) in ee.written_ranges() {
        let inside = REGIONS.iter().any(|&(_, base, len)| a >= base && b <= base + len) || (a >= frame.0 && b <= frame.1);
        if !inside {
            return Err(format!("the original wrote {a:#010x}..{b:#010x}, which is not the wheel, the body or the stack"));
        }
    }
    Ok(())
}

/// Calls compared by their `Debug` text, which tells `-0.0` from `0.0`.
fn texts(calls: &[Call]) -> Vec<String> {
    calls.iter().map(|c| format!("{c:?}")).collect()
}

type Log = Rc<RefCell<Vec<Call>>>;
type Answers = Rc<RefCell<VecDeque<(Vf, Vf)>>>;

/// The ground query's answer to `query`.
fn answer(s: &State, i: usize, query: Vf) -> (Vf, Vf) {
    let (p, n) = s.ground[i];
    (if s.echo { query } else { p }, n)
}

/// The three callees hooked: each checks what it is handed, logs it and writes the next answer.
fn hook(ee: &mut Ee, log: &Log, ground: &Answers, echo: bool, velocity: &Answers) {
    let (l, g) = (log.clone(), ground.clone());
    ee.hooks.insert(
        GROUND_QUERY,
        Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, WORLD, "the ground query's world");
            assert_eq!((ee.r(7), ee.r(8) as u32), (0, WHEEL + 0x110), "the ground query's last two arguments");
            let (p, n) = (ee.r(5) as u32, ee.r(6) as u32);
            l.borrow_mut().push(Call::GroundQuery(get_v(ee, p)));
            let (point, normal) = g.borrow_mut().pop_front().expect("more ground queries than answers");
            if !echo {
                put_v(ee, p, point);
            }
            put_v(ee, n, normal);
        }),
    );
    let (l, v) = (log.clone(), velocity.clone());
    ee.hooks.insert(
        CONTACT_VELOCITY,
        Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, WHEEL, "the contact velocity's wheel");
            l.borrow_mut().push(Call::ContactVelocity);
            let (c, b) = v.borrow_mut().pop_front().expect("more contact velocities than answers");
            put_v(ee, WHEEL + 0x170, c);
            put_v(ee, WHEEL + 0x180, b);
        }),
    );
    let l = log.clone();
    ee.hooks.insert(
        HINGE_FORCE,
        Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, WHEEL, "the hinge's wheel");
            assert_eq!(ee.f(12).to_bits(), 1.0f32.to_bits(), "the hinge's f12");
            let (f, p) = (get_v(ee, ee.r(5) as u32), get_v(ee, ee.r(6) as u32));
            assert_eq!(ee.r(6) as u32, WHEEL + 0x140, "the hinge's point");
            l.borrow_mut().push(Call::HingeForce(f, p));
        }),
    );
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
    fn one_in(&mut self, n: u32) -> bool {
        self.bits() % n == 0
    }
    fn vec(&mut self, lo: f32, hi: f32) -> Vf {
        Vf::new(self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0)
    }
    /// A direction: mostly up, sometimes anywhere, not quite unit length, `w` near 1.
    fn normal(&mut self) -> Vf {
        let tilt = if self.one_in(4) { 1.0 } else { 0.5 };
        let (x, z) = (self.f(-tilt, tilt).0, self.f(-tilt, tilt).0);
        let y = if self.one_in(6) { self.f(-1.0, 1.0).0 } else { (1.0 - (x * x + z * z).min(0.99)).sqrt() };
        let s = self.f(0.9, 1.1).0;
        Vf::new(x * s, y * s, z * s, self.f(0.99, 1.0).0)
    }
    fn quaternion(&mut self) -> Vf {
        // Mostly near upright, so that the suspension points down.
        let mut v = [self.f(-0.3, 0.3).0, self.f(-1.0, 1.0).0, self.f(-0.3, 0.3).0, self.f(0.5, 1.0).0];
        if self.one_in(5) {
            v = [self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0];
        }
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2] + v[3] * v[3]).sqrt().max(1e-3);
        Vf::new(v[0] / n, v[1] / n, v[2] / n, v[3] / n)
    }

    fn state(&mut self) -> State {
        let mut w = Wheel {
            has_ground: !self.one_in(20),
            steered: self.b(),
            balancing: self.b(),
            max_compression: self.f(0.2, 3.0),
            max_damper_speed: self.f(2.0, 30.0),
            extension_rate: self.f(1.0, 20.0),
            spring_rate: self.f(100.0, 3000.0),
            damping_out: self.f(10.0, 300.0),
            damping_in: self.f(10.0, 300.0),
            // Mostly down the body, as the suspension leaves it.
            probe: if self.one_in(6) {
                self.vec(-2.0, 2.0)
            } else {
                Vf::new(self.f(-0.3, 0.3).0, self.f(-2.5, -0.3).0, self.f(-0.3, 0.3).0, self.f(0.0, 1.0).0)
            },
            mount: Vf::new(self.f(-2.0, 2.0).0, self.f(-1.0, 0.5).0, self.f(-2.5, 2.5).0, self.f(0.0, 1.0).0),
            ray_hit: if self.one_in(3) { 0 } else { 1 },
            swept_contact: if self.b() { 1 } else { 0 },
            terrain_wins: if self.b() { 1 } else { 0 },
            contact: self.b(),
            ray_steps: self.bits() as i32 % 7,
            compression: self.f(-1.0, 1.0),
            last_compression: if self.one_in(4) { F::ZERO } else { self.f(0.0, 1.5) },
            spring_force: self.f(-100.0, 100.0),
            probe_end: self.vec(-5.0, 5.0),
            load_point: self.vec(-5.0, 5.0),
            mount_world: self.vec(-5.0, 5.0),
            probe_end_world: self.vec(-5.0, 5.0),
            contact_velocity: self.vec(-5.0, 5.0),
            chassis_velocity: self.vec(-5.0, 5.0),
            ground_normal: self.normal(),
            probe_length: self.f(0.5, 2.5),
            ..Wheel::default()
        };
        if self.one_in(10) {
            w.max_compression = F::ZERO;
        }
        let b = Body {
            position: self.vec(-50.0, 50.0),
            force: self.vec(-500.0, 500.0),
            moment: self.vec(-500.0, 500.0),
            correction: self.vec(-5.0, 5.0),
            orientation: self.quaternion(),
            cofg: self.vec(-0.5, 0.5),
        };
        // The contact sits around the wheel: somewhere about the body's centre, a few feet down.
        let around = |g: &mut Gen| {
            Vf::new(
                b.position.x() + g.f(-3.0, 3.0).0,
                b.position.y() + g.f(-4.0, 1.0).0,
                b.position.z() + g.f(-3.0, 3.0).0,
                g.f(0.0, 1.0).0,
            )
        };
        w.contact_point = around(self);
        w.contact_normal = self.normal();
        if self.one_in(15) {
            // Next to nothing: the suspension line is as good as parallel to it.
            w.contact_normal = w.contact_normal.mul(Vf::splat(1e-7));
        }
        let point = around(self);
        // The same point every time it asks: only the normals move it on.
        let n = self.normal();
        let ground = (0..5)
            .map(|_| (point, if self.b() { n } else if self.one_in(3) { w.ground_normal } else { self.normal() }))
            .collect();
        // A swept contact a tenth either side of the ray's answer, now and then exactly.
        if self.one_in(3) {
            let d = match self.bits() % 4 {
                0 => 0.1,
                1 => -0.1,
                _ => self.f(-0.3, 0.3).0,
            };
            w.contact_point = Vf::new(w.contact_point.x(), (F(point.y()) + d).0, w.contact_point.z(), w.contact_point.w());
        }
        let velocity = (0..2).map(|_| (self.vec(-20.0, 20.0), self.vec(-20.0, 20.0))).collect();
        let x = More { record: self.bits() as u8, kept: if self.one_in(10) { Vf::ZERO } else { self.vec(-3.0, 3.0) } };
        State { w, x, b, dt: self.f(0.001, 0.04), ground, echo: self.one_in(8), velocity }
    }
}

fn tally(t: &mut BTreeMap<&'static str, u32>, path: &'static str) {
    *t.entry(path).or_insert(0) += 1;
}

fn each_path_seen(tally: &BTreeMap<&'static str, u32>, paths: &[&str], at_least: u32) {
    for want in paths {
        let n = tally.get(want).copied().unwrap_or(0);
        assert!(n >= at_least, "only {n} state(s) went down {want:?}: {tally:?}");
    }
}

/// Run `addr` on the state, then the port, and compare the regions and the calls.
fn compare(ee: &mut Ee, k: usize, s: &State, addr: u32, port: impl Fn(&mut State) -> Vec<Call>) {
    let log: Log = Rc::default();
    let ground: Answers = Rc::new(RefCell::new(s.ground.iter().copied().collect()));
    let velocity: Answers = Rc::new(RefCell::new(s.velocity.iter().copied().collect()));
    hook(ee, &log, &ground, s.echo, &velocity);
    reset(ee);
    poke(ee, s);
    ee.call(addr, &[u64::from(WHEEL)], &[s.dt.0], 50_000);
    only_touched_the_regions(ee).unwrap_or_else(|e| panic!("state {k}: {e}\n{s:#?}"));
    let was = snap(ee);
    let mut t = s.clone();
    let calls = port(&mut t);
    reset(ee);
    poke(ee, &t);
    let now = snap(ee);
    if let Some(i) = was.iter().zip(&now).position(|(a, b)| a != b) {
        panic!(
            "state {k}: {} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\n{s:#?}",
            where_is(i),
            was[i],
            f32::from_bits(was[i]),
            now[i],
            f32::from_bits(now[i]),
        );
    }
    assert_eq!(texts(&log.borrow()), texts(&calls), "state {k}: the calls\n{s:#?}");
}

#[test]
fn the_atv_wheel_suspension_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let mut g = Gen(0x0037_b001);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        let s = g.state();
        let mut d = s.clone();
        let mut asked = 0;
        let calls = atv_wheel::suspension(&mut d.w, &mut d.x, &d.b, &mut |q| {
            asked += 1;
            answer(&s, asked - 1, q)
        });
        tally(&mut t, match asked {
            0 => "no world",
            1 => "one ray",
            5 => "five rays",
            _ => "two to four rays",
        });
        if asked > 0 {
            tally(&mut t, match (d.w.ray_hit, s.w.swept_contact, d.w.swept_contact) {
                (0, 0, _) => "missed",
                (0, _, _) if d.x.record != s.x.record => "missed under a swept contact",
                (0, _, _) if d.w.terrain_wins != s.w.terrain_wins => "swept kept, terrain dropped",
                (0, _, _) => "swept kept",
                (_, 0, _) => "ray hit",
                (_, _, 0) if s.w.terrain_wins == 0 => "terrain won",
                _ => "terrain kept",
            });
            if s.echo {
                tally(&mut t, "level with the probe");
            }
        }
        compare(&mut ee, k, &s, SUSPENSION, |t| {
            let mut i = 0;
            let u = t.clone();
            atv_wheel::suspension(&mut t.w, &mut t.x, &t.b, &mut |q| {
                i += 1;
                answer(&u, i - 1, q)
            })
        });
        assert_eq!(texts(&calls).len(), asked);
    }
    each_path_seen(
        &t,
        &[
            "no world",
            "one ray",
            "two to four rays",
            "five rays",
            "missed",
            "missed under a swept contact",
            "swept kept",
            "swept kept, terrain dropped",
            "ray hit",
            "terrain won",
            "terrain kept",
            "level with the probe",
        ],
        100,
    );
}

#[test]
fn the_atv_wheel_tire_forces_are_the_originals() {
    let Some(mut ee) = image() else { return };
    let mut g = Gen(0x0037_b002);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        let mut s = g.state();
        let port = |t: &mut State| {
            let mut v = t.velocity.clone().into_iter();
            let mut x = t.x;
            let calls = atv_wheel::tire_forces(&mut t.w, &mut t.b, t.dt, &mut |c, w, _| {
                if c == Call::ContactVelocity {
                    (w.contact_velocity, w.chassis_velocity) = v.next().unwrap();
                }
            }, &mut x);
            t.x = x;
            calls
        };
        // The ties, found by a first run of the port: the contact exactly on the probe's end (no
        // travel at all), and the travel exactly at the suspension's limit.
        if g.one_in(8) && (s.w.ray_hit != 0 || s.w.swept_contact != 0) {
            let mut d = s.clone();
            port(&mut d);
            s.w.contact_point = d.w.probe_end_world;
            tally(&mut t, "on the probe's end");
        } else if g.one_in(7) {
            s.w.last_compression = F::ZERO;
            let mut d = s.clone();
            port(&mut d);
            if d.w.contact && d.w.compression < s.w.max_compression {
                s.w.max_compression = d.w.compression;
                tally(&mut t, "at the limit");
            }
        }
        let mut d = s.clone();
        let calls = port(&mut d);
        tally(&mut t, match () {
            _ if s.w.ray_hit == 0 && s.w.swept_contact == 0 => "no contact",
            _ if !d.w.contact => "no load",
            _ => "loaded",
        });
        if (s.w.ray_hit != 0 || s.w.swept_contact != 0) && d.w.load_point == s.w.load_point {
            tally(&mut t, "no load point");
        }
        if d.w.contact {
            let bottomed = calls.iter().filter(|c| **c == Call::ContactVelocity).count() == 2;
            tally(&mut t, match (bottomed, d.b.position != s.b.position) {
                (false, _) => "sprung",
                (true, true) => "bottomed out, shoved",
                (true, false) => "bottomed out, no shove",
            });
            tally(&mut t, if calls.iter().any(|c| matches!(c, Call::HingeForce(..))) { "through the hinge" } else { "into the body" });
            tally(&mut t, if d.w.spring_force == F::ZERO { "no spring force" } else { "spring force" });
        }
        tally(&mut t, if d.w.compression != s.w.last_compression && d.w.compression > d.w.last_compression.min(d.w.compression) {
            "extension limited"
        } else {
            "extension free"
        });
        compare(&mut ee, k, &s, TIRE_FORCES, port);
    }
    each_path_seen(
        &t,
        &[
            "no contact",
            "no load",
            "no load point",
            "on the probe's end",
            "at the limit",
            "loaded",
            "sprung",
            "bottomed out, shoved",
            "bottomed out, no shove",
            "through the hinge",
            "into the body",
            "no spring force",
            "spring force",
        ],
        100,
    );
}
