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
    Net { nodes, ..Net::default() }
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

// --- the free-ride target seeker (S282a) -------------------------------------------------------------
//
// The seeker runs in the interpreter from ai_tick_flag's first instruction to where it hands on: its
// tail (S282b) or the path branch (S282b), both hooked to return. vt+0x80 and vt+0x94 are the
// original's own reach and hand-off, rand() the original's; the list build is hooked to do nothing, every
// other vtable slot to panic. The world — up to three target sets with pick lists and courses, eight
// targets with per-player done flags, six jumps, and the fixed sequence when there is one — is
// generated around a rider placed near the setup point, the lip, the landing or anywhere, and written
// out the way the original lays it. Compared each tick: what it returned, every field of the seeker,
// the four zeroed controls, every leg of every jump, and the generator's state.

use game_sim::ai::seek as sk;

impl Pick {
    /// A vector out to `r` with a `w` in −1..1.
    fn vec_w(&mut self, r: f32) -> Vf {
        let w = self.range(-1.0, 1.0);
        self.vec(r, w)
    }
}

const SK_AI: u32 = 0x0180_0000;
const SK_VT: u32 = 0x0180_1000;
const SK_IFACE: u32 = 0x0180_2000;
const SK_RACE: u32 = 0x0180_3000;
const SK_BIKE: u32 = 0x0180_4000;
const SK_SETS: u32 = 0x0181_0000;
const SK_SET_SIZE: u32 = 0x200;
const SK_LISTS: u32 = 0x0182_0000;
const SK_TARGETS: u32 = 0x0183_0000;
const SK_TARGET_SIZE: u32 = 0x200;
const SK_JUMPS: u32 = 0x0184_0000;
const SK_JUMP_SIZE: u32 = 0x120;
/// Not code: the vtable slots the seeker must not call land here, and panic.
const SK_HOOKS: u32 = 0x0186_0000;
const SK_TAIL: u64 = 0x7a11;
const SK_PATH: u64 = 0x9a7;
const SK_JUMP_COUNT: usize = 6;
const SK_TARGET_COUNT: usize = 8;
const SK_SET_COUNT: usize = 3;
const SK_GROUPS: u32 = 0x0185_0000;
const SK_GROUP_SIZE: u32 = 0x100;
const SK_GROUP_COUNT: usize = 6;
const SK_OWNERS: u32 = 0x0185_1000;
const SK_OWNER_SIZE: u32 = 0x200;
const SK_OWNER_VT: u32 = 0x0185_2000;
/// Not code: the owner's vtable slots land here, and all but the flag question panic.
const SK_OWNER_HOOKS: u32 = 0x0185_3000;
/// The bit the name registry hands out for "HitFreeride" (any one bit does).
const SK_FLAG_BIT: u64 = 0x40;
/// Where an owner keeps its answer to the flag question, for the hook to give back.
const SK_OWNER_ANSWER: u32 = 0xf0;
const SK_ZERO: [u8; 0x4000] = [0; 0x4000];

fn sk_ptr(base: u32, size: u32, i: Option<usize>) -> u32 {
    i.map_or(0, |i| base + size * i as u32)
}

fn sk_index(base: u32, size: u32, a: u32) -> Option<usize> {
    (a != 0).then(|| ((a - base) / size) as usize)
}

fn sk_vf(ee: &ps2_oracle::Ee, at: u32) -> Vf {
    Vf::from_bits(got(ee, at))
}

fn sk_f(ee: &ps2_oracle::Ee, at: u32) -> F {
    F(f32::from_bits(ee.u32_at(at)))
}

/// A list the original reads through a holder: a zero word either side of the entries, so the entry
/// before the first and the one after the last read as no target. Gives the entries' address.
fn sk_list(ee: &mut ps2_oracle::Ee, bump: &mut u32, items: &[Option<usize>]) -> u32 {
    let base = *bump;
    ee.set_u32(base, 0);
    for (k, t) in items.iter().enumerate() {
        ee.set_u32(base + 4 + 4 * k as u32, sk_ptr(SK_TARGETS, SK_TARGET_SIZE, *t));
    }
    ee.set_u32(base + 4 + 4 * items.len() as u32, 0);
    *bump += (8 + 4 * items.len() as u32 + 15) & !15;
    base + 4
}

fn sk_alloc(bump: &mut u32, n: u32) -> u32 {
    let a = *bump;
    *bump += (n + 15) & !15;
    a
}

fn sk_leg(ee: &mut ps2_oracle::Ee, at: u32, l: &sk::Leg) {
    put(ee, at, l.velocity);
    put(ee, at + 0x10, l.aim);
    put(ee, at + 0x20, l.point);
    put(ee, at + 0x30, l.facing);
    ee.set_f32(at + 0x4c, l.learnt.0);
}

fn sk_read_leg(ee: &ps2_oracle::Ee, at: u32) -> sk::Leg {
    sk::Leg {
        velocity: sk_vf(ee, at),
        aim: sk_vf(ee, at + 0x10),
        point: sk_vf(ee, at + 0x20),
        facing: sk_vf(ee, at + 0x30),
        learnt: sk_f(ee, at + 0x4c),
    }
}

/// The leg as bits, so -0 and +0 differ.
fn sk_leg_bits(l: &sk::Leg) -> [u32; 17] {
    let mut out = [0; 17];
    for (k, v) in [l.velocity, l.aim, l.point, l.facing].iter().enumerate() {
        out[4 * k..4 * k + 4].copy_from_slice(&v.to_bits());
    }
    out[16] = l.learnt.0.to_bits();
    out
}

fn sk_seeker_bits(s: &sk::Seeker) -> Vec<(&'static str, u128)> {
    let v = |v: Vf| v.to_bits().iter().enumerate().fold(0u128, |a, (k, b)| a | u128::from(*b) << (32 * k));
    let o = |i: Option<usize>| i.map_or(u128::MAX, |i| i as u128);
    vec![
        ("flags", u128::from(s.flags)),
        ("flags2", u128::from(s.flags2)),
        ("set", o(s.set)),
        ("target", o(s.target)),
        ("last target", o(s.last_target)),
        ("jump", o(s.jump)),
        ("index", u128::from(s.index as u32)),
        ("state", u128::from(s.state as u32)),
        ("tries", u128::from(s.tries as u32)),
        ("tolerance", u128::from(s.tolerance.0.to_bits())),
        ("learnt", u128::from(s.learnt.0.to_bits())),
        ("aim step", u128::from(s.aim_step.0.to_bits())),
        ("speed step", u128::from(s.speed_step.0.to_bits())),
        ("learn step", u128::from(s.learn_step.0.to_bits())),
        ("last miss", v(s.last_miss)),
        ("miss", v(s.miss)),
        ("lead", u128::from(s.lead.0.to_bits())),
        ("distance", u128::from(s.distance.0.to_bits())),
        ("follow", u128::from(s.follow)),
        ("aim", v(s.aim)),
        ("heading", v(s.heading)),
        ("speed", u128::from(s.speed.0.to_bits())),
    ]
}

fn sk_write_seeker(ee: &mut ps2_oracle::Ee, s: &sk::Seeker) {
    ee.write(SK_AI + 0x30, 8, u128::from(s.flags));
    ee.write(SK_AI + 0x38, 8, u128::from(s.flags2));
    ee.set_u32(SK_AI + 0x340, sk_ptr(SK_SETS, SK_SET_SIZE, s.set));
    ee.set_u32(SK_AI + 0x344, sk_ptr(SK_TARGETS, SK_TARGET_SIZE, s.target));
    ee.set_u32(SK_AI + 0x348, sk_ptr(SK_TARGETS, SK_TARGET_SIZE, s.last_target));
    ee.set_u32(SK_AI + 0x34c, sk_ptr(SK_JUMPS, SK_JUMP_SIZE, s.jump));
    ee.set_u32(SK_AI + 0x350, s.index as u32);
    ee.set_u32(SK_AI + 0x354, s.state as u32);
    ee.set_u32(SK_AI + 0x358, s.tries as u32);
    for (at, f) in [
        (0x35c, s.tolerance),
        (0x360, s.learnt),
        (0x364, s.aim_step),
        (0x368, s.speed_step),
        (0x36c, s.learn_step),
        (0x25c, s.lead),
        (0x2fc, s.distance),
        (0x1e0, s.speed),
    ] {
        ee.set_f32(SK_AI + at, f.0);
    }
    put(ee, SK_AI + 0x370, s.last_miss);
    put(ee, SK_AI + 0x380, s.miss);
    ee.set_u32(SK_AI + 0x220, s.follow);
    put(ee, SK_AI + 0x1b0, s.aim);
    put(ee, SK_AI + 0x1d0, s.heading);
}

fn sk_read_seeker(ee: &ps2_oracle::Ee) -> sk::Seeker {
    sk::Seeker {
        flags: ee.read(SK_AI + 0x30, 8) as u64,
        flags2: ee.read(SK_AI + 0x38, 8) as u64,
        set: sk_index(SK_SETS, SK_SET_SIZE, ee.u32_at(SK_AI + 0x340)),
        target: sk_index(SK_TARGETS, SK_TARGET_SIZE, ee.u32_at(SK_AI + 0x344)),
        last_target: sk_index(SK_TARGETS, SK_TARGET_SIZE, ee.u32_at(SK_AI + 0x348)),
        jump: sk_index(SK_JUMPS, SK_JUMP_SIZE, ee.u32_at(SK_AI + 0x34c)),
        index: ee.u32_at(SK_AI + 0x350) as i32,
        state: ee.u32_at(SK_AI + 0x354) as i32,
        tries: ee.u32_at(SK_AI + 0x358) as i32,
        tolerance: sk_f(ee, SK_AI + 0x35c),
        learnt: sk_f(ee, SK_AI + 0x360),
        aim_step: sk_f(ee, SK_AI + 0x364),
        speed_step: sk_f(ee, SK_AI + 0x368),
        learn_step: sk_f(ee, SK_AI + 0x36c),
        last_miss: sk_vf(ee, SK_AI + 0x370),
        miss: sk_vf(ee, SK_AI + 0x380),
        lead: sk_f(ee, SK_AI + 0x25c),
        distance: sk_f(ee, SK_AI + 0x2fc),
        follow: ee.u32_at(SK_AI + 0x220),
        aim: sk_vf(ee, SK_AI + 0x1b0),
        heading: sk_vf(ee, SK_AI + 0x1d0),
        speed: sk_f(ee, SK_AI + 0x1e0),
    }
}

/// Lays the world out the original's way: the targets with their done flags, the jumps, the groups,
/// the sets with their pick lists, courses and owners, and behind the root pointer the set list
/// (`gaps` null entries before each set, and `gaps[sets]` after), the group list and the sequence.
fn sk_write_world(ee: &mut ps2_oracle::Ee, w: &sk::World, gaps: &[u32], seq_on: u32, seq_root: u32) {
    for i in 0..SK_GROUP_COUNT as u32 {
        ee.load(SK_GROUPS + SK_GROUP_SIZE * i, &SK_ZERO[..SK_GROUP_SIZE as usize]);
    }
    for i in 0..SK_SET_COUNT as u32 {
        ee.load(SK_OWNERS + SK_OWNER_SIZE * i, &SK_ZERO[..SK_OWNER_SIZE as usize]);
    }
    let mut bump = SK_LISTS;
    for (i, g) in w.groups.iter().enumerate() {
        let at = SK_GROUPS + SK_GROUP_SIZE * i as u32;
        if let Some(list) = &g.targets {
            let l = sk_alloc(&mut bump, 0x10);
            let items = sk_list(ee, &mut bump, list);
            ee.set_u32(l + 4, list.len() as u32);
            ee.set_u32(l + 0xc, items);
            ee.set_u32(at + 0x48, l);
        }
        // the open bit is bit 0 of a byte per player; the other bits must not count
        let open = sk_alloc(&mut bump, 0x10);
        for (p, o) in g.open.iter().enumerate() {
            ee.write(open + p as u32, 1, if *o { 0xa5 } else { 0x5a });
        }
        ee.set_u32(at + 0x44, open);
        put(ee, at + 0x50, g.point);
        // in play is the word 1 exactly
        let word = if g.active { 1 } else { [0, 2, 0xffff_ffff, 0x101][i % 4] };
        ee.set_u32(at + 0x78, word);
    }
    for (i, t) in w.targets.iter().enumerate() {
        let at = SK_TARGETS + SK_TARGET_SIZE * i as u32;
        ee.set_u32(at + 0x3c, sk_ptr(SK_JUMPS, SK_JUMP_SIZE, t.jump));
        ee.set_u32(at + 0x40, at + 0x80);
        for (p, done) in t.done.iter().enumerate() {
            // other bits of the word around the done bit, which must not count
            let word = if *done { 0x2 | 0x4d } else { 0x4d };
            ee.write(at + 0x80 + 0x30 * p as u32 + 0x20, 2, word);
        }
    }
    for (i, j) in w.jumps.iter().enumerate() {
        let at = SK_JUMPS + SK_JUMP_SIZE * i as u32;
        sk_leg(ee, at, &j.take_off);
        sk_leg(ee, at + 0x60, &j.landing);
        sk_leg(ee, at + 0xc0, &j.setup);
    }
    for (i, s) in w.sets.iter().enumerate() {
        let at = SK_SETS + SK_SET_SIZE * i as u32;
        ee.set_u32(at + 0x11c, sk_ptr(SK_GROUPS, SK_GROUP_SIZE, s.picks));
        // the owner, its model's root node two pointers down, and its answer to the flag question
        let o = SK_OWNERS + SK_OWNER_SIZE * i as u32;
        ee.set_u32(o + 8, SK_OWNER_VT);
        ee.set_u32(o + 0x30, s.owner.player as u32);
        ee.set_u32(o + 0x34, s.owner.kind as u32);
        ee.set_u32(o + 0x3c, o + 0x100);
        ee.set_u32(o + 0x100, o + 0x120);
        ee.set_u32(o + 0x120 + 0x18, o + 0x140);
        put(ee, o + 0x140 + 0x70, s.owner.position);
        // a yes with only high bits set still counts: the original tests all 64
        let yes = if i == 1 { 1u128 << 40 } else { 1 };
        ee.write(o + SK_OWNER_ANSWER, 8, if s.owner.hit_freeride { yes } else { 0 });
        ee.set_u32(at + 0x54, o);
        match &s.course {
            Some(c) => {
                let holder = sk_alloc(&mut bump, 0xa0);
                let l = sk_alloc(&mut bump, 0x10);
                let items = sk_list(ee, &mut bump, &c.targets);
                ee.set_u32(holder + 0x48, l);
                ee.set_f32(holder + 0x94, c.chance.0);
                ee.set_u32(l + 4, c.targets.len() as u32);
                ee.set_u32(l + 0xc, items);
                ee.set_u32(at + 0x114, holder);
            }
            None => ee.set_u32(at + 0x114, 0),
        }
    }
    let x = sk_alloc(&mut bump, 0x50);
    let y = sk_alloc(&mut bump, 0x60);
    let mut entries = Vec::new();
    for i in 0..=w.sets.len() {
        entries.extend((0..gaps[i]).map(|_| 0));
        if i < w.sets.len() {
            entries.push(SK_SETS + SK_SET_SIZE * i as u32);
        }
    }
    let list = sk_alloc(&mut bump, 4 * entries.len() as u32);
    for (k, e) in entries.iter().enumerate() {
        ee.set_u32(list + 4 * k as u32, *e);
    }
    ee.set_u32(x + 0x30, entries.len() as u32);
    ee.set_u32(x + 0x38, list);
    let groups = sk_alloc(&mut bump, 4 * w.groups.len() as u32);
    for i in 0..w.groups.len() {
        ee.set_u32(groups + 4 * i as u32, SK_GROUPS + SK_GROUP_SIZE * i as u32);
    }
    ee.set_u32(x + 0x44, y);
    ee.set_u32(y + 0x30, w.groups.len() as u32);
    ee.set_u32(y + 0x38, groups);
    ee.set_u32(seq_root, x);
    match &w.sequence {
        Some(seq) => {
            let z = sk_alloc(&mut bump, 0x50);
            let g = sk_alloc(&mut bump, 0x40);
            let items = sk_list(ee, &mut bump, seq);
            ee.set_u32(y + 0x50, z);
            ee.set_u32(z + 0x48, g);
            ee.set_u32(g + 0x30, seq.len() as u32);
            ee.set_u32(g + 0x38, items);
            ee.set_u32(seq_on, 1);
        }
        None => ee.set_u32(seq_on, 0),
    }
}

/// Every set's pick list as the original left it.
fn sk_read_picks(ee: &ps2_oracle::Ee, n: usize) -> Vec<Option<usize>> {
    (0..n as u32).map(|i| sk_index(SK_GROUPS, SK_GROUP_SIZE, ee.u32_at(SK_SETS + SK_SET_SIZE * i + 0x11c))).collect()
}

/// The map's groups and every set's owner, around `near`: the owners and the groups' points close
/// enough together that the nearest is often a near thing, and now and then two points the same.
fn sk_gen_groups(p: &mut Pick, targets: &[sk::Target], near: Vf) -> Vec<sk::Group> {
    let n = p.below(SK_GROUP_COUNT as u32 + 1) as usize;
    let mut groups: Vec<sk::Group> = (0..n)
        .map(|_| sk::Group {
            targets: (!p.chance(6)).then(|| sk_gen_list(p, targets, 6, false, 4)),
            point: {
                let r = if p.chance(2) { 40.0 } else { 300.0 };
                sk_offset(near, p.vec_w(r))
            },
            active: p.below(10) < 7,
            open: (0..p.below(5)).map(|_| p.chance(2)).collect(),
        })
        .collect();
    if n >= 2 && p.chance(6) {
        groups[n - 1].point = groups[0].point;
    }
    groups
}

fn sk_gen_owner(p: &mut Pick, near: Vf) -> sk::Owner {
    sk::Owner {
        kind: [0, 1, 1, 1, 2, -1][p.below(6) as usize],
        player: p.below(5) as usize,
        position: {
            let r = if p.chance(2) { 40.0 } else { 300.0 };
            sk_offset(near, p.vec_w(r))
        },
        hit_freeride: p.chance(2),
    }
}

fn sk_gen_gaps(p: &mut Pick) -> Vec<u32> {
    (0..=SK_SET_COUNT).map(|_| if p.chance(5) { 1 + p.below(2) as u32 } else { 0 }).collect()
}

fn sk_write_ride(ee: &mut ps2_oracle::Ee, r: &sk::Ride) {
    put(ee, SK_AI + 0xc0, r.position);
    put(ee, SK_AI + 0x290, r.last_position);
    put(ee, SK_AI + 0xb0, r.forward);
    ee.set_f32(SK_AI + 0x80, r.speed.0);
    ee.set_f32(SK_AI + 0x254, r.min_speed.0);
    ee.set_f32(SK_AI + 0x210, r.reach.base.0);
    ee.set_f32(SK_AI + 0x214, r.reach.max.0);
    ee.set_f32(SK_AI + 0x218, r.reach.gain.0);
    ee.set_u32(SK_AI + 0x40, SK_IFACE);
    ee.set_f32(SK_IFACE + 0x110, r.speed_scale.0);
    ee.set_f32(SK_IFACE + 0x114, r.slow_radius.0);
    ee.set_f32(SK_IFACE + 0x118, r.slow_speed.0);
    ee.set_u32(SK_IFACE + 0x1e4, r.player as u32);
    match r.race_phase {
        Some(phase) => {
            ee.set_u32(SK_RACE + 0x60, phase as u32);
            ee.set_u32(SK_AI + 0xd8, SK_RACE);
        }
        None => ee.set_u32(SK_AI + 0xd8, 0),
    }
    match r.bike {
        Some(b) => {
            ee.write(SK_BIKE + 8, 8, u128::from(b.flags));
            ee.set_f32(SK_BIKE + 0xfc, b.settle.0);
            ee.set_u32(SK_AI + 0xd4, SK_BIKE);
        }
        None => ee.set_u32(SK_AI + 0xd4, 0),
    }
    // the path net is only tested for being there: the branch it starts is hooked
    ee.set_u32(SK_AI + 0xdc, if r.path_net { SK_RACE } else { 0 });
}

/// `v` scaled to a length, on the host: generated data only.
fn sk_along(v: Vf, len: f32) -> Vf {
    let l = (v.x() * v.x() + v.z() * v.z()).sqrt().max(1e-3);
    Vf::new(v.x() / l * len, v.y(), v.z() / l * len, v.w())
}

fn sk_offset(a: Vf, b: Vf) -> Vf {
    Vf::new(a.x() + b.x(), a.y() + b.y(), a.z() + b.z(), a.w())
}

fn sk_gen_jump(p: &mut Pick) -> sk::Jump {
    let setup = Vf::new(p.range(-300.0, 300.0), p.range(-20.0, 20.0), p.range(-300.0, 300.0), p.range(-2.0, 2.0));
    let run = sk_along(p.vec(1.0, 0.0), p.range(30.0, 80.0));
    let lip = sk_offset(Vf::new(setup.x(), p.range(-20.0, 30.0), setup.z(), p.range(-2.0, 2.0)), run);
    let fly = sk_along(sk_offset(sk_along(run, 1.0), p.vec(0.4, 0.0)), p.range(40.0, 120.0));
    let landing = sk_offset(Vf::new(lip.x(), p.range(-20.0, 20.0), lip.z(), p.range(-2.0, 2.0)), fly);
    let velocity = match p.below(5) {
        0 => Vf::new(0.0, 0.0, 0.0, p.range(-1.0, 1.0)),
        1 => p.vec_w(150.0),
        _ => sk_along(Vf::new(fly.x(), p.range(-5.0, 20.0), fly.z(), p.range(-1.0, 1.0)), p.range(10.0, 160.0)),
    };
    let mut aim = sk_offset(lip, p.vec(10.0, 0.0));
    match p.below(10) {
        0 => aim = Vf::new(0.0, aim.y(), aim.z(), aim.w()),
        1 => aim = Vf::new(aim.x(), aim.y(), -0.0, aim.w()),
        _ => {}
    }
    let facing = |p: &mut Pick, along: Vf| {
        if p.chance(4) { p.vec_w(1.0) } else { sk_along(along, p.range(0.5, 1.5)) }
    };
    sk::Jump {
        take_off: sk::Leg {
            velocity,
            aim,
            point: lip,
            facing: facing(p, run),
            learnt: F(if p.chance(6) { 100.0 } else { p.range(-20.0, 115.0) }),
        },
        landing: sk::Leg {
            velocity: if p.chance(4) { Vf::new(0.0, 0.0, 0.0, 0.0) } else { p.vec_w(120.0) },
            aim: p.vec_w(300.0),
            point: landing,
            facing: facing(p, fly),
            learnt: F(p.range(-10.0, 110.0)),
        },
        setup: sk::Leg {
            velocity: if p.chance(3) { p.vec(100.0, 0.0) } else { Vf::new(0.0, 0.0, 0.0, 0.0) },
            aim: p.vec_w(300.0),
            point: setup,
            facing: facing(p, run),
            learnt: F(p.range(-10.0, 110.0)),
        },
    }
}

fn sk_gen_list(p: &mut Pick, targets: &[sk::Target], max: u32, with_jump: bool, none_in: u32) -> Vec<Option<usize>> {
    let ok: Vec<usize> =
        (0..targets.len()).filter(|&t| !with_jump || targets[t].jump.is_some()).collect();
    (0..p.below(max + 1))
        .map(|_| {
            if ok.is_empty() || p.chance(none_in) { None } else { Some(ok[p.below(ok.len() as u32) as usize]) }
        })
        .collect()
}

fn sk_gen(p: &mut Pick) -> (sk::Seeker, sk::Ride, sk::World, Vec<u32>) {
    let jumps: Vec<sk::Jump> = (0..SK_JUMP_COUNT).map(|_| sk_gen_jump(p)).collect();
    let mut targets: Vec<sk::Target> = (0..SK_TARGET_COUNT)
        .map(|_| sk::Target {
            jump: if p.chance(7) { None } else { Some(p.below(SK_JUMP_COUNT as u32) as usize) },
            done: (0..4).map(|_| p.chance(4)).collect(),
        })
        .collect();
    if targets.iter().all(|t| t.jump.is_none()) {
        targets[0].jump = Some(0);
    }
    let near = p.vec_w(300.0);
    let groups = sk_gen_groups(p, &targets, near);
    let sets: Vec<sk::Set> = (0..SK_SET_COUNT)
        .map(|_| sk::Set {
            picks: if groups.is_empty() || p.chance(3) { None } else { Some(p.below(groups.len() as u32) as usize) },
            owner: sk_gen_owner(p, near),
            course: (p.below(10) < 3).then(|| sk::Course {
                targets: sk_gen_list(p, &targets, 6, true, 7),
                chance: F(match p.below(4) {
                    0 => 0.0,
                    1 => 1.0,
                    _ => p.fraction(),
                }),
            }),
        })
        .collect();
    let gaps = sk_gen_gaps(p);
    let sequence = p.chance(4).then(|| {
        let mut seq = sk_gen_list(p, &targets, 6, true, 3);
        if seq.iter().all(|t| t.is_none()) {
            let t = (0..SK_TARGET_COUNT).find(|&t| targets[t].jump.is_some());
            seq.push(t);
        }
        seq
    });
    let world = sk::World { sets, targets, jumps, sequence, groups };

    let sequenced = world.sequence.is_some();
    let set = if sequenced || !p.chance(10) { Some(p.below(SK_SET_COUNT as u32) as usize) } else { None };
    let state = [0, 1, 2, 3, 3, 3][p.below(6) as usize];
    // a rider in the air mostly has its target
    let keep = if state == 3 { 15 } else { 9 };
    let target = if set.is_none() || p.below(20) >= keep { None } else { Some(p.below(SK_TARGET_COUNT as u32) as usize) };
    let jump = match (target.and_then(|t| world.targets[t].jump), p.below(8)) {
        (Some(j), 0..=4) => Some(j),
        (_, 7) => None,
        _ => Some(p.below(SK_JUMP_COUNT as u32) as usize),
    };
    let index = match &world.sequence {
        Some(seq) => p.below(seq.len() as u32),
        None => p.below(8),
    };
    let mut flags = 0u64;
    for (bit, odds) in [(flag::ON_GROUND, 6), (sk::CLOSING, 5), (sk::NO_LEARN, 2), (0x1, 5), (0x80, 5), (0x1_0000_0000, 5)] {
        if p.below(10) < odds {
            flags |= bit;
        }
    }
    let mut flags2 = 0u64;
    for (bit, odds) in [(sk::HEADLONG, 3), (sk::LEG_DONE, 5), (0x1, 5), (0x2_0000_0000, 5)] {
        if p.below(10) < odds {
            flags2 |= bit;
        }
    }
    let last_miss = if p.below(10) < 3 {
        sk::MISS_START
    } else {
        let y = match p.below(4) {
            0 | 1 => 0.0,
            2 => -p.range(0.0, 2.0),
            _ => p.range(0.0, 2.0),
        };
        Vf::new(p.range(-40.0, 40.0), y, p.range(-40.0, 40.0), p.range(-1.0, 1.0))
    };
    let seeker = sk::Seeker {
        flags,
        flags2,
        set,
        target,
        last_target: if p.chance(3) { None } else { Some(p.below(SK_TARGET_COUNT as u32) as usize) },
        jump,
        index,
        state,
        tries: if p.chance(3) { p.below(12) } else { p.below(3) },
        tolerance: F(if p.chance(2) { p.range(0.0, 40.0) } else { p.range(0.0, 8.0) }),
        learnt: F(p.range(-10.0, 110.0)),
        aim_step: F(if p.chance(3) { 0.05 } else { p.range(0.0, 0.5) }),
        speed_step: F(p.range(0.0, 20.0)),
        learn_step: F(p.range(0.0, 20.0)),
        last_miss,
        miss: p.vec_w(50.0),
        lead: F(p.range(0.0, 80.0)),
        distance: F(p.range(0.0, 300.0)),
        follow: p.next() as u32,
        aim: p.vec(300.0, 1.0),
        heading: p.vec(1.0, 0.0),
        speed: F(p.range(0.0, 150.0)),
    };

    // the rider somewhere that sets a state off
    let j = &world.jumps[jump.unwrap_or(p.below(SK_JUMP_COUNT as u32) as usize)];
    let run = sk_along(Vf::new(j.take_off.point.x() - j.setup.point.x(), 0.0, j.take_off.point.z() - j.setup.point.z(), 0.0), 1.0);
    // half the time where the rider's own state moves on
    let kind = match (p.chance(2), seeker.state) {
        (true, 1) => 3,
        (true, 2) => 1,
        (true, 3) => 2,
        _ => p.below(6),
    };
    let near = if p.chance(2) { 12.0 } else { 30.0 };
    let position = match kind {
        0 => sk_offset(j.setup.point, p.vec(25.0, 0.0)),
        1 => sk_offset(j.take_off.point, p.vec(near, 0.0)),
        2 => sk_offset(j.landing.point, p.vec(near, 0.0)),
        3 => sk_offset(j.setup.point, sk_offset(sk_along(run, p.range(-5.0, 25.0)), p.vec(4.0, 0.0))),
        _ => p.vec_w(300.0),
    };
    let step = if p.chance(2) { sk_along(j.landing.facing, p.range(-5.0, 15.0)) } else { p.vec(10.0, 0.0) };
    let mut last_position = Vf::new(position.x() - step.x(), position.y() - step.y(), position.z() - step.z(), p.range(-1.0, 1.0));
    if p.chance(10) {
        // on the landing point: the crossing test's "before" is exactly 0
        last_position = j.landing.point;
    }
    let forward = if p.chance(2) { sk_offset(run, p.vec(0.5, 0.0)) } else { p.vec(1.0, 0.0) };
    let race_phase = (p.below(10) < 3).then(|| [4, 0, 2][p.below(3) as usize]);
    let mut bike = (!p.chance(7)).then(|| sk::Mount {
        flags: [0x0, 0x2, 0x100, 0x102][p.below(4) as usize] | if p.chance(2) { 0x8_0001 } else { 0 },
        settle: F(if p.chance(5) { 0.5 } else { p.range(0.2, 1.0) }),
    });
    if race_phase == Some(4) && bike.is_none() {
        bike = Some(sk::Mount { flags: 0x100, settle: F(1.0) });
    }
    let ride = sk::Ride {
        position,
        last_position,
        forward,
        speed: F(p.range(0.0, 100.0)),
        min_speed: F(p.range(0.0, 60.0)),
        speed_scale: F(match p.below(10) {
            0 => 0.0,
            1..=5 => 1.0,
            _ => p.range(0.0, 2.0),
        }),
        slow_radius: F(p.range(0.0, 150.0)),
        slow_speed: F(p.range(0.0, 60.0)),
        player: p.below(4) as usize,
        reach: if p.chance(2) {
            sk::Reach { base: F(10.0), max: F(60.0), gain: F(0.2) }
        } else {
            sk::Reach { base: F(p.range(0.0, 30.0)), max: F(p.range(0.0, 80.0)), gain: F(p.range(0.0, 1.0)) }
        },
        race_phase,
        bike,
        path_net: p.chance(7),
    };
    (seeker, ride, world, gaps)
}

fn sk_controls() -> ai::Controls {
    let z = F(0.0);
    ai::Controls {
        steer: F(0.25),
        pitch: F(-0.5),
        throttle: F(0.75),
        brake: F(0.125),
        heading: z,
        aim: z,
        roll: z,
        flat_forward: Vf::new(0.0, 0.0, 0.0, 0.0),
        target_speed: z,
    }
}

/// The name registry (it allocates for a name it has not seen) hands out [`SK_FLAG_BIT`], and the
/// owners' vtable answers the flag question from [`SK_OWNER_ANSWER`]; every other owner slot panics.
fn sk_owner_hooks(ee: &mut ps2_oracle::Ee, name_bit: u32) {
    ee.hooks.insert(name_bit, Box::new(|ee| ee.set(2, SK_FLAG_BIT)));
    for slot in (0..0x100).step_by(4) {
        let h = SK_OWNER_HOOKS + slot;
        if slot == 0xac {
            ee.hooks.insert(h, Box::new(|ee| {
                assert_eq!((ee.r(5), ee.r(6)), (SK_FLAG_BIT, 0), "the flag question's arguments");
                let answer = ee.read(ee.r(4) as u32 + SK_OWNER_ANSWER, 8) as u64;
                ee.set(2, answer);
            }));
        } else {
            ee.hooks.insert(h, Box::new(move |_| panic!("the build called the owner's vtable slot {slot:#x}")));
        }
        ee.set_u32(SK_OWNER_VT + slot, h);
    }
}

/// Sets up the hooks and the vtable for the seeker; gives (ai_tick_flag, rand_state, sequence word,
/// sequence root).
fn sk_setup(ee: &mut ps2_oracle::Ee) -> Option<[u32; 4]> {
    let at = named(&[
        "ai_tick_flag",
        "rand_state",
        "ai_seek_reach",
        "ai_seek_hand_off",
        "name_bit",
        "ai_seek_path_branch",
        "ai_seek_tail",
        "ai_seek_sequence_on",
        "ai_seek_sequence_root",
    ])?;
    let [tick, rand_state, reach, hand_off, name_bit, path, tail, seq_on, seq_root] = at[..] else { unreachable!() };
    for slot in (0..0x100).step_by(4) {
        let target = match slot {
            0x80 => reach,
            0x94 => hand_off,
            _ => {
                let h = SK_HOOKS + slot;
                ee.hooks.insert(h, Box::new(move |_| panic!("the seeker called vtable slot {slot:#x}")));
                h
            }
        };
        ee.set_u32(SK_VT + slot, target);
    }
    sk_owner_hooks(ee, name_bit);
    ee.hooks.insert(path, Box::new(|ee| {
        ee.set(2, SK_PATH);
        ee.set(31, ps2_oracle::DONE as i32 as i64 as u64);
    }));
    ee.hooks.insert(tail, Box::new(|ee| {
        ee.set(2, SK_TAIL);
        ee.set(31, ps2_oracle::DONE as i32 as i64 as u64);
    }));
    Some([tick, rand_state, seq_on, seq_root])
}

#[test]
fn the_target_seeker_matches_over_ten_thousand_ticks() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some([tick, rand_state, seq_on, seq_root]) = sk_setup(&mut ee) else { return };

    let mut p = Pick(0x5282_a7a2_93e7_5eed);
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    for case in 0..10_000 {
        let (mut seeker, ride, mut world, gaps) = sk_gen(&mut p);
        let state = p.next();
        let before = (seeker, world.clone());

        for (base, len) in [(SK_AI, 0x1000), (SK_LISTS, 0x4000), (SK_RACE, 0x200), (SK_BIKE, 0x200)] {
            ee.load(base, &SK_ZERO[..len]);
        }
        for i in 0..SK_TARGET_COUNT as u32 {
            ee.load(SK_TARGETS + SK_TARGET_SIZE * i, &SK_ZERO[..SK_TARGET_SIZE as usize]);
        }
        ee.set_u32(SK_AI + 8, SK_VT);
        sk_write_world(&mut ee, &world, &gaps, seq_on, seq_root);
        sk_write_ride(&mut ee, &ride);
        sk_write_seeker(&mut ee, &seeker);
        let mut out = sk_controls();
        for (at, f) in [(0xf0, out.steer), (0xf4, out.pitch), (0xe0, out.throttle), (0xe4, out.brake)] {
            ee.set_f32(SK_AI + at, f.0);
        }
        ee.write(rand_state, 8, u128::from(state));

        ee.call(tick, &[u64::from(SK_AI)], &[1.0 / 30.0], BUDGET);

        let mut rng = Rng::from_state(state);
        let how = sk::seek(&mut seeker, &ride, &mut world, &mut rng, &mut out);
        let what = format!("case {case}: {how:?}, state {} -> {}", before.0.state, seeker.state);
        let want = match how {
            sk::Seek::Still => 1,
            sk::Seek::Lost => 0,
            sk::Seek::Path => SK_PATH,
            sk::Seek::Steered => SK_TAIL,
        };
        assert_eq!(ee.v0() as u32 as u64, want, "{what}: how it ended");
        let theirs = sk_read_seeker(&ee);
        for ((name, a), (_, b)) in sk_seeker_bits(&theirs).iter().zip(sk_seeker_bits(&seeker).iter()) {
            assert_eq!(a, b, "{what}: {name}: original {a:#x} port {b:#x}");
        }
        for (at, f, name) in [(0xf0, out.steer, "steer"), (0xf4, out.pitch, "pitch"), (0xe0, out.throttle, "throttle"), (0xe4, out.brake, "brake")] {
            assert_eq!(ee.u32_at(SK_AI + at), f.0.to_bits(), "{what}: {name}");
        }
        for (i, j) in world.jumps.iter().enumerate() {
            let at = SK_JUMPS + SK_JUMP_SIZE * i as u32;
            for (off, leg, name) in [(0, &j.take_off, "take-off"), (0x60, &j.landing, "landing"), (0xc0, &j.setup, "setup")] {
                assert_eq!(sk_leg_bits(&sk_read_leg(&ee, at + off)), sk_leg_bits(leg), "{what}: jump {i} {name}");
            }
        }
        assert_eq!(ee.read(rand_state, 8) as u64, rng.state(), "{what}: the generator");
        let picks: Vec<_> = world.sets.iter().map(|s| s.picks).collect();
        assert_eq!(sk_read_picks(&ee, world.sets.len()), picks, "{what}: the pick lists");

        // what the tick went through
        let (b, bw) = &before;
        let mut count = |k: &'static str| *seen.entry(k).or_default() += 1;
        count(match how {
            sk::Seek::Still => "still",
            sk::Seek::Lost => "lost",
            sk::Seek::Path => "path",
            sk::Seek::Steered => "steered",
        });
        if rng.state() != state {
            count("drew");
        }
        if seeker.target != b.target && seeker.target.is_some() {
            count(if bw.sequence.is_some() {
                "picked from the sequence"
            } else if b.set.is_some_and(|s| bw.sets[s].course.is_some()) {
                "picked off the course"
            } else {
                "picked off the list"
            });
        }
        match (b.state, seeker.state) {
            (1, 2) => count("1 -> 2"),
            (2, 3) => count("2 -> 3"),
            (3, 0) if b.target.is_some() && seeker.target.is_none() => count("3: dropped"),
            (3, 1) if world.jumps.iter().zip(&bw.jumps).any(|(a, c)| a.take_off.velocity != c.take_off.velocity) => {
                count("3: learnt")
            }
            (3, 1) if seeker.flags2 & sk::LEG_DONE != 0 && b.flags2 & sk::LEG_DONE == 0 => count("3: landed"),
            (3, 1) => count("3 -> 1"),
            _ => {}
        }
        if seeker.speed == F(150.0) && how == sk::Seek::Steered && seeker.state != 0 {
            count("flat out");
        }
        if seeker.flags & sk::CLOSING != b.flags & sk::CLOSING {
            count("closing flipped");
        }
        let halved = seeker.aim_step.0 != b.aim_step.0 || seeker.speed_step.0 != b.speed_step.0;
        if halved && seeker.state == 1 {
            count("steps halved");
        }
        let moved = |a: &sk::Jump, c: &sk::Jump| {
            a.take_off.aim != c.take_off.aim && c.take_off.aim.x() != 0.0 && c.take_off.aim.z() != 0.0
        };
        if b.state == 3 && world.jumps.iter().zip(&bw.jumps).any(|(a, c)| moved(a, c)) {
            count("lip re-aimed");
        }
        if world.sets.iter().zip(&bw.sets).any(|(a, c)| a.picks != c.picks) {
            count(if world.sets[0].picks.is_some() { "built" } else { "built none" });
        }
    }
    eprintln!("target seeker: {seen:?}");
    for (k, n) in [
        ("still", 500),
        ("lost", 200),
        ("path", 200),
        ("steered", 3000),
        ("picked from the sequence", 200),
        ("picked off the course", 200),
        ("picked off the list", 200),
        ("1 -> 2", 100),
        ("2 -> 3", 100),
        ("3: dropped", 100),
        ("3: learnt", 100),
        ("3 -> 1", 50),
        ("flat out", 500),
        ("closing flipped", 500),
        ("steps halved", 30),
        ("lip re-aimed", 100),
        ("built", 100),
        ("built none", 20),
    ] {
        assert!(seen.get(k).copied().unwrap_or(0) >= n, "only {:?} ticks went through {k}", seen.get(k));
    }
}

// The pick-list build (S306) on its own: the sets' owners and the map's groups generated around one
// spot, so the nearest group is often a near thing, with null entries in the set list, groups out of
// play or closed to the player, an owner that asks for open groups and one that does not, no owner of
// kind 1, no groups, and two groups at the same point. The original's build is called on the root;
// every set's pick list is compared.
#[test]
fn the_pick_list_build_matches_over_ten_thousand_worlds() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_seek_list_build", "name_bit", "ai_seek_sequence_on", "ai_seek_sequence_root"]) else { return };
    let [build, name_bit, seq_on, seq_root] = at[..] else { unreachable!() };
    sk_owner_hooks(&mut ee, name_bit);

    let mut p = Pick(0x5306_b1d1_57a7_e5ed);
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    for case in 0..10_000 {
        let targets: Vec<sk::Target> = (0..SK_TARGET_COUNT)
            .map(|t| sk::Target { jump: Some(t % SK_JUMP_COUNT), done: Vec::new() })
            .collect();
        let near = p.vec_w(300.0);
        let groups = sk_gen_groups(&mut p, &targets, near);
        let sets: Vec<sk::Set> = (0..SK_SET_COUNT)
            .map(|_| sk::Set {
                picks: if groups.is_empty() || p.chance(2) { None } else { Some(p.below(groups.len() as u32) as usize) },
                course: None,
                owner: sk_gen_owner(&mut p, near),
            })
            .collect();
        let gaps = sk_gen_gaps(&mut p);
        let mut world = sk::World { sets, targets, jumps: Vec::new(), sequence: None, groups };

        ee.load(SK_LISTS, &SK_ZERO);
        sk_write_world(&mut ee, &world, &gaps, seq_on, seq_root);
        ee.call(build, &[u64::from(ee.u32_at(seq_root))], &[], BUDGET);

        let human = world.sets.iter().find(|s| s.owner.kind == 1).map(|s| s.owner);
        sk::build(&mut world);
        let picks: Vec<_> = world.sets.iter().map(|s| s.picks).collect();
        assert_eq!(sk_read_picks(&ee, world.sets.len()), picks, "case {case}: the pick lists");

        let mut count = |k: &'static str| *seen.entry(k).or_default() += 1;
        count(match (human, picks[0]) {
            (None, _) => "no owner of kind 1",
            (Some(_), None) => "no group",
            (Some(o), Some(_)) if o.hit_freeride => "an open group",
            (Some(_), Some(_)) => "any group in play",
        });
        if let (Some(o), Some(g)) = (human, picks[0]) {
            let skipped_nearer = world.groups.iter().enumerate().any(|(i, x)| {
                i != g && {
                    let d = |v: Vf| (v.x() - o.position.x()).powi(2) + (v.y() - o.position.y()).powi(2) + (v.z() - o.position.z()).powi(2);
                    d(x.point) < d(world.groups[g].point)
                }
            });
            if skipped_nearer {
                count("a nearer group passed over");
            }
        }
    }
    eprintln!("pick-list build: {seen:?}");
    for (k, n) in [
        ("no owner of kind 1", 300),
        ("no group", 300),
        ("an open group", 1000),
        ("any group in play", 1000),
        ("a nearer group passed over", 500),
    ] {
        assert!(seen.get(k).copied().unwrap_or(0) >= n, "only {:?} worlds went through {k}", seen.get(k));
    }
}

// --- S282b: the tick's tail ------------------------------------------------------------------------

use game_sim::ai::follow as fw;

const TL_AI: u32 = 0x0180_0000;
const TL_VT: u32 = 0x0180_1000;
const TL_IFACE: u32 = 0x0180_2000;
const TL_IFACE_VT: u32 = 0x0180_2400;
const TL_BIKE: u32 = 0x0180_3000;
/// Not code: the vtable slots the tail must not call land here, and panic.
const TL_HOOKS: u32 = 0x0186_0000;
const TL_AVOID: u32 = 0x0187_0000;
const TL_PLAN: u32 = 0x0187_0010;
const TL_ASSIST: u32 = 0x0187_0020;

/// What one of the tail's calls did: the brain as it saw it, and what it changed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct TlCall {
    seen: fw::Brain,
    /// Applied by the call (the original's hook writes them, the port's closure too).
    set_heading: Option<Vf>,
    set_speed: Option<F>,
    flip: u64,
}

fn tl_brain(ee: &ps2_oracle::Ee) -> fw::Brain {
    fw::Brain {
        flags: ee.read(TL_AI + 0x30, 8) as u64,
        heading: sk_vf(ee, TL_AI + 0x1d0),
        speed: sk_f(ee, TL_AI + 0x1e0),
        assist_x: sk_f(ee, TL_AI + 0x1c0),
        assist_z: sk_f(ee, TL_AI + 0x1c8),
    }
}

fn tl_brain_bits(b: &fw::Brain) -> [u64; 8] {
    let h = b.heading.to_bits();
    [b.flags, h[0].into(), h[1].into(), h[2].into(), h[3].into(), b.speed.0.to_bits().into(), b.assist_x.0.to_bits().into(), b.assist_z.0.to_bits().into()]
}

fn tl_apply(b: &mut fw::Brain, c: &TlCall) {
    if let Some(h) = c.set_heading {
        b.heading = h;
    }
    if let Some(s) = c.set_speed {
        b.speed = s;
    }
    b.flags ^= c.flip;
}

fn tl_gen_call(p: &mut Pick) -> TlCall {
    let w = p.range(-1.0, 1.0);
    TlCall {
        seen: fw::Brain::default(),
        set_heading: p.chance(2).then(|| p.vec(1.0, w)),
        set_speed: p.chance(2).then(|| F(p.range(0.0, 150.0))),
        flip: if p.chance(3) { [ai::flag::ON_GROUND, fw::ASSIST, fw::AVOIDS, 0x8000][p.below(4) as usize] } else { 0 },
    }
}

/// A unit-ish vector on the ground, `y` small, `w` any.
fn tl_dir(p: &mut Pick) -> Vf {
    let a = p.range(-3.2, 3.2);
    let (s, c) = a.sin_cos();
    let w = p.range(-1.0, 1.0);
    Vf::new(c, p.range(-0.2, 0.2), s, w)
}

#[test]
fn the_ticks_tail_matches_over_ten_thousand_ticks() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_seek_tail", "ai_tick_end"]) else { return };
    let [tail, end] = at[..] else { unreachable!() };
    for slot in (0..0x100).step_by(4) {
        for (vt, name) in [(TL_VT, "brain"), (TL_IFACE_VT, "interface")] {
            let target = match (vt, slot) {
                (TL_VT, 0x74) => TL_AVOID,
                (TL_VT, 0x84) => TL_PLAN,
                (TL_IFACE_VT, 0x18) => TL_ASSIST,
                _ => {
                    let h = TL_HOOKS + slot + if vt == TL_VT { 0 } else { 0x100 };
                    ee.hooks.insert(h, Box::new(move |_| panic!("the tail called the {name}'s vtable slot {slot:#x}")));
                    h
                }
            };
            ee.set_u32(vt + slot, target);
        }
    }
    // The three calls the tail makes: each logs the brain as it saw it and its arguments, then
    // applies the change the case asks of it.
    let todo: Rc<RefCell<[TlCall; 3]>> = Rc::default();
    let log: Rc<RefCell<Vec<(usize, TlCall, Vec<u64>, u32)>>> = Rc::default();
    for (k, addr) in [TL_AVOID, TL_PLAN, TL_ASSIST].into_iter().enumerate() {
        let (todo, log) = (todo.clone(), log.clone());
        ee.hooks.insert(addr, Box::new(move |ee| {
            let mut c = todo.borrow()[k];
            c.seen = tl_brain(ee);
            let args = (4..12).map(|r| ee.r(r) as u32 as u64).collect();
            log.borrow_mut().push((k, c, args, ee.f(12).to_bits()));
            let mut b = c.seen;
            tl_apply(&mut b, &c);
            put(ee, TL_AI + 0x1d0, b.heading);
            ee.set_f32(TL_AI + 0x1e0, b.speed.0);
            ee.write(TL_AI + 0x30, 8, u128::from(b.flags));
            ee.set(2, 1);
        }));
    }
    ee.hooks.insert(end, Box::new(|ee| ee.set(31, ps2_oracle::DONE as i32 as i64 as u64)));

    let mut p = Pick(0x5282_b7a1_1ee7_c0de);
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    for case in 0..10_000 {
        ee.load(TL_AI, &SK_ZERO[..0x1000]);
        ee.load(TL_BIKE, &SK_ZERO[..0x100]);
        ee.load(TL_IFACE, &SK_ZERO[..0x200]);
        ee.set_u32(TL_AI + 8, TL_VT);
        ee.set_u32(TL_IFACE + 8, TL_IFACE_VT);
        ee.set_u32(TL_AI + 0x40, TL_IFACE);

        let mut flags = p.next() & 0xffff_ffff_0000_f000;
        for bit in [fw::AVOIDS, ai::flag::ON_GROUND, ai::flag::NO_RIDER_SWEEP, fw::ASSIST, fw::SWEEP_BY_BIKE] {
            if p.chance(2) {
                flags |= bit;
            }
        }
        let forward = tl_dir(&mut p);
        let heading = match p.below(4) {
            0 => forward,
            1 => tl_dir(&mut p),
            2 => {
                let w = p.range(-1.0, 1.0);
                p.vec(1.5, w)
            }
            _ => {
                // near the 45° edge
                let f = forward;
                let a = p.range(0.70, 0.72);
                let b = (1.0 - a * a).sqrt();
                Vf::new(f.x() * a - f.z() * b, f.y(), f.z() * a + f.x() * b, f.w())
            }
        };
        let velocity = match p.below(4) {
            0 => {
                let w = p.range(-1.0, 1.0);
                p.vec(60.0, w)
            }
            1 => sk_along(heading, p.range(0.0, 60.0)),
            _ => sk_along(tl_dir(&mut p), p.range(0.0, 60.0)),
        };
        let len = (velocity.x() * velocity.x() + velocity.y() * velocity.y() + velocity.z() * velocity.z()).sqrt();
        let own_speed = match p.below(8) {
            0 => 0.0,
            1 => -p.range(0.0, 5.0),
            2 => p.range(0.0, 80.0),
            _ => len,
        };
        let air_cone = match p.below(5) {
            0 => p.range(-1.0, 1.0),
            1 => 1.0,
            _ => p.range(0.3, 1.0),
        };
        let bike_word = match p.below(3) {
            0 => None,
            1 => Some(0),
            _ => Some(p.next() as u32 | 1),
        };
        let ride = fw::Tail { forward, velocity, own_speed: F(own_speed), air_cone: F(air_cone), bike_word };
        let mut brain = fw::Brain {
            flags,
            heading,
            speed: F(p.range(0.0, 150.0)),
            assist_x: F(p.range(-9.0, 9.0)),
            assist_z: F(p.range(-9.0, 9.0)),
        };
        let calls = [tl_gen_call(&mut p), tl_gen_call(&mut p), tl_gen_call(&mut p)];
        *todo.borrow_mut() = calls;
        log.borrow_mut().clear();
        let dt = p.range(0.01, 0.05);

        ee.write(TL_AI + 0x30, 8, u128::from(brain.flags));
        put(&mut ee, TL_AI + 0x1d0, brain.heading);
        ee.set_f32(TL_AI + 0x1e0, brain.speed.0);
        ee.set_f32(TL_AI + 0x1c0, brain.assist_x.0);
        ee.set_f32(TL_AI + 0x1c8, brain.assist_z.0);
        put(&mut ee, TL_AI + 0xb0, ride.forward);
        put(&mut ee, TL_AI + 0x50, ride.velocity);
        ee.set_f32(TL_AI + 0x80, own_speed);
        ee.set_f32(TL_IFACE + 0x150, air_cone);
        ee.set_u32(TL_AI + 0x150, p.next() as u32);
        ee.set_u32(TL_AI + 0x154, p.next() as u32);
        if let Some(w) = bike_word {
            ee.set_u32(TL_AI + 0xd4, TL_BIKE);
            ee.set_u32(TL_BIKE + 0x40, w);
        }
        ee.set(19, u64::from(TL_AI));
        ee.set(3, brain.flags);
        ee.set_f(22, dt);
        ee.call(tail, &[], &[], BUDGET);
        assert_eq!(ee.v0() as u32, 1, "case {case}: the answer");

        let before = brain;
        let mut mine: Vec<(usize, TlCall)> = Vec::new();
        let mut plan_log: Vec<fw::Brain> = Vec::new();
        let mut assist_log: Vec<fw::Brain> = Vec::new();
        fw::tail(
            &mut brain,
            &ride,
            &mut |b| {
                let mut c = calls[0];
                c.seen = *b;
                mine.push((0, c));
                tl_apply(b, &c);
            },
            &mut |b| {
                plan_log.push(*b);
                tl_apply(b, &calls[1]);
            },
            &mut |b| {
                assist_log.push(*b);
                tl_apply(b, &calls[2]);
            },
        );
        let what = format!("case {case}: {before:?} {ride:?} calls {calls:?}");
        let theirs = tl_brain(&ee);
        assert_eq!(tl_brain_bits(&theirs), tl_brain_bits(&brain), "{what}: the brain\n original {theirs:?}\n port     {brain:?}");

        let log = log.borrow();
        let order: Vec<usize> = log.iter().map(|e| e.0).collect();
        let mut want_order: Vec<usize> = mine.iter().map(|e| e.0).collect();
        want_order.extend(std::iter::repeat_n(1, plan_log.len()));
        want_order.extend(std::iter::repeat_n(2, assist_log.len()));
        assert_eq!(order, want_order, "{what}: the calls");
        let mut port_seen = mine.iter().map(|e| e.1.seen).chain(plan_log.iter().copied()).chain(assist_log.iter().copied());
        for (k, c, args, f12) in log.iter() {
            let ours = port_seen.next().unwrap();
            assert_eq!(tl_brain_bits(&c.seen), tl_brain_bits(&ours), "{what}: call {k} saw\n original {:?}\n port     {ours:?}", c.seen);
            match k {
                0 => assert_eq!(
                    args[..],
                    [TL_AI, ee.u32_at(TL_AI + 0x150), ee.u32_at(TL_AI + 0x154), TL_AI + 0xc0, TL_AI + 0x1d0, TL_AI + 0x1d0, TL_AI + 0x1e0, TL_AI + 0x1e0].map(u64::from),
                    "{what}: the avoidance's arguments"
                ),
                1 => {
                    assert_eq!(args[0], u64::from(TL_AI), "{what}: the plan's brain");
                    assert_eq!(*f12, dt.to_bits(), "{what}: the plan's dt");
                }
                _ => {
                    assert_eq!(args[0], u64::from(TL_IFACE), "{what}: the assist's interface");
                    assert_eq!(*f12, dt.to_bits(), "{what}: the assist's dt");
                }
            }
        }

        let mut count = |k: &'static str| *seen.entry(k).or_default() += 1;
        if before.flags & fw::AVOIDS != 0 {
            count("avoids");
            if mine[0].1.seen.heading != before.heading {
                count("turned to 45");
            }
        }
        if (before.flags ^ brain.flags) & ai::flag::NO_RIDER_SWEEP != 0 {
            count("sweep flipped");
        }
        let after_avoid = mine.first().map_or(before, |(_, c)| {
            let mut b = c.seen;
            tl_apply(&mut b, c);
            b
        });
        if plan_log[0].heading != after_avoid.heading {
            count("air cone");
        }
        if !assist_log.is_empty() {
            count("assist");
        }
    }
    eprintln!("tail: {seen:?}");
    for (k, n) in [("avoids", 3000), ("turned to 45", 800), ("sweep flipped", 800), ("air cone", 800), ("assist", 3000)] {
        assert!(seen.get(k).copied().unwrap_or(0) >= n, "only {:?} ticks went through {k}", seen.get(k));
    }
}

// ---------------------------------------------------------------------------------------------------
// S282c: a track's two path nets off the disc, against a race image's, node for node.
// ---------------------------------------------------------------------------------------------------

/// sx01's race files out of `Tracks\sx01_2.res`, inside MX.PSR, by their names in the archive.
fn sx01_files(names: &[&str]) -> Option<Vec<Vec<u8>>> {
    let kit = ps2_oracle::Kit::load().map_err(|why| skip(&why)).ok()?;
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = std::fs::File::open(&at) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
        return None;
    };
    use std::io::{Read, Seek, SeekFrom};
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = game_sim::archive::Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("Tracks\\sx01_2.res").expect("sx01's second track archive");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = game_sim::archive::Res::parse(&bytes).expect("the archive's layout");
    let out = names
        .iter()
        .map(|n| {
            let e = res.entries.iter().find(|e| e.name.eq_ignore_ascii_case(n)).unwrap_or_else(|| panic!("{n} in sx01_2.res"));
            res.read(&bytes, e).expect("inflate")
        })
        .collect();
    Some(out)
}

/// The sx01 race image: research/c31's race start run on to frame 604, three AI riders on the
/// path follower.
const S282C_STATE: &str = "s282c-sx01-race.ee.bin";
/// The three AI riders in that image.
const S282C_RIDERS: [u32; 3] = [0x008c_9af0, 0x0090_3050, 0x0111_de50];

/// A net in RAM against the port's: every path's flags, length, first node and branches, every
/// node's centre and edges (`x`, `z`, `w`: the heights are the ground's), its length and its next.
/// Hands back the RAM addresses of the paths and nodes, in the port's order.
fn same_net(ee: &ps2_oracle::Ee, track: u32, net: &Net, what: &str) -> (Vec<u32>, Vec<u32>) {
    let count = ee.u32_at(track + 0x40) as usize;
    assert_eq!(count, net.paths.len(), "{what}: the path count");
    let paths: Vec<u32> = (0..count as u32).map(|k| ee.u32_at(ee.u32_at(track + 0x50) + 4 * k)).collect();
    let mut nodes = Vec::new();
    for (k, &p) in paths.iter().enumerate() {
        assert_eq!(net.paths[k].first, nodes.len(), "{what}: path {k}'s first node");
        let mut n = ee.u32_at(p + 0x10);
        while n != 0 {
            nodes.push(n);
            n = ee.u32_at(n + 0x54);
        }
    }
    assert_eq!(nodes.len(), net.nodes.len(), "{what}: the node count");
    let path_index = |a: u32| paths.iter().position(|&p| p == a);
    let node_index = |a: u32| nodes.iter().position(|&n| n == a);
    for (k, (&p, ours)) in paths.iter().zip(&net.paths).enumerate() {
        let list = ee.u32_at(p + 0x2c);
        let branches: Vec<Option<usize>> =
            (0..ee.u32_at(p + 0x18)).map(|b| path_index(ee.u32_at(list + 4 * b))).collect();
        assert_eq!(
            (ee.u32_at(p), ee.u32_at(p + 8), branches),
            (ours.flags, ours.length.0.to_bits(), ours.branches.iter().map(|&b| Some(b)).collect()),
            "{what}: path {k}'s flags, length and branches"
        );
    }
    for (i, (&a, n)) in nodes.iter().zip(&net.nodes).enumerate() {
        let flat = |v: [u32; 4]| [v[0], v[2], v[3]];
        let mine = |v: Vf| {
            let b = v.to_bits();
            [b[0], b[2], b[3]]
        };
        assert_eq!(
            (flat(got(ee, a + 0x10)), flat(got(ee, a + 0x20)), flat(got(ee, a + 0x30)), ee.u32_at(a + 0x40), node_index(ee.u32_at(a + 0x54))),
            (mine(n.centre), mine(n.edges[0]), mine(n.edges[1]), n.length.0.to_bits(), n.next),
            "{what}: node {i}"
        );
    }
    (paths, nodes)
}

#[test]
fn sx01s_path_nets_off_the_disc_are_the_race_images() {
    use game_sim::race::{TrackFile, TrackPaths};
    let Some(files) = sx01_files(&["tracks\\sx01\\sx01.tdf", "tracks\\sx01\\sx01ai.tdf"]) else { return };
    let kit = match ps2_oracle::Kit::load() {
        Ok(k) => k,
        Err(why) => return skip(&why),
    };
    let image = kit.ctx("states").join(S282C_STATE);
    if !image.exists() {
        return skip(&format!(
            "{} is not there: run research/c31's start state on to frame ~600, save, `./kit state ram`",
            image.display()
        ));
    }
    let Some(at) = named(&["activity_manager", "boot_gp", "path_locate"]) else { return };
    let [manager, gp, locate] = at[..] else { unreachable!() };
    let mut ee = ps2_oracle::Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(gp));
    let act = ee.u32_at(ee.u32_at(ee.u32_at(manager) + 0x58));
    let (race_track, ai_track) = (ee.u32_at(act + 0xb8), ee.u32_at(act + 0xbc));
    let net = |bytes: &[u8]| {
        let file = TrackFile::parse(bytes).expect("the race file reads");
        Net::from_track(&TrackPaths::new(&file).expect("its paths walk"))
    };
    let (race, ai) = (net(&files[0]), net(&files[1]));
    assert_eq!((race.paths.len(), race.nodes.len()), (2, 96), "sx01.tdf: a start run and the lap");
    assert_eq!((ai.paths.len(), ai.nodes.len()), (38, 444), "sx01AI.tdf: the AI's web");
    let (race_paths, race_nodes) = same_net(&ee, race_track, &race, "sx01.tdf");
    let (ai_paths, ai_nodes) = same_net(&ee, ai_track, &ai, "sx01AI.tdf");

    // Which net is whose: the follower's (+0xdc) is the AI file's; the avoidance's edge asks the
    // race's (+0xd8's +0x68) at the card's path and node (+0xd4's +0x88, +0x8c).
    let mut on_ai = 0;
    for r in S282C_RIDERS {
        assert_eq!(ee.u32_at(r + 0xdc), ai_track, "rider {r:#x}'s follower net");
        assert_eq!(ee.u32_at(ee.u32_at(r + 0xd8) + 0x68), race_track, "rider {r:#x}'s race net");
        let card = ee.u32_at(r + 0xd4);
        assert!(race_paths.contains(&ee.u32_at(card + 0x88)), "rider {r:#x}'s card path is the race net's");
        assert!(race_nodes.contains(&ee.u32_at(card + 0x8c)), "rider {r:#x}'s card node is the race net's");
        // The follower's spot (+0x220: path, node, along), once it has one, is on the AI net.
        if ee.u32_at(r + 0x220) != 0 {
            assert!(ai_paths.contains(&ee.u32_at(r + 0x220)), "rider {r:#x}'s spot path");
            assert!(ai_nodes.contains(&ee.u32_at(r + 0x224)), "rider {r:#x}'s spot node");
            on_ai += 1;
        }
    }
    assert!(on_ai > 0, "no rider has a spot on the AI net");

    // The original's locate on the image's own nets against the port's on the nets off the disc.
    let mut pick = Pick(0x5282_c0de_7a11_0001);
    let mut placed = 0;
    for (name, net, paths, nodes) in [("sx01.tdf", &race, &race_paths, &race_nodes), ("sx01AI.tdf", &ai, &ai_paths, &ai_nodes)] {
        for case in 0..5_000 {
            let k = pick.below(net.paths.len() as u32) as usize;
            let n = &net.nodes[net.paths[k].first + pick.below(paths_len(net, k) as u32) as usize];
            let side = pick.below(3) as usize;
            let base = if side == 0 { n.centre } else { n.edges[side - 1] };
            let point = Vf([base.x() + pick.range(-60.0, 60.0), base.y(), base.z() + pick.range(-60.0, 60.0), 1.0]);
            let limit = if pick.chance(3) { pick.range(0.0, 300.0) } else { -1.0 };
            let holding_only = pick.chance(3);
            put(&mut ee, PATH_POINT, point);
            for o in [0, 4, 8] {
                ee.set_u32(PATH_OUT + o, UNTOUCHED);
            }
            ee.set_u32(PATH_DIST, UNTOUCHED);
            ee.call(
                locate,
                &[0, 0, u64::from(PATH_POINT), u64::from(paths[k]), u64::from(PATH_OUT), u64::from(PATH_DIST), 0, u64::from(holding_only)],
                &[limit],
                BUDGET,
            );
            let ours = path::locate(net, net.paths[k].first, point, F(limit), holding_only);
            let w = format!("{name} case {case}: path {k}, point {point:?}, limit {limit}, holding only {holding_only}: {ours:?}");
            assert_eq!(ee.v0(), 1, "{w}: locate's answer");
            match ours.place {
                Some(pl) => {
                    assert_eq!(ee.u32_at(PATH_OUT + 4), nodes[pl.node], "{w}: the node");
                    assert_eq!(ee.u32_at(PATH_OUT + 8), pl.along.to_bits(), "{w}: how far along");
                    placed += 1;
                }
                None => assert_eq!(ee.u32_at(PATH_OUT + 4), UNTOUCHED, "{w}: a node written"),
            }
            assert_eq!(ee.u32_at(PATH_DIST), ours.distance.to_bits(), "{w}: the distance");
        }
    }
    assert!(placed > 5_000, "only {placed} points placed");
}

/// How many nodes path `k` of `net` has.
fn paths_len(net: &Net, k: usize) -> usize {
    let mut n = 1;
    let mut at = net.paths[k].first;
    while let Some(j) = net.nodes[at].next {
        n += 1;
        at = j;
    }
    n
}

// ---------------------------------------------------------------------------------------------------
// S309: the tick's path branch on sx01's real AI net, the race image's three riders.
// ---------------------------------------------------------------------------------------------------

/// A track object's paths read out of RAM as the port holds them, with the RAM addresses of the
/// paths and of each path's nodes.
fn s309_paths(ee: &ps2_oracle::Ee, track: u32) -> (game_sim::race::TrackPaths, Vec<u32>, Vec<Vec<u32>>) {
    use game_sim::race::{PathNode, RacePath, TrackPaths};
    let f = |a: u32| F(f32::from_bits(ee.u32_at(a)));
    let quad = |a: u32| Vf(core::array::from_fn(|k| f32::from_bits(ee.u32_at(a + 4 * k as u32))));
    let count = ee.u32_at(track + 0x40) as usize;
    let recs: Vec<u32> = (0..count as u32).map(|k| ee.u32_at(ee.u32_at(track + 0x50) + 4 * k)).collect();
    let index = |a: u32| recs.iter().position(|&r| r == a);
    let mut node_addrs = Vec::new();
    let mut paths = Vec::new();
    for &r in &recs {
        let mut addrs = Vec::new();
        let mut n = ee.u32_at(r + 0x10);
        while n != 0 {
            addrs.push(n);
            n = ee.u32_at(n + 0x54);
        }
        let nodes = addrs
            .iter()
            .map(|&a| PathNode {
                id: ee.u32_at(a) as i32,
                word_04: ee.u32_at(a + 4),
                points: [quad(a + 0x10), quad(a + 0x20), quad(a + 0x30)],
                span: f(a + 0x40),
                word_44: ee.u32_at(a + 0x44),
                word_48: ee.u32_at(a + 0x48),
                word_4c: ee.u32_at(a + 0x4c),
            })
            .collect();
        let list = ee.u32_at(r + 0x2c);
        let links = (0..ee.u32_at(r + 0x18)).map(|k| index(ee.u32_at(list + 4 * k)).expect("a link on the net")).collect();
        let best = ee.u32_at(r + 0x28);
        paths.push(RacePath {
            flags: ee.u32_at(r),
            length: f(r + 8),
            word_0c: ee.u32_at(r + 0xc),
            nodes,
            links,
            dist: f(r + 0x1c),
            best: if best == 0 { None } else { Some(index(best).expect("a best link on the net")) },
        });
        node_addrs.push(addrs);
    }
    let pool = |head: u32, next: u32| {
        let (mut n, mut e) = (0, ee.u32_at(head));
        while e != 0 {
            n += 1;
            e = ee.u32_at(e + next);
        }
        n
    };
    let t = TrackPaths {
        word_34: ee.u32_at(track + 0x34),
        ahead: f(track + 0x38),
        behind: f(track + 0x3c),
        lap_length: f(track + 0x54),
        pick: ee.u32_at(track + 0xac),
        paths,
        free_routes: pool(track + 0x98, 0xc),
        free_parents: pool(track + 0xa0, 8),
        free_candidates: pool(track + 0xa8, 0xc),
    };
    (t, recs, node_addrs)
}

/// A spot's three words in RAM as the port's, `None` for a node that is not on the spot's path.
fn s309_spot(ee: &ps2_oracle::Ee, at: u32, recs: &[u32], nodes: &[Vec<u32>]) -> Option<game_sim::race::Spot> {
    let (path, node) = (ee.u32_at(at), ee.u32_at(at + 4));
    let along = F(f32::from_bits(ee.u32_at(at + 8)));
    if path == 0 {
        return Some(game_sim::race::Spot { path: 0, piece: 0, along });
    }
    let p = recs.iter().position(|&r| r == path)?;
    let n = if node == 0 { 0 } else { nodes[p].iter().position(|&a| a == node)? as u32 + 1 };
    Some(game_sim::race::Spot { path: p as u32 + 1, piece: n, along })
}

/// The follower's words of a brain in RAM.
fn s309_follower(ee: &ps2_oracle::Ee, ai: u32, recs: &[u32], nodes: &[Vec<u32>]) -> Option<game_sim::ai::follow::Follower> {
    let w = |o: u32| F(f32::from_bits(ee.u32_at(ai + o)));
    let q = |o: u32| Vf(got(ee, ai + o).map(f32::from_bits));
    Some(game_sim::ai::follow::Follower {
        flags: ee.read(ai + 0x30, 8) as u64,
        state: ee.u32_at(ai + 0x354) as i32,
        position: q(0xc0),
        forward: q(0xb0),
        own_speed: w(0x80),
        spot: s309_spot(ee, ai + 0x220, recs, nodes)?,
        draws: core::array::from_fn(|k| w(0x22c + 4 * k as u32)),
        aim: q(0x1b0),
        heading: q(0x1d0),
        speed: w(0x1e0),
        angles: q(0x1f0),
        drift: w(0x164),
        kick: w(0x168),
        kick_time: w(0x16c),
        kick_decay: w(0x170),
        kick_clock: w(0x174),
        drift_target: w(0x178),
        drift_low: w(0x17c),
        drift_high: w(0x180),
        drift_rate: w(0x184),
        drift_max: w(0x188),
        drift_clock: w(0x18c),
        period_low: w(0x190),
        period_high: w(0x194),
        period: w(0x198),
        reach: game_sim::ai::seek::Reach { base: w(0x210), max: w(0x214), gain: w(0x218) },
        cruise: w(0x2e0),
        look: w(0x2f0),
        look_cone: w(0x2f4),
    })
}

/// A follower as words, `-0` told from `0`.
fn s309_bits(f: &game_sim::ai::follow::Follower) -> Vec<u32> {
    let mut v = vec![f.flags as u32, (f.flags >> 32) as u32, f.state as u32, f.spot.path, f.spot.piece, f.spot.along.0.to_bits()];
    for q in [f.position, f.forward, f.aim, f.heading, f.angles] {
        v.extend(q.to_bits());
    }
    v.extend(f.draws.iter().map(|d| d.0.to_bits()));
    let words = [
        f.own_speed, f.speed, f.drift, f.kick, f.kick_time, f.kick_decay, f.kick_clock, f.drift_target, f.drift_low, f.drift_high, f.drift_rate,
        f.drift_max, f.drift_clock, f.period_low, f.period_high, f.period, f.reach.base, f.reach.max, f.reach.gain, f.cruise, f.look, f.look_cone,
    ];
    v.extend(words.iter().map(|w| w.0.to_bits()));
    v
}

/// The tick's path branch on the sx01 race image (research/c31's race start run on to frame 604): each
/// of its three AI riders, as the image holds it, on the AI net the image holds (ground-snapped
/// heights, the race's scratch words and pools), from `ai_seek_path_branch` to `ai_seek_path_end`
/// against `ai::follow::path`, then 400 more branches with the rider moved along its own heading at
/// its own target speed between them (a stand-in for the ride, the same on both sides). Each branch:
/// the follower's words, the net's scratch words and pools, and the generator.
#[test]
fn the_path_branch_rides_sx01s_ai_net_as_the_race_image() {
    use game_sim::ai::follow;
    let kit = match ps2_oracle::Kit::load() {
        Ok(k) => k,
        Err(why) => return skip(&why),
    };
    let image = kit.ctx("states").join(S282C_STATE);
    if !image.exists() {
        return skip(&format!(
            "{} is not there: run research/c31's start state on to frame ~600, save, `./kit state ram`",
            image.display()
        ));
    }
    let Some(at) = named(&["boot_gp", "ai_seek_path_branch", "ai_seek_path_end", "rand_state"]) else { return };
    let [gp, branch, end, rand_state] = at[..] else { unreachable!() };
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    for r in S282C_RIDERS {
        let mut ee = ps2_oracle::Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
        ee.set(28, u64::from(gp));
        ee.hooks.insert(end, Box::new(|ee| ee.set(31, ps2_oracle::DONE as i32 as i64 as u64)));
        let net = ee.u32_at(r + 0xdc);
        let (mut t, recs, nodes) = s309_paths(&ee, net);
        assert_eq!(ee.u32_at(net + 0x4c), recs[0], "the spot search's first path is path 0");
        assert_eq!((ee.u32_at(r + 0x354), ee.u32_at(r + 0x34c)), (0, 0), "rider {r:#x}: on the path, no goal");
        let mut f = s309_follower(&ee, r, &recs, &nodes).expect("the image's spot is on its path");
        let race = ee.u32_at(r + 0xd8);
        let race_word = (race != 0).then(|| ee.u32_at(race + 0x5c));
        let mut rng = Rng::from_state(ee.read(rand_state, 8) as u64);
        let dt = F(1.0 / 30.0);
        for step in 0..=400 {
            let before = f;
            ee.set(19, u64::from(r));
            ee.set_f(22, dt.0);
            ee.call(branch, &[u64::from(net)], &[], BUDGET * 20);
            follow::path(&mut f, &mut t, dt, None, race_word, &mut rng);
            let what = format!("rider {r:#x} step {step}: {before:?}");
            let Some(theirs) = s309_follower(&ee, r, &recs, &nodes) else {
                panic!("{what}: the original's spot names another path's node");
            };
            assert_eq!(s309_bits(&theirs), s309_bits(&f), "{what}\n original {theirs:?}\n port     {f:?}");
            let (now, _, _) = s309_paths(&ee, net);
            for (k, (a, b)) in now.paths.iter().zip(&t.paths).enumerate() {
                assert_eq!((a.flags, a.dist.0.to_bits(), a.best), (b.flags, b.dist.0.to_bits(), b.best), "{what}: path {k}'s scratch words");
            }
            assert_eq!(
                (now.free_routes, now.free_parents, now.free_candidates),
                (t.free_routes, t.free_parents, t.free_candidates),
                "{what}: the pools"
            );
            assert_eq!(rng.state(), ee.read(rand_state, 8) as u64, "{what}: the generator");

            let mut count = |k: &'static str| *seen.entry(k).or_default() += 1;
            count("branches");
            if f.spot.path != before.spot.path {
                count("changed path");
            }
            if f.draws != before.draws {
                count("drew again");
            }
            if f.drift_clock == F(0.0) {
                count("new drift target");
            }
            if f.speed != before.speed {
                count("speed changed");
            }
            if f.kick_clock != before.kick_clock {
                count("side step");
            }
            if f.drift == F(0.0) && before.drift != F(0.0) {
                count("drift dropped");
            }

            // Ride on: along the heading at the target speed, on both sides alike.
            let go = f.speed.0 * dt.0;
            let p = f.position;
            f.position = Vf([p.x() + f.heading.x() * go, p.y(), p.z() + f.heading.z() * go, p.w()]);
            f.forward = f.heading;
            f.own_speed = f.speed;
            put(&mut ee, r + 0xc0, f.position);
            put(&mut ee, r + 0xb0, f.forward);
            ee.set_f32(r + 0x80, f.own_speed.0);
        }
    }
    eprintln!("sx01 path branch: {seen:?}");
    assert!(seen["branches"] == 3 * 401 && seen.get("changed path").copied().unwrap_or(0) > 3, "{seen:?}");
}

// --- S313: a map's seeker world, read off the disc ------------------------------------------------------

/// A member of the disc's container.
fn s313_member(f: &mut std::fs::File, psr: &game_sim::archive::Psr, name: &str) -> Vec<u8> {
    use std::io::{Read, Seek, SeekFrom};
    let e = psr.find(name).unwrap_or_else(|| panic!("no {name}"));
    f.seek(SeekFrom::Start(e.at)).unwrap();
    let mut b = vec![0u8; e.size as usize];
    f.read_exact(&mut b).unwrap();
    b
}

/// A file of one of the container's archives, by its full name.
fn s313_file(file: &[u8], name: &str) -> Vec<u8> {
    let res = game_sim::archive::Res::parse(file).unwrap();
    let want = name.to_ascii_lowercase();
    let e = res.entries.iter().find(|e| e.name.to_ascii_lowercase() == want).unwrap_or_else(|| panic!("no {name}"));
    res.read(file, e).unwrap()
}

/// A quad as RAM holds it, bit for bit.
fn s313_bits(ee: &ps2_oracle::Ee, at: u32) -> [u32; 4] {
    [0, 4, 8, 12].map(|k| ee.u32_at(at + k))
}

/// **free09's seeker world, loaded from the disc, is the one recording2's image holds, field for
/// field.** `hit.dat`, the hit database and the hit-set database it names come out of the disc's
/// `Tracks\free09_2.res`, and the points are put on free09's terrain; the riders (kind, player, the
/// model root's position) and the slot count are the image's, and a zone is open to player 0 when
/// it is the first of its progression (what this save's progress has unlocked) and to nobody else.
/// Against the image: every jump's three legs (velocity, aim, point, facing, learnt; the scale the
/// seeker never reads is skipped), every target's jump and per-slot done flags, every group's list
/// (by target), point, in-play word and two open bytes (bit 0), and every set's owner, with no pick
/// list yet, no course and no sequence.
#[test]
fn s313_free09_world_loads_as_the_image_holds_it() {
    use game_sim::phy::Ini;
    use game_sim::track::{Scene, Terrain};
    use game_sim::world::{Ground, Place, Props};
    use std::io::Read;

    let kit = match ps2_oracle::Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let image = kit.ctx("states").join("recording2.p2m2_SaveState.eeMemory.bin");
    if !image.exists() {
        return skip(&format!("{} is not there: `./kit state ram` of recording2.p2m2_SaveState.p2s", image.display()));
    }
    let mut f = match std::fs::File::open(kit.ctx("iso").join("MX.PSR")) {
        Ok(f) => f,
        Err(e) => return skip(&format!("no disc container: {e}")),
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).unwrap();
    let psr = game_sim::archive::Psr::parse(&front).unwrap();
    let track = s313_member(&mut f, &psr, "Tracks\\free09.res");
    let scene = Scene::parse(&s313_file(&track, "tracks\\free09\\free09.scn")).unwrap();
    let trn = s313_file(&track, "tracks\\free09\\free09.ps2.trn");
    let terrain = Terrain::parse(&trn).unwrap();
    let mut place = Place { ground: Ground::new(&terrain, &scene.terrain), props: Props::default() };
    let data = s313_member(&mut f, &psr, "Tracks\\free09_2.res");
    let hit_dat = Ini::parse_bytes(&s313_file(&data, "tracks\\free09\\hit.dat"));
    let (hits, sets) = sk::databases(&hit_dat);
    let (hits, sets) = (hits.unwrap(), sets.unwrap());
    assert_eq!((hits, sets), ("free.hit", "free.set"));
    let hits = Ini::parse_bytes(&s313_file(&data, &format!("tracks\\free09\\{hits}")));
    let hit_sets = Ini::parse_bytes(&s313_file(&data, &format!("tracks\\free09\\{sets}")));

    let ee = ps2_oracle::Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
    let root = ee.u32_at(addr("ai_seek_sequence_root").unwrap());
    let set_items = ee.u32_at(root + 0x38);
    let owners: Vec<sk::Owner> = (0..ee.u32_at(root + 0x30))
        .map(|i| {
            let o = ee.u32_at(ee.u32_at(set_items + 4 * i) + 0x54);
            let node = ee.u32_at(ee.u32_at(ee.u32_at(o + 0x3c)) + 0x18);
            sk::Owner {
                kind: ee.u32_at(o + 0x34) as i32,
                player: ee.u32_at(o + 0x30) as usize,
                position: Vf::from_bits(s313_bits(&ee, node + 0x70)),
                hit_freeride: false,
            }
        })
        .collect();
    let zones = ee.u32_at(root + 0x44);
    let manager = ee.u32_at(ee.u32_at(zones + 0x50) + 0x48);
    let slots = ee.u32_at(manager + 0x48) as usize;
    let open = |player: usize, zone: sk::Zone| player == 0 && zone.place == Some(0);
    let world = sk::load(&hit_dat, &hits, &hit_sets, &owners, slots, &open, &mut place);

    // The sets: one per rider, as passed, and nothing built yet.
    assert_eq!(world.sets.len(), 4);
    for (i, set) in world.sets.iter().enumerate() {
        let s = ee.u32_at(set_items + 4 * i as u32);
        assert_eq!(ee.u32_at(s + 0x114), 0, "set {i}: the image's has a course");
        assert_eq!((set.picks, &set.course), (None, &None), "set {i}");
        let o = ee.u32_at(s + 0x54);
        assert_eq!((set.owner.kind, set.owner.player), (ee.u32_at(o + 0x34) as i32, ee.u32_at(o + 0x30) as usize));
    }
    assert_eq!(world.sequence, None);

    // The targets and their jumps, in the database's order.
    let target_items = ee.u32_at(manager + 0x38);
    let targets: Vec<u32> = (0..ee.u32_at(manager + 0x30)).map(|i| ee.u32_at(target_items + 4 * i)).collect();
    assert_eq!((world.targets.len(), world.jumps.len(), targets.len()), (106, 106, 106));
    let leg = |at: u32, l: &sk::Leg, what: &str| {
        for (name, off, port) in
            [("velocity", 0, l.velocity), ("aim", 0x10, l.aim), ("point", 0x20, l.point), ("facing", 0x30, l.facing)]
        {
            assert_eq!(port.to_bits(), s313_bits(&ee, at + off), "{what}: {name} {port:?}");
        }
        assert_eq!(l.learnt.0.to_bits(), ee.u32_at(at + 0x4c), "{what}: learnt {:?}", l.learnt);
    };
    for (i, (t, &at)) in world.targets.iter().zip(&targets).enumerate() {
        let hit = ee.u32_at(at + 0x3c);
        assert_eq!(ee.u16_at(hit + 0x120) as usize, i + 1, "target {i}'s hit id");
        let records = ee.u32_at(at + 0x40);
        let done: Vec<bool> = (0..slots as u32).map(|p| ee.u16_at(records + 0x30 * p + 0x20) & 2 != 0).collect();
        assert_eq!((t.jump, &t.done), (Some(i), &done), "target {i}");
        let j = &world.jumps[i];
        leg(hit, &j.take_off, &format!("jump {i} take-off"));
        leg(hit + 0x60, &j.landing, &format!("jump {i} landing"));
        leg(hit + 0xc0, &j.setup, &format!("jump {i} setup"));
    }

    // The groups: the start zones.
    let group_items = ee.u32_at(zones + 0x38);
    assert_eq!(world.groups.len() as u32, ee.u32_at(zones + 0x30));
    assert_eq!(world.groups.len(), 12);
    for (i, g) in world.groups.iter().enumerate() {
        let at = ee.u32_at(group_items + 4 * i as u32);
        let list = ee.u32_at(at + 0x48);
        let want = (list != 0).then(|| {
            let items = ee.u32_at(list + 0xc);
            (0..ee.u32_at(list + 4))
                .map(|k| {
                    let t = ee.u32_at(items + 4 * k);
                    targets.iter().position(|&a| a == t)
                })
                .collect::<Vec<_>>()
        });
        assert_eq!(g.targets, want, "group {i}'s list");
        assert_eq!(g.point.to_bits(), s313_bits(&ee, at + 0x50), "group {i}'s point {:?}", g.point);
        assert_eq!(g.active, ee.u32_at(at + 0x78) == 1, "group {i} in play");
        let bytes = ee.u32_at(at + 0x44);
        let open: Vec<bool> = (0..2).map(|p| ee.u8_at(bytes + p) & 1 != 0).collect();
        assert_eq!(g.open, open, "group {i}'s open bytes");
    }
    // A hit zone in play, a run zone's list, the race and stunt zones with none, and the image's
    // repeat inside a list: what the loop above covered.
    assert!(world.groups[0].active && !world.groups[5].active);
    assert_eq!(world.groups[8].targets.as_ref().map(Vec::len), Some(4));
    assert_eq!((&world.groups[10].targets, &world.groups[11].targets), (&None, &None));
}
