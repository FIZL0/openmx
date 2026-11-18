//! **The bike's particle emitters' draws** (S328c): how many times each rider's emitters draw on the
//! game's one generator in a frame, and the Sim's own run of them.
//!
//! The particles are presentation, but their emitters are not, in one thing: every emitter that
//! places particles in a frame makes two `rand()`s on the shared generator first (the fraction of the
//! frame's rate it carries, and the pattern). The particles themselves draw on another generator. C33's
//! four racers show it: from tick 17 each moving rider's exhaust, rear-tyre dust and rear sprays make
//! 4-26 draws a tick, each rider's straight after its own AI's. The shell's particles
//! (`openmx/src/particles.rs`) run on dice of their own; the count lives here so the Sim's generator
//! can hold the original's.
//!
//! # The Sim's run
//!
//! [`Dust`] is a list of entries per player, given to a Sim that races ([`crate::Sim::dust`]); without
//! it nothing here draws, as before. In each rider's frame, after its brain has thought:
//!
//! - **The manager** reads the bike's snapshot, which the bike takes after the manager has run: what
//!   it reads is the frame before's. It sets the frame's flags — wrecked 8, else 0x10; each tyre's no contact 1, ground
//!   2, object 4 — and switches each entry on or off by its `Disable` mask against them. A live entry
//!   then runs its own update: the exhaust is on while `throttle · ½ · (MaxSpeed − speed) / MaxSpeed`
//!   is above 0, the generic is always on, and a spray is on while the rear wheel spins under throttle
//!   (or the side load passes its threshold) and for a quarter second after.
//! - **The emitter** takes its point: last frame's becomes the previous one and the new one is the
//!   bike's transform applied to the emitter's place on the bike ([`Emitter::at`]). An emitter switched
//!   on this frame only takes its point. One that is on and moved at least its `MinimumEmissionSpeed`
//!   draws twice.
//!
//! The emitter's place on the bike is the one thing here that is not the original's: its point is the
//! node it hangs from, under the swingarm or the fork, which the suspension moves. Here it is a fixed
//! place in the chassis' frame (`research/journal`, S328c).

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::math::{Mat4, transform_point};
use crate::rng::Rng;
use ps2_float::{F, Vf};

/// The manager's frame flags, as the entries' `Disable` masks name them.
pub const NO_CONTACT: u8 = 1;
pub const GROUND: u8 = 2;
pub const OBJECT: u8 = 4;
pub const WRECKING: u8 = 8;
pub const NOT_WRECKING: u8 = 0x10;

/// What a tyre tells the manager this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tyre {
    pub ground: bool,
    pub object: bool,
    pub spinning: bool,
    /// The side load per unit of load, the spray's sideways push.
    pub side: F,
}

impl Tyre {
    /// The tyre's flags: its contacts, and the frame's wreck flag.
    fn flags(&self, wrecking: bool) -> u8 {
        let ground = if self.ground { GROUND } else { 0 };
        let object = if self.object { OBJECT } else { 0 };
        let none = if !self.ground && !self.object { NO_CONTACT } else { 0 };
        let wreck = if wrecking { WRECKING } else { NOT_WRECKING };
        wreck | ground | object | none
    }
}

/// What a rider's manager reads this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Feed {
    pub wrecking: bool,
    pub throttle: F,
    pub clutch: F,
    pub airspeed: F,
    /// Front, then rear.
    pub tyres: [Tyre; 2],
}

/// An entry's own update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// On while the throttle pushes below `max_speed`.
    Exhaust { max_speed: F },
    /// On whenever it is live.
    Generic,
    /// The tyre's spray: on while its wheel spins under throttle or the side load passes `threshold`,
    /// and for `clock` seconds more.
    Spray { threshold: F, clock: F },
    /// An entry whose emitter does not draw on the shared generator here: the collision models, the
    /// rider's foot dust, the tyre tracks (the module header).
    Quiet,
}

/// An emitter's switch and its two points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emitter {
    /// Switched on (the emitter's on bit).
    pub on: bool,
    /// Switched on since its last point (the just-on bit): its next point only takes the place.
    pub just_on: bool,
    /// `MinimumEmissionSpeed`.
    pub min_speed: F,
    /// Its place in the chassis' frame.
    pub at: Vf,
    pub pos: Vf,
    pub prev: Vf,
}

impl Emitter {
    fn switch_on(&mut self) {
        if !self.on {
            self.on = true;
            self.just_on = true;
        }
    }

    fn switch_off(&mut self) {
        self.on = false;
        self.prev = self.pos;
    }

    /// The emitter's frame: its new point, then the draws it makes (0 or 2).
    fn frame(&mut self, bike: &Mat4, dt: F) -> u8 {
        self.prev = self.pos;
        self.pos = transform_point(bike, self.at);
        if self.just_on {
            self.just_on = false;
            self.prev = self.pos;
            return 0;
        }
        if !self.on {
            self.prev = self.pos;
            return 0;
        }
        let d = self.pos.sub(self.prev);
        let (x, y, z) = (F(d.x()), F(d.y()), F(d.z()));
        let speed = F::dot([x, y, z], [x, y, z]).sqrt() / dt;
        if speed < self.min_speed { 0 } else { 2 }
    }
}

/// One of the manager's entries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub kind: Kind,
    /// The tyre it reads, 0 front or 1 rear, if any.
    pub tyre: Option<usize>,
    /// The frame flags that switch it off.
    pub disable: u8,
    /// Live (the entry's bit 0).
    pub live: bool,
    /// Its tyre is in a zone with no particles (bit 1): live, but its emitter stays off.
    pub zone_off: bool,
    pub emitter: Emitter,
    /// The draws its emitter made last frame.
    pub draws: u8,
}

impl Entry {
    fn enable(&mut self) {
        self.live = true;
        if !self.zone_off {
            self.emitter.switch_on();
        }
    }

    fn disable(&mut self) {
        self.live = false;
        if let Kind::Spray { clock, .. } = &mut self.kind {
            *clock = F(0.0);
        }
        self.emitter.switch_off();
    }

    /// The entry's own update, while it is live.
    fn update(&mut self, feed: &Feed, dt: F) {
        match &mut self.kind {
            Kind::Exhaust { max_speed } => {
                let max = *max_speed;
                let f = feed.throttle * F(0.5) * (max - feed.airspeed) / max;
                if f <= F(0.0) { self.emitter.switch_off() } else { self.emitter.switch_on() }
            }
            Kind::Generic => self.emitter.switch_on(),
            Kind::Spray { threshold, clock } => {
                let tyre = feed.tyres[self.tyre.unwrap_or(1)];
                let idle = feed.throttle == F(0.0) || feed.clutch != F(0.0) || !tyre.spinning;
                let level = -*threshold <= tyre.side && tyre.side <= *threshold;
                if idle && level {
                    if F(0.0) < *clock {
                        *clock = *clock - dt;
                        if F(0.0) < *clock {
                            self.emitter.switch_on();
                            return;
                        }
                    }
                    *clock = F(0.0);
                    self.emitter.switch_off();
                } else {
                    *clock = F(0.25);
                    self.emitter.switch_on();
                }
            }
            Kind::Quiet => {}
        }
    }
}

/// A rider's entries, in the manager's order, and the snapshot of the bike they read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RiderDust {
    pub entries: Vec<Entry>,
    /// What the manager reads: the bike's snapshot, which the bike takes after the manager has run,
    /// so each frame's is the frame before's.
    pub feed: Feed,
}

impl RiderDust {
    /// **The rider's frame** (the module header): the manager's switches and updates on the last
    /// snapshot, then each emitter's point and draws, then this frame's snapshot, `now`. Answers the
    /// draws, which the caller makes on the game's generator.
    pub fn frame(&mut self, now: Feed, bike: &Mat4, dt: F) -> u32 {
        let feed = &std::mem::replace(&mut self.feed, now);
        let wreck = if feed.wrecking { WRECKING } else { NOT_WRECKING };
        let tyres = feed.tyres.map(|t| t.flags(feed.wrecking));
        for e in &mut self.entries {
            let tyre = e.tyre.map_or(0, |t| tyres[t]);
            if e.disable & tyre == 0 && e.disable & wreck == 0 {
                if !e.live {
                    e.enable();
                }
            } else if e.live {
                e.disable();
            }
            if e.live && !e.zone_off {
                e.update(feed, dt);
            }
        }
        let mut draws = 0;
        for e in &mut self.entries {
            e.draws = match e.kind {
                Kind::Quiet => 0,
                _ => e.emitter.frame(bike, dt),
            };
            draws += u32::from(e.draws);
        }
        draws
    }
}

/// Every player's entries, by player.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dust {
    pub riders: Vec<RiderDust>,
}

/// An emitter's place in the chassis' frame, from where it is in the world with the bike at `bike`:
/// what a seed from an image puts in [`Emitter::at`].
pub fn place_on(bike: &Mat4, world: Vf) -> Option<Vf> {
    crate::math::inverse(bike).map(|inv| transform_point(&inv, world))
}

impl crate::Sim {
    /// **Player `i`'s dust frame** (S328c, the module header), on the game's generator — the race's,
    /// or the AI lane's when there is no race (S387). A Sim with no dust, or no generator, draws nothing here.
    pub(crate) fn dust_frame(&mut self, i: usize, dt: F) {
        let Some(dust) = self.dust.as_deref() else { return };
        if dust.riders.len() <= i {
            return;
        }
        let rider = self.rider_of(i);
        let r = &rider.ride;
        let tyre = |w: &crate::ride::Wheel| Tyre {
            ground: w.w.ray_hit != 0,
            object: w.w.swept_contact != 0,
            spinning: w.slip.spinning,
            side: w.side_per_load,
        };
        let feed = Feed {
            wrecking: rider.crash.is_some(),
            throttle: r.held.gas,
            clutch: r.mv.clutch_in,
            airspeed: r.chassis.speed,
            tyres: [tyre(&r.front), tyre(&r.rear)],
        };
        let bike = r.transform();
        let Some(dust) = self.dust.as_deref_mut() else { return };
        let draws = dust.riders[i].frame(feed, &bike, dt);
        let rng: &mut Rng = match (self.ai.as_deref_mut(), self.race.as_deref_mut()) {
            // In a race, the race's: the lane's own is for a ride with no race (S387).
            (_, Some(race)) => &mut race.rng,
            (Some(lane), None) => &mut lane.rng,
            (None, None) => return,
        };
        for _ in 0..draws {
            rng.rand();
        }
    }
}
