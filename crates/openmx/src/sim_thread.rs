//! The fixed update on a thread of its own (N23a): a tick of the ride run beside the picture that
//! shows the tick before it, so a split-screen frame costs the slower of the two and not their sum.
//!
//! Why not the seats side by side: every seat steps the one world. Each seat's traffic and toys write
//! their cars' and toys' poses and moving words into the place's shared props, and the seat stepped
//! after reads what the one before it wrote this tick; the shared prop set is struck in seat order,
//! and the rivals, the bumps and the meetings read the seats already stepped. The original walks its
//! riders one after the other in the one world too. Stepping the seats at once would give a
//! different ride, so the seats still step one after the other, in their order, and the whole tick
//! moves off the loop's thread instead.
//!
//! What runs there is exactly the loop's own `engine.step` on the loop's own world, moved over and
//! back: nothing the tick reads or writes is copied. A tick's first half (the pads, the menu, the
//! picture's before-copies) runs on the loop as ever; its simulation runs while the next picture is
//! drawn; the rest of it (sound, cameras, the body's frame) runs on the loop after that picture,
//! before anything else. The thread-locals the simulation keeps (the flight search's kept worlds and
//! filed searches) are caches, which give the same answer built fresh, so which thread runs a tick
//! does not change it. While the tick runs, the loop draws from a copy of the seats as they were
//! before it (the picture the last tick ended on, held), over a place with the same ground and no
//! props, which is all a picture asks of it.
//!
//! The browser has no second thread ([`crate::thread`]): there is no worker, and every tick runs on
//! the loop as before.

use std::time::Duration;

/// `--sim-thread`: whether a tick may run on the worker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// When the ride can (a window, offline, two seats or more, no tiled world) and the ticks and
    /// the pictures do not both fit in a picture's time ([`Gauge::wants`]).
    #[default]
    Auto,
    /// Every tick the ride can run there, window or not.
    On,
    /// None: the loop's own thread, as before N23a.
    Off,
}

impl Mode {
    pub fn parse(word: &str) -> Option<Mode> {
        match word {
            "auto" => Some(Mode::Auto),
            "on" => Some(Mode::On),
            "off" => Some(Mode::Off),
            _ => None,
        }
    }
}

/// The one thread the ticks run on, kept for the whole ride so its kept worlds stay warm: a pool of
/// one, whose scoped jobs may borrow the loop's locals (rayon's `in_place_scope`, which waits for
/// them before it returns).
#[cfg(not(target_arch = "wasm32"))]
pub struct Worker(rayon_core::ThreadPool);

#[cfg(target_arch = "wasm32")]
pub struct Worker(std::convert::Infallible);

impl Worker {
    /// The thread, started; none in the browser.
    pub fn new() -> Option<Worker> {
        #[cfg(not(target_arch = "wasm32"))]
        return rayon_core::ThreadPoolBuilder::new().num_threads(1).thread_name(|_| "openmx sim".into()).build().ok().map(Worker);
        #[cfg(target_arch = "wasm32")]
        None
    }
}

/// `here()` on the loop while `run(job)` runs on the worker, both over when this returns; with no
/// worker or no job, `here()` alone. A panic in the job comes out here, as it would have on the loop.
pub fn beside<T: Send, R>(job: Option<(&Worker, &mut T)>, run: fn(&mut T), here: impl FnOnce() -> R) -> R {
    match job {
        #[cfg(not(target_arch = "wasm32"))]
        Some((Worker(pool), job)) => pool.in_place_scope(|s| {
            s.spawn(move |_| run(job));
            here()
        }),
        // There is none to be had (`Worker::new`).
        #[cfg(target_arch = "wasm32")]
        Some((_, job)) => {
            run(job);
            here()
        }
        None => here(),
    }
}

/// What a tick and a picture took lately, against the time the display gives a picture.
#[derive(Default)]
pub struct Gauge {
    /// The ticks' simulation times, newest last.
    ticks: Vec<Duration>,
    /// The last picture's drawing time, and the time the display gives a picture: the frame limit's,
    /// or one refresh.
    drawn: Duration,
    budget: Duration,
    /// Whether the last tick went to the worker, for `--timings`' word when it changes.
    pub engaged: bool,
}

/// How many ticks the slowest is taken over: a second at 30 Hz.
const KEPT: usize = 30;

impl Gauge {
    pub fn ticked(&mut self, took: Duration) {
        if self.ticks.len() == KEPT {
            self.ticks.remove(0);
        }
        self.ticks.push(took);
    }

    pub fn drew(&mut self, took: Duration, budget: Duration) {
        (self.drawn, self.budget) = (took, budget);
    }

    /// The slowest recent tick.
    pub fn slowest(&self) -> Duration {
        self.ticks.iter().copied().max().unwrap_or_default()
    }

    /// Whether a tick and a picture one after the other overrun a picture's time, which is when the
    /// overlap is worth the held picture it costs.
    pub fn wants(&self) -> bool {
        !self.budget.is_zero() && self.slowest() + self.drawn > self.budget
    }

    /// What it was decided on.
    pub fn line(&self) -> String {
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        format!("slowest tick {:.2} ms + picture {:.2} ms against {:.2} ms", ms(self.slowest()), ms(self.drawn), ms(self.budget))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_runs_on_the_worker_beside_the_loop() {
        let worker = Worker::new().unwrap();
        let here = std::thread::current().id();
        let mut job = (5, None);
        let mut at = Vec::new();
        let said = beside(Some((&worker, &mut job)), |(n, at)| {
            *n += 1;
            *at = Some(std::thread::current().id());
        }, || {
            at.push(std::thread::current().id());
            "drawn"
        });
        assert_eq!(said, "drawn");
        assert_eq!(at, [here]);
        assert_eq!(job.0, 6);
        assert!(job.1.is_some_and(|t| t != here));
    }

    #[test]
    fn every_job_runs_on_the_one_thread() {
        let worker = Worker::new().unwrap();
        let mut seen = Vec::new();
        for _ in 0..4 {
            let mut at = None;
            beside(Some((&worker, &mut at)), |at| *at = Some(std::thread::current().id()), || ());
            seen.push(at.unwrap());
        }
        assert!(seen.iter().all(|t| *t == seen[0]));
    }

    #[test]
    fn the_loop_waits_for_the_job() {
        let worker = Worker::new().unwrap();
        let mut n = 0;
        beside(Some((&worker, &mut n)), |n| {
            std::thread::sleep(Duration::from_millis(50));
            *n = 1;
        }, || ());
        assert_eq!(n, 1);
    }

    #[test]
    fn a_panic_on_the_worker_comes_back_on_the_loop() {
        let worker = Worker::new().unwrap();
        let mut n = 0;
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| beside(Some((&worker, &mut n)), |_| panic!("tick"), || ())));
        assert!(caught.is_err());
    }

    #[test]
    fn the_gauge_wants_the_overlap_only_when_both_overrun() {
        let mut gauge = Gauge::default();
        let ms = Duration::from_millis;
        gauge.drew(ms(8), ms(16));
        gauge.ticked(ms(5));
        assert!(!gauge.wants());
        gauge.ticked(ms(12));
        assert!(gauge.wants());
        (0..KEPT).for_each(|_| gauge.ticked(ms(2)));
        assert!(!gauge.wants());
    }
}
