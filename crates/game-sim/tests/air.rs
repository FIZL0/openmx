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
