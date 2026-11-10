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
    FIRST_AI_IDENTITY, Flight, JUMP, Jump, Rider, RosterIn, Senses, TrickPlan, ask_for, bump, feed, gate_holds,
    plan_trick, spawn_roster,
};
use game_sim::collide::{Bundle, Record};
use game_sim::world::World;
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

// --- avoidance (S12c) -------------------------------------------------------------------------------
//
// The motocross AI's whole avoidance pass runs in the interpreter, ten thousand times. What it calls
// is hooked: the scene walk (a fake vtable that hands out a generated list of riders and placed
// scenery), the feeler sweep against each rider (it answers at random, and writes random hits into
// the feeler record — which the pass then reads and throws away), the scenery groups' part count, and
// the three track-path queries of the off-the-track test (scripted per case). The race's rider list
// is generated around the rider, its own bike included sometimes so the pass has to skip it.
// Compared each time: the steer direction (all four lanes), the aimed-for speed, the flag word, the
// feeler fan when the flags let it be built (and the fan buffer untouched when not), which objects
// were swept, how many scenery groups were taken in, and the point the off-the-track test asked about.

use game_sim::ai::{self, AvoidTuning, Feeler, Me, Other, Steer, flag};
use game_sim::camera::CurveTable;
use ps2_float::Vf;

const AI: u32 = 0x0180_0000;
const OWN: u32 = 0x0180_1000; // its own bike: +0x40, +0x88, +0x8c, +0xbc progress
const RACE: u32 = 0x0180_2000; // +0x34 the rider list, +0x68 the track's path
const SCENE: u32 = 0x0180_3000; // +0x08 its vtable
const SCENE_VT: u32 = 0x0180_3100;
const BUNDLE: u32 = 0x0180_4000; // the feeler bundle: +0x6c record, +0x134/+0x138/+0x13c, +0x150
const FEELER_RECORD: u32 = 0x0180_4400;
const LENGTHS: u32 = 0x0180_4800;
const ORIGINS: u32 = 0x0180_5000;
const DIRS: u32 = 0x0180_6000;
const REC_NORMALS: u32 = 0x0180_7000;
const REC_POINTS: u32 = 0x0180_7800;
const REC_FLAGS: u32 = 0x0180_8000;
const REC_FRACTIONS: u32 = 0x0180_8400;
const ARGS: u32 = 0x0180_9000; // position, heading, steer at +0x00/+0x10/+0x20; speeds at +0x30/+0x34
const IGNORE: u32 = 0x0180_9100;
const WORLD: u32 = 0x0180_a000;
const PATH_AT: u32 = 0x0180_b000; // the located segment, its next (+0x54) and that one's next
const HOOK_BEGIN: u32 = 0x0180_c000;
const HOOK_NEXT: u32 = 0x0180_c008;
const HOOK_END: u32 = 0x0180_c010;
const RIDERS: u32 = 0x0181_0000;
const RIDER_SIZE: u32 = 0x200;
const OBJECTS: u32 = 0x0182_0000;
const MAX_FEELERS: u32 = 16;

impl Pick {
    fn fraction(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.fraction()
    }
    fn chance(&mut self, n: u32) -> bool {
        self.below(n) == 0
    }
    fn vec(&mut self, r: f32, w: f32) -> Vf {
        Vf::new(self.range(-r, r), self.range(-r, r), self.range(-r, r), w)
    }
}

fn put(ee: &mut ps2_oracle::Ee, at: u32, v: Vf) {
    for (k, b) in v.to_bits().iter().enumerate() {
        ee.set_u32(at + 4 * k as u32, *b);
    }
}

fn got(ee: &ps2_oracle::Ee, at: u32) -> [u32; 4] {
    core::array::from_fn(|k| ee.u32_at(at + 4 * k as u32))
}

/// Which object the scene walk hands out: (address, type, collidable).
type SceneObject = (u32, u8, bool);

#[derive(Default)]
struct Script {
    scene: Vec<u32>,
    at: usize,
    opens: bool,
    begun: bool,
    swept: Vec<u32>,
    groups: usize,
    feelers: u32,
    rays: u64,
    holds: Vec<u64>,
    push: Option<Vf>,
    located: bool,
    asked: Option<[u32; 4]>,
}

#[test]
fn avoidance_matches_over_ten_thousand_passes() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&[
        "ai_update_mx",
        "bundle_sweep_object",
        "group_part_count",
        "world_manager",
        "path_locate",
        "path_segment_holds",
        "path_node_direction",
        "curve_table_build",
        "mem_alloc",
    ]) else {
        return;
    };
    let [update, sweep, part_count, world_manager, locate, holds, node_direction, curve_build, mem_alloc] = at[..]
    else {
        unreachable!()
    };
    ee.hooks.insert(mem_alloc, Box::new(|ee| ee.set(2, 0x0100_0000)));
    ee.call(curve_build, &[], &[], 2_000_000_000);
    ee.hooks.remove(&mem_alloc);
    let curve = CurveTable::build();

    let script = Rc::new(RefCell::new(Script::default()));
    let s = script.clone();
    ee.hooks.insert(HOOK_BEGIN, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        s.begun = true;
        ee.set(2, u64::from(s.opens));
    }));
    let s = script.clone();
    ee.hooks.insert(HOOK_NEXT, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        let o = s.scene.get(s.at).copied().unwrap_or(0);
        s.at += 1;
        ee.set(2, u64::from(o));
    }));
    ee.hooks.insert(HOOK_END, Box::new(|_| {}));
    let s = script.clone();
    ee.hooks.insert(sweep, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        s.swept.push(ee.r(5) as u32);
        // A random answer, and random hits for the pass to read and drop.
        let mut x = s.rays.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        for k in 0..s.feelers {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            ee.set_u32(REC_FLAGS + 4 * k, (x & 1) as u32);
            for l in 0..3 {
                ee.set_f32(REC_POINTS + 16 * k + 4 * l, ((x >> (8 + 8 * l)) & 0xff) as f32 - 128.0);
            }
        }
        s.rays = x;
        ee.set(2, x & 2);
    }));
    let s = script.clone();
    ee.hooks.insert(part_count, Box::new(move |ee| {
        s.borrow_mut().groups += 1;
        ee.set(2, 0);
    }));
    let s = script.clone();
    ee.hooks.insert(locate, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        s.asked = Some(got(ee, ee.r(6) as u32));
        let out = ee.r(8) as u32;
        ee.set_u32(out + 4, PATH_AT);
        ee.set(2, u64::from(s.located));
    }));
    let s = script.clone();
    ee.hooks.insert(holds, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        let r = if s.holds.is_empty() { 0 } else { s.holds.remove(0) };
        ee.set(2, r);
    }));
    let s = script.clone();
    ee.hooks.insert(node_direction, Box::new(move |ee| {
        let s = s.borrow();
        if let Some(v) = s.push {
            put(ee, ee.r(6) as u32, v);
        }
        ee.set(2, u64::from(s.push.is_some()));
    }));

    ee.set_u32(world_manager, WORLD);
    ee.set_f32(WORLD + 0xd4, 0.5);
    ee.set_u32(SCENE + 8, SCENE_VT);
    ee.set_u32(SCENE_VT + 0x2c, HOOK_BEGIN);
    ee.set_u32(SCENE_VT + 0x30, HOOK_NEXT);
    ee.set_u32(SCENE_VT + 0x34, HOOK_END);
    ee.set_u32(AI + 0x40, SCENE);
    ee.set_u32(AI + 0xd0, 2);
    ee.set_u32(AI + 0xd4, OWN);
    ee.set_u32(AI + 0x224, 0);
    ee.set_u32(AI + 0x2a0, BUNDLE);
    ee.set_u32(OWN + 0x40, 1);
    ee.set_u32(OWN + 0x88, 1);
    ee.set_u32(OWN + 0x8c, 1);
    ee.set_u32(BUNDLE + 0x6c, FEELER_RECORD);
    ee.set_u32(BUNDLE + 0x134, LENGTHS);
    ee.set_u32(BUNDLE + 0x138, ORIGINS);
    ee.set_u32(BUNDLE + 0x13c, DIRS);
    ee.set_u32(FEELER_RECORD + 0x94, REC_NORMALS);
    ee.set_u32(FEELER_RECORD + 0x98, REC_POINTS);
    ee.set_u32(FEELER_RECORD + 0xa4, REC_FLAGS);
    ee.set_u32(FEELER_RECORD + 0xa8, REC_FRACTIONS);
    ee.set_u32(PATH_AT + 0x54, PATH_AT + 0x100);

    let mut pick = Pick(0x7a11_c0de_5eed_0c12);
    let (mut swerved, mut braked, mut fans, mut edged, mut idle) = (0, 0, 0, 0, 0);
    for case in 0..10_000 {
        let p = &mut pick;
        let defaults = p.chance(4);
        let mut t = AvoidTuning::default();
        if !defaults {
            t = AvoidTuning {
                near_range: F(p.range(2.0, 15.0)),
                far_range: F(p.range(0.0, 45.0)),
                far_radius: F(p.range(1.0, 12.0)),
                far_response: F(p.range(0.0, 1.0)),
                far_approach_limit: F(p.range(0.0, 4.0)),
                far_min_braking_speed: F(p.range(0.0, 40.0)),
                far_braking_response: F(p.range(0.0, 2.0)),
                near_separation: F(p.range(0.0, 1.0)),
                near_response: F(p.range(0.0, 1.0)),
                near_braking_response: F(p.range(0.0, 1.5)),
                passing_speed: F(p.range(-5.0, 15.0)),
                near_min_braking_speed: F(p.range(0.0, 40.0)),
                front_window: F(p.range(-5.0, 20.0)),
                rear_window: F(p.range(-20.0, 5.0)),
                drift_match: F(p.range(-1.0, 1.0)),
                far_heading_cos: F(p.range(-1.0, 1.0)),
                edge_look_ahead: F(p.range(0.0, 20.0)),
                edge_push: F(p.range(0.0, 2.0)),
                feelers: 1 + p.below(MAX_FEELERS),
                feeler_height: F(p.range(0.0, 5.0)),
                feeler_length: F(p.range(10.0, 150.0)),
            };
        }
        let yaw = p.range(-7.0, 7.0);
        let heading = Vf::new(yaw.sin(), p.range(-0.2, 0.2), yaw.cos(), p.range(-1.0, 1.0));
        let me = Me {
            position: p.vec(500.0, 1.0),
            heading,
            velocity: p.vec(30.0, 0.0),
            speed: F(if p.chance(20) { p.range(-5.0, 0.0) } else { p.range(0.01, 60.0) }),
            held_speed: F(p.range(-5.0, 60.0)),
            progress: F(p.range(0.0, 100.0)),
        };
        let mut flags = p.next() & 0xffff_ffff_0000_0000 | p.next() & 0x0000_0000_fffe_00ff;
        for bit in [flag::NO_RIDER_SWEEP, flag::QUIET_AREA, flag::AVOID_DUE, flag::KEEP_ON_TRACK, flag::SKIP_MARKED] {
            flags = if p.chance(2) { flags | bit } else { flags & !bit };
        }
        let reach = if p.chance(3) { 8.0 } else { 40.0 };
        let mut others = Vec::new();
        let mut list = Vec::new();
        let count = p.below(9) as u32;
        let own_at = if p.chance(3) { p.below(count + 1) as u32 } else { u32::MAX };
        for k in 0..count {
            if k == own_at {
                list.push(OWN);
            }
            let o = Other {
                active: !p.chance(8),
                marked: p.chance(3),
                position: Vf::new(
                    me.position.x() + p.range(-reach, reach),
                    me.position.y() + p.range(-3.0, 3.0),
                    me.position.z() + p.range(-reach, reach),
                    p.range(-1.0, 1.0),
                ),
                velocity: if p.chance(4) { Vf::new(0.0, 0.0, 0.0, 0.0) } else { p.vec(30.0, 0.0) },
                heading: p.vec(1.0, 0.0),
                speed: F(if p.chance(4) { 0.0 } else { p.range(0.0, 60.0) }),
                progress: F(p.range(0.0, 100.0)),
            };
            others.push(o);
            list.push(RIDERS + k * RIDER_SIZE);
        }
        if own_at == count {
            list.push(OWN);
        }
        let w = p.range(-1.0, 1.0);
        let steer_in = Steer { direction: p.vec(1.0, w), speed: F(p.range(0.0, 60.0)) };

        // The scene the walk hands out.
        let mut scene: Vec<SceneObject> = Vec::new();
        for k in 0..p.below(6) as u32 {
            let kind = [1u8, 2, 2, 3][p.below(4) as usize];
            scene.push((OBJECTS + k * 0x100, kind, p.chance(2) || kind != 2));
        }
        let ignored: Vec<u32> = scene.iter().filter(|_| p.chance(4)).map(|o| o.0).collect();
        let opens = !p.chance(8);

        // The off-the-track test's answers.
        let located = !p.chance(5);
        let next_next = !p.chance(6);
        let hold = [u64::from(p.chance(3)), u64::from(p.chance(3))];
        let push = (!p.chance(4)).then(|| p.vec(1.0, 0.0));

        // --- into RAM --------------------------------------------------------------------------
        ee.write(AI + 0x30, 8, u128::from(flags));
        put(&mut ee, AI + 0x50, me.velocity);
        ee.set_f32(AI + 0x80, me.held_speed.0);
        ee.set_f32(OWN + 0xbc, me.progress.0);
        ee.set_u32(AI + 0xd8, RACE);
        ee.set_u32(RACE + 0x34, list.first().copied().unwrap_or(0));
        ee.set_u32(RACE + 0x68, 1);
        ee.set_u32(OWN + 4, 1);
        let next_of = |i: usize| list.get(i + 1).copied().unwrap_or(0);
        for (i, &r) in list.iter().enumerate() {
            ee.set_u32(r + 0x1c8, next_of(i));
        }
        for (k, o) in others.iter().enumerate() {
            let r = RIDERS + k as u32 * RIDER_SIZE;
            ee.set_u32(r + 4, u32::from(o.active));
            ee.write(r + 8, 8, if o.marked { 0x8000_0004 } else { 0x8000_0003 });
            put(&mut ee, r + 0x60, o.position);
            put(&mut ee, r + 0x110, o.velocity);
            put(&mut ee, r + 0x120, o.heading);
            ee.set_f32(r + 0x108, o.speed.0);
            ee.set_f32(r + 0xbc, o.progress.0);
        }
        for (f, v) in [
            (0x280, t.near_range),
            (0x284, t.far_range),
            (0x3d8, t.far_radius),
            (0x3dc, t.far_response),
            (0x3e0, t.far_approach_limit),
            (0x3e4, t.far_min_braking_speed),
            (0x3e8, t.far_braking_response),
            (0x3a4, t.near_separation),
            (0x3ac, t.near_response),
            (0x3b0, t.near_braking_response),
            (0x2f8, t.passing_speed),
            (0x3b4, t.near_min_braking_speed),
            (0x3d0, t.front_window),
            (0x3d4, t.rear_window),
            (0x3a8, t.drift_match),
            (0x3ec, t.far_heading_cos),
            (0x288, t.edge_look_ahead),
            (0x3c8, t.edge_push),
            (0x2a8, t.feeler_height),
            (0x2ac, t.feeler_length),
        ] {
            ee.set_f32(AI + f, v.0);
        }
        // The scenery branch's own numbers: read only into what the pass throws away.
        for f in [0x3b8, 0x3bc, 0x3c0, 0x3c4, 0x3cc] {
            ee.set_f32(AI + f, pick.range(0.0, 200.0));
        }
        let p = &mut pick;
        ee.set_u32(AI + 0x2a4, t.feelers as u32);
        for k in 0..MAX_FEELERS {
            put(&mut ee, DIRS + 16 * k, Vf::new(-7.0, -7.0, -7.0, 0.25));
        }
        for &(o, kind, collidable) in &scene {
            ee.write(o + 2, 1, u128::from(kind));
            ee.write(o + 0x31, 1, u128::from(collidable));
            ee.write(o + 8, 2, p.below(4000) as u128);
            ee.write(o + 0xc, 2, p.below(4000) as u128);
        }
        for (k, &o) in ignored.iter().enumerate() {
            ee.set_u32(IGNORE + 4 * k as u32, o);
        }
        ee.set_u32(PATH_AT + 0x154, u32::from(next_next) * (PATH_AT + 0x200));
        put(&mut ee, ARGS, me.position);
        put(&mut ee, ARGS + 0x10, me.heading);
        put(&mut ee, ARGS + 0x20, steer_in.direction);
        ee.set_f32(ARGS + 0x30, me.speed.0);
        ee.set_f32(ARGS + 0x34, steer_in.speed.0);
        *script.borrow_mut() = Script {
            scene: scene.iter().map(|o| o.0).collect(),
            opens,
            feelers: t.feelers as u32,
            holds: hold.to_vec(),
            push,
            located,
            ..Script::default()
        };

        ee.call(
            update,
            &[
                u64::from(AI),
                ignored.len() as u64,
                u64::from(IGNORE),
                u64::from(ARGS),
                u64::from(ARGS + 0x10),
                u64::from(ARGS + 0x20),
                u64::from(ARGS + 0x30),
                u64::from(ARGS + 0x34),
            ],
            &[],
            BUDGET,
        );

        // --- the port ----------------------------------------------------------------------------
        let mut port_flags = flags;
        let mut steer = steer_in;
        let mut asked = None;
        let mut edge = |ahead: Vf| {
            asked = Some(ahead.to_bits());
            let off = located && next_next && hold == [0, 0];
            off.then(|| push.unwrap_or(ahead))
        };
        let fan = ai::avoid(&t, &me, &mut port_flags, &others, Some(&mut edge), &mut steer, &curve);
        let look = ai::look(flags);
        let what = format!("case {case}: flags {flags:#x}, {} riders, scene {scene:?}", others.len());

        assert_eq!(got(&ee, ARGS + 0x20), steer.direction.to_bits(), "{what}: the steer direction");
        assert_eq!(ee.u32_at(ARGS + 0x34), steer.speed.to_bits(), "{what}: the aimed-for speed");
        assert_eq!(ee.read(AI + 0x30, 8) as u64, port_flags, "{what}: the flags");
        let s = script.borrow();
        let ran = me.speed > F(0.0);
        let walked = ran && look.fan;
        assert_eq!(s.begun, walked, "{what}: the scene walk");
        match &fan {
            Some(fan) => {
                fans += 1;
                for (k, Feeler { origin, direction, length }) in fan.iter().enumerate() {
                    let k = k as u32;
                    assert_eq!(got(&ee, DIRS + 16 * k)[..3], direction.to_bits()[..3], "{what}: feeler {k}'s direction");
                    assert_eq!(got(&ee, ORIGINS + 16 * k), origin.to_bits(), "{what}: feeler {k}'s origin");
                    assert_eq!(ee.u32_at(LENGTHS + 4 * k), length.to_bits(), "{what}: feeler {k}'s length");
                }
                assert_eq!(got(&ee, BUNDLE + 0x150), me.position.to_bits(), "{what}: the bundle's centre");
            }
            None => {
                for k in 0..MAX_FEELERS {
                    assert_eq!(got(&ee, DIRS + 16 * k), Vf::new(-7.0, -7.0, -7.0, 0.25).to_bits(), "{what}: a fan built");
                }
            }
        }
        let walked = walked && opens;
        let want_swept: Vec<u32> = if walked && look.riders {
            scene.iter().filter(|o| o.1 == 2 && o.2 && !ignored.contains(&o.0)).map(|o| o.0).collect()
        } else {
            Vec::new()
        };
        assert_eq!(s.swept, want_swept, "{what}: the riders swept");
        let want_groups = if walked && look.scenery { scene.iter().filter(|o| o.1 == 3).count() } else { 0 };
        assert_eq!(s.groups, want_groups, "{what}: the scenery taken in");
        let edge_on = ran && flags & flag::KEEP_ON_TRACK != 0;
        assert_eq!(s.asked, if edge_on { asked } else { None }, "{what}: the point ahead");
        drop(s);

        idle += usize::from(!ran);
        edged += usize::from(edge_on && asked.is_some() && located && next_next && hold == [0, 0]);
        braked += usize::from(steer.speed != steer_in.speed);
        swerved += usize::from(ran && others.iter().any(|o| o.active));
    }
    eprintln!("ai avoidance: {fans} fans, {braked} brakes, {edged} pushed back, {swerved} with riders, {idle} idle");
    assert!(fans > 3000 && braked > 500 && edged > 300 && idle > 200, "fans {fans}, brakes {braked}, edges {edged}, idle {idle}");
}

// ---------------------------------------------------------------------------------------------------
// S12e — the rider-to-brain copy
// ---------------------------------------------------------------------------------------------------

/// The rider, its bike's frame object (rider +0x18, rows at +0x80), its body (rider +0x7a4) and its
/// brain (rider +0x10d0), in RAM nothing else uses.
const FEED_RIDER: u32 = 0x0196_0000;
const FEED_FRAME: u32 = 0x0198_0000;
const FEED_BODY: u32 = 0x0198_1000;
const FEED_BRAIN: u32 = 0x0198_2000;
/// Never read: the ground query is hooked.
const FEED_WORLD: u32 = 0x0198_3000;

/// The ground both sides are asked: one answer per query in turn, and the points they were asked at.
#[derive(Default)]
struct Answers {
    heights: Vec<f32>,
    normals: Vec<Vf>,
    asked: Vec<[u32; 4]>,
}

/// The port's side of [`Answers`].
struct Asked(Answers);

impl World for Asked {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let k = self.0.asked.len();
        self.0.asked.push(at.to_bits());
        (Vf::new(at.x(), self.0.heights[k], at.z(), at.w()), self.0.normals[k])
    }
    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// The rider-to-brain copy against the port over ten thousand frames: the original `ai_feed` with
/// `ground_query` hooked to answer generated grounds (the query itself is S64's) and the original's
/// `rand()`. Every brain field it writes is compared bit for bit, with the generator's state, and the
/// points the ground was asked at.
#[test]
fn the_rider_to_brain_copy_matches_over_ten_thousand_frames() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_feed", "ground_query", "rand_state"]) else { return };
    let [feed_fn, query, rand_state] = at[..] else { unreachable!() };

    let script = Rc::new(RefCell::new(Answers::default()));
    let s = script.clone();
    ee.hooks.insert(
        query,
        Box::new(move |ee| {
            let mut s = s.borrow_mut();
            let (point, normal) = (ee.r(5) as u32, ee.r(6) as u32);
            let k = s.asked.len();
            s.asked.push(got(ee, point));
            ee.set_f32(point + 4, s.heights[k]);
            if normal != 0 {
                put(ee, normal, s.normals[k]);
            }
            ee.set(2, 1);
        }),
    );

    let mut pick = Pick(0x51e2_feed_0b0d_ca7e);
    // on the ground; rising past 5; forward past 30; the roll passes; the no-brain case is not the port's
    let mut seen = [0u32; 6];
    for case in 0..10_000 {
        let frame: [Vf; 4] = core::array::from_fn(|_| {
            let w = float(&mut pick, 2.0);
            pick.vec(400.0, w)
        });
        let w = float(&mut pick, 2.0);
        let mut velocity = pick.vec(60.0, w);
        if pick.chance(3) {
            let y = [5.0, -5.0, 5.000_001, 4.999_999, 0.0, -0.0][pick.below(6) as usize];
            velocity = Vf::new(velocity.x(), y, velocity.z(), velocity.w());
        } else if pick.chance(2) {
            velocity = Vf::new(velocity.x(), pick.range(4.0, 30.0), velocity.z(), velocity.w());
        }
        let w = float(&mut pick, 2.0);
        let acceleration = pick.vec(100.0, w);
        let w = float(&mut pick, 2.0);
        let spin = pick.vec(8.0, w);
        let forward_speed = match pick.below(4) {
            0 => [30.0, 30.000_002, 29.999_998, -30.0][pick.below(4) as usize],
            1 => float(&mut pick, 80.0),
            _ => pick.range(20.0, 80.0),
        };
        let rider = Rider {
            frame,
            velocity,
            acceleration,
            spin,
            forward_speed: F(forward_speed),
            on_ground: pick.below(3) != 0,
            tricking: pick.chance(3),
            look_ahead: F(float(&mut pick, 1.0)),
            bump_chance: F([0.0, 100.0, 10.0, 50.0][pick.below(4) as usize] + float(&mut pick, 1.0)),
        };
        let mut brain = Senses {
            last_position: pick.vec(400.0, 1.0),
            frame: core::array::from_fn(|_| pick.vec(400.0, 1.0)),
            velocity: pick.vec(60.0, 0.0),
            acceleration: pick.vec(60.0, 0.0),
            spin: pick.vec(8.0, 0.0),
            speed: F(pick.range(0.0, 60.0)),
            ground_normal: pick.vec(1.0, 0.0),
            ground: F(pick.range(-50.0, 50.0)),
            flags: pick.next(),
            bumps: pick.next(),
        };
        let heights = vec![pick.range(-400.0, 400.0), float(&mut pick, 400.0)];
        let w = float(&mut pick, 1.0);
        let normals = vec![pick.vec(1.0, w), pick.vec(1.0, 0.0)];
        let state = pick.next();

        ee.load(FEED_RIDER, &[0u8; 0x1200]);
        ee.load(FEED_BRAIN, &[0u8; 0x400]);
        ee.load(FEED_FRAME, &[0u8; 0x100]);
        ee.load(FEED_BODY, &[0u8; 0x300]);
        ee.set_u32(FEED_RIDER + 0x18, FEED_FRAME);
        ee.set_u32(FEED_RIDER + 0x7a4, FEED_BODY);
        ee.set_u32(FEED_RIDER + 0x7ac, FEED_WORLD);
        ee.set_u32(FEED_RIDER + 0x10d0, FEED_BRAIN);
        ee.set_u32(FEED_RIDER + 0x574, u32::from(rider.on_ground));
        ee.set_u32(FEED_RIDER + IN_PROGRESS, u32::from(rider.tricking));
        ee.set_f32(FEED_RIDER + 0x11d0, rider.look_ahead.to_f32());
        ee.set_f32(FEED_RIDER + 0x1100, rider.bump_chance.to_f32());
        for (k, row) in rider.frame.iter().enumerate() {
            put(&mut ee, FEED_FRAME + 0x80 + 0x10 * k as u32, *row);
        }
        put(&mut ee, FEED_BODY + 0x260, rider.velocity);
        put(&mut ee, FEED_BODY + 0x70, rider.acceleration);
        put(&mut ee, FEED_BODY + 0x290, rider.spin);
        ee.set_f32(FEED_BODY + 0x278, forward_speed);
        put(&mut ee, FEED_BRAIN + 0x290, brain.last_position);
        for (k, row) in brain.frame.iter().enumerate() {
            put(&mut ee, FEED_BRAIN + 0x90 + 0x10 * k as u32, *row);
        }
        put(&mut ee, FEED_BRAIN + 0x50, brain.velocity);
        put(&mut ee, FEED_BRAIN + 0x60, brain.acceleration);
        put(&mut ee, FEED_BRAIN + 0x70, brain.spin);
        ee.set_f32(FEED_BRAIN + 0x80, brain.speed.to_f32());
        put(&mut ee, FEED_BRAIN + 0x2c0, brain.ground_normal);
        ee.set_f32(FEED_BRAIN + 0x204, brain.ground.to_f32());
        ee.write(FEED_BRAIN + 0x30, 8, u128::from(brain.flags));
        ee.write(FEED_BRAIN + 0x38, 8, u128::from(brain.bumps));
        ee.write(rand_state, 8, u128::from(state));
        *script.borrow_mut() = Answers { heights: heights.clone(), normals: normals.clone(), asked: Vec::new() };

        ee.call(feed_fn, &[u64::from(FEED_RIDER)], &[], BUDGET);
        assert_eq!(ee.v0(), 1, "case {case}: the copy returns 1 with a brain");

        let mut rng = Rng::from_state(state);
        let mut world = Asked(Answers { heights, normals, asked: Vec::new() });
        feed(&rider, &mut brain, &mut world, &mut rng);

        let theirs = script.borrow();
        assert_eq!(theirs.asked.len(), 2, "case {case}: the original asks the ground twice");
        assert_eq!(world.0.asked.len(), 2, "case {case}: the port asks the ground twice");
        // The first point's w is the original's stack, never set.
        assert_eq!(theirs.asked[0][..3], world.0.asked[0][..3], "case {case}: the look-ahead point");
        assert_eq!(theirs.asked[1], world.0.asked[1], "case {case}: the point under the bike");
        let quads = [
            ("last position", 0x290, brain.last_position),
            ("side", 0x90, brain.frame[0]),
            ("up", 0xa0, brain.frame[1]),
            ("forward", 0xb0, brain.frame[2]),
            ("position", 0xc0, brain.frame[3]),
            ("velocity", 0x50, brain.velocity),
            ("acceleration", 0x60, brain.acceleration),
            ("spin", 0x70, brain.spin),
            ("ground normal", 0x2c0, brain.ground_normal),
        ];
        for (name, off, ours) in quads {
            assert_eq!(got(&ee, FEED_BRAIN + off), ours.to_bits(), "case {case}: {name}");
        }
        assert_eq!(ee.u32_at(FEED_BRAIN + 0x80), brain.speed.to_f32().to_bits(), "case {case}: speed");
        assert_eq!(ee.u32_at(FEED_BRAIN + 0x204), brain.ground.to_f32().to_bits(), "case {case}: ground");
        assert_eq!(ee.read(FEED_BRAIN + 0x30, 8) as u64, brain.flags, "case {case}: flags");
        assert_eq!(ee.read(FEED_BRAIN + 0x38, 8) as u64, brain.bumps, "case {case}: bump flags");
        assert_eq!(ee.read(rand_state, 8) as u64, rng.state(), "case {case}: the generator");

        let v = rider.velocity;
        let rising = !(v.y() <= 5.0);
        let fast = !(forward_speed <= 30.0);
        seen[0] += u32::from(rider.on_ground);
        seen[1] += u32::from(rider.on_ground && rising);
        seen[2] += u32::from(rider.on_ground && rising && fast);
        seen[3] += u32::from(brain.bumps & (bump::A | bump::B) != 0);
        seen[4] += u32::from(rider.on_ground && rising && fast && brain.bumps & bump::A == 0);
        seen[5] += u32::from(rider.tricking);
    }
    println!(
        "on the ground {}, rising {}, rolled {}, bumped {}, roll failed {}, tricking {}",
        seen[0], seen[1], seen[2], seen[3], seen[4], seen[5]
    );
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 100, "branch {k} reached only {n} times: {seen:?}");
    }

    // No brain attached: nothing is copied and nothing is drawn.
    ee.set_u32(FEED_RIDER + 0x10d0, 0);
    ee.write(rand_state, 8, 0x1234);
    script.borrow_mut().asked.clear();
    ee.call(feed_fn, &[u64::from(FEED_RIDER)], &[], BUDGET);
    assert_eq!(ee.v0(), 0);
    assert_eq!(ee.read(rand_state, 8), 0x1234);
    assert!(script.borrow().asked.is_empty());
}

// ---------------------------------------------------------------------------------------------------
// S282 — the track path's queries, and the avoidance's keep-on-the-track push on them
// ---------------------------------------------------------------------------------------------------
//
// Generated paths are laid into RAM as the original keeps them: a node every 0x60 bytes with its
// centre at +0x10, its two edge points at +0x20/+0x30, the length on at +0x40 and the next node at
// +0x54, the words between filled with junk the queries must not read; the path object (the rider's
// own +0x88) holds the first node at +0x10 and no branches.

use game_sim::ai::path::{self, Net, Node};

const NET_NODES: u32 = 0x019a_0000;
const NODE_SIZE: u32 = 0x60;
const PATH_OBJ: u32 = 0x019b_0000;
const PATH_OUT: u32 = 0x019b_1000; // locate's place: +0 the path, +4 the node, +8 how far along
const PATH_DIST: u32 = 0x019b_1100;
const PATH_POINT: u32 = 0x019b_1200;
const PATH_IO: u32 = 0x019b_1300;
/// What `locate` leaves in a word it does not write.
const UNTOUCHED: u32 = 0x5eed_f00d;

fn node_at(i: usize) -> u32 {
    NET_NODES + i as u32 * NODE_SIZE
}

fn index_of(a: u32) -> Option<usize> {
    (a != 0).then(|| ((a - NET_NODES) / NODE_SIZE) as usize)
}

fn write_net(ee: &mut ps2_oracle::Ee, net: &Net, junk: &mut Pick) {
    for (i, n) in net.nodes.iter().enumerate() {
        let a = node_at(i);
        for o in (0..NODE_SIZE).step_by(4) {
            ee.set_u32(a + o, junk.next() as u32);
        }
        put(ee, a + 0x10, n.centre);
        put(ee, a + 0x20, n.edges[0]);
        put(ee, a + 0x30, n.edges[1]);
        ee.set_f32(a + 0x40, n.length.0);
        ee.set_u32(a + 0x54, n.next.map_or(0, node_at));
    }
}

/// A coordinate, now and then snapped to a whole number so sides line up and points land on them.
fn coord(p: &mut Pick, lo: f32, hi: f32, snap: bool) -> f32 {
    let x = p.range(lo, hi);
    if snap { x.round() } else { x }
}

/// A chain of one to twelve cross-sections wandering across the ground, most a true length apart,
/// some with a length that is nothing, negative or wrong; now and then on whole numbers, with edge
/// points out of order or folded back onto the centre.
fn gen_net(p: &mut Pick) -> Net {
    let count = 1 + p.below(12) as usize;
    let snap = p.chance(4);
    let mut nodes: Vec<Node> = Vec::new();
    let (mut x, mut z, mut yaw) = (coord(p, -60.0, 60.0, snap), coord(p, -60.0, 60.0, snap), p.range(-7.0, 7.0));
    for i in 0..count {
        let w = p.range(2.0, 15.0);
        let (s, c) = (yaw.sin(), yaw.cos());
        let centre = Vf::new(x, p.range(-20.0, 20.0), z, p.range(-1.0, 1.0));
        let mut side = |k: f32| {
            let jx = if p.chance(3) { 0.0 } else { p.range(-3.0, 3.0) };
            let jz = if p.chance(3) { 0.0 } else { p.range(-3.0, 3.0) };
            let e = Vf::new(x + k * c * w + jx, p.range(-20.0, 20.0), z - k * s * w + jz, p.range(-1.0, 1.0));
            if snap { Vf::new(e.x().round(), e.y(), e.z().round(), e.w()) } else { e }
        };
        let mut edges = [side(1.0), side(-1.0)];
        if p.chance(8) {
            edges.swap(0, 1);
        }
        if p.chance(16) {
            edges[p.below(2) as usize] = centre;
        }
        nodes.push(Node { centre, edges, length: F(0.0), next: (i + 1 < count).then_some(i + 1) });
        if p.chance(6) {
            yaw += p.range(-1.5, 1.5);
        }
        let step = if p.chance(12) { 0.0 } else { p.range(2.0, 30.0) };
        x += yaw.sin() * step;
        z += yaw.cos() * step;
        if snap {
            (x, z) = (x.round(), z.round());
        }
    }
    for i in 0..count {
        let length = match nodes[i].next {
            Some(j) => {
                let (a, b) = (nodes[i].centre, nodes[j].centre);
                let true_len = ((b.x() - a.x()).powi(2) + (b.z() - a.z()).powi(2)).sqrt();
                match p.below(10) {
                    0 => 0.0,
                    1 => -p.range(0.0, 10.0),
                    2 => true_len * p.range(0.5, 2.0),
                    _ => true_len,
                }
            }
            None => p.range(-5.0, 30.0),
        };
        nodes[i].length = F(length);
    }
    Net { nodes }
}

/// A point near the net: on a node or edge point, between two, or anywhere around.
fn near(p: &mut Pick, net: &Net) -> Vf {
    let n = &net.nodes[p.below(net.nodes.len() as u32) as usize];
    let base = match p.below(4) {
        0 => n.centre,
        1 => n.edges[p.below(2) as usize],
        _ => Vf::new(
            n.centre.x() + p.range(-25.0, 25.0),
            n.centre.y(),
            n.centre.z() + p.range(-25.0, 25.0),
            p.range(-1.0, 1.0),
        ),
    };
    if p.chance(4) { base } else { Vf::new(base.x() + p.range(-2.0, 2.0), base.y(), base.z() + p.range(-2.0, 2.0), base.w()) }
}

/// The three path queries against the original over ten thousand generated paths: `path_locate`
/// (no hint, with and without a distance limit, the holding-only filter and the distance out),
/// `path_segment_holds` on a stretch and on any two nodes, and `path_node_direction` on any node,
/// none, or the last. Every word each one writes is compared, and the words it must not write.
#[test]
fn the_path_queries_match_over_ten_thousand_paths() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["path_locate", "path_segment_holds", "path_node_direction"]) else { return };
    let [locate, holds, direction] = at[..] else { unreachable!() };
    let mut pick = Pick(0x5282_9a7b_0c1d_e2f3);
    let mut junk = Pick(0x0bad_cafe_1234_5678);
    let mut seen = [0u32; 8];
    for case in 0..10_000 {
        let p = &mut pick;
        let net = gen_net(p);
        write_net(&mut ee, &net, &mut junk);
        let count = net.nodes.len();
        let first = p.below(count as u32) as usize;
        ee.set_u32(PATH_OBJ, junk.next() as u32);
        ee.set_u32(PATH_OBJ + 0x10, node_at(first));
        ee.set_u32(PATH_OBJ + 0x18, 0);
        let point = near(p, &net);
        put(&mut ee, PATH_POINT, point);
        let what = format!("case {case}: {count} nodes from {first}, point {point:?}");

        // locate
        let limit = match p.below(3) {
            0 => p.range(0.0, 120.0),
            1 => -p.range(0.0, 5.0),
            _ => -1.0,
        };
        let holding_only = p.chance(3);
        let with_distance = !p.chance(5);
        for o in [0, 4, 8] {
            ee.set_u32(PATH_OUT + o, UNTOUCHED);
        }
        ee.set_u32(PATH_DIST, UNTOUCHED);
        ee.call(
            locate,
            &[
                0,
                0,
                u64::from(PATH_POINT),
                u64::from(PATH_OBJ),
                u64::from(PATH_OUT),
                if with_distance { u64::from(PATH_DIST) } else { 0 },
                0,
                u64::from(holding_only),
            ],
            &[limit],
            BUDGET,
        );
        let ours = path::locate(&net, first, point, F(limit), holding_only);
        let w = format!("{what}, limit {limit}, holding only {holding_only}: {ours:?}");
        assert_eq!(ee.v0(), 1, "{w}: locate's answer");
        assert_eq!(ee.u32_at(PATH_OUT), PATH_OBJ, "{w}: the place's path");
        match ours.place {
            Some(pl) => {
                assert_eq!(index_of(ee.u32_at(PATH_OUT + 4)), Some(pl.node), "{w}: the node");
                assert_eq!(ee.u32_at(PATH_OUT + 8), pl.along.to_bits(), "{w}: how far along");
                seen[0] += u32::from(pl.along > F(0.0) && pl.along < F(1.0));
            }
            None => {
                assert_eq!(ee.u32_at(PATH_OUT + 4), UNTOUCHED, "{w}: a node written");
                assert_eq!(ee.u32_at(PATH_OUT + 8), UNTOUCHED, "{w}: a fraction written");
                seen[1] += 1;
            }
        }
        let want_dist = if with_distance { ours.distance.to_bits() } else { UNTOUCHED };
        assert_eq!(ee.u32_at(PATH_DIST), want_dist, "{w}: the distance");
        seen[2] += u32::from(limit >= 0.0 && ours.place.is_some_and(|pl| pl.node != first));

        // holds, on a stretch or any two nodes
        let a = p.below(count as u32) as usize;
        let b = match net.nodes[a].next {
            Some(j) if !p.chance(4) => j,
            _ => p.below(count as u32) as usize,
        };
        ee.call(holds, &[0, u64::from(PATH_POINT), u64::from(node_at(a)), u64::from(node_at(b))], &[], BUDGET);
        let held = path::segment_holds(&net, point, a, b);
        assert_eq!(ee.v0() != 0, held, "{what}: holds {a}..{b}");
        assert!(ee.v0() <= 1, "{what}: holds answered {}", ee.v0());
        seen[3] += u32::from(held);

        // the direction at a node
        let node = if p.chance(10) { None } else { Some(p.below(count as u32) as usize) };
        let io_w = p.range(-1.0, 1.0);
        let io_in = p.vec(5.0, io_w);
        put(&mut ee, PATH_IO, io_in);
        ee.call(direction, &[0, node.map_or(0, |i| u64::from(node_at(i))), u64::from(PATH_IO)], &[], BUDGET);
        let mut io = io_in;
        let gave = path::node_direction(&net, node, &mut io);
        assert_eq!(ee.v0() != 0, gave, "{what}: direction at {node:?}");
        assert_eq!(got(&ee, PATH_IO), io.to_bits(), "{what}: the direction at {node:?}");
        seen[4] += u32::from(gave);
        seen[5] += u32::from(!gave && node.is_some());
        seen[6] += u32::from(count == 1);
        seen[7] += u32::from(holding_only && ours.place.is_some());
    }
    eprintln!(
        "path queries: {} inside a stretch, {} unplaced, {} limited, {} held, {} directions, {} refused, {} lone, {} held places",
        seen[0], seen[1], seen[2], seen[3], seen[4], seen[5], seen[6], seen[7]
    );
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 100, "branch {k} reached only {n} times: {seen:?}");
    }
}

/// The keep-on-the-track push end to end: the original's motocross avoidance pass (`ai_update_mx`)
/// on generated paths with the three path queries running as the original's own code, against
/// [`ai::avoid`] with [`path::off_track`] as its edge. No other riders and no feeler fan (flags 0x80
/// and 0x40 both up), so the only thing bending the steer is the push. Ten thousand passes.
#[test]
fn the_keep_on_track_push_matches_over_ten_thousand_passes() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_update_mx", "curve_table_build", "mem_alloc"]) else { return };
    let [update, curve_build, mem_alloc] = at[..] else { unreachable!() };
    ee.hooks.insert(mem_alloc, Box::new(|ee| ee.set(2, 0x0100_0000)));
    ee.call(curve_build, &[], &[], 2_000_000_000);
    ee.hooks.remove(&mem_alloc);
    let curve = CurveTable::build();
    // The feeler bundle the pass keeps (it would allocate one were there none), never filled here.
    ee.set_u32(AI + 0x2a0, BUNDLE);
    ee.set_u32(BUNDLE + 0x6c, FEELER_RECORD);
    ee.set_u32(BUNDLE + 0x134, LENGTHS);
    ee.set_u32(BUNDLE + 0x138, ORIGINS);
    ee.set_u32(BUNDLE + 0x13c, DIRS);
    ee.hooks.insert(mem_alloc, Box::new(|_| panic!("the pass allocated")));
    let mut pick = Pick(0x0ff7_7ac4_5282_0001);
    let mut junk = Pick(0x7e57_0000_aaaa_5555);
    ee.set_u32(AI + 0x40, SCENE);
    ee.set_u32(AI + 0xd0, 2);
    ee.set_u32(AI + 0xd4, OWN);
    ee.set_u32(AI + 0xd8, RACE);
    ee.set_u32(OWN + 0x40, 1);
    ee.set_u32(RACE + 0x34, 0);
    ee.set_u32(RACE + 0x68, PATH_OBJ + 0x800);
    let (mut pushed, mut held, mut short, mut no_path, mut flat, mut lengthless) = (0, 0, 0, 0, 0, 0);
    for case in 0..10_000 {
        let p = &mut pick;
        let net = gen_net(p);
        write_net(&mut ee, &net, &mut junk);
        let count = net.nodes.len();
        let first = if p.chance(20) { None } else { Some(p.below(count as u32) as usize) };
        let node = if p.chance(10) { None } else { Some(p.below(count as u32) as usize) };
        ee.set_u32(PATH_OBJ, junk.next() as u32);
        ee.set_u32(PATH_OBJ + 0x10, first.map_or(0, node_at));
        ee.set_u32(PATH_OBJ + 0x18, 0);
        ee.set_u32(OWN + 0x88, if first.is_some() { PATH_OBJ } else { 0 });
        ee.set_u32(OWN + 0x8c, node.map_or(0, node_at));

        let t = AvoidTuning { edge_look_ahead: F(p.range(0.0, 20.0)), edge_push: F(p.range(0.0, 2.0)), ..AvoidTuning::default() };
        let yaw = p.range(-7.0, 7.0);
        let me = Me {
            position: near(p, &net),
            heading: Vf::new(yaw.sin(), p.range(-0.2, 0.2), yaw.cos(), p.range(-1.0, 1.0)),
            velocity: p.vec(30.0, 0.0),
            speed: F(p.range(0.01, 60.0)),
            held_speed: F(p.range(-5.0, 60.0)),
            progress: F(p.range(0.0, 100.0)),
        };
        let flags = (p.next() & 0xffff_ffff_0000_0000 | p.next() & 0x0000_0000_fffe_00ff) & !(flag::SKIP_MARKED | flag::AVOID_DUE)
            | flag::NO_RIDER_SWEEP
            | flag::QUIET_AREA
            | flag::KEEP_ON_TRACK
            | if p.chance(2) { flag::AVOID_DUE } else { 0 };
        let steer_w = p.range(-1.0, 1.0);
        let steer_in = Steer { direction: p.vec(1.0, steer_w), speed: F(p.range(0.0, 60.0)) };

        ee.write(AI + 0x30, 8, u128::from(flags));
        put(&mut ee, AI + 0x50, me.velocity);
        ee.set_f32(AI + 0x80, me.held_speed.0);
        ee.set_f32(OWN + 0xbc, me.progress.0);
        ee.set_f32(AI + 0x288, t.edge_look_ahead.0);
        ee.set_f32(AI + 0x3c8, t.edge_push.0);
        put(&mut ee, ARGS, me.position);
        put(&mut ee, ARGS + 0x10, me.heading);
        put(&mut ee, ARGS + 0x20, steer_in.direction);
        ee.set_f32(ARGS + 0x30, me.speed.0);
        ee.set_f32(ARGS + 0x34, steer_in.speed.0);
        ee.call(
            update,
            &[
                u64::from(AI),
                0,
                u64::from(IGNORE),
                u64::from(ARGS),
                u64::from(ARGS + 0x10),
                u64::from(ARGS + 0x20),
                u64::from(ARGS + 0x30),
                u64::from(ARGS + 0x34),
            ],
            &[],
            BUDGET,
        );

        let mut port_flags = flags;
        let mut steer = steer_in;
        let mut answer = None;
        let mut edge = |ahead: Vf| {
            let a = path::off_track(&net, first.unwrap(), node.unwrap(), ahead);
            answer = Some(a);
            a
        };
        let edge: Option<&mut dyn FnMut(Vf) -> Option<Vf>> =
            if first.is_some() && node.is_some() { Some(&mut edge) } else { None };
        let fan = ai::avoid(&t, &me, &mut port_flags, &[], edge, &mut steer, &curve);
        let what = format!("case {case}: {count} nodes, first {first:?}, node {node:?}, answer {answer:?}");
        assert!(fan.is_none(), "{what}: a fan built");
        assert_eq!(got(&ee, ARGS + 0x20), steer.direction.to_bits(), "{what}: the steer direction");
        assert_eq!(ee.u32_at(ARGS + 0x34), steer.speed.to_bits(), "{what}: the aimed-for speed");
        assert_eq!(ee.read(AI + 0x30, 8) as u64, port_flags, "{what}: the flags");
        match answer {
            Some(Some(q)) => {
                let across = me.heading.x() * q.z() - me.heading.z() * q.x();
                if across > 0.0 { pushed += 1 } else { flat += 1 }
                lengthless += u32::from(q == ahead_of(&me, &t));
            }
            None => no_path += 1,
            Some(None) if count < 3 => short += 1,
            Some(None) => held += 1,
        }
    }
    eprintln!(
        "keep on track: {pushed} pushed, {flat} pointed away, {held} on the track, {short} short paths, \
         {no_path} no path or node, {lengthless} pushed along the point itself"
    );
    assert!(pushed > 1000 && flat > 1000 && held > 1000 && short > 100 && no_path > 100 && lengthless > 100);
}

/// The point the pass looks at, the way it builds it (`vmulx.xyz` then `vadd.xyz`).
fn ahead_of(me: &Me, t: &AvoidTuning) -> Vf {
    let s = me.heading.mul(Vf::new(t.edge_look_ahead.0, 0.0, 0.0, 0.0).bc(ps2_float::Field::X));
    me.position.merge(ps2_float::Dest::XYZ, me.position.add(s))
}

// --- the character file (S283) ----------------------------------------------------------------------
//
// The whole of the original's character reader runs in the interpreter, the engine's own ini reader
// included: it opens the file through the four file calls (served from a generated text or the
// retail file), parses it, selects its sections and runs strtod and the double-to-float on every
// value. Hooked: the heap (a bump allocator), the C library's lock and memset, and sbrk. Compared: every
// word of the brain's and the rider's objects (the fields the reader writes, as the port computes
// them, and every other word untouched), the generator's state, and the file name it opened.

use game_sim::ai::{Character, Pair, Profile, Spread, Triple, read_character};
use game_sim::phy::Ini;

const CH_BRAIN: u32 = 0x0100_0000;
const CH_BRAIN_LEN: u32 = 0x400;
const CH_RIDER: u32 = 0x0101_0000;
const CH_RIDER_LEN: u32 = 0x200;
const CH_APP: u32 = 0x0102_0000;
const CH_SETTINGS: u32 = 0x0102_1000;
const CH_EVENT: u32 = 0x0102_2000;
const CH_HEAP: u32 = 0x0120_0000;
const CH_HEAP_END: u32 = 0x01a0_0000;
const CH_BREAK: u32 = 0x01a0_0000;
const CH_BREAK_END: u32 = 0x01f0_0000;
/// The folder the game has put in the reader's path prefix by the time it runs.
const CH_PREFIX: &str = "Rider\\";
const CH_FILE: &str = "Rider\\AIChrctr.aip";

/// The scalar keys of an `AI_<n>` section, and the two quadword ones.
const CH_KEYS: [&str; 87] = [
    "SteeringGain", "MinRandomOffset", "MaxRandomOffset", "MaxDriftSize", "MaxDesiredOffsetDriftRate",
    "NoTrackOffsetLookAhead", "NoTrackOffsetMinTurnDP", "MaxLookAheadDistance", "MinLookAheadDistance",
    "LookAheadGain", "OversteerCorrectionGain", "SpeedMinTolerance", "SpeedMaxTolerance",
    "MinNearCollisionAvoidanceResponseScale", "MaxNearCollisionAvoidanceResponseScale",
    "OverDriveNearCollisionAvoidanceResponseScale", "NearCollisionRange", "FarCollisionRange",
    "FarCollisionAvoidanceRadius", "FarCollisionAvoidanceResponseScale", "FarCollisionBrakingApproachRateLimit",
    "FarCollisionMinBrakingSpeed", "FarCollisionBrakingResponseScale", "NearCollisionSeperationRate",
    "NearCollisionBrakingResponse", "PassingSpeed", "NearCollisionMinBrakingSpeed", "FrontLateralAvoidanceWindow",
    "RearLateralAvoidanceWindow", "RollGain", "RollControlDistance", "PitchControlDistance", "PitchGain",
    "PitchJitterDeadZone", "MinimumStuntInterval", "StuntProbability", "MinPreloadSuccessProbability",
    "MaxPreloadSuccessProbability", "OverDrivePreloadSuccessProbability", "PullBackSpeed", "PullBackAmount",
    "LandingPitchAngle", "MinHangTimeForLandingLag", "MinLandingLagProbability", "MaxLandingLagProbability",
    "OverDriveLandingLagProbability", "MaxLandingLagTime", "MinHorsePowerScale", "MaxHorsePowerScale",
    "OverDriveHorsePowerScale", "OverPowerPhysicsTurnRateTweak", "OverPowerPhysicsVelocityTweak",
    "MinBrilliantMomentPercent", "MaxBrilliantMomentPercent", "OverDriveBrilliantMomentPercent",
    "BrilliantMomentMinDuration", "BrilliantMomentRandomDuration", "PreloadPercentagePerMPHDifference",
    "MinGLoadWreckScale", "MaxGLoadWreckScale", "OverDriveGLoadWreckScale", "RandomStartGateDelay",
    "ClutchStartProbability", "WheelieStartProbability", "WheelieStartMinDuration", "WheelieStartMaxDuration",
    "WheelieMinPullBackWithClutch", "WheelieMaxPullBackWithClutch", "WheelieMinPullBackWithoutClutch",
    "WheelieMaxPullBackWithoutClutch", "CrossUpProbability", "CrossUpMinDuration", "CrossUpMaxDuration",
    "CrossUpMinHeight", "MinALODWindow", "MaxALODWindow", "MinALODLowerLimit", "MaxALODLowerLimit",
    "MinALODUpperLimit", "MaxALODUpperLimit", "MinALODSpeed", "MaxALODSpeed", "MinALODLeadWindow",
    "MaxALODLeadWindow", "SteeringJitterFilter", "SteeringRateFilter", "Unused",
];

/// Every field the reader writes, with where the original keeps it: the brain (`true`) or the rider,
/// and the offset. The quadwords come as four lanes.
fn ch_slots(c: &mut Character) -> Vec<(bool, u32, &mut f32)> {
    let Character { tuning: t, avoid: a, alod, profile: p, .. } = c;
    let mut out: Vec<(bool, u32, &mut f32)> = vec![
        (true, 0x44, &mut alod.window.0),
        (true, 0x4c, &mut alod.lower_limit.0),
        (true, 0x48, &mut alod.upper_limit.0),
        (true, 0x2e4, &mut alod.speed.0),
        (true, 0x2e8, &mut alod.lead_window.0),
        (true, 0x208, &mut t.steering_gain.0),
        (true, 0x21c, &mut t.oversteer_gain.0),
        (true, 0x260, &mut t.slow_tolerance.0),
        (true, 0x264, &mut t.fast_tolerance.0),
        (true, 0x2b4, &mut t.pitch_distance.0),
        (true, 0x2d4, &mut t.pitch_gain.0),
        (true, 0x2d8, &mut t.pitch_filter.0),
        (true, 0x2ec, &mut t.landing_pitch.0),
        (true, 0x3ac, &mut a.near_response.0),
        (true, 0x280, &mut a.near_range.0),
        (true, 0x284, &mut a.far_range.0),
        (true, 0x3d8, &mut a.far_radius.0),
        (true, 0x3dc, &mut a.far_response.0),
        (true, 0x3e0, &mut a.far_approach_limit.0),
        (true, 0x3e4, &mut a.far_min_braking_speed.0),
        (true, 0x3e8, &mut a.far_braking_response.0),
        (true, 0x3a4, &mut a.near_separation.0),
        (true, 0x3b0, &mut a.near_braking_response.0),
        (true, 0x2f8, &mut a.passing_speed.0),
        (true, 0x3b4, &mut a.near_min_braking_speed.0),
        (true, 0x3d0, &mut a.front_window.0),
        (true, 0x3d4, &mut a.rear_window.0),
        (false, 0x120, &mut p.alod_window.min.0),
        (false, 0x124, &mut p.alod_window.max.0),
        (false, 0x128, &mut p.alod_lower_limit.min.0),
        (false, 0x12c, &mut p.alod_lower_limit.max.0),
        (false, 0x130, &mut p.alod_upper_limit.min.0),
        (false, 0x134, &mut p.alod_upper_limit.max.0),
        (false, 0x140, &mut p.alod_speed.min.0),
        (false, 0x144, &mut p.alod_speed.max.0),
        (false, 0x138, &mut p.alod_lead_window.min.0),
        (false, 0x13c, &mut p.alod_lead_window.max.0),
        (false, 0x148, &mut p.look_ahead_gain.0),
        (false, 0xcc, &mut p.near_response.min.0),
        (false, 0xd0, &mut p.near_response.max.0),
        (false, 0xd4, &mut p.near_response.overdrive.0),
        (false, 0x34, &mut p.pull_back_speed.0),
        (false, 0x38, &mut p.pull_back_amount.0),
        (false, 0x3c, &mut p.landing_lag_hang_time.0),
        (false, 0x50, &mut p.landing_lag_time.0),
        (false, 0x68, &mut p.turn_rate_tweak.0),
        (false, 0x6c, &mut p.velocity_tweak.0),
        (false, 0x80, &mut p.brilliant_min_duration.0),
        (false, 0x84, &mut p.brilliant_random_duration.0),
        (false, 0x20, &mut p.preload_per_speed.0),
        (false, 0xa0, &mut p.start_gate_delay.0),
        (false, 0xa8, &mut p.clutch_start_probability.0),
        (false, 0xac, &mut p.wheelie_start_probability.0),
        (false, 0xb0, &mut p.wheelie_start_min.0),
        (false, 0xb4, &mut p.wheelie_start_max.0),
        (false, 0xb8, &mut p.wheelie_pull_back_clutch.min.0),
        (false, 0xbc, &mut p.wheelie_pull_back_clutch.max.0),
        (false, 0xc0, &mut p.wheelie_pull_back.min.0),
        (false, 0xc4, &mut p.wheelie_pull_back.max.0),
        (false, 0xe8, &mut p.crossup_probability.0),
        (false, 0xec, &mut p.crossup_min_duration.0),
        (false, 0xf0, &mut p.crossup_max_duration.0),
        (false, 0xf4, &mut p.crossup_min_height.0),
    ];
    for (at, s) in [
        (0x24, &mut p.preload_success),
        (0x40, &mut p.landing_lag),
        (0x58, &mut p.horse_power),
        (0x70, &mut p.brilliant_moment),
        (0x90, &mut p.g_load_wreck),
    ] {
        let Spread { min, max, overdrive, value } = s;
        out.extend([(false, at, &mut min.0), (false, at + 4, &mut max.0), (false, at + 8, &mut overdrive.0), (false, at + 12, &mut value.0)]);
    }
    for (at, v) in [(0x120, &mut c.steering_jitter_filter), (0x110, &mut c.steering_rate_filter)] {
        // Four lanes of a quadword, written back as one.
        let lanes: &mut [f32; 4] = &mut v.0;
        for (k, lane) in lanes.iter_mut().enumerate() {
            out.push((true, at + 4 * k as u32, lane));
        }
    }
    let scalars: [(u32, &mut F); 12] = [
        (0x17c, &mut c.min_random_offset),
        (0x180, &mut c.max_random_offset),
        (0x188, &mut c.max_drift_size),
        (0x184, &mut c.max_drift_rate),
        (0x2f0, &mut c.no_track_look_ahead),
        (0x2f4, &mut c.no_track_min_turn),
        (0x214, &mut c.max_look_ahead),
        (0x210, &mut c.min_look_ahead),
        (0x2dc, &mut c.roll_gain),
        (0x2b0, &mut c.roll_distance),
        (0x158, &mut c.min_stunt_interval),
        (0x160, &mut c.stunt_probability),
    ];
    out.extend(scalars.into_iter().map(|(at, v)| (true, at, &mut v.0)));
    out
}

/// A number as a file might write it: plain, signed, with an exponent, with a trailing comment, or
/// spaced out (the reader squeezes the spaces away).
fn ch_number(p: &mut Pick) -> String {
    let span = [1.0, 10.0, 200.0, 5000.0][p.below(4) as usize];
    let x = float(p, span);
    match p.below(9) {
        0 => format!("{}", x as i32),
        1 => format!("{x:.1}"),
        2 => format!("{x:e}"),
        3 => format!("{x:.6} ;- a comment, 12"),
        4 => format!(" {x:.3} "),
        5 => format!("{:.2}", x.abs()),
        6 => "0.1".to_string(),
        _ => format!("{x}"),
    }
}

/// A generated character file: some of `DefaultALODSettings` and of a few `AI_<n>` sections (now and
/// then the reader's own, twice), each with a random part of the keys, in a random order and case.
fn ch_file(p: &mut Pick, skill: i32) -> String {
    let mut text = String::new();
    // No pair before the first section: the original's reader files it under section −1 and writes
    // outside its tables (a fault in the interpreter); no shipped file has one.
    let mut sections: Vec<String> = Vec::new();
    if !p.chance(5) {
        sections.push("DefaultALODSettings".into());
    }
    for _ in 0..p.below(4) {
        sections.push(format!("AI_{}", skill + p.below(5) - 2));
    }
    if !p.chance(5) {
        sections.push(format!("AI_{skill}"));
    }
    if p.chance(6) {
        sections.push(format!("ai_{skill}"));
    }
    let n = sections.len();
    for k in 0..n {
        let j = k + p.below((n - k) as u32) as usize;
        sections.swap(k, j);
    }
    for name in &sections {
        text.push_str(&format!("[{name}]\n; a comment\n\n"));
        let share = p.below(4);
        for key in CH_KEYS {
            if p.below(4) < share {
                continue;
            }
            let key = if p.chance(10) { key.to_ascii_lowercase() } else { key.to_string() };
            let value = if key.eq_ignore_ascii_case("SteeringJitterFilter") || key.eq_ignore_ascii_case("SteeringRateFilter") {
                let lanes = 3 + p.below(2);
                let sep = [",", ", ", " "][p.below(3) as usize];
                (0..lanes).map(|_| format!("{:.3}", float(p, 10.0))).collect::<Vec<_>>().join(sep)
            } else if p.chance(30) {
                String::new() // an empty value: no pair at all
            } else {
                ch_number(p)
            };
            text.push_str(&format!("{key} = {value}\n"));
        }
    }
    text
}

/// The retail character file, out of the bike archive that carries it.
fn ch_retail() -> Option<Vec<u8>> {
    let kit = ps2_oracle::Kit::load().ok()?;
    let mut f = std::fs::File::open(kit.ctx("iso").join("MX.PSR")).ok()?;
    use std::io::{Read, Seek, SeekFrom};
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).ok()?;
    let psr = game_sim::archive::Psr::parse(&front).ok()?;
    let e = psr.find("Bike\\250sx.res")?;
    f.seek(SeekFrom::Start(e.at)).ok()?;
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).ok()?;
    let res = game_sim::archive::Res::parse(&bytes).ok()?;
    let e = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with("aichrctr.aip"))?;
    res.read(&bytes, e).ok()
}

/// The heap and the library pieces the reader reaches (as tests/phy.rs): a bump allocator over every
/// allocation, `realloc` copying, `free` a no-op, `sbrk` growing a fixed break, the lock and memset.
/// The heap's state comes back so that each case can start it empty.
#[allow(clippy::type_complexity)]
fn ch_machine(ee: &mut ps2_oracle::Ee) -> Rc<RefCell<(u32, std::collections::BTreeMap<u32, u32>)>> {
    let heap = Rc::new(RefCell::new((CH_HEAP, std::collections::BTreeMap::<u32, u32>::new())));
    let take = |state: &Rc<RefCell<(u32, std::collections::BTreeMap<u32, u32>)>>, size: u32| {
        let mut s = state.borrow_mut();
        let at = s.0;
        s.0 = at + size.max(16).next_multiple_of(16);
        assert!(s.0 < CH_HEAP_END, "the heap ran out at {size} bytes");
        s.1.insert(at, size);
        at
    };
    for name in ["malloc", "new_array", "operator_new"] {
        let heap = Rc::clone(&heap);
        ee.hooks.insert(addr(name).unwrap(), Box::new(move |ee| ee.set(2, u64::from(take(&heap, ee.r(4) as u32)))));
    }
    {
        let heap = Rc::clone(&heap);
        ee.hooks.insert(
            addr("realloc").unwrap(),
            Box::new(move |ee| {
                let (old, size) = (ee.r(4) as u32, ee.r(5) as u32);
                let was = heap.borrow().1.get(&old).copied().unwrap_or(0);
                let at = take(&heap, size);
                let keep: Vec<u8> = (0..was.min(size)).map(|k| ee.u8_at(old + k)).collect();
                ee.load(at, &keep);
                ee.set(2, u64::from(at));
            }),
        );
    }
    for name in ["free", "delete_array", "operator_delete", "libc_lock", "libc_unlock"] {
        ee.hooks.insert(addr(name).unwrap(), Box::new(|ee| ee.set(2, 0)));
    }
    let brk = Rc::new(RefCell::new(CH_BREAK));
    ee.hooks.insert(
        addr("sbrk").unwrap(),
        Box::new(move |ee| {
            let incr = ee.r(5) as i32;
            let mut at = brk.borrow_mut();
            let was = *at;
            *at = was.wrapping_add(incr as u32);
            assert!((CH_BREAK..CH_BREAK_END).contains(&*at), "the C library's heap ran out");
            ee.set(2, u64::from(was));
        }),
    );
    ee.hooks.insert(
        addr("memset").unwrap(),
        Box::new(|ee| {
            let (at, byte, n) = (ee.r(4) as u32, ee.r(5) as u8, ee.r(6) as u32);
            ee.load(at, &vec![byte; n as usize]);
            ee.set(2, u64::from(at));
        }),
    );
    heap
}

/// Serve `file` (or nothing) to the four file calls, and remember the names asked for.
fn ch_serve(ee: &mut ps2_oracle::Ee, file: Option<Vec<u8>>, asked: Rc<RefCell<Vec<String>>>) {
    const HANDLE: u64 = 7;
    let size = file.as_ref().map_or(0, |f| f.len() as u64);
    let there = file.is_some();
    ee.hooks.insert(
        addr("fs_open").unwrap(),
        Box::new(move |ee| {
            let at = ee.r(5) as u32;
            let name: Vec<u8> = (0..0x100).map(|k| ee.u8_at(at + k)).take_while(|&c| c != 0).collect();
            asked.borrow_mut().push(String::from_utf8_lossy(&name).into_owned());
            ee.set(2, if there { HANDLE } else { u64::MAX });
        }),
    );
    ee.hooks.insert(addr("fs_size").unwrap(), Box::new(move |ee| ee.set(2, size)));
    let data = file.unwrap_or_default();
    ee.hooks.insert(
        addr("fs_read").unwrap(),
        Box::new(move |ee| {
            let (dst, len) = (ee.r(6) as u32, (ee.r(7) as u32 as usize).min(data.len()));
            ee.load(dst, &data[..len]);
            ee.set(2, len as u64);
        }),
    );
    ee.hooks.insert(addr("fs_close").unwrap(), Box::new(|ee| ee.set(2, 0)));
}

/// Stand the port's parse of `ini` in for the original's three lookups: the open (it has the file), the
/// section select (a miss selects nothing) and the raw text of a key, copied with its terminator as
/// the original copies it. Its number reading, the quadword split and the reader's arithmetic still
/// run.
fn ch_lookups(ee: &mut ps2_oracle::Ee, ini: Ini) {
    let ini = Rc::new(ini);
    let selected = Rc::new(RefCell::new(None::<String>));
    ee.hooks.insert(addr("ini_open").unwrap(), Box::new(|ee| ee.set(2, 1)));
    {
        let (ini, selected) = (Rc::clone(&ini), Rc::clone(&selected));
        ee.hooks.insert(
            addr("ini_section").unwrap(),
            Box::new(move |ee| {
                let at = ee.r(5) as u32;
                let name: Vec<u8> = (0..0x100).map(|k| ee.u8_at(at + k)).take_while(|&c| c != 0).collect();
                let name = String::from_utf8_lossy(&name).into_owned();
                let found = ini.section(&name).found();
                *selected.borrow_mut() = found.then_some(name);
                ee.set(2, u64::from(found));
            }),
        );
    }
    ee.hooks.insert(
        addr("ini_text").unwrap(),
        Box::new(move |ee| {
            let (key_at, dst, size) = (ee.r(5) as u32, ee.r(6) as u32, ee.r(7) as u32 as usize);
            let key: Vec<u8> = (0..0x100).map(|k| ee.u8_at(key_at + k)).take_while(|&c| c != 0).collect();
            let key = String::from_utf8_lossy(&key).into_owned();
            let value = selected.borrow().as_ref().and_then(|s| ini.section(s).text(&key).map(str::to_string));
            match value {
                Some(v) => {
                    let mut bytes: Vec<u8> = v.into_bytes();
                    bytes.truncate(size - 1);
                    bytes.push(0);
                    ee.load(dst, &bytes);
                    ee.set(2, 1);
                }
                None => ee.set(2, 0),
            }
        }),
    );
}

/// A float to start a field from: anything finite, small or large, signed or zero.
fn ch_start(p: &mut Pick) -> f32 {
    let span = [1.0, 100.0, 1.0e4][p.below(3) as usize];
    float(p, span)
}

// Ten thousand files in four quarters, which the test harness runs side by side.
#[test]
fn the_character_reader_matches_over_ten_thousand_files_1() {
    ch_run(0);
}

#[test]
fn the_character_reader_matches_over_ten_thousand_files_2() {
    ch_run(1);
}

#[test]
fn the_character_reader_matches_over_ten_thousand_files_3() {
    ch_run(2);
}

#[test]
fn the_character_reader_matches_over_ten_thousand_files_4() {
    ch_run(3);
}

/// One quarter of the character reader's ten thousand cases.
fn ch_run(quarter: u64) {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_character_read", "app_pointer", "rand_state", "fs_open", "realloc", "sbrk", "memset", "ini_open", "ini_section", "ini_text"]) else {
        return;
    };
    let [read, app_pointer, rand_state, ..] = at[..] else { unreachable!() };
    let retail = ch_retail();
    if retail.is_none() {
        skip("no MX.PSR (or no Bike\\250sx.res in it): the retail character file is left out");
    }
    let heap = ch_machine(&mut ee);
    ee.set_u32(app_pointer, CH_APP);
    ee.set_u32(CH_APP + 0x3d4, CH_SETTINGS);
    ee.set_u32(CH_SETTINGS + 0xc, CH_EVENT);
    // The reader's folder: the startup fill (path_prefix_init in research/oracle.txt) puts "Rider\" in
    // the prefix 0x150 bytes below the bike one. The fill itself starts other systems, so it is not run.
    let prefix = addr("path_prefix_bike").unwrap() - 0x150;
    let mut bytes = CH_PREFIX.as_bytes().to_vec();
    bytes.push(0);
    ee.load(prefix, &bytes);

    let mut p = Pick(0x5283_c4a7_0000_0001 ^ (quarter << 40));
    let (mut lerped, mut missing, mut fallback, mut retail_runs, mut parsed) = (0, 0, 0, 0, 0);
    for case in quarter * 2500..(quarter + 1) * 2500 {
        let difficulty: u8 = match case % 4 {
            0 => [0, 69, 70, 71, 85, 99, 100, 101, 125, 255][p.below(10) as usize],
            _ => p.below(256) as u8,
        };
        let skill = p.below(14) as i32 - 2;
        let use_retail = retail.is_some() && case % 800 == 0;
        let text: Option<Vec<u8>> = if use_retail {
            retail_runs += 1;
            retail.clone()
        } else if p.chance(50) {
            None
        } else {
            Some(ch_file(&mut p, skill).into_bytes())
        };
        let skill = if use_retail { (case / 800) as i32 } else { skill };

        // The two objects, every word random, and the fields read back into the port's start.
        let mut brain: Vec<u32> = (0..CH_BRAIN_LEN / 4).map(|_| ch_start(&mut p).to_bits()).collect();
        let mut rider: Vec<u32> = (0..CH_RIDER_LEN / 4).map(|_| ch_start(&mut p).to_bits()).collect();
        rider[0] = CH_BRAIN;
        rider[0x104 / 4] = skill as u32;
        let mut c = ch_blank();
        for (in_brain, off, v) in ch_slots(&mut c) {
            *v = f32::from_bits(if in_brain { brain[off as usize / 4] } else { rider[off as usize / 4] });
        }
        for (k, w) in brain.iter().enumerate() {
            ee.set_u32(CH_BRAIN + 4 * k as u32, *w);
        }
        for (k, w) in rider.iter().enumerate() {
            ee.set_u32(CH_RIDER + 4 * k as u32, *w);
        }
        ee.write(CH_EVENT + 0x117, 1, u128::from(difficulty));
        let state = p.next();
        ee.write(rand_state, 8, u128::from(state));
        let asked = Rc::new(RefCell::new(Vec::new()));
        // The original's own parse is slow in the interpreter (most of a second for the retail file),
        // and tests/phy.rs proves the port's reading of it; so the first 40 generated files of each quarter and the
        // retail ones run it, and the rest hand the original's lookups the port's parse.
        let real_parse = case % 2500 < 40 || use_retail || text.is_none();
        if real_parse {
            ch_serve(&mut ee, text.clone(), Rc::clone(&asked));
            for name in ["ini_open", "ini_section", "ini_text"] {
                ee.hooks.remove(&addr(name).unwrap());
            }
            parsed += 1;
        } else {
            ch_lookups(&mut ee, Ini::parse_bytes(text.as_ref().unwrap()));
        }

        *heap.borrow_mut() = (CH_HEAP, Default::default());
        ee.call(read, &[u64::from(CH_RIDER)], &[], 400_000_000);

        let ini = text.as_ref().map(|t| Ini::parse_bytes(t));
        if let Some(ini) = &ini {
            let own = ini.section(&format!("AI_{skill}")).found();
            if !own {
                fallback += 1;
            }
        } else {
            missing += 1;
        }
        let mut rng = Rng::from_state(state);
        read_character(ini.as_ref(), skill, difficulty, &mut rng, &mut c);
        if ini.is_some() && difficulty != 100 {
            lerped += 1;
        }
        for (in_brain, off, v) in ch_slots(&mut c) {
            let w = v.to_bits();
            if in_brain { brain[off as usize / 4] = w } else { rider[off as usize / 4] = w }
        }
        let what = format!("case {case}: skill {skill}, difficulty {difficulty}, state {state:#x}, retail {use_retail}");
        if real_parse {
            assert_eq!(asked.borrow().as_slice(), [CH_FILE.to_string()], "{what}: the name it opened");
        }
        assert_eq!(rng.state(), ee.read(rand_state, 8) as u64, "{what}: the generator ends elsewhere");
        for (k, want) in brain.iter().enumerate() {
            let got = ee.u32_at(CH_BRAIN + 4 * k as u32);
            if got != *want {
                let file = text.as_ref().map(|t| String::from_utf8_lossy(t).into_owned()).unwrap_or_default();
                let file = if use_retail { String::new() } else { file };
                panic!("{what}: brain +{:#x} is {got:#010x} ({}), the port has {want:#010x} ({})\n{file}", 4 * k, f32::from_bits(got), f32::from_bits(*want));
            }
        }
        for (k, want) in rider.iter().enumerate() {
            let got = ee.u32_at(CH_RIDER + 4 * k as u32);
            if got != *want {
                let file = text.as_ref().map(|t| String::from_utf8_lossy(t).into_owned()).unwrap_or_default();
                let file = if use_retail { String::new() } else { file };
                panic!("{what}: rider +{:#x} is {got:#010x} ({}), the port has {want:#010x} ({})\n{file}", 4 * k, f32::from_bits(got), f32::from_bits(*want));
            }
        }
    }
    println!("character files: {lerped} put between, {missing} missing, {fallback} without their own section, {retail_runs} retail, {parsed} parsed by the original");
    assert!(missing > 10 && fallback > 100, "the cases did not reach every branch");
}

/// A [`Character`] with every field zero, for [`ch_slots`] to fill.
fn ch_blank() -> Character {
    let z = F(0.0);
    let v = ps2_float::Vf::new(0.0, 0.0, 0.0, 0.0);
    Character {
        tuning: game_sim::ai::Tuning {
            steering_gain: z, oversteer_gain: z, aim_gain: z, pitch_distance: z, pitch_gain: z, pitch_filter: z,
            landing_pitch: z, slow_tolerance: z, fast_tolerance: z, cruise_base: z, cruise_gain: z, brake_gain: z,
            steer_scale: z, pitch_scale: z,
        },
        avoid: game_sim::ai::AvoidTuning::default(),
        alod: Default::default(),
        steering_jitter_filter: v,
        steering_rate_filter: v,
        min_random_offset: z, max_random_offset: z, max_drift_size: z, max_drift_rate: z,
        no_track_look_ahead: z, no_track_min_turn: z, min_look_ahead: z, max_look_ahead: z,
        roll_gain: z, roll_distance: z, min_stunt_interval: z, stunt_probability: z,
        profile: Profile { near_response: Triple::default(), wheelie_pull_back: Pair::default(), ..Default::default() },
    }
}

// ---------------------------------------------------------------------------------------------------
// S281 — the AI tick: its order and its own clauses, the lever hand-off, and the rider assist
// ---------------------------------------------------------------------------------------------------
//
// `ai_update` runs whole in the interpreter with every call it makes out hooked: the interface's four
// slots and the brain's two through fake vtables, the route follower, the listener walk and the base
// update by address. Each hook logs its name and plays what the real one would leave behind (the copy
// a new speed, forward height and ground bit; the controller new outputs; the route follower and the
// walk an answer), and the port's `ai::update` is handed the same script through `ai::Steps`. The hand-
// off and the assist are each called as the original's own functions; the route follower's tail, which
// builds the assist's aim, is run as a stretch of its function.

use game_sim::ai::{Brain, Climb, Levers, Recovery, Steps, Weights, tick_flag};

const TK_RIDER: u32 = 0x01a0_0000; // the rider: +0x10d0 its interface (+0 the brain)
const TK_BODY: u32 = 0x01a2_0000;
const TK_BRAIN: u32 = 0x01a3_0000;
const TK_BRAIN_VT: u32 = 0x01a3_1000;
const TK_IFACE_VT: u32 = 0x01a3_2000;
const TK_CARD: u32 = 0x01a3_3000;
const TK_KEEPER: u32 = 0x01a3_4000;
/// Not code: the hooked slots.
const TK_FEED: u32 = 0x01a3_5000;
const TK_HAND_OFF: u32 = 0x01a3_5008;
const TK_APPROACH: u32 = 0x01a3_5010;
const TK_RESCUE: u32 = 0x01a3_5018;
const TK_CONTROLS: u32 = 0x01a3_5020;
const TK_EXTRA: u32 = 0x01a3_5028;
const TK_ASSIST: u32 = 0x01a3_5030;

/// A timer or a weight: often an edge the comparisons turn on.
fn edge(pick: &mut Pick, near: f32, span: f32) -> f32 {
    match pick.below(8) {
        0 => near,
        1 => f32::from_bits(near.to_bits().wrapping_add(1)),
        2 => f32::from_bits(near.to_bits().wrapping_sub(1)),
        3 => 0.0,
        4 => -0.0,
        5 => near + float(pick, 0.05),
        _ => float(pick, span),
    }
}

/// The hand-off against the port over ten thousand ticks: with and without a brain, a race card and the
/// score keeper's running bit; the swerve and the back-up timers either side of 0; on and off the ground.
/// The five levers are compared bit for bit, and the answer.
#[test]
fn the_lever_hand_off_matches_over_ten_thousand_ticks() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_hand_off"]) else { return };
    let mut pick = Pick(0x5281_0ff0_a11e_7e45);
    let mut seen = [0u32; 5]; // let go, air, ground, backing up, swerving
    for case in 0..10_000 {
        let has_brain = !pick.chance(12);
        let live = !pick.chance(12);
        let has_card = live || pick.chance(2);
        let keeper_word = (pick.next() as u32 & !1) | u32::from(live || !has_card && pick.chance(2));
        let brain = Brain {
            kind: Some(1),
            mode: 5,
            flags: pick.next(),
            speed: F(0.0),
            forward_y: F(0.0),
            stuck: F(0.0),
            climb: Climb::default(),
            steer: F(float(&mut pick, 1.5)),
            pitch: F(float(&mut pick, 1.5)),
            throttle: F(float(&mut pick, 1.5)),
            brake: F(float(&mut pick, 1.5)),
        };
        let recovery = Recovery {
            back_up: F(edge(&mut pick, 0.0, 2.0)),
            swerve: F(edge(&mut pick, 0.0, 2.0)),
            swerve_bar: F([1.0, -1.0][pick.below(2) as usize]),
        };
        let before = Levers {
            steer: F(float(&mut pick, 2.0)),
            lean: F(float(&mut pick, 2.0)),
            gas: F(float(&mut pick, 2.0)),
            brake: F(float(&mut pick, 2.0)),
            reverse: F(float(&mut pick, 2.0)),
        };

        ee.load(TK_RIDER, &[0u8; 0x1500]);
        ee.load(TK_BRAIN, &[0u8; 0x400]);
        ee.set_u32(TK_RIDER + 0x10d0, if has_brain { TK_BRAIN } else { 0 });
        ee.set_u32(TK_RIDER + 0xf9c, if has_card { TK_CARD } else { 0 });
        ee.set_u32(TK_RIDER + 0xfa0, TK_KEEPER);
        ee.set_u32(TK_KEEPER + 0x5c, keeper_word);
        ee.write(TK_BRAIN + 0x30, 8, u128::from(brain.flags));
        for (off, v) in [(0xe0, brain.throttle), (0xe4, brain.brake), (0xf0, brain.steer), (0xf4, brain.pitch)] {
            ee.set_f32(TK_BRAIN + off, v.to_f32());
        }
        ee.set_f32(TK_RIDER + 0x14c0, recovery.back_up.to_f32());
        ee.set_f32(TK_RIDER + 0x14c4, recovery.swerve_bar.to_f32());
        ee.set_f32(TK_RIDER + 0x14c8, recovery.swerve.to_f32());
        let lever_offs = [0x628, 0x62c, 0x63c, 0x640, 0x648];
        let lever_of = |l: &Levers| [l.steer, l.lean, l.gas, l.brake, l.reverse];
        for (off, v) in lever_offs.iter().zip(lever_of(&before)) {
            ee.set_f32(TK_RIDER + off, v.to_f32());
        }
        ee.call(at[0], &[u64::from(TK_RIDER)], &[0.033_333], BUDGET);

        let mut levers = before;
        let ok = game_sim::ai::hand_off(has_brain.then_some(&brain), has_card && keeper_word & 1 != 0, &recovery, &mut levers);
        assert_eq!(ee.v0(), u64::from(ok), "case {case}: the answer");
        for ((off, ours), name) in lever_offs.iter().zip(lever_of(&levers)).zip(["bar", "lean", "gas", "brake", "reverse"]) {
            assert_eq!(ee.u32_at(TK_RIDER + off), ours.to_f32().to_bits(), "case {case}: the {name} lever");
        }
        let ground = brain.flags & game_sim::ai::flag::ON_GROUND != 0;
        seen[0] += u32::from(!ok);
        seen[1] += u32::from(ok && !ground);
        seen[2] += u32::from(ok && ground);
        seen[3] += u32::from(ok && ground && !(recovery.back_up <= F(0.0)));
        seen[4] += u32::from(ok && !(recovery.swerve <= F(0.0)));
    }
    println!("let go {}, air {}, ground {}, backing up {}, swerving {}", seen[0], seen[1], seen[2], seen[3], seen[4]);
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 300, "branch {k} reached only {n} times: {seen:?}");
    }
}

/// The route follower's tail and the rider assist against the port over ten thousand ticks. The tail
/// (from just after the trick plan to the function's epilogue) runs as a stretch with the interface's
/// assist slot hooked, and the original assist is run on the rider when the tail called it; the aim it leaves on the brain and the
/// body's yaw rate and velocity after are compared bit for bit, with the rest of the body untouched.
#[test]
fn the_route_tail_and_the_rider_assist_match_over_ten_thousand_ticks() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_tick_flag", "ai_rider_assist"]) else { return };
    let [follow, assist_fn] = at[..] else { unreachable!() };
    // The tail's first instruction (after the trick plan's call) and its epilogue, as offsets into the
    // route follower.
    let (tail, epilogue) = (follow + 0x16c0, follow + 0x1708);
    assert_eq!(ee.u32_at(tail), 0xde62_0030, "the tail starts with `ld v0, 48(s3)`");

    let assisted = Rc::new(RefCell::new(0u32));
    let a = assisted.clone();
    ee.hooks.insert(
        TK_ASSIST,
        Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, TK_RIDER + 0x10d0, "the assist is called on the interface");
            *a.borrow_mut() += 1;
        }),
    );
    ee.hooks.insert(epilogue, Box::new(|ee| ee.set(31, u64::from(ps2_oracle::DONE) | 0xffff_ffff_0000_0000)));

    let mut pick = Pick(0x5281_a551_57ed_0001);
    let mut seen = [0u32; 3]; // assisted, on the ground, a -0 yaw rate in
    for case in 0..10_000 {
        let flags = pick.next();
        let w = float(&mut pick, 1.0);
        let direction = pick.vec(1.0, w);
        let target_speed = F(match pick.below(3) {
            0 => 150.0,
            1 => float(&mut pick, 200.0),
            _ => pick.range(0.0, 160.0),
        });
        let weights = Weights { heading: F(edge(&mut pick, 1.0, 1.5)), aim: F(edge(&mut pick, 1.0, 1.5)) };
        let heading = F(float(&mut pick, 3.2));
        let on_ground = pick.below(3) != 0;
        let yaw0 = if pick.chance(10) { -0.0 } else { float(&mut pick, 6.0) };
        let w = float(&mut pick, 2.0);
        let vel0 = pick.vec(160.0, w);
        let old_aim = (float(&mut pick, 100.0), float(&mut pick, 100.0));

        ee.load(TK_RIDER, &[0u8; 0x1500]);
        ee.load(TK_BODY, &[0u8; 0x400]);
        ee.load(TK_BRAIN, &[0u8; 0x400]);
        ee.set_u32(TK_RIDER + 0x10d0, TK_BRAIN);
        ee.set_u32(TK_RIDER + 0x10d4, 1);
        ee.set_u32(TK_RIDER + 0x10d8, TK_IFACE_VT);
        ee.set_u32(TK_IFACE_VT + 0x18, TK_ASSIST);
        ee.set_u32(TK_RIDER + 0x7a4, TK_BODY);
        ee.set_u32(TK_RIDER + 0x574, if on_ground { pick.next() as u32 | 1 } else { 0 });
        ee.set_f32(TK_RIDER + 0x1138, weights.heading.to_f32());
        ee.set_f32(TK_RIDER + 0x113c, weights.aim.to_f32());
        ee.set_u32(TK_BRAIN + 0x40, TK_RIDER + 0x10d0);
        ee.write(TK_BRAIN + 0x30, 8, u128::from(flags));
        put(&mut ee, TK_BRAIN + 0x1d0, direction);
        ee.set_f32(TK_BRAIN + 0x1e0, target_speed.to_f32());
        ee.set_f32(TK_BRAIN + 0x1c0, old_aim.0);
        ee.set_f32(TK_BRAIN + 0x1c8, old_aim.1);
        ee.set_f32(TK_BRAIN + 0x1f4, heading.to_f32());
        for k in 0..0x40u32 {
            ee.set_u32(TK_BODY + 0x200 + 4 * k, pick.next() as u32);
        }
        ee.set_f32(TK_BODY + 0x284, yaw0);
        put(&mut ee, TK_BODY + 0x260, vel0);
        let body_before: Vec<u32> = (0..0x40u32).map(|k| ee.u32_at(TK_BODY + 0x200 + 4 * k)).collect();
        *assisted.borrow_mut() = 0;

        ee.set(19, u64::from(TK_BRAIN));
        ee.set_f(22, 0.033_333);
        ee.call(tail, &[], &[], BUDGET);
        // The assist is the tail's last call, so the original's runs straight after it (calling it from
        // inside the hook would move the hook).
        if *assisted.borrow() > 0 {
            ee.call(assist_fn, &[u64::from(TK_RIDER)], &[], BUDGET);
            assert_eq!(ee.v0(), 1, "case {case}: the assist returns 1");
        }

        let mut yaw = F(yaw0);
        let mut vel = vel0;
        let aim = game_sim::ai::assist_aim(flags, direction, target_speed);
        if let Some(aim) = aim {
            game_sim::ai::assist(&weights, heading, aim, on_ground, &mut yaw, &mut vel);
        }
        assert_eq!(*assisted.borrow(), u32::from(aim.is_some()), "case {case}: the assist calls");
        let (ax, az) = aim.unwrap_or((F(old_aim.0), F(old_aim.1)));
        assert_eq!(ee.u32_at(TK_BRAIN + 0x1c0), ax.to_f32().to_bits(), "case {case}: aim x");
        assert_eq!(ee.u32_at(TK_BRAIN + 0x1c8), az.to_f32().to_bits(), "case {case}: aim z");
        let mut want = body_before;
        want[(0x284 - 0x200) / 4] = yaw.to_f32().to_bits();
        for (k, b) in vel.to_bits().iter().enumerate() {
            want[(0x260 - 0x200) / 4 + k] = *b;
        }
        for (k, w) in want.iter().enumerate() {
            assert_eq!(ee.u32_at(TK_BODY + 0x200 + 4 * k as u32), *w, "case {case}: body +{:#x}", 0x200 + 4 * k);
        }
        seen[0] += u32::from(aim.is_some());
        seen[1] += u32::from(aim.is_some() && on_ground);
        seen[2] += u32::from(aim.is_some() && yaw0.to_bits() == (-0.0f32).to_bits());
    }
    println!("assisted {}, on the ground {}, -0 yaw rate {}", seen[0], seen[1], seen[2]);
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 200, "branch {k} reached only {n} times: {seen:?}");
    }
}

/// What a hooked call of the tick plays back.
#[derive(Clone, Default)]
struct TickScript {
    log: Vec<&'static str>,
    fed: (f32, f32, bool),
    controlled: [f32; 4],
    routed: bool,
    heard: bool,
    other: bool,
}

/// The port's side of [`TickScript`].
struct Played(TickScript);

impl Steps for Played {
    fn rescue(&mut self, _: &mut Brain) {
        self.0.log.push("rescue");
    }
    fn feed(&mut self, b: &mut Brain) {
        self.0.log.push("feed");
        let (speed, forward_y, ground) = self.0.fed;
        b.speed = F(speed);
        b.forward_y = F(forward_y);
        b.flags = if ground { b.flags | game_sim::ai::flag::ON_GROUND } else { b.flags & !game_sim::ai::flag::ON_GROUND };
    }
    fn route(&mut self, _: &mut Brain, _: F) -> bool {
        self.0.log.push("route");
        self.0.routed
    }
    fn controls(&mut self, b: &mut Brain) {
        self.0.log.push("controls");
        let [t, k, s, p] = self.0.controlled;
        (b.throttle, b.brake, b.steer, b.pitch) = (F(t), F(k), F(s), F(p));
    }
    fn approach(&mut self, _: &mut Brain, _: F) {
        self.0.log.push("approach");
    }
    fn extra(&mut self, _: &mut Brain, _: F) {
        self.0.log.push("extra");
    }
    fn hand_off(&mut self, _: &mut Brain, _: F) {
        self.0.log.push("hand-off");
    }
    fn listeners(&mut self, _: &mut Brain, _: F) -> bool {
        self.0.log.push("listeners");
        self.0.heard
    }
    fn other_mode(&mut self, _: &mut Brain, _: F) -> bool {
        self.0.log.push("other mode");
        self.0.other
    }
}

/// `ai_update` against `ai::update` over ten thousand ticks: every interface type and none, riding and
/// other modes, the stuck clock and the climb clock about their edges, the flagged steps on and off. The
/// calls made (in order), the answer and every brain field the tick or its scripted calls touch are
/// compared bit for bit.
#[test]
fn the_ai_tick_matches_over_ten_thousand_ticks() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_update", "ai_tick_flag", "ai_listener_walk", "ai_update_base"]) else { return };
    let [update, follow, walk, base] = at[..] else { unreachable!() };

    let script = Rc::new(RefCell::new(TickScript::default()));
    let logged = |name: &'static str, s: &Rc<RefCell<TickScript>>| {
        let s = s.clone();
        Box::new(move |_: &mut ps2_oracle::Ee| s.borrow_mut().log.push(name)) as Box<dyn FnMut(&mut ps2_oracle::Ee)>
    };
    ee.hooks.insert(TK_RESCUE, logged("rescue", &script));
    ee.hooks.insert(TK_APPROACH, logged("approach", &script));
    ee.hooks.insert(TK_EXTRA, logged("extra", &script));
    ee.hooks.insert(TK_HAND_OFF, logged("hand-off", &script));
    let s = script.clone();
    ee.hooks.insert(
        TK_FEED,
        Box::new(move |ee| {
            let mut s = s.borrow_mut();
            s.log.push("feed");
            let (speed, forward_y, ground) = s.fed;
            ee.set_f32(TK_BRAIN + 0x80, speed);
            ee.set_f32(TK_BRAIN + 0xb4, forward_y);
            let f = ee.read(TK_BRAIN + 0x30, 8) as u64 & !0x20;
            ee.write(TK_BRAIN + 0x30, 8, u128::from(f | if ground { 0x20 } else { 0 }));
        }),
    );
    let s = script.clone();
    ee.hooks.insert(
        TK_CONTROLS,
        Box::new(move |ee| {
            let mut s = s.borrow_mut();
            s.log.push("controls");
            for (off, v) in [0xe0, 0xe4, 0xf0, 0xf4].into_iter().zip(s.controlled) {
                ee.set_f32(TK_BRAIN + off, v);
            }
        }),
    );
    let s = script.clone();
    ee.hooks.insert(follow, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        s.log.push("route");
        ee.set(2, u64::from(s.routed));
    }));
    let s = script.clone();
    ee.hooks.insert(walk, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        s.log.push("listeners");
        ee.set(2, u64::from(s.heard));
    }));
    let s = script.clone();
    ee.hooks.insert(base, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        s.log.push("other mode");
        ee.set(2, u64::from(s.other));
    }));
    let iface = TK_RIDER + 0x10d0;
    // With no interface the original calls the hand-off through a null pointer: low RAM holds a vtable
    // pointer here so that call lands on the hook too.
    let null_vt = ee.u32_at(8);
    ee.set_u32(8, TK_IFACE_VT);

    let mut pick = Pick(0x5281_71c4_0bde_0001);
    let mut seen = [0u32; 8]; // rescued, stuck counting, other mode, climb cut, climb lean, cut starts, approach, extra
    for case in 0..10_000 {
        let kind = match pick.below(10) {
            0 => None,
            1..=5 => Some(1),
            _ => Some([0, 2, 3, 6, 4, 5, 7][pick.below(7) as usize]),
        };
        let mode = if pick.chance(8) { [0, 1, 2, 3, 4, 6][pick.below(6) as usize] } else { 5 };
        let mut flags = pick.next();
        for bit in [tick_flag::APPROACH, tick_flag::EXTRA] {
            if pick.chance(2) {
                flags &= !bit;
            }
        }
        let dt = if pick.chance(2) { 0.033_333_335 } else { pick.range(0.0, 0.1) };
        let climb_speed = if pick.chance(2) { 40.0 } else { pick.range(0.0, 60.0) };
        let steep = if pick.chance(2) { 0.7 } else { pick.range(-1.0, 1.0) };
        let mut brain = Brain {
            kind,
            mode,
            flags,
            speed: F(edge(&mut pick, 10.0, 60.0)),
            forward_y: F(edge(&mut pick, steep, 1.0)),
            stuck: F(edge(&mut pick, 5.0 - dt, 5.0)),
            climb: Climb {
                hold: F(edge(&mut pick, 0.0, 5.0)),
                steep: F(steep),
                timer: F(if pick.chance(2) { 5.0 - dt * pick.fraction() } else { edge(&mut pick, 5.0 - dt, 5.0) }),
                speed: F(climb_speed),
            },
            steer: F(float(&mut pick, 1.0)),
            pitch: F(float(&mut pick, 1.0)),
            throttle: F(edge(&mut pick, 0.0, 1.0)),
            brake: F(float(&mut pick, 1.0)),
        };
        let played = TickScript {
            log: Vec::new(),
            fed: (edge(&mut pick, climb_speed, 60.0), edge(&mut pick, steep, 1.0), pick.chance(2)),
            controlled: [edge(&mut pick, 0.0, 1.0), float(&mut pick, 1.0), float(&mut pick, 1.0), float(&mut pick, 1.0)],
            routed: !pick.chance(4),
            heard: !pick.chance(4),
            other: pick.chance(2),
        };

        ee.load(TK_RIDER, &[0u8; 0x1500]);
        ee.load(TK_BRAIN, &[0u8; 0x400]);
        ee.set_u32(TK_BRAIN + 8, TK_BRAIN_VT);
        ee.set_u32(TK_BRAIN_VT + 0x90, TK_CONTROLS);
        ee.set_u32(TK_BRAIN_VT + 0xa8, TK_EXTRA);
        ee.set_u32(iface, TK_BRAIN);
        ee.set_u32(iface + 4, kind.unwrap_or(0) as u32);
        ee.set_u32(iface + 8, TK_IFACE_VT);
        for (slot, hook) in [(0x8, TK_FEED), (0xc, TK_HAND_OFF), (0x28, TK_APPROACH), (0x40, TK_RESCUE)] {
            ee.set_u32(TK_IFACE_VT + slot, hook);
        }
        ee.set_u32(TK_BRAIN + 0x40, if kind.is_some() { iface } else { 0 });
        ee.set_u32(TK_BRAIN + 0xd0, mode as u32);
        ee.write(TK_BRAIN + 0x30, 8, u128::from(brain.flags));
        let fields = |b: &Brain| {
            [
                (0x80, b.speed, "speed"),
                (0xb4, b.forward_y, "forward height"),
                (0x3f0, b.stuck, "stuck clock"),
                (0x390, b.climb.hold, "climb hold"),
                (0x394, b.climb.steep, "climb steepness"),
                (0x398, b.climb.timer, "climb clock"),
                (0x30c, b.climb.speed, "climb speed"),
                (0xe0, b.throttle, "throttle"),
                (0xe4, b.brake, "brake"),
                (0xf0, b.steer, "steer"),
                (0xf4, b.pitch, "pitch"),
            ]
        };
        for (off, v, _) in fields(&brain) {
            ee.set_f32(TK_BRAIN + off, v.to_f32());
        }
        *script.borrow_mut() = played.clone();

        ee.call(update, &[u64::from(TK_BRAIN)], &[dt], BUDGET);

        let before = brain;
        let mut ours = Played(played);
        let ok = game_sim::ai::update(&mut brain, &mut ours, F(dt));
        let theirs = script.borrow();
        assert_eq!(theirs.log, ours.0.log, "case {case}: the calls, in order ({before:?})");
        assert_eq!(ee.v0(), u64::from(ok), "case {case}: the answer");
        assert_eq!(ee.read(TK_BRAIN + 0x30, 8) as u64, brain.flags, "case {case}: flags");
        for (off, v, name) in fields(&brain) {
            assert_eq!(ee.u32_at(TK_BRAIN + off), v.to_f32().to_bits(), "case {case}: {name} ({before:?})");
        }
        let riding_mx = mode == 5 && kind == Some(1);
        seen[0] += u32::from(ours.0.log.contains(&"rescue"));
        seen[1] += u32::from(brain.stuck != F(0.0));
        seen[2] += u32::from(mode != 5);
        seen[3] += u32::from(riding_mx && !(before.climb.hold <= F(0.0)));
        seen[4] += u32::from(riding_mx && before.climb.hold <= F(0.0) && brain.climb.timer != F(0.0));
        seen[5] += u32::from(riding_mx && before.climb.hold <= F(0.0) && !(brain.climb.hold <= F(0.0)));
        seen[6] += u32::from(ours.0.log.contains(&"approach"));
        seen[7] += u32::from(ours.0.log.contains(&"extra"));
    }
    ee.set_u32(8, null_vt);
    println!(
        "rescued {}, stuck counting {}, other mode {}, climb cut {}, climb lean {}, cut starts {}, approach {}, extra {}",
        seen[0], seen[1], seen[2], seen[3], seen[4], seen[5], seen[6], seen[7]
    );
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 100, "branch {k} reached only {n} times: {seen:?}");
    }
}

/// The tick's live side, on the free09 riders: one AI tick of each of the three, replayed through
/// `ai::update` on every pair of consecutive rows of `context/capture/s281/ai-tick.csv` (watch/s281-ai-
/// tick.toml, slot 44, no pad). The rider-to-brain copy and the controller play the next row's speed,
/// forward height, ground bit and controls; the hand-off runs the port's `ai::hand_off` on the levers of
/// the row before. The port must leave the next row's stuck and climb clocks, its controls and its
/// levers bit for bit, and carry the ground bit into its flag word.
///
/// The swerve timer is read at its own phase: a hit reaction sets it to 1 before the AI tick and the
/// bike counts it down by `dt` after, so the hand-off saw it positive exactly when it changed (and was
/// not set afresh, to 1 exactly, after the countdown) or is still positive.
#[test]
fn the_free09_riders_ticks_are_the_originals() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/capture/s281/ai-tick.csv");
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!(
            "no {}: `./kit pcsx2 with -- ./kit record watch/s281-ai-tick.toml -o context/capture/s281/ai-tick.csv` \
             (slot 44 of copy 1, journal s12-ai-tick)",
            path.display()
        ));
        return;
    };
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().expect("a header").split(',').collect();
    let rows: Vec<Vec<&str>> = lines.map(|l| l.split(',').collect()).collect();
    let col = |name: &str| head.iter().position(|h| *h == name).unwrap_or_else(|| panic!("no column {name}"));
    let fl = |row: &[&str], name: &str| F(row[col(name)].parse::<f64>().unwrap_or_else(|e| panic!("{name}: {e}")) as f32);
    let word = |row: &[&str], name: &str| row[col(name)].parse::<u64>().unwrap_or_else(|e| panic!("{name}: {e}"));

    struct Live {
        fed: (F, F, bool),
        controlled: [F; 4],
        live: bool,
        recovery: Recovery,
        levers: Levers,
        log: Vec<&'static str>,
    }
    impl Steps for Live {
        fn rescue(&mut self, _: &mut Brain) {
            self.log.push("rescue");
        }
        fn feed(&mut self, b: &mut Brain) {
            (b.speed, b.forward_y) = (self.fed.0, self.fed.1);
            b.flags = if self.fed.2 { b.flags | game_sim::ai::flag::ON_GROUND } else { b.flags & !game_sim::ai::flag::ON_GROUND };
        }
        fn route(&mut self, _: &mut Brain, _: F) -> bool {
            true
        }
        fn controls(&mut self, b: &mut Brain) {
            [b.throttle, b.brake, b.steer, b.pitch] = self.controlled;
        }
        fn approach(&mut self, _: &mut Brain, _: F) {}
        fn extra(&mut self, _: &mut Brain, _: F) {
            self.log.push("extra");
        }
        fn hand_off(&mut self, b: &mut Brain, _: F) {
            game_sim::ai::hand_off(Some(b), self.live, &self.recovery, &mut self.levers);
        }
        fn listeners(&mut self, _: &mut Brain, _: F) -> bool {
            true
        }
        fn other_mode(&mut self, _: &mut Brain, _: F) -> bool {
            self.log.push("other mode");
            true
        }
    }

    let mut pairs = 0;
    let mut swerves = 0;
    let mut air = 0;
    let mut carry_misses = Vec::new();
    for w in rows.windows(2) {
        let (p, r) = (&w[0][..], &w[1][..]);
        let step = fl(r, "clock") - fl(p, "clock");
        // A row the recorder missed (two ticks between rows) or repeated (none).
        if !(step > F(0.03) && step < F(0.04)) {
            continue;
        }
        let dt = F(0.033_333_335);
        for k in 0..3 {
            let b = |n: &str| format!("b{k}_{n}");
            let rr = |n: &str| format!("r{k}_{n}");
            let flags = |row: &[&str]| word(row, &b("flags_lo")) | word(row, &b("flags_hi")) << 32;
            let mut brain = Brain {
                kind: Some(1),
                mode: 5,
                flags: flags(p),
                speed: fl(p, &b("speed")),
                forward_y: fl(p, &b("fwd_y")),
                stuck: fl(p, &b("stuck")),
                climb: Climb {
                    hold: fl(p, &b("climb_hold")),
                    steep: fl(p, &b("climb_steep")),
                    timer: fl(p, &b("climb_timer")),
                    speed: fl(p, &b("climb_speed")),
                },
                throttle: fl(p, &b("throttle")),
                brake: fl(p, &b("brake")),
                steer: fl(p, &b("steer")),
                pitch: fl(p, &b("pitch")),
            };
            let (was, now) = (fl(p, &rr("swerve")), fl(r, &rr("swerve")));
            let swerving = if now.to_bits() == was.to_bits() {
                now > F(0.0)
            } else {
                now != F(1.0) || was > F(0.0)
            };
            let lever = |row: &[&str]| Levers {
                steer: fl(row, &rr("lever_steer")),
                lean: fl(row, &rr("lever_lean")),
                gas: fl(row, &rr("lever_gas")),
                brake: fl(row, &rr("lever_brake")),
                reverse: fl(row, &rr("lever_reverse")),
            };
            let mut live = Live {
                fed: (fl(r, &b("speed")), fl(r, &b("fwd_y")), flags(r) & 0x20 != 0),
                controlled: [fl(r, &b("throttle")), fl(r, &b("brake")), fl(r, &b("steer")), fl(r, &b("pitch"))],
                live: word(r, &rr("card")) != 0,
                recovery: Recovery {
                    back_up: fl(r, &rr("back_up")),
                    swerve: if swerving { F(1.0) } else { F(0.0) },
                    swerve_bar: fl(r, &rr("swerve_bar")),
                },
                levers: lever(p),
                log: Vec::new(),
            };
            let at = format!("frame {} rider {k}", r[col("frame")]);
            assert!(game_sim::ai::update(&mut brain, &mut live, dt), "{at}: the answer");
            assert!(live.log.is_empty(), "{at}: calls the free09 riders never make: {:?}", live.log);
            let want = lever(r);
            assert_eq!(
                [live.levers.steer, live.levers.lean, live.levers.gas, live.levers.brake, live.levers.reverse].map(F::to_bits),
                [want.steer, want.lean, want.gas, want.brake, want.reverse].map(F::to_bits),
                "{at}: the levers (bar, lean, gas, brake, reverse)"
            );
            for (name, ours) in [
                ("stuck", brain.stuck),
                ("climb_hold", brain.climb.hold),
                ("climb_timer", brain.climb.timer),
                ("throttle", brain.throttle),
                ("brake", brain.brake),
                ("steer", brain.steer),
                ("pitch", brain.pitch),
            ] {
                assert_eq!(ours.to_bits(), fl(r, &b(name)).to_bits(), "{at}: {name}");
            }
            if brain.flags & tick_flag::WAS_ON_GROUND != flags(r) & tick_flag::WAS_ON_GROUND {
                carry_misses.push(at);
            }
            pairs += 1;
            swerves += u32::from(swerving);
            air += u32::from(flags(r) & 0x20 == 0);
        }
    }
    println!("{pairs} rider ticks, {swerves} swerving, {air} in the air; carry misses {carry_misses:?}");
    assert!(pairs >= 2500, "only {pairs} rider ticks");
    assert!(swerves >= 50 && air >= 500, "swerving {swerves}, in the air {air}");
    // The ground-bit carry: the route follower writes the flag word too, and on the first airborne tick
    // the row can catch the word between writers (two take-offs in this capture, journal s12-ai-tick).
    assert!(carry_misses.len() <= 2, "the ground-bit carry missed {} ticks: {carry_misses:?}", carry_misses.len());
}
