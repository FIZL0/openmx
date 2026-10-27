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

    /// Wall time `d` the ride did not run in (P150: the warm frame behind the loading screen): the
    /// ride starts that much later, so no tick is owed for it and `--seconds` still rides as many.
    pub fn hold(&mut self, d: Duration) {
        self.start += d;
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

/// The least time one picture takes (P90), from the VSync and frame limit settings: `limit` is the
/// most pictures a second, if any, and `refresh` the display's. `waits` is whether the present itself
/// waits for the display when VSync is on — wgpu's Fifo does; the X11 window's `PutImage` does not,
/// so there VSync is a pace of the refresh's own. None of it reaches the ticks: [`Pacer`] owes them
/// by the wall clock whatever the picture's rate.
pub fn frame_floor(vsync: bool, limit: Option<f32>, refresh: f32, waits: bool) -> Option<Duration> {
    let limit = limit.filter(|fps| *fps > 0.0);
    let fps = match (vsync, limit) {
        // The present holds it to the refresh; only a limit below that is the loop's to keep.
        (true, limit) if waits => limit.filter(|fps| *fps < refresh),
        (true, limit) => Some(limit.map_or(refresh, |fps| fps.min(refresh))),
        (false, limit) => limit,
    };
    fps.map(|fps| Duration::from_secs_f64(1.0 / f64::from(fps)))
}

/// Holds the pictures to a floor (P90): each frame ends no sooner than the floor after the last
/// one's end was due, so a frame limit of 60 is sixty pictures a second on average, the sleep's own
/// lateness not piling up. Behind, it starts again from now rather than hurrying to catch up.
#[derive(Default)]
pub struct Limiter {
    last: Option<Instant>,
}

impl Limiter {
    /// Sleep until this frame is due, `floor` after the last; none at all with no floor.
    pub fn wait(&mut self, floor: Option<Duration>) {
        let now = Instant::now();
        let Some(floor) = floor else {
            self.last = Some(now);
            return;
        };
        match self.last.map(|last| last + floor) {
            Some(due) if due > now => {
                std::thread::sleep(due - now);
                self.last = Some(due);
            }
            _ => self.last = Some(now),
        }
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
        if place(&a).sub(place(&b)).len() > SNAP {
            return b;
        }
        self.slide(a, b)
    }

    /// [`Blend::at`] with no jump test: for what a seat's [`Cut`] has already said is no jump.
    pub fn slide(&self, a: M4, b: M4) -> M4 {
        let row = |m: &M4, i: usize| V3 { x: m.0[i][0], y: m.0[i][1], z: m.0[i][2] };
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
        self.pan(a, b)
    }

    /// [`Blend::camera`] with no cut test: for a seat's [`Cut`] to have decided.
    pub fn pan(&self, a: &Camera, b: &Camera) -> Camera {
        Camera { eye: self.v3(a.eye, b.eye), at: self.v3(a.at, b.at), fov: self.f(a.fov, b.fov), ..*b }
    }

    /// A pose of the same skeleton in the space of a placement that moves under it, `a` in `a_at`'s
    /// and `b` in `b_at`'s, blended where the nodes are in the world and handed back in the space of
    /// `at`, the placement drawn (P120). It is how a thrown rider is blended: the ragdoll's bones are
    /// bodies of their own in the world, and the wrecked bike they are kept relative to tumbles. Blended
    /// in the bike's space instead, a rider lying still beside a spinning bike is swung round it
    /// between ticks and springs back on the next — seventy feet a frame, for a rider thrown far off a
    /// fast crash. The pose decides its jump as one: every node slid, or every node where it lands.
    pub fn thrown(&self, a: &[M4], a_at: M4, b: &[M4], b_at: M4, at: M4) -> Vec<M4> {
        if a.len() != b.len() {
            return b.to_vec();
        }
        let back = inverse(at);
        let world: Vec<(M4, M4)> = a.iter().zip(b).map(|(a, b)| (a.mul(a_at), b.mul(b_at))).collect();
        if world.iter().any(|(a, b)| place(a).sub(place(b)).len() > SNAP) {
            return world.iter().map(|(_, b)| b.mul(back)).collect();
        }
        world.iter().map(|(a, b)| self.slide(*a, *b).mul(back)).collect()
    }
}

/// Where a placement is: its translation row.
fn place(m: &M4) -> V3 {
    V3 { x: m.0[3][0], y: m.0[3][1], z: m.0[3][2] }
}

/// The inverse of a placement (an affine matrix, row vectors: `v * M`), scaled or not.
fn inverse(m: M4) -> M4 {
    let r = m.0;
    let det = r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1]) - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0])
        + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0]);
    if det.abs() < 1e-12 {
        return M4::IDENTITY;
    }
    let k = 1.0 / det;
    let mut out = [[0.0f32; 4]; 4];
    out[0][0] = (r[1][1] * r[2][2] - r[1][2] * r[2][1]) * k;
    out[0][1] = (r[0][2] * r[2][1] - r[0][1] * r[2][2]) * k;
    out[0][2] = (r[0][1] * r[1][2] - r[0][2] * r[1][1]) * k;
    out[1][0] = (r[1][2] * r[2][0] - r[1][0] * r[2][2]) * k;
    out[1][1] = (r[0][0] * r[2][2] - r[0][2] * r[2][0]) * k;
    out[1][2] = (r[0][2] * r[1][0] - r[0][0] * r[1][2]) * k;
    out[2][0] = (r[1][0] * r[2][1] - r[1][1] * r[2][0]) * k;
    out[2][1] = (r[0][1] * r[2][0] - r[0][0] * r[2][1]) * k;
    out[2][2] = (r[0][0] * r[1][1] - r[0][1] * r[1][0]) * k;
    for c in 0..3 {
        out[3][c] = -(r[3][0] * out[0][c] + r[3][1] * out[1][c] + r[3][2] * out[2][c]);
    }
    out[3][3] = 1.0;
    M4(out)
}

/// Whether a move of `moved` feet between the last two ticks is a jump, when the tick before moved
/// `was` (P120): more than [`SNAP`] past what it was already doing, so that riding or flying
/// however fast is never one, and a put-back or a swap from a standstill is.
pub fn jumped(was: f32, moved: f32) -> bool {
    moved > SNAP + 2.0 * was
}

/// How far a machine jumped over a tick, from `was` to `now`, when it was moving `flying` feet a tick
/// (P125): the move, if it is a jump by [`jumped`] — a marker's put-back, say — or `None`.
pub fn jump(was: V3, now: V3, flying: f32) -> Option<V3> {
    let by = now.sub(was);
    jumped(flying, by.len()).then_some(by)
}

/// One seat's cut (P120): whether its picture is shown where the last tick left it rather than slid
/// there. Everything the seat's view shows — its machine, its rider, its camera — cuts or slides
/// together: tested one by one, a bike that crossed the jump distance a tick before the camera
/// following it would be drawn where it landed under a camera still sliding, and swing a tick's
/// travel across the screen and back. A move is measured against the one before it ([`jumped`]).
#[derive(Clone, Default)]
pub struct Cut {
    /// The spots at the tick before the last: the machine, the camera's eye and what it looks at.
    at: Option<[V3; 3]>,
    /// How far each moved over the tick before that.
    was: [f32; 3],
}

impl Cut {
    /// The spots the picture blends from, as the tick before the last left them.
    pub fn before(&mut self, spots: [V3; 3]) {
        if let Some(at) = self.at {
            self.was = moved(at, spots);
        }
        self.at = Some(spots);
    }

    /// The spots before moved `by`, with the machine they follow, over a wrapped world's seam
    /// (MD29): the whole width is no move of the picture's.
    pub fn carry(&mut self, by: V3) {
        if let Some(at) = &mut self.at {
            for s in at {
                *s = s.add(by);
            }
        }
    }

    /// Whether the last tick's spots, `now`, jumped from the ones before: a cut. `marked` is a jump
    /// the simulation says it made (a put-back), whatever the distance.
    pub fn cuts(&self, now: [V3; 3], marked: bool) -> bool {
        let Some(at) = self.at else { return true };
        marked || moved(at, now).iter().zip(self.was).any(|(m, was)| jumped(was, *m))
    }
}

fn moved(a: [V3; 3], b: [V3; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i].sub(b[i]).len())
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

    /// P90: the VSync and frame limit settings, as a floor on the picture's time.
    #[test]
    fn the_settings_make_the_pictures_floor() {
        let ms = |d: Option<Duration>| d.map(|d| (d.as_secs_f64() * 1000.0 * 10.0).round() / 10.0);
        // VSync on, a present that waits: nothing for the loop to keep but a limit below the refresh.
        assert_eq!(frame_floor(true, None, 144.0, true), None);
        assert_eq!(frame_floor(true, Some(240.0), 144.0, true), None);
        assert_eq!(frame_floor(true, Some(144.0), 144.0, true), None);
        assert_eq!(ms(frame_floor(true, Some(60.0), 144.0, true)), Some(16.7));
        // One that does not: the refresh's own pace, or the limit's below it.
        assert_eq!(ms(frame_floor(true, None, 60.0, false)), Some(16.7));
        assert_eq!(ms(frame_floor(true, Some(240.0), 60.0, false)), Some(16.7));
        assert_eq!(ms(frame_floor(true, Some(30.0), 60.0, false)), Some(33.3));
        // VSync off: the limit alone, or as fast as it draws.
        assert_eq!(frame_floor(false, None, 60.0, true), None);
        assert_eq!(ms(frame_floor(false, Some(120.0), 60.0, true)), Some(8.3));
        assert_eq!(ms(frame_floor(false, Some(120.0), 60.0, false)), Some(8.3));
    }

    /// P90: a 60 fps limit sleeps each frame to about 16.7 ms, and the ride under it is the same
    /// ticks as one drawn uncapped or at any other limit: the same count, each reading the same clock.
    #[test]
    fn a_60_fps_limit_sleeps_the_frame_and_leaves_the_ticks_alone() {
        let floor = frame_floor(false, Some(60.0), 144.0, true);
        let mut limiter = Limiter::default();
        limiter.wait(floor);
        let start = Instant::now();
        let mut frames = Vec::new();
        for _ in 0..30 {
            let top = Instant::now();
            // A frame's work, well under the floor.
            std::thread::sleep(Duration::from_millis(2));
            limiter.wait(floor);
            frames.push(top.elapsed());
        }
        let mean = start.elapsed().as_secs_f64() * 1000.0 / 30.0;
        assert!((16.6..17.5).contains(&mean), "a frame took {mean:.2} ms on average");
        assert!(frames.iter().all(|f| *f >= Duration::from_micros(16_000)), "a frame ended early: {frames:?}");

        // The ticks over three seconds of wall clock, drawn at each limit (and uncapped, at 1 ms a
        // picture): the same ticks, and the engine's dt for each the same bits.
        let start = Instant::now();
        let ride = |floor: Duration| {
            let mut pacer = Pacer::new(TICK, start);
            let mut engine = Engine::new();
            after_load(&mut engine);
            let mut dts = Vec::new();
            let mut now = start;
            while now <= start + Duration::from_secs(3) {
                while pacer.owed(now) {
                    dts.push(engine.step(tick_clock(pacer.ticks()), Input::default(), &mut Idle).dt.0.to_bits());
                    pacer.ran();
                }
                now += floor;
            }
            dts
        };
        let uncapped = ride(Duration::from_millis(1));
        assert_eq!(uncapped.len() as u64, 3_000_000_000 / TICK.as_nanos() as u64 + 1);
        for fps in [30.0, 60.0, 120.0, 144.0, 240.0] {
            let floor = frame_floor(false, Some(fps), 60.0, true).unwrap();
            assert_eq!(ride(floor), uncapped, "the ride drawn at {fps} fps");
        }
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

    /// P120: a rider lying still forty feet from a bike that turns a quarter round in a tick is drawn
    /// where they lie at every blend. In the bike's space they were pulled toward it, and a fast enough
    /// turn snapped some nodes and slid the rest.
    #[test]
    fn a_thrown_rider_stays_put_beside_a_tumbling_bike() {
        let v = |x, y, z| V3 { x, y, z };
        let bike_a = M4::placement(v(0.0, 10.0, 0.0), v(0.0, 0.0, 1.0), v(0.0, 1.0, 0.0));
        let bike_b = M4::placement(v(0.0, 10.0, 0.0), v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0));
        let lies = M4::placement(v(40.0, 0.0, 0.0), v(0.0, 0.0, 1.0), v(0.0, 1.0, 0.0));
        let in_space = |bike: M4| lies.mul(inverse(bike));
        let (a, b) = (vec![in_space(bike_a)], vec![in_space(bike_b)]);
        for k in 0..=10 {
            let by = Blend(k as f32 / 10.0);
            let bike = by.slide(bike_a, bike_b);
            let drawn = by.thrown(&a, bike_a, &b, bike_b, bike)[0].mul(bike);
            let off = place(&drawn).sub(place(&lies)).len();
            assert!(off < 1e-3, "at {}: drawn {off} ft from where the rider lies", by.0);
            let swung = by.pose(&a, &b)[0].mul(bike);
            if k == 5 {
                assert!(place(&swung).sub(place(&lies)).len() > 10.0, "the bike's-space blend no longer swings");
            }
        }
        // The affine inverse undoes a scaled placement too.
        let mut scaled = bike_b;
        for i in 0..3 {
            for c in 0..3 {
                scaled.0[i][c] *= 2.5;
            }
        }
        let round = scaled.mul(inverse(scaled));
        for i in 0..4 {
            for c in 0..4 {
                assert!((round.0[i][c] - M4::IDENTITY.0[i][c]).abs() < 1e-5, "{round:?}");
            }
        }
    }

    /// P120: riding however fast is not a cut, a put-back from a standstill is, and so is anything the
    /// simulation marks. The seat's three spots cut together.
    #[test]
    fn a_cut_is_a_jump_past_the_seats_own_speed() {
        let v = |z: f32| V3 { x: 0.0, y: 0.0, z };
        let spots = |z: f32| [v(z), v(z - 30.0), v(z + 5.0)];
        let mut cut = Cut::default();
        assert!(cut.cuts(spots(0.0), false), "nothing to blend from");
        // Fifty feet a tick, steadily: past [`SNAP`], and no jump once the speed is known.
        cut.before(spots(0.0));
        for t in 1..10 {
            cut.before(spots(50.0 * t as f32));
            assert!(!cut.cuts(spots(50.0 * (t + 1) as f32), false), "tick {t}");
        }
        assert!(cut.cuts(spots(550.0), true), "a put-back is a cut whatever the distance");
        // Standing still, then a put-back forty feet off.
        let mut cut = Cut::default();
        cut.before(spots(0.0));
        cut.before(spots(0.0));
        assert!(!cut.cuts(spots(0.0), false));
        assert!(cut.cuts(spots(40.0), false));
        // The camera alone jumping cuts the seat.
        let moved = [v(0.0), v(-30.0 + 40.0), v(5.0)];
        assert!(cut.cuts(moved, false));
    }

    /// P120: taking the ticks owed and the blend at one moment a frame, a ride at a steady speed is
    /// drawn exactly a tick behind the wall clock at every frame, at 45, 60 and 144 pictures a second,
    /// whatever the frames' own lengths: the blend never reaches 1 and the drawn spot never steps back.
    #[test]
    fn a_frames_moment_draws_a_steady_ride_a_tick_behind() {
        let start = Instant::now();
        for fps in [45.0f64, 60.0, 144.0] {
            let mut pacer = Pacer::new(TICK, start);
            // The spot after `n` ticks, at four feet a tick.
            let spot = |n: u64| 4.0 * n as f64;
            let (mut last, mut frame) = (f64::MIN, 0u32);
            while frame < (fps * 5.0) as u32 {
                // Uneven frames: every third one a third longer, as a frame that ran a tick is.
                let moment = start + Duration::from_secs_f64((frame as f64 + (frame % 3 == 0) as u8 as f64 / 3.0) / fps);
                frame += 1;
                while pacer.owed(moment) {
                    pacer.ran();
                }
                let Some(now) = pacer.ticks().checked_sub(1) else { continue };
                let f = pacer.blend(moment);
                assert!((0.0..1.0).contains(&f), "{fps} fps: blend {f}");
                let drawn = spot(now.saturating_sub(1)) + (spot(now) - spot(now.saturating_sub(1))) * f as f64;
                assert!(drawn >= last, "{fps} fps: drawn at {drawn}, back from {last}");
                last = drawn;
                if now >= 1 {
                    let wall = moment.duration_since(start).as_secs_f64() / TICK.as_secs_f64();
                    assert!((drawn - 4.0 * (wall - 1.0)).abs() < 1e-3, "{fps} fps: drawn at {drawn}, the clock says {}", 4.0 * (wall - 1.0));
                }
            }
        }
    }
}
