//! A supercross stadium's camera flashes (V127).
//!
//! The race activity's loader, on a track whose name holds `sx`, gathers every scene model named
//! `cameraflash` (sx01 places 1200 of them in its stands), hides them all, and keeps them in a pool.
//! Its flash object then runs every tick: a brightness that rests at the track's constant share
//! (`[CameraFlash]` `ConstantFlashPercent`), and that each podium finish
//! ([`crate::race::Supercross::race_over`]) sends up toward a burst share and back down again over
//! the burst's length. Whenever no batch is lit, a batch is: that share of the hidden pool, each
//! picked at random and shown. A batch stays up 0.18 s, its flashes fading from full as it runs
//! down, and then goes back into the pool, and the next batch is picked on that same tick.
//!
//! This is the flash object's tick as the original runs it, word for word: the brightness ramps,
//! the queue of lit flashes with its times (the first of a batch carries the 0.18 s, the rest 0, so
//! they all run out together), the pick from the pool (the original's `rand()` scaled to an index,
//! swap-removed), and each shown flash's fade. What the shell does with it is draw the shown flashes
//! ([`CameraFlashes::shown`]); nothing in the simulation reads it.
//!
//! The draws: the original takes them from the one `rand()` everything shares, so a race on an sx
//! track moves the shared generator about a dozen draws every 0.18 s. [`CameraFlashes::tick`] takes
//! the generator it is handed; the shell hands it one of its own (see V127's journal).
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::race::PodiumFlashes;
use crate::rng::Rng;
use ps2_float::{F, fpu};

/// How long a batch stays up, in seconds: the first of a batch's queue time, and what its fade is
/// counted over.
pub const BATCH: F = F(0.18);

/// Where the brightness starts, before the first tick puts it at the floor (the object's constructor).
const FIRST_BRIGHTNESS: F = F(f32::from_bits(0x3e4c_cccd));

/// The model name the loader gathers (the original compares the whole name).
pub const MODEL: &str = "cameraflash";

/// A word as the FPU reads it out of memory (denormals flushed).
fn read(x: F) -> F {
    F(fpu::operand(x.0.to_bits()))
}

/// `a < b` as `c.olt.s` reads the two words.
fn lt(a: F, b: F) -> bool {
    read(a).0 < read(b).0
}

/// `a <= b` as `c.ole.s` reads the two words.
fn le(a: F, b: F) -> bool {
    read(a).0 <= read(b).0
}

/// The flash object (the activity's `+0x414`). Flashes are numbered by their place in the scene's
/// list of `cameraflash` models, from 0.
#[derive(Clone, Debug, PartialEq)]
pub struct CameraFlashes {
    /// The brightness (`from`, `+0x5c`) and the burst a podium finish sets going (`+0x64`, `+0x68`,
    /// `+0x6c`): what [`crate::race::Supercross::race_over`] is handed.
    pub burst: PodiumFlashes,
    /// Where the brightness rests between bursts (`+0x60`, the track's `ConstantFlashPercent`).
    pub floor: F,
    /// No batch is lit: the next tick picks one (`+0x58`).
    pub ready: bool,
    /// The hidden flashes (`+0x30`), in the order the swap-removes and returns leave them.
    pool: Vec<u32>,
    /// The queue of lit flashes (`+0x48`), a ring as long as there are flashes (`+0x54`): each
    /// entry's time and flash, from `head` (`+0x50`), `count` (`+0x4c`) of them.
    times: Vec<F>,
    lit: Vec<u32>,
    head: usize,
    count: usize,
    /// Each flash's fade (the model's `+0x16c`), set while it is lit: 1 as its batch is picked,
    /// running down to 0 as the batch's time does.
    fade: Vec<F>,
}

impl CameraFlashes {
    /// The flash object over `flashes` hidden flashes, resting at `floor`. `None` with no flashes,
    /// as the loader throws the object away when no model matched.
    pub fn new(flashes: u32, floor: F) -> Option<CameraFlashes> {
        if flashes == 0 {
            return None;
        }
        let n = flashes as usize;
        Some(CameraFlashes {
            burst: PodiumFlashes { from: FIRST_BRIGHTNESS, ..PodiumFlashes::default() },
            floor,
            ready: true,
            pool: (0..flashes).collect(),
            times: vec![F::ZERO; n],
            lit: vec![0; n],
            head: 0,
            count: 0,
            // What a model's fade word holds before the object first sets it was not read: taken
            // as full (see V127's journal).
            fade: vec![F::ONE; n],
        })
    }

    /// The object as a capture finds it on the tick a batch was picked: the queue holds that batch
    /// alone (`count` flashes from `head`, the first carrying [`BATCH`]), and the rest are hidden.
    /// Which flash is which is not in the capture's words; they are numbered in queue order.
    pub fn resumed(flashes: u32, floor: F, burst: PodiumFlashes, head: usize, count: usize) -> CameraFlashes {
        let mut f = CameraFlashes::new(flashes, floor).expect("flashes");
        let n = flashes as usize;
        f.burst = burst;
        f.ready = false;
        f.head = head;
        f.count = count;
        for k in 0..count {
            f.lit[(head + k) % n] = k as u32;
            f.times[(head + k) % n] = if k == 0 { BATCH } else { F::ZERO };
        }
        f.pool = (count as u32..flashes).collect();
        f
    }

    /// How bright the crowd's flashing is: the share of the hidden pool the next batch lights.
    pub fn brightness(&self) -> F {
        self.burst.from
    }

    /// How many flashes are lit, how many hidden, and where the queue starts.
    pub fn lit_count(&self) -> usize {
        self.count
    }

    pub fn hidden(&self) -> usize {
        self.pool.len()
    }

    pub fn head(&self) -> usize {
        self.head
    }

    /// The lit flashes and their fades, in queue order.
    pub fn shown(&self) -> impl Iterator<Item = (u32, F)> + '_ {
        let n = self.times.len();
        (0..self.count).map(move |k| {
            let flash = self.lit[(self.head + k) % n];
            (flash, self.fade[flash as usize])
        })
    }

    /// One tick of `dt` seconds. The brightness first: while more than half the burst is left it
    /// climbs at the burst's rate, after that it falls at it (both clamped to `[0, 1]`, the length
    /// running down to 0), and with no burst left it rests at the floor. Then the queue runs `dt`
    /// down its times: each entry it runs past is done (and leaves the object ready for a batch);
    /// the one it stops in keeps what is left. The flashes still lit take the fade of the first of
    /// them, its time over [`BATCH`]. Then, when ready and anything is hidden, a batch of
    /// `brightness × hidden` flashes (truncated) is picked, each `rng.index(hidden)` swap-removed
    /// from the pool, shown and queued with time 0, and if there were any the first of them gets
    /// [`BATCH`] and the object is no longer ready. Last, the done entries are hidden and go back to
    /// the pool, in queue order.
    pub fn tick(&mut self, dt: F, rng: &mut Rng) {
        let b = &mut self.burst;
        if le(b.length, b.half) {
            if le(b.length, F::ZERO) {
                b.from = self.floor;
            } else {
                b.length = F(fpu::max(fpu::sub(b.length.0, dt.0), 0.0));
                let down = fpu::sub(b.from.0, fpu::mul(b.rate.0, dt.0));
                b.from = F(fpu::min(fpu::max(down, 0.0), 1.0));
            }
        } else {
            b.length = F(fpu::max(fpu::sub(b.length.0, dt.0), 0.0));
            let up = fpu::add(b.from.0, fpu::mul(b.rate.0, dt.0));
            b.from = F(fpu::min(fpu::max(up, 0.0), 1.0));
        }
        let n = self.times.len();
        let mut left = dt;
        let mut done = 0;
        for k in 0..self.count {
            let at = (k + self.head) % n;
            left = F(fpu::sub(left.0, self.times[at].0));
            if lt(left, F::ZERO) {
                self.times[at] = F(fpu::mul(-1.0, left.0));
                break;
            }
            self.ready = true;
            done += 1;
        }
        if done < self.count {
            let fade = F(fpu::div(self.times[(done + self.head) % n].0, BATCH.0));
            for k in done..self.count {
                self.fade[self.lit[(k + self.head) % n] as usize] = fade;
            }
        }
        let before = self.count;
        if self.ready && !self.pool.is_empty() {
            let picks = fpu::to_i32(fpu::mul(self.burst.from.0, fpu::from_i32(self.pool.len() as i32)));
            for _ in 0..picks.max(0) {
                // A draw whose unit rounds to 1 (one in 2^25) indexes one past the pool; the original
                // reads the stale word there and fails the remove. Taken as the last flash here.
                let k = (rng.index(self.pool.len() as i32) as usize).min(self.pool.len() - 1);
                let flash = self.pool[k];
                self.pool.swap_remove(k);
                let at = (self.head + self.count) % n;
                self.lit[at] = flash;
                self.times[at] = F::ZERO;
                self.count += 1;
            }
            if picks > 0 {
                self.ready = false;
                self.times[(self.head + before) % n] = BATCH;
            }
        }
        for k in 0..done {
            self.pool.push(self.lit[(self.head + k) % n]);
        }
        self.head = (self.head + done) % n;
        self.count -= done;
    }
}
