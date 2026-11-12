// ---------------------------------------------------------------------------------------------------
// S12e — the AI rider joined: `ai::Racer::tick` against the original's whole AI tick
// ---------------------------------------------------------------------------------------------------
//
// recording2's free09 image holds three AI riders mid-ride. Each one's brain, interface, card and bike
// are read out of RAM into a `Racer` and its `Around`; then, rider by rider in the image's actor order
// and tick after tick, the original's `ai_update` runs in the interpreter and the port's tick runs on
// what it read. Nothing of the original is hooked but the ground query, which runs as it is and is
// recorded: the two the rider-to-brain copy asks are played back to the port (and the points it asks
// at compared), and the eleven the avoidance's feeler sweep asks are only counted (the sweep's hits are
// thrown away). The bike is frozen between ticks — its physics is not run — so the brain sees the same
// bike every tick and only its own state moves on.

use super::*;
use game_sim::ai::{Around, Card, Habits, Racer, Senses, Told};
use game_sim::ragdoll::Tables;
use super::seek_world::{s313_file, s313_member};
use ps2_oracle::{Ee, Kit};
use std::cell::Cell;

const S12E_STATE: &str = "recording2.p2m2_SaveState.eeMemory.bin";
/// The image's three brains and their riders, in the order the scene runs them.
pub(crate) const S12E_RIDERS: [(u32, u32); 3] = [(0x0148_3eb0, 0x00d4_f510), (0x00c6_4000, 0x00c4_f040), (0x0104_8000, 0x0150_8a50)];
/// The step the image's scene root hands its children (its dt word).
const S12E_DT: u32 = 0x3d08_850a;
const S12E_TICKS: usize = 60;
const S12E_BUDGET: u64 = 50_000_000;

fn s12e_f(ee: &Ee, at: u32) -> F {
    F(f32::from_bits(ee.u32_at(at)))
}

fn s12e_vf(ee: &Ee, at: u32) -> Vf {
    Vf::from_bits(got(ee, at))
}

fn s12e_u64(ee: &Ee, at: u32) -> u64 {
    ee.read(at, 8) as u64
}

/// The habits' own words (the ones not shared with the brain's other pieces), by where the image keeps
/// them: the brain's, the rider's, its engine's (rider +0x7b8), its body's (+0x7a4) and its card's.
type HabitWord = (u32, &'static str, fn(&mut Habits) -> &mut F);
fn s12e_habit_words(b: u32, r: u32, engine: u32, body: u32, card: u32) -> Vec<HabitWord> {
    vec![
        (b + 0x2b0, "low air", |h| &mut h.low_air),
        (r + 0x10e4, "preload base", |h| &mut h.preload_base),
        (r + 0x10f0, "preload per speed", |h| &mut h.preload_per_speed),
        (r + 0x10ec, "preload", |h| &mut h.preload),
        (r + 0x1108, "pull-back", |h| &mut h.pull_back_amount),
        (r + 0x110c, "lag hang time", |h| &mut h.lag_hang_time),
        (r + 0x111c, "lag chance", |h| &mut h.lag_chance),
        (r + 0x1120, "lag time", |h| &mut h.lag_time),
        (r + 0x1124, "lag", |h| &mut h.lag),
        (r + 0x1134, "horse power", |h| &mut h.horse_power),
        (r + 0x114c, "brilliant chance", |h| &mut h.brilliant_chance),
        (r + 0x1150, "brilliant min", |h| &mut h.brilliant_min),
        (r + 0x1154, "brilliant random", |h| &mut h.brilliant_random),
        (r + 0x1158, "brilliant", |h| &mut h.brilliant),
        (r + 0x115c, "brilliant roll", |h| &mut h.brilliant_roll),
        (r + 0x1170, "gate delay", |h| &mut h.gate_delay),
        (r + 0x1174, "gate clock", |h| &mut h.gate_clock),
        (r + 0x11ec, "gate wait", |h| &mut h.gate_wait),
        (r + 0x1184, "wheelie max", |h| &mut h.wheelie_max),
        (r + 0x1198, "wheelie", |h| &mut h.wheelie),
        (r + 0x11a8, "wheelie lean", |h| &mut h.wheelie_lean),
        (r + 0x11ac, "cross-up side", |h| &mut h.cross_up_side),
        (r + 0x11b0, "cross-up time", |h| &mut h.cross_up_time),
        (r + 0x11b4, "cross-up clock", |h| &mut h.cross_up),
        (r + 0x11b8, "cross-up chance", |h| &mut h.cross_up_chance),
        (r + 0x11bc, "cross-up min", |h| &mut h.cross_up_min),
        (r + 0x11c0, "cross-up max", |h| &mut h.cross_up_max),
        (r + 0x11c4, "cross-up height", |h| &mut h.cross_up_height),
        (r + 0x11cc, "low-air bar", |h| &mut h.low_air_bar),
        (engine + 0xbc, "power scale", |h| &mut h.power_scale),
        (r + 0x494, "preload force", |h| &mut h.preload_force),
        (r + 0x4b0, "preload clock", |h| &mut h.preload_clock),
        (r + 0x4ac, "preload clock start", |h| &mut h.preload_clock_start),
        (r + 0x4c4, "max preload", |h| &mut h.max_preload),
        (r + 0x644, "clutch", |h| &mut h.clutch),
        (body + 0x264, "forward speed", |h| &mut h.forward_speed),
        (card + 0xfc, "air time", |h| &mut h.air_time),
        (card + 0x1b0, "lag clock", |h| &mut h.lag_clock),
    ]
}

/// The free09 seeker world's lists in the image: the sets (in the set root's order) and the targets
/// (in the hit database's), which the seeker's pointers index.
pub(crate) struct S12eLists {
    pub(crate) sets: Vec<u32>,
    pub(crate) targets: Vec<u32>,
}

impl S12eLists {
    fn index(list: &[u32], at: u32, what: &str) -> Option<usize> {
        (at != 0).then(|| list.iter().position(|&a| a == at).unwrap_or_else(|| panic!("{what} {at:#x} is in no list")))
    }
    /// A jump is its target's hit (target +0x3c), and has the target's index.
    fn jump(&self, ee: &Ee, at: u32) -> Option<usize> {
        (at != 0).then(|| {
            self.targets.iter().position(|&t| ee.u32_at(t + 0x3c) == at).unwrap_or_else(|| panic!("jump {at:#x} is no target's"))
        })
    }
    pub(crate) fn seeker(&self, ee: &Ee, b: u32) -> sk::Seeker {
        sk::Seeker {
            flags: s12e_u64(ee, b + 0x30),
            flags2: s12e_u64(ee, b + 0x38),
            set: Self::index(&self.sets, ee.u32_at(b + 0x340), "set"),
            target: Self::index(&self.targets, ee.u32_at(b + 0x344), "target"),
            last_target: Self::index(&self.targets, ee.u32_at(b + 0x348), "last target"),
            jump: self.jump(ee, ee.u32_at(b + 0x34c)),
            index: ee.u32_at(b + 0x350) as i32,
            state: ee.u32_at(b + 0x354) as i32,
            tries: ee.u32_at(b + 0x358) as i32,
            tolerance: s12e_f(ee, b + 0x35c),
            learnt: s12e_f(ee, b + 0x360),
            aim_step: s12e_f(ee, b + 0x364),
            speed_step: s12e_f(ee, b + 0x368),
            learn_step: s12e_f(ee, b + 0x36c),
            last_miss: s12e_vf(ee, b + 0x370),
            miss: s12e_vf(ee, b + 0x380),
            lead: s12e_f(ee, b + 0x25c),
            distance: s12e_f(ee, b + 0x2fc),
            follow: ee.u32_at(b + 0x220),
            aim: s12e_vf(ee, b + 0x1b0),
            heading: s12e_vf(ee, b + 0x1d0),
            speed: s12e_f(ee, b + 0x1e0),
        }
    }
}

/// free09's seeker world off the disc, as `seek_world.rs` loads it, and the image's lists.
pub(crate) fn s12e_seek_world(kit: &Kit, ee: &Ee) -> Option<(sk::World, S12eLists)> {
    use game_sim::track::{Scene, Terrain};
    use game_sim::world::{Ground, Place, Props};
    use std::io::Read;

    let mut f = match std::fs::File::open(kit.ctx("iso").join("MX.PSR")) {
        Ok(f) => f,
        Err(e) => {
            skip(&format!("no disc container: {e}"));
            return None;
        }
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
    let hits = Ini::parse_bytes(&s313_file(&data, &format!("tracks\\free09\\{}", hits.unwrap())));
    let hit_sets = Ini::parse_bytes(&s313_file(&data, &format!("tracks\\free09\\{}", sets.unwrap())));

    let root = ee.u32_at(addr("ai_seek_sequence_root").unwrap());
    let set_items = ee.u32_at(root + 0x38);
    let set_list: Vec<u32> = (0..ee.u32_at(root + 0x30)).map(|i| ee.u32_at(set_items + 4 * i)).collect();
    let owners: Vec<sk::Owner> = set_list
        .iter()
        .map(|&s| {
            let o = ee.u32_at(s + 0x54);
            let node = ee.u32_at(ee.u32_at(ee.u32_at(o + 0x3c)) + 0x18);
            sk::Owner {
                kind: ee.u32_at(o + 0x34) as i32,
                player: ee.u32_at(o + 0x30) as usize,
                position: s12e_vf(ee, node + 0x70),
                hit_freeride: false,
            }
        })
        .collect();
    let zones = ee.u32_at(root + 0x44);
    let manager = ee.u32_at(ee.u32_at(zones + 0x50) + 0x48);
    let slots = ee.u32_at(manager + 0x48) as usize;
    let open = |player: usize, zone: sk::Zone| player == 0 && zone.place == Some(0);
    let world = sk::load(&hit_dat, &hits, &hit_sets, &owners, slots, &open, &mut place);
    let target_items = ee.u32_at(manager + 0x38);
    let targets = (0..ee.u32_at(manager + 0x30)).map(|i| ee.u32_at(target_items + 4 * i)).collect();
    Some((world, S12eLists { sets: set_list, targets }))
}

/// One brain read out of RAM.
fn s12e_racer(ee: &Ee, lists: &S12eLists, b: u32, r: u32) -> Racer {
    let iface = ee.u32_at(b + 0x40);
    let card = ee.u32_at(b + 0xd4);
    let flags = s12e_u64(ee, b + 0x30);
    let brain = Brain {
        kind: (iface != 0).then(|| ee.u32_at(iface + 4) as i32),
        mode: ee.u32_at(b + 0xd0) as i32,
        flags,
        speed: s12e_f(ee, b + 0x80),
        forward_y: s12e_f(ee, b + 0xb4),
        stuck: s12e_f(ee, b + 0x3f0),
        climb: Climb {
            hold: s12e_f(ee, b + 0x390),
            steep: s12e_f(ee, b + 0x394),
            timer: s12e_f(ee, b + 0x398),
            speed: s12e_f(ee, b + 0x30c),
        },
        steer: s12e_f(ee, b + 0xf0),
        pitch: s12e_f(ee, b + 0xf4),
        throttle: s12e_f(ee, b + 0xe0),
        brake: s12e_f(ee, b + 0xe4),
    };
    let senses = Senses {
        last_position: s12e_vf(ee, b + 0x290),
        frame: core::array::from_fn(|k| s12e_vf(ee, b + 0x90 + 0x10 * k as u32)),
        velocity: s12e_vf(ee, b + 0x50),
        acceleration: s12e_vf(ee, b + 0x60),
        spin: s12e_vf(ee, b + 0x70),
        speed: s12e_f(ee, b + 0x80),
        ground_normal: s12e_vf(ee, b + 0x2c0),
        ground: s12e_f(ee, b + 0x204),
        flags,
        bumps: s12e_u64(ee, b + 0x38),
    };
    let (_, tuning) = controls::read(ee, b);
    let avoid = AvoidTuning {
        near_range: s12e_f(ee, b + 0x280),
        far_range: s12e_f(ee, b + 0x284),
        far_radius: s12e_f(ee, b + 0x3d8),
        far_response: s12e_f(ee, b + 0x3dc),
        far_approach_limit: s12e_f(ee, b + 0x3e0),
        far_min_braking_speed: s12e_f(ee, b + 0x3e4),
        far_braking_response: s12e_f(ee, b + 0x3e8),
        near_separation: s12e_f(ee, b + 0x3a4),
        near_response: s12e_f(ee, b + 0x3ac),
        near_braking_response: s12e_f(ee, b + 0x3b0),
        passing_speed: s12e_f(ee, b + 0x2f8),
        near_min_braking_speed: s12e_f(ee, b + 0x3b4),
        front_window: s12e_f(ee, b + 0x3d0),
        rear_window: s12e_f(ee, b + 0x3d4),
        drift_match: s12e_f(ee, b + 0x3a8),
        far_heading_cos: s12e_f(ee, b + 0x3ec),
        edge_look_ahead: s12e_f(ee, b + 0x288),
        edge_push: s12e_f(ee, b + 0x3c8),
        feelers: ee.u32_at(b + 0x2a4) as i32,
        feeler_height: s12e_f(ee, b + 0x2a8),
        feeler_length: s12e_f(ee, b + 0x2ac),
    };
    let plan = ai::TrickPlan {
        cooldown: s12e_f(ee, b + 0x158),
        since: s12e_f(ee, b + 0x15c),
        chance: s12e_f(ee, b + 0x160),
        estimate: s12e_f(ee, b + 0x19c),
        slot: ee.u32_at(b + 0x1a0) as i32,
        air: s12e_f(ee, b + 0x200),
    };
    let (engine, body) = (ee.u32_at(r + 0x7b8), ee.u32_at(r + 0x7a4));
    let keeper = ee.u32_at(r + 0xfa0);
    let mut habits = Habits {
        preload_firing: ee.u32_at(r + 0x4a4) as i32,
        racing: card != 0 && s12e_u64(ee, card + 8) & 0x8_0000 != 0,
        keeper_state: ee.u32_at(keeper + 0x64) as i32,
        ..Habits::default()
    };
    for (at, _, field) in s12e_habit_words(b, r, engine, body, card) {
        *field(&mut habits) = s12e_f(ee, at);
    }
    let listening = ee.u32_at(b + 0x24) & 2 != 0;
    assert_eq!(ee.u32_at(b + 0x18), 0, "brain {b:#x} has listeners: their own ticks are not the port's");
    Racer {
        brain,
        senses,
        moves: s12e_u64(ee, b + 0x38),
        seeker: lists.seeker(ee, b),
        reach: sk::Reach { base: s12e_f(ee, b + 0x210), max: s12e_f(ee, b + 0x214), gain: s12e_f(ee, b + 0x218) },
        min_speed: s12e_f(ee, b + 0x254),
        assist_x: s12e_f(ee, b + 0x1c0),
        assist_z: s12e_f(ee, b + 0x1c8),
        controls: ai::Controls {
            steer: brain.steer,
            pitch: brain.pitch,
            throttle: brain.throttle,
            brake: brain.brake,
            heading: s12e_f(ee, b + 0x1f4),
            aim: s12e_f(ee, b + 0x1f0),
            roll: s12e_f(ee, b + 0x1f8),
            flat_forward: s12e_vf(ee, b + 0x270),
            target_speed: s12e_f(ee, b + 0x1e0),
        },
        bar_rest: [s12e_f(ee, b + 0xf8), s12e_f(ee, b + 0xfc)],
        tuning,
        avoid,
        plan,
        habits,
        filter: ai::SteerFilter {
            rate: s12e_vf(ee, b + 0x110),
            jitter: s12e_vf(ee, b + 0x120),
            out: s12e_vf(ee, b + 0x130),
            gain: s12e_vf(ee, b + 0x140),
        },
        weights: Weights { heading: s12e_f(ee, r + 0x1138), aim: s12e_f(ee, r + 0x113c) },
        listening,
        listeners: Vec::new(),
    }
}

/// What a rider's `Around` is made of that the tick does not change: its bike as the copy reads it,
/// its interface, its card and the race's other riders.
struct S12eBike {
    rider: ai::Rider,
    speed_scale: F,
    slow_radius: F,
    slow_speed: F,
    player: usize,
    air_cone: F,
    card: Option<Card>,
    race_phase: Option<i32>,
    path_net: bool,
    others: Vec<Other>,
    live: bool,
    recovery: Recovery,
    levers: Levers,
    yaw_rate: F,
    velocity: Vf,
}

fn s12e_bike(ee: &Ee, b: u32, r: u32) -> S12eBike {
    let iface = ee.u32_at(b + 0x40);
    let frame = ee.u32_at(r + 0x18);
    let body = ee.u32_at(r + 0x7a4);
    let card = ee.u32_at(b + 0xd4);
    let race = ee.u32_at(b + 0xd8);
    let mut others = Vec::new();
    if race != 0 {
        let mut c = ee.u32_at(race + 0x34);
        while c != 0 {
            if c != card {
                others.push(Other {
                    active: ee.u32_at(c + 4) != 0,
                    marked: s12e_u64(ee, c + 8) & 4 != 0,
                    position: s12e_vf(ee, c + 0x60),
                    velocity: s12e_vf(ee, c + 0x110),
                    heading: s12e_vf(ee, c + 0x120),
                    speed: s12e_f(ee, c + 0x108),
                    progress: s12e_f(ee, c + 0xbc),
                });
            }
            c = ee.u32_at(c + 0x1c8);
        }
    }
    let keeper = ee.u32_at(r + 0xfa0);
    S12eBike {
        rider: ai::Rider {
            frame: core::array::from_fn(|k| s12e_vf(ee, frame + 0x80 + 0x10 * k as u32)),
            velocity: s12e_vf(ee, body + 0x260),
            acceleration: s12e_vf(ee, body + 0x70),
            spin: s12e_vf(ee, body + 0x290),
            forward_speed: s12e_f(ee, body + 0x278),
            on_ground: ee.u32_at(r + 0x574) != 0,
            tricking: ee.u32_at(r + IN_PROGRESS) != 0,
            look_ahead: s12e_f(ee, r + 0x11d0),
            bump_chance: s12e_f(ee, r + 0x1100),
        },
        speed_scale: s12e_f(ee, iface + 0x110),
        slow_radius: s12e_f(ee, iface + 0x114),
        slow_speed: s12e_f(ee, iface + 0x118),
        player: ee.u32_at(iface + 0x1e4) as usize,
        air_cone: s12e_f(ee, iface + 0x150),
        card: (card != 0).then(|| Card {
            flags: s12e_u64(ee, card + 8),
            settle: s12e_f(ee, card + 0xfc),
            bike_word: ee.u32_at(card + 0x40),
            progress: s12e_f(ee, card + 0xbc),
        }),
        race_phase: (race != 0).then(|| ee.u32_at(race + 0x60) as i32),
        path_net: ee.u32_at(b + 0xdc) != 0,
        others,
        live: ee.u32_at(r + 0xf9c) != 0 && keeper != 0 && ee.u32_at(keeper + 0x5c) & 1 != 0,
        recovery: Recovery {
            back_up: s12e_f(ee, r + 0x14c0),
            swerve_bar: s12e_f(ee, r + 0x14c4),
            swerve: s12e_f(ee, r + 0x14c8),
        },
        levers: Levers {
            steer: s12e_f(ee, r + 0x628),
            lean: s12e_f(ee, r + 0x62c),
            gas: s12e_f(ee, r + 0x63c),
            brake: s12e_f(ee, r + 0x640),
            reverse: s12e_f(ee, r + 0x648),
        },
        yaw_rate: s12e_f(ee, body + 0x284),
        velocity: s12e_vf(ee, body + 0x260),
    }
}

/// A ground query the original asked: the point (bits), its answer's height and normal, and whether
/// the avoidance asked it.
#[derive(Clone, Copy, Debug)]
struct S12eQuery {
    asked: [u32; 4],
    height: f32,
    normal: Vf,
    in_avoid: bool,
}

/// The port's ground: the original's answers to the rider-to-brain copy, in turn.
struct S12eReplay {
    answers: Vec<S12eQuery>,
    asked: Vec<[u32; 4]>,
}

impl World for S12eReplay {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let k = self.asked.len();
        self.asked.push(at.to_bits());
        let q = self.answers.get(k).unwrap_or_else(|| panic!("the port asked the ground {} times", k + 1));
        (Vf::new(at.x(), q.height, at.z(), at.w()), q.normal)
    }
    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// A hook that runs the hooked function as it is, on its own stack below the caller's, with `before`
/// and `after` around it.
fn s12e_pass_through(ee: &mut Ee, at: u32, mut before: impl FnMut(&Ee) + 'static, mut after: impl FnMut(&Ee) + 'static) {
    ee.hooks.insert(
        at,
        Box::new(move |e: &mut Ee| {
            before(e);
            let (ra, sp, old) = (e.r(31), e.r(29), e.stack);
            e.stack = ((sp as u32) - 0x4000) & !0xf;
            let hook = e.hooks.remove(&at);
            e.call(at, &[], &[], S12E_BUDGET);
            if let Some(h) = hook {
                e.hooks.insert(at, h);
            }
            e.stack = old;
            e.set(29, sp);
            e.set(31, ra);
            after(e);
            e.pc = at;
        }),
    );
}

/// **The joined AI tick is the original's, on free09's three riders, sixty ticks each, interleaved.**
/// Per rider and tick, against what the original's `ai_update` left in RAM: the answer; the flag
/// words; the brain's speed, frame, motion, ground and last position; the stuck and climb clocks; the
/// four controls and the controller's working values; the whole seeker; the assist's aim; the trick
/// plan; every habit word and the preload's firing word; the levers; the body's yaw rate and velocity;
/// every jump's legs; and the generator's state, which both sides carry from rider to rider.
///
/// A riding brain avoids with the generic vehicle avoidance, which is not ported: the heading, speed
/// and flags it left in the original stand in for it (`Around::vehicle_avoid`), and its ground probes
/// are kept out of the replayed ground.
#[test]
fn s12e_the_joined_tick_is_the_originals_on_free09() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let path = kit.ctx("states").join(S12E_STATE);
    if !path.exists() {
        return skip(&format!("{} is not there: `./kit state ram` of recording2.p2m2_SaveState.p2s", path.display()));
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let Some(at) = named(&["ai_update", "ground_query", "ai_update_mx", "ai_update_vehicle", "rand_state"]) else { return };
    let [update, query, update_mx, update_vehicle, rand_state] = at[..] else { unreachable!() };
    let Some((mut seek_world, lists)) = s12e_seek_world(&kit, &ee) else { return };
    let tables: Tables = controls::tables(&ee);

    let queries: Rc<RefCell<Vec<S12eQuery>>> = Rc::default();
    let in_avoid: Rc<Cell<bool>> = Rc::default();
    let vehicle: Rc<Cell<Option<(Vf, F, u64)>>> = Rc::default();
    let asking: Rc<Cell<(u32, u32, [u32; 4])>> = Rc::default();
    {
        let (a, a2) = (asking.clone(), asking.clone());
        let (q, inside) = (queries.clone(), in_avoid.clone());
        s12e_pass_through(
            &mut ee,
            query,
            // The query writes its height over the point's y: the point is read going in.
            move |e| a.set((e.r(5) as u32, e.r(6) as u32, got(e, e.r(5) as u32))),
            move |e| {
                let (point, normal, asked) = a2.get();
                q.borrow_mut().push(S12eQuery {
                    asked,
                    height: e.f32_at(point + 4),
                    normal: if normal == 0 { Vf::new(0.0, 0.0, 0.0, 0.0) } else { s12e_vf(e, normal) },
                    in_avoid: inside.get(),
                });
            },
        );
        let (i1, i2) = (in_avoid.clone(), in_avoid.clone());
        s12e_pass_through(&mut ee, update_mx, move |_| i1.set(true), move |_| i2.set(false));
        // The generic avoidance is not ported: what it left stands in for it.
        let (v, brain) = (vehicle.clone(), Rc::new(Cell::new(0u32)));
        let at = brain.clone();
        s12e_pass_through(&mut ee, update_vehicle, move |e| at.set(e.r(4) as u32), move |e| {
            let b = brain.get();
            assert!(v.replace(Some((s12e_vf(e, b + 0x1d0), F(e.f32_at(b + 0x1e0)), s12e_u64(e, b + 0x30)))).is_none());
        });
    }

    // The port's side: read once, then carried.
    let mut racers: Vec<Racer> = S12E_RIDERS.iter().map(|&(b, r)| s12e_racer(&ee, &lists, b, r)).collect();
    let mut bikes: Vec<S12eBike> = S12E_RIDERS.iter().map(|&(b, r)| s12e_bike(&ee, b, r)).collect();
    let mut rng = Rng::from_state(s12e_u64(&ee, rand_state));
    let dt = f32::from_bits(S12E_DT);

    let mut draws = 0;
    let mut seen = std::collections::BTreeSet::new();
    for tick in 0..S12E_TICKS {
        for (k, &(b, r)) in S12E_RIDERS.iter().enumerate() {
            let what = format!("tick {tick} rider {k} (brain {b:#x})");
            let before = rng.state();
            queries.borrow_mut().clear();
            ee.call(update, &[u64::from(b)], &[dt], S12E_BUDGET);
            let answer = ee.v0() as u32;

            let racer = &mut racers[k];
            let bike = &mut bikes[k];
            assert_eq!(racer.brain.flags & flag::KEEP_ON_TRACK, 0, "{what}: the keep-on-track push needs the track's path");
            let theirs = queries.borrow().clone();
            let mut world = S12eReplay { answers: theirs.iter().filter(|q| !q.in_avoid).copied().collect(), asked: Vec::new() };
            // The card's settle word is the habits' air time; its racing bit the habits'.
            let card = bike.card.map(|c| Card {
                settle: racer.habits.air_time,
                flags: c.flags & !0x8_0000 | if racer.habits.racing { 0x8_0000 } else { 0 },
                ..c
            });
            let theirs_vehicle = vehicle.take().expect("a riding brain runs the generic avoidance");
            let mut stand_in = |fb: &mut game_sim::ai::follow::Brain| {
                (fb.heading, fb.speed, fb.flags) = theirs_vehicle;
            };
            let mut rider = bike.rider;
            rider.velocity = bike.velocity;
            let mut around = Around {
                rider,
                world: &mut world,
                seek_world: &mut seek_world,
                tables: &tables,
                speed_scale: bike.speed_scale,
                slow_radius: bike.slow_radius,
                slow_speed: bike.slow_speed,
                player: bike.player,
                air_cone: bike.air_cone,
                card,
                race_phase: bike.race_phase,
                path_net: bike.path_net,
                others: &bike.others,
                edge: None,
                vehicle_avoid: Some(&mut stand_in),
                jump: None,
                node: None,
                spot: None,
                tricks: None,
                race_state: None,
                live: bike.live,
                recovery: bike.recovery,
                levers: &mut bike.levers,
                yaw_rate: &mut bike.yaw_rate,
                velocity: &mut bike.velocity,
            };
            let told: Told = racer.tick(&mut around, &mut rng, F(dt));
            assert_eq!(ee.u32_at(b + 0x224), 0, "{what}: a route node: the trick plan and the habits would read it");
            assert_eq!(ee.u32_at(ee.u32_at(r + 0xfa0) + 0x68), 0, "{what}: a track: the cross-up would read its spot");

            // What was asked of the ground.
            let feed_asked: Vec<_> = theirs.iter().filter(|q| !q.in_avoid).map(|q| q.asked).collect();
            assert_eq!(feed_asked.len(), world.asked.len(), "{what}: the copy's ground queries");
            assert_eq!(feed_asked[0][..3], world.asked[0][..3], "{what}: the look-ahead point");
            assert_eq!(feed_asked[1], world.asked[1], "{what}: the point under the bike");
            // The motocross fan is not built: the generic avoidance's own probes are in what it left.
            assert!(told.feelers.is_none(), "{what}: the motocross avoidance ran");

            assert_eq!(answer, u32::from(told.answer), "{what}: the answer");
            assert!(told.missing.is_empty() && told.rescue.is_none() && told.listeners.is_empty(), "{what}: {told:?}");
            assert_eq!(s12e_u64(&ee, rand_state), rng.state(), "{what}: the generator");
            assert_eq!(s12e_u64(&ee, b + 0x30), racer.brain.flags, "{what}: flags");
            assert_eq!(s12e_u64(&ee, b + 0x38), racer.moves, "{what}: second flags");
            let s = &racer.senses;
            let quads = [
                ("last position", b + 0x290, s.last_position),
                ("side", b + 0x90, s.frame[0]),
                ("up", b + 0xa0, s.frame[1]),
                ("forward", b + 0xb0, s.frame[2]),
                ("position", b + 0xc0, s.frame[3]),
                ("velocity", b + 0x50, s.velocity),
                ("acceleration", b + 0x60, s.acceleration),
                ("spin", b + 0x70, s.spin),
                ("ground normal", b + 0x2c0, s.ground_normal),
                ("flat forward", b + 0x270, racer.controls.flat_forward),
                ("filtered", b + 0x130, racer.filter.out),
                ("body velocity", ee.u32_at(r + 0x7a4) + 0x260, bike.velocity),
            ];
            for (name, at, ours) in quads {
                assert_eq!(got(&ee, at), ours.to_bits(), "{what}: {name}");
            }
            let br = &racer.brain;
            let mut words = vec![
                ("speed", b + 0x80, br.speed),
                ("ground", b + 0x204, s.ground),
                ("stuck", b + 0x3f0, br.stuck),
                ("climb hold", b + 0x390, br.climb.hold),
                ("climb timer", b + 0x398, br.climb.timer),
                ("steer", b + 0xf0, br.steer),
                ("pitch", b + 0xf4, br.pitch),
                ("throttle", b + 0xe0, br.throttle),
                ("brake", b + 0xe4, br.brake),
                ("heading angle", b + 0x1f4, racer.controls.heading),
                ("aim angle", b + 0x1f0, racer.controls.aim),
                ("roll angle", b + 0x1f8, racer.controls.roll),
                ("assist x", b + 0x1c0, racer.assist_x),
                ("assist z", b + 0x1c8, racer.assist_z),
                ("trick since", b + 0x15c, racer.plan.since),
                ("trick estimate", b + 0x19c, racer.plan.estimate),
                ("trick air", b + 0x200, racer.plan.air),
                ("lever steer", r + 0x628, bike.levers.steer),
                ("lever lean", r + 0x62c, bike.levers.lean),
                ("lever gas", r + 0x63c, bike.levers.gas),
                ("lever brake", r + 0x640, bike.levers.brake),
                ("lever reverse", r + 0x648, bike.levers.reverse),
                ("yaw rate", ee.u32_at(r + 0x7a4) + 0x284, bike.yaw_rate),
            ];
            let (engine, body, card_at) = (ee.u32_at(r + 0x7b8), ee.u32_at(r + 0x7a4), ee.u32_at(b + 0xd4));
            let mut h = racer.habits;
            for (at, name, field) in s12e_habit_words(b, r, engine, body, card_at) {
                words.push((name, at, *field(&mut h)));
            }
            // Every word that is off, at once: one wrong input shows up in several.
            let mut off: Vec<String> = words
                .into_iter()
                .filter(|&(_, at, ours)| ee.u32_at(at) != ours.to_bits())
                .map(|(name, at, ours)| format!("{name}: theirs {} ours {}", ee.f32_at(at), ours.to_f32()))
                .collect();
            if ee.u32_at(b + 0x1a0) as i32 != racer.plan.slot {
                off.push(format!("trick slot: theirs {} ours {}", ee.u32_at(b + 0x1a0) as i32, racer.plan.slot));
            }
            if ee.u32_at(r + 0x4a4) as i32 != racer.habits.preload_firing {
                off.push(format!("preload firing: theirs {} ours {}", ee.u32_at(r + 0x4a4) as i32, racer.habits.preload_firing));
            }
            let theirs_seeker = lists.seeker(&ee, b);
            let mut ours_seeker = racer.seeker;
            (ours_seeker.flags, ours_seeker.flags2) = (racer.brain.flags, racer.moves);
            if theirs_seeker != ours_seeker {
                off.push(format!("the seeker:\n  theirs {theirs_seeker:?}\n  ours   {ours_seeker:?}"));
            }
            assert!(off.is_empty(), "{what}:\n{}", off.join("\n"));

            draws += u32::from(rng.state() != before);
            seen.insert(format!("{:?} {}", racer.seeker.state, racer.brain.flags & flag::ON_GROUND != 0));
        }
    }
    // The world the seekers learn into.
    for (i, (j, &t)) in seek_world.jumps.iter().zip(&lists.targets).enumerate() {
        let hit = ee.u32_at(t + 0x3c);
        for (off, leg, name) in [(0, &j.take_off, "take-off"), (0x60, &j.landing, "landing"), (0xc0, &j.setup, "setup")] {
            for (o, v, field) in [(0, leg.velocity, "velocity"), (0x10, leg.aim, "aim"), (0x20, leg.point, "point"), (0x30, leg.facing, "facing")] {
                assert_eq!(got(&ee, hit + off + o), v.to_bits(), "jump {i} {name} {field}");
            }
            assert_eq!(ee.u32_at(hit + off + 0x4c), leg.learnt.to_bits(), "jump {i} {name} learnt");
        }
    }
    println!("{} rider ticks, {draws} drawing; seeker states and ground: {seen:?}", S12E_TICKS * 3);
}
