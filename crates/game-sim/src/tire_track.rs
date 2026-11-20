//! **The tyre tracks' rows on the game's generator** (S448): which ticks each tyre track lays a row,
//! and the one `rand()` each of its updates makes for the row's height.
//!
//! The marks are presentation, but their updates are not, in one thing: every update of a tyre track
//! that hands its strip a row draws once on the shared generator (the row's height jitter, `Height +
//! rand()·2⁻³¹·0.005`), whether or not the strip then lays it. The original runs those updates in
//! its **render pass**, after the whole sim half of the frame (S328d), so the frame's last draws are
//! the tracks'. The shell's marks (`openmx/src/tire_tracks.rs`) run on dice of their own; the count,
//! and the rows, live here so the Sim's generator can hold the original's.
//!
//! # The Sim's run
//!
//! [`Tracks`] is a list of tracks per player, given to a Sim that races ([`crate::Sim::tracks`]);
//! without it nothing here draws, as before.
//!
//! - **The manager's pass** (in each rider's frame, beside its emitters, [`RiderTracks::switch`]):
//!   on the bike's snapshot of the frame before (the bike takes it after the manager has run, as
//!   [`crate::dust`]'s), a tyre whose zone byte changed switches each of its tracks into the zone
//!   (`None` in the track's descriptor holds it off and ends its strip), then each track is switched
//!   on or off by its `Disable` mask against the tyre's flags and the wreck's. Off ends the strip,
//!   leaves no fade-out and all of the fade-in to go.
//! - **The render walk** (after the sim half, [`Tracks::render`]): every player from the last to the
//!   first, each live track not held off by its zone in the manager's order, runs its update on this
//!   frame's snapshot ([`Track::update`]): the fades, the strength, and a call to the strip
//!   ([`Strip::add`]) above 0.005, which draws. At or below it the strip ends.
//!
//! # A track's update (`vehicle_fx_track_update`)
//!
//! `d` is how far the tyre's point moved since the last update (none while the strip is ended). The
//! tyre *marks* while it spins or is compressed past `MinimumCompressionThreshold`, and the bike is
//! faster than `MinimumSpeedThreshold`:
//!
//! - **Marking:** `f = (FadeIn − in) / FadeIn`, `in = max(in − d, 0)`, `out = f · FadeOut`.
//! - **Not:** `f = out / FadeOut`, `out = max(out − d, 0)`, scaled by `speed / MinSpeed` below it
//!   when `MinSpeed > 1`, `in = f · FadeIn`.
//! - **Strength:** `a = f · (Base + clamp01((c − MinC) / (MaxC − MinC)) · (1 − Base))`.
//!
//! A track switched off (all of the fade-in to go, its strip ended, so `d` is 0) lays nothing while it
//! marks; its first live frame that does not mark sets `in = 0`, and the next marking frame lays at
//! full strength (R49, `tests/tire_track_fade.rs` against the original). After a jump or a wreck that
//! is the first frame back on with the tyre at or under its threshold, or the bike under its speed.
//!
//! # A strip's row (`track_strip_add`)
//!
//! The alpha byte is `a · 128`, truncated. An ended strip starts again at the tyre when the byte is
//! not 0. Then, every call, the draw; a tyre still on the segment's anchor lays nothing more. `u`
//! grows by half the distance from the last row laid; a byte of 0 lays nothing; nor does a tyre less
//! than 2 ft from the last row while `u + 1 < 8`. A row's two edges are the tyre's point raised
//! `h` along the ground's normal, `Width` either side of the line back to the anchor. The first row of
//! a strip starts its first segment; a later one carries on, unless the line from the segment's start
//! has turned past `BreakAngle` from the segment's direction (the game's arc-cosine table) or
//! `u + 1 > 8`, when it starts a new one: `u = −8`, a restart row at the same point.
//!
//! The rows go into the original's ring of batches, which is the picture's business: [`Track::rows`]
//! hands each frame's rows out, and the shell lays them as it likes.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air::AcosTable;
use crate::dust::{GROUND, NO_CONTACT, NOT_WRECKING, OBJECT, WRECKING};
use crate::rng::Rng;
use ps2_float::vu::{self, Dest};
use ps2_float::{F, Vf};

/// A zone group the descriptor names `None`: a tyre on it lays no track.
pub const ZONE_NONE: u8 = 0xff;

/// The strip's empty direction and the vector a row's line is tested against: `(0, 0, 0, 1)`, word
/// for word.
const UNSET: Vf = Vf([0.0, 0.0, 0.0, 1.0]);

/// The strength at or below which a track ends its strip, and the row height's jitter scale.
const FAINT: F = F(0.005);

/// The distance from the last row that lays the next one.
pub const LAY_DISTANCE: F = F(2.0);

/// The degrees-to-radians factor the track's reader turns `BreakAngle` with.
const RADIANS: F = F(0.017_453_29);

/// The generator's draw as a fraction: 2⁻³¹.
const RAND_SCALE: F = F(f32::from_bits(0x3000_0000));

/// What one tyre track is: its file's numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// `AttachedToTire`: 0 front, 1 rear.
    pub tyre: usize,
    /// `Disable`, the manager's frame flags that switch it off.
    pub disable: u8,
    /// `ZoneDescriptor`'s sixteen zones, [`ZONE_NONE`] for a zone it lays nothing on.
    pub zones: [u8; 16],
    /// `Width`, half the strip's.
    pub width: F,
    /// `BreakAngle`, in radians ([`Params::break_angle`]).
    pub break_angle: F,
    pub min_speed: F,
    pub min_compression: F,
    pub max_compression: F,
    pub base_alpha: F,
    pub fade_in: F,
    pub fade_out: F,
    /// `HeightAboveGround`, the strip's.
    pub height: F,
    /// The strip's segment length in `u` (8) and `u`'s scale (1).
    pub segment: F,
    pub u_scale: F,
}

impl Params {
    /// `BreakAngle` as the reader keeps it: the file's degrees times the factor.
    pub fn break_angle(degrees: F) -> F {
        degrees * RADIANS
    }
}

/// A tyre as the bike's snapshot holds it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tyre {
    /// Where the suspension line crosses the contact's plane (the wheel's load point), and the
    /// ground's normal there.
    pub at: Vf,
    pub normal: Vf,
    /// The ground ray found the ground; the collision sweep put a contact on it.
    pub ground: bool,
    pub object: bool,
    pub compression: F,
    pub spinning: bool,
    /// The surface under it: the zone byte.
    pub zone: u8,
}

impl Tyre {
    /// The tyre's flags with the frame's wreck flag, as the manager builds them.
    fn flags(&self, wrecking: bool) -> u8 {
        let none = if !self.ground && !self.object { NO_CONTACT } else { 0 };
        let ground = if self.ground { GROUND } else { 0 };
        let object = if self.object { OBJECT } else { 0 };
        let wreck = if wrecking { WRECKING } else { NOT_WRECKING };
        wreck | ground | object | none
    }
}

/// The bike's snapshot as the tracks read it: the tyres (front, rear), the speed and the wreck.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub tyres: [Tyre; 2],
    pub speed: F,
    pub wrecking: bool,
}

/// A row handed to the ring: its two edges, the texture's `u`, the alpha byte it was laid with and
/// whether it starts a segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    pub edges: [Vf; 2],
    pub u: F,
    pub alpha: u8,
    pub restart: bool,
}

/// The strip one track lays, the words its row decision keeps.
#[derive(Clone, Debug, PartialEq)]
pub struct Strip {
    /// Ended: the next row it is handed starts it again.
    pub ended: bool,
    /// Points in the segments laid since the strip began, two a row; 0 until its first row.
    pub count: i32,
    /// Along the texture.
    pub u: F,
    /// The segment's anchor, the line a row's sides are square to, and the normal it was laid on.
    pub anchor: Vf,
    pub anchor_normal: Vf,
    /// The tyre's point and normal at the last call.
    pub at: Vf,
    pub normal: Vf,
    /// The last row's edges.
    pub edges: [Vf; 2],
    /// The segment's direction, [`UNSET`] until it has one, and its first row.
    pub dir: Vf,
    pub start: Vf,
    /// Where the last row was laid.
    pub last_lay: Vf,
    /// The alpha byte the rows carry.
    pub alpha: u8,
}

impl Default for Strip {
    /// A strip as its constructor leaves it.
    fn default() -> Strip {
        Strip {
            ended: true,
            count: 0,
            u: F::ZERO,
            anchor: Vf::ZERO,
            anchor_normal: Vf::ZERO,
            at: Vf::ZERO,
            normal: Vf::ZERO,
            edges: [Vf::ZERO; 2],
            dir: UNSET,
            start: Vf::ZERO,
            last_lay: UNSET,
            alpha: 0x80,
        }
    }
}

/// The length of `v`'s `xyz` (`mula.s`, `madda.s`, `madd.s`, `sqrt.s`).
fn length(v: Vf) -> F {
    let (x, y, z) = (F(v.x()), F(v.y()), F(v.z()));
    F::dot([x, y, z], [x, y, z]).sqrt()
}

/// `v`'s `xyz` made unit on the VU (`vmul.xyz`, `vadday.x`, `vmaddz.x`, `vrsqrt`, `vmulq.xyz`), `w` kept.
fn unit(v: Vf) -> Vf {
    let s = v.mul(v);
    let q = vu::rsqrt(1.0, vu::madd(vu::add(s.x(), s.y()), 1.0, s.z()));
    v.merge(Dest::XYZ, v.mul_q(q))
}

/// `c` clamped to −1..1 as the code does it: above 1 is 1, below −1 is −1.
fn clamp_unit(c: F) -> F {
    if c <= F(1.0) {
        if c < F(-1.0) { F(-1.0) } else { c }
    } else {
        F(1.0)
    }
}

impl Strip {
    /// Stop laying: a strip with rows forgets its count and `u`.
    pub fn end(&mut self) {
        if !self.ended {
            if self.count > 0 {
                self.count = 0;
                self.u = F::ZERO;
            }
            self.ended = true;
        }
    }

    /// **A call with the tyre at `at`** on ground facing `normal`, `alpha` strong (the module header):
    /// the draw on `rng`, then the rows it lays, pushed onto `rows`.
    #[allow(clippy::too_many_arguments)]
    pub fn add(&mut self, p: &Params, at: Vf, normal: Vf, alpha: F, acos: &AcosTable, rng: &mut Rng, rows: &mut Vec<Row>) {
        let a8 = (F(128.0) * alpha.min(F(1.0)).max(F::ZERO)).to_i32() as u8;
        if self.ended && a8 > 0 {
            self.anchor = at;
            self.start = at;
            self.anchor_normal = normal;
            self.ended = false;
        }
        self.at = at;
        self.normal = normal;
        let d = length(at.sub(self.last_lay));
        let h = p.height + FAINT * (RAND_SCALE * F::from(rng.rand()));
        let back = self.anchor.merge(Dest::XYZ, self.anchor.sub(at));
        if back.to_bits() == UNSET.to_bits() {
            return;
        }
        self.u = self.u + p.u_scale * (d / LAY_DISTANCE);
        if a8 == 0 || (d < LAY_DISTANCE && self.u + p.u_scale < p.segment) {
            return;
        }
        self.last_lay = at;
        // The row's sides: square to the line back to the anchor, in the ground's plane.
        let line = unit(back);
        let side = line.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(line, normal), normal, line));
        let up = normal.merge(Dest::XYZ, normal.mul(Vf::splat(h.0)));
        let width = Vf::splat(p.width.0);
        let left = Vf([(-F(side.x())).0, (-F(side.y())).0, (-F(side.z())).0, side.w()]);
        let left = left.merge(Dest::XYZ, left.mul(width));
        let left = left.merge(Dest::XYZ, left.add(up));
        let right = side.merge(Dest::XYZ, side.mul(width));
        let right = right.merge(Dest::XYZ, right.add(up));
        self.edges = [left.merge(Dest::XYZ, left.add(at)), right.merge(Dest::XYZ, right.add(at))];
        if self.count == 0 {
            rows.push(self.row(true));
            self.count += 2;
            self.anchor = at;
            self.start = at;
            self.dir = UNSET;
            self.alpha = a8;
            return;
        }
        rows.push(self.row(false));
        if self.dir.to_bits() == UNSET.to_bits() {
            self.dir = unit(at.merge(Dest::XYZ, at.sub(self.start)));
        }
        let along = unit(at.merge(Dest::XYZ, at.sub(self.start)));
        let dot = F::dot([F(along.x()), F(along.y()), F(along.z())], [F(self.dir.x()), F(self.dir.y()), F(self.dir.z())]);
        let angle = acos.at(clamp_unit(clamp_unit(dot)));
        if angle <= p.break_angle && self.u + p.u_scale <= p.segment {
            self.alpha = a8;
            return;
        }
        // A new segment, from this row.
        self.count += 2;
        self.u = -p.segment;
        self.dir = unit(at.merge(Dest::XYZ, at.sub(self.anchor)));
        self.start = at;
        self.anchor = at;
        self.anchor_normal = self.normal;
        rows.push(self.row(true));
        self.alpha = a8;
    }

    /// The row at the edges, as the ring is handed it.
    fn row(&self, restart: bool) -> Row {
        Row { edges: self.edges, u: self.u, alpha: self.alpha, restart }
    }
}

/// One tyre track and its strip.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub params: Params,
    /// Switched on by the manager (the entry's bit 0), and held off by its zone (bit 1).
    pub live: bool,
    pub zone_off: bool,
    /// Fade-out and fade-in distance still to go.
    pub out_left: F,
    pub in_left: F,
    /// Its tyre's point at the last update.
    pub last: Vf,
    pub strip: Strip,
    /// The draws its last render update made (0 or 1).
    pub draws: u8,
    /// The rows its last render update laid.
    pub rows: Vec<Row>,
}

impl Track {
    /// A track as its constructor leaves it, live, its strip ended.
    pub fn new(params: Params) -> Track {
        Track {
            params,
            live: true,
            zone_off: false,
            out_left: F(5.0),
            in_left: F::ZERO,
            last: Vf::ZERO,
            strip: Strip::default(),
            draws: 0,
            rows: Vec::new(),
        }
    }

    /// Off: the strip ended, nothing faded out left, all of the fade-in to go.
    fn off(&mut self) {
        self.strip.end();
        self.out_left = F::ZERO;
        self.in_left = self.params.fade_in;
    }

    /// The manager's zone change on its tyre: a zone the descriptor names `None` holds it off. A zone
    /// past the sixteen reads the entry's next bytes, never 0xff (as [`crate::dust`]'s).
    fn zone(&mut self, zone: u8) {
        if self.params.zones.get(usize::from(zone)).is_some_and(|&z| z == ZONE_NONE) {
            self.zone_off = true;
            self.off();
        } else {
            self.zone_off = false;
        }
    }

    /// **The render's update** (the module header) on this frame's `snap`: the draws on `rng`.
    pub fn update(&mut self, snap: &Snapshot, acos: &AcosTable, rng: &mut Rng) {
        self.rows.clear();
        let p = self.params;
        let tyre = &snap.tyres[p.tyre.min(1)];
        if self.strip.ended {
            self.last = tyre.at;
        }
        let marking = (tyre.spinning || !(tyre.compression <= p.min_compression)) && !(snap.speed <= p.min_speed);
        let f = if marking {
            let f = (p.fade_in - self.in_left) / p.fade_in;
            let d = length(tyre.at.sub(self.last));
            self.in_left = (self.in_left - d).max(F::ZERO);
            self.out_left = f * p.fade_out;
            f
        } else {
            let f = self.out_left / p.fade_out;
            let d = length(tyre.at.sub(self.last));
            self.out_left = (self.out_left - d).max(F::ZERO);
            if !(p.min_speed <= F(1.0)) && snap.speed < p.min_speed {
                self.out_left = self.out_left * (snap.speed / p.min_speed);
            }
            self.in_left = f * p.fade_in;
            f
        };
        self.last = tyre.at;
        let t = F::ZERO.max(F(1.0).min((tyre.compression - p.min_compression) / (p.max_compression - p.min_compression)));
        let alpha = f * (p.base_alpha + t * (F(1.0) - p.base_alpha));
        let before = rng.state();
        if alpha <= FAINT {
            self.strip.end();
        } else {
            self.strip.add(&p, tyre.at, tyre.normal, alpha, acos, rng, &mut self.rows);
        }
        self.draws = u8::from(rng.state() != before);
    }
}

/// A rider's tyre tracks, in the manager's order, the snapshot they last read and the manager's
/// zone byte for each tyre.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RiderTracks {
    pub tracks: Vec<Track>,
    /// The bike's snapshot: the manager's pass reads the frame before's, the render this frame's.
    pub snap: Snapshot,
    /// The zone each tyre was last seen on, by the manager.
    pub zones: [u8; 2],
}

impl RiderTracks {
    /// **The manager's pass** (the module header) on the last snapshot, then this frame's, `now`, kept.
    pub fn switch(&mut self, now: Snapshot) {
        let snap = std::mem::replace(&mut self.snap, now);
        for (t, tyre) in snap.tyres.iter().enumerate() {
            if tyre.zone != self.zones[t] {
                self.zones[t] = tyre.zone;
                for k in self.tracks.iter_mut().filter(|k| k.params.tyre == t) {
                    k.zone(tyre.zone);
                }
            }
        }
        let wreck = if snap.wrecking { WRECKING } else { NOT_WRECKING };
        for k in &mut self.tracks {
            let flags = snap.tyres[k.params.tyre.min(1)].flags(snap.wrecking);
            if k.params.disable & flags == 0 && k.params.disable & wreck == 0 {
                k.live = true;
            } else if k.live {
                k.live = false;
                k.off();
            }
        }
    }
}

/// Every player's tracks, by player.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tracks {
    pub riders: Vec<RiderTracks>,
}

impl Tracks {
    /// **The render walk** (the module header): every player from the last, each live track out of
    /// a held-off zone updated on its rider's snapshot. Answers the draws, made on `rng`.
    pub fn render(&mut self, acos: &AcosTable, rng: &mut Rng) -> u32 {
        let mut draws = 0;
        for r in self.riders.iter_mut().rev() {
            let snap = r.snap;
            for k in &mut r.tracks {
                k.draws = 0;
                k.rows.clear();
                if k.live && !k.zone_off {
                    k.update(&snap, acos, rng);
                    draws += u32::from(k.draws);
                }
            }
        }
        draws
    }
}

impl crate::Sim {
    /// Player `i`'s bike snapshot as the tracks read it.
    fn track_snapshot(&self, i: usize) -> Snapshot {
        let rider = self.rider_of(i);
        let r = &rider.ride;
        let tyre = |w: &crate::ride::Wheel| Tyre {
            at: w.w.load_point,
            normal: w.w.contact_normal,
            ground: w.w.ray_hit != 0,
            object: w.w.swept_contact != 0,
            compression: w.w.compression,
            spinning: w.slip.spinning,
            zone: w.surface,
        };
        Snapshot { tyres: [tyre(&r.front), tyre(&r.rear)], speed: r.chassis.speed, wrecking: rider.crash.is_some() }
    }

    /// **Player `i`'s tracks in the manager's pass** (S448), beside its emitters: switched on the last
    /// snapshot, then this frame's kept. A Sim with no tracks does nothing here.
    pub(crate) fn tracks_switch(&mut self, i: usize) {
        if self.tracks.as_deref().is_none_or(|t| t.riders.len() <= i) {
            return;
        }
        let now = self.track_snapshot(i);
        if let Some(t) = self.tracks.as_deref_mut() {
            t.riders[i].switch(now);
        }
    }

    /// **The tracks' render walk** (S448), after the frame's sim half, on the game's generator — the
    /// race's, or the AI lane's when there is no race, as the dust's. A Sim with no tracks, or no
    /// generator, draws nothing here.
    pub(crate) fn tracks_render(&mut self) {
        let Some(tracks) = self.tracks.as_deref_mut() else { return };
        let rng: &mut Rng = match (self.ai.as_deref_mut(), self.race.as_deref_mut()) {
            (_, Some(race)) => &mut race.rng,
            (Some(lane), None) => &mut lane.rng,
            (None, None) => return,
        };
        tracks.render(&self.tables.ride.acos, rng);
    }
}
