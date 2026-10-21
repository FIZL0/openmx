//! The ride's own clock (P52): the simulation on a fixed schedule, the picture between its ticks.
//!
//! The shell used to step the game once per drawn frame and hand the engine the host's wall clock.
//! The engine's `dt` pipeline (`game_sim::tick::DeltaTime`) caps every frame at 1/30 s, so a picture
//! drawn at 5 fps ran the world at a sixth of real time while the sound ran on real time, and the
//! uneven frame times fed the thirty-frame average, so `dt` wandered. Now the two are apart:
//!
//! - **The ticks** run on the schedule a PS2 holding 30 fps keeps: one every two NTSC fields,
//!   1001/30000 s. A drawn frame runs as many as the wall clock owes ([`Pacer::owed`]), none at all
//!   when the picture is faster than the game, and several when it is slower.
//! - **The clock each tick reads** is not the host's: it is the Timer 0 a steady PS2 would have read
//!   at that frame ([`ps2_clock`]). Every tick is the same length whatever the host did, so a ride
//!   is the same ride at any drawn rate, and `dt` is the original's: its save states hold the last
//!   thirty frame deltas the engine measured, and they are 33 and 34 ms in exactly the order this
//!   clock gives them (the `the_ps2_clock_is_the_originals` test).
//! - **The picture** is drawn as fast as the machine allows and shows the world between the last two
//!   ticks ([`Blend`]), by how far the wall clock is past the last one. It is render-side only: the
//!   simulation never sees it.

use std::time::{Duration, Instant};

use game_render::mat::{M4, V3};
use game_render::world::Camera;
use game_sim::tick::{Engine, Timer0, clock_ms};

/// Timer 0's counts in a second: the 147,456,000 Hz bus clock over 256.
const TIMER0_HZ: u64 = 576_000;
/// Counts in one 100 ms period of Timer 0, when its interrupt takes it back to zero.
const TIMER0_PERIOD: u64 = 57_600;
/// One frame of a game holding 30 fps on an NTSC PS2 is two fields of 1001/60,000 s, so 2002/60,000 s:
/// 19,219.2 counts, which is this numerator over [`FRAME_COUNTS_DEN`]. Integer arithmetic, so the
/// same clock on every machine.
const FRAME_COUNTS_NUM: u64 = 2 * 1001 * TIMER0_HZ / 1000;
const FRAME_COUNTS_DEN: u64 = 60;

/// The wall time a tick of the ride stands for: the PS2's 30 fps frame, 1001/30,000 s.
pub const TICK: Duration = Duration::from_nanos(1_001_000_000_000 / 30_000);

/// The ride's first tick comes after the world load, which blocks the original's frame for about
/// nineteen seconds (`game_sim::tick`'s notes): the frame before it, the menu's, is this many frames
/// earlier. All the engine makes of it is a delta past its long-frame limit, as the original's does.
const LOAD_FRAMES: u64 = 19 * 30;

/// The Timer 0 pair a PS2 holding 30 fps reads at its `frame`th frame, counted from when its clock
/// read zero.
pub fn ps2_clock(frame: u64) -> Timer0 {
    let counts = frame * FRAME_COUNTS_NUM / FRAME_COUNTS_DEN;
    Timer0 { ticks: (counts / TIMER0_PERIOD) as u32, count: (counts % TIMER0_PERIOD) as u32 }
}

/// The clock the ride's `tick`th tick reads (the first is 0).
pub fn tick_clock(tick: u64) -> Timer0 {
    ps2_clock(1 + LOAD_FRAMES + tick)
}

/// Set the engine up as the original's is on the first frame of a ride: the frame before it was
/// the menu's, a whole world load ago. The first tick then measures a delta past
/// `DeltaTime::LONG_FRAME`, fills the average with 0.033 s, and the next second of ticks averages its
/// way up to the 30 Hz cap, which is what the original's ring does after every load.
pub fn after_load(engine: &mut Engine) {
    engine.prev_clock_ms = clock_ms(ps2_clock(1));
}

/// The most wall time the ride catches up in one go. Behind by more than this, the rest is
/// forgotten and the world goes on from where it was rather than running a burst of ticks to make
/// up a stall it would then have to make up again (a window dragged, the machine asleep). Half a
/// second keeps the ride in real time down to 2 fps drawn. The original's own rule for a long frame
/// is harsher (`DeltaTime::LONG_FRAME`: past 0.1 s the time is simply lost), but that is a rule about
/// the PS2's own frame, which here never comes late: only the picture does.
pub const CATCH_UP: Duration = Duration::from_millis(500);

/// The fixed schedule: when each tick is due, and how many have run.
pub struct Pacer {
    /// What one tick stands for in wall time.
    step: Duration,
    /// When tick 0 was due.
    start: Instant,
    /// The ticks run so far.
    done: u64,
    /// Wall time forgotten by stalls past [`CATCH_UP`], which every later tick is due that much later
    /// for.
    lost: Duration,
    /// No tick at or past this one, from `--frames`.
    most: Option<u64>,
    /// No tick due at or after this, from `--seconds`.
    until: Option<Duration>,
}

impl Pacer {
    pub fn new(step: Duration, start: Instant) -> Pacer {
        Pacer { step, start, done: 0, lost: Duration::ZERO, most: None, until: None }
    }

    /// Stop after `ticks` ticks.
    pub fn most(mut self, ticks: Option<u32>) -> Pacer {
        self.most = ticks.map(u64::from);
        self
    }

    /// Run no tick due `seconds` or more after the start: the ride is the same number of ticks
    /// whatever rate it was drawn at.
    pub fn until(mut self, seconds: Option<f32>) -> Pacer {
        self.until = seconds.map(Duration::from_secs_f32);
        self
    }

    /// The ticks run so far.
    pub fn ticks(&self) -> u64 {
        self.done
    }

    /// When tick `n` is due, after the start.
    fn due(&self, n: u64) -> Duration {
        self.step * n as u32 + self.lost
    }

    /// Whether a tick is owed at `now`. Behind by more than [`CATCH_UP`], the backlog is dropped and
    /// the next tick is due now.
    pub fn owed(&mut self, now: Instant) -> bool {
        if self.most.is_some_and(|most| self.done >= most) {
            return false;
        }
        let since = now.saturating_duration_since(self.start);
        let due = self.due(self.done);
        if self.until.is_some_and(|until| due >= until) || since < due {
            return false;
        }
        if since - due > CATCH_UP {
            self.lost += since - due;
        }
        true
    }

    /// A tick has run.
    pub fn ran(&mut self) {
        self.done += 1;
    }

    /// How far the picture at `now` is from the tick before the last one to the last one: 0 shows the
    /// one before, 1 the last. The picture runs a tick behind the game, which is the price of drawing
    /// between two states that both exist.
    pub fn blend(&self, now: Instant) -> f32 {
        let Some(last) = self.done.checked_sub(1) else { return 1.0 };
        let past = now.saturating_duration_since(self.start).saturating_sub(self.due(last));
        (past.as_secs_f32() / self.step.as_secs_f32()).clamp(0.0, 1.0)
    }
}

/// A move longer than this between two ticks is a jump — a respawn, a swap, a camera cut — and is
/// shown where it lands rather than slid across. Faster than anything in the game rides or flies
/// (a bike at full speed covers about four feet a tick, the plane about eight).
pub const SNAP: f32 = 30.0;

/// Render-side blending between the tick before the last and the last.
pub struct Blend(pub f32);

impl Blend {
    pub fn f(&self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.0
    }

    pub fn v3(&self, a: V3, b: V3) -> V3 {
        V3 { x: self.f(a.x, b.x), y: self.f(a.y, b.y), z: self.f(a.z, b.z) }
    }

    /// A placement: its position blended, and its three axes blended and put back to their lengths,
    /// so a turn does not shrink the model halfway through it. A jump is not blended.
    pub fn at(&self, a: M4, b: M4) -> M4 {
        let row = |m: &M4, i: usize| V3 { x: m.0[i][0], y: m.0[i][1], z: m.0[i][2] };
        if row(&a, 3).sub(row(&b, 3)).len() > SNAP {
            return b;
        }
        let mut out = b;
        for i in 0..4 {
            let v = self.v3(row(&a, i), row(&b, i));
            let v = match i {
                3 => v,
                _ => {
                    let (len, want) = (v.len(), self.f(row(&a, i).len(), row(&b, i).len()));
                    if len > 1e-6 { v.scale(want / len) } else { row(&b, i) }
                }
            };
            out.0[i][..3].copy_from_slice(&[v.x, v.y, v.z]);
            out.0[i][3] = self.f(a.0[i][3], b.0[i][3]);
        }
        out
    }

    /// A pose: every node's matrix blended as [`Blend::at`] does, when the two are poses of the same
    /// skeleton.
    pub fn pose(&self, a: &[M4], b: &[M4]) -> Vec<M4> {
        if a.len() != b.len() {
            return b.to_vec();
        }
        a.iter().zip(b).map(|(a, b)| self.at(*a, *b)).collect()
    }

    /// A camera: where it is and what it looks at. A cut is not blended.
    pub fn camera(&self, a: &Camera, b: &Camera) -> Camera {
        if a.eye.sub(b.eye).len() > SNAP || a.at.sub(b.at).len() > SNAP {
            return *b;
        }
        Camera { eye: self.v3(a.eye, b.eye), at: self.v3(a.at, b.at), fov: self.f(a.fov, b.fov), ..*b }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_sim::Input;
    use game_sim::tick::{DeltaTime, Game};
    use ps2_float::F;

    /// The thirty frame deltas, in milliseconds and oldest first, the original's engine was averaging
    /// when each of five of the project's save states was taken mid-ride: the dt ring read out of each
    /// state's RAM image, turned to start at its write index. `x` is 34 ms, `.` is 33.
    const RINGS: [&str; 5] = [
        "..x..x.x..x..x..x.x..x..x..x.x",
        ".x..x.x..x..x..x.x..x..x..x.x.",
        "x..x.x..x..x..x.x..x..x..x.x..",
        "x.x..x..x.x..x..x..x.x..x..x..",
        "..x..x..x.x..x..x..x.x..x..x.x",
    ];

    /// The deltas, in the rings' letters, of a clock `frame` counts of Timer 0 long.
    fn deltas(clock: impl Fn(u64) -> Timer0) -> String {
        (0..300u64)
            .map(|f| match clock_ms(clock(f + 1)) - clock_ms(clock(f)) {
                33 => '.',
                34 => 'x',
                _ => '?',
            })
            .collect()
    }

    /// Every ring the original measured is thirty frames of this clock: the 34s fall exactly where
    /// a 19,219.2-count frame read in whole milliseconds puts them, 1001 ms every thirty frames.
    #[test]
    fn the_ps2_clock_is_the_originals() {
        let ours = deltas(ps2_clock);
        for ring in RINGS {
            assert!(ours.contains(ring), "{ring} is not a stretch of the PS2 clock {ours}");
        }
        // A clock of exactly 1/30 s a frame, 19,200 counts, makes 1000 ms of thirty frames and none
        // of the rings.
        let exact = deltas(|f| Timer0 { ticks: (f * 19_200 / TIMER0_PERIOD) as u32, count: (f * 19_200 % TIMER0_PERIOD) as u32 });
        assert!(RINGS.iter().all(|ring| !exact.contains(ring)), "{exact}");
    }

    struct Idle;
    impl Game for Idle {
        fn simulate(&mut self, _: F, _: Input, _: &mut game_sim::rng::Rng) -> bool {
            true
        }
    }

    /// The first tick follows a load: the long-frame rule fills the ring, and the ticks average their
    /// way up to the cap and stay on it.
    #[test]
    fn a_ride_starts_after_a_load_and_settles_on_the_cap() {
        let mut engine = Engine::new();
        after_load(&mut engine);
        let dts: Vec<F> = (0..300).map(|t| engine.step(tick_clock(t), Input::default(), &mut Idle).dt).collect();
        assert!((dts[0].0 - DeltaTime::AFTER_LONG_FRAME).abs() < 1e-6, "the first tick's dt is {:?}", dts[0]);
        let settled = dts.iter().position(|&dt| dt == F(DeltaTime::CAP_30)).expect("it reaches the cap");
        assert!(settled < 40, "it took {settled} ticks to reach the cap");
        assert!(dts[settled..].iter().all(|&dt| dt == F(DeltaTime::CAP_30)), "it left the cap");
        assert!(dts[..settled].windows(2).all(|w| w[0] <= w[1]), "the ramp is not a ramp");
    }

    /// N6: the frame length online is the one the offline ride's engine hands each tick, bit for
    /// bit, over a hundred thousand ticks (an hour of riding): an online ride on the same pads is the
    /// offline ride.
    #[test]
    fn the_online_ticks_are_the_offline_ones() {
        let mut engine = Engine::new();
        after_load(&mut engine);
        for t in 0..100_000u64 {
            let dt = engine.step(tick_clock(t), Input::default(), &mut Idle).dt;
            let online = crate::net::rides::tick_dt(t as usize);
            assert_eq!(online.0.to_bits(), dt.0.to_bits(), "tick {t}: online {online:?}, offline {dt:?}");
        }
    }

    /// The ticks owed are the wall clock's, however it is sampled; a stall past the catch-up limit is
    /// forgotten.
    #[test]
    fn the_schedule_owes_the_wall_clocks_ticks() {
        let start = Instant::now();
        let run = |every: Duration, until: Duration| {
            let mut p = Pacer::new(TICK, start);
            let mut now = start;
            while now <= start + until {
                while p.owed(now) {
                    p.ran();
                }
                now += every;
            }
            p.ticks()
        };
        let (fast, slow) = (run(Duration::from_millis(7), Duration::from_secs(3)), run(Duration::from_millis(200), Duration::from_secs(3)));
        assert_eq!(fast, slow);
        assert_eq!(fast, 3_000_000_000 / TICK.as_nanos() as u64 + 1);

        let mut p = Pacer::new(TICK, start).until(Some(1.0));
        for ms in (0..3000).step_by(100) {
            while p.owed(start + Duration::from_millis(ms)) {
                p.ran();
            }
        }
        assert_eq!(p.ticks(), 30, "the ticks due inside the first second");

        let mut p = Pacer::new(TICK, start);
        let mut n = 0;
        while p.owed(start + Duration::from_secs(10)) {
            p.ran();
            n += 1;
        }
        assert_eq!(n, 1, "a ten-second stall runs one tick, not three hundred");
        assert!(!p.owed(start + Duration::from_secs(10)));
        assert!(p.owed(start + Duration::from_secs(10) + TICK));
    }

    /// Halfway round a quarter turn the model is still its own size, and a jump is shown where it
    /// lands.
    #[test]
    fn a_blend_turns_without_shrinking_and_does_not_slide_a_jump() {
        let v = |x, y, z| V3 { x, y, z };
        let a = M4::placement(v(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0), v(0.0, 1.0, 0.0));
        let b = M4::placement(v(2.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0));
        let half = Blend(0.5).at(a, b);
        assert_eq!(half.0[3][..3], [1.0, 0.0, 0.0]);
        for i in 0..3 {
            let len = v(half.0[i][0], half.0[i][1], half.0[i][2]).len();
            assert!((len - 1.0).abs() < 1e-5, "axis {i} is {len} long");
        }
        let far = M4::placement(v(SNAP + 1.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0));
        assert_eq!(Blend(0.5).at(a, far), far);
        assert_eq!(Blend(0.0).at(a, b).0[3], a.0[3]);
    }
}
