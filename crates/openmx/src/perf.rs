//! `--fps`: how fast the shell is drawing, and where a frame's time goes.
//!
//! Once a second a line with the frame rate, the slowest frame, how many frames missed a 60 Hz
//! refresh, and each stage's average and worst time — and, as it happens, a line for any frame over
//! two refreshes, which is the stutter a rider feels. The stages are the loop's own: the simulation's
//! step, the animation that poses each body and bike, the skinning that turns a pose into what is
//! drawn, the scene (cameras, the list of what stands where, the views), the draw itself and the
//! present. Opentennis' `HST_FPS` printed its slow frames the same way (its z1-perf entry).
//!
//! Nothing here reads or writes the simulation; it only reads the wall clock.

use std::time::{Duration, Instant};

/// The stages a frame is cut into, in the order the loop runs them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Sim,
    Animation,
    Skinning,
    Scene,
    Draw,
    Present,
}

impl Stage {
    pub const ALL: [Stage; 6] = [
        Stage::Sim,
        Stage::Animation,
        Stage::Skinning,
        Stage::Scene,
        Stage::Draw,
        Stage::Present,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Stage::Sim => "sim",
            Stage::Animation => "anim",
            Stage::Skinning => "skin",
            Stage::Scene => "scene",
            Stage::Draw => "draw",
            Stage::Present => "present",
        }
    }
}

/// One refresh at 60 Hz: a frame longer than this missed one.
const REFRESH: Duration = Duration::from_micros(16_667);
/// A frame longer than this is reported on its own line.
const SLOW: Duration = Duration::from_micros(33_333);

/// The stage times of the frame being drawn and of the second being summed.
pub struct Perf {
    pub on: bool,
    started: Instant,
    window: Instant,
    frame_start: Instant,
    /// This frame's time in each stage so far.
    now: [Duration; 6],
    /// This second's: the sum and the worst of each stage, and of whole frames.
    sum: [Duration; 6],
    worst: [Duration; 6],
    frames: u32,
    frame_sum: Duration,
    frame_worst: Duration,
    missed: u32,
    slow: u32,
    /// The whole session's, for the line it ends on.
    total_frames: u64,
    total_missed: u64,
    total_slow: u64,
    total_sum: [Duration; 6],
}

impl Perf {
    pub fn new(on: bool) -> Perf {
        let now = Instant::now();
        Perf {
            on,
            started: now,
            window: now,
            frame_start: now,
            now: [Duration::ZERO; 6],
            sum: [Duration::ZERO; 6],
            worst: [Duration::ZERO; 6],
            frames: 0,
            frame_sum: Duration::ZERO,
            frame_worst: Duration::ZERO,
            missed: 0,
            slow: 0,
            total_frames: 0,
            total_missed: 0,
            total_slow: 0,
            total_sum: [Duration::ZERO; 6],
        }
    }

    /// The top of a frame.
    pub fn begin(&mut self) {
        self.frame_start = Instant::now();
        self.now = [Duration::ZERO; 6];
    }

    /// Time spent in `stage`, added to this frame's.
    pub fn add(&mut self, stage: Stage, took: Duration) {
        self.now[stage as usize] += took;
    }

    /// This frame's time in every stage so far, for [`Perf::lap`].
    pub fn spent(&self) -> Duration {
        self.now.iter().sum()
    }

    /// Count the time since `since` against `stage`, less whatever other stages were given in the
    /// meantime (`spent` is [`Perf::spent`] at `since`): a stretch with a stage timed inside it.
    pub fn lap(&mut self, stage: Stage, since: Instant, spent: Duration) {
        let inside = self.spent().saturating_sub(spent);
        self.add(stage, since.elapsed().saturating_sub(inside));
    }

    /// Run `f`, and count its time against `stage`.
    pub fn time<T>(&mut self, stage: Stage, f: impl FnOnce() -> T) -> T {
        let t = Instant::now();
        let out = f();
        self.add(stage, t.elapsed());
        out
    }

    /// The bottom of a frame: fold it into the second, and say so if it was slow or the second is up.
    /// `views` is how many views the frame drew, for the line.
    pub fn end(&mut self, views: usize) {
        if !self.on {
            return;
        }
        let took = self.frame_start.elapsed();
        for i in 0..6 {
            self.sum[i] += self.now[i];
            self.total_sum[i] += self.now[i];
            self.worst[i] = self.worst[i].max(self.now[i]);
        }
        self.frames += 1;
        self.total_frames += 1;
        self.frame_sum += took;
        self.frame_worst = self.frame_worst.max(took);
        if took > REFRESH {
            self.missed += 1;
            self.total_missed += 1;
        }
        if took > SLOW {
            self.slow += 1;
            self.total_slow += 1;
            println!(
                "fps: slow frame {:.1} ms at {:.1} s ({})",
                ms(took),
                self.started.elapsed().as_secs_f32(),
                self.stages(&self.now, None)
            );
        }
        let second = self.window.elapsed();
        if second >= Duration::from_secs(1) {
            let n = self.frames.max(1);
            let avg: [Duration; 6] = std::array::from_fn(|i| self.sum[i] / n);
            println!(
                "fps: {:.1} fps, {} view{}, frame {:.2} ms avg {:.2} worst, {} over 16.7 ms, {} over 33 ms | {}",
                self.frames as f32 / second.as_secs_f32(),
                views,
                if views == 1 { "" } else { "s" },
                ms(self.frame_sum / n),
                ms(self.frame_worst),
                self.missed,
                self.slow,
                self.stages(&avg, Some(&self.worst)),
            );
            self.window = Instant::now();
            self.sum = [Duration::ZERO; 6];
            self.worst = [Duration::ZERO; 6];
            self.frames = 0;
            self.frame_sum = Duration::ZERO;
            self.frame_worst = Duration::ZERO;
            self.missed = 0;
            self.slow = 0;
        }
    }

    /// The session's own line, for the end of a run.
    pub fn summary(&self) {
        if !self.on || self.total_frames == 0 {
            return;
        }
        let n = self.total_frames as u32;
        let avg: [Duration; 6] = std::array::from_fn(|i| self.total_sum[i] / n);
        println!(
            "fps: session {} frames in {:.1} s, {:.1} fps, {} over 16.7 ms, {} over 33 ms | {}",
            self.total_frames,
            self.started.elapsed().as_secs_f32(),
            self.total_frames as f32 / self.started.elapsed().as_secs_f32(),
            self.total_missed,
            self.total_slow,
            self.stages(&avg, None)
        );
    }

    fn stages(&self, avg: &[Duration; 6], worst: Option<&[Duration; 6]>) -> String {
        Stage::ALL
            .iter()
            .map(|&s| match worst {
                Some(w) => format!("{} {:.2}/{:.2}", s.name(), ms(avg[s as usize]), ms(w[s as usize])),
                None => format!("{} {:.2}", s.name(), ms(avg[s as usize])),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn ms(d: Duration) -> f32 {
    d.as_secs_f32() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_sums_its_stages() {
        let mut p = Perf::new(true);
        p.begin();
        p.add(Stage::Draw, Duration::from_millis(3));
        p.add(Stage::Draw, Duration::from_millis(2));
        p.add(Stage::Sim, Duration::from_millis(1));
        assert_eq!(p.now[Stage::Draw as usize], Duration::from_millis(5));
        assert_eq!(p.now[Stage::Sim as usize], Duration::from_millis(1));
        p.end(1);
        assert_eq!(p.total_frames, 1);
        assert_eq!(p.total_sum[Stage::Draw as usize], Duration::from_millis(5));
        // The next frame starts from nothing.
        p.begin();
        assert_eq!(p.now[Stage::Draw as usize], Duration::ZERO);
    }
}
