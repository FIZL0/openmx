//! The ride online: one [`Sim`] a seat, stepped in seat order over one world.
//!
//! A seat is a whole [`Sim`] of its own — its own input manager, bike and rider. The bikes push each
//! other apart (S78, [`rider_contact::among`]): every seat's contacts are found from the tick's start
//! and each seat's step resolves its own, pushing the others' chassis, and meets the others as rivals,
//! whose head-on joust wrecks as S54's one Sim of N riders does (S224); a seat's plane has the
//! others' bikes in its world (S225). Split screen steps its seats the same (N22). Each reads its seat's pad as pad 0, the shape every one-player capture of
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
use ps2_float::{F, Vf};

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
    /// Every seat's cars as built and never driven (S249, [`crate::vehicle::Lot::online_cars`]), by
    /// seat: what a seat's switch to a car takes its copy from into its [`Sim::car`], and what that
    /// car is stepped with. A seat with none here stays where it is when it asks for a car.
    pub cars: &'a [Vec<vehicle::CarRide>],
}

/// **A flying seat's step, held at its plane's frame** (N22a1): its plane readied
/// ([`vehicle::ready_lane`]), and what [`fly_held`] needs to fly it with the others and finish it.
struct Held {
    seat: usize,
    /// Its place among the flown planes.
    k: Option<usize>,
    plane: vehicle::Plane,
    ready: vehicle::Readied,
    met: Option<game_sim::flight::Met>,
    bikes: Vec<game_sim::flight_search::Bike>,
    wanted: Wanted,
}

/// The held flying seats' steps finished (N22a1): their planes in one [`vehicle::fly_together`], in
/// the walk's order, then each seat's flight back in its lane, its markers, and its plane's pushes
/// on the bikes and the mirrors for their lists, as [`Rides::step`] has a lone plane's.
fn fly_held(
    sims: &mut [Sim],
    world: &mut dyn World,
    dt: F,
    spots: &[Spot],
    riding: &[bool],
    mut held: Vec<Held>,
    struck: &mut [Vec<Vec<game_sim::contact::Contact>>],
) {
    if let Some(first) = held.first() {
        let tables = sims[first.seat].tables.clone();
        let mut fleet: Vec<game_sim::flight::Fly<'_>> = held
            .iter_mut()
            .filter_map(|h| {
                let pad = h.ready.pad;
                Some(game_sim::flight::Fly { flight: h.plane.flight.as_mut()?, pad, met: h.met.as_mut(), bikes: &h.bikes })
            })
            .collect();
        vehicle::fly_together(dt, &mut fleet, world, &tables.ride);
    }
    for Held { seat, k, plane, ready, met, wanted, .. } in held {
        let sim = &mut sims[seat];
        vehicle::settle_lane(sim, plane, ready);
        if !wanted.dpad_rides {
            vehicle::lane_markers(sim, seat, spots, riding, world);
        }
        if let (Some(k), Some(m)) = (k, met) {
            rider_contact::pushed_among(sims, &m);
            struck[k] = m.struck;
        }
    }
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
        // (P88): the same on every machine, as every seat's state is. A flying seat's is its plane
        // as it flies (P141; the ground under it while it is on or near the ground, P107, as
        // offline P104's), not at the bike it left.
        let spots: Vec<Spot> = self.sims.iter().map(|s| vehicle::lane_spot(s, self.world)).collect();
        // Who is in the ride (P94): a seat nobody sits in has the missing pad ([`Game::step`]), and is
        // no marker's pick. The pads are the session's, so every machine counts the same.
        let riding: Vec<bool> = (0..self.sims.len()).map(|s| pads.get(s).is_some_and(|p| p.0[0] != Report::missing().0[0])).collect();
        // A seat whose player has their pause open (P84, in its byte) is frozen where it is: not
        // polled, not stepped, and out of every bike's contacts, so the others pass through it.
        let paused: Vec<bool> = (0..self.sims.len()).map(|s| pads.get(s).is_some_and(|p| Wanted::of(p.0[super::SWITCH]).paused)).collect();
        let touching: Vec<bool> = riding.iter().zip(&paused).map(|(&r, &p)| r && !p).collect();
        // Rider against rider (S78): every seat's contacts, from where the last tick left every bike.
        // On a world that wraps (MD44), each bike against every other's nearest copy.
        // A flying seat's plane is in it too (N22a): its hull struck by the bikes, the mirrors on its
        // list. The planes step first, as the original's dynamics world steps before its riders.
        let aloft: Vec<bool> = self.sims.iter().zip(&touching).map(|(s, &t)| t && s.plane.as_ref().is_some_and(|f| f.hull(0).is_some())).collect();
        let (mut lists, mut mirrors) = match rider_contact::among_met(self.sims, &touching, dt, &*self.world) {
            Some((l, m)) => (Some(l), m),
            None => (None, Vec::new()),
        };
        let flown: Vec<usize> = (0..self.sims.len()).filter(|&i| aloft[i]).collect();
        // The last seat first, as the original walks its riders (S88, `Sim::step_all`), after the
        // flying seats (N22a).
        let n = self.sims.len();
        let order: Vec<usize> = (0..n).rev().filter(|&i| aloft[i]).chain((0..n).rev().filter(|&i| !aloft[i])).collect();
        // What each flown plane's own rays put on each bike's list (N22b), by plane, then by seat:
        // ahead of the bike's own entries, as the original searches its planes first.
        let mut struck: Vec<Vec<Vec<game_sim::contact::Contact>>> = vec![Vec::new(); flown.len()];
        // The seats' one prop set for the tick (S197c): every seat's bike strikes it, and it steps
        // once after them all.
        let mut props = game_sim::prop_seats::Shared::take(&mut self.sims.iter_mut().collect::<Vec<_>>());
        // Two planes or more fly their frames together (N22a1), as the original's level node steps
        // its planes in one dynamics world: every flying seat's pad first, then one step of all
        // their planes, then each seat's markers, before the first seat on its bike.
        let together = flown.len() > 1;
        let mut held: Vec<Held> = Vec::new();
        for seat in order {
            if !aloft[seat] && !held.is_empty() {
                fly_held(self.sims, &mut *self.world, dt, &spots, &riding, std::mem::take(&mut held), &mut struck);
            }
            let at = self.sims[seat].rider.ride.transform().0[3];
            let firsts: Vec<Vec<game_sim::contact::Contact>> = struck.iter_mut().map(|s| s.get_mut(seat).map(std::mem::take).unwrap_or_default()).collect();
            let own = lists.as_mut().map(|l| std::mem::take(&mut l[seat]));
            let mut bump = own.map(|l| rider_contact::struck_first(&firsts, l, at, &*self.world)).filter(|l| !l.is_empty()).map(|l| {
                let mut b = rider_contact::bump_among_in(self.sims, seat, l, &*self.world);
                let from = self.sims[seat].rider.ride.transform().0[3];
                b.planes = flown.iter().filter_map(|&j| Some(self.sims[j].plane.as_ref()?.flown(j, from, &*self.world))).collect();
                b
            });
            // A flown plane meets the bikes both ways (N22a, N22b): the mirrors of their strikes on
            // its hull, and the bikes its own rays strike, which it pushes.
            let plane = flown.iter().position(|&j| j == seat);
            let mut met = plane.and_then(|k| {
                let m = mirrors.get_mut(k).map(std::mem::take).unwrap_or_default();
                let at = self.sims[seat].plane.as_ref()?.body.position;
                Some(rider_contact::met_plane_among(self.sims, seat, n + k, m, at, &*self.world))
            });
            if paused[seat] {
                continue;
            }
            // The other seats where they stand now, the ones after this one already stepped, as the
            // original's walk meets them (S88): the riders this one can joust (S224), and the bikes
            // in its plane's world if it flies (S225). A seat on the plane has left its bike.
            let on: Vec<(&game_sim::rider::Rider, bool)> =
                self.sims.iter().zip(&touching).map(|(s, &t)| (&s.rider, t && s.plane.is_none() && s.car.is_none())).collect();
            let rivals = rider_contact::rivals_among(&on, seat, &*self.world);
            let flying = self.sims[seat].plane.as_ref().map(|f| f.body.position);
            let bikes = flying.map(|at| rider_contact::plane_bikes(&on, seat, at, &*self.world)).unwrap_or_default();
            drop(on);
            let sim = &mut self.sims[seat];
            let mut reports = [Report::missing(); PADS];
            reports[0] = pads.get(seat).copied().unwrap_or(Report::missing());
            // The machine the seat's player wants (N17), switched to before the tick's step, as
            // the shell switches between frames offline (P102). A switch that cannot be had (no
            // plane, no ground) leaves the seat where it is, and is asked again next tick.
            let wanted = Wanted::of(reports[0].0[super::SWITCH]);
            let cars = self.cars.get(seat).map_or(&[][..], |c| &c[..]);
            if let Some(to) = wanted.to {
                let _ = vehicle::switch_lane_among(to, self.plane.as_deref(), cars, sim, self.world);
            }
            // The shell's hands, the d-pad the markers' and not the ride's (P93) — unless the seat's
            // Markers switch is off (P129, in its byte), and then the d-pad is the original's.
            let mut hands = vehicle::hands(sim.poll(dt, &reports), !wanted.dpad_rides);
            // And the new layout's reset chord, pressed together (P67), if the seat rides it.
            if wanted.together {
                hands = hands.chord_together(sim.input.pad(0), sim.table());
            }
            // On the plane the bike is not stepped, its markers do not run and its rider's body
            // does not move, as offline (`Seat::pass`); the plane's own markers do (P107).
            if together && aloft[seat]
                && let Some((readied, ready)) = vehicle::ready_lane(sim, dt, wanted, self.world)
            {
                held.push(Held { seat, k: plane, plane: readied, ready, met, bikes, wanted });
                continue;
            }
            // In a car (S249) the bike is not stepped either, nor are any markers: offline a car has
            // none yet (S149e).
            if sim.car.is_some() {
                vehicle::drive_lane(sim, cars, dt, self.world);
                continue;
            }
            if sim.plane.is_some() {
                vehicle::fly_lane_shared(sim, dt, wanted, self.world, &bikes, met.as_mut(), props.as_mut().map(|p| (p, seat)));
                if !wanted.dpad_rides {
                    vehicle::lane_markers(sim, seat, &spots, &riding, self.world);
                }
                if let (Some(k), Some(m)) = (plane, met) {
                    rider_contact::pushed_among(self.sims, &m);
                    struck[k] = m.struck;
                }
                continue;
            }
            match props.as_mut() {
                Some(p) => sim.step_shared(dt, &hands, self.world, bump.as_mut(), &rivals, p, seat),
                None => sim.step_among(dt, &hands, self.world, bump.as_mut(), &rivals),
            };
            if !wanted.dpad_rides {
                markers::frame_among(sim, seat, &spots, &riding, &hands);
            }
            if let Some(after) = &mut self.after {
                after(seat, dt, sim, &hands);
            }
            if let Some(b) = &bump {
                rider_contact::bumped_among(self.sims, b);
                for f in &b.planes {
                    if let Some(p) = self.sims[f.seat].plane.as_mut() {
                        p.pushed(f);
                    }
                }
            }
        }
        if !held.is_empty() {
            fly_held(self.sims, &mut *self.world, dt, &spots, &riding, held, &mut struck);
        }
        if let Some(p) = props {
            p.finish(&mut self.sims.iter_mut().collect::<Vec<_>>(), dt, self.world);
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
        // And a seat decoded in a car (S249) whose collision came without its ray probe, the car's
        // `m.col`, another constant the codec leaves out: any seat's copy's, the file being the car's.
        if let Some(lane) = sim.car.as_mut()
            && let Some(struck) = lane.struck.as_mut()
            && struck.rays.segments.is_empty()
            && let Some(rays) = self.cars.iter().flatten().find(|c| c.machine.car_physics() == Some(lane.kind)).and_then(|c| c.rays.as_ref())
        {
            struck.rays = Arc::new(rays.clone());
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
    spawn_flying_on(first, spawn, template, bay, None)
}

/// [`spawn_flying`] with the body's spot and up of a file bay (B6, [`vehicle::start_lane_on`]).
pub fn spawn_flying_on(
    first: &Sim,
    spawn: &[u8],
    template: &Flight,
    bay: vehicle::Spawn,
    body: Option<(Vf, Vf)>,
) -> Result<Vec<u8>, String> {
    let mut sim = first.clone();
    from_seat_bytes(&mut sim, spawn)?;
    vehicle::start_lane_on(template, bay, body, &mut sim);
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
            // Each seat on its own row (P134); a ride whose rows are all 1P's is as before.
            to.resim(&was, seat, self.stock.get(seat).unwrap_or(first), sim);
            // A plane in flight retuned where it is, as the pause does offline (P98, `replane`).
            if let (Some(file), Some(f)) = (self.stock_plane, sim.plane.as_mut()) {
                to.replane(seat, file, f);
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

// ---- every seat's own modifiers online (P134) ------------------------------------------------------

/// The per-seat sets an online ride rides, kept beside [`Mods`](game_sim::modifiers::Mods) so the
/// menu can read them too.
pub use game_sim::modifiers::online::{host_set, own_row, own_set, ride_set, with_own};
