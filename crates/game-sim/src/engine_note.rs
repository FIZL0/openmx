//! **The bike's engine note** (S328b): the original's state machine that picks the engine's samples,
//! and the Sim's own run of it on the game's generator.
//!
//! The note is presentation in what it plays, but not in what it draws: each sample it picks from a
//! list of more than one is a `rand()` on the game's one generator, and so is the pitch variance of
//! every sample it starts where someone can hear it. C33's four racers show it: ai0's and ai2's notes
//! each make two draws at tick 15, when they open the throttle, between the AI riders' own. A port
//! whose note draws its own dice parts from the original's generator there (S328), so the machine
//! lives here, where the Sim can run it on the race's generator, and the shell's mixer runs the same
//! machine on its own dice (`openmx::audio`, which re-exports this module's types).
//!
//! # The Sim's run
//!
//! [`Notes`] is a note per player, given to a Sim that races ([`crate::Sim::notes`]); without it the
//! Sim draws nothing for the notes, as before. Each frame, after the bodies' look-around clocks and
//! before the race manager, every player's note runs in the walk's order (the last player first, as
//! the original's actor list walks them):
//!
//! - **The start delay.** The loader gives each rider's note a delay of one unit draw in seconds; the
//!   rider's update counts it down and switches the note on when it runs out. Until then the note is
//!   off: it neither decides nor draws.
//! - **The machine**, [`Brain::frame`], with the rider's hands: on the ground, the throttle, the speed
//!   and the clutch lever the bike rode.
//! - **The voices**, [`Voices`]: what the machine reads back of its samples (sounding or not, how far
//!   in) is the sound chip's in the original. Here each sample started is a clock that runs with the
//!   frames, and one is heard — started, and its pitch drawn — only while the emitter is within its
//!   far distance of a listener. The listeners are the players who are not AI riders (the original's
//!   are the cameras, which follow them).

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::ride::Controls;
use crate::rng::Rng;
use ps2_float::{F, Vf};
use std::sync::Arc;

/// The engine note's states, numbered as the original's bike sound numbers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Note {
    /// Nothing chosen: a chain that ran out with nowhere to go.
    Off = 0,
    Idle = 1,
    LowAccel = 2,
    HighAccel = 3,
    Power = 4,
    Sputter = 5,
    Decel = 6,
    Clutch = 7,
    /// The free rev, `[Panic]`'s first sample, up to `RevMidpoint`.
    Rev = 8,
    /// Held at the limiter: the rev sample paused at its midpoint, `[Panic]`'s loop playing over it.
    Panic = 9,
    /// Off the gas after the limiter: the rev sample resumed, falling back to the idle.
    Unwind = 10,
}

impl Note {
    /// The four states a throttle held on the ground plays.
    fn pulling(self) -> bool {
        matches!(self, Note::LowAccel | Note::HighAccel | Note::Power | Note::Sputter)
    }

    /// The list a state's samples are in, in [`NoteSounds::lists`]' order: `Rev` and the limiter's
    /// states are `[Panic]`.
    pub fn slot(self) -> usize {
        match self {
            Note::Idle => 0,
            Note::LowAccel => 1,
            Note::HighAccel => 2,
            Note::Power => 3,
            Note::Sputter => 4,
            Note::Decel => 5,
            Note::Clutch => 6,
            Note::Rev | Note::Panic | Note::Unwind | Note::Off => 7,
        }
    }
}

/// What the machine reads of a bike's `vehicle_*.aud`: how many samples each list has, each one's
/// looping flag and length, and the `[Params]` it decides by.
pub trait Sounds {
    /// How many samples `list` has.
    fn count(&self, list: Note) -> usize;
    fn looping(&self, s: Sample) -> bool;
    /// A sample's length in seconds (0 for one the disc lacks).
    fn seconds(&self, s: Sample) -> f32;
    /// `LowTorqueSpeed` in feet a second.
    fn low_torque_speed(&self) -> f32;
    fn airborne_to_decel(&self) -> f32;
    fn rev_midpoint(&self) -> f32;
    /// `OverlapTime`: how long before a sample's end the next one starts under it.
    fn overlap(&self) -> f32;
    /// `PitchVariance`, in the sound chip's pitch units.
    fn pitch_variance(&self) -> i32;
}

/// A bike's sounds as plain numbers, for the Sim, which holds no clips.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NoteSounds {
    /// Each list's samples, in [`Note::slot`] order: length in seconds and looping flag.
    pub lists: [Vec<(f32, bool)>; 8],
    pub low_torque_speed: f32,
    pub airborne_to_decel: f32,
    pub rev_midpoint: f32,
    pub overlap: f32,
    pub pitch_variance: i32,
    /// The distance past which the emitter is heard by no one, in feet.
    pub far: f32,
}

impl NoteSounds {
    fn get(&self, s: Sample) -> Option<&(f32, bool)> {
        self.lists[s.list.slot()].get(s.index)
    }
}

impl Sounds for NoteSounds {
    fn count(&self, list: Note) -> usize {
        self.lists[list.slot()].len()
    }

    fn looping(&self, s: Sample) -> bool {
        self.get(s).is_some_and(|l| l.1)
    }

    fn seconds(&self, s: Sample) -> f32 {
        self.get(s).map_or(0.0, |l| l.0)
    }

    fn low_torque_speed(&self) -> f32 {
        self.low_torque_speed
    }

    fn airborne_to_decel(&self) -> f32 {
        self.airborne_to_decel
    }

    fn rev_midpoint(&self) -> f32 {
        self.rev_midpoint
    }

    fn overlap(&self) -> f32 {
        self.overlap
    }

    fn pitch_variance(&self) -> i32 {
        self.pitch_variance
    }
}

/// What the engine note hears from the ride each frame: the original's four (on the ground, the
/// throttle, the speed, the clutch), and where the bike is and how fast it is going for the mix.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hands {
    pub grounded: bool,
    pub gas: f32,
    pub clutch: f32,
    /// Feet a second.
    pub speed: f32,
    pub at: [f32; 3],
    /// Feet a second, the world's axes.
    pub vel: [f32; 3],
}

impl Hands {
    /// The hands while the rider is wrecked: off the bike, so off the throttle and the clutch lever
    /// whatever the pad still holds (user 2026-10-30: 'when wrecking im still hearing the bike motor
    /// drive, it should not'). The note falls to the decel and the idle; the rest is the bike's own.
    pub fn wrecked(self, wrecked: bool) -> Hands {
        match wrecked {
            true => Hands { gas: 0.0, clutch: 0.0, ..self },
            false => self,
        }
    }

    /// The hands with MX vs. ATV's quad's throttle lock (its throttle-enabled word cleared, as at a
    /// start gate). The quad's note decides as the bike's but for one test: on the ground it revs
    /// freely when the clutch is in *or* the throttle is locked. The clutch is read nowhere else, so
    /// a locked throttle is the clutch held in to the note.
    pub fn throttle_locked(self, locked: bool) -> Hands {
        match locked {
            true => Hands { clutch: 1.0, ..self },
            false => self,
        }
    }
}

/// One of a bike's engine samples: the list it is in (as [`Note::slot`] names them) and its place
/// there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub list: Note,
    pub index: usize,
}

impl Sample {
    pub const IDLE: Sample = Sample { list: Note::Idle, index: 0 };
    /// The free rev.
    pub const REV: Sample = Sample { list: Note::Rev, index: 0 };
    /// The limiter's loop.
    pub const PANIC: Sample = Sample { list: Note::Rev, index: 1 };
}

/// A source of the original's `rand()`: `0..=0x7fff_ffff`.
pub trait Draw {
    fn rand(&mut self) -> i32;
}

/// The game's own generator, which is what the original's note draws.
impl Draw for Rng {
    fn rand(&mut self) -> i32 {
        Rng::rand(self)
    }
}

/// `rand()` as the original scales it, `[0, 1)`.
fn unit(d: &mut impl Draw) -> F {
    F(1.0 / 2_147_483_648.0) * F::from(d.rand())
}

/// **The no-repeat pick** the engine's lists use: a list of one or none answers its first sample and
/// draws nothing; a longer one draws an index, moved on by one (and wrapped) when it is the last pick.
pub fn pick(count: i32, last: &mut i32, d: &mut impl Draw) -> i32 {
    if count < 2 {
        *last = 0;
        return 0;
    }
    let mut i = (F::from(count) * unit(d)).to_i32();
    if i == *last {
        i = (i + 1) % count;
    }
    *last = i;
    i
}

/// **A sample's pitch variance**: a uniform offset in `-variance..variance` pitch units, from one draw.
pub fn jitter(variance: i32, d: &mut impl Draw) -> i32 {
    let r = d.rand();
    (F::from(variance << 1) * F::from(r) * F(1.0 / 2_147_483_648.0)).to_i32() - variance
}

/// What the engine note plays on: the original's emitter and its sounds, the mixer in the shell, the
/// original itself in the test, [`Voices`] in the Sim.
pub trait Bed {
    /// The sample is sounding (a paused one is still sounding).
    fn playing(&self, s: Sample) -> bool;
    /// Seconds into the sample's voice, or -1 when it has none.
    fn position(&self, s: Sample) -> f32;
    /// Stop the sample.
    fn stop(&mut self, s: Sample);
    /// Start the sample where the bike is: false, and nothing started, when no one is near enough to
    /// hear it or the bike's sound is gated off.
    fn play(&mut self, s: Sample) -> bool;
    /// Move a sample just started off its own pitch, in pitch units.
    fn detune(&mut self, s: Sample, by: i32);
    /// The limiter: pause the free rev and start `[Panic]`'s loop over it, or stop the loop and let
    /// the rev go on.
    fn hold(&mut self, on: bool);
}

/// **The engine note's state machine**, the original's: the frame's decision from the hands, else the
/// chain to the next sample once the one sounding is about to end. One sample is the note's at a time
/// (`current`); one that is not cut plays out under the next.
#[derive(Clone, Debug, PartialEq)]
pub struct Brain {
    pub note: Note,
    /// The state the chain goes to when the current sample ends.
    pub next: Note,
    /// The sample that is the note's: the emitter's queue.
    pub current: Sample,
    /// Counts down off the throttle from a quarter second; with `hold_off_gas` the accelerations hold
    /// until it runs out.
    pub off_gas: f32,
    /// Counts down in the air from `AirborneTimeToDecel`.
    pub air: f32,
    pub hold_off_gas: bool,
    /// A throttle in the air revs the engine; without it the air is the throttle let go.
    pub air_revs: bool,
    /// The throttle was held last frame.
    pub was_gas: bool,
    /// The limiter's loop is playing.
    pub held: bool,
    pub muted: bool,
    /// The last picks: low, high, power, sputter, clutch.
    pub last: [i32; 5],
}

/// How long the throttle is remembered after it is let go.
const OFF_GAS_HOLD: f32 = 0.25;

impl Default for Brain {
    fn default() -> Brain {
        Brain::new()
    }
}

impl Brain {
    /// The player's note as the loader leaves it: idle queued and not yet started, nothing to chain to,
    /// nothing picked, the throttle in the air revving the engine.
    pub fn new() -> Brain {
        Brain {
            note: Note::Idle,
            next: Note::Off,
            current: Sample::IDLE,
            off_gas: 0.0,
            air: 0.0,
            hold_off_gas: false,
            air_revs: true,
            was_gas: false,
            held: false,
            muted: false,
            last: [-1; 5],
        }
    }

    /// One frame.
    pub fn frame(&mut self, h: &Hands, dt: f32, sounds: &impl Sounds, bed: &mut impl Bed, d: &mut impl Draw) {
        if !self.muted && !self.decide(h, dt, sounds, bed, d) {
            self.chain(h, sounds, bed, d);
        }
    }

    /// **The frame's decision**: true when it started a note (and the chain waits a frame).
    pub fn decide(&mut self, h: &Hands, dt: f32, sounds: &impl Sounds, bed: &mut impl Bed, d: &mut impl Draw) -> bool {
        let gas = 0.1 < h.gas;
        let mut started = false;
        if !h.grounded {
            self.air = (F(self.air) - F(dt)).0;
            if gas && self.air_revs {
                if !self.was_gas {
                    self.air = 0.0;
                    self.rev(Note::Rev, h, sounds, bed, d);
                    started = true;
                } else if self.air <= 0.0 && self.note.pulling() {
                    self.air = 0.0;
                    self.decel(bed, d, sounds);
                    started = true;
                }
                self.was_gas = true;
                return started;
            }
            if self.note == Note::Panic {
                self.rev(self.next, h, sounds, bed, d);
            } else if self.note.pulling() {
                self.decel(bed, d, sounds);
            } else {
                self.was_gas = false;
                return false;
            }
            self.was_gas = false;
            return true;
        }
        if 0.1 < h.clutch {
            if gas {
                if !self.was_gas {
                    self.rev(Note::Rev, h, sounds, bed, d);
                    started = true;
                } else if 1.0 < h.speed && self.note < Note::Clutch {
                    self.rev(Note::Clutch, h, sounds, bed, d);
                    started = true;
                }
                self.was_gas = true;
            } else {
                if self.note.pulling() {
                    self.decel(bed, d, sounds);
                    started = true;
                } else if self.note == Note::Clutch {
                    self.next = Note::Idle;
                } else if self.note == Note::Panic {
                    self.rev(self.next, h, sounds, bed, d);
                    started = true;
                }
                self.was_gas = false;
            }
        } else if gas {
            self.off_gas = OFF_GAS_HOLD;
            if matches!(self.note, Note::Unwind | Note::Panic | Note::Rev | Note::Idle | Note::Decel | Note::Off) {
                self.accel(h, sounds, bed, d);
                started = true;
            } else if self.note == Note::Clutch {
                self.power(sounds, bed, d);
                started = true;
            }
            self.was_gas = true;
        } else {
            self.off_gas = (F(self.off_gas) - F(dt)).0;
            if !self.hold_off_gas || self.off_gas <= 0.0 {
                if self.note == Note::Panic {
                    self.rev(self.next, h, sounds, bed, d);
                    started = true;
                } else if self.note.pulling() {
                    self.decel(bed, d, sounds);
                    started = true;
                }
            }
            self.was_gas = false;
        }
        self.air = sounds.airborne_to_decel();
        started
    }

    /// **The chain**: once the current sample is within `OverlapTime` of its end (or has ended, or never
    /// started), the next state's sample; the free rev hands on at `RevMidpoint` instead. A looping
    /// sample never ends.
    pub fn chain(&mut self, h: &Hands, sounds: &impl Sounds, bed: &mut impl Bed, d: &mut impl Draw) {
        let cur = self.current;
        let at_midpoint = self.note == Note::Rev && sounds.rev_midpoint() < bed.position(cur);
        if !at_midpoint {
            if sounds.looping(cur) {
                return;
            }
            if bed.playing(cur) && (F(sounds.overlap()) + F(bed.position(cur))).0 < sounds.seconds(cur) {
                return;
            }
        }
        match self.next {
            Note::Unwind => {}
            Note::Panic | Note::Rev => self.rev(self.next, h, sounds, bed, d),
            Note::Sputter => self.sputter(sounds, bed, d),
            Note::Power => self.power(sounds, bed, d),
            Note::Idle => self.idle(sounds, bed, d),
            Note::Off => self.note = Note::Off,
            _ => {}
        }
    }

    fn limiter(&mut self, on: bool, bed: &mut impl Bed) {
        if self.held != on {
            self.held = on;
            bed.hold(on);
        }
    }

    /// Make `s` the note's sample and start it. `cut` stops the one sounding first; `unless_playing`
    /// leaves a sample that is still sounding alone. The idle is never detuned.
    fn sound(&mut self, s: Sample, cut: bool, unless_playing: bool, sounds: &impl Sounds, bed: &mut impl Bed, d: &mut impl Draw) {
        if self.muted {
            return;
        }
        if self.held {
            self.limiter(false, bed);
        }
        if cut {
            bed.stop(self.current);
        }
        self.current = s;
        if unless_playing && bed.playing(s) {
            return;
        }
        if bed.play(s) && s != Sample::IDLE {
            let by = jitter(sounds.pitch_variance(), d);
            bed.detune(s, by);
        }
    }

    fn picked(&mut self, list: Note, slot: usize, sounds: &impl Sounds, d: &mut impl Draw) -> Sample {
        let n = sounds.count(list) as i32;
        Sample { list, index: pick(n, &mut self.last[slot], d) as usize }
    }

    fn idle(&mut self, sounds: &impl Sounds, bed: &mut impl Bed, d: &mut impl Draw) {
        if self.note != Note::Idle {
            self.note = Note::Idle;
            self.sound(Sample::IDLE, false, true, sounds, bed, d);
            self.next = Note::Idle;
        }
    }

    /// The throttle opened: the low or high acceleration by the speed, cut straight in from the idle,
    /// the deceleration or the rev.
    fn accel(&mut self, h: &Hands, sounds: &impl Sounds, bed: &mut impl Bed, d: &mut impl Draw) {
        if matches!(self.note, Note::Unwind | Note::Panic | Note::Rev) {
            let s = self.picked(Note::HighAccel, 1, sounds, d);
            self.note = Note::HighAccel;
            self.sound(s, true, false, sounds, bed, d);
        } else {
            let cut = matches!(self.note, Note::Decel | Note::Idle);
            let s = if h.speed <= sounds.low_torque_speed() {
                self.note = Note::LowAccel;
                self.picked(Note::LowAccel, 0, sounds, d)
            } else {
                self.note = Note::HighAccel;
                self.picked(Note::HighAccel, 1, sounds, d)
            };
            self.sound(s, cut, !cut, sounds, bed, d);
        }
        self.next = Note::Power;
    }

    fn power(&mut self, sounds: &impl Sounds, bed: &mut impl Bed, d: &mut impl Draw) {
        let s = self.picked(Note::Power, 2, sounds, d);
        self.note = Note::Power;
        self.sound(s, false, true, sounds, bed, d);
        self.next = Note::Sputter;
    }

    fn sputter(&mut self, sounds: &impl Sounds, bed: &mut impl Bed, d: &mut impl Draw) {
        let s = self.picked(Note::Sputter, 3, sounds, d);
        self.note = Note::Sputter;
        self.sound(s, false, true, sounds, bed, d);
        self.next = Note::Power;
    }

    fn decel(&mut self, bed: &mut impl Bed, d: &mut impl Draw, sounds: &impl Sounds) {
        self.note = Note::Decel;
        self.sound(Sample { list: Note::Decel, index: 0 }, true, false, sounds, bed, d);
        self.next = Note::Idle;
    }

    /// The clutch's blip, the free rev, the limiter and the fall from it, by the state asked for.
    fn rev(&mut self, want: Note, h: &Hands, sounds: &impl Sounds, bed: &mut impl Bed, d: &mut impl Draw) {
        match want {
            Note::Clutch => {
                let s = self.picked(Note::Clutch, 4, sounds, d);
                self.note = Note::Clutch;
                self.sound(s, true, false, sounds, bed, d);
                self.next = Note::Rev;
            }
            Note::Panic if 0.1 < h.gas => {
                self.limiter(true, bed);
                self.note = Note::Panic;
                self.next = Note::Unwind;
            }
            Note::Panic | Note::Unwind => {
                self.limiter(false, bed);
                self.note = Note::Unwind;
                self.next = Note::Idle;
            }
            _ => {
                self.note = Note::Rev;
                self.sound(Sample::REV, true, false, sounds, bed, d);
                self.next = Note::Panic;
            }
        }
    }
}

// --- the Sim's run --------------------------------------------------------------------------------

/// One sample's voice as the Sim keeps it: how far in it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Voice {
    pub s: Sample,
    /// Seconds into the sample.
    pub at: F,
    /// Started this frame: the voice keys on after the frame, so its clock starts with the next.
    pub fresh: bool,
    /// Held by the limiter.
    pub paused: bool,
}

/// **A note's voices**: each sample it started, as a clock that runs with the frames and ends at the
/// sample's length unless it loops.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Voices {
    pub voices: Vec<Voice>,
}

impl Voices {
    /// A frame's time on every voice, and the ended ones gone.
    fn run(&mut self, dt: F, sounds: &NoteSounds) {
        for v in &mut self.voices {
            if v.fresh {
                v.fresh = false;
            } else if !v.paused {
                v.at = v.at + dt;
            }
        }
        self.voices.retain(|v| sounds.looping(v.s) || v.at.0 < sounds.seconds(v.s));
    }
}

/// [`Voices`] as the machine's [`Bed`] for one frame: `heard` is whether a listener is near enough.
struct Stage<'a> {
    voices: &'a mut Voices,
    heard: bool,
}

impl Bed for Stage<'_> {
    fn playing(&self, s: Sample) -> bool {
        self.voices.voices.iter().any(|v| v.s == s)
    }

    fn position(&self, s: Sample) -> f32 {
        self.voices.voices.iter().rev().find(|v| v.s == s).map_or(-1.0, |v| v.at.0)
    }

    fn stop(&mut self, s: Sample) {
        self.voices.voices.retain(|v| v.s != s);
    }

    fn play(&mut self, s: Sample) -> bool {
        // A sample is one voice: starting it again starts it over.
        self.stop(s);
        if self.heard {
            self.voices.voices.push(Voice { s, at: F::ZERO, fresh: true, paused: false });
        }
        self.heard
    }

    fn detune(&mut self, _: Sample, _: i32) {}

    fn hold(&mut self, on: bool) {
        for v in self.voices.voices.iter_mut().filter(|v| v.s == Sample::REV) {
            v.paused = on;
        }
    }
}

/// **One player's note** in the Sim.
#[derive(Clone, Debug, PartialEq)]
pub struct RiderNote {
    pub brain: Brain,
    /// Seconds until the note is switched on; 0 once it has been (or was never set).
    pub delay: F,
    /// Switched on: the note runs each frame.
    pub on: bool,
    /// Where the emitter was put last frame: the 3-D mix measures from there. None before the first.
    pub at: Option<Vf>,
    pub voices: Voices,
    pub sounds: Arc<NoteSounds>,
}

impl RiderNote {
    /// A note as the loader leaves it, switched on after `delay` seconds (the loader's is one unit
    /// draw, [`Rng::unit`]).
    pub fn new(sounds: Arc<NoteSounds>, delay: F) -> RiderNote {
        RiderNote { brain: Brain::new(), delay, on: false, at: None, voices: Voices::default(), sounds }
    }

    /// The rider's update's countdown: the note switches on when the delay runs out.
    fn count_down(&mut self, dt: F) {
        if F::ZERO < self.delay {
            self.delay = self.delay - dt;
            if self.delay <= F::ZERO {
                self.on = true;
                self.delay = F::ZERO;
            }
        }
    }

    /// One frame of a note that is on: the voices' clocks, the machine, and the idle the loader
    /// queued started when it is not sounding; then the emitter is put at the bike.
    fn frame(&mut self, h: &Hands, here: Vf, listeners: &[Vf], dt: F, rng: &mut Rng) {
        let sounds = Arc::clone(&self.sounds);
        let from = self.at.unwrap_or(here);
        let far = F(sounds.far);
        let heard = listeners.iter().any(|&l| {
            let d = l - from;
            let (x, y, z) = (F(d.x()), F(d.y()), F(d.z()));
            x * x + y * y + z * z < far * far
        });
        self.voices.run(dt, &sounds);
        let mut stage = Stage { voices: &mut self.voices, heard };
        self.brain.frame(h, dt.0, &*sounds, &mut stage, rng);
        let b = &self.brain;
        if !b.muted && b.current == Sample::IDLE && b.note == Note::Idle && !stage.playing(Sample::IDLE) {
            stage.play(Sample::IDLE);
        }
        self.at = Some(here);
    }
}

/// **Every player's note**, by Sim player.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Notes {
    pub riders: Vec<RiderNote>,
}

impl crate::Sim {
    /// **The engine notes' frame** (S328b, the module header): every player's note in the walk's order
    /// (the last player first), on the game's generator — the AI lane's, or the race's when there is no
    /// lane. A Sim with no notes, or no generator, draws nothing here. `rode` is the controls each bike
    /// rode, for its clutch lever.
    pub(crate) fn notes_frame(&mut self, dt: F, rode: &[Controls]) {
        if self.notes.is_none() {
            return;
        }
        let ai = |i: usize| self.ai.as_deref().is_some_and(|l| l.pilots.iter().any(|(p, _)| *p == i));
        let n = rode.len();
        let listeners: Vec<Vf> = (0..n).filter(|&i| !ai(i)).map(|i| self.rider_of(i).ride.position()).collect();
        let hands: Vec<(Hands, Vf)> = (0..n)
            .map(|i| {
                let r = &self.rider_of(i).ride;
                let at = r.position();
                let h = Hands {
                    grounded: r.air.on_ground,
                    gas: r.held.gas.0,
                    clutch: rode[i].clutch.0,
                    speed: r.chassis.speed.0,
                    at: [at.x(), at.y(), at.z()],
                    vel: [r.chassis.vel.x(), r.chassis.vel.y(), r.chassis.vel.z()],
                };
                (h, at)
            })
            .collect();
        let rng = match (self.ai.as_deref_mut(), self.race.as_deref_mut()) {
            (Some(lane), _) => &mut lane.rng,
            (None, Some(race)) => &mut race.rng,
            (None, None) => return,
        };
        let Some(notes) = self.notes.as_deref_mut() else { return };
        for (i, note) in notes.riders.iter_mut().enumerate().take(n).rev() {
            note.count_down(dt);
            if note.on {
                note.frame(&hands[i].0, hands[i].1, &listeners, dt, rng);
            }
        }
    }
}
