//! The fixed update, proved against the original's own frame.
//!
//! The frame is not a pure function of its arguments, so the oracle cannot simply be pointed at it: it
//! reads a hardware counter, and it makes four indirect calls into subsystems that are not ported. Two
//! things make it provable anyway, and both are worth understanding before trusting the result.
//!
//! 1. **Hooks.** The device-manager update, the frame-marker notify, the scene root's simulate pass and
//!    the render are replaced by Rust closures, reached through pointers this test writes into a
//!    scratch application object; so are the two interrupt guards, the display ring, the IOP flush and
//!    the eleven mode handlers. Nothing the game *computes* is stubbed — the hooks only record what
//!    they were handed and say what to return. This is also how the test sees the one value the frame
//!    never stores anywhere: the `dt` the simulation is called with.
//! 2. **One patched instruction per clock read.** The two places that sample EE Timer 0 form the
//!    address `0x10000000` with a `lui`, and that address is a register, not memory. The test rewrites
//!    *only* that `lui`, so the counter is read from a word this test owns. Every float instruction in
//!    the clock, and every instruction in the `dt` pipeline, is the game's own and untouched. The
//!    original word is asserted before the patch, so a different build fails loudly instead of quietly
//!    proving nothing.
//!
//! What is proved here, over twelve thousand generated frames and ten thousand clock samples: the
//! millisecond clock (including its unsigned conversion and its behaviour past 2³¹ ms), the first
//! frame's priming, the raw delta, the thirty-entry ring and its index, the long-frame reset, the mean,
//! the 30 fps clamp, the time scale and its floor, the accumulated millisecond step, `1/dt`, the frame
//! counter and its wrap, the phase word, the order of the two frame markers around the simulation, the
//! early return a halted simulation takes (and the stale clock it leaves behind), the frozen frame, the
//! deferred suspend flag, the quit flag's timing, and every mode request's shape frame by frame.
//!
//! What is **not** proved here: what any of the eleven mode handlers do, and what the renderer does.
//! Neither is this task's, and neither can run in an interpreter with no GS. The frame *order* around
//! them is proved; their contents are not.

use std::cell::RefCell;
use std::rc::Rc;

use game_sim::tick::{
    DeltaTime, Engine, Game, Marker, Outcome, Phase, Request, Serviced, Timer0, clock_ms,
};
use game_sim::{Input, rng::Rng};
use ps2_float::F;
use ps2_oracle::{Ee, addr, game_or_skip};

/// The tick is about 450 instructions plus a thirty-entry sum; the mode switch adds a handful.
const BUDGET: u64 = 200_000;

// --- where this test puts the objects the frame walks through ---------------------------------------
// All well past the program image (which ends below 0x4f0000) and well below the interpreter's stack.

/// The application object the whole engine hangs off.
const APP: u32 = 0x0080_0000;
/// The device manager (`app+0x104`), whose table holds the per-frame update.
const DEVICE: u32 = 0x0080_1000;
const DEVICE_TABLE: u32 = 0x0080_1100;
/// The application's own dispatch table (`app+0x3c4`): the marker notify and the render live in it.
const DISPATCH: u32 = 0x0080_1200;
/// The scene tree's root node and its table — the simulation is one slot in it.
const ROOT: u32 = 0x0080_1300;
const ROOT_TABLE: u32 = 0x0080_1400;
/// The display singleton: the mode switch raises a flag in it, and the quit path calls through it.
const DISPLAY: u32 = 0x0080_1500;
const DISPLAY_TABLE: u32 = 0x0080_1550;
/// The UI manager (`app+0x3d0`): its "still loading" pair guards two of the requests.
const UI: u32 = 0x0080_1600;
/// … and the second half of that pair is a pointer to a pointer.
const UI_LOADING_PTR: u32 = 0x0080_1700;
/// The settings object (`app+0x3d4`), only ever dereferenced for its event.
const SETTINGS: u32 = 0x0080_1800;
/// The time-scale object, when a test wants slow motion.
const TIME_SCALE: u32 = 0x0080_1900;
/// The word that stands in for the timer's counter register.
const COUNT_WORD: u32 = 0x0081_0000;

/// Addresses the hooks live at: not code, just somewhere for a table entry to point.
const HOOK_DEVICE: u32 = 0x0082_0000;
const HOOK_MARKER: u32 = 0x0082_0010;
const HOOK_SIM: u32 = 0x0082_0020;
const HOOK_RENDER: u32 = 0x0082_0030;
const HOOK_DISPLAY: u32 = 0x0082_0040;

// --- the fields of the application object this test reads and writes --------------------------------

const F_SIM_MS: u32 = 0x330;
const F_PREV_SIM_MS: u32 = 0x334;
const F_CLOCK: u32 = 0x338;
const F_PREV_CLOCK: u32 = 0x33c;
/// The flag word. Bit 3 of its first byte is the simulation gate; bit 4 of its *second* byte asks for
/// bit 3 of that second byte at the end of the frame. Two unrelated things one byte apart.
const F_FLAGS: u32 = 0x324;
const F_LATCH_BYTE: u32 = 0x325;
const GATE_BIT: u32 = 0x08;
const SUSPEND_REQUEST_BIT: u8 = 0x10;
const SUSPENDED_BIT: u8 = 0x08;
const F_PHASE: u32 = 0x32c;
const F_DT: u32 = 0x344;
const F_INV_DT: u32 = 0x348;
const F_RAW_DT: u32 = 0x358;
const F_FRAME: u32 = 0x374;
const F_DEVICE: u32 = 0x104;
const F_ROOT: u32 = 0x378;
const F_PROFILING: u32 = 0x3c0;
const F_DISPATCH: u32 = 0x3c4;
const F_UI: u32 = 0x3d0;
const F_SETTINGS: u32 = 0x3d4;
const F_MODE_REQUEST: u32 = 0x404;
const F_QUIT: u32 = 0x454;
const F_DELAY_ARMED: u32 = 0x578;
const F_DELAY_COUNT: u32 = 0x57c;

/// `lui $2, 0x1000` — how both clock reads form the address of Timer 0's counter.
const LUI_TIMER_REGISTER: u32 = 0x3c02_1000;
/// The same instruction pointing at [`COUNT_WORD`] instead.
const LUI_COUNT_WORD: u32 = 0x3c02_0000 | (COUNT_WORD >> 16);
/// Where that instruction sits in each of the two functions that sample the clock.
const CLOCK_READ_IN_TICK: u32 = 0x280;
const CLOCK_READ_IN_PRIME: u32 = 0x20;

/// What the hooks saw, in the order they saw it.
#[derive(Default)]
struct Log {
    /// The `dt` the device-manager update was handed — the engine's only float argument to it.
    dt_to_input: Vec<f32>,
    /// The `dt` the simulation was handed. The engine stores this too, but only on frames it runs.
    dt_to_sim: Vec<f32>,
    /// Marker ids, in order.
    markers: Vec<u32>,
    /// Mode handlers, in call order, by the name `research/oracle.txt` gives them.
    handlers: Vec<&'static str>,
    /// What the simulation hook returns. Zero is "the simulation asked to stop".
    sim_returns: u64,
}

impl Log {
    fn clear(&mut self) {
        self.dt_to_input.clear();
        self.dt_to_sim.clear();
        self.markers.clear();
        self.handlers.clear();
    }
}

/// The original's frame, set up so it can be called like a function.
struct Original {
    ee: Ee,
    log: Rc<RefCell<Log>>,
    engine_tick: u32,
    game_tick: u32,
}

impl Original {
    /// Load the game, patch the two clock reads, and wire a scratch application object up to hooks.
    /// `None` (with a printed reason) when there is no disc to read.
    fn new() -> Option<Original> {
        let mut ee = game_or_skip()?;
        let engine_tick = at("engine_tick");
        let game_tick = at("game_tick");

        for patch in [
            engine_tick + CLOCK_READ_IN_TICK,
            at("engine_clock_prime") + CLOCK_READ_IN_PRIME,
        ] {
            assert_eq!(
                ee.u32_at(patch),
                LUI_TIMER_REGISTER,
                "{patch:#010x} is not the `lui` that forms the timer register's address — this is a \
                 different build of the game, and the rest of this test would prove nothing"
            );
            ee.set_u32(patch, LUI_COUNT_WORD);
        }

        let log = Rc::new(RefCell::new(Log::default()));

        // The four subsystems the frame calls through its own pointers.
        let l = log.clone();
        ee.hooks.insert(
            HOOK_DEVICE,
            Box::new(move |ee: &mut Ee| l.borrow_mut().dt_to_input.push(ee.f(12))),
        );
        let l = log.clone();
        ee.hooks.insert(
            HOOK_MARKER,
            Box::new(move |ee: &mut Ee| l.borrow_mut().markers.push(ee.r(5) as u32)),
        );
        let l = log.clone();
        ee.hooks.insert(
            HOOK_SIM,
            Box::new(move |ee: &mut Ee| {
                let mut log = l.borrow_mut();
                log.dt_to_sim.push(ee.f(12));
                let r = log.sim_returns;
                ee.set(2, r);
            }),
        );
        ee.hooks.insert(HOOK_RENDER, Box::new(|_: &mut Ee| {}));
        ee.hooks.insert(HOOK_DISPLAY, Box::new(|_: &mut Ee| {}));

        // Everything the frame calls by name that this test does not want to run: the interrupt guards
        // around the clock read, the display-list ring, and the IOP's command flush.
        for name in [
            "interrupts_off",
            "interrupts_on",
            "display_ring_advance",
            "iop_command_flush",
        ] {
            ee.hooks.insert(at(name), Box::new(|_: &mut Ee| {}));
        }
        // The mode switch's handlers, which record that they ran.
        for name in [
            "mode_enter_world",
            "mode_leave_world",
            "mode_build_front_end",
            "mode_tear_down_front_end",
            "mode_reset_viewports",
            "mode_commit_event",
            "mode_shutdown",
            "mode_loading_begin",
            "mode_loading_end",
            "mode_world_wait",
            "mode_ui_flush",
        ] {
            let l = log.clone();
            ee.hooks.insert(
                at(name),
                Box::new(move |_: &mut Ee| l.borrow_mut().handlers.push(name)),
            );
        }

        let mut o = Original {
            ee,
            log,
            engine_tick,
            game_tick,
        };
        o.wire();
        Some(o)
    }

    /// The pointers that do not change from frame to frame.
    fn wire(&mut self) {
        let ee = &mut self.ee;
        ee.set_u32(APP + F_DEVICE, DEVICE);
        ee.set_u32(DEVICE + 0x65c, DEVICE_TABLE);
        ee.set_u32(DEVICE_TABLE + 0x10, HOOK_DEVICE);
        ee.set_u32(APP + F_DISPATCH, DISPATCH);
        ee.set_u32(DISPATCH + 0x80, HOOK_MARKER);
        ee.set_u32(DISPATCH + 0x38, HOOK_RENDER);
        ee.set_u32(APP + F_ROOT, ROOT);
        ee.set_u32(ROOT + 0x08, ROOT_TABLE);
        ee.set_u32(ROOT_TABLE + 0x38, HOOK_SIM);
        ee.set_u32(APP + F_PROFILING, 0);
        ee.set_u32(APP + F_UI, UI);
        ee.set_u32(UI + 0x104, UI_LOADING_PTR);
        ee.set_u32(APP + F_SETTINGS, SETTINGS);
        ee.set_u32(DISPLAY + 0x50, DISPLAY_TABLE);
        ee.set_u32(DISPLAY_TABLE + 0x28, HOOK_DISPLAY);
        // The globals the frame and the mode switch reach for by name.
        ee.set_u32(at("boot_countdown"), 0);
        ee.set_u32(at("display_object"), DISPLAY);
        ee.set_u32(at("app_pointer"), APP);
        self.log.borrow_mut().sim_returns = 1;
    }

    /// Put the engine back where it is on the game's very first frame.
    fn restart(&mut self, cap_30: bool, time_scale: Option<f32>) {
        for field in [
            F_SIM_MS,
            F_PREV_SIM_MS,
            F_CLOCK,
            F_PREV_CLOCK,
            F_FLAGS,
            F_PHASE,
            F_DT,
            F_INV_DT,
            F_RAW_DT,
            F_FRAME,
            F_MODE_REQUEST,
            F_QUIT,
            F_DELAY_ARMED,
            F_DELAY_COUNT,
        ] {
            self.ee.set_u32(APP + field, 0);
        }
        self.ee.set_u32(at("dt_ring_index"), 0);
        self.ee.set_u32(at("dt_ring_primed"), 0);
        for k in 0..DeltaTime::RING as u32 {
            self.ee.set_f32(at("dt_ring") + 4 * k, 0.0);
        }
        self.ee.set_u32(at("fps30_cap"), cap_30 as u32);
        match time_scale {
            Some(s) => {
                self.ee.set_u32(at("time_scale_object"), TIME_SCALE);
                self.ee.set_f32(TIME_SCALE + 0x40, s);
            }
            None => self.ee.set_u32(at("time_scale_object"), 0),
        }
        self.ee.set_u32(UI + 0x100, 0);
        self.ee.set_u32(UI_LOADING_PTR, 0);
        self.log.borrow_mut().clear();
    }

    /// Put the clock where the test says it is, and clear the log.
    fn arrive_at(&mut self, now: Timer0) {
        self.log.borrow_mut().clear();
        self.ee.set_u32(at("wall_ticks"), now.ticks);
        self.ee.set_u32(COUNT_WORD, now.count);
    }

    /// One frame through the engine's tick.
    fn engine_frame(&mut self, now: Timer0) {
        self.arrive_at(now);
        let f = self.engine_tick;
        self.ee.call(f, &[APP as u64], &[], BUDGET);
    }

    /// One frame through the game's tick — the mode switch and the quit check included.
    fn game_frame(&mut self, now: Timer0) -> bool {
        self.arrive_at(now);
        let f = self.game_tick;
        self.ee.call(f, &[APP as u64], &[], BUDGET);
        self.ee.v0() & 1 != 0
    }

    fn i32_field(&self, field: u32) -> i32 {
        self.ee.i32_at(APP + field)
    }
    fn u32_field(&self, field: u32) -> u32 {
        self.ee.u32_at(APP + field)
    }
    fn bits(&self, field: u32) -> u32 {
        self.ee.u32_at(APP + field)
    }
    fn ring(&self) -> Vec<u32> {
        (0..DeltaTime::RING as u32)
            .map(|k| self.ee.u32_at(at("dt_ring") + 4 * k))
            .collect()
    }
}

/// One named address out of `research/oracle.txt`, or a panic saying which name is missing.
fn at(name: &str) -> u32 {
    addr(name).unwrap_or_else(|e| panic!("{e}"))
}

/// A tiny xorshift, so the generated frames are the same on every machine and a failure can be
/// reproduced from the seed in the source alone.
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// The port's game: a recorder, with a switch for the frame that asks to stop.
struct Recorder {
    dt_to_sim: Vec<f32>,
    halt: bool,
}

impl Recorder {
    fn new() -> Recorder {
        Recorder {
            dt_to_sim: Vec::new(),
            halt: false,
        }
    }
}

impl Game for Recorder {
    fn simulate(&mut self, dt: F, _input: Input, _rng: &mut Rng) -> bool {
        self.dt_to_sim.push(dt.to_f32());
        !self.halt
    }
}

/// The counter value for a given millisecond, as the timer would hold it: 576 counts per millisecond,
/// 57,600 per interrupt.
fn timer_at(ms: u64) -> Timer0 {
    let counts = ms * 576;
    Timer0 {
        ticks: (counts / 57_600) as u32,
        count: (counts % 57_600) as u32,
    }
}

/// Compare every value the frame leaves behind, and say which one differed.
#[track_caller]
fn same_frame(o: &Original, e: &Engine, port: &Recorder, at_: &str) {
    assert_eq!(o.i32_field(F_CLOCK), e.clock_ms, "clock: {at_}");
    assert_eq!(
        o.u32_field(F_RAW_DT),
        e.dt.raw.to_bits(),
        "raw dt: {at_}"
    );
    for (k, (got, want)) in o.ring().iter().zip(e.dt.ring.iter()).enumerate() {
        assert_eq!(*got, want.to_bits(), "ring[{k}]: {at_}");
    }
    assert_eq!(
        o.ee.u32_at(at("dt_ring_index")),
        e.dt.index,
        "ring index: {at_}"
    );
    assert_eq!(
        o.ee.u32_at(at("dt_ring_primed")) != 0,
        e.dt.primed,
        "ring primed: {at_}"
    );
    assert_eq!(o.u32_field(F_DT), e.dt_s.to_bits(), "dt: {at_}");
    assert_eq!(o.u32_field(F_INV_DT), e.inv_dt.to_bits(), "1/dt: {at_}");
    assert_eq!(o.u32_field(F_FRAME), e.frame, "frame counter: {at_}");
    assert_eq!(o.i32_field(F_SIM_MS), e.sim_ms, "accumulated ms: {at_}");
    assert_eq!(
        o.i32_field(F_PREV_SIM_MS),
        e.prev_sim_ms,
        "previous accumulated ms: {at_}"
    );
    assert_eq!(
        o.i32_field(F_PREV_CLOCK),
        e.prev_clock_ms,
        "previous clock: {at_}"
    );
    assert_eq!(o.u32_field(F_PHASE), e.phase.code(), "phase: {at_}");
    assert_eq!(
        o.bits(F_FLAGS) >> 8 & 0xff,
        u32::from(
            u8::from(e.suspended) * SUSPENDED_BIT + u8::from(e.suspend_requested) * SUSPEND_REQUEST_BIT
        ),
        "the deferred flag byte: {at_}"
    );

    let log = o.log.borrow();
    assert_eq!(
        log.dt_to_sim.len(),
        port.dt_to_sim.len(),
        "the simulation ran the same number of times: {at_}"
    );
    for (got, want) in log.dt_to_sim.iter().zip(&port.dt_to_sim) {
        assert_eq!(
            got.to_bits(),
            want.to_bits(),
            "the dt handed to the simulation: {at_}"
        );
    }
}

/// **The proof.** Two hundred runs of sixty frames each — twelve thousand frames — over clocks that
/// cover a 30 fps game, a 60 fps one, jitter, level-load gaps, the 30 fps cap on and off, slow motion,
/// fast forward, and a clock far enough along that the millisecond count no longer fits in a
/// single-precision mantissa.
#[test]
fn the_frame_matches_the_original() {
    let Some(mut o) = Original::new() else { return };
    let mut pick = Pick(0x5f1d_2b47_9ace_1357);
    let mut frames = 0usize;

    for seq in 0..200u64 {
        let cap_30 = seq % 3 != 0;
        let scale = match seq % 5 {
            0 => None,
            1 => Some(0.0),
            2 => Some(-0.5),  // slow motion
            3 => Some(2.0),   // fast forward
            _ => Some(-1.05), // past the floor, which is the branch that rewrites the ms step
        };
        // Four starting points for the clock, the last of them just short of 2³¹ ms — about 25 days,
        // which the game will never see, but the conversion that handles it runs on every frame and is
        // the one a port writes differently without noticing.
        let start_ms = match seq % 4 {
            0 => 1,
            1 => 1_000 + pick.below(100_000),
            2 => 2_000_000_000 + pick.below(1_000_000),
            _ => 2_147_480_000 + pick.below(10_000),
        };

        o.restart(cap_30, scale);
        let mut e = Engine::new();
        e.cap_30 = cap_30;
        e.time_scale = scale.map(F);
        let mut port = Recorder::new();

        let mut ms = start_ms;
        for frame in 0..60 {
            ms += match pick.below(20) {
                0 => 1 + pick.below(16),       // a short frame
                1 => 200 + pick.below(20_000), // a load, or a stall
                k if k < 11 => 33 + pick.below(2),
                _ => 16 + pick.below(2),
            };
            let now = timer_at(ms);
            let where_ = format!("sequence {seq}, frame {frame}, clock {ms} ms");

            o.engine_frame(now);
            port.dt_to_sim.clear();
            let step = e.step(now, Input::default(), &mut port);

            same_frame(&o, &e, &port, &where_);
            assert_eq!(
                o.log.borrow().dt_to_input,
                vec![step.dt.to_f32()],
                "the pads are updated once, with the frame's dt: {where_}"
            );
            assert_eq!(
                o.log.borrow().markers,
                step.markers.iter().map(|m| m as u32).collect::<Vec<_>>(),
                "the markers and their order: {where_}"
            );
            frames += 1;
        }
    }
    assert!(frames >= 12_000, "only {frames} frames compared");
}

/// The clock on its own, over the whole range of the counter and of the interrupt count, including the
/// values where the single-precision arithmetic starts chopping and where the unsigned conversion takes
/// over from the signed one.
#[test]
fn the_wall_clock_matches_the_original() {
    let Some(mut o) = Original::new() else { return };
    let mut pick = Pick(0x1b7e_4d90_3f62_a8c5);

    for case in 0..10_000u64 {
        let ticks = match case % 4 {
            0 => pick.below(2_000) as u32,
            1 => pick.below(1_000_000) as u32,
            2 => (21_474_000 + pick.below(10_000)) as u32, // either side of 2³¹ ms
            _ => pick.next() as u32,                       // including counts that wrap the word
        };
        let now = Timer0 {
            ticks,
            count: pick.below(57_600) as u32,
        };
        // The clock is sampled by the frame, so the frame is how the test gets at it: one restart, one
        // frame, and the engine's own millisecond word is the answer.
        o.restart(true, None);
        o.engine_frame(now);
        assert_eq!(
            o.i32_field(F_CLOCK),
            clock_ms(now),
            "the clock at {now:?} (case {case})"
        );
    }
}

/// The first frame primes both ends of the delta instead of measuring one, so it steps nothing: dt is
/// exactly zero, and the accumulated clock starts at the wall clock rather than at zero.
#[test]
fn the_first_frame_primes_the_clock() {
    let Some(mut o) = Original::new() else { return };
    o.restart(true, None);
    let mut e = Engine::new();
    let mut port = Recorder::new();

    let now = timer_at(123_456);
    o.engine_frame(now);
    let step = e.step(now, Input::default(), &mut port);

    same_frame(&o, &e, &port, "the first frame");
    assert_eq!(step.dt, F(0.0));
    assert_eq!(step.ms_step, 0);
    assert_eq!(e.sim_ms, clock_ms(now), "the accumulated clock starts here");
    assert_eq!(o.i32_field(F_SIM_MS), clock_ms(now));
}

/// A frozen frame still keeps time and still reads the pads; it just does not simulate, does not count
/// and does not render.
#[test]
fn a_frozen_frame_keeps_time_but_does_not_simulate() {
    let Some(mut o) = Original::new() else { return };
    o.restart(true, None);
    let mut e = Engine::new();
    let mut port = Recorder::new();

    for (frame, ms) in [100u64, 133].into_iter().enumerate() {
        let now = timer_at(ms);
        o.engine_frame(now);
        port.dt_to_sim.clear();
        e.step(now, Input::default(), &mut port);
        same_frame(&o, &e, &port, &format!("warm-up frame {frame}"));
    }

    o.ee.write(APP + F_FLAGS, 1, u128::from(GATE_BIT));
    e.frozen = true;
    let now = timer_at(166);
    o.engine_frame(now);
    port.dt_to_sim.clear();
    let step = e.step(now, Input::default(), &mut port);

    same_frame(&o, &e, &port, "the frozen frame");
    assert!(o.log.borrow().dt_to_sim.is_empty(), "the simulation was skipped");
    assert!(!step.simulated);
    assert_eq!(
        o.log.borrow().dt_to_input.len(),
        1,
        "the pads were read anyway"
    );
    assert_eq!(
        o.log.borrow().markers,
        vec![Marker::FrameStart as u32],
        "only the top-of-frame marker: the second one is inside the gate"
    );
    assert_eq!(step.markers.iter().count(), 1);
    assert_ne!(e.dt.raw, F(0.0), "and the clock still ran");
}

/// A simulation that asks to stop takes the frame's early return: no second marker, no render, and —
/// the part a port forgets — the clock the *next* frame measures from is left where it was.
#[test]
fn a_halted_simulation_leaves_the_clock_where_it_was() {
    let Some(mut o) = Original::new() else { return };
    o.restart(true, None);
    let mut e = Engine::new();
    let mut port = Recorder::new();

    let now = timer_at(100);
    o.engine_frame(now);
    e.step(now, Input::default(), &mut port);
    same_frame(&o, &e, &port, "the frame before");

    // The frame that halts.
    o.log.borrow_mut().sim_returns = 0;
    port.halt = true;
    let now = timer_at(133);
    let keep_going = o.game_frame(now);
    port.dt_to_sim.clear();
    let step = e.step(now, Input::default(), &mut port);

    same_frame(&o, &e, &port, "the halted frame");
    assert!(step.sim_halted);
    assert_eq!(
        o.log.borrow().markers,
        vec![Marker::FrameStart as u32],
        "the frame stopped at the simulation"
    );
    assert_eq!(step.markers.iter().count(), 1);
    assert!(keep_going, "the game's tick ignores the engine's return value");
    assert_eq!(step.outcome, Outcome::Continue);
    assert_eq!(e.prev_clock_ms, clock_ms(timer_at(100)), "the stale clock");
    assert_eq!(o.u32_field(F_PHASE), Phase::Simulate.code(), "and the phase");

    // … and the next frame measures a 66 ms delta, not a 33 ms one.
    o.log.borrow_mut().sim_returns = 1;
    port.halt = false;
    let now = timer_at(166);
    o.engine_frame(now);
    port.dt_to_sim.clear();
    let step = e.step(now, Input::default(), &mut port);
    same_frame(&o, &e, &port, "the frame after");
    assert_eq!(step.markers.iter().count(), 2);
    assert_eq!(e.dt.raw, F(0.066), "two frames' worth of time, in one");
}

/// The deferred flag the end of the frame raises: asked for on one frame, set on that frame's very last
/// instruction, and the request cleared with it. It is not the simulation gate — the frame it is asked
/// on simulates, and so does every frame after.
#[test]
fn the_deferred_flag_is_raised_at_the_end_of_the_frame() {
    let Some(mut o) = Original::new() else { return };
    o.restart(true, None);
    let mut e = Engine::new();
    let mut port = Recorder::new();

    let now = timer_at(100);
    o.engine_frame(now);
    e.step(now, Input::default(), &mut port);

    o.ee.write(APP + F_LATCH_BYTE, 1, u128::from(SUSPEND_REQUEST_BIT));
    e.suspend_requested = true;
    let now = timer_at(133);
    o.engine_frame(now);
    port.dt_to_sim.clear();
    let step = e.step(now, Input::default(), &mut port);

    same_frame(&o, &e, &port, "the frame the flag was asked for on");
    assert_eq!(o.ee.u8_at(APP + F_LATCH_BYTE), SUSPENDED_BIT);
    assert!(e.suspended && !e.suspend_requested);
    assert!(step.simulated, "and the frame ran as usual");
}

/// The quit flag is checked *after* the frame, so the frame it is raised on still simulates — and the
/// loop ends on that frame, not the next one.
#[test]
fn the_quit_flag_ends_the_loop_after_the_frame() {
    let Some(mut o) = Original::new() else { return };
    o.restart(true, None);
    let mut e = Engine::new();
    let mut port = Recorder::new();

    let now = timer_at(100);
    assert!(o.game_frame(now));
    e.step(now, Input::default(), &mut port);

    o.ee.set_u32(APP + F_QUIT, 1);
    e.quit = true;
    let now = timer_at(133);
    let keep_going = o.game_frame(now);
    port.dt_to_sim.clear();
    let step = e.step(now, Input::default(), &mut port);

    same_frame(&o, &e, &port, "the quitting frame");
    assert!(!keep_going);
    assert_eq!(step.outcome, Outcome::Quit);
    assert!(step.simulated, "the frame still ran");
    assert_eq!(o.log.borrow().dt_to_sim.len(), 1);
}

/// The frame counter wraps at 2³¹−1, not at 2³².
#[test]
fn the_frame_counter_wraps_where_the_original_wraps() {
    let Some(mut o) = Original::new() else { return };
    o.restart(true, None);
    let mut e = Engine::new();
    let mut port = Recorder::new();
    o.ee.set_u32(APP + F_FRAME, 0x7fff_fffd);
    e.frame = 0x7fff_fffd;
    // 0x7ffffffe is the last frame number: the test is `0x7ffffffe < frame`, after the increment, so
    // the number the wrap skips is 0x7fffffff and the one after the last is 0 — not 0x7ffffffe again.
    for (frame, ms, want) in [
        (0, 100u64, 0x7fff_fffe),
        (1, 133, 0),
        (2, 166, 1),
    ] {
        let now = timer_at(ms);
        o.engine_frame(now);
        port.dt_to_sim.clear();
        e.step(now, Input::default(), &mut port);
        same_frame(&o, &e, &port, &format!("frame {frame}"));
        assert_eq!(e.frame, want, "frame {frame}");
    }
}

/// The phase word is 2 only while the simulation runs, and 0 everywhere a sample from outside the frame
/// could land.
#[test]
fn the_phase_is_two_only_inside_the_simulation() {
    let Some(mut o) = Original::new() else { return };
    o.restart(true, None);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    o.ee.hooks.insert(
        HOOK_SIM,
        Box::new(move |ee: &mut Ee| {
            s.borrow_mut().push(ee.u32_at(APP + F_PHASE));
            ee.set(2, 1);
        }),
    );
    o.engine_frame(timer_at(100));
    o.engine_frame(timer_at(133));
    assert_eq!(*seen.borrow(), vec![Phase::Simulate.code(); 2]);
    assert_eq!(o.u32_field(F_PHASE), Phase::Idle.code());
}

/// The mode mailbox: which handlers each request calls, on which frame, and what the request word holds
/// afterwards — the guards, the three-frame shape of a delayed request, the restart's chain into a
/// world build, and the one request that does not run the frame.
#[test]
fn the_mode_requests_are_serviced_the_way_the_original_services_them() {
    let Some(mut o) = Original::new() else { return };
    o.restart(true, None);
    let mut e = Engine::new();

    /// The port's side: the same recording the hooks do.
    struct Modes {
        loading: bool,
        transitions: Vec<Request>,
        shutdowns: usize,
        frames: usize,
    }
    impl Game for Modes {
        fn simulate(&mut self, _: F, _: Input, _: &mut Rng) -> bool {
            self.frames += 1;
            true
        }
        fn ui_loading(&self) -> bool {
            self.loading
        }
        fn transition(&mut self, t: Request) {
            self.transitions.push(t);
        }
        fn shutdown(&mut self) {
            self.shutdowns += 1;
        }
    }
    let mut port = Modes {
        loading: false,
        transitions: Vec::new(),
        shutdowns: 0,
        frames: 0,
    };

    let mut ms = 1_000u64;
    let mut frame = |o: &mut Original, e: &mut Engine, port: &mut Modes| {
        ms += 33;
        let now = timer_at(ms);
        let keep_going = o.game_frame(now);
        let step = e.step_at(clock_ms(now), Input::default(), port);
        // Whatever the request did, the two agree on the frame around it.
        assert_eq!(o.i32_field(F_CLOCK), e.clock_ms, "clock at {ms} ms");
        assert_eq!(o.u32_field(F_DT), e.dt_s.to_bits(), "dt at {ms} ms");
        assert_eq!(o.u32_field(F_FRAME), e.frame, "frame number at {ms} ms");
        assert_eq!(o.u32_field(F_MODE_REQUEST), e.mode.request, "request word");
        assert_eq!(o.i32_field(F_DELAY_ARMED) != 0, e.mode.armed, "armed");
        assert_eq!(o.i32_field(F_DELAY_COUNT), e.mode.delay, "delay");
        assert_eq!(
            o.log.borrow().dt_to_sim.len(),
            usize::from(step.simulated),
            "whether the frame ran, at {ms} ms"
        );
        (keep_going, step)
    };

    // 1 — the world build: one frame, one handler, the request cleared, and the frame still runs.
    o.ee.set_u32(APP + F_MODE_REQUEST, 1);
    e.mode.post(Request::EnterWorld);
    let (keep_going, step) = frame(&mut o, &mut e, &mut port);
    assert!(keep_going);
    assert_eq!(o.log.borrow().handlers, vec!["mode_enter_world"]);
    assert_eq!(step.serviced, Serviced::Performed(Request::EnterWorld));
    assert_eq!(e.mode.request, 0);
    assert!(step.simulated);

    // 2 — guarded by the UI: the frame still ticks, the request stays pending.
    o.ee.set_u32(UI + 0x100, 1);
    port.loading = true;
    o.ee.set_u32(APP + F_MODE_REQUEST, 2);
    e.mode.post(Request::WorldToMenu);
    let (_, step) = frame(&mut o, &mut e, &mut port);
    assert!(o.log.borrow().handlers.is_empty(), "no handler ran");
    assert_eq!(step.serviced, Serviced::Waiting(Request::WorldToMenu));
    assert_eq!(e.mode.request, 2);
    assert!(step.simulated, "a guarded frame is an ordinary frame");

    // The other half of the guard is a pointer to a flag, not a flag: still loading.
    o.ee.set_u32(UI + 0x100, 0);
    o.ee.set_u32(UI_LOADING_PTR, 1);
    let (_, step) = frame(&mut o, &mut e, &mut port);
    assert!(o.log.borrow().handlers.is_empty(), "still guarded");
    assert_eq!(step.serviced, Serviced::Waiting(Request::WorldToMenu));

    // … and once the UI is done, the six handlers in their order.
    o.ee.set_u32(UI_LOADING_PTR, 0);
    port.loading = false;
    let (_, step) = frame(&mut o, &mut e, &mut port);
    assert_eq!(
        o.log.borrow().handlers,
        vec![
            "mode_world_wait",
            "mode_reset_viewports",
            "mode_loading_begin",
            "mode_leave_world",
            "mode_build_front_end",
            "mode_loading_end",
        ]
    );
    assert_eq!(step.serviced, Serviced::Performed(Request::WorldToMenu));
    assert_eq!(e.mode.request, 0);

    // 5 — the front end rebuilt in place: unguarded, one frame.
    o.ee.set_u32(APP + F_MODE_REQUEST, 5);
    e.mode.post(Request::RebuildFrontEnd);
    let (_, step) = frame(&mut o, &mut e, &mut port);
    assert_eq!(
        o.log.borrow().handlers,
        vec!["mode_loading_begin", "mode_build_front_end", "mode_loading_end"]
    );
    assert_eq!(step.serviced, Serviced::Performed(Request::RebuildFrontEnd));

    // 4 and 6 — the delayed pair: arm, count, act. Three frames, and all three tick.
    for (code, request, armed_with, acted_with) in [
        (
            4u32,
            Request::CheckToMenu,
            vec!["mode_ui_flush"],
            vec![
                "mode_tear_down_front_end",
                "mode_loading_begin",
                "mode_build_front_end",
                "mode_loading_end",
            ],
        ),
        (
            6,
            Request::MenuToWorld,
            vec![],
            vec![
                "mode_tear_down_front_end",
                "mode_commit_event",
                "mode_enter_world",
            ],
        ),
    ] {
        o.ee.set_u32(APP + F_MODE_REQUEST, code);
        e.mode.post(request);

        let (_, step) = frame(&mut o, &mut e, &mut port);
        assert_eq!(o.log.borrow().handlers, armed_with, "the arming frame of {code}");
        assert_eq!(step.serviced, Serviced::Waiting(request));
        assert!(e.mode.armed);
        assert_eq!(e.mode.delay, 0);
        assert!(step.simulated);

        let (_, step) = frame(&mut o, &mut e, &mut port);
        assert!(o.log.borrow().handlers.is_empty(), "the waiting frame of {code}");
        assert_eq!(step.serviced, Serviced::Waiting(request));
        assert_eq!(e.mode.delay, 1);

        let (_, step) = frame(&mut o, &mut e, &mut port);
        assert_eq!(o.log.borrow().handlers, acted_with, "the acting frame of {code}");
        assert_eq!(step.serviced, Serviced::Performed(request));
        assert!(!e.mode.armed);
        assert_eq!(e.mode.delay, 0);
        assert_eq!(e.mode.request, 0);
        assert!(step.simulated);
    }

    // 7 — the restart chains into 1 rather than clearing.
    o.ee.set_u32(APP + F_MODE_REQUEST, 7);
    e.mode.post(Request::RestartEvent);
    let (_, step) = frame(&mut o, &mut e, &mut port);
    assert_eq!(o.log.borrow().handlers, vec!["mode_leave_world"]);
    assert_eq!(step.serviced, Serviced::Performed(Request::RestartEvent));
    assert_eq!(e.mode.request, 1, "chained");
    let (_, _) = frame(&mut o, &mut e, &mut port);
    assert_eq!(o.log.borrow().handlers, vec!["mode_enter_world"]);
    assert_eq!(e.mode.request, 0);

    // 3 — not a request at all: the switch has no such case, so nothing happens and the word stays.
    o.ee.set_u32(APP + F_MODE_REQUEST, 3);
    e.mode.request = 3;
    let (_, step) = frame(&mut o, &mut e, &mut port);
    assert!(o.log.borrow().handlers.is_empty());
    assert_eq!(step.serviced, Serviced::Nothing);
    assert_eq!(e.mode.request, 3);
    assert!(step.simulated);
    o.ee.set_u32(APP + F_MODE_REQUEST, 0);
    e.mode.request = 0;

    // 8 — the shutdown is the one request that does not run the frame.
    o.ee.set_u32(APP + F_MODE_REQUEST, 8);
    e.mode.post(Request::Shutdown);
    let (keep_going, step) = frame(&mut o, &mut e, &mut port);
    assert_eq!(o.log.borrow().handlers, vec!["mode_shutdown"]);
    assert!(o.log.borrow().markers.is_empty(), "the frame did not run");
    assert!(!keep_going);
    assert_eq!(step.outcome, Outcome::Quit);
    assert!(!step.simulated);
    assert_eq!(port.shutdowns, 1);
    assert_eq!(e.mode.request, 0);

    assert_eq!(
        port.transitions,
        vec![
            Request::EnterWorld,
            Request::WorldToMenu,
            Request::RebuildFrontEnd,
            Request::CheckToMenu,
            Request::MenuToWorld,
            Request::RestartEvent,
            Request::EnterWorld,
            Request::Shutdown,
        ]
    );
}

// ---------------------------------------------------------------------------------------------------
// The second proof: six hundred frames of the game actually running.
//
// The oracle above proves the port against the *code*. This proves it against a recording of the code
// running on an emulated console — the clock a real Timer 0 produced, with the jitter a real frame has.
// The capture is game data, so it is git-ignored and the test says why it skipped when it is not there.
// ---------------------------------------------------------------------------------------------------

/// C4's recording: 600 consecutive engine frames at the main menu, one row per frame, with the clock,
/// the dt pair and the whole ring in every row.
const CAPTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../context/captures/timing-menu-600.csv"
);

/// One row of it, by the column names the spec writes.
struct Row {
    clock_ms: i32,
    cap_30: bool,
    index: u32,
    dt: f32,
    inv_dt: f32,
    raw: f32,
    ring: Vec<f32>,
}

fn capture() -> Option<Vec<Row>> {
    let Ok(text) = std::fs::read_to_string(CAPTURE) else {
        ps2_oracle::skip(concat!(
            "no context/captures/timing-menu-600.csv — the recording is game data, so it is not in ",
            "the repository. Remake it with: ./kit record watch/timing.toml --state 8 --frames 600 ",
            "-o context/captures/timing-menu-600.csv"
        ));
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().expect("a header").split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name).expect(name);
    let (clock, cap, index, dt, inv_dt, raw) = (
        col("clock_ms"),
        col("fps30_cap"),
        col("dt_ring_index"),
        col("dt"),
        col("inv_dt"),
        col("dt_raw"),
    );
    let ring: Vec<usize> = (0..DeltaTime::RING).map(|k| col(&format!("ring{k}.dt"))).collect();
    Some(
        lines
            .map(|line| {
                let f: Vec<&str> = line.split(',').collect();
                Row {
                    clock_ms: f[clock].parse::<u32>().expect("clock_ms") as i32,
                    cap_30: f[cap] != "0",
                    index: f[index].parse().expect("dt_ring_index"),
                    dt: f[dt].parse().expect("dt"),
                    inv_dt: f[inv_dt].parse().expect("inv_dt"),
                    raw: f[raw].parse().expect("dt_raw"),
                    ring: ring.iter().map(|k| f[*k].parse().expect("ring")).collect(),
                }
            })
            .collect(),
    )
}

/// Replay the recorded clock through the port's pipeline and require every frame's `dt`, raw delta,
/// ring, index and reciprocal to come out bit for bit as the console had them.
///
/// Only the clock column is an input. Everything else in the row is an answer.
#[test]
fn the_recorded_frames_replay_bit_for_bit() {
    let Some(rows) = capture() else { return };
    assert!(rows.len() > 500, "only {} rows", rows.len());

    // Start from the recording's own first row: the ring is thirty frames of history this replay was
    // not there for.
    let first = &rows[0];
    let mut dt = DeltaTime {
        ring: std::array::from_fn(|k| F(first.ring[k])),
        index: first.index,
        primed: true,
        raw: F(first.raw),
    };

    for (n, row) in rows.iter().enumerate().skip(1) {
        let delta_ms = row.clock_ms.wrapping_sub(rows[n - 1].clock_ms);
        let (got, _) = dt.update(delta_ms, row.cap_30, None);
        let inv_dt = if got > 0.0 { F(1.0) / got } else { F(1.0) };

        assert_eq!(dt.raw.to_bits(), row.raw.to_bits(), "raw dt, row {n}");
        assert_eq!(dt.index, row.index, "ring index, row {n}");
        for (k, entry) in dt.ring.iter().enumerate() {
            assert_eq!(entry.to_bits(), row.ring[k].to_bits(), "ring[{k}], row {n}");
        }
        assert_eq!(got.to_bits(), row.dt.to_bits(), "dt, row {n}");
        assert_eq!(inv_dt.to_bits(), row.inv_dt.to_bits(), "1/dt, row {n}");
    }
}
