//! S10 — the bike in the air, proved against the original.
//!
//! Seven proofs. Two for the tables the air code reads out of RAM: the 30001-entry arc cosine table,
//! built here against the one the game's own startup code builds through its soft-float `acos`, and the
//! one entry of the startup sine table that the airborne control step has folded into a constant. Then
//! one for each of the five ported steps, each over several thousand generated states.
//!
//! The five step proofs all work the same way, and the way is worth explaining once. The original's air
//! code is five functions over one big bike object, so the harness lays that object out in the
//! interpreter's RAM — the bike, its chassis, its two wheels and its collision record — writes a
//! generated state into it, and runs the original. Then it writes the *same* generated state again,
//! applies the port to it, writes the port's answer over the top, and compares the whole region byte for
//! byte. Comparing the region rather than a list of fields is deliberate: a field the original writes
//! that the port does not model shows up as a difference, which is exactly the bug a hand-written list
//! of comparisons hides.

use game_sim::air::{self, AcosTable, Air, Axis, Chassis, Loads, Params, Rotations, STEEP_SINE, Step, Wreck};
use ps2_float::{F, Vf, fpu};
use ps2_oracle::{Ee, addr, game_or_skip};

/// Where the harness puts things in RAM: past the executable and anything it would allocate, and far
/// below the stack the interpreter starts calls with.
const TABLE: u32 = 0x0100_0000;

/// Hand the game's allocator one fixed buffer. An oracle run starts with zeroed globals, so the heap
/// has nothing in it and the real allocator would hand back null — which the table builders check.
fn alloc_at(ee: &mut Ee, at: u32) {
    ee.hooks.insert(addr("mem_alloc").unwrap(), Box::new(move |ee| ee.set(2, at as u64)));
}

/// The arc cosine table, all 30001 entries, against the one the game builds at startup.
///
/// This is the proof that [`AcosTable`] may build the table itself instead of being handed one: the
/// game fills it by calling its own double-precision `acos` 30000 times, and the port does the same
/// arithmetic in `f64`, with the coefficients read out of the executable.
#[test]
fn the_acos_table_is_the_games_own() {
    let Some(mut ee) = game_or_skip() else { return };
    alloc_at(&mut ee, TABLE);
    ee.call(addr("acos_table_build").unwrap(), &[], &[], 2_000_000_000);
    println!("the game's builder took {} instructions", ee.steps);
    assert_eq!(ee.v0(), 1, "the game's table builder reported failure");
    assert_eq!(ee.u32_at(addr("acos_table_ptr").unwrap()), TABLE, "the table went somewhere else");
    assert_eq!(ee.f32_at(addr("acos_table_scale").unwrap()), 14999.5, "index scale");

    let ours = AcosTable::build();
    assert_eq!(ours.entries().len(), 30001, "the table is 30000 entries plus the duplicated last one");
    let mut wrong = 0;
    for (i, e) in ours.entries().iter().enumerate() {
        let theirs = ee.u32_at(TABLE + 4 * i as u32);
        if theirs != e.to_bits() {
            if wrong < 6 {
                println!("entry {i}: theirs {theirs:#010x}, ours {:#010x}", e.to_bits());
            }
            wrong += 1;
        }
    }
    assert_eq!(wrong, 0, "{wrong} of 30001 arc cosine entries differ");
}

/// The one sine the airborne control step needs. The original reads it out of the startup sine table
/// every substep, by an index that is made of nothing but startup constants; the port holds the answer
/// as [`STEEP_SINE`], and this is what says the answer is right.
#[test]
fn steep_sine_is_the_curve_tables_entry() {
    let Some(mut ee) = game_or_skip() else { return };
    alloc_at(&mut ee, TABLE);
    ee.call(addr("curve_table_build").unwrap(), &[], &[], 2_000_000_000);
    let ptr = ee.u32_at(addr("curve_table_ptr").unwrap());
    let invstep = ee.f32_at(addr("curve_table_invstep").unwrap());
    // The original's own index: the angle wrapped into -pi..pi (which leaves 70 degrees alone), plus
    // pi because the table starts there, times the index scale, truncated.
    let i = F(fpu::mul(fpu::add(3.1415927, air::STEEP), invstep)).to_i32();
    let sine = ee.u32_at(ptr + 8 * i as u32 + 4);
    assert_eq!(sine, STEEP_SINE.to_bits(), "entry {i} of the sine table is not STEEP_SINE");
}

// --- the harness the five step proofs share ---------------------------------------------------------

/// The bike object and the four things it points at, laid out in RAM past anything the executable uses.
/// The sizes are the highest offset each one is read at, rounded up — the whole of each is compared, so
/// a field the original writes and the port does not know about shows up as a difference.
const OBJ: u32 = 0x0120_0000;
const CHASSIS: u32 = 0x0120_1000;
const FRONT: u32 = 0x0120_2000;
const REAR: u32 = 0x0120_3000;
const COLL: u32 = 0x0120_4000;
const VTABLE: u32 = 0x0120_5000;
/// Where the vtable's "the rider came off" slot points. Nothing is there: a hook catches the call and
/// writes down the wreck code, which is the only way out of the original that is not a memory write.
const CAME_OFF: u32 = 0x0120_6000;
const REGIONS: [(&str, u32, u32); 5] = [
    ("bike", OBJ, 0xa90),
    ("chassis", CHASSIS, 0x2a0),
    ("front wheel", FRONT, 0x1a0),
    ("rear wheel", REAR, 0x1a0),
    ("collision", COLL, 0x110),
];

/// One generated substep: the bike's tuning, the air state going in, the chassis, and the substep's
/// own inputs.
#[derive(Clone, Debug)]
struct State {
    p: Params,
    air: Air,
    ch: Chassis,
    st: Step,
    lo: Loads,
}

fn put_f(ee: &mut Ee, a: u32, v: F) {
    ee.set_f32(a, v.0);
}
fn put_b(ee: &mut Ee, a: u32, v: bool) {
    ee.set_u32(a, v as u32);
}
fn put_v(ee: &mut Ee, a: u32, v: Vf) {
    for (k, x) in [v.x(), v.y(), v.z(), v.w()].into_iter().enumerate() {
        ee.set_f32(a + 4 * k as u32, x);
    }
}

/// Zero every byte of every region, so that a byte the original writes and the port does not cannot
/// hide behind a value that happened to be left over from the run before.
fn reset(ee: &mut Ee) {
    for (_, base, len) in REGIONS {
        for o in (0..len).step_by(4) {
            ee.set_u32(base + o, 0);
        }
    }
}

/// Write a state into the object graph: the pointers between the pieces, the two guards that cut the
/// parts of the wreck test this task does not port, and then every field the five steps read or write.
fn poke(ee: &mut Ee, s: &State) {
    ee.set_u32(OBJ + 0x18, 0); // the transform; `orientation_axes` is hooked, so it is never read
    ee.set_u32(OBJ + 0x48, VTABLE);
    ee.set_u32(VTABLE + 0xfc, CAME_OFF);
    ee.set_u32(OBJ + 0x7a4, CHASSIS);
    ee.set_u32(OBJ + 0x7b0, FRONT);
    ee.set_u32(OBJ + 0x7b4, REAR);
    ee.set_u32(OBJ + 0x7d0, COLL);
    // The wreck test's two other halves, cut: zero here skips the out-of-bounds and hitzone prologue,
    // and an empty bike list skips the loop that tests this bike against the others.
    ee.set_u32(OBJ + 0x570, 0);
    ee.set_u32(OBJ + 0x748, 0);
    ee.set_u32(OBJ + 0x7d8, 0);
    ee.set_u32(COLL + 0x100, 0);
    ee.set_u32(COLL + 0x108, 0);

    let (p, air, ch, st, lo) = (&s.p, &s.air, &s.ch, &s.st, &s.lo);
    put_v(ee, OBJ + 0x4d0, p.airborne_control_moment);
    put_f(ee, OBJ + 0x4e0, p.airborne_damping);
    put_f(ee, OBJ + 0x4e4, p.airborne_control_force);
    put_f(ee, OBJ + 0x6ac, p.brake_gyro_torque);
    put_f(ee, OBJ + 0x6b0, p.unroll_gain);
    put_f(ee, OBJ + 0x6b4, p.unroll_torque);
    put_f(ee, OBJ + 0x6b8, p.unyaw_gain);
    put_f(ee, OBJ + 0x6bc, p.unyaw_torque);
    put_f(ee, OBJ + 0x4ec, p.control_authority_time);
    put_f(ee, OBJ + 0x4ac, p.preload_time);
    put_f(ee, OBJ + 0x4c4, p.max_preload);
    put_f(ee, OBJ + 0x4b4, p.min_preload_time);
    put_f(ee, OBJ + 0x4b8, p.max_preload_time);
    put_f(ee, OBJ + 0x4bc, p.min_dpad_preload_time);
    put_f(ee, OBJ + 0x4c0, p.max_dpad_preload_time);
    put_f(ee, OBJ + 0x504, p.min_backflip_time);
    put_f(ee, OBJ + 0x508, p.max_backflip_time);
    put_f(ee, OBJ + 0x510, p.min_backflip_torque);
    put_f(ee, OBJ + 0x514, p.max_backflip_torque);
    put_f(ee, OBJ + 0x518, p.backflip_torque_rate_increase);
    put_f(ee, OBJ + 0x51c, p.backflip_torque_rate_decrease);
    put_f(ee, OBJ + 0x754, p.front_wreck_speed_limit);
    put_f(ee, OBJ + 0x758, p.back_wreck_speed_limit);
    put_f(ee, OBJ + 0x75c, p.side_wreck_speed_limit);
    put_f(ee, OBJ + 0x760, p.racing_down_wreck_speed_limit);
    put_f(ee, OBJ + 0x764, p.freestyle_down_wreck_speed_limit);
    put_f(ee, OBJ + 0x768, p.ground_pitch_down_wreck_angle);
    put_f(ee, OBJ + 0x76c, p.ground_pitch_up_wreck_angle);
    put_f(ee, OBJ + 0x770, p.airborne_pitch_down_wreck_angle);
    put_f(ee, OBJ + 0x774, p.airborne_pitch_up_wreck_angle);
    put_f(ee, OBJ + 0x778, p.airborne_roll_wreck_angle);
    put_f(ee, OBJ + 0x77c, p.no_hander_wreck_speed_limit);
    put_f(ee, OBJ + 0x780, p.no_hander_max_roll_angle);
    put_f(ee, OBJ + 0x784, p.joust_wreck_height);
    put_f(ee, OBJ + 0x78c, p.crossup_wreck);
    put_f(ee, OBJ + 0x790, p.crossup_wreck_time);

    put_b(ee, OBJ + 0x574, air.on_ground);
    put_b(ee, OBJ + 0x578, air.both_wheels);
    put_b(ee, OBJ + 0x57c, air.airborne);
    put_b(ee, OBJ + 0x580, air.was_airborne);
    put_f(ee, OBJ + 0x4f0, air.hang_time);
    put_v(ee, OBJ + 0x430, air.contact_vel);
    put_f(ee, OBJ + 0x600, air.crossup);
    put_f(ee, OBJ + 0x4e8, air.authority);
    put_b(ee, OBJ + 0x4a0, air.preload_ready);
    put_b(ee, OBJ + 0x4a4, air.preload_firing);
    put_b(ee, OBJ + 0x4a8, air.preload_armed);
    put_f(ee, OBJ + 0x4b0, air.preload_timer);
    put_v(ee, OBJ + 0x490, air.preload_force);
    put_b(ee, OBJ + 0x4f4, air.flip_torque_applied);
    put_b(ee, OBJ + 0x4f8, air.flipping);
    put_b(ee, OBJ + 0x4fc, air.flip_active);
    put_b(ee, OBJ + 0x500, air.flip_armed);
    put_f(ee, OBJ + 0x50c, air.flip_grace);
    put_v(ee, OBJ + 0x520, air.flip_torque);
    put_v(ee, OBJ + 0x560, air.wreck_impact);

    put_f(ee, CHASSIS + 0x34, ch.speed);
    put_f(ee, CHASSIS + 0x90, ch.pitch);
    put_f(ee, CHASSIS + 0x98, ch.roll);
    put_v(ee, CHASSIS + 0x130, ch.force);
    put_v(ee, CHASSIS + 0x140, ch.moment);
    put_v(ee, CHASSIS + 0x260, ch.vel);
    put_v(ee, CHASSIS + 0x270, ch.local_vel);
    put_v(ee, CHASSIS + 0x290, ch.ang_vel);

    put_f(ee, OBJ + 0x630, st.stick_hold);
    put_v(ee, OBJ + 0x410, st.heading);
    put_v(ee, OBJ + 0x420, st.up);
    put_b(ee, FRONT + 0xdc, st.front_contact);
    put_b(ee, REAR + 0xdc, st.rear_contact);
    put_v(ee, FRONT + 0x190, st.front_vel);
    put_v(ee, REAR + 0x190, st.rear_vel);
    put_f(ee, OBJ + 0x628, st.stick.0);
    put_f(ee, OBJ + 0x62c, st.stick.1);
    put_b(ee, OBJ + 0x624, st.dpad_y);
    ee.set_u32(OBJ + 0x9fc, st.dpad_dir as u32);
    put_f(ee, OBJ + 0x640, st.brake);
    put_f(ee, OBJ + 0x650, st.front_brake);
    put_f(ee, OBJ + 0x654, st.rear_brake);
    put_f(ee, OBJ + 0x670, st.lean);
    put_b(ee, OBJ + 0xa0c, st.stunt);
    put_b(ee, COLL + 0x40, st.touching);

    put_f(ee, OBJ + 0x550, lo.side);
    put_f(ee, OBJ + 0x554, lo.vertical);
    put_f(ee, OBJ + 0x558, lo.fore_aft);
    put_f(ee, OBJ + 0x788, lo.landing_crossup);
    put_b(ee, OBJ + 0xa44, lo.no_hander);
    put_b(ee, OBJ + 0x810, lo.racing);
    ee.set_u32(OBJ + 0xa78, lo.pose as u32);
    ee.set_u32(OBJ + 0xa84, lo.pose_wanted as u32);
    put_f(ee, OBJ + 0xa2c, lo.pose_blend);
    put_b(ee, OBJ + 0xa10, lo.trick_wrecked);
}

/// Every word of every region, in order.
fn snap(ee: &Ee) -> Vec<u32> {
    let mut out = Vec::new();
    for (_, base, len) in REGIONS {
        for o in (0..len).step_by(4) {
            out.push(ee.u32_at(base + o));
        }
    }
    out
}

/// Which field a differing word is, by name of its region and offset in it.
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

/// Nothing the original writes may land outside the object graph: a write to a global would be state
/// the port cannot have, and this is the test that would say so.
fn only_touched_the_regions(ee: &Ee) -> Result<(), String> {
    let frame = (ee.stack - 0x2000, ee.stack + 0x40);
    for (a, b) in ee.written_ranges() {
        let inside = REGIONS.iter().any(|&(_, base, len)| a >= base && b <= base + len)
            || (a >= frame.0 && b <= frame.1);
        if !inside {
            return Err(format!("the original wrote {a:#010x}..{b:#010x}, which is not the bike or the stack"));
        }
    }
    Ok(())
}

/// A machine with the acos table in place and the two hooks the harness needs.
///
/// The table written in is the port's own, which `the_acos_table_is_the_games_own` has already proved
/// is the game's to the bit; doing it that way keeps every other proof off the 268 million instructions
/// the game's builder takes.
fn harness(table: &AcosTable) -> Option<(Ee, std::rc::Rc<std::cell::RefCell<Vec<u32>>>)> {
    let mut ee = game_or_skip()?;
    // The curve table is the game's own, built here: the original looks the steep-lean angle's sine up
    // in it every substep, and the port's folded `STEEP_SINE` has to agree with it every time.
    alloc_at(&mut ee, 0x0110_0000);
    ee.call(addr("curve_table_build").unwrap(), &[], &[], 2_000_000_000);
    ee.hooks.remove(&addr("mem_alloc").unwrap());
    let at = 0x0100_0000u32;
    for (i, e) in table.entries().iter().enumerate() {
        ee.set_f32(at + 4 * i as u32, *e);
    }
    ee.set_u32(addr("acos_table_ptr").unwrap(), at);
    ee.set_f32(addr("acos_table_scale").unwrap(), 14999.5);
    ee.set_u32(addr("wreck_enabled_flag").unwrap(), 1);
    // The orientation step belongs to the chassis: the harness writes the two axes itself.
    ee.hooks.insert(addr("orientation_axes").unwrap(), Box::new(|_| {}));
    let codes = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = codes.clone();
    ee.hooks.insert(CAME_OFF, Box::new(move |ee| seen.borrow_mut().push(ee.r(5) as u32)));
    Some((ee, codes))
}

/// A generator. Plain host arithmetic: these are only inputs, and every one of them goes into the
/// original and into the port alike.
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
    /// Often exactly zero: a zero brake, a zero authority and a zero tuning value are each a branch.
    fn maybe_zero(&mut self, lo: f32, hi: f32) -> F {
        if self.one_in(3) { F::ZERO } else { self.f(lo, hi) }
    }
    /// A tuning range that is sometimes empty, which is a branch in both the preload and the backflip.
    fn range(&mut self, lo: f32, hi: f32) -> (F, F) {
        let a = self.f(lo, hi);
        if self.one_in(8) { (a, a) } else { (a, self.f(lo, hi)) }
    }
    /// A stick axis, landing on the held threshold itself a third of the time.
    fn axis(&mut self) -> F {
        if self.one_in(3) {
            F(if self.b() { 0.8 } else { -0.8 })
        } else {
            self.f(-1.2, 1.2)
        }
    }
    fn vec(&mut self, lo: f32, hi: f32) -> Vf {
        Vf::new(self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0)
    }
    /// An axis of the bike's orientation: a direction, but occasionally the zero vector, so that what
    /// the normalise idiom does with a length of nothing is covered too.
    fn dir(&mut self) -> Vf {
        if self.one_in(32) {
            Vf::new(0.0, 0.0, 0.0, 1.0)
        } else {
            Vf::new(self.f(-1.0, 1.0).0, self.f(-1.1, 1.1).0, self.f(-1.0, 1.0).0, 1.0)
        }
    }

    fn state(&mut self) -> State {
        // Half the states are on a lip: a wheel down, the chassis rising, speed at the contact patch
        // and no brake. It is the gate on the preload and the backflip both, and four conditions deep,
        // so unbiased states almost never pass it. The values chosen still straddle the two
        // thresholds, so the near misses are generated too.
        let lip = self.one_in(3);
        let (min_preload_time, max_preload_time) = self.range(0.0, 1.5);
        let (min_dpad_preload_time, max_dpad_preload_time) = self.range(0.0, 1.5);
        let (min_backflip_time, max_backflip_time) = self.range(0.0, 1.5);
        let p = Params {
            airborne_control_moment: self.vec(-4000.0, 4000.0),
            airborne_damping: self.f(0.0, 400.0),
            airborne_control_force: self.f(0.0, 2000.0),
            brake_gyro_torque: self.f(-600.0, 600.0),
            unroll_gain: self.f(0.0, 12.0),
            unroll_torque: self.f(0.0, 600.0),
            unyaw_gain: self.f(0.0, 12.0),
            unyaw_torque: self.f(0.0, 600.0),
            control_authority_time: self.maybe_zero(0.05, 2.0),
            preload_time: self.f(0.0, 0.5),
            max_preload: self.f(-200.0, 4000.0),
            min_preload_time,
            max_preload_time,
            min_dpad_preload_time,
            max_dpad_preload_time,
            min_backflip_time,
            max_backflip_time,
            min_backflip_torque: self.f(0.0, 500.0),
            max_backflip_torque: self.f(0.0, 2000.0),
            backflip_torque_rate_increase: self.f(0.0, 3000.0),
            backflip_torque_rate_decrease: self.f(0.0, 3000.0),
            // The wreck limits sit inside the range the loads and angles are drawn from, so that every
            // one of them is both passed and failed many times over.
            front_wreck_speed_limit: self.f(0.0, 60.0),
            back_wreck_speed_limit: self.f(0.0, 60.0),
            side_wreck_speed_limit: self.f(0.0, 60.0),
            racing_down_wreck_speed_limit: self.f(0.0, 60.0),
            freestyle_down_wreck_speed_limit: self.f(0.0, 60.0),
            ground_pitch_down_wreck_angle: self.f(0.0, 1.5),
            ground_pitch_up_wreck_angle: self.f(0.0, 1.5),
            airborne_pitch_down_wreck_angle: self.f(0.0, 1.5),
            airborne_pitch_up_wreck_angle: self.f(0.0, 1.5),
            airborne_roll_wreck_angle: self.f(0.0, 1.5),
            no_hander_wreck_speed_limit: self.f(0.0, 60.0),
            no_hander_max_roll_angle: self.f(-1.0, 1.0),
            joust_wreck_height: self.f(0.0, 5.0),
            crossup_wreck: self.f(0.5, 1.0),
            crossup_wreck_time: self.maybe_zero(0.0, 1.5),
            rotation_speed: None,
        };
        let on_ground = lip || self.one_in(4);
        let contact_vel = {
            let v = self.vec(-40.0, 40.0);
            let z = if lip { self.f(29.0, 45.0) } else { F(v.z()) };
            Vf::new(v.x(), v.y(), z.0, v.w())
        };
        let air = Air {
            on_ground,
            both_wheels: self.b(),
            airborne: !on_ground || self.one_in(4),
            was_airborne: self.b(),
            hang_time: self.f(0.0, 3.0),
            contact_vel,
            crossup: self.f(-1.6, 1.6),
            authority: self.maybe_zero(-0.5, 2.0),
            preload_ready: self.b(),
            preload_firing: self.one_in(3),
            preload_armed: self.b(),
            preload_timer: self.f(-0.1, 1.5),
            preload_force: self.vec(-100.0, 3000.0),
            flip_torque_applied: self.b(),
            flipping: self.b(),
            flip_active: self.one_in(3),
            flip_armed: self.b(),
            flip_grace: self.f(-0.1, 0.7),
            flip_torque: self.vec(-2000.0, 2000.0),
            wreck_impact: self.vec(-40.0, 40.0),
        };
        let ch = Chassis {
            speed: self.f(0.0, 30.0),
            pitch: self.f(-2.0, 2.0),
            roll: self.f(-2.0, 2.0),
            force: self.vec(-2000.0, 2000.0),
            moment: self.vec(-2000.0, 2000.0),
            vel: {
                let y = if lip { self.f(4.5, 12.0) } else { self.f(-10.0, 10.0) };
                Vf::new(self.f(-30.0, 30.0).0, y.0, self.f(-30.0, 30.0).0, self.f(-1.0, 1.0).0)
            },
            local_vel: self.vec(-40.0, 40.0),
            ang_vel: self.vec(-4.0, 4.0),
        };
        let st = Step {
            dt: if self.one_in(8) { self.maybe_zero(0.0, 0.5) } else { F(1.0 / 60.0) },
            heading: self.dir(),
            up: self.dir(),
            front_contact: self.b(),
            rear_contact: self.b(),
            front_vel: self.vec(-40.0, 40.0),
            rear_vel: self.vec(-40.0, 40.0),
            stick: (self.axis(), self.axis()),
            stick_hold: self.f(-0.2, 2.0),
            dpad_y: self.b(),
            dpad_dir: if self.b() { 0 } else { 1 + (self.bits() % 4) as i32 },
            brake: if lip { F::ZERO } else { self.maybe_zero(0.0, 1.0) },
            front_brake: self.maybe_zero(0.0, 1.0),
            rear_brake: self.maybe_zero(0.0, 1.0),
            lean: if self.one_in(2) { self.f(-0.12, 0.12) } else { self.f(-0.6, 0.6) },
            stunt: self.b(),
            touching: self.b(),
            modifier: false,
            spinning: false,
            spin_active: false,
            spin_ramp: false,
        };
        let lo = Loads {
            side: self.f(-60.0, 60.0),
            vertical: self.f(-60.0, 60.0),
            fore_aft: self.f(-60.0, 60.0),
            landing_crossup: self.f(-0.2, 1.2),
            no_hander: self.b(),
            racing: self.b(),
            pose: (self.bits() % 3) as i32,
            pose_wanted: (self.bits() % 3) as i32,
            pose_blend: self.f(0.85, 1.0),
            trick_wrecked: self.b(),
        };
        State { p, air, ch, st, lo }
    }
}

/// One step proved: for each of `n` generated states, run the original on it and the port on a copy,
/// then compare the whole object graph word for word.
///
/// The port's answer goes back into the object graph the same way the state did, so the comparison is
/// not of the fields the port happens to know about but of every byte of the bike: a field the
/// original writes and the port leaves alone is a difference, and so is one the port writes and the
/// original does not.
fn prove(
    name: &str,
    n: u32,
    seed: u64,
    at: u32,
    mut apply: impl FnMut(&mut State, &[u32]) -> Result<(), String>,
) {
    let Some((mut ee, codes)) = harness(&AcosTable::build()) else { return };
    let mut g = Gen(seed);
    for k in 0..n {
        let s = g.state();
        reset(&mut ee);
        poke(&mut ee, &s);
        codes.borrow_mut().clear();
        // The substep's `dt` goes in `$f12` and the bike in `$a0`; the wreck test takes a second bike
        // in `$a1`, and a null one there is how the harness asks for the single-bike half of it.
        ee.call(at, &[OBJ as u64, 0], &[s.st.dt.0], 20_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("{name}, state {k}: {e}\n{s:#?}"));
        let was = snap(&ee);

        let mut t = s.clone();
        let wrecked = codes.borrow().clone();
        apply(&mut t, &wrecked).unwrap_or_else(|e| panic!("{name}, state {k}: {e}\n{s:#?}"));
        reset(&mut ee);
        poke(&mut ee, &t);
        let now = snap(&ee);
        if was != now {
            let i = was.iter().zip(&now).position(|(a, b)| a != b).unwrap();
            let (a, b) = (was[i], now[i]);
            panic!(
                "{name}, state {k}: {} is {a:#010x} ({}) in the original and {b:#010x} ({}) in the port\n\
                 and {} word(s) differ in all\n{s:#?}",
                where_is(i),
                f32::from_bits(a),
                f32::from_bits(b),
                was.iter().zip(&now).filter(|(a, b)| a != b).count(),
            );
        }
    }
}

/// Is the bike on a lip? The three thresholds again, written out here so that the tallies below are a
/// statement about the generated states and not a second copy of the port's answer.
fn lip(s: &State) -> bool {
    s.air.on_ground && s.ch.vel.y() > 5.0 && s.air.contact_vel.z() > 30.0 && s.st.brake == F::ZERO
}

/// A proof is only worth the branches its states reach. Each step proof tallies the path every state
/// took, and this is the assertion that none of those paths was reached only a handful of times.
fn covered(name: &str, tally: &std::collections::BTreeMap<&str, u32>, paths: &[&str], least: u32) {
    if tally.is_empty() {
        return; // no game, nothing proved
    }
    for path in paths {
        let n = tally.get(path).copied().unwrap_or(0);
        assert!(n >= least, "{name}: only {n} state(s) went down the {path:?} path; the tally is {tally:?}");
    }
}

#[test]
fn the_contact_step_is_the_originals() {
    let table = AcosTable::build();
    fn path(s: &State) -> &'static str {
        if s.st.front_contact && s.st.rear_contact {
            "both wheels"
        } else if s.st.front_contact || s.st.rear_contact {
            "one wheel"
        } else if s.air.flipping {
            "airborne, mid flip"
        } else if s.ch.speed <= F(10.0) {
            "airborne, slow"
        } else if s.air.authority > F::ZERO {
            "airborne, control coming back"
        } else {
            "airborne"
        }
    }
    let mut tally = std::collections::BTreeMap::new();
    prove(
        "contact",
        8000,
        0x0c01,
        addr("bike_air_state").unwrap(),
        |t, _| {
            *tally.entry(path(t)).or_insert(0u32) += 1;
            air::contact(&t.p, &mut t.air, &t.ch, &t.st, &table);
            Ok(())
        },
    );
    covered(
        "contact",
        &tally,
        &[
            "both wheels",
            "one wheel",
            "airborne, mid flip",
            "airborne, slow",
            "airborne, control coming back",
            "airborne",
        ],
        50,
    );
}

#[test]
fn the_preload_step_is_the_originals() {
    fn path(s: &State) -> &'static str {
        if s.air.preload_firing {
            if s.air.preload_timer > s.st.dt { "shoving" } else { "spent" }
        } else if !lip(s) {
            "off the lip"
        } else if s.st.stick.1 > F(0.8) {
            "arming"
        } else if s.st.stick.1 < F(-0.8) && s.air.preload_armed {
            "firing"
        } else if s.air.preload_armed {
            "held"
        } else {
            "waiting"
        }
    }
    let mut tally = std::collections::BTreeMap::new();
    prove(
        "preload",
        8000,
        0x0c02,
        addr("bike_preload").unwrap(),
        |t, _| {
            *tally.entry(path(t)).or_insert(0u32) += 1;
            air::preload(&t.p, &mut t.air, &mut t.ch, &t.st);
            Ok(())
        },
    );
    covered(
        "preload",
        &tally,
        &["shoving", "spent", "off the lip", "arming", "firing", "held", "waiting"],
        50,
    );
}

#[test]
fn the_backflip_step_is_the_originals() {
    fn path(s: &State) -> &'static str {
        if !s.air.flip_active {
            if !lip(s) {
                "off the lip"
            } else if s.st.stick.1 < F(-0.8) {
                "arming"
            } else if s.st.stick.1 > F(0.8) && s.air.flip_armed {
                "triggered"
            } else {
                "waiting"
            }
        } else if !s.air.flip_armed {
            if s.air.on_ground { "in the grace" } else { "off the lip in time" }
        } else if s.air.on_ground || s.st.touching || s.air.flip_torque.x().abs() <= s.p.min_backflip_torque.0 {
            "done turning"
        } else {
            "turning"
        }
    }
    let mut tally = std::collections::BTreeMap::new();
    prove(
        "backflip",
        8000,
        0x0c03,
        addr("bike_backflip").unwrap(),
        |t, _| {
            *tally.entry(path(t)).or_insert(0u32) += 1;
            air::backflip(&t.p, &mut t.air, &mut t.ch, &t.st);
            Ok(())
        },
    );
    covered(
        "backflip",
        &tally,
        &[
            "off the lip",
            "arming",
            "triggered",
            "waiting",
            "in the grace",
            "off the lip in time",
            "done turning",
            "turning",
        ],
        50,
    );
}

#[test]
fn the_airborne_control_step_is_the_originals() {
    fn path(s: &State) -> &'static str {
        if s.air.on_ground {
            "on the ground"
        } else if F(s.st.heading.y()).abs() >= F(STEEP_SINE) {
            "steeply leant"
        } else if s.st.lean.abs() >= F(0.1) {
            "leaning"
        } else if s.air.flipping || s.air.authority > F::ZERO {
            "no control yet"
        } else if s.st.stunt {
            "unyawed, in a stunt"
        } else {
            "unyawed"
        }
    }
    let mut tally = std::collections::BTreeMap::new();
    prove(
        "airborne control",
        8000,
        0x0c04,
        addr("bike_airborne_control").unwrap(),
        |t, _| {
            *tally.entry(path(t)).or_insert(0u32) += 1;
            air::airborne_control(&t.p, &mut t.air, &mut t.ch, &t.st);
            Ok(())
        },
    );
    covered(
        "airborne control",
        &tally,
        &[
            "on the ground",
            "steeply leant",
            "leaning",
            "no control yet",
            "unyawed, in a stunt",
            "unyawed",
        ],
        50,
    );
}

#[test]
fn the_wreck_test_is_the_originals() {
    // The one step whose answer is not a memory write: the original calls the bike's "the rider came
    // off" slot with the wreck's code, and the harness' hook writes down every call it makes.
    // Every wreck is counted, because a proof is only worth the branches the generated states reach:
    // the assertion below is what says they reach all of them.
    let mut tally = std::collections::BTreeMap::new();
    prove("wreck test", 8000, 0x0c05, addr("bike_wreck_test").unwrap(), |t, wrecked| {
        *tally.entry(wrecked.first().copied()).or_insert(0u32) += 1;
        let got = air::wreck_test(&t.p, &mut t.air, &t.ch, &t.st, &t.lo);
        let want: Vec<u32> = got.map(|w| w as u32).into_iter().collect();
        if wrecked != want.as_slice() {
            return Err(format!("the original wrecked with {wrecked:?} and the port with {want:?} ({got:?})"));
        }
        Ok(())
    });
    if tally.is_empty() {
        return; // no game, nothing proved
    }
    for want in [
        None,
        Some(Wreck::Front),
        Some(Wreck::Left),
        Some(Wreck::Right),
        Some(Wreck::Back),
        Some(Wreck::Flat),
        Some(Wreck::Joust),
        Some(Wreck::NoseDive),
        Some(Wreck::Tail),
        Some(Wreck::NoHander),
    ] {
        let code = want.map(|w| w as u32);
        let n = tally.get(&code).copied().unwrap_or(0);
        assert!(n >= 25, "only {n} of the states came out as {want:?}; the rest of the tally is {tally:?}");
    }
}

// --- S37: the hits -----------------------------------------------------------------------------------

/// Where the hits proof puts the contact list, and the actors its entries point at: one 0x100-byte
/// record per entry, then that actor's chassis or rigid body.
const LIST: u32 = 0x0120_7000;
const OTHERS: u32 = 0x0120_8000;
const BODIES: u32 = 0x0120_a000;

/// The part of a generated substep the wreck test's last clause reads, besides the record's "touched"
/// word, which is [`Step::touching`] and already in the state.
#[derive(Clone, Debug)]
struct Hits {
    fifty: bool,
    mass: F,
    height: F,
    list: Vec<air::Contact>,
}

#[allow(clippy::float_arithmetic)]
impl Gen {
    fn hits(&mut self) -> Hits {
        let mass = self.f(5.0, 20.0);
        let height = self.f(-50.0, 300.0);
        let n = (self.bits() % 12) as usize;
        let mut list = Vec::new();
        for _ in 0..n {
            // The reaches the bike's eleven rays end at, the two limits themselves, and anything.
            let reach = match self.bits() % 7 {
                0 => F(6.6743364),
                1 => F(4.580028),
                2 => F(3.0401340),
                3 => F(6.0),
                4 => F(4.0),
                _ => self.f(0.0, 8.0),
            };
            let other = match self.bits() % 3 {
                0 => air::Other::Scenery,
                1 => air::Other::Rider { height: F(height.0 + self.f(-6.0, 8.0).0), vel: self.vec(-40.0, 40.0) },
                _ => air::Other::Object { mass: F(mass.0 + self.f(-8.0, 8.0).0), vel: self.vec(-40.0, 40.0) },
            };
            list.push(air::Contact { mirrored: self.one_in(4), reach, other });
        }
        Hits { fifty: self.one_in(3), mass, height, list }
    }
}

/// Lay the hits into the object graph: the 50cc flag, the chassis' mass and height, and the record's
/// list, each entry pointing at an actor record of its own (or at none, for scenery).
fn lay(ee: &mut Ee, h: &Hits) {
    put_b(ee, OBJ + 0x588, h.fifty);
    put_f(ee, CHASSIS + 0x210, h.mass);
    put_f(ee, CHASSIS + 0x64, h.height);
    ee.set_u32(COLL + 0x100, h.list.len() as u32);
    ee.set_u32(COLL + 0x104, 11);
    ee.set_u32(COLL + 0x108, LIST);
    for (i, c) in h.list.iter().enumerate() {
        let (e, other, body) = (LIST + 0x80 * i as u32, OTHERS + 0x100 * i as u32, BODIES + 0x400 * i as u32);
        for o in (0..0x80).step_by(4) {
            ee.set_u32(e + o, 0);
        }
        ee.set_u32(e, c.mirrored as u32);
        put_f(ee, e + 0x24, c.reach);
        match c.other {
            air::Other::Scenery => ee.set_u32(e + 0x74, 0),
            air::Other::Rider { height, vel } => {
                ee.set_u32(e + 0x74, other);
                ee.set_u32(other + 0xf0, body);
                put_f(ee, body + 0x64, height);
                put_v(ee, body + 0x260, vel);
            }
            air::Other::Object { mass, vel } => {
                ee.set_u32(e + 0x74, other);
                ee.set_u32(other + 0xf0, 0);
                ee.set_u32(other + 0xf4, body);
                put_f(ee, body + 0xc, mass);
                put_v(ee, body + 0x70, vel);
            }
        }
    }
}

/// The wreck test's contact clause, over 10000 generated substeps with a contact list in the record:
/// the original's whole wreck test against [`air::wreck_test`] then [`air::hit_wreck`], every call
/// to the "came off" slot in order — the first test and the hits can both enter a wreck on one
/// substep — and every word of the object graph after.
#[test]
fn the_hits_are_the_originals() {
    let Some((mut ee, codes)) = harness(&AcosTable::build()) else { return };
    let mut g = Gen(0x0537);
    let mut tally = std::collections::BTreeMap::new();
    for k in 0..10_000 {
        let s = g.state();
        let h = g.hits();
        reset(&mut ee);
        poke(&mut ee, &s);
        lay(&mut ee, &h);
        codes.borrow_mut().clear();
        ee.call(addr("bike_wreck_test").unwrap(), &[OBJ as u64, 0], &[s.st.dt.0], 40_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("hits, state {k}: {e}\n{s:#?}\n{h:#?}"));
        let was = snap(&ee);
        let wrecked = codes.borrow().clone();

        let mut t = s.clone();
        let landed = air::wreck_test(&t.p, &mut t.air, &t.ch, &t.st, &t.lo);
        let hit = air::hit_wreck(&t.p, &mut t.air, t.st.touching, h.fifty, h.mass, h.height, &h.list);
        let calls: Vec<u32> = landed.into_iter().chain(hit).map(|w| w as u32).collect();
        assert_eq!(wrecked, calls, "hits, state {k}: the original's calls against the port's\n{s:#?}\n{h:#?}");
        reset(&mut ee);
        poke(&mut ee, &t);
        lay(&mut ee, &h);
        let now = snap(&ee);
        if let Some(i) = was.iter().zip(&now).position(|(a, b)| a != b) {
            panic!("hits, state {k}: {} is {:#010x} in the original and {:#010x} in the port\n{s:#?}\n{h:#?}",
                   where_is(i), was[i], now[i]);
        }
        let path = match hit {
            Some(w) => format!("{w:?}"),
            None if !t.st.touching => "untouched".into(),
            None if h.list.iter().any(|c| !c.mirrored && c.reach > F(4.0)) => "an actor let off".into(),
            None => "nothing reaching".into(),
        };
        *tally.entry(path).or_insert(0u32) += 1;
        if h.fifty && h.list.iter().find(|c| !c.mirrored && c.reach > F(4.0)).is_some_and(|c| c.reach <= F(6.0)) {
            *tally.entry("a 50cc's shoulder".into()).or_insert(0u32) += 1;
        }
        if landed.is_some() && hit.is_some() {
            *tally.entry("both".into()).or_insert(0u32) += 1;
        }
    }
    for path in ["Scenery", "Joust", "untouched", "an actor let off", "nothing reaching", "a 50cc's shoulder", "both"] {
        let n = tally.get(path).copied().unwrap_or(0);
        assert!(n >= 50, "only {n} state(s) went down the {path:?} path; the tally is {tally:?}");
    }
}

// --- P17: a rotation is a row ------------------------------------------------------------------------

/// The default set is the disc's: the backflip alone. The invented rows only come in with the flag.
#[test]
fn the_default_set_of_rotations_is_the_discs() {
    let names = |r: &Rotations| r.rows().iter().map(|r| r.name).collect::<Vec<_>>();
    assert_eq!(names(&Rotations::default()), ["backflip"]);
    assert_eq!(names(&Rotations::new(false)), ["backflip"]);
    let more = Rotations::new(true);
    assert_eq!(names(&more), ["backflip", "flat spin"]);
    assert_eq!(more.rows()[1].axis, Axis::Yaw, "the invented row turns about another axis");
}

/// With the flag off, the set's step *is* [`air::backflip`], the function the oracle proves above; with
/// it on, the lean inside the held threshold so that only the backflip's gesture can be made, and the
/// yaw lane clear as [`Air::default`] starts it, it is still the same to the bit. 8000 generated states each, the same generator as the oracle proofs, so
/// no emulator is needed.
#[test]
fn the_flag_off_leaves_the_backflip_the_discs() {
    let mut g = Gen(0x0c17);
    for k in 0..8000 {
        let mut s = g.state();
        if s.st.stick.0.abs() >= F(0.8) {
            s.st.stick.0 = F(0.0);
        }
        for flag in [false, true] {
            // With the flag on, a fire clears the flat spin's yaw lane so a spin's leftover cannot ride
            // along with the backflip. The generator fills that lane with noise the disc never puts
            // there (only the backflip writes `flip_torque`, and only `x`), so start it clear.
            let mut s = s.clone();
            if flag {
                let v = s.air.flip_torque;
                s.air.flip_torque = Vf::new(v.x(), 0.0, v.z(), v.w());
            }
            let mut want = s.clone();
            air::backflip(&want.p, &mut want.air, &mut want.ch, &want.st);
            let mut got = s.clone();
            Rotations::new(flag).step(&got.p, &mut got.air, &mut got.ch, &got.st);
            assert_eq!(
                format!("{:?}", (got.air, got.ch)),
                format!("{:?}", (want.air, want.ch)),
                "state {k}, flag {flag}: the set's step is not the backflip's\n{s:#?}"
            );
        }
    }
}

/// The invented flat spin: lean left on the lip, then right, and a positive yaw torque goes into the
/// chassis with pitch untouched. The same stick with the flag off does nothing at all.
#[test]
fn a_flat_spin_is_a_row_the_flag_adds() {
    let mut s = Gen(0x0c18).state();
    s.p.min_backflip_time = F(0.0);
    s.p.max_backflip_time = F(1.0);
    s.p.min_backflip_torque = F(100.0);
    s.p.max_backflip_torque = F(1000.0);
    s.p.backflip_torque_rate_increase = F(200.0);
    s.p.backflip_torque_rate_decrease = F(400.0);
    s.air = Air {
        on_ground: true,
        contact_vel: Vf::new(0.0, 0.0, 40.0, 1.0),
        flip_torque: Vf::new(-777.0, 0.0, 0.0, 1.0),
        ..Air::default()
    };
    s.ch.vel = Vf::new(0.0, 8.0, 20.0, 1.0);
    s.ch.moment = Vf::new(1.0, 2.0, 3.0, 4.0);
    s.st = Step { dt: F(1.0 / 60.0), stick_hold: F(0.5), ..Step::default() };

    let run = |flag: bool| {
        let mut set = Rotations::new(flag);
        let mut t = s.clone();
        let mut go = |t: &mut State, stick: (f32, f32), on_ground: bool| {
            t.st.stick = (F(stick.0), F(stick.1));
            t.air.on_ground = on_ground;
            set.step(&t.p, &mut t.air, &mut t.ch, &t.st);
        };
        go(&mut t, (-1.0, 0.0), true); // arm
        let armed = t.air.flip_armed;
        go(&mut t, (1.0, 0.0), true); // fire
        let fired = (t.air.flip_active, t.air.flip_torque);
        go(&mut t, (0.0, 0.0), false); // off the lip
        go(&mut t, (0.0, 0.0), false); // turning
        (armed, fired, t, set.active().name)
    };

    let (armed, (active, torque), t, name) = run(true);
    assert!(armed, "leaning left on the lip arms the flat spin");
    assert!(active, "leaning right fires it");
    assert_eq!(name, "flat spin");
    // Fired with 0.5 s held: 1.25 x 100 to 1000, half way, positive, in the yaw lane — and the
    // backflip's leftover pitch torque cleared rather than carried along.
    assert_eq!((torque.x(), torque.y(), torque.z()), (0.0, 562.5, 0.0));
    assert!(t.air.flip_torque_applied && t.air.flipping);
    assert_eq!(t.ch.moment.x(), 1.0, "a flat spin does not pitch the bike");
    assert_eq!(t.ch.moment.y(), 2.0 + 562.5, "the yaw torque went into the chassis");
    assert_eq!(t.ch.moment.z(), 3.0);

    let (armed, (active, torque), t, name) = run(false);
    assert!(!armed && !active, "with the flag off the lean is not a gesture");
    assert_eq!(name, "backflip");
    assert_eq!(torque.x(), -777.0, "nothing was written");
    assert!(!t.air.flip_torque_applied && !t.air.flipping);
    assert_eq!((t.ch.moment.x(), t.ch.moment.y(), t.ch.moment.z()), (1.0, 2.0, 3.0));
}

// ---------------------------------------------------------------------------------------------------
// S31 — the human's backflips, against the original's own run of them
// ---------------------------------------------------------------------------------------------------

/// One of the backflips the human's second recording asks for, as the original rode it. Every number
/// is a frame counter read off `recording2-backflips.csv` (see [`the_humans_backflips`] for the
/// recipe), kept here so the measurement is in the repository and checked back against the capture
/// whenever the capture is present.
struct Flip {
    /// The first tick the lip is open at the end of (on the ground, chassis `vel.y` past 5, contact
    /// speed past 30, no brake), and the last.
    lip: (u64, u64),
    /// The tick `flip_armed` goes up, and the tick the flip fires (`flip_active` up, torque chosen).
    arm: u64,
    fire: u64,
    /// The stick `y` the human pushed through on the way from below `-HELD` to above `+HELD`: the
    /// last row below, every row between, and the first row above — from the first of them.
    cross: (u64, &'static [f32]),
    /// The torque chosen at the fire, and the stick hold it came from.
    torque: f32,
    hold: f32,
    /// The first tick with both wheels off, the first tick the rotation is over (`flipping` down),
    /// and the tick the wheels are back — or, for a flip the rider does not come out of, the tick of
    /// the wreck.
    takeoff: u64,
    over: Option<u64>,
    down: u64,
    landed: bool,
    /// The rotation from takeoff to `down`: the chassis' body pitch rate summed over the ticks, in
    /// turns, and the fastest the body pitched, rad/s. Negative is the backflip's way.
    turns: f32,
    peak_rate: f32,
}

const HUMANS_FLIPS: [Flip; 3] = [
    Flip {
        lip: (2493, 2510),
        arm: 2500,
        fire: 2506,
        cross: (2504, &[-0.8767, 1.0]),
        torque: -1400.0,
        hold: 469.4456,
        takeoff: 2511,
        over: Some(2580),
        down: 2595,
        landed: true,
        turns: -1.043,
        peak_rate: -3.111,
    },
    Flip {
        lip: (3281, 3299),
        arm: 3284,
        fire: 3292,
        cross: (3288, &[-1.0, -0.3277, 0.8431]),
        torque: -1215.5457,
        hold: 32.3319,
        takeoff: 3300,
        over: Some(3379),
        down: 3417,
        landed: true,
        turns: -1.000,
        peak_rate: -2.761,
    },
    Flip {
        lip: (4065, 4084),
        arm: 4069,
        fire: 4077,
        cross: (4074, &[-1.0, 0.5742, 1.0]),
        torque: -1360.3353,
        hold: 54.0503,
        takeoff: 4085,
        over: None,
        down: 4124,
        landed: false,
        turns: -0.600,
        peak_rate: -3.111,
    },
];

/// The ticks the port does not reproduce from the original's end-of-tick state: none. S31 shipped four
/// here (2553 and 2572 turning, 3291 firing, 3518 arming), all put down to the stick's timing inside the
/// tick; they were the input layer's squaring instead. The air steps read the stick after the rate
/// limiter has multiplied each axis by its own magnitude, so a stick 0.05 past `HELD` (0.8) on the row
/// is short of it (0.843² is 0.71), and a stick at -0.43 turns the torque as -0.185 does — which is the
/// "two substeps at about -0.37" S31 measured. P30 found it; the list stays so that a miss shows up.
const HUMANS_MISSED: [(u64, &str); 0] = [];

/// A capture's cells by column name.
struct Cells {
    cap: game_sim::replay::Capture,
    at: std::collections::HashMap<String, usize>,
}

impl Cells {
    fn f(&self, row: usize, col: &str) -> F {
        let cell = &self.cap.rows[row][self.at[col]];
        F(cell.parse::<f64>().unwrap_or_else(|_| panic!("{col} row {row}: '{cell}'")) as f32)
    }
    fn v(&self, row: usize, col: &str) -> Vf {
        let l = |c: &str| self.f(row, &format!("{col}.{c}")).0;
        Vf::new(l("x"), l("y"), l("z"), l("w"))
    }
    fn on(&self, row: usize, col: &str) -> bool {
        self.cap.rows[row][self.at[col]] != "0"
    }
    fn fc(&self, row: usize) -> u64 {
        self.cap.rows[row][self.at["frame_counter"]].parse().unwrap()
    }
}

/// The flip state at the end of a row, in the port's terms.
fn flip_state(c: &Cells, r: usize) -> Air {
    Air {
        on_ground: c.on(r, "on_ground"),
        both_wheels: c.on(r, "both_wheels"),
        airborne: c.on(r, "airborne"),
        contact_vel: c.v(r, "s31_contact_vel"),
        authority: c.f(r, "s31_auth.x"),
        flip_torque_applied: c.on(r, "flip_torqued"),
        flipping: c.on(r, "flip_ramp"),
        flip_active: c.on(r, "flip_on"),
        flip_armed: c.on(r, "flip_arm"),
        flip_grace: c.f(r, "flip_grace"),
        flip_torque: c.v(r, "flip_torque"),
        ..Air::default()
    }
}

/// S31: the human's three backflips in `recording2`, measured on the original and replayed through
/// [`air::backflip`].
///
/// The capture is the human's second recording played back in PCSX2 with the bike object's flip
/// state, stick, contact speed and chassis (velocity, Euler angles, both angular velocities) recorded
/// every tick from frame counter 2374 to 4267 — 1894 rows, no gaps. It is game data, so it lives in
/// `context/capture/acceptance/recording2-backflips.csv` and this test skips without it; the spec and
/// the playback recipe are in the S31 journal.
///
/// Three things are checked, all against the capture:
///
/// 1. **The measurements.** Every number in [`HUMANS_FLIPS`] is read back off the capture: the lip's
///    first and last tick, the arm and the fire, the stick's crossing, the torque and the hold it came
///    from, takeoff, the end of the rotation, the landing or the wreck, the turns and the peak rate.
/// 2. **The gesture, tick by tick.** For every tick, the port is given the original's
///    flip state at the end of the tick before, that tick's inputs (on the ground, contact speed,
///    chassis velocity and angles, stick, hold, d-pad, brake, touching), and runs the original's two
///    substeps of half a tick each. Its flip state must be the original's at the end of the tick —
///    armed, fired, grace, the torque to the bit, turning, applied. It is, on every tick
///    ([`HUMANS_MISSED`] is empty since P30 squared the stick), and the test insists on it. Ticks
///    after a wreck are left out: the original stops running the step there (flip 3's `flipping` stays
///    up through the whole crash).
/// 3. **The flight.** Over each flight the port turns the bike with the original's torque, to the bit,
///    on every tick, and the rotation ends on the original's tick.
#[test]
fn the_humans_backflips() {
    let path = match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => std::path::PathBuf::from(dir).join("capture"),
        None => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/capture"),
    }
    .join("acceptance/recording2-backflips.csv");
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!(
            "skipped: no {} — play recording2 back with the S31 spec (journal \
             2026-10-20-s31-s31-the-humans-backflips) and record frame counters 2374..4267",
            path.display()
        );
        return;
    };
    let cap = game_sim::replay::Capture::parse(&text).expect("the recorder's format");
    let at = cap.columns.iter().enumerate().map(|(i, c)| (c.clone(), i)).collect();
    let c = Cells { cap, at };
    let n = c.cap.rows.len();
    assert_eq!((c.fc(0), c.fc(n - 1), n), (2374, 4267, 1894), "the capture is the one measured");
    let row = |fc: u64| (fc - 2374) as usize;

    // The tuning, read out of the bike object rather than assumed: only the backflip's fields and the
    // two angles its ending reads are used, the rest of `Params` is whatever the generator gives.
    let mut p = Gen(0x531).state().p;
    let (ta, tb, wa) = (c.v(0, "s31_tune_a"), c.v(0, "s31_tune_b"), c.v(0, "s31_wreck_ang"));
    p.min_backflip_time = F(ta.y());
    p.max_backflip_time = F(ta.z());
    p.min_backflip_torque = F(tb.x());
    p.max_backflip_torque = F(tb.y());
    p.backflip_torque_rate_increase = F(tb.z());
    p.backflip_torque_rate_decrease = F(tb.w());
    p.airborne_pitch_down_wreck_angle = F(wa.x());
    p.airborne_pitch_up_wreck_angle = F(wa.y());
    p.control_authority_time = c.f(0, "s31_auth.y");
    assert_eq!(
        (ta.y(), ta.z(), tb.x(), tb.y(), tb.z(), tb.w()),
        (0.0, 60.0, 800.0, 1400.0, 500.0, 2000.0),
        "the bike's backflip tuning"
    );

    let lip = |r: usize| {
        c.on(r, "on_ground")
            && c.f(r, "s31_ch_vel.y") > 5.0
            && c.f(r, "s31_contact_vel.z") > 30.0
            && c.f(r, "s31_brake.x") == 0.0
    };

    // 1. The measurements.
    for (k, fl) in HUMANS_FLIPS.iter().enumerate() {
        let (a, b) = (row(fl.lip.0), row(fl.lip.1));
        assert!(!lip(a - 1) && (a..=b).all(lip) && !lip(b + 1), "flip {k}: the lip is open {:?}", fl.lip);
        let up = |col: &str, fc: u64| !c.on(row(fc) - 1, col) && c.on(row(fc), col);
        assert!(up("flip_arm", fl.arm) && !c.on(row(fl.arm), "flip_on"), "flip {k}: arms at {}", fl.arm);
        assert!(up("flip_on", fl.fire), "flip {k}: fires at {}", fl.fire);
        let from = row(fl.cross.0);
        let cross: Vec<f32> = (from..from + fl.cross.1.len()).map(|r| c.f(r, "s31_stick.w").0).collect();
        assert!(cross.iter().zip(fl.cross.1).all(|(a, b)| (a - b).abs() < 1e-4), "flip {k}: the stick {cross:?}");
        assert!(cross[0] < -0.8 && cross[1..cross.len() - 1].iter().all(|s| s.abs() <= 0.8) && cross[cross.len() - 1] > 0.8);
        assert!((c.f(row(fl.fire), "flip_torque.x").0 - fl.torque).abs() < 1e-3, "flip {k}: the torque chosen");
        assert!((c.f(row(fl.fire), "s31_hold.x").0 - fl.hold).abs() < 1e-3, "flip {k}: the hold");
        assert!((row(fl.fire)..=row(fl.lip.1)).all(|r| c.on(r, "on_ground")), "flip {k}: fired on the ground");
        assert!(c.on(row(fl.takeoff), "airborne") && !c.on(row(fl.takeoff) - 1, "airborne"), "flip {k}: takeoff");
        match fl.over {
            Some(o) => assert!(c.on(row(o) - 1, "flip_ramp") && !c.on(row(o), "flip_ramp"), "flip {k}: over at {o}"),
            None => assert!((row(fl.takeoff) + 1..=row(fl.down)).all(|r| c.on(r, "flip_ramp")), "flip {k}: still turning"),
        }
        let wreck = c.f(row(fl.down), "wreck_reason").0 != 0.0;
        if fl.landed {
            assert!(c.on(row(fl.down), "on_ground") && !c.on(row(fl.down) - 1, "on_ground"), "flip {k}: lands at {}", fl.down);
            assert!((row(fl.fire)..row(fl.down) + 30).all(|r| c.f(r, "wreck_reason").0 == 0.0), "flip {k}: no wreck");
        } else {
            assert!(wreck && c.f(row(fl.down) - 1, "wreck_reason").0 == 0.0, "flip {k}: wrecks at {}", fl.down);
        }
        let dt = c.f(row(fl.takeoff), "dt").0;
        let rates: Vec<f32> = (row(fl.takeoff)..row(fl.down)).map(|r| c.f(r, "s31_ch_wb.x").0).collect();
        let turns = rates.iter().map(|w| w * dt).sum::<f32>() / std::f32::consts::TAU;
        let peak = rates.iter().cloned().fold(0.0, f32::min);
        println!("flip {k}: lip {} ticks, {:+.3} turns, peak body pitch rate {peak:.3} rad/s", b - a + 1, turns);
        assert!((turns - fl.turns).abs() < 1e-3, "flip {k}: {turns} turns");
        assert!((peak - fl.peak_rate).abs() < 1e-3, "flip {k}: peak rate {peak}");
    }

    // 2 and 3. Every tick, from the original's state at the end of the one before.
    let mut early = Vec::new();
    let mut turned = [0u32; 3];
    for r in 1..n {
        if c.f(r, "wreck_reason").0 != 0.0 {
            continue;
        }
        let mut air = flip_state(&c, r - 1);
        let e = c.v(r - 1, "s31_ch_euler");
        let mut ch = Chassis { vel: c.v(r - 1, "s31_ch_vel"), pitch: F(e.x()), roll: F(e.z()), ..Chassis::default() };
        let stick = c.v(r - 1, "s31_stick");
        let st = Step {
            dt: c.f(r, "dt") * F(0.5),
            // The capture's row holds the pad read's stick; the air steps read it after the rate
            // limiter has squared it (P30).
            stick: (game_sim::ride::squared(F(stick.z())), game_sim::ride::squared(F(stick.w()))),
            stick_hold: c.f(r - 1, "s31_hold.x"),
            dpad_dir: c.f(r - 1, "trick_group").0 as i32,
            brake: c.f(r - 1, "s31_brake.x"),
            touching: c.on(r - 1, "s31_touch"),
            ..Step::default()
        };
        for _ in 0..2 {
            air::backflip(&p, &mut air, &mut ch, &st);
        }
        let want = flip_state(&c, r);
        let got = (air.flip_armed, air.flip_active, air.flipping, air.flip_torque_applied, air.flip_grace.0, air.flip_torque.x());
        let orig = (want.flip_armed, want.flip_active, want.flipping, want.flip_torque_applied, want.flip_grace.0, want.flip_torque.x());
        let fc = c.fc(r);
        if got != orig {
            let what = match () {
                _ if air.flip_active && !want.flip_active => "fires",
                _ if air.flip_armed && !want.flip_armed => "arms",
                _ if air.flip_torque_applied && want.flip_torque_applied => "turns",
                _ => "?",
            };
            println!("tick {fc}: port {got:?}, original {orig:?}");
            early.push((fc, what));
        }
        if want.flip_torque_applied {
            if let Some(k) = HUMANS_FLIPS.iter().position(|fl| (fl.takeoff..=fl.down).contains(&fc)) {
                turned[k] += 1;
            }
        }
    }
    println!("turning ticks per flight {turned:?}");
    assert_eq!(early, HUMANS_MISSED, "the ticks the port does not reproduce");
    assert!(turned.iter().all(|&t| t > 30), "every flight turned: {turned:?}");
}

// ---------------------------------------------------------------------------------------------------
/// P30: **the flick the backflip reads is the original's, its clock aside.** `+0x630` is not a time
/// the stick was held: it is the player record's fastest crossing of the stick's fore-aft axis from
/// past 0.8 one way to past 0.8 the other, over its last 32 samples. The original takes those samples
/// on the hardware timer, about five a tick; `ride::Flick` takes one a frame on the simulation's
/// clock and pushes it as many times. Fed recording2's stick row by row, it is checked against the
/// captured `+0x630` on every one of the 1894 rows:
///
/// - **when**: the port has a flick on the same rows as the original, but for the edges — a flick
///   appears on the original's tick every time, and is gone within a tick of the original's going
///   ([`FLICK_EDGES`] rows differ, all at the end of one);
/// - **which way**: wherever both have one, it has the original's sign;
/// - **how fast**: a flick across two ticks is the original's within 1% (both measure it over the two
///   ticks); any other is no faster than the original's and at least a tenth of it, because the
///   original's samples can fall inside a tick and the port's cannot (a one-tick flick: 56 here,
///   469 there).
#[test]
fn the_flick_is_the_originals_clock_aside() {
    let path = match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => std::path::PathBuf::from(dir).join("capture"),
        None => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/capture"),
    }
    .join("acceptance/recording2-backflips.csv");
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!("skipped: no {} — see `the_humans_backflips`", path.display());
        return;
    };
    let cap = game_sim::replay::Capture::parse(&text).expect("the recorder's format");
    let at = cap.columns.iter().enumerate().map(|(i, c)| (c.clone(), i)).collect();
    let c = Cells { cap, at };
    let n = c.cap.rows.len();
    assert_eq!(n, 1894, "the capture is the one measured");

    let mut ring = game_sim::ride::Flick::default();
    let (mut edges, mut flicks, mut two_tick) = (0, 0, 0);
    for r in 0..n {
        let port = ring.sample(c.f(r, "s31_stick.w"), c.f(r, "dt")).0;
        let orig = c.f(r, "s31_hold.x").0;
        let was = if r == 0 { 0.0 } else { c.f(r - 1, "s31_hold.x").0 };
        if (port != 0.0) != (orig != 0.0) {
            // Only where the original's has just gone, or is about to.
            let next = if r + 1 < n { c.f(r + 1, "s31_hold.x").0 } else { 0.0 };
            assert!(orig == 0.0 && was != 0.0 || orig != 0.0 && next == 0.0, "row {r}: port {port}, original {orig}");
            edges += 1;
            continue;
        }
        if orig == 0.0 {
            continue;
        }
        assert_eq!(port < 0.0, orig < 0.0, "row {r}: port {port}, original {orig}");
        if orig != was {
            flicks += 1;
            println!("row {r}: a flick of {orig} measured {port}");
        }
        // Two ticks' travel over two ticks' time.
        if (port.abs() - 30.0).abs() < 0.01 && (orig.abs() - 30.0).abs() < 0.5 {
            assert!((port / orig - 1.0).abs() < 0.01, "row {r}: port {port}, original {orig}");
            two_tick += 1;
        } else {
            assert!(port.abs() <= orig.abs() * 1.01 && port.abs() >= orig.abs() * 0.1, "row {r}: port {port}, original {orig}");
        }
    }
    println!("{flicks} flicks, {edges} edge rows, {two_tick} two-tick rows");
    assert_eq!(flicks, 12, "every flick in the capture is met");
    assert_eq!(edges, FLICK_EDGES, "the rows the port's flick ends a tick off the original's");
    assert!(two_tick > 0);
}

/// How many rows [`the_flick_is_the_originals_clock_aside`] finds a flick on one side and not the
/// other: each the tick a flick ends, which on the original falls wherever its fifth-newer sample does.
const FLICK_EDGES: usize = 5;

// P22 — a flight, against the original's own flights

/// **The bike falls and carries the way the original's does**, measured on the original's flights in
/// both acceptance captures and on the port flying the disc's own Race 125 from where each began.
///
/// A capture's `bike_vel` is not the body's velocity: it is the type-4 actor's +0x130, which the
/// vehicle-anim object's update writes as the body's velocity (the chassis's +0x260, reached through
/// the owner's +0x488 block at +0x90) times 0.8 — `vmulx.xyz` by `0x3f4ccccd` at `0x0024d42c`. Read
/// raw, that copy changes at −25.8/s² and the drawn bike's root moves 1.25× it per second — R6's two
/// readings, and P15's "−38" and "1.25×" against the port. Divided back by 0.8 it is the body again,
/// and in body units the original's flight is the port's:
///
/// - **gravity**: dvy/dt fitted against vy over every ballistic tick is −32.2 at vy = 0 on both
///   sides — `phy::G` × the file's `Gravity` of 1 — on 600-odd ticks a capture (−32.282 against the
///   port's −32.275 on `acceptance-freeride`, −32.240 against −32.236 on `recording2-freeride`);
/// - **drag**: the slope of that fit, the vertical share of `Drag × ½ × speed² / Weight`, is the same
///   −0.061 / −0.064 per ft/s on both sides, to the third decimal;
/// - **integration**: the position advances 1.00× the body's velocity × dt, horizontally, on both
///   sides — no 1.25, no 1/24 s step.
///
/// Statistical, not bit-exact: the original's flights carry the human's stick (lean torques that do
/// not change the centre of mass's fall but do move the drawn root around it), the rows are the drawn
/// root rather than the chassis, and the port is flown hands-off over a floor far below.
mod flight {
    use game_sim::archive::{Psr, Res};
    use game_sim::phy::Phy;
    use game_sim::replay::Capture;
    use game_sim::ride::{Controls, Ride, Tables};
    use game_sim::world::Flat;
    use ps2_float::{Acc, F, Vf};
    use ps2_oracle::{Kit, skip};
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    /// The factor between the captured `bike_vel` and the body's velocity.
    const COPY: f32 = 0.8;

    fn race125p(kit: &Kit) -> Option<Vec<u8>> {
        let at = kit.ctx("iso").join("MX.PSR");
        let Ok(mut f) = File::open(&at) else {
            skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
            return None;
        };
        let mut front = vec![0u8; 64 << 10];
        f.read_exact(&mut front).expect("read the front of MX.PSR");
        let psr = Psr::parse(&front).expect("MX.PSR's directory");
        for e in &psr.entries {
            let n = e.name.to_ascii_lowercase();
            if !(n.starts_with("bike\\") && n.ends_with(".res")) {
                continue;
            }
            f.seek(SeekFrom::Start(e.at)).expect("seek");
            let mut bytes = vec![0u8; e.size as usize];
            f.read_exact(&mut bytes).expect("read");
            let Ok(res) = Res::parse(&bytes) else { continue };
            for r in &res.entries {
                if r.name.to_ascii_lowercase().ends_with("race125p.phy") {
                    return Some(res.read(&bytes, r).expect("race125p.phy"));
                }
            }
        }
        panic!("no race125p.phy in any Bike\\*.res");
    }

    /// One captured tick in body units: dt, velocity, the drawn root's position.
    #[derive(Clone, Copy)]
    struct Row {
        dt: f32,
        v: [f32; 3],
        p: [f32; 3],
    }

    fn rows(cap: &Capture) -> Vec<Row> {
        let col = |name: &str| {
            cap.columns.iter().position(|c| c == name).unwrap_or_else(|| panic!("no {name} column"))
        };
        let (dt, vx, vy, vz) = (col("dt"), col("bike_vel.x"), col("bike_vel.y"), col("bike_vel.z"));
        let (px, py, pz) = (col("bike_xform.m30"), col("bike_xform.m31"), col("bike_xform.m32"));
        let f = |r: &Vec<String>, i: usize| r[i].parse::<f32>().expect("a number");
        cap.rows
            .iter()
            .map(|r| Row {
                dt: f(r, dt),
                v: [f(r, vx) / COPY, f(r, vy) / COPY, f(r, vz) / COPY],
                p: [f(r, px), f(r, py), f(r, pz)],
            })
            .collect()
    }

    /// The ballistic stretches: runs of at least 20 ticks whose dvy/dt is free fall's and nothing
    /// else's (no ground, no landing), as index ranges `a..b` with tick `a - 1` the one before.
    fn stretches(rows: &[Row]) -> Vec<std::ops::Range<usize>> {
        let mut out = Vec::new();
        let mut start = None;
        for i in 1..=rows.len() {
            let falling = i < rows.len() && {
                let g = (rows[i].v[1] - rows[i - 1].v[1]) / rows[i].dt;
                -36.0 < g && g < -28.0
            };
            match (falling, start) {
                (true, None) => start = Some(i),
                (false, Some(a)) => {
                    // The stretch's first and last ticks can be part ground: drop them.
                    if i - a >= 22 {
                        out.push(a + 1..i - 1);
                    }
                    start = None;
                }
                _ => {}
            }
        }
        out
    }

    /// dvy/dt = a + b·vy by least squares, and the mean of the horizontal |dpos| / (|v| dt).
    #[derive(Default)]
    struct Fit {
        pts: Vec<(f64, f64)>,
        carry: Vec<f64>,
    }

    impl Fit {
        fn tick(&mut self, before: Row, after: Row) {
            let dt = after.dt as f64;
            let g = (after.v[1] - before.v[1]) as f64 / dt;
            self.pts.push((after.v[1] as f64, g));
            let d = ((after.p[0] - before.p[0]) as f64).hypot((after.p[2] - before.p[2]) as f64);
            let s = (after.v[0] as f64).hypot(after.v[2] as f64);
            if s > 5.0 {
                self.carry.push(d / (s * dt));
            }
        }

        fn line(&self) -> (f64, f64) {
            let n = self.pts.len() as f64;
            let mx = self.pts.iter().map(|p| p.0).sum::<f64>() / n;
            let my = self.pts.iter().map(|p| p.1).sum::<f64>() / n;
            let sxy: f64 = self.pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
            let sxx: f64 = self.pts.iter().map(|p| (p.0 - mx) * (p.0 - mx)).sum();
            let b = sxy / sxx;
            (my - b * mx, b)
        }

        fn carry(&self) -> f64 {
            self.carry.iter().sum::<f64>() / self.carry.len() as f64
        }
    }

    #[test]
    fn a_flight_falls_and_carries_like_the_originals() {
        let kit = match Kit::load() {
            Ok(k) => k,
            Err(why) => return skip(&why),
        };
        let Some(bytes) = race125p(&kit) else { return };
        let dir = kit.ctx("capture").join("acceptance");
        let mut csvs: Vec<_> = std::fs::read_dir(&dir)
            .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect())
            .unwrap_or_default();
        csvs.retain(|p| p.extension().is_some_and(|x| x == "csv"));
        // C7's biplane capture (watch/biplane.toml) is parked here too: the human flies the plane and
        // the bike stands parked, so it has no bike flight to fit.
        csvs.retain(|p| {
            std::fs::read_to_string(p)
                .map(|t| !t.lines().next().unwrap_or("").split(',').any(|c| c == "plane_type"))
                .unwrap_or(true)
        });
        csvs.sort();
        if csvs.is_empty() {
            return skip(&format!("no captures in {}", dir.display()));
        }
        let tables = Tables::build();
        let p = Phy::read_bytes(&bytes, &tables.curve);
        let mounts = (p.front.body_tire_force_position, p.rear.body_tire_force_position);

        let mut report = String::new();
        for csv in &csvs {
            let text = std::fs::read_to_string(csv).expect("read the capture");
            let cap = Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", csv.display()));
            let rows = rows(&cap);
            let (mut theirs, mut ours) = (Fit::default(), Fit::default());
            let runs = stretches(&rows);
            for run in &runs {
                for i in run.clone() {
                    theirs.tick(rows[i - 1], rows[i]);
                }
                // The port, from the tick before the stretch, high over a floor it never reaches.
                let first = rows[run.start - 1];
                let [x, y, z] = first.v;
                let facing = Vf::new(x, 0.0, z, 0.0);
                let len = (x * x + z * z).sqrt();
                let facing = Vf::new(facing.x() / len, 0.0, facing.z() / len, 0.0);
                let mut ride = Ride::new(&p, mounts, Vf::new(0.0, 5000.0, 0.0, 1.0), facing);
                let vel = Vf::new(x, y, z, 1.0);
                ride.chassis.vel = vel;
                ride.chassis.speed =
                    Acc::mula(vel.x(), vel.x()).madda(vel.y(), vel.y()).madd(vel.z(), vel.z()).sqrt();
                let mut world = Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) };
                let at = |ride: &Ride, dt: f32| {
                    let (q, v) = (ride.position(), ride.chassis.vel);
                    Row { dt, v: [v.x(), v.y(), v.z()], p: [q.x(), q.y(), q.z()] }
                };
                let mut before = at(&ride, first.dt);
                for (k, i) in run.clone().enumerate() {
                    ride.frame(&p, &tables, &Controls::default(), F(rows[i].dt), &mut world);
                    let now = at(&ride, rows[i].dt);
                    // A fresh ride's first frame has no integrator history behind it.
                    if k > 0 {
                        ours.tick(before, now);
                    }
                    before = now;
                }
            }
            let ((ta, tb), (oa, ob)) = (theirs.line(), ours.line());
            let (tc, oc) = (theirs.carry(), ours.carry());
            let name = csv.file_name().unwrap().to_string_lossy();
            report += &format!(
                "{name}: {} flights, {} ticks — original dvy/dt {ta:.3} {tb:+.4}·vy, carry {tc:.4}; \
                 port {oa:.3} {ob:+.4}·vy, carry {oc:.4}\n",
                runs.len(),
                theirs.pts.len(),
            );
            assert!(theirs.pts.len() >= 300, "{name}: only {} ballistic ticks", theirs.pts.len());
            assert!((ta - oa).abs() < 0.05, "{name}: gravity — {report}");
            assert!((tb - ob).abs() < 0.005, "{name}: drag — {report}");
            assert!((tc - 1.0).abs() < 0.01 && (oc - 1.0).abs() < 0.01, "{name}: carry — {report}");
        }
        println!("{report}");
    }
}

/// MD30: MX vs. ATV's air control under its 360, as SLUS_211.04's reads (openmx-mods
/// `notes/tricks.md`, "The whip": the stick is left out only under both moves' torques at once, yaw
/// and roll only under a backflip's ramp, and the 360's ramp damps). Over 10,000 generated airborne
/// substeps with no flip of MX Unleashed's going: while the 360 turns, the moment is the stick's own
/// plus plain damping — the very moment a hard lean (the damped path, no 360) gives with the same
/// stick — and there is no lean push; once it has ended in the air, its ramp still damps the same
/// moment and the push is back; and under a flip of MD20's at once the stick stays out, as MD20 rode.
#[test]
fn md30_a_three_sixty_alone_keeps_the_stick_and_damps_to_the_ground() {
    let bits = |v: Vf| [v.x(), v.y(), v.z(), v.w()].map(f32::to_bits);
    let mut g = Gen(0x3d30);
    let (mut n, mut stick_moved) = (0, 0);
    while n < 10_000 {
        let mut s = g.state();
        if s.air.on_ground {
            continue;
        }
        n += 1;
        (s.air.flip_torque_applied, s.air.flipping, s.air.flip_active) = (false, false, false);
        let run = |s: &State, spinning: bool, ramp: bool, stick: (F, F), lean: F| {
            let mut t = s.clone();
            (t.st.spinning, t.st.spin_ramp, t.st.stick, t.st.lean) = (spinning, ramp, stick, lean);
            air::airborne_control(&t.p, &mut t.air, &mut t.ch, &t.st);
            (bits(t.ch.moment), bits(t.ch.force))
        };
        let lean = s.st.lean;
        let (turning, pushed) = run(&s, true, true, s.st.stick, lean);
        let (damped, _) = run(&s, false, false, s.st.stick, F(0.5));
        assert_eq!(turning, damped, "{n}: the 360's moment is the stick's, damped");
        assert_eq!(pushed, bits(s.ch.force), "{n}: no lean push while it turns");
        let (ended, _) = run(&s, false, true, s.st.stick, lean);
        assert_eq!(ended, turning, "{n}: its ramp damps on once it has ended");
        let (still, _) = run(&s, true, true, (F::ZERO, F::ZERO), lean);
        stick_moved += (still != turning) as u32;
        let mut both = s.clone();
        both.air.flipping = true;
        assert_eq!(run(&both, true, true, both.st.stick, lean).0, run(&both, true, true, (F::ZERO, F::ZERO), lean).0, "{n}: MD20's");
    }
    assert!(stick_moved > 5_000, "the stick moved the moment on {stick_moved} of 10,000");
}
