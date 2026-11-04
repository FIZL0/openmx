//! MD63d — MX vs. ATV's `GravityScale` and its sixteen `FrictionZone`s, proved against MX vs. ATV's
//! own `[Mass Properties]` reader.
//!
//! The oracle is SLUS-21104 off a RAM image of a Free Ride in it (`context/mxvatv-ps2/state04.ee.bin`,
//! `tests/mxvatv_step.rs`' image). The reader runs as it is, with the rigid body's vtable and the
//! collision record's setters: only the ini's lookups are hooked, handing it the generated file's text,
//! so the game's own number parse reads every value. Its gravity block and the record's frictions are
//! compared word for word with [`handling::MassPart`], over 10,000 generated files.
//!
//! Without the image the oracle test skips (and says so).

use game_sim::camera::CurveTable;
use game_sim::handling::{self, MassPart, mass};
use game_sim::phy::{Ini, MAX_SURFACES, number};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// The rigid body's vtable in the image (`tests/mxvatv_step.rs`' check that it is the image).
const BIKE_VTABLE: u32 = 0x003d_0450;
const BODY_VTABLE: u32 = 0x003c_d200;
const WRECK_ENTRY: u32 = 0x004e_2260;
/// The `[Mass Properties]` reader, the ini's string and flag lookups it reads through, the section
/// select, and the model's bounding box.
const READER: u32 = 0x0049_3210;
const INI_STRING: u32 = 0x0014_2ce0;
const INI_FLAG: u32 = 0x0014_2ed0;
const INI_SECTION: u32 = 0x0014_4140;
const MODEL_BOX: u32 = 0x0019_61f0;
/// The kernel's syscall stubs.
const SYSCALLS: (u32, u32) = (0x0033_e800, 0x0033_f400);

/// Where the harness lays the vehicle out: the top of RAM, under the interpreter's stack.
const OBJ: u32 = 0x01c0_0000;
const GRAVITY: u32 = 0x01c0_0400;
const RECORD: u32 = 0x01c0_0800;
const BODY: u32 = 0x01c0_0c00;
const INI: u32 = 0x01c0_1000;
const REGIONS: [(u32, u32); 4] = [(OBJ, 0x400), (GRAVITY, 0x100), (RECORD, 0x400), (BODY, 0x400)];

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2").join(format!("state04.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!(
            "{} is not there: unzip eeMemory.bin from MX vs. ATV's save state 4 (SLUS-21104 (95BB1901).04.p2s, \
             ps2-decomp-kit-slots/pcsx2/s9) to it (s1 has it)",
            path.display()
        ));
        return None;
    }
    let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    if ee.u32_at(BIKE_VTABLE + 0x368) != WRECK_ENTRY {
        panic!("{} is not the MX vs. ATV image these addresses are from", path.display());
    }
    Some(ee)
}

fn cstr(ee: &Ee, mut a: u32) -> String {
    let mut s = String::new();
    loop {
        let c = ee.u8_at(a);
        if c == 0 {
            return s;
        }
        s.push(char::from(c));
        a += 1;
    }
}

fn set_byte(ee: &mut Ee, a: u32, b: u8) {
    let (at, shift) = (a & !3, 8 * (a & 3));
    ee.set_u32(at, ee.u32_at(at) & !(0xff << shift) | u32::from(b) << shift);
}

/// The ini's lookups, answered from one file's `[Mass Properties]`: the string one copies the value's
/// text into the caller's buffer (the reader parses it), the flag one writes 1 for `True`.
fn hook_ini(ee: &mut Ee, file: &Rc<RefCell<BTreeMap<String, String>>>) {
    let f = file.clone();
    ee.hooks.insert(INI_STRING, Box::new(move |ee| {
        let key = cstr(ee, ee.r(5) as u32);
        let (buf, len) = (ee.r(6) as u32, ee.r(7) as u32);
        let found = match f.borrow().get(&key) {
            Some(text) => {
                assert!((text.len() as u32) < len, "{key}'s text does not fit");
                for (i, b) in text.bytes().chain([0]).enumerate() {
                    set_byte(ee, buf + i as u32, b);
                }
                1
            }
            None => 0,
        };
        ee.set(2, found);
    }));
    let f = file.clone();
    ee.hooks.insert(INI_FLAG, Box::new(move |ee| {
        let key = cstr(ee, ee.r(5) as u32);
        let found = match f.borrow().get(&key) {
            Some(text) => {
                ee.set_u32(ee.r(6) as u32, u32::from(text.eq_ignore_ascii_case("true")));
                1
            }
            None => 0,
        };
        ee.set(2, found);
    }));
    ee.hooks.insert(INI_SECTION, Box::new(|ee| ee.set(2, 1)));
    // The kernel calls the C library's number parse and print make (its locks): stubs of
    // `addiu v1, n; syscall`, answered 0.
    for a in (SYSCALLS.0..SYSCALLS.1).step_by(4) {
        if ee.u32_at(a) >> 16 == 0x2403 && ee.u32_at(a + 4) == 0x0000_000c {
            ee.hooks.insert(a, Box::new(|ee| ee.set(2, 0)));
        }
    }
    // The model's box, which only the inertia the bike's own loader replaces is worked out from.
    ee.hooks.insert(MODEL_BOX, Box::new(|ee| {
        for (k, v) in [1.5f32, 2.0, 3.5, 1.0].into_iter().enumerate() {
            ee.set_f32(ee.r(6) as u32 + 4 * k as u32, v);
        }
    }));
}

fn reset(ee: &mut Ee) {
    for (base, len) in REGIONS {
        for o in (0..len).step_by(4) {
            ee.set_u32(base + o, 0);
        }
    }
    ee.set_u32(OBJ + 0xb8, BODY);
    ee.set_u32(OBJ + 0xc8, RECORD);
    ee.set_u32(OBJ + 0xe8, GRAVITY);
    ee.set_u32(BODY + 0x110, BODY_VTABLE);
    // No collision shapes and no model library: the reader's tail for a part with its collision off
    // then does nothing.
    for o in [0xcc, 0xd0, 0xd4, 0xdc, 0xe4] {
        ee.set_u32(OBJ + o, 0);
    }
}

#[derive(Clone, Copy, Default)]
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
    fn one_in(&mut self, n: u32) -> bool {
        self.bits() % n == 0
    }
    /// A value as a file writes one: whole, a few places, many, signed, sometimes on 0 or 1.
    fn number(&mut self, lo: f32, hi: f32) -> String {
        let v = lo + (hi - lo) * self.unit();
        match self.bits() % 8 {
            0 => "0".into(),
            1 => "1".into(),
            2 => format!("{}", v.round() as i32),
            3 => format!("{v:.1}"),
            4 => format!("{v:.2}"),
            5 => format!("{v:.4}"),
            6 => format!("{v:.7}"),
            _ => format!("{v}"),
        }
    }
    /// One `[Mass Properties]`, each key the reader asks for here present or not.
    fn file(&mut self) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        if !self.one_in(5) {
            m.insert("Weight".into(), self.number(30.0, 1200.0));
        }
        if !self.one_in(5) {
            m.insert("GravityScale".into(), self.number(-2.0, 3.0));
        }
        for k in 0..MAX_SURFACES {
            if !self.one_in(5) {
                m.insert(format!("FrictionZone{}", k + 1), self.number(0.0, 2.0));
            }
        }
        if !self.one_in(5) {
            m.insert("Friction".into(), self.number(0.0, 2.0));
        }
        match self.bits() % 5 {
            0 => {}
            1 => {
                m.insert("BodyFriction".into(), "-1".into());
            }
            _ => {
                m.insert("BodyFriction".into(), self.number(-1.0, 2.0));
            }
        }
        if self.one_in(4) {
            m.insert("CollisionEnabled".into(), "False".into());
        }
        m
    }
}

fn text(file: &BTreeMap<String, String>) -> String {
    let mut t = String::from("[Mass Properties]\n");
    for (k, v) in file {
        t.push_str(&format!("{k} = {v}\n"));
    }
    t
}

fn path(file: &BTreeMap<String, String>) -> Vec<&'static str> {
    let mut p = vec![];
    p.push(if file.contains_key("Weight") { "weight" } else { "no weight" });
    p.push(if file.contains_key("GravityScale") { "scale" } else { "no scale" });
    p.push(if file.contains_key("FrictionZone1") { "zone 1" } else { "no zone 1" });
    p.push(if file.contains_key("FrictionZone16") { "zone 16" } else { "no zone 16" });
    p.push(if file.contains_key("Friction") { "friction" } else { "no friction" });
    p.push(match file.get("BodyFriction").map(String::as_str) {
        None => "no body friction",
        Some("-1") => "body friction -1",
        Some(_) => "body friction",
    });
    p
}

fn word(ee: &Ee, a: u32) -> u32 {
    ee.u32_at(a)
}

/// SLUS-21104's `[Mass Properties]` reader, [`MassPart::read`]: 10,000 files, its gravity block
/// (`GravityScale` and the force) and the collision record's frictions (`Friction`, `BodyFriction`,
/// the sixteen zones) word for word.
#[test]
fn mxvatv_mass_properties_gravity_and_frictions_are_the_originals() {
    let Some(mut ee) = image() else { return };
    let file = Rc::new(RefCell::new(BTreeMap::new()));
    hook_ini(&mut ee, &file);
    let mut g = Gen(0x0d63_000d);
    let mut tally = BTreeMap::<&str, u32>::new();
    for k in 0..10_000 {
        let f = g.file();
        for p in path(&f) {
            *tally.entry(p).or_default() += 1;
        }
        *file.borrow_mut() = f.clone();
        reset(&mut ee);
        ee.call(READER, &[u64::from(OBJ), u64::from(INI)], &[], 2_000_000);

        let m = MassPart::read(&Ini::parse(&text(&f)));
        let mut want = vec![("GravityScale", GRAVITY, m.gravity_scale.0.to_bits())];
        for (i, lane) in ["x", "y", "z", "w"].into_iter().enumerate() {
            want.push((lane, GRAVITY + 0x40 + 4 * i as u32, m.gravity.0[i].to_bits()));
        }
        want.push(("Friction", RECORD + 0x1a0, m.friction.0.to_bits()));
        want.push(("BodyFriction", RECORD + 0x1a4, m.body_friction.0.to_bits()));
        for (z, v) in m.surface_friction.iter().enumerate() {
            want.push(("a FrictionZone", RECORD + 0x1a8 + 4 * z as u32, v.0.to_bits()));
        }
        for (name, at, bits) in want {
            let was = word(&ee, at);
            assert_eq!(
                was,
                bits,
                "file {k}: {name} at {at:#x} is {} in the original and {} in the port\n{}",
                f32::from_bits(was),
                f32::from_bits(bits),
                text(&f)
            );
        }
    }
    for want in ["weight", "no weight", "scale", "no scale", "zone 1", "no zone 1", "zone 16", "no zone 16", "friction",
                 "no friction", "body friction", "no body friction", "body friction -1"] {
        let n = tally.get(want).copied().unwrap_or(0);
        assert!(n >= 500, "only {n} file(s) had {want:?}: {tally:?}");
    }
}

/// The ride's bike gets the reader's gravity and frictions, and MX Unleashed's `Gravity` and
/// `TerrainFriction` stand in for nothing (no image needed).
#[test]
fn an_mx_phy_rides_on_its_gravity_scale_and_friction_zones() {
    let mut t = String::from("[Mass Properties]\nWeight = 400\nGravityScale = 1.03\nFriction = 0.5\nBodyFriction = -1\n");
    for k in 0..MAX_SURFACES {
        t.push_str(&format!("FrictionZone{} = 0.{}\n", k + 1, k + 1));
    }
    let phy = handling::mxvatv_phy(t.as_bytes(), &CurveTable::build());
    let m = MassPart::read(&Ini::parse(&t));
    assert_eq!(phy.handling, handling::Handling::MxVsAtv);
    assert_eq!(phy.mv.gravity, m.gravity);
    assert_eq!(phy.mv.gravity, Vf::new(0.0, (-number("1.03") * F(400.0)).0, 0.0, 1.0));
    assert_eq!(phy.mass.gravity, number("1.03"));
    assert_eq!(phy.collisions.friction, F(0.5));
    assert_eq!(phy.collisions.body_friction, F(0.5), "a -1 body friction is the friction");
    assert_eq!(phy.collisions.surface_friction, m.surface_friction);
    assert_ne!(m.surface_friction[0], m.surface_friction[15], "each zone its own");
    // The stand-ins are gone: no `Gravity`, no `TerrainFriction` comes out of the move.
    let moved = handling::mxvatv_ini(&Ini::parse(&t));
    assert_eq!(moved.section("Mass Properties").float("Gravity"), None);
    assert_eq!(moved.section("Collisions").float("TerrainFriction"), None);
    // MX Unleashed's own files keep one friction on every surface.
    let mxu = game_sim::phy::Phy::read(&Ini::parse("[Collisions]\nTerrainFriction = 0.7\n"), &CurveTable::build());
    assert_eq!(mxu.collisions.surface_friction, [number("0.7"); MAX_SURFACES]);
}

// --- F31b4c: every word of the reader ---------------------------------------------------------------

/// The curve table the record's facing reads, and its step.
const CURVE_TABLE: u32 = 0x003e_ab20;
const CURVE_STEP: u32 = 0x003e_a9c8;
const CURVE_INV_STEP: u32 = 0x003e_a9d0;

/// The image's own sine and cosine table, which must be the one the port builds.
fn curve(ee: &Ee) -> CurveTable {
    let built = CurveTable::build();
    assert_eq!(ee.f32_at(CURVE_STEP).to_bits(), 0.0010473721f32.to_bits(), "the curve table's step");
    assert_eq!(ee.f32_at(CURVE_INV_STEP).to_bits(), 954.77045f32.to_bits(), "the curve table's inverse step");
    let at = ee.u32_at(CURVE_TABLE);
    for (i, e) in built.entries().iter().enumerate().take(game_sim::camera::CURVE_ENTRIES) {
        let got = [ee.f32_at(at + 8 * i as u32), ee.f32_at(at + 8 * i as u32 + 4)];
        assert_eq!(got.map(f32::to_bits), e.map(f32::to_bits), "curve table entry {i}");
    }
    built
}

#[allow(clippy::float_arithmetic)]
impl Gen {
    fn pick<'a>(&mut self, of: &[&'a str]) -> &'a str {
        of[self.bits() as usize % of.len()]
    }
    /// Three or four numbers, comma separated (the file loader takes the blanks out of a line).
    fn vector(&mut self, lo: f32, hi: f32) -> String {
        let n = if self.one_in(3) { 4 } else { 3 };
        let sep = self.pick(&[", ", ","]);
        (0..n).map(|_| self.number(lo, hi)).collect::<Vec<_>>().join(sep)
    }
    fn flag(&mut self) -> String {
        self.pick(&["True", "False", "true", "FALSE", "T", "f", "On", "Off", "ON", "oFF", "One", "Yes", "no", "N", "1",
                    "0", "2", "3", "-1", "0x3", "010", "7"])
            .into()
    }
    /// A value of a key a `-1` stands in for the default of.
    fn or_unset(&mut self, lo: f32, hi: f32) -> String {
        if self.one_in(3) { "-1".into() } else { self.number(lo, hi) }
    }
    /// One `[Mass Properties]` with every key the reader asks for present or not, and the model's
    /// half box.
    fn full(&mut self) -> (BTreeMap<String, String>, [f32; 4]) {
        let mut m = BTreeMap::new();
        let mut put = |g: &mut Gen, k: &str, v: String| {
            if !g.one_in(4) {
                m.insert(k.to_string(), v);
            }
        };
        let w = if self.one_in(10) { self.number(-50.0, 0.0) } else { self.number(30.0, 1200.0) };
        put(self, "Weight", w);
        let v = self.vector(-1.0, 3.0);
        put(self, "CofG", v);
        let v = self.number(-2.0, 3.0);
        put(self, "GravityScale", v);
        let v = self.vector(-0.5, 3.0);
        put(self, "InertiaScale", v);
        let v = self.number(-1.0, 2.0);
        put(self, "Radius", v);
        for k in ["DefaultVelocityDamping", "DefaultAngularRateDamping", "CollisionCofGOffset", "HighSpeedRayOffset",
                  "BodyAngularImpulseScale"] {
            let v = self.vector(-1.0, 3.0);
            put(self, k, v);
        }
        for k in ["CollisionEnabled", "CheckForCollisions", "DetectCollisionsWhenDisabled", "CheckForHighSpeedCollisions",
                  "AngularImpulseEnabled", "BodyAngularImpulseEnabled", "ResetOffEdgeOfWorld", "ResetBelowMinimum"] {
            let v = self.flag();
            put(self, k, v);
        }
        let v = if self.one_in(8) { self.number(-50.0, 0.0) } else { self.number(30.0, 1200.0) };
        put(self, "CollisionWeight", v);
        let v = self.number(0.0, 4.0);
        put(self, "HighSpeedRaySpeedScale", v);
        let v = self.number(0.0, 4.0);
        put(self, "HighSpeedUnpenetrationDistance", v);
        let v = self.number(-400.0, 400.0);
        put(self, "HighSpeedMinCollisionAngle", v);
        let v = self.number(-2.0, 20.0);
        put(self, "SpringStiffness", v);
        let v = self.number(-0.5, 1.5);
        put(self, "SpringDamping", v);
        let v = self.number(0.0, 1.0);
        put(self, "MaxPenetration", v);
        let v = self.number(0.0, 1.0);
        put(self, "Restitution", v);
        let v = self.or_unset(-2.0, 20.0);
        put(self, "BodySpringStiffness", v);
        let v = self.or_unset(-0.5, 1.5);
        put(self, "BodySpringDamping", v);
        let v = self.or_unset(0.0, 1.0);
        put(self, "BodyMaxPenetration", v);
        let v = self.or_unset(0.0, 1.0);
        put(self, "BodyRestitution", v);
        for k in 0..MAX_SURFACES {
            let v = self.number(0.0, 2.0);
            put(self, &format!("FrictionZone{}", k + 1), v);
        }
        let v = self.number(0.0, 2.0);
        put(self, "Friction", v);
        let v = self.or_unset(0.0, 2.0);
        put(self, "BodyFriction", v);
        for k in ["SpringMomentScale", "FrictionMomentScale", "BodySpringMomentScale", "BodyFrictionMomentScale"] {
            let v = match self.bits() % 4 {
                0 => self.pick(&["-1, -1, -1", "-1,-1,-1,1", "-1,-1,-1,-1", "-1, -1, -1, 1.0"]).into(),
                _ => self.vector(-1.0, 2.0),
            };
            put(self, k, v);
        }
        let v = self.number(0.0, 10.0);
        put(self, "ResetTime", v);
        let v = self.number(-100.0, 100.0);
        put(self, "MinimumResetHeight", v);
        for k in ["SpeedThreshold", "AngularRateThreshold", "AccelerationThreshold", "AngularAccelerationThreshold", "SpeedImpulseThreshold",
                  "BodyAngularSpeedImpulseThreshold"] {
            let v = self.number(0.0, 100.0);
            put(self, k, v);
        }
        let mut half = [0.0; 4];
        for h in half.iter_mut() {
            *h = match self.bits() % 12 {
                0 => 0.0,
                1 => -self.unit(),
                _ => 0.05 + 5.0 * self.unit(),
            };
        }
        (m, half)
    }
}

fn full_path(f: &BTreeMap<String, String>, p: &mass::Part) -> Vec<&'static str> {
    let mut t = vec![];
    let get = |k: &str| f.get(k).map(String::as_str);
    t.push(if p.body.mass <= F::ZERO { "no mass" } else { "mass" });
    t.push(if p.record.mass.is_none() { "no collision mass" } else { "collision mass" });
    if p.record.inv_inertia.is_some_and(|v| v.to_bits() == Vf::new(0.0, 0.0, 0.0, 1.0).to_bits()) {
        t.push("record inertia not over 0");
    }
    if p.body.inv_inertia.contains(&F::ZERO) {
        t.push("body inertia lane not over 0");
    }
    for k in ["BodySpringStiffness", "BodySpringDamping", "BodyMaxPenetration", "BodyRestitution", "BodyFriction"] {
        if get(k) == Some("-1") {
            t.push(k);
        }
    }
    for k in ["BodySpringMomentScale", "BodyFrictionMomentScale"] {
        if get(k).is_some_and(|v| ["-1, -1, -1", "-1,-1,-1,1", "-1, -1, -1, 1.0"].contains(&v)) {
            t.push(if k.starts_with("BodySpring") { "body spring moment scale -1" } else { "body friction moment scale -1" });
        }
    }
    if p.record.springs[4] == F::ZERO {
        t.push("spring damping 0");
    }
    if p.record.facing < F::ZERO {
        t.push("facing behind");
    }
    if get("CheckForCollisions") == Some("One") {
        t.push("ON prefix");
    }
    if get("CheckForCollisions").is_some_and(|v| v.starts_with("0")) {
        t.push("strtol base 0");
    }
    t.push(if p.flags.collision_enabled { "collision enabled" } else { "collision disabled" });
    t
}

/// F31b4c — SLUS-21104's `[Mass Properties]` reader, [`mass::Part::read`], over 10,000 files with
/// every key it reads present or not and a model box each: every word of the object, the gravity
/// block, the collision record and the rigid body, the ones it writes and the ones it leaves alone.
/// The ini's flag parse is the game's own here (only the string lookup is hooked).
#[test]
fn mxvatv_mass_properties_every_word_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let curve = curve(&ee);
    let file = Rc::new(RefCell::new(BTreeMap::new()));
    hook_ini(&mut ee, &file);
    ee.hooks.remove(&INI_FLAG);
    let half = Rc::new(RefCell::new([0.0f32; 4]));
    let h = half.clone();
    ee.hooks.insert(MODEL_BOX, Box::new(move |ee| {
        for (k, v) in h.borrow().iter().enumerate() {
            ee.set_f32(ee.r(6) as u32 + 4 * k as u32, *v);
        }
    }));
    let at = mass::Layout { object: OBJ, body: BODY, record: RECORD, gravity: GRAVITY };
    let mut g = Gen(0xf31b_4c00);
    let mut tally = BTreeMap::<&str, u32>::new();
    for k in 0..10_000 {
        let (f, b) = g.full();
        *file.borrow_mut() = f.clone();
        *half.borrow_mut() = b;
        reset(&mut ee);
        ee.set_u32(RECORD + 0xe4, BODY);
        let mut want = BTreeMap::new();
        for (base, len) in REGIONS {
            for o in (0..len).step_by(4) {
                want.insert(base + o, ee.u32_at(base + o));
            }
        }
        ee.call(READER, &[u64::from(OBJ), u64::from(INI)], &[], 2_000_000);

        let ini = Ini::parse(&text(&f));
        let p = mass::Part::read(&ini.section("Mass Properties"), Vf::new(b[0], b[1], b[2], b[3]), &curve);
        for t in full_path(&f, &p) {
            *tally.entry(t).or_default() += 1;
        }
        for (a, mask, v) in p.words(at) {
            let w = want.get_mut(&a).unwrap_or_else(|| panic!("{a:#x} is outside the regions"));
            *w = *w & !mask | v & mask;
        }
        for (&a, &bits) in &want {
            let was = word(&ee, a);
            assert_eq!(
                was,
                bits,
                "file {k}: the word at {a:#x} is {was:#010x} ({}) in the original and {bits:#010x} ({}) in the port; box {b:?}\n{}",
                f32::from_bits(was),
                f32::from_bits(bits),
                text(&f)
            );
        }
    }
    for want in ["mass", "no mass", "collision mass", "no collision mass", "record inertia not over 0",
                 "body inertia lane not over 0", "BodySpringStiffness", "BodySpringDamping", "BodyMaxPenetration",
                 "BodyRestitution", "BodyFriction", "body spring moment scale -1", "body friction moment scale -1",
                 "spring damping 0", "facing behind", "ON prefix", "strtol base 0", "collision enabled",
                 "collision disabled"] {
        let n = tally.get(want).copied().unwrap_or(0);
        assert!(n >= 50, "only {n} file(s) had {want:?}: {tally:?}");
    }
}
