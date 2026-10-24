//! The ride online: one [`Sim`] a seat, stepped in seat order over one world.
//!
//! A seat is a whole [`Sim`] of its own — its own input manager, bike and rider. The bikes push each
//! other apart (S78, [`rider_contact::among`]): every seat's contacts are found from the tick's start
//! and each seat's step resolves its own, pushing the others' chassis. S54's one Sim of N riders (its
//! head-on joust's wreck) is not what the shell rides online yet. Each reads its seat's pad as pad 0, the shape every one-player capture of
//! the original has. In bytes (a resync, a restart) each seat is [`game_sim::snapshot`]'s codec.
//!
//! The frame length is the tick's own: every machine must hand every tick the same `dt`, and a wall
//! clock is a different number on each. It is [`tick_dt`], what the engine hands the simulation on
//! the offline ride's schedule (P52, `pace.rs`): one tick per 30 Hz frame of a PS2 holding 30 fps,
//! the clock a steady PS2 reads, a world load before the first. So an online ride is the ride the
//! same pads ride offline, tick for tick and `dt` for `dt` (N6).

use std::sync::{Arc, OnceLock};

use game_sim::input::{PADS, Report};
use game_sim::markers::{self, Spot};
use game_sim::ride::Controls;
use game_sim::tick::{Engine, Timer0, clock_ms};
use game_sim::flight::Flight;
use game_sim::world::World;
use game_sim::rider_contact;
use game_sim::{Input, Sim};
use ps2_float::F;

use super::Game;
use crate::vehicle::{self, Wanted};

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
    /// The ride's biplane as built and never flown (N17, [`plane_on`]), which a seat's switch to
    /// the plane takes a copy of into its [`Sim::plane`]: `None` on a track that parks none, or a
    /// ride with no `biplane.phy`, where a seat asking for the plane stays on its bike.
    pub plane: Option<&'a mut Flight>,
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
        // Where every seat's rider is before anybody's tick, which is where a marker's Up rides to
        // (P88): the same on every machine, as every seat's state is. A flying seat's is on the
        // ground under its plane (P107, as offline P104's), not at the bike it left.
        let spots: Vec<Spot> = self.sims.iter().map(|s| vehicle::lane_spot(s, self.world)).collect();
        // Who is in the ride (P94): a seat nobody sits in has the missing pad ([`Game::step`]), and is
        // no marker's pick. The pads are the session's, so every machine counts the same.
        let riding: Vec<bool> = (0..self.sims.len()).map(|s| pads.get(s).is_some_and(|p| p.0[0] != Report::missing().0[0])).collect();
        // A seat whose player has their pause open (P84, in its byte) is frozen where it is: not
        // polled, not stepped, and out of every bike's contacts, so the others pass through it.
        let paused: Vec<bool> = (0..self.sims.len()).map(|s| pads.get(s).is_some_and(|p| Wanted::of(p.0[super::SWITCH]).paused)).collect();
        let touching: Vec<bool> = riding.iter().zip(&paused).map(|(&r, &p)| r && !p).collect();
        // Rider against rider (S78): every seat's contacts, from where the last tick left every bike.
        let mut lists = rider_contact::among(self.sims, &touching, dt);
        // The last seat first, as the original walks its riders (S88, `Sim::step_all`).
        for seat in (0..self.sims.len()).rev() {
            let mut bump = lists
                .as_mut()
                .map(|l| std::mem::take(&mut l[seat]))
                .filter(|l| !l.is_empty())
                .map(|l| rider_contact::bump_among(self.sims, seat, l));
            if paused[seat] {
                continue;
            }
            let sim = &mut self.sims[seat];
            let mut reports = [Report::missing(); PADS];
            reports[0] = pads.get(seat).copied().unwrap_or(Report::missing());
            // The machine the seat's player wants (N17), switched to before the tick's step, as
            // the shell switches between frames offline (P102). A switch that cannot be had (no
            // plane, no ground) leaves the seat where it is, and is asked again next tick.
            let wanted = Wanted::of(reports[0].0[super::SWITCH]);
            if let Some(to) = wanted.to {
                let _ = vehicle::switch_lane(to, self.plane.as_deref(), sim, self.world);
            }
            // The shell's hands, the d-pad the markers' and not the ride's (P93) — unless the seat's
            // Markers switch is off (P129, in its byte), and then the d-pad is the original's.
            let hands = vehicle::hands(sim.poll(dt, &reports), !wanted.dpad_rides);
            // On the plane the bike is not stepped, its markers do not run and its rider's body
            // does not move, as offline (`Seat::pass`); the plane's own markers do (P107).
            if sim.plane.is_some() {
                vehicle::fly_lane(sim, dt, wanted, self.world);
                if !wanted.dpad_rides {
                    vehicle::lane_markers(sim, seat, &spots, &riding, self.world);
                }
                continue;
            }
            sim.step_bumped(dt, &hands, self.world, bump.as_mut());
            if !wanted.dpad_rides {
                markers::frame_among(sim, seat, &spots, &riding, &hands);
            }
            if let Some(after) = &mut self.after {
                after(seat, dt, sim, &hands);
            }
            if let Some(b) = &bump {
                rider_contact::bumped_among(self.sims, b);
            }
        }
    }

    /// Every seat's hash, and its put-back ghost beside it (P59): a one-player [`Sim`]'s walk leaves
    /// the ghost out (S63), and online every seat is one, so the checksum peers compare names it here.
    fn hash(&self) -> u64 {
        self.sims.iter().fold(0, |h, s| {
            let g = &s.rider.ghost;
            let mut bytes = s.hash().to_le_bytes().to_vec();
            bytes.extend(g.clock.0.to_bits().to_le_bytes());
            bytes.extend(g.alpha.0.to_bits().to_le_bytes());
            bytes.push(g.on as u8);
            game_sim::state_hash(&bytes, h)
        })
    }

    /// Every seat's state in [`game_sim::snapshot`]'s codec (N1): the seat count, then each seat's
    /// bytes behind its length ([`seat_bytes`]: its controller preset, then its state). What a resync
    /// sends a peer that fell out of step and a restart sends everyone; the constants (tables, bike,
    /// the rider's disc files) stay each machine's own, all but the preset (N13), which a seat a
    /// join added on another machine could not know otherwise.
    fn encode(&self) -> Option<Vec<u8>> {
        let mut out = (self.sims.len() as u32).to_le_bytes().to_vec();
        for sim in self.sims.iter() {
            let bytes = seat_bytes(sim);
            out.extend((bytes.len() as u32).to_le_bytes());
            out.extend(bytes);
        }
        Some(out)
    }

    /// [`Rides::encode`]'s bytes back, onto this machine's seats. Nothing is written unless every
    /// seat decodes, so a broken state leaves the ride as it was. A state with more seats than here
    /// is a ride someone joined (N10): the seats past this machine's are added on the first seat's
    /// constants, as [`Rides::seat`] adds them on the host; one with fewer is refused.
    fn decode(&mut self, bytes: &[u8]) -> Result<(), String> {
        let word = |at: usize, what: &str| -> Result<usize, String> {
            let w = bytes.get(at..at + 4).ok_or(format!("the state ends before {what}"))?;
            Ok(u32::from_le_bytes(w.try_into().unwrap()) as usize)
        };
        let seats = word(0, "its seat count")?;
        if seats < self.sims.len() || self.sims.is_empty() || seats > super::SEATS {
            return Err(format!("{seats} seats in the state, {} here", self.sims.len()));
        }
        let mut at = 4;
        let mut sims = self.sims.clone();
        sims.resize(seats, self.sims[0].clone());
        for (seat, sim) in sims.iter_mut().enumerate() {
            let n = word(at, "a seat's length")?;
            let one = bytes.get(at + 4..at + 4 + n).ok_or(format!("seat {}'s state is cut short", seat + 1))?;
            from_seat_bytes(sim, one).map_err(|e| format!("seat {}: {e}", seat + 1))?;
            at += 4 + n;
        }
        if at != bytes.len() {
            return Err(format!("{} bytes left over after the seats", bytes.len() - at));
        }
        sims.iter_mut().for_each(|sim| self.probe(sim));
        *self.sims = sims;
        Ok(())
    }

    /// A new rider on seat `seat` (N10's join): the first seat's constants — its tables, its bike's
    /// `.phy`, the rider's files, which every machine has the same of — in `spawn`'s state, one
    /// seat in [`spawn_on`]'s bytes, which say the joiner's own preset for each of its seats (N13). Seats between the last and it are added the same,
    /// nobody's (their pads unplugged).
    fn seat(&mut self, seat: usize, spawn: &[u8]) -> Result<(), String> {
        if seat >= super::SEATS {
            return Err(format!("seat {} of {}", seat + 1, super::SEATS));
        }
        let mut sim = self.sims.first().ok_or("no seat to take the constants from")?.clone();
        let (&n, rest) = spawn.split_first().ok_or("the spawn is empty")?;
        let (presets, state) = rest.split_at_checked(2 * n as usize).ok_or("the spawn's presets are cut short")?;
        from_seat_bytes(&mut sim, state).map_err(|e| format!("the spawn: {e}"))?;
        if let Some(p) = presets.chunks(2).find(|p| p[0] as usize == seat) {
            sim.set_presets([p[1] as usize, 0]);
        }
        self.probe(&mut sim);
        while self.sims.len() < seat {
            self.sims.push(sim.clone());
        }
        match self.sims.get_mut(seat) {
            Some(s) => *s = sim,
            None => self.sims.push(sim),
        }
        Ok(())
    }
}

impl Rides<'_> {
    /// A seat decoded flying (N17) whose plane came without its ray probe, a constant of the disc
    /// the codec leaves out and puts back only onto a seat that was flying here too: the ride's own.
    fn probe(&self, sim: &mut Sim) {
        if let (Some(f), Some(t)) = (sim.plane.as_mut(), self.plane.as_deref())
            && f.rays.is_none()
        {
            f.rays = t.rays.clone();
        }
    }
}

/// The ride's biplane for [`Rides::plane`] (N17): `stock`, the disc's `biplane.phy`, on 1P's
/// modifiers `mods` (every seat's online, [`Modded`]), built as the shell builds a seat's plane — the
/// flight off the file, then the ray probe and the track's area of `like` (any plane of this ride's) —
/// so every machine, and a machine the host's change rebuilt it on, holds the same one.
pub fn plane_on(stock: &game_sim::phy::Plane, mods: &game_sim::modifiers::Mods, like: &Flight) -> Flight {
    let mut file = *stock;
    mods.plane(0, &mut file);
    let mut f = Flight::new(&file);
    if let Some(rays) = &like.rays {
        f = f.with_rays((**rays).clone());
    }
    f.area = like.area;
    f
}

/// One seat in bytes (N13): its controller preset (P79), which its binding table is built on,
/// then its state in [`game_sim::snapshot`]'s codec. A seat's preset is a constant of the ride like
/// its bike, and every machine builds the seat's table on it; it travels with the state because a
/// seat a join adds is decoded onto another seat's constants. A table built on none of them —
/// which no online seat is: each is built by [`from_seat_bytes`] or `Sim::set_presets([p, 0])` —
/// goes as preset 0.
pub fn seat_bytes(sim: &Sim) -> Vec<u8> {
    let mut out = vec![preset(sim).unwrap_or(0) as u8];
    out.extend(game_sim::snapshot::encode(sim));
    out
}

/// [`seat_bytes`] back onto `sim`: its state, and its binding table built on the preset.
pub fn from_seat_bytes(sim: &mut Sim, bytes: &[u8]) -> Result<(), String> {
    let (&p, state) = bytes.split_first().ok_or("no controller preset")?;
    if p as usize >= game_sim::bindings::PRESETS {
        return Err(format!("controller preset {p}"));
    }
    game_sim::snapshot::decode(sim, state)?;
    sim.set_presets([p as usize, 0]);
    Ok(())
}

/// A joiner's spawn for [`Rides::seat`] (N13): `spawn`, [`seat_bytes`] of where the ride starts,
/// behind the controller preset of each of the joiner's seats as (seat, preset) — their count,
/// then the pairs. A seat not in the list rides the spawn's own preset.
pub fn spawn_on(spawn: &[u8], presets: &[(usize, u8)]) -> Vec<u8> {
    let mut out = vec![presets.len() as u8];
    for &(seat, preset) in presets {
        out.extend([seat as u8, preset]);
    }
    out.extend(spawn);
    out
}

/// A joiner's seat (P105): `spawn` ([`seat_bytes`] of where the ride starts) decoded onto `first`'s
/// constants with `markers` — the joiner's player's saved markers, as its handshake brought them —
/// put on it, in [`seat_bytes`] again, for [`spawn_on`]. The state every machine is sent at the join
/// carries them, so every machine seats the same slots.
pub fn spawn_marked(first: &Sim, spawn: &[u8], markers: markers::Markers) -> Result<Vec<u8>, String> {
    let mut sim = first.clone();
    from_seat_bytes(&mut sim, spawn)?;
    sim.markers = markers;
    Ok(seat_bytes(&sim))
}

/// A joiner's seat that starts in the biplane (N19): `spawn` ([`seat_bytes`] of where the ride
/// starts, or [`spawn_marked`]'s) decoded onto `first`'s constants with a copy of the ride's plane
/// `template` ([`plane_on`]) standing in `bay` in its [`Sim::plane`] ([`vehicle::start_lane`], N18's
/// start), in [`seat_bytes`] again, for [`spawn_on`]. The state every machine is sent at the join
/// carries the flight, so every machine seats the joiner flying from the same place.
pub fn spawn_flying(first: &Sim, spawn: &[u8], template: &Flight, bay: vehicle::Spawn) -> Result<Vec<u8>, String> {
    let mut sim = first.clone();
    from_seat_bytes(&mut sim, spawn)?;
    vehicle::start_lane(template, bay, &mut sim);
    Ok(seat_bytes(&sim))
}

/// Which controller preset a seat's bike is read through: the one whose table on pad 0 (and the
/// shipped one on pad 1, which no online seat reads) is the seat's.
pub fn preset(sim: &Sim) -> Option<usize> {
    (0..game_sim::bindings::PRESETS).find(|&p| game_sim::bindings::Table::built([p, 0]) == sim.bindings)
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

/// The ride with its game modifiers (P55), which the host changes mid-ride (P100): [`Rides`] as it
/// is, and the set every seat rides on with each seat's stock bike file, so that a restart that
/// changes the set ([`Game::remodify`]) moves every seat onto it from where it is. Online every seat
/// rides 1P's modifiers (each seat's rider is built on them), so the change is 1P's for all of them.
pub struct Modded<'a, 'b> {
    pub rides: &'a mut Rides<'b>,
    /// Each seat's bike file before any modifier; a seat past the last rides the first's, as
    /// [`Rides::seat`] and [`Rides::decode`] build it on the first seat's constants.
    pub stock: &'a [game_sim::phy::Phy],
    pub mods: &'a mut game_sim::modifiers::Mods,
    /// The disc's `biplane.phy` before any modifier (N17), which [`Rides::plane`] and every seat's
    /// plane in flight are moved onto a change from; `None` with no plane in the ride.
    pub stock_plane: Option<&'a game_sim::phy::Plane>,
}

impl Game for Modded<'_, '_> {
    type State = State;

    fn save(&self) -> State {
        self.rides.save()
    }

    fn restore(&mut self, s: &State) {
        self.rides.restore(s)
    }

    fn step(&mut self, tick: usize, pads: &[Report]) {
        self.rides.step(tick, pads)
    }

    fn hash(&self) -> u64 {
        self.rides.hash()
    }

    fn encode(&self) -> Option<Vec<u8>> {
        self.rides.encode()
    }

    fn decode(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.rides.decode(bytes)
    }

    fn seat(&mut self, seat: usize, spawn: &[u8]) -> Result<(), String> {
        self.rides.seat(seat, spawn)
    }

    /// Every seat's bike built again from its stock file on `to` and its ride retuned where it is
    /// ([`game_sim::modifiers::Mods::resim`], as the pause does offline, P98), from the state the
    /// host had on `was`; the ticks after it scaled by `to`'s time scale. A machine already on `to`
    /// with no change in it (a joiner the room told the set it is on) is left as it is.
    fn remodify(&mut self, was: &str, to: &str) -> Result<(), String> {
        use game_sim::modifiers::Mods;
        let (was, to) = (Mods::parse(was)?, Mods::parse(to)?);
        if was == to && *self.mods == to {
            return Ok(());
        }
        let first = self.stock.first().ok_or("no stock bike file to build the modifiers on")?;
        for (seat, sim) in self.rides.sims.iter_mut().enumerate() {
            to.resim(&was, 0, self.stock.get(seat).unwrap_or(first), sim);
            // A plane in flight retuned where it is, as the pause does offline (P98, `replane`).
            if let (Some(file), Some(f)) = (self.stock_plane, sim.plane.as_mut()) {
                to.replane(0, file, f);
            }
        }
        // And the plane a switch takes from here built on the new set.
        if let (Some(file), Some(t)) = (self.stock_plane, self.rides.plane.as_deref_mut()) {
            *t = plane_on(file, &to, t);
        }
        *self.mods = to;
        self.rides.time_scale = to.time_scale();
        Ok(())
    }
}
