//! The ride online: one [`Sim`] a seat, stepped in seat order over one world.
//!
//! A seat is a whole [`Sim`] of its own — its own input manager, bike and rider — and the riders do
//! not touch: S54's one Sim of N riders (with the original's rider-on-rider contact) is not what the
//! shell rides online yet. Each reads its seat's pad as pad 0, the shape every one-player capture of
//! the original has. In bytes (a resync, a restart) each seat is [`game_sim::snapshot`]'s codec.
//!
//! The frame length is the tick's own: every machine must hand every tick the same `dt`, and a wall
//! clock is a different number on each. It is [`tick_dt`], what the engine hands the simulation on
//! the offline ride's schedule (P52, `pace.rs`): one tick per 30 Hz frame of a PS2 holding 30 fps,
//! the clock a steady PS2 reads, a world load before the first. So an online ride is the ride the
//! same pads ride offline, tick for tick and `dt` for `dt` (N6).

use std::sync::{Arc, OnceLock};

use game_sim::input::{PADS, Report};
use game_sim::ride::Controls;
use game_sim::tick::{Engine, Timer0, clock_ms};
use game_sim::world::World;
use game_sim::{Input, Sim};
use ps2_float::F;

use super::Game;

/// The seats' states: what ggrs keeps a copy of every tick (each [`Sim`] shares its tables).
pub type State = Arc<Vec<Sim>>;

pub struct Rides<'a> {
    pub sims: &'a mut Vec<Sim>,
    pub world: &'a mut dyn World,
    /// After each seat's tick (the seat, the frame length, its state and the hands it rode with): the
    /// shell's body machine, which is the simulation's too ([`game_sim::rider::Rider::body_frame`])
    /// but needs the clip table the renderer holds.
    pub after: Option<&'a mut dyn FnMut(usize, F, &mut Sim, &Controls)>,
    /// The host's time-scale modifier (P55), [`game_sim::modifiers::Mods::time_scale`]: `None`, the
    /// original's, leaves [`tick_dt`] as it is.
    pub time_scale: Option<F>,
}

impl Game for Rides<'_> {
    type State = State;

    fn save(&self) -> State {
        Arc::new(self.sims.clone())
    }

    fn restore(&mut self, s: &State) {
        self.sims.clone_from(&**s);
    }

    fn step(&mut self, tick: usize, pads: &[Report]) {
        let dt = game_sim::modifiers::scaled_dt(tick_dt(tick), self.time_scale);
        for (seat, sim) in self.sims.iter_mut().enumerate() {
            let mut reports = [Report::missing(); PADS];
            reports[0] = pads.get(seat).copied().unwrap_or(Report::missing());
            let hands = sim.poll(dt, &reports);
            sim.step(dt, &hands, self.world);
            if let Some(after) = &mut self.after {
                after(seat, dt, sim, &hands);
            }
        }
    }

    fn hash(&self) -> u64 {
        self.sims.iter().fold(0, |h, s| game_sim::state_hash(&s.hash().to_le_bytes(), h))
    }

    /// Every seat's state in [`game_sim::snapshot`]'s codec (N1): the seat count, then each seat's
    /// bytes behind its length. What a resync sends a peer that fell out of step and a restart sends
    /// everyone; the constants (tables, bike, the rider's disc files) stay each machine's own.
    fn encode(&self) -> Option<Vec<u8>> {
        let mut out = (self.sims.len() as u32).to_le_bytes().to_vec();
        for sim in self.sims.iter() {
            let bytes = game_sim::snapshot::encode(sim);
            out.extend((bytes.len() as u32).to_le_bytes());
            out.extend(bytes);
        }
        Some(out)
    }

    /// [`Rides::encode`]'s bytes back, onto this machine's seats. Nothing is written unless every
    /// seat decodes, so a broken state leaves the ride as it was.
    fn decode(&mut self, bytes: &[u8]) -> Result<(), String> {
        let word = |at: usize, what: &str| -> Result<usize, String> {
            let w = bytes.get(at..at + 4).ok_or(format!("the state ends before {what}"))?;
            Ok(u32::from_le_bytes(w.try_into().unwrap()) as usize)
        };
        let seats = word(0, "its seat count")?;
        if seats != self.sims.len() {
            return Err(format!("{seats} seats in the state, {} here", self.sims.len()));
        }
        let mut at = 4;
        let mut sims = self.sims.clone();
        for (seat, sim) in sims.iter_mut().enumerate() {
            let n = word(at, "a seat's length")?;
            let one = bytes.get(at + 4..at + 4 + n).ok_or(format!("seat {}'s state is cut short", seat + 1))?;
            game_sim::snapshot::decode(sim, one).map_err(|e| format!("seat {}: {e}", seat + 1))?;
            at += 4 + n;
        }
        if at != bytes.len() {
            return Err(format!("{} bytes left over after the seats", bytes.len() - at));
        }
        *self.sims = sims;
        Ok(())
    }
}

/// The Timer 0 a PS2 holding 30 fps reads at its `frame`th frame: two NTSC fields a frame, 19,219.2
/// counts of the 576 kHz timer, in integers. `pace::ps2_clock`'s clock (the shell's offline one,
/// which this module cannot reach from the tests that mount it alone); `pace.rs`'s
/// `the_online_ticks_are_the_offline_ones` holds the two to the same `dt` tick for tick.
fn ps2_clock(frame: u64) -> Timer0 {
    let counts = frame * (2 * 1001 * 576) / 60;
    Timer0 { ticks: (counts / 57_600) as u32, count: (counts % 57_600) as u32 }
}

/// How many ticks of [`tick_dt`]'s ramp are worked out; every later tick is the last of them.
const RAMP: usize = 64;

/// The frame length of the ride's `tick`th tick (the first is 0), as the engine works it out on
/// pace's schedule (P52): the frame before the first is the menu's, a world load earlier, so the
/// first tick is the long-frame rule's 0.033 and the ticks average their way up to the 30 Hz cap.
/// There they stay for good: any thirty frames of this clock are 576,576 counts, 1001 ms exactly,
/// whose average is over the cap. The same bits on every machine: integer timer arithmetic and the
/// port's own float.
pub fn tick_dt(tick: usize) -> F {
    static DTS: OnceLock<Vec<F>> = OnceLock::new();
    let dts = DTS.get_or_init(|| {
        struct Idle;
        impl game_sim::tick::Game for Idle {
            fn simulate(&mut self, _: F, _: Input, _: &mut game_sim::rng::Rng) -> bool {
                true
            }
        }
        let mut engine = Engine::new();
        engine.prev_clock_ms = clock_ms(ps2_clock(1));
        (0..RAMP as u64).map(|t| engine.step(ps2_clock(1 + 19 * 30 + t), Input::default(), &mut Idle).dt).collect()
    });
    dts[tick.min(RAMP - 1)]
}
