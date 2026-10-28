//! A ride built off the window's thread (P127).
//!
//! Building a ride — the scene, the simulation's tiles, the props, the models and every ground
//! chunk — takes long enough on a big track that a window which stops pumping its events meanwhile
//! is marked Not Responding by the desktop: a five-tile world was 39 s in a release build. So the
//! front end hands the build to a thread of its own ([`on_worker`]) and draws the loading screen while
//! it runs, reading the bar off the [`Progress`] the build ticks at each stage and at every ground
//! chunk. A cancel is a flag the build answers at its next tick, and the thread is joined before the
//! front end goes on, so nothing is left running.
//!
//! The build is the same code on either thread and reads nothing the thread could change, so the
//! ride it makes is the same ride: tests/loading.rs rides one both ways and compares the hashes.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use crate::clock::Instant;

/// A ride's load, in the order it goes: [`Progress::reach`] says which is under way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// The disc's archives, the scene file and its look.
    Scene,
    /// The terrain and the simulation's ground: a world's tiles and joins.
    Tiles,
    /// The props and the sky.
    Props,
    /// The bikes, the riders, the biplane and the cars.
    Models,
    /// The drawn ground, chunk by chunk.
    Ground,
    /// Everything sent to the GPU, a slice between frames, on the window's thread.
    Upload,
}

impl Stage {
    pub const ALL: [Stage; 6] = [Stage::Scene, Stage::Tiles, Stage::Props, Stage::Models, Stage::Ground, Stage::Upload];

    /// Its word in [`HOLD`].
    pub fn word(self) -> &'static str {
        match self {
            Stage::Scene => "scene",
            Stage::Tiles => "tiles",
            Stage::Props => "props",
            Stage::Models => "models",
            Stage::Ground => "ground",
            Stage::Upload => "upload",
        }
    }

    /// What the loading screen says under the bar.
    pub fn label(self) -> &'static str {
        match self {
            Stage::Scene => "Scene",
            Stage::Tiles => "Tiles",
            Stage::Props => "Props",
            Stage::Models => "Models",
            Stage::Ground => "Ground",
            Stage::Upload => "Upload",
        }
    }

    fn index(self) -> usize {
        Stage::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

/// The test hook: `<stage>` holds the load at that stage until it is cancelled, `<stage>:<seconds>`
/// for that long, so a test can watch a slow load without a slow track.
pub const HOLD: &str = "OPENMX_LOAD_HOLD";
/// Set, a ride with no front end builds on a worker too, so its hash can be compared with the plain
/// build's.
pub const THREADED: &str = "OPENMX_THREADED_LOAD";
/// The other test hook: `<frames>` presses Back on the loading screen once it has drawn that many,
/// as Circle or Escape would.
pub const CANCEL: &str = "OPENMX_LOAD_CANCEL";
/// What a cancelled build fails with.
pub const CANCELLED: &str = "the load was cancelled";

/// A stage to hold at and for how long, `None` until cancelled: [`HOLD`]'s value read.
pub type Hold = Option<(Stage, Option<Duration>)>;

/// [`HOLD`]'s value: `ground`, `models:1.5`.
pub fn parse_hold(s: &str) -> Result<Hold, String> {
    let (word, seconds) = match s.split_once(':') {
        Some((w, t)) => (w, Some(t.parse::<f32>().map_err(|_| format!("{HOLD}: {t}? seconds"))?)),
        None => (s, None),
    };
    let stage = Stage::ALL.into_iter().find(|st| st.word() == word).ok_or(format!("{HOLD}: {word}? a stage, like ground"))?;
    Ok(Some((stage, seconds.map(|t| Duration::from_secs_f32(t.max(0.0))))))
}

/// How far a load has got, written by the build and read by the window's thread.
pub struct Progress {
    stage: AtomicU32,
    /// Of the stage's own count, when it has one: ground chunk `n` of `of`.
    n: AtomicU32,
    of: AtomicU32,
    cancel: AtomicBool,
    hold: Hold,
    held: AtomicBool,
}

impl Progress {
    /// A load held as [`HOLD`] says, or not at all.
    pub fn new() -> Progress {
        let hold = match std::env::var(HOLD) {
            Ok(s) if !s.is_empty() => parse_hold(&s).unwrap_or_else(|e| {
                println!("loading: {e}");
                None
            }),
            _ => None,
        };
        Progress::with_hold(hold)
    }

    pub fn with_hold(hold: Hold) -> Progress {
        Progress {
            stage: AtomicU32::new(0),
            n: AtomicU32::new(0),
            of: AtomicU32::new(0),
            cancel: AtomicBool::new(false),
            hold,
            held: AtomicBool::new(false),
        }
    }

    /// The build is at `stage`; held there if the hook says so. A cancelled load's error.
    pub fn reach(&self, stage: Stage) -> Result<(), String> {
        self.stage.store(stage.index() as u32, Ordering::Relaxed);
        self.n.store(0, Ordering::Relaxed);
        self.of.store(0, Ordering::Relaxed);
        self.check()?;
        if let Some((at, long)) = self.hold
            && at == stage
        {
            println!("loading: held at {} ({HOLD})", stage.word());
            self.held.store(true, Ordering::Relaxed);
            let from = Instant::now();
            while long.is_none_or(|l| from.elapsed() < l) {
                if let Err(e) = self.check() {
                    self.held.store(false, Ordering::Relaxed);
                    return Err(e);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            self.held.store(false, Ordering::Relaxed);
        }
        Ok(())
    }

    /// `n` of the stage's `of` done; a cancelled load's error.
    pub fn count(&self, n: usize, of: usize) -> Result<(), String> {
        self.of.store(of as u32, Ordering::Relaxed);
        self.n.store(n.min(of) as u32, Ordering::Relaxed);
        self.check()
    }

    /// A cancelled load's error, between two steps of a stage.
    pub fn check(&self) -> Result<(), String> {
        match self.cancel.load(Ordering::Relaxed) {
            true => Err(CANCELLED.to_string()),
            false => Ok(()),
        }
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Whether the hook is holding the load now.
    pub fn held(&self) -> bool {
        self.held.load(Ordering::Relaxed)
    }

    pub fn stage(&self) -> Stage {
        Stage::ALL[(self.stage.load(Ordering::Relaxed) as usize).min(Stage::ALL.len() - 1)]
    }

    /// The bar: each stage an equal share, the counted ones filling theirs as they count.
    pub fn done(&self) -> f32 {
        let (n, of) = (self.n.load(Ordering::Relaxed), self.of.load(Ordering::Relaxed));
        let within = if of == 0 { 0.0 } else { n as f32 / of as f32 };
        (self.stage().index() as f32 + within) / Stage::ALL.len() as f32
    }

    /// The line under the bar: "Ground 12 of 55", "Props".
    pub fn line(&self) -> String {
        let (n, of) = (self.n.load(Ordering::Relaxed), self.of.load(Ordering::Relaxed));
        match of {
            0 => self.stage().label().to_string(),
            _ => format!("{} {n} of {of}", self.stage().label()),
        }
    }
}

impl Default for Progress {
    fn default() -> Progress {
        Progress::with_hold(None)
    }
}

/// Builds running now, on any thread: a test's proof that a cancelled one is gone.
static WORKERS: AtomicU32 = AtomicU32::new(0);

pub fn workers() -> u32 {
    WORKERS.load(Ordering::SeqCst)
}

/// Counts a build in [`WORKERS`] while it runs, panic or not.
struct Running;

impl Running {
    fn new() -> Running {
        WORKERS.fetch_add(1, Ordering::SeqCst);
        Running
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        WORKERS.fetch_sub(1, Ordering::SeqCst);
    }
}

/// `build` on a thread of its own, and `wait` on this one, again and again until it ends: what it
/// built, or what `wait` stopped for (`Err`), in which case the build is cancelled and has ended
/// before this returns. The thread is scoped, so the build may borrow what outlives this call — the
/// ride's terrain bytes, which the simulation's ground points into — and may hand back what borrows
/// it. A panic in the build is an error like any other.
pub fn on_worker<'env, T: Send + 'env, L>(
    progress: &Progress,
    build: impl FnOnce() -> Result<T, String> + Send + 'env,
    mut wait: impl FnMut() -> Result<Option<L>, String>,
) -> Result<Result<T, L>, String> {
    // The browser has no second thread (P196): the build runs here, whole, and `wait` never does.
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (progress, &mut wait);
        let _running = Running::new();
        return build().map(Ok);
    }
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .name("ride load".into())
            .spawn_scoped(scope, move || {
                let _running = Running::new();
                build()
            })
            .map_err(|e| format!("the ride's load could not start a thread: {e}"))?;
        while !handle.is_finished() {
            let stop = match wait() {
                Ok(None) => continue,
                Ok(Some(leave)) => Ok(Err(leave)),
                Err(e) => Err(e),
            };
            progress.cancel();
            let _ = handle.join();
            return stop;
        }
        handle.join().map(|built| built.map(Ok)).unwrap_or_else(|panic| {
            let why = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "a panic".to_string());
            Err(format!("the load stopped: {why}"))
        })
    })
}
