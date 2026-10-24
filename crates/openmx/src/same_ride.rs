//! N8: a one-seat ride online is the offline ride, tick for tick.
//!
//! N6 proved the two hand every tick the same `dt`. This proves the rest: the same pads into the
//! same start give the same [`Sim`] after every tick, whichever way it was stepped.
//!
//! - **Offline** is the shell's own: [`World`] and its [`Seat`]s under the engine's fixed update,
//!   `Engine::step` on pace's clock after a load ([`pace::after_load`], [`pace::tick_clock`]) with
//!   the engine's input made by [`engine_input`], exactly as `ride`'s loop calls it; then the
//!   body's own frame, which the loop runs after the step with the hands the pass kept.
//! - **Online** is the session's: a [`Netplay`] of one seat driving [`Rides`], a frame a tick, with
//!   the body's frame in the `after` hook as `online.rs` has it.
//!
//! The pads each online tick ran on are logged by the session's game ([`Logged`]), and the offline
//! ride is handed those, tick for tick: the session holds a pad back [`net::DELAY`] ticks (the
//! first ticks run on an idle pad), which is the one thing between "the pad this frame" and "the
//! pad this tick", and it is asserted as such. After every tick both seats' whole states are
//! compared in [`game_sim::snapshot`]'s bytes, and their hashes.
//!
//! The world is a flat floor with the stand-in bike ([`Sim::new`]), not a track off the disc: the
//! two paths hand the one world to the one [`Sim::step`], so what a test can tell apart here is
//! the plumbing around it — the clock, the pad's way in, the poll, the body's frame, the order.

use std::path::PathBuf;

use game_render::vehicle::params as vehicle_params;
use game_sim::input::{Control, PADS, Report, mask_of};
use game_sim::motion::{Clip, ClipId};
use game_sim::ride::Controls;
use game_sim::rider::{self, Gate};
use game_sim::tick::Engine;
use game_sim::vehicle_anim::Library;
use game_sim::world::Flat;
use game_sim::Sim;
use ps2_float::F;

use super::{Seat, World, engine_input, pace};
use crate::net::rides::{self, Rides};
use crate::net::{self, Game, Lineup, Netplay, Timing};
use crate::vehicle::{Garage, Kind};

/// How long the ride is: a minute of the 30 Hz clock, well past the `dt` ramp's end.
const TICKS: usize = 1800;

/// A clip table with every state bound, each to a one-second clip, so the body's machine has a
/// clip to play in whatever state the ride puts it in (the renderer's table is the disc's).
fn clips() -> (Library, impl Fn(ClipId) -> Clip) {
    let mut lib = Library::default();
    for (s, slot) in lib.clip.iter_mut().enumerate() {
        *slot = Some(ClipId(0x100 + s as u32));
    }
    (lib, |id: ClipId| Clip::new(F(1.0), id.0 % 3 != 0))
}

/// The bike both rides start on, as the shell builds a seat's: the body's step rates set.
fn start() -> Sim {
    let mut sim = Sim::new(7);
    sim.rider.body_rates = vehicle_params();
    sim
}

/// The pad at frame `f`: a ride that goes, steers both ways at every stick byte, brakes, throws the
/// trick buttons and the shoulders, and lets go — every button but Start and Select (Start is the
/// shell's freeze offline and nothing online: `Not verified` in the journal). Bytes past the wire's
/// twenty, which the session does not carry, are junk here offline: the game never reads them.
fn pad(f: usize) -> Report {
    // A small generator of the frame alone, so any frame's pad is the same without the ones before.
    let mut x = (f as u32 / 9).wrapping_mul(0x9e37_79b9) ^ 0x5bd1_e995;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^= x >> 12;
    let leg = (f / 150) % 6;
    let (gas, brake) = (Control::Cross.mask(), Control::Square.mask());
    let not_start = !mask_of(&[Control::Start, Control::Select]);
    let mut buttons: u16 = match leg {
        0 | 1 | 3 => gas,
        // and the trick buttons, the d-pad and the shoulders on top
        2 => gas | (x as u16 & not_start),
        4 => brake,
        _ => x as u16 & not_start, // anything at all
    };
    if f % 97 < 4 {
        buttons = 0; // a moment off the gas
    }
    let mut r = Report::from_buttons(buttons);
    for (i, p) in r.0[8..20].iter_mut().enumerate() {
        *p = (x >> (i % 4 * 8)) as u8 | 0x20;
    }
    r.0[14] = (x >> 8) as u8 | 0x40; // Cross's pressure
    r.set_axis(2, (f * 7 % 256) as u8); // the bars, every byte from full left to full right
    r.set_axis(3, if leg == 2 { (x >> 16) as u8 } else { 0x80 });
    r.set_axis(0, (x >> 24) as u8);
    r.set_axis(1, (x >> 4) as u8);
    for b in &mut r.0[net::WIRE..] {
        *b = 0xa5;
    }
    r
}

/// The session's game, logging the pads each tick ran on.
struct Logged<'a> {
    rides: Rides<'a>,
    ran: &'a mut Vec<(usize, Report)>,
}

impl Game for Logged<'_> {
    type State = rides::State;

    fn save(&self) -> rides::State {
        self.rides.save()
    }

    fn restore(&mut self, s: &rides::State) {
        self.rides.restore(s)
    }

    fn step(&mut self, tick: usize, pads: &[Report]) {
        self.ran.push((tick, pads[0]));
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
}

/// The online ride: one seat, this machine's, `ticks` frames of the session. Each tick's pad and
/// the seat's state after it, in the codec's bytes.
fn online(ticks: usize, pads: impl Fn(usize) -> Report) -> (Vec<Report>, Vec<Vec<u8>>) {
    let dumps: PathBuf = std::env::temp_dir().join(format!("openmx-same-ride-{}", std::process::id()));
    let mut np: Netplay<rides::State> = Netplay::new(Lineup::parse("me").unwrap(), "me", Timing::default(), dumps).unwrap();
    let mut sims = vec![start()];
    let mut world = Flat::default();
    let (lib, clip) = clips();
    let (mut ran, mut after_each) = (Vec::new(), Vec::new());
    for f in 0..ticks {
        let before = ran.len();
        // `online.rs`'s hook: the body's frame inside the tick, so a rollback rolls it back too.
        let mut after = |_: usize, dt: F, sim: &mut Sim, c: &Controls| {
            sim.rider.body_frame(dt, c, Gate::default(), &lib, &clip);
        };
        let mut game = Logged { rides: Rides { sims: &mut sims, world: &mut world, after: Some(&mut after), time_scale: None, plane: None }, ran: &mut ran };
        np.frame(&mut game, &[pads(f)], pace::TICK.as_secs_f32());
        np.outgoing();
        assert_eq!(np.end, None, "frame {f}: the session ended");
        assert_eq!(ran.len(), before + 1, "frame {f}: a seat alone runs one tick a frame, no re-runs");
        assert_eq!(ran[before].0, f, "frame {f}: the session's tick");
        after_each.push(game_sim::snapshot::encode(&sims[0]));
    }
    (ran.into_iter().map(|(_, p)| p).collect(), after_each)
}

/// The offline ride: the shell's world of one seat (no front end, so Start would freeze it) under
/// the engine, on `pads[t]` at tick `t`. Each tick's `dt` and the seat's state after it.
fn offline(pads: &[Report]) -> (Vec<F>, Vec<Vec<u8>>) {
    offline_marked(pads, true)
}

/// [`offline`] with the seat's Markers switch (P129).
fn offline_marked(pads: &[Report], markers: bool) -> (Vec<F>, Vec<Vec<u8>>) {
    let mut engine = Engine::new();
    pace::after_load(&mut engine);
    let seat = Seat {
        reports: [Report::missing(); PADS],
        sim: start(),
        garage: Garage::new(Kind::Bike, None).unwrap(),
        riding: true,
        start_freezes: true,
        waiting: false,
        said: rider::Frame::default(),
        inverted: false,
        hands: Controls::default(),
        teleported: false,
        clears: 0,
        markers,
        // The online side's byte leaves the together chord off (P67), so this does too.
        together: false,
    };
    let mut world = World { place: Flat::default(), seats: vec![seat], tape: None, played_dt: None };
    let (lib, clip) = clips();
    let (mut dts, mut after_each) = (Vec::new(), Vec::new());
    for (t, pad) in pads.iter().enumerate() {
        // The loop's order: this frame's reports into the seat, then the engine's frame.
        world.seats[0].reports = [*pad, Report::missing()];
        let step = engine.step(pace::tick_clock(t as u64), engine_input(&world.seats[0].reports[0]), &mut world);
        assert!(step.simulated, "tick {t}: the engine skipped it");
        let dt = world.played_dt.unwrap_or(step.dt);
        // The body's frame after the step, as the loop poses a riding seat (`Body::ride`'s first half).
        let seat = &mut world.seats[0];
        assert!(seat.riding, "tick {t}: the ride froze");
        seat.sim.rider.body_frame(dt, &seat.hands, Gate::default(), &lib, &clip);
        dts.push(dt);
        after_each.push(game_sim::snapshot::encode(&seat.sim));
    }
    (dts, after_each)
}

#[test]
fn one_seat_online_is_the_offline_ride() {
    let (ran, online) = online(TICKS, pad);
    // What a tick runs on is the frame's pad `DELAY` ticks earlier, carried in its wire bytes; the
    // ticks before that have the session's idle pad.
    for (t, p) in ran.iter().enumerate() {
        let want = match t.checked_sub(net::DELAY) {
            Some(f) => net::unwire(net::wire(&pad(f))),
            None => Report::idle(),
        };
        assert_eq!(p.0, want.0, "tick {t}: the session ran on another pad");
    }
    // Offline, the same pads at the same ticks, with the junk past the wire's bytes put back.
    let pads: Vec<Report> = (0..TICKS)
        .map(|t| match t.checked_sub(net::DELAY) {
            Some(f) => pad(f),
            None => Report::idle(),
        })
        .collect();
    let (dts, offline) = offline(&pads);

    let (mut moved, mut farthest) = (0, 0.0f32);
    let at = |bytes: &[u8]| {
        let mut sim = start();
        game_sim::snapshot::decode(&mut sim, bytes).unwrap();
        game_render::chassis::placed(&sim.rider.ride).0[3]
    };
    let from = at(&online[0]);
    for t in 0..TICKS {
        assert_eq!(dts[t].0.to_bits(), rides::tick_dt(t).0.to_bits(), "tick {t}: dt");
        if online[t] != offline[t] {
            let at = online[t].iter().zip(&offline[t]).position(|(a, b)| a != b);
            panic!("tick {t}: the states part (online {} bytes, offline {}, first differing byte {at:?})", online[t].len(), offline[t].len());
        }
        moved += usize::from(t > 0 && online[t] != online[t - 1]);
        let now = at(&online[t]);
        farthest = farthest.max(((now[0] - from[0]).powi(2) + (now[2] - from[2]).powi(2)).sqrt());
    }
    // And the ride was one: the state moved every tick and the bike went somewhere.
    assert_eq!(moved, TICKS - 1, "ticks where nothing changed");
    assert!(farthest > 100.0, "the farthest the bike got from the start: {farthest} ft");
    println!("{TICKS} ticks: online and offline the same state after every one; the bike got {farthest:.0} ft out");
}

/// The proof can fail: one tick's pad changed offline only, and the states part on that tick.
#[test]
fn a_pad_off_by_a_tick_parts_them() {
    let (_, online) = online(300, pad);
    let mut pads: Vec<Report> = (0..300usize).map(|t| t.checked_sub(net::DELAY).map_or(Report::idle(), pad)).collect();
    pads[200] = Report::from_buttons(Control::Square.mask());
    let (_, offline) = offline(&pads);
    let parted = (0..300).position(|t| online[t] != offline[t]);
    assert_eq!(parted, Some(200));
}

/// P129: a seat with its Markers switch off rides online as offline too. Its pad carries the
/// switch's bit ([`crate::vehicle::Wanted::dpad_rides`]), and every tick's state is the offline
/// seat's with the switch off — the d-pad presses of the pad's wilder legs riding as the original's
/// — and not the ride with markers on, which parts from it.
#[test]
fn markers_off_online_is_the_offline_ride() {
    let off = |f: usize| {
        let mut r = pad(f);
        r.0[net::SWITCH] = crate::vehicle::Wanted { dpad_rides: true, ..Default::default() }.byte();
        r
    };
    let (_, online_off) = online(TICKS, off);
    let (_, online_on) = online(TICKS, pad);
    let pads: Vec<Report> = (0..TICKS).map(|t| t.checked_sub(net::DELAY).map_or(Report::idle(), off)).collect();
    let (_, offline_off) = offline_marked(&pads, false);
    for t in 0..TICKS {
        assert!(online_off[t] == offline_off[t], "tick {t}: markers off, online and offline part");
    }
    assert!((0..TICKS).any(|t| online_off[t] != online_on[t]), "the switch changed nothing online");
}
