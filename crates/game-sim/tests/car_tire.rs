//! The car's tire model (S149b) against the original's, tick by tick on S149's captured drives of the
//! baja, the buggy and the monster truck (`context/capture/s149b/<car>.csv`, S149b's capture.sh: every
//! row carries each tire object's first 0x2c0 bytes and its 0x9b0..0x9d0 tail as hex, with the car's
//! and the engine's raw blocks).
//!
//! The car steps its tires once every physics substep, four to a 30 Hz tick, so a row holds the fourth
//! substep's tire and nothing of the three before it. Two proofs come out of that:
//!
//! - **What a row fixes on its own** — the surface forces from the row's own slip, load and camber;
//!   a fast tire's slip from its contact velocity and spin; the camber from the ground normal; the
//!   suspension's contact point and load from the row's axle and ground points; the load the car hands
//!   the rear tires — is compared with the row to the bit.
//! - **The whole step**, which reads the substep before: the original's own tire step runs under the
//!   interpreter on every row, from the previous row's tire with this row's inputs laid in (what the
//!   parts of the car step this task does not port leave in the tire before the model reads it), and
//!   the port must write every word it writes. Its frame and contact-velocity helpers and the body's
//!   force calls are stood in for: they hand back the row's own words, and apply nothing.

use game_sim::air::AcosTable;
use game_sim::archive::{Psr, Res};
use game_sim::camera::CurveTable;
use game_sim::car::{self, Car, Ground, Kind, TireState};
use ps2_float::{F, Vf, fpu};
use ps2_oracle::{Ee, Kit, skip};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// Where S149a's switch parks the car, in every drive.
const CAR: u32 = 0x00605270;
/// The car's raw block starts at car +0x200.
const CAR_RAW: u32 = 0x200;
const TIRES: [u32; 4] = [0x548, 0x54c, 0x550, 0x554];
const NAMES: [&str; 4] = ["fl", "fr", "rl", "rr"];
/// The engine's drive slots (tire pointers), and the torque it hands each slot this tick.
const ENGINE_SLOTS: u32 = 0x30;
const ENGINE_TORQUE: u32 = 0x118;
/// The tail block starts at tire +0x9b0.
const TAIL: usize = 0x9b0;

/// The tire object's words the model reads and writes.
mod at {
    pub const BRAKE: usize = 0x30;
    pub const STEER: usize = 0x34;
    pub const TOP_WHEEL_SPEED: usize = 0xa0;
    pub const ABS: usize = 0xa8;
    pub const SOLID: usize = 0xb4;
    pub const TOUCHING: usize = 0xb8;
    pub const GROUNDED: usize = 0xc0;
    pub const COMPRESSION: usize = 0xdc;
    pub const LAST_COMPRESSION: usize = 0xe0;
    pub const LOAD: usize = 0xe4;
    pub const HANDED: usize = 0xc8;
    pub const HANDED_LOAD: usize = 0xcc;
    pub const HANDED_CONTACTS: usize = 0xd0;
    pub const CONTACTS: usize = 0x1ac;
    pub const BODY: usize = 0x1bc;
    pub const FRAME: usize = 0x270;
    pub const SURFACE: usize = 0x100;
    pub const GROUND_POINT: usize = 0x110;
    pub const NORMAL: usize = 0x120;
    pub const CAR_NORMAL: usize = 0x130;
    pub const CONTACT: usize = 0x140;
    pub const AXLE_TOP: usize = 0x150;
    pub const AXLE_BOTTOM: usize = 0x160;
    pub const POINT_VELOCITY: usize = 0x170;
    pub const ANGLE: usize = 0x1a0;
    pub const SPIN: usize = 0x1a4;
    pub const GRIP: usize = 0x1e0;
    pub const SLIPPING: usize = 0x1e4;
    pub const SLIDING: usize = 0x1e8;
    pub const LOCKED: usize = 0x1ec;
    pub const TORQUE: usize = 0x1f0;
    pub const LAST_TORQUE: usize = 0x1f4;
    pub const SLIP_RATIO: usize = 0x20c;
    pub const SLIP_ANGLE: usize = 0x210;
    pub const SLIP_TAN: usize = 0x214;
    pub const VELOCITY: usize = 0x220;
    pub const LAST_VELOCITY: usize = 0x230;
    pub const FORCE: usize = 0x240;
    pub const LAST_FORCE: usize = 0x250;
    pub const STEER_ANGLE: usize = 0x260;
    pub const SIDE_RATIO: usize = 0x2b0;
    pub const FRICTION_SCALE: usize = 0x9b4;
    pub const CAMBER: usize = 0x9bc;
}

/// One tire's bytes in a row: the head block and the tail block, addressed by tire offset.
struct Raw {
    head: Vec<u8>,
    tail: Vec<u8>,
}

impl Raw {
    fn bytes(&self, o: usize) -> &[u8] {
        if o >= TAIL { &self.tail[o - TAIL..] } else { &self.head[o..] }
    }
    fn word(&self, o: usize) -> u32 {
        u32::from_le_bytes(self.bytes(o)[..4].try_into().unwrap())
    }
    fn f(&self, o: usize) -> F {
        F(f32::from_bits(self.word(o)))
    }
    fn flag(&self, o: usize) -> bool {
        self.word(o) != 0
    }
    fn v(&self, o: usize) -> Vf {
        Vf::from_bits([0, 4, 8, 12].map(|i| self.word(o + i)))
    }
}

/// A row: its frame counter, dt, the body, and the raw blocks.
struct Row {
    frame_counter: u32,
    dt: F,
    body: [[f32; 4]; 4],
    car: Vec<u8>,
    engine: Vec<u8>,
    tires: [Raw; 4],
}

impl Row {
    fn car_word(&self, addr: u32) -> u32 {
        let o = (addr - CAR - CAR_RAW) as usize;
        u32::from_le_bytes(self.car[o..o + 4].try_into().unwrap())
    }
    fn engine_word(&self, o: u32) -> u32 {
        let o = o as usize;
        u32::from_le_bytes(self.engine[o..o + 4].try_into().unwrap())
    }
    /// A float the tire points at in the car (its brake or steer input), if the pointer is set.
    fn input(&self, tire: usize, field: usize) -> Option<F> {
        let p = self.tires[tire].word(field);
        (p != 0).then(|| F(f32::from_bits(self.car_word(p))))
    }
    /// The torque the engine handed this tire this tick, if a drive slot names it.
    fn engine_torque(&self, tire: usize) -> F {
        let me = self.car_word(CAR + TIRES[tire]);
        (0..4)
            .find(|&k| self.engine_word(ENGINE_SLOTS + 4 * k) == me)
            .map_or(F::ZERO, |k| F(f32::from_bits(self.engine_word(ENGINE_TORQUE + 4 * k))))
    }
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
}

fn load_rows(path: &std::path::Path) -> Option<Vec<Row>> {
    let Ok(text) = std::fs::read_to_string(path) else {
        skip(&format!(
            "no {} — S149b's capture.sh records it (research/journal/2026-10-29-s149b-car-tire-model)",
            path.display()
        ));
        return None;
    };
    let mut lines = text.lines();
    let head: HashMap<&str, usize> = lines.next()?.split(',').enumerate().map(|(i, h)| (h, i)).collect();
    let rows = lines
        .map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            let col = |n: &str| c[head[n]];
            Row {
                frame_counter: col("frame_counter").parse().unwrap(),
                dt: F(col("dt").parse::<f32>().unwrap()),
                body: ["body_pos", "body_vel", "body_quat", "body_rate"]
                    .map(|b| ["x", "y", "z", "w"].map(|l| col(&format!("{b}.{l}")).parse::<f32>().unwrap())),
                car: unhex(col("car_raw")),
                engine: unhex(col("engine_raw")),
                tires: NAMES.map(|n| Raw {
                    head: unhex(col(&format!("tire_{n}_raw"))),
                    tail: unhex(col(&format!("tire_{n}_tail"))),
                }),
            }
        })
        .collect();
    Some(rows)
}

/// A car's file, out of `machine.res` in MX.PSR.
fn car_file(kit: &Kit, kind: Kind) -> Option<Vec<u8>> {
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = File::open(&at) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("machine.res").expect("machine.res is in MX.PSR");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("machine.res");
    let e = res
        .entries
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(kind.phy()))
        .unwrap_or_else(|| panic!("machine.res has no {}", kind.phy()));
    Some(res.read(&bytes, e).expect("its bytes"))
}

/// The state the model carries from one tick to the next, out of a row.
fn state(r: &Raw) -> TireState {
    TireState {
        surface: r.bytes(at::SURFACE)[0] as usize,
        grounded: r.flag(at::GROUNDED),
        handoff: r.flag(at::HANDED).then(|| r.f(at::HANDED_LOAD)),
        compression: r.f(at::COMPRESSION),
        last_compression: r.f(at::LAST_COMPRESSION),
        load: r.f(at::LOAD),
        contact: r.v(at::CONTACT),
        angle: r.f(at::ANGLE),
        spin: r.f(at::SPIN),
        torque: r.f(at::TORQUE),
        last_torque: r.f(at::LAST_TORQUE),
        slip_ratio: r.f(at::SLIP_RATIO),
        slip_angle: r.f(at::SLIP_ANGLE),
        slip_tan: r.f(at::SLIP_TAN),
        velocity: r.v(at::VELOCITY),
        last_velocity: r.v(at::LAST_VELOCITY),
        force: r.v(at::FORCE),
        last_force: r.v(at::LAST_FORCE),
        steer: r.f(at::STEER_ANGLE),
        side_ratio: r.f(at::SIDE_RATIO),
        camber: r.f(at::CAMBER),
        friction_scale: r.f(at::FRICTION_SCALE),
        low_speed_grip: r.flag(at::GRIP),
        slipping: r.flag(at::SLIPPING),
        sliding: r.flag(at::SLIDING),
        locked: r.flag(at::LOCKED),
    }
}

/// Mismatches per word, with the first few spelled out.
#[derive(Default)]
struct Tally {
    checked: usize,
    bad: HashMap<&'static str, usize>,
    first: Vec<String>,
}

impl Tally {
    fn word(&mut self, what: &'static str, fc: u32, tire: usize, got: u32, want: u32) {
        self.checked += 1;
        if got != want {
            *self.bad.entry(what).or_default() += 1;
            if self.first.len() < 24 {
                self.first.push(format!(
                    "fc {fc} {} {what}: port {:#010x} ({}) original {:#010x} ({})",
                    NAMES[tire],
                    got,
                    f32::from_bits(got),
                    want,
                    f32::from_bits(want)
                ));
            }
        }
    }
    fn f(&mut self, what: &'static str, fc: u32, tire: usize, got: F, want: F) {
        self.word(what, fc, tire, got.to_bits(), want.to_bits());
    }
    fn flag(&mut self, what: &'static str, fc: u32, tire: usize, got: bool, want: bool) {
        self.word(what, fc, tire, got as u32, want as u32);
    }
    fn v(&mut self, what: &'static str, fc: u32, tire: usize, got: Vf, want: Vf, lanes: &[usize]) {
        for &i in lanes {
            self.word(what, fc, tire, got.to_bits()[i], want.to_bits()[i]);
        }
    }
}

/// Rows the recorder read while the original was part way through a tire step, so their words come
/// from different substeps (the journal has the evidence): the truck's fc 1031 holds fr's new load
/// beside the slip tangent and camber of fc 1030 and a force with nothing added yet; fc 1105 holds a
/// ground normal a substep ahead of every tire's camber. The whole-step test reads them as inputs only.
const TORN: &[(&str, u32)] = &[("truck.csv", 1031), ("truck.csv", 1105)];

/// What a replay saw, so a test can say what its words covered.
#[derive(Default, Debug)]
struct Seen {
    ticks: usize,
    torn: usize,
    grounded: usize,
    airborne: usize,
    fast: usize,
    slow: usize,
    braked: usize,
    handed: usize,
    gripping: usize,
    bottomed: usize,
    locked: usize,
    abs: usize,
}

impl Seen {
    fn tire(&mut self, tire: &car::Tire, s: &TireState, brake: Option<F>) {
        self.abs += (tire.abs_enabled != 0 && s.grounded && brake.is_some_and(|b| b > 0.0)) as usize;
        if s.grounded {
            self.grounded += 1;
        } else {
            self.airborne += 1;
        }
        if F(s.velocity.z()).abs() > 20.0 {
            self.fast += 1;
        } else {
            self.slow += 1;
        }
        self.braked += brake.is_some_and(|b| b > 0.0) as usize;
        self.handed += (!s.grounded && s.handoff.is_some()) as usize;
        self.gripping += s.low_speed_grip as usize;
        self.locked += s.locked as usize;
    }
}

/// The car's file, its rows, and the tables.
struct Drive {
    kit: Kit,
    car: Car,
    rows: Vec<Row>,
    acos: AcosTable,
    curve: CurveTable,
}

fn drive(kind: Kind, csv: &str) -> Option<Drive> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let rows = load_rows(&kit.ctx("capture").join("s149b").join(csv))?;
    let car = Car::read_bytes(&car_file(&kit, kind)?, kind);
    Some(Drive { kit, car, rows, acos: AcosTable::build(), curve: CurveTable::build() })
}

fn ground(r: &Raw) -> Option<Ground> {
    (r.flag(at::SOLID) || r.flag(at::TOUCHING)).then(|| Ground {
        point: r.v(at::GROUND_POINT),
        normal: r.v(at::NORMAL),
        solid: r.flag(at::SOLID),
    })
}

fn report(csv: &str, what: &str, tally: &Tally, seen: &Seen) {
    let mut bad: Vec<_> = tally.bad.iter().collect();
    bad.sort();
    assert!(
        bad.is_empty(),
        "{csv} ({what}): {} of {} words off: {bad:?}\n{}",
        bad.iter().map(|(_, n)| **n).sum::<usize>(),
        tally.checked,
        tally.first.join("\n")
    );
    eprintln!("{csv} ({what}): {} words bit-exact over {seen:?}", tally.checked);
    assert!(tally.checked > 0 && seen.ticks > 0, "{csv}: nothing was compared");
}

/// The words a row fixes on its own, against the row.
fn from_the_row(kind: Kind, csv: &str) -> Option<Seen> {
    let d = drive(kind, csv)?;
    let mut tally = Tally::default();
    let mut seen = Seen::default();
    for row in &d.rows {
        let fc = row.frame_counter;
        if TORN.contains(&(csv, fc)) {
            seen.torn += 1;
            continue;
        }
        seen.ticks += 1;
        let front = [state(&row.tires[car::FRONT_LEFT]), state(&row.tires[car::FRONT_RIGHT])];
        for k in 0..4 {
            let (tire, r) = (&d.car.tires[k], &row.tires[k]);
            let want = state(r);
            seen.tire(tire, &want, row.input(k, at::BRAKE));

            // The load the car hands a rear tire, from the fronts as they ended this substep.
            if k >= car::REAR_LEFT {
                let handed = car::hand_off(&front[0], &front[1]);
                tally.flag("handed", fc, k, handed.is_some(), want.handoff.is_some());
                if let (Some(got), Some(w)) = (handed, want.handoff) {
                    tally.f("handed load", fc, k, got, w);
                }
            }

            // The suspension's contact and load from the axle and ground points it read. A rear off
            // the ground that was handed a load had its lower axle point rewritten on the way out, so
            // the row no longer holds the one the contact read: only its load is checked.
            let mut s = want;
            if !want.grounded && want.handoff.is_some() {
                tire.spring(&mut s, r.v(at::POINT_VELOCITY), r.v(at::NORMAL));
                tally.f("load", fc, k, s.load, want.load);
            } else {
                let push = tire.contact(&mut s, (r.v(at::AXLE_TOP), r.v(at::AXLE_BOTTOM)), ground(r).as_ref());
                seen.bottomed += push.is_some() as usize;
                tally.flag("grounded", fc, k, s.grounded, want.grounded);
                if s.grounded {
                    tally.v("contact", fc, k, s.contact, want.contact, &[0, 1, 2]);
                }
                tire.spring(&mut s, r.v(at::POINT_VELOCITY), r.v(at::NORMAL));
                tally.f("load", fc, k, s.load, want.load);
            }

            // The forces from the slip, load and camber the substep left.
            let mut s = want;
            s.force = Vf([0.0, 0.0, 0.0, 1.0]);
            if F(s.velocity.z()).abs() <= 20.0 && s.low_speed_grip {
                s.force[0] = (F(s.force.x()) - car::grip(&s)).0;
            }
            if s.grounded {
                tire.surface_force(&mut s, &d.curve);
            }
            tally.v("force", fc, k, s.force, want.force, &[0, 1, 2, 3]);

            // The camber, and a fast tire's slip.
            let mut s = want;
            s.camber = tire.camber((F(r.v(at::CAR_NORMAL).x()), F(r.v(at::CAR_NORMAL).y())), &d.acos);
            if s.grounded && F(s.velocity.z()).abs() > 20.0 {
                tire.slip_at_speed(&mut s, &d.acos, &d.curve);
                car::clamp_slip(&mut s);
                tally.f("slip ratio", fc, k, s.slip_ratio, want.slip_ratio);
                tally.f("slip angle", fc, k, s.slip_angle, want.slip_angle);
                tally.f("slip tan", fc, k, s.slip_tan, want.slip_tan);
            }
            car::clamp_slip(&mut s);
            tally.f("camber", fc, k, s.camber, want.camber);
        }
    }
    report(csv, "from the row", &tally, &seen);
    Some(seen)
}

/// The substep the world cuts a frame into: `(int)(dt * 115) + 1` of them.
fn substep(dt: F) -> F {
    dt / F(fpu::from_i32((dt * F(115.0)).to_i32() + 1))
}

fn ram(ee: &Ee, a: u32, n: usize) -> Vec<u8> {
    let at = (a & 0x0fff_ffff) as usize;
    ee.ram[at..at + n].to_vec()
}

fn lay(ee: &mut Ee, a: u32, bytes: &[u8]) {
    ee.load(a, bytes);
}

/// The row's body position, velocity, orientation and rate onto the tire's body.
fn lay_body(ee: &mut Ee, body: u32, words: &[[f32; 4]; 4]) {
    for (o, v) in [0x60u32, 0x70, 0x170, 0x1b0].into_iter().zip(words) {
        let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        lay(ee, body + o, &bytes);
    }
}

/// The whole tire step, the port against the original's under the interpreter.
fn the_whole_step(kind: Kind, csv: &str, image: &str) -> Option<Seen> {
    let d = drive(kind, csv)?;
    let path = d.kit.ctx("states").join(image);
    if !path.exists() {
        skip(&format!("no {} — S149a's switch recipe makes it", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    let addr = |n: &str| d.kit.addr(n).unwrap_or_else(|e| panic!("{e}"));
    let (step, suspension, frame, contact_velocity) = (
        addr("car_tire_step"),
        addr("car_tire_suspension"),
        addr("car_tire_frame"),
        addr("car_tire_contact_velocity"),
    );
    let mut tally = Tally::default();
    let mut seen = Seen::default();
    for pair in d.rows.windows(2) {
        let (prev, row) = (&pair[0], &pair[1]);
        if row.frame_counter != prev.frame_counter + 1 {
            continue;
        }
        seen.ticks += 1;
        let fc = row.frame_counter;
        let h = substep(row.dt);
        lay(&mut ee, CAR + CAR_RAW, &row.car);
        let tires = TIRES.map(|t| row.car_word(CAR + t));
        for k in 0..4 {
            let (tire, r, was) = (&d.car.tires[k], &row.tires[k], &prev.tires[k]);
            let at_tire = tires[k];

            // The tire as the substep before left it, with what this substep laid in before the model.
            let mut head = was.head.clone();
            let mut tail = was.tail.clone();
            for (o, n) in [
                (at::TOP_WHEEL_SPEED, 4),
                (at::SOLID, 8),
                (at::HANDED, 16),
                (at::SURFACE, 4),
                (at::GROUND_POINT, 48),
                (at::CONTACTS, 8),
            ] {
                head[o..o + n].copy_from_slice(&r.head[o..o + n]);
            }
            let fs = at::FRICTION_SCALE - TAIL;
            tail[fs..fs + 4].copy_from_slice(&r.tail[fs..fs + 4]);
            let torque = row.engine_torque(k);
            head[at::TORQUE..at::TORQUE + 4].copy_from_slice(&torque.to_bits().to_le_bytes());
            lay(&mut ee, at_tire, &head);
            lay(&mut ee, at_tire + TAIL as u32, &tail);
            let body = r.word(at::BODY);
            lay_body(&mut ee, body, &row.body);

            // The helpers this task does not port hand back the row's words; the body's force calls
            // apply nothing.
            let point_velocity = r.head[at::POINT_VELOCITY..at::POINT_VELOCITY + 16].to_vec();
            ee.hooks.insert(
                contact_velocity,
                Box::new(move |ee| {
                    let t = ee.r(4) as u32;
                    ee.load(t + at::POINT_VELOCITY as u32, &point_velocity);
                }),
            );
            let (velocity, axes) = (
                r.head[at::VELOCITY..at::VELOCITY + 16].to_vec(),
                r.head[at::FRAME..at::FRAME + 48].to_vec(),
            );
            ee.hooks.insert(
                frame,
                Box::new(move |ee| {
                    let t = ee.r(4) as u32;
                    ee.load(t + at::VELOCITY as u32, &velocity);
                    ee.load(t + at::FRAME as u32, &axes);
                }),
            );
            let vtable = ee.u32_at(ee.u32_at(body + 0x100));
            for slot in [0x3c, 0x5c, 0x64, 0x68] {
                ee.hooks.insert(ee.u32_at(vtable + slot), Box::new(|_| {}));
            }
            // A rear off the ground that is handed a load rewrites its lower axle point on the way
            // out, so the points the contact read come from a suspension run without the hand-off.
            let axle = if head[at::HANDED] != 0 {
                let mut alone = head.clone();
                alone[at::HANDED..at::HANDED + 4].fill(0);
                lay(&mut ee, at_tire, &alone);
                ee.call(suspension, &[at_tire as u64], &[h.0], 1_000_000);
                let alone = Raw { head: ram(&ee, at_tire, head.len()), tail: tail.clone() };
                lay(&mut ee, at_tire, &head);
                lay_body(&mut ee, body, &row.body);
                Some((alone.v(at::AXLE_TOP), alone.v(at::AXLE_BOTTOM)))
            } else {
                None
            };
            ee.call(step, &[at_tire as u64], &[h.0], 1_000_000);
            let after = Raw {
                head: ram(&ee, at_tire, was.head.len()),
                tail: ram(&ee, at_tire + TAIL as u32, was.tail.len()),
            };
            let want = state(&after);

            // The port, from the same tire and inputs.
            let mut s = state(&Raw { head, tail });
            let axle = axle.unwrap_or((after.v(at::AXLE_TOP), after.v(at::AXLE_BOTTOM)));
            let push = tire.contact(&mut s, axle, ground(r).as_ref());
            seen.bottomed += push.is_some() as usize;
            tire.spring(&mut s, r.v(at::POINT_VELOCITY), r.v(at::NORMAL));
            tire.settle(&mut s, h);
            let brake = row.input(k, at::BRAKE);
            tire.roll(&mut s, brake);
            let car_normal = r.v(at::CAR_NORMAL);
            tire.forces(
                &mut s,
                h,
                brake,
                row.input(k, at::STEER),
                r.f(at::TOP_WHEEL_SPEED),
                r.v(at::VELOCITY),
                (F(car_normal.x()), F(car_normal.y())),
                &d.acos,
                &d.curve,
            );
            seen.tire(tire, &s, brake);
            compare(&mut tally, fc, k, &s, &want);
        }
    }
    report(csv, "the whole step", &tally, &seen);
    Some(seen)
}

/// Every word the port writes, against the original's.
fn compare(t: &mut Tally, fc: u32, k: usize, s: &TireState, w: &TireState) {
    t.flag("grounded", fc, k, s.grounded, w.grounded);
    if s.grounded {
        t.v("contact", fc, k, s.contact, w.contact, &[0, 1, 2]);
    }
    t.f("compression", fc, k, s.compression, w.compression);
    t.f("last compression", fc, k, s.last_compression, w.last_compression);
    t.f("load", fc, k, s.load, w.load);
    t.f("torque", fc, k, s.torque, w.torque);
    t.f("last torque", fc, k, s.last_torque, w.last_torque);
    t.f("spin", fc, k, s.spin, w.spin);
    t.f("slip ratio", fc, k, s.slip_ratio, w.slip_ratio);
    t.f("slip angle", fc, k, s.slip_angle, w.slip_angle);
    t.f("slip tan", fc, k, s.slip_tan, w.slip_tan);
    t.f("camber", fc, k, s.camber, w.camber);
    t.v("velocity", fc, k, s.velocity, w.velocity, &[0, 1, 2, 3]);
    t.v("last velocity", fc, k, s.last_velocity, w.last_velocity, &[0, 1, 2, 3]);
    t.v("force", fc, k, s.force, w.force, &[0, 1, 2, 3]);
    t.v("last force", fc, k, s.last_force, w.last_force, &[0, 1, 2, 3]);
    t.f("steer", fc, k, s.steer, w.steer);
    t.f("wheel angle", fc, k, s.angle, w.angle);
    t.f("side ratio", fc, k, s.side_ratio, w.side_ratio);
    t.flag("low-speed grip", fc, k, s.low_speed_grip, w.low_speed_grip);
    t.flag("slipping", fc, k, s.slipping, w.slipping);
    t.flag("sliding", fc, k, s.sliding, w.sliding);
    t.flag("locked", fc, k, s.locked, w.locked);
}

/// ABS, which no retail car turns on: every braked row's tire with ABS forced on and a slip window
/// laid in, through the original's rolling resistance and brake against the port's.
fn the_abs_window(kind: Kind, csv: &str, image: &str) -> Option<usize> {
    let d = drive(kind, csv)?;
    let path = d.kit.ctx("states").join(image);
    if !path.exists() {
        skip(&format!("no {} — S149a's switch recipe makes it", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    let roll = d.kit.addr("car_tire_roll").unwrap_or_else(|e| panic!("{e}"));
    let mut tally = Tally::default();
    let (mut braked, mut inside) = (0, 0);
    for row in &d.rows {
        let fc = row.frame_counter;
        lay(&mut ee, CAR + CAR_RAW, &row.car);
        for k in 0..4 {
            let r = &row.tires[k];
            let brake = row.input(k, at::BRAKE);
            if !(r.flag(at::GROUNDED) && brake.is_some_and(|b| b > 0.0)) {
                continue;
            }
            braked += 1;
            let at_tire = row.car_word(CAR + TIRES[k]);
            // A window the slip runs through, and one with no width (which leaves the brake whole).
            for (lo, hi) in [(F(0.02), F(0.3)), (F(0.1), F(0.1))] {
                let slip = r.f(at::SLIP_RATIO).abs();
                inside += (lo < hi && lo < slip && slip < hi) as usize;
                let mut head = r.head.clone();
                head[at::ABS..at::ABS + 4].copy_from_slice(&1u32.to_le_bytes());
                head[at::ABS + 4..at::ABS + 8].copy_from_slice(&lo.to_bits().to_le_bytes());
                head[at::ABS + 8..at::ABS + 12].copy_from_slice(&hi.to_bits().to_le_bytes());
                lay(&mut ee, at_tire, &head);
                ee.call(roll, &[at_tire as u64], &[], 100_000);
                let want = F::from_bits(ee.u32_at(at_tire + at::TORQUE as u32));
                let mut tire = d.car.tires[k].clone();
                (tire.abs_enabled, tire.min_braking_slip, tire.max_braking_slip) = (1, lo, hi);
                let mut s = state(r);
                tire.roll(&mut s, brake);
                tally.f("abs torque", fc, k, s.torque, want);
            }
        }
    }
    let seen = Seen { ticks: braked, abs: inside, ..Seen::default() };
    report(csv, "ABS", &tally, &seen);
    assert!(inside > 0, "{csv}: no braked slip fell inside the window");
    Some(braked)
}

#[test]
fn the_baja_tires_match_the_capture() {
    from_the_row(Kind::Baja, "baja.csv");
}

#[test]
fn the_buggy_tires_match_the_capture() {
    from_the_row(Kind::Buggy, "buggy.csv");
}

#[test]
fn the_truck_tires_match_the_capture() {
    from_the_row(Kind::MonsterTruck, "truck.csv");
}

#[test]
fn the_baja_tire_step_matches_the_original() {
    the_whole_step(Kind::Baja, "baja.csv", "s149a-baja-switched.ee.bin");
}

#[test]
fn the_buggy_tire_step_matches_the_original() {
    the_whole_step(Kind::Buggy, "buggy.csv", "s149a-buggy-switched.ee.bin");
}

#[test]
fn the_truck_tire_step_matches_the_original() {
    the_whole_step(Kind::MonsterTruck, "truck.csv", "s149a-truck-switched.ee.bin");
}

#[test]
fn abs_matches_the_original_on_every_car() {
    the_abs_window(Kind::Baja, "baja.csv", "s149a-baja-switched.ee.bin");
    the_abs_window(Kind::Buggy, "buggy.csv", "s149a-buggy-switched.ee.bin");
    the_abs_window(Kind::MonsterTruck, "truck.csv", "s149a-truck-switched.ee.bin");
}
