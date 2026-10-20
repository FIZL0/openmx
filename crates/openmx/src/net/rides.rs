//! The ride online: one [`Sim`] a seat, stepped in seat order over one world.
//!
//! Until S54 makes the simulation N riders, a seat is a whole [`Sim`] of its own — its own input
//! manager, bike and rider — and the riders do not touch. Each reads its seat's pad as pad 0, the
//! shape every one-player capture of the original has.
//!
//! The frame length is fixed: every machine must hand every tick the same `dt`, and a wall clock is
//! a different number on each. It is [`steady_dt`], what the engine's own averaged frame time settles
//! at when the game holds 60 frames a second — the step a player at full speed rides offline.

use std::sync::Arc;

use game_sim::input::{PADS, Report};
use game_sim::ride::Controls;
use game_sim::tick::{Engine, Timer0};
use game_sim::world::World;
use game_sim::{Input, Sim};
use ps2_float::F;

use super::Game;

/// The seats' states: what ggrs keeps a copy of every tick (each [`Sim`] shares its tables).
pub type State = Arc<Vec<Sim>>;

pub struct Rides<'a> {
    pub sims: &'a mut Vec<Sim>,
    pub world: &'a mut dyn World,
    pub dt: F,
    /// After each seat's tick (the seat, the frame length, its state and the hands it rode with): the
    /// shell's body machine, which is the simulation's too ([`game_sim::rider::Rider::body_frame`])
    /// but needs the clip table the renderer holds.
    pub after: Option<&'a mut dyn FnMut(usize, F, &mut Sim, &Controls)>,
}

impl Game for Rides<'_> {
    type State = State;

    fn save(&self) -> State {
        Arc::new(self.sims.clone())
    }

    fn restore(&mut self, s: &State) {
        self.sims.clone_from(&**s);
    }

    fn step(&mut self, pads: &[Report]) {
        for (seat, sim) in self.sims.iter_mut().enumerate() {
            let mut reports = [Report::missing(); PADS];
            reports[0] = pads.get(seat).copied().unwrap_or(Report::missing());
            let hands = sim.poll(self.dt, &reports);
            sim.step(self.dt, &hands, self.world);
            if let Some(after) = &mut self.after {
                after(seat, self.dt, sim, &hands);
            }
        }
    }

    fn hash(&self) -> u64 {
        self.sims.iter().fold(0, |h, s| game_sim::state_hash(&s.hash().to_le_bytes(), h))
    }

    /// Not yet: the simulation has no byte codec until N1 (`game_sim::snapshot`). Without one a desync
    /// is caught and reported but not mended, and a silent peer ends the ride.
    fn encode(&self) -> Option<Vec<u8>> {
        None
    }

    fn decode(&mut self, _bytes: &[u8]) -> Result<(), String> {
        Err("the ride has no state codec yet (N1)".into())
    }
}

/// The frame length the engine settles at on a steady 60 Hz clock: its thirty-frame average after a
/// second of frames exactly a 60th apart (9,600 counts of the 576 kHz timer each). Integer timer
/// arithmetic and the port's own float, so the same number on every machine.
pub fn steady_dt() -> F {
    struct Idle;
    impl game_sim::tick::Game for Idle {
        fn simulate(&mut self, _: F, _: Input, _: &mut game_sim::rng::Rng) -> bool {
            true
        }
    }
    let mut engine = Engine::new();
    let mut dt = F(0.0);
    for n in 1..=60u32 {
        let counts = n * 9_600;
        let now = Timer0 { ticks: counts / 57_600, count: counts % 57_600 };
        dt = engine.step(now, Input::default(), &mut Idle).dt;
    }
    dt
}
