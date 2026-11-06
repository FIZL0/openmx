//! The AI roster (S12a), proved against the original's own player spawn; and, at the end of the file,
//! the AI rider's trick decision (S12d).
//!
//! The whole spawn runs in the interpreter: the allocator, the player constructor and the two virtual
//! calls it makes on each player (its set-up, and the player manager's add) are hooked, so what is left
//! is exactly the part the port is — the AllAI pair, the ghost-race branch, the skill range from the
//! difficulty byte, the identity draws with their re-draw on a repeat, and one skill draw per AI
//! rider. Ten thousand rosters over every difficulty byte, racer and human counts from nothing to a
//! full grid, database sizes down to just enough, and the generator anywhere in its chain; each one is
//! compared player by player (kind, number, identity, skill) and on the generator's state afterwards.

use game_sim::ai::{
    FIRST_AI_IDENTITY, Flight, JUMP, Jump, RosterIn, TrickPlan, ask_for, gate_holds, plan_trick, spawn_roster,
};
use game_sim::rng::Rng;
use game_sim::trick::{SLOTS, Set, Stunt, Trick, Tricks};
use ps2_float::F;
use ps2_oracle::{addr, game_or_skip, skip};
use std::cell::RefCell;
use std::rc::Rc;

const BUDGET: u64 = 5_000_000;

/// The objects the spawn reads, laid out in RAM no program segment uses.
const CALLER: u32 = 0x0180_0000; // the spawn's own object: +0x30 -> +0x30 the player manager
const LINK: u32 = 0x0180_1000;
const MANAGER: u32 = 0x0180_2000; // +0x08 its vtable
const APP: u32 = 0x0180_3000;
const SETTINGS: u32 = 0x0180_4000;
const EVENT: u32 = 0x0180_5000;
const HUMAN_IDS: u32 = 0x0180_6000;
const DATABASE: u32 = 0x0180_7000;
const ID_HOLDER: u32 = 0x0180_8000;
const ID_LIST: u32 = 0x0180_9000;
const ID_ENTRIES: u32 = 0x0180_a000;
const VTABLE: u32 = 0x0180_b000;
/// Not code: the two virtual calls land here and are hooked.
const HOOK_SET_UP: u32 = 0x0180_c000;
const HOOK_ADD: u32 = 0x0180_c008;
/// Where the hooked allocator hands out players.
const PLAYERS: u32 = 0x0181_0000;
const PLAYER_SIZE: u32 = 0x800;

/// The entry pointer the database holds for identity `i`.
fn entry(i: i32) -> u32 {
    0x5a00_0000 + i as u32
}

struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u32) -> i32 {
        ((self.next() >> 32) as u32 % n) as i32
    }
}

fn named(names: &[&str]) -> Option<Vec<u32>> {
    let mut out = Vec::new();
    for n in names {
        match addr(n) {
            Ok(a) => out.push(a),
            Err(why) => {
                skip(&why);
                return None;
            }
        }
    }
    Some(out)
}

/// A player as the spawn left it: (kind, +0x6b0 number, +0x6b8 identity entry, +0x6bc skill).
type Seen = (u32, i32, u32, i32);

#[test]
fn the_roster_matches_over_ten_thousand_spawns() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&[
        "spawn_racers",
        "operator_new",
        "player_construct",
        "app_pointer",
        "all_ai",
        "rand_seeded",
        "rand_state",
    ]) else {
        return;
    };
    let [spawn, new, construct, app_pointer, all_ai_knob, seeded, rand_state] = at[..] else {
        unreachable!()
    };

    let next = Rc::new(RefCell::new(PLAYERS));
    let n = next.clone();
    ee.hooks.insert(
        new,
        Box::new(move |ee| {
            let at = *n.borrow();
            *n.borrow_mut() += PLAYER_SIZE;
            for k in (0..PLAYER_SIZE).step_by(4) {
                ee.set_u32(at + k, 0);
            }
            ee.set(2, u64::from(at));
        }),
    );
    // The constructor's own fields, as it leaves them, and the vtable the two calls go through.
    ee.hooks.insert(
        construct,
        Box::new(|ee| {
            let p = ee.r(4) as u32;
            ee.set_u32(p + 8, VTABLE);
            ee.set_u32(p + 0x6b0, u32::MAX);
            ee.set_u32(p + 0x6b4, 0);
            ee.set_u32(p + 0x6b8, 0);
            ee.set_u32(p + 0x6bc, 1);
            ee.set(2, u64::from(p));
        }),
    );
    let kinds = Rc::new(RefCell::new(Vec::<(u32, u32)>::new()));
    let added = Rc::new(RefCell::new(Vec::<u32>::new()));
    let k = kinds.clone();
    ee.hooks.insert(
        HOOK_SET_UP,
        Box::new(move |ee| k.borrow_mut().push((ee.r(4) as u32, ee.r(5) as u32))),
    );
    let a = added.clone();
    ee.hooks.insert(HOOK_ADD, Box::new(move |ee| a.borrow_mut().push(ee.r(5) as u32)));

    ee.set_u32(VTABLE + 0xb8, HOOK_SET_UP);
    ee.set_u32(VTABLE + 0x74, HOOK_ADD);
    ee.set_u32(MANAGER + 8, VTABLE);
    ee.set_u32(CALLER + 0x30, LINK);
    ee.set_u32(LINK + 0x30, MANAGER);
    ee.set_u32(app_pointer, APP);
    ee.set_u32(APP + 0x3d4, SETTINGS);
    ee.set_u32(APP + 0x3d8, DATABASE);
    ee.set_u32(SETTINGS + 0xc, EVENT);
    ee.set_u32(SETTINGS + 0x10, HUMAN_IDS);
    ee.set_u32(DATABASE + 0x14, ID_HOLDER);
    ee.set_u32(ID_HOLDER, ID_LIST);
    ee.set_u32(ID_LIST + 8, ID_ENTRIES);
    for i in 0..64 {
        ee.set_u32(ID_ENTRIES + i * 4, entry(i as i32));
        ee.set_u32(HUMAN_IDS + i * 4, 0x4800_0000 + i);
    }
    // Seeded already, so the spawn never asks the clock.
    ee.set_u32(seeded, 1);

    let mut pick = Pick(0x51a3_9e37_c0de_0012);
    let (mut ghosts, mut all, mut full) = (0, 0, 0);
    for case in 0..10_000 {
        let racers = pick.below(13);
        let humans = pick.below(5);
        let all_ai = pick.below(8) == 0 && racers >= 1;
        // The knob makes it one human whatever was asked.
        let ais = (racers - if all_ai { 1 } else { humans }).max(0);
        // Just enough identities to tell the riders apart, up to a database of sixty.
        let identities = FIRST_AI_IDENTITY + ais.max(1) + pick.below(60 - FIRST_AI_IDENTITY as u32 - 12);
        let input = RosterIn {
            racers,
            humans,
            difficulty: if case % 3 == 0 { [0, 69, 70, 79, 80, 89, 90, 99, 100, 255][case % 10] } else { pick.below(256) as u8 },
            identities,
            all_ai,
            ghost_race: pick.below(16) == 0,
        };
        let state = pick.next();

        ee.write(rand_state, 8, u128::from(state));
        ee.set_u32(all_ai_knob, u32::from(input.all_ai));
        ee.set_u32(SETTINGS, input.racers as u32);
        ee.set_u32(EVENT + 4, if input.ghost_race { 3 } else { 1 });
        ee.write(EVENT + 0x116, 1, 0);
        ee.write(EVENT + 0x117, 1, u128::from(input.difficulty));
        ee.set_u32(ID_LIST, input.identities as u32);
        ee.set_u32(CALLER + 0xe8, 0);
        ee.set_u32(CALLER + 0xf0, 0);
        *next.borrow_mut() = PLAYERS;
        kinds.borrow_mut().clear();
        added.borrow_mut().clear();

        ee.call(spawn, &[u64::from(CALLER), input.humans as u64], &[], BUDGET);

        let mut rng = Rng::from_state(state);
        let roster = spawn_roster(&mut rng, input);
        let what = format!("case {case}: {input:?} from {state:#018x}");

        assert_eq!(rng.state(), ee.read(rand_state, 8) as u64, "{what}: the generator ends elsewhere");
        let got: Vec<Seen> = added
            .borrow()
            .iter()
            .map(|&p| {
                let kind = kinds.borrow().iter().find(|&&(o, _)| o == p).map_or(0, |&(_, k)| k);
                (kind, ee.i32_at(p + 0x6b0), ee.u32_at(p + 0x6b8), ee.i32_at(p + 0x6bc))
            })
            .collect();
        let mut want: Vec<Seen> = Vec::new();
        if roster.ghost {
            want.push((3, 1, 0, 1));
            want.push((1, 0, 0, 1));
        } else {
            for h in 0..roster.humans {
                want.push((1, h, 0, 1));
            }
            for (j, r) in roster.ai.iter().enumerate() {
                want.push((2, roster.humans + j as i32, entry(r.identity), r.skill));
            }
        }
        assert_eq!(got, want, "{what}: the players differ");
        match roster.all_ai {
            Some(d) => {
                all += 1;
                assert_eq!(ee.i32_at(CALLER + 0xe8), d.racer, "{what}: the AllAI racer");
                assert_eq!(ee.f32_at(CALLER + 0xf0).to_bits(), d.seconds.to_f32().to_bits(), "{what}: the AllAI time");
            }
            None => assert_eq!(ee.u32_at(CALLER + 0xe8), 0, "{what}: an AllAI draw with the knob off"),
        }
        ghosts += usize::from(roster.ghost);
        full += usize::from(roster.ai.len() >= 8);
    }
    eprintln!("ai roster: {ghosts} ghost races, {all} AllAI, {full} grids of 8+ AI");
    // The branches the cases were meant to reach, reached.
    assert!(ghosts > 300 && all > 600 && full > 100, "ghosts {ghosts}, AllAI {all}, full grids {full}");
}

// ---------------------------------------------------------------------------------------------------
// S12b: the ground controller, against the original's own, on the recording2 free09 image.
// ---------------------------------------------------------------------------------------------------

mod controls {
    use super::{Pick, named};
    use game_sim::ai::{self, CURVE_INV_STEP, CURVE_STEP, Controls, Situation, Tuning, flag};
    use game_sim::camera::{CURVE_ENTRIES, CurveTable};
    use game_sim::ragdoll::Tables;
    use ps2_float::{F, Vf};
    use ps2_oracle::{Ee, Kit, addr, skip};

    /// The free09 save-state's RAM: three AI riders at skill 4, every one of them on the ground
    /// controller (their interface is type 1).
    const STATE: &str = "recording2.p2m2_SaveState.eeMemory.bin";
    const RIDERS: [u32; 3] = [0x0148_3eb0, 0x00c6_4000, 0x0104_8000];
    const BUDGET: u64 = 200_000;

    fn state() -> Option<Ee> {
        let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
        let path = kit.ctx("states").join(STATE);
        if !path.exists() {
            skip(&format!("{} is not there: `./kit state ram` of recording2.p2m2_SaveState.p2s", path.display()));
            return None;
        }
        let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
        ee.set(28, u64::from(addr("boot_gp").unwrap()));
        Some(ee)
    }

    /// The game's curve table out of the image, and the port's fine arc cosine (proved the game's in
    /// `tests/ragdoll.rs`); and the step beside the table is the port's.
    fn tables(ee: &Ee) -> Tables {
        assert_eq!(ee.f32_at(addr("curve_table_step").unwrap()).to_bits(), CURVE_STEP.to_bits());
        assert_eq!(ee.f32_at(addr("curve_table_invstep").unwrap()).to_bits(), CURVE_INV_STEP.to_bits());
        let base = ee.u32_at(addr("curve_table_ptr").unwrap());
        let curve: Vec<[f32; 2]> =
            (0..CURVE_ENTRIES as u32).map(|k| [ee.f32_at(base + 8 * k), ee.f32_at(base + 8 * k + 4)]).collect();
        Tables { curve: CurveTable::from_entries(&curve), ..Tables::build() }
    }

    fn quad(ee: &Ee, a: u32) -> Vf {
        Vf([0, 4, 8, 12].map(|o| ee.f32_at(a + o)))
    }

    fn set_quad(ee: &mut Ee, a: u32, v: Vf) {
        for (k, x) in v.0.iter().enumerate() {
            ee.set_f32(a + 4 * k as u32, *x);
        }
    }

    fn f(ee: &Ee, a: u32) -> F {
        F(ee.f32_at(a))
    }

    /// What the controller reads, off a rider in RAM.
    fn read(ee: &Ee, ai: u32) -> (Situation, Tuning) {
        let at = Situation {
            up: quad(ee, ai + 0xa0),
            forward: quad(ee, ai + 0xb0),
            height: f(ee, ai + 0xc4),
            ground: f(ee, ai + 0x204),
            ground_normal: quad(ee, ai + 0x2c0),
            spin: quad(ee, ai + 0x70),
            speed: f(ee, ai + 0x80),
            flags: u64::from(ee.u32_at(ai + 0x30)) | u64::from(ee.u32_at(ai + 0x34)) << 32,
            target_dir: quad(ee, ai + 0x1d0),
            target_speed: f(ee, ai + 0x1e0),
            min_speed: f(ee, ai + 0x254),
            pitch: f(ee, ai + 0xf4),
        };
        let tune = Tuning {
            steering_gain: f(ee, ai + 0x208),
            oversteer_gain: f(ee, ai + 0x21c),
            aim_gain: f(ee, ai + 0x2d0),
            pitch_distance: f(ee, ai + 0x2b4),
            pitch_gain: f(ee, ai + 0x2d4),
            pitch_filter: f(ee, ai + 0x2d8),
            landing_pitch: f(ee, ai + 0x2ec),
            slow_tolerance: f(ee, ai + 0x260),
            fast_tolerance: f(ee, ai + 0x264),
            cruise_base: f(ee, ai + 0x32c),
            cruise_gain: f(ee, ai + 0x330),
            brake_gain: f(ee, ai + 0x334),
            steer_scale: f(ee, ai + 0x100),
            pitch_scale: f(ee, ai + 0x104),
        };
        (at, tune)
    }

    fn write(ee: &mut Ee, ai: u32, at: &Situation, tune: &Tuning) {
        set_quad(ee, ai + 0xa0, at.up);
        set_quad(ee, ai + 0xb0, at.forward);
        ee.set_f32(ai + 0xc4, at.height.0);
        ee.set_f32(ai + 0x204, at.ground.0);
        set_quad(ee, ai + 0x2c0, at.ground_normal);
        set_quad(ee, ai + 0x70, at.spin);
        ee.set_f32(ai + 0x80, at.speed.0);
        ee.set_u32(ai + 0x30, at.flags as u32);
        ee.set_u32(ai + 0x34, (at.flags >> 32) as u32);
        set_quad(ee, ai + 0x1d0, at.target_dir);
        ee.set_f32(ai + 0x1e0, at.target_speed.0);
        ee.set_f32(ai + 0x254, at.min_speed.0);
        ee.set_f32(ai + 0xf4, at.pitch.0);
        for (o, v) in [
            (0x208, tune.steering_gain),
            (0x21c, tune.oversteer_gain),
            (0x2d0, tune.aim_gain),
            (0x2b4, tune.pitch_distance),
            (0x2d4, tune.pitch_gain),
            (0x2d8, tune.pitch_filter),
            (0x2ec, tune.landing_pitch),
            (0x260, tune.slow_tolerance),
            (0x264, tune.fast_tolerance),
            (0x32c, tune.cruise_base),
            (0x330, tune.cruise_gain),
            (0x334, tune.brake_gain),
            (0x100, tune.steer_scale),
            (0x104, tune.pitch_scale),
        ] {
            ee.set_f32(ai + o, v.0);
        }
    }

    /// What the controller left in RAM, as the port's [`Controls`].
    fn left(ee: &Ee, ai: u32) -> Controls {
        Controls {
            steer: f(ee, ai + 0xf0),
            pitch: f(ee, ai + 0xf4),
            throttle: f(ee, ai + 0xe0),
            brake: f(ee, ai + 0xe4),
            heading: f(ee, ai + 0x1f4),
            aim: f(ee, ai + 0x1f0),
            roll: f(ee, ai + 0x1f8),
            flat_forward: quad(ee, ai + 0x270),
            target_speed: f(ee, ai + 0x1e0),
        }
    }

    fn bits(c: &Controls) -> [u32; 12] {
        let w = c.flat_forward.to_bits();
        [
            c.steer.0.to_bits(),
            c.pitch.0.to_bits(),
            c.throttle.0.to_bits(),
            c.brake.0.to_bits(),
            c.heading.0.to_bits(),
            c.aim.0.to_bits(),
            c.roll.0.to_bits(),
            w[0],
            w[1],
            w[2],
            w[3],
            c.target_speed.0.to_bits(),
        ]
    }

    const NAMES: [&str; 12] =
        ["steer", "pitch", "throttle", "brake", "heading", "aim", "roll", "flat.x", "flat.y", "flat.z", "flat.w", "target"];

    fn differs(ours: &Controls, theirs: &Controls) -> Vec<String> {
        bits(ours)
            .iter()
            .zip(bits(theirs))
            .zip(NAMES)
            .filter(|((o, t), _)| **o != *t)
            .map(|((o, t), n)| {
                format!("{n}: ours {} ({o:#010x}) theirs {} ({t:#010x})", f32::from_bits(*o), f32::from_bits(t))
            })
            .collect()
    }

    impl Pick {
        /// Mostly a float in `lo..hi`; now and then one of the edges the controller's tests sit on.
        fn float(&mut self, lo: f32, hi: f32) -> F {
            const EDGES: [f32; 9] = [0.0, -0.0, 1.0, -1.0, 1.5707964, -1.5707964, 3.1415927, -3.1415927, 0.5];
            if self.below(16) == 0 {
                return F(EDGES[self.below(EDGES.len() as u32) as usize]);
            }
            let u = (self.next() >> 40) as f32 / (1u64 << 24) as f32;
            F(lo + (hi - lo) * u)
        }
        fn unit(&mut self) -> Vf {
            let v = [self.float(-1.0, 1.0).0, self.float(-1.0, 1.0).0, self.float(-1.0, 1.0).0];
            let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if n < 1e-3 || self.below(8) == 0 {
                return Vf([v[0], v[1], v[2], self.float(-2.0, 2.0).0]);
            }
            Vf([v[0] / n, v[1] / n, v[2] / n, self.float(-2.0, 2.0).0])
        }
        fn flat(&mut self) -> Vf {
            let a = self.float(-7.0, 7.0).0;
            Vf([a.cos(), 0.0, a.sin(), 0.0])
        }
    }

    fn case(p: &mut Pick) -> (Situation, Tuning) {
        let forward = p.unit();
        let target_dir = match p.below(4) {
            0 => p.unit(),
            1 => Vf([forward.0[0], 0.0, forward.0[2], 0.0]),
            _ => p.flat(),
        };
        let flags = [flag::ON_GROUND, flag::OVERSTEER_CORRECTION, flag::CRUISE, flag::GRADED_BRAKE, 0x80, 0x0100_0000_0000]
            .iter()
            .filter(|_| p.below(2) == 0)
            .fold(0x03e0_b80b, |a, b| a | b)
            & !if p.below(2) == 0 { flag::ON_GROUND } else { 0 };
        let speed = p.float(0.0, 80.0);
        let at = Situation {
            up: p.unit(),
            forward,
            height: p.float(-20.0, 60.0),
            ground: p.float(-20.0, 40.0),
            ground_normal: p.unit(),
            spin: Vf([p.float(-6.0, 6.0).0, p.float(-6.0, 6.0).0, p.float(-6.0, 6.0).0, 0.0]),
            speed,
            flags,
            target_dir,
            target_speed: if p.below(4) == 0 { speed + p.float(-3.0, 3.0) } else { p.float(-10.0, 150.0) },
            min_speed: p.float(0.0, 40.0),
            pitch: p.float(-1.5, 1.5),
        };
        let tune = Tuning {
            steering_gain: p.float(0.0, 6.0),
            oversteer_gain: p.float(-1.0, 1.0),
            aim_gain: p.float(-3.0, 3.0),
            pitch_distance: p.float(0.0, 30.0),
            pitch_gain: p.float(-4.0, 4.0),
            pitch_filter: p.float(0.0, 1.0),
            landing_pitch: if p.below(8) == 0 { p.float(-3.0e5, 3.0e5) } else { p.float(-20.0, 20.0) },
            slow_tolerance: p.float(0.0, 10.0),
            fast_tolerance: p.float(0.0, 10.0),
            cruise_base: p.float(0.0, 0.05),
            cruise_gain: p.float(0.0, 2.0),
            brake_gain: p.float(0.0, 0.25),
            steer_scale: p.float(-1.5, 1.5),
            pitch_scale: p.float(-1.5, 1.5),
        };
        (at, tune)
    }

    /// Ten thousand rider situations through the original's controller and the port, on the image's
    /// first AI rider: every output and every working value it leaves, bit for bit.
    #[test]
    fn the_controller_matches_over_ten_thousand_ticks() {
        let Some(at) = named(&["ai_controls"]) else { return };
        let Some(mut ee) = state() else { return };
        let tables = tables(&ee);
        let ai = RIDERS[0];
        let mut p = Pick(0x5eed_12b0_c0ff_ee01);
        let mut bad = 0;
        let mut seen = [0usize; 6];
        for n in 0..10_000 {
            let (sit, tune) = case(&mut p);
            write(&mut ee, ai, &sit, &tune);
            ee.call(at[0], &[u64::from(ai)], &[], BUDGET);
            assert_eq!(ee.r(2), 1, "case {n}: the controller returns 1");
            let theirs = left(&ee, ai);
            let ours = ai::controls(&tables, &sit, &tune);
            let d = differs(&ours, &theirs);
            if !d.is_empty() {
                if bad < 10 {
                    println!("case {n}: {sit:?} {tune:?}\n  {}", d.join("\n  "));
                }
                bad += 1;
            }
            let h = sit.height - sit.ground;
            seen[0] += usize::from(sit.flags & flag::ON_GROUND == 0 && !(h <= tune.pitch_distance));
            seen[1] += usize::from(sit.flags & flag::ON_GROUND == 0 && h <= tune.pitch_distance);
            seen[2] += usize::from(theirs.throttle.0 == 1.0);
            seen[3] += usize::from(theirs.brake.0 > 0.0 && theirs.brake.0 < 1.0);
            seen[4] += usize::from(theirs.throttle.0 > 0.0 && theirs.throttle.0 < 1.0);
            seen[5] += usize::from(theirs.steer.0.abs() == 1.0);
        }
        println!("air {} landing {} open {} graded brake {} cruise {} full lock {}", seen[0], seen[1], seen[2], seen[3], seen[4], seen[5]);
        assert_eq!(bad, 0, "{bad} of 10,000 ticks differ");
        assert!(seen.iter().all(|&s| s > 100), "a branch went untried: {seen:?}");
    }

    /// The three AI riders as the image holds them: the port on what each one's controller read
    /// gives what that controller left, and the original run again on the image agrees.
    #[test]
    fn the_free09_riders_controls_are_the_originals() {
        let Some(at) = named(&["ai_controls"]) else { return };
        let Some(mut ee) = state() else { return };
        let tables = tables(&ee);
        for ai in RIDERS {
            let (sit, tune) = read(&ee, ai);
            let held = left(&ee, ai);
            let ours = ai::controls(&tables, &sit, &tune);
            println!("rider {ai:#x}: held {held:?}\n  ours {ours:?}");
            let d = differs(&ours, &held);
            assert!(d.is_empty(), "rider {ai:#x} against the image:\n  {}", d.join("\n  "));
            ee.call(at[0], &[u64::from(ai)], &[], BUDGET);
            let d = differs(&ours, &left(&ee, ai));
            assert!(d.is_empty(), "rider {ai:#x} against the original rerun:\n  {}", d.join("\n  "));
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// S12d — when an AI rider asks for a trick, and which
// ---------------------------------------------------------------------------------------------------

/// The rider, with its stunt records, and the AI brain, in RAM nothing else uses. The rider's AI
/// interface is the sub-object at +0x10d0, whose vtable pointer is at its +8.
const RIDER: u32 = 0x0190_0000;
const IFACE: u32 = RIDER + 0x10d0;
const RECORDS: u32 = 0x0193_0000;
const BRAIN: u32 = 0x0194_0000;
const NODE: u32 = 0x0195_0000;
/// The rider's trick fields the decision writes, and its stunt array.
const ARMED: u32 = 0xa18;
const IN_PROGRESS: u32 = 0xa0c;
const STUNTS: u32 = 0xa34;
const REQUESTED: u32 = 0xa7c;
const RESTART: u32 = 0xaf8;
const RECORD: u32 = 0x54;

/// A float in `-span..span`, now and then an edge the comparisons turn on.
fn float(pick: &mut Pick, span: f32) -> f32 {
    match pick.below(16) {
        0 => 0.0,
        1 => -0.0,
        _ => {
            let r = pick.next();
            let unit = (r >> 40) as f32 / (1u32 << 24) as f32;
            if r & 1 == 0 { span * unit } else { -span * unit }
        }
    }
}

/// Stand in for the library's `strcmp`, which uses two quadword compares the interpreter lacks (as in
/// tests/trick.rs).
fn hook_strcmp(ee: &mut ps2_oracle::Ee, strcmp: u32) {
    ee.hooks.insert(
        strcmp,
        Box::new(|ee| {
            let (mut a, mut b) = (ee.r(4) as u32, ee.r(5) as u32);
            loop {
                let (x, y) = (ee.u8_at(a), ee.u8_at(b));
                if x != y || x == 0 {
                    ee.set(2, (i64::from(x) - i64::from(y)) as u64);
                    return;
                }
                a += 1;
                b += 1;
            }
        }),
    );
}

/// The rider's 24 stunt records as its own copy lays them out.
fn write_stunts(ee: &mut ps2_oracle::Ee, stunts: &[Stunt]) {
    for (k, s) in stunts.iter().enumerate() {
        let at = RECORDS + k as u32 * RECORD;
        let mut name = [0u8; 0x20];
        name[..s.name.len()].copy_from_slice(s.name.as_bytes());
        ee.load(at, &name);
        ee.set_f32(at + 0x48, s.duration.to_f32());
        ee.set_u32(at + 0x50, s.index as u32);
    }
    ee.set_u32(RIDER + STUNTS, RECORDS);
}

/// The brain's whole trick decision against the port, with the original's own interface vtable, its
/// chooser and its "do this trick" call behind it, and the original's `rand()`.
#[test]
fn the_trick_decision_matches_over_ten_thousand_frames() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_trick_plan", "ai_stunt_vtable", "rand_state", "strcmp"]) else {
        return;
    };
    let [plan_fn, vtable, rand_state, strcmp] = at[..] else { unreachable!() };
    hook_strcmp(&mut ee, strcmp);

    // flag bits the decision reads, and two it does not, so a port that clears too much shows.
    const BITS: [u64; 8] = [0x2, 0x8, 0x10, 0x20, 0x2000, 0x8_0000, 0x1, 0x1_0000_0000];
    let mut pick = Pick(0x2bd1_77e0_5eed_0d12);
    let mut seen = [0u32; 10];
    for case in 0..10_000 {
        let mut flags: u64 = 0;
        for (k, b) in BITS.iter().enumerate() {
            // The roll needs 2 and 8 set and 0x10, 0x20 clear: lean the draws that way.
            let p = match k {
                0 | 1 => 7,
                2 => 1,
                3 => 3,
                _ => 4,
            };
            if pick.below(8) < p {
                flags |= b;
            }
        }
        let attached = pick.below(16) != 0;
        let jump = match pick.below(4) {
            0 => None,
            1 => Some(Jump { flags: pick.below(0x100) as u32 & !JUMP, airtime: F(float(&mut pick, 4.0)) }),
            _ => Some(Jump { flags: pick.below(0x100) as u32 | JUMP, airtime: F(float(&mut pick, 4.0)) }),
        };
        let mut plan = TrickPlan {
            cooldown: F(float(&mut pick, 12.0).abs()),
            since: F(float(&mut pick, 12.0).abs()),
            chance: F([0.0, 100.0, 10.0, 50.0][pick.below(4) as usize] + float(&mut pick, 1.0)),
            estimate: F(float(&mut pick, 4.0)),
            slot: pick.below(24) as i32,
            air: F(float(&mut pick, 2.0).abs()),
        };
        let flight = Flight {
            vertical_speed: F(float(&mut pick, 40.0)),
            height: F(float(&mut pick, 60.0)),
            ground: F(float(&mut pick, 20.0)),
            jump,
        };
        let dt = F([1.0 / 30.0, 1.0 / 60.0, 0.0][pick.below(3) as usize]);
        let throwaway = pick.below(SLOTS as u32 + 1) as usize;
        let stunts: Vec<Stunt> = (0..SLOTS)
            .map(|i| Stunt {
                name: if i == throwaway { "throwaway".to_owned() } else { format!("stunt{i}") },
                duration: F(float(&mut pick, 3.0).abs()),
                index: i as i32,
                ..Stunt::default()
            })
            .collect();
        let requested = pick.below(30) as u8;
        let restart = pick.below(2) == 1;
        let armed = pick.below(2) == 1;
        let state = pick.next();

        ee.load(BRAIN, &[0u8; 0x400]);
        ee.load(RIDER, &[0u8; 0x1100]);
        write_stunts(&mut ee, &stunts);
        ee.set_u32(IFACE + 8, vtable);
        ee.set_u32(BRAIN + 0x40, if attached { IFACE } else { 0 });
        ee.write(BRAIN + 0x30, 8, u128::from(flags));
        match jump {
            Some(j) => {
                ee.set_u32(NODE + 4, j.flags);
                ee.set_f32(NODE + 0x48, j.airtime.to_f32());
                ee.set_u32(BRAIN + 0x224, NODE);
            }
            None => ee.set_u32(BRAIN + 0x224, 0),
        }
        ee.set_f32(BRAIN + 0x158, plan.cooldown.to_f32());
        ee.set_f32(BRAIN + 0x15c, plan.since.to_f32());
        ee.set_f32(BRAIN + 0x160, plan.chance.to_f32());
        ee.set_f32(BRAIN + 0x19c, plan.estimate.to_f32());
        ee.set_u32(BRAIN + 0x1a0, plan.slot as u32);
        ee.set_f32(BRAIN + 0x200, plan.air.to_f32());
        ee.set_f32(BRAIN + 0x54, flight.vertical_speed.to_f32());
        ee.set_f32(BRAIN + 0xc4, flight.height.to_f32());
        ee.set_f32(BRAIN + 0x204, flight.ground.to_f32());
        ee.set_u32(RIDER + REQUESTED, u32::from(requested));
        ee.set_u32(RIDER + RESTART, u32::from(restart));
        ee.set_u32(RIDER + ARMED, u32::from(armed));
        ee.write(rand_state, 8, u128::from(state));

        ee.call(plan_fn, &[u64::from(BRAIN)], &[dt.to_f32()], BUDGET);
        assert_eq!(ee.v0(), 1, "case {case}: the decision always returns 1");

        let mut rng = Rng::from_state(state);
        let set = Set::new(stunts.clone());
        let mut tricks = Tricks::default();
        tricks.requested = Trick(requested);
        tricks.restart = restart;
        tricks.gesture.armed = armed;
        let mut got_flags = flags;
        let before = plan;
        plan_trick(dt, &mut got_flags, &mut plan, flight, attached.then_some((&set, &mut tricks)), &mut rng);

        let what = format!("case {case}: flags {flags:#x} {before:?} {flight:?} dt {dt:?} attached {attached}");
        let want = (
            ee.read(BRAIN + 0x30, 8) as u64,
            ee.u32_at(BRAIN + 0x15c),
            ee.u32_at(BRAIN + 0x19c),
            ee.i32_at(BRAIN + 0x1a0),
            ee.u32_at(BRAIN + 0x200),
            ee.u32_at(RIDER + REQUESTED),
            ee.u32_at(RIDER + RESTART) != 0,
            ee.u32_at(RIDER + ARMED) != 0,
            ee.read(rand_state, 8) as u64,
        );
        let got = (
            got_flags,
            plan.since.to_bits(),
            plan.estimate.to_bits(),
            plan.slot,
            plan.air.to_bits(),
            u32::from(tricks.requested.0),
            tricks.restart,
            tricks.gesture.armed,
            rng.state(),
        );
        assert_eq!(got, want, "{what}: (flags, since, estimate, slot, air, requested, restart, armed, rng)");

        // Which way the frame went, so the run can show it reached every one.
        let rolled = rng.state() != state;
        let k = if !rolled && got_flags & 0x8 == 0 && flags & 0x8 != 0 {
            1 // gave up on the cool-down (or a roll it could not make)
        } else if !rolled {
            0
        } else if got_flags & 0x8_0000 != 0 && flags & 0x8_0000 == 0 {
            2 // asked for a trick
        } else if got_flags & 0x8 == 0 {
            3 // the roll failed
        } else {
            4 // the chooser found nothing
        };
        seen[k] += 1;
        if rolled {
            let ballistic = flags & 0x2000 != 0;
            seen[5 + usize::from(ballistic) + usize::from(ballistic && flight.vertical_speed <= F::ZERO)] += 1;
        }
        if flags & 0x20 != 0 && flags & 0x8_0000 != 0 {
            seen[8] += 1; // a tricked flight's landing
        }
        if !attached {
            seen[9] += 1;
        }
    }
    println!(
        "the trick decision: {seen:?} (none, cool-down, asked, failed roll, nothing fits, route, rising, falling, \
         landings, no rider)"
    );
    assert!(seen.iter().all(|&n| n > 0), "a branch was never reached: {seen:?}");
}

/// The "do this trick" call over every slot either side of the 24, against the original's.
#[test]
fn ask_for_matches_every_slot() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_ask_for"]) else { return };
    for slot in -40i32..60 {
        for (restart, armed) in [(false, false), (false, true), (true, false), (true, true)] {
            ee.load(RIDER, &[0u8; 0xb00]);
            ee.set_u32(RIDER + REQUESTED, 7);
            ee.set_u32(RIDER + RESTART, u32::from(restart));
            ee.set_u32(RIDER + ARMED, u32::from(armed));
            ee.call(at[0], &[u64::from(RIDER), slot as i64 as u64], &[], BUDGET);
            let want = (
                ee.v0() != 0,
                ee.u32_at(RIDER + REQUESTED),
                ee.u32_at(RIDER + RESTART) != 0,
                ee.u32_at(RIDER + ARMED) != 0,
            );
            let mut t = Tricks::default();
            t.requested = Trick(7);
            t.restart = restart;
            t.gesture.armed = armed;
            let ok = ask_for(&mut t, slot);
            assert_eq!((ok, u32::from(t.requested.0), t.restart, t.gesture.armed), want, "slot {slot}");
        }
    }
}

/// The AI rider's request gate over every state it reads: whether it holds, or hands over to the pad.
#[test]
fn the_gesture_gate_matches_every_state() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_gesture_gate", "stunt_request"]) else { return };
    let (gate, request) = (at[0], at[1]);
    let asked = Rc::new(RefCell::new(false));
    let a = asked.clone();
    ee.hooks.insert(
        request,
        Box::new(move |ee| {
            *a.borrow_mut() = true;
            ee.set(2, 0);
        }),
    );
    for brain in [0u32, 0x0196_0000] {
        for restart in [0u32, 1, 0x100] {
            for in_progress in [0u32, 1, 0x8000_0000] {
                ee.load(RIDER, &[0u8; 0x1100]);
                ee.set_u32(RIDER + 0x10d0, brain);
                ee.set_u32(RIDER + RESTART, restart);
                ee.set_u32(RIDER + IN_PROGRESS, in_progress);
                *asked.borrow_mut() = false;
                ee.call(gate, &[u64::from(RIDER), 0xff], &[], BUDGET);
                let want = (ee.v0() != 0 && !*asked.borrow(), *asked.borrow());
                let mut t = Tricks::default();
                t.restart = restart != 0;
                t.in_progress = in_progress != 0;
                let holds = gate_holds(brain != 0, &t);
                assert_eq!((holds, !holds), want, "brain {brain:#x} restart {restart} in progress {in_progress}");
            }
        }
    }
}
