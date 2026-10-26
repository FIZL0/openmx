//! **The bike that moves: the chassis skeleton posed from the vehicle's own animation machine, and
//! the five nodes the simulation drives instead of a clip.**
//!
//! Until this module the shell drew a statue. Every part of a `.sub` was placed at its chassis node's
//! *bind* transform, read once per frame off the model the archive shipped
//! (`chassis.model.world(i)`), so the forks never compressed although the ground step prints their
//! travel every frame, the wheels never turned although their angular velocity is right there, the
//! front end never steered with the stick and the swingarm never moved. And S25 — the sixty-state
//! animation machine — asked for the bike's clips every frame with nothing to consume the pose it
//! answered.
//!
//! [`Chassis`] is the other side of both. It is a [`crate::anim::Stance`] — one animation set with a
//! machine, a clip table and a clock beside it — and then the half a rider does not have.
//!
//! **Whose machine that is, is not the bike's.** V12 proved the sixty-state machine is the *rider's
//! body*: 22 joints, clips out of `Rider\Anim\`, all sixty slots bound. The 12 `bike_*` clips a
//! bike archive ships are the other machine's ([`crate::anim::Rig`]). V13 pointed the sixty-state
//! machine at the set it belongs to and left the bike stepping a stance of its own; what is left of
//! the re-pairing is in [`Chassis`]' own docs and in V13's journal.
//!
//! ## Two halves, because the original has two
//!
//! A frame of the original's chassis animation is the blend and then the drives, in that order, and
//! [`Chassis::frame`] is the same:
//!
//! ```text
//!   vehicle_anim::Driver::frame   the ramp, the restart, the latch, the two lags, the selection
//!   Set::rewind                   on a restart: the pose the cross-fade fades from
//!   Player::play / ::run          the clock: a clip put on, a dt advance, or a phase-driven sample
//!   Set::apply_vehicle            that blend's clips sampled on the chassis skeleton      -> a Pose
//!   Chassis::drive                five node locals written over the pose from the ride's numbers
//!   Skeleton::compose             the lot down the tree                            -> a matrix a joint
//! ```
//!
//! The drives are the second half and they are not a clip. **No `.mot` on this disc animates a
//! suspension, a steering head or a wheel's spin** — they could not: those are numbers the physics
//! solves, different every frame, and the original writes them straight onto the chassis' nodes after
//! the blend has had its say. Five nodes, in the order they have to be written (a parent before the
//! child that is re-expressed in its frame):
//!
//! | node | parent | what is written | from |
//! |---|---|---|---|
//! | `Handlebars` | `Bike` | the rotation, replaced by a turn about the fork axis | the front wheel's `steer_angle`, and only with a wheel on the ground |
//! | `Shocks` | `Handlebars` | the translation only | the front wheel's `compression`, up the fork axis |
//! | `TireFront` | `Shocks` | the rotation, replaced by a turn about its own `x` | the front wheel's rolled angle |
//! | `SwingArm` | `Bike` | the whole local, as a placement aimed at the rear axle | the rear wheel's `compression` and mount |
//! | `TireRear` | `SwingArm` | the rotation, replaced by a turn about its own `x` | the rear wheel's rolled angle |
//!
//! The rolled angle is the one number in the table the simulation does not keep. The wheel's own
//! record has the *rate* ([`game_sim::r#move::Wheel::spin`], `wheel+0x1d4`, which S16's integration
//! writes) and the original integrates it into an angle a word below (`+0x1d0`) that no ported
//! struct has a field for, because nothing in the simulation reads it — it exists to turn a tire on
//! screen. So [`Roll`] keeps it here, with the original's own wrap.
//!
//! ## The geometry is the model's, not a constant
//!
//! Three numbers are worked out once, when the set is loaded, out of the chassis skeleton's own bind
//! pose ([`Geometry`]): the fork axis, which is the line from the `Shocks` node up to the
//! `Handlebars` node; `inv_sin`, which turns a vertical compression into a distance along that
//! slanted line; and where `Shocks` and `SwingArm` sit in the `Bike` frame, which is what the drives
//! move them away from. A different bike with a different rake gets different numbers from the same
//! code, which is the point.

use crate::anim::{Set, Stance};
use crate::disc::Res;
use crate::mat::{M4, V3, v3};
use crate::psg::Model;
use crate::skin::{Skeleton, skin};
use crate::world::Loaded;
use game_sim::ride::Ride;
use game_sim::rider::Rider;
use game_sim::trick::{SLOTS, Stunt, Tricks};
use game_sim::vehicle_anim::{self as va, Params, Request};
use ps2_float::F;

/// The name every `.sub` on this disc places its unmoving parts at, and the chassis skeleton's root.
pub const ROOT: &str = "Bike";
/// The steering head: the `.sub`'s front fender, number plate and crossbar hang off this one.
pub const BARS: &str = "Handlebars";
/// The sliding half of the front fork.
pub const SHOCKS: &str = "Shocks";
/// The front wheel.
pub const FRONT: &str = "TireFront";
/// The swingarm.
pub const SWINGARM: &str = "SwingArm";
/// The rear wheel.
pub const REAR: &str = "TireRear";

/// The fixed nudge the original adds to the `Shocks` node after the compression, in the `Bike`
/// frame. It is a constant quad in the drive itself, `(-0.03, 0.0, 0.02, 1.0)`, and not a key of
/// any file on the disc.
const SHOCKS_NUDGE: V3 = v3(-0.03, 0.0, 0.02);

/// How far the drawn front tyre is kept from anything that hangs off the bars (V62), in feet. The
/// fender is placed at `Handlebars` and does not travel with the fork, so at a bottom-out the
/// ported slide would put the tyre up into it; [`Chassis::stop_fork_under`] stops the drawn fork
/// this far short. A little over the nudge's sideways reach, which a steered fork swings round.
const FORK_STOP_CLEAR: f32 = 0.04;
/// The most compression [`Chassis::stop_fork_under`] looks at: past any fork's travel.
const FORK_STOP_SEARCH: f32 = 2.0;

/// Two pi and one over it, as the original's angle wrap spells them.
const TAU: f32 = 6.2831855;
const INV_TAU: f32 = 0.15915494;

/// A wheel's rolled angle: the one piece of the drives' arithmetic the simulation has no field for.
///
/// `+0x1d4` on a wheel is how fast it is spinning, in radians per second, and S16's integration
/// writes it every substep. `+0x1d0` is how far it has turned, and the original's own integrate is
/// the three lines below: step by the rate, take whole turns off with a truncating multiply by
/// `1/2pi`, and then fold what is left into `-pi..pi` with one test each way. The second step alone
/// would do, but the original does both and the port keeps both: the truncate is what stops the
/// angle losing its low bits after a few minutes of riding, and the fold is what keeps the sign.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Roll {
    /// Radians, in `-pi..pi`.
    pub angle: f32,
}

impl Roll {
    /// One frame of the integration, at a rate in radians per second.
    pub fn step(&mut self, dt: f32, rate: f32) -> f32 {
        let mut a = self.angle + rate * dt;
        a -= (a * INV_TAU).trunc() * TAU;
        if a > std::f32::consts::PI {
            a -= TAU;
        } else if a < -std::f32::consts::PI {
            a += TAU;
        }
        // A rate the solver has not settled yet would otherwise stick in the angle for good.
        self.angle = if a.is_finite() { a } else { 0.0 };
        self.angle
    }
}

/// The most a wheel is drawn turning in one tick, in radians (V59).
///
/// Off the ground on the gas the rear wheel spins up to 80 or 90 radians a second, which at the
/// ride's 30 ticks is close to half a turn a tick, and on a straight the front is not far behind.
/// The picture is blended between two ticks the short way round, so past half a turn a wheel is
/// drawn turning backwards, and well before it a wheel of thin spokes stands still or crawls
/// (the wagon-wheel effect). Drawn, the turn is held to this, a fifth of a turn; past the rate
/// where that matters the [`Spin::blur`] has taken over and the eye reads speed from the blur,
/// not from the spokes. [`Roll`] keeps the original's own angle, uncapped.
pub const SPIN_STEP_CAP: f32 = 1.2;
/// The spin rate, in radians per second, where the blur starts to show (a walking pace on a
/// 250's wheel), and where it is whole (about 15 mph). Between them it eases in.
pub const BLUR_FROM: f32 = 6.0;
pub const BLUR_FULL: f32 = 20.0;
/// How far behind the drawn wheel its blurred copies trail, in radians: two spoke pitches of a
/// 36-spoke wheel, and about one of the tyre's knobs.
pub const BLUR_ARC: f32 = 0.8;
/// The blurred copies' alphas, nearest the wheel first, at a whole blur. The renderer lays its
/// see-through copies faintest first, so the nearest copy is laid last and weighs most, and with
/// these the wheel itself keeps about a quarter of the picture and each copy behind it less: a
/// smear that fades the way the wheel came from.
pub const BLUR_ALPHA: [f32; 8] = [0.2, 0.18, 0.17, 0.16, 0.15, 0.14, 0.13, 0.12];

/// A wheel as it is drawn turning, which is not the original's (V59): [`Roll`] turned by at most
/// [`SPIN_STEP_CAP`] a tick, and how blurred the spin is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spin {
    /// Radians, in `-pi..pi`: the turn the wheel is drawn at.
    pub angle: f32,
    /// 0 sharp to 1 a whole blur, from the rate between [`BLUR_FROM`] and [`BLUR_FULL`].
    pub blur: f32,
    /// Which way it turns, 1 or -1: the blur trails the other way.
    pub dir: f32,
}

impl Spin {
    /// One tick at a rate in radians per second.
    pub fn step(&mut self, dt: f32, rate: f32) -> f32 {
        if !(rate * dt).is_finite() {
            self.blur = 0.0;
            return self.angle;
        }
        let a = self.angle + (rate * dt).clamp(-SPIN_STEP_CAP, SPIN_STEP_CAP);
        let a = (a + std::f32::consts::PI).rem_euclid(TAU) - std::f32::consts::PI;
        self.angle = if a.is_finite() { a } else { 0.0 };
        let t = ((rate.abs() - BLUR_FROM) / (BLUR_FULL - BLUR_FROM)).clamp(0.0, 1.0);
        self.blur = t * t * (3.0 - 2.0 * t);
        self.dir = if rate < 0.0 { -1.0 } else { 1.0 };
        self.angle
    }
}

/// One see-through copy of the chassis for the spin blur: a pose of it in which every node but
/// one wheel's is shrunk to a point, so only that wheel draws, turned back along its trail.
#[derive(Clone, Debug, PartialEq)]
pub struct BlurCopy {
    pub pose: Vec<M4>,
    pub alpha: f32,
}

/// Which wheel each node of a chassis turns with: 1 under `TireFront`, 2 under `TireRear`, else 0.
fn wheels_of(skeleton: &Skeleton, nodes: Nodes) -> Vec<u8> {
    let mut out = vec![0u8; skeleton.len()];
    for j in 0..skeleton.len() {
        let mut at = Some(j);
        while let Some(k) = at {
            if Some(k) == nodes.front {
                out[j] = 1;
                break;
            }
            if Some(k) == nodes.rear {
                out[j] = 2;
                break;
            }
            at = skeleton.bones[k].parent;
        }
    }
    out
}

/// A rigid transform's inverse: the rotation transposed, the translation taken back through it.
fn rigid_inverse(m: M4) -> M4 {
    let t = v3(m.0[3][0], m.0[3][1], m.0[3][2]);
    let mut out = M4::IDENTITY;
    for r in 0..3 {
        for c in 0..3 {
            out.0[r][c] = m.0[c][r];
        }
    }
    let back = out.dir(t);
    out.0[3] = [-back.x, -back.y, -back.z, 1.0];
    out
}

/// What the chassis' bind pose says about how this bike is put together, worked out once.
///
/// Every number here comes out of the `.psg`'s own node tree. None of them is a constant: a bike
/// with a steeper rake has a different fork axis and a different `inv_sin`, and gets them from the
/// same two node positions.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Geometry {
    /// The fork, as a unit vector in the `Bike` frame: from the `Shocks` node up to the
    /// `Handlebars` node. Both the steering axis and the line the compression slides along.
    pub fork: V3,
    /// What a vertical compression is multiplied by to become a distance along the slanted fork:
    /// `sqrt(y^2 + z^2) / y` of the axis.
    ///
    /// The original reaches the same number the long way round — the axis' `y` through the arc-cosine
    /// table for an angle, then that angle's entry in the `{cos, sin}` table, then a reciprocal —
    /// and this is the closed form of it. It is worked out once at load time and the renderer's
    /// standard is by eye, so the two table lookups buy nothing here; see the journal.
    pub inv_sin: f32,
    /// Where `Shocks` sits in the `Bike` frame with the suspension fully extended.
    pub shocks_rest: V3,
    /// Where `SwingArm` pivots, in the `Bike` frame.
    pub swingarm_pivot: V3,
}

/// The chassis nodes the drives write, by index into the skeleton. A bike whose model is missing one
/// of them simply does not get that drive.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Nodes {
    pub root: Option<usize>,
    pub bars: Option<usize>,
    pub shocks: Option<usize>,
    pub front: Option<usize>,
    pub swingarm: Option<usize>,
    pub rear: Option<usize>,
}

impl Nodes {
    /// The six nodes of a chassis skeleton, by the names every `.psg` on this disc spells them with.
    pub fn of(skeleton: &Skeleton) -> Nodes {
        Nodes {
            root: skeleton.find(ROOT),
            bars: skeleton.find(BARS),
            shocks: skeleton.find(SHOCKS),
            front: skeleton.find(FRONT),
            swingarm: skeleton.find(SWINGARM),
            rear: skeleton.find(REAR),
        }
    }
}

/// The ride's own numbers the drives read, in one value: everything [`Chassis::frame`] needs out of
/// the simulation that is not the animation state.
///
/// Five of the six come straight off a wheel and the sixth off the air state. None of them is
/// touched here — [`Drive::of`] is a read of [`game_sim::ride::Ride`] and nothing else — which is
/// what keeps the bit-exact side bit-exact: the renderer's `f32` arithmetic starts after this.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drive {
    /// How far the front suspension is compressed, in metres (`wheel+0xf0`).
    pub front_compression: f32,
    /// The same for the rear.
    pub rear_compression: f32,
    /// Where the bars are, in radians (`wheel+0x8ac`).
    pub steer_angle: f32,
    /// How fast each wheel is spinning, in radians per second (`wheel+0x1d4`).
    pub front_spin: f32,
    pub rear_spin: f32,
    /// Where the rear wheel hangs off the chassis, in the chassis' own frame (`wheel+0xa0`), which
    /// is the point the swingarm is aimed at once the compression has moved it.
    pub rear_mount: V3,
    /// Either wheel is touching something (`actor+0x574`). The steering drive is the one thing here
    /// that stops when the bike is in the air.
    pub on_ground: bool,
}

impl Drive {
    /// The numbers, read off a stepped ride.
    pub fn of(ride: &Ride) -> Drive {
        let mount = ride.rear.w.mount;
        Drive {
            front_compression: ride.front.w.compression.0,
            rear_compression: ride.rear.w.compression.0,
            steer_angle: ride.front.w.steer_angle.0,
            front_spin: ride.front.mv.spin.0,
            rear_spin: ride.rear.mv.spin.0,
            rear_mount: v3(mount.x(), mount.y(), mount.z()),
            on_ground: ride.air.on_ground,
        }
    }
}

/// A chassis ready to be drawn: the bike's clips bound to its own skeleton, an animation machine
/// stepping them, and the transforms the last frame left on every node.
///
/// This is to the bike what [`crate::rider::Body`] is to the rider, with two differences. The
/// driving is *not* the simulation's — nothing in the ported ride runs a
/// [`game_sim::vehicle_anim::Driver`], because the function that decides its state is the actor's
/// own update and is not ported (see [`Chassis::want`]) — and the pose a clip writes is not the last
/// word, because the drives come after it.
///
/// **Which machine this is, is V13's unfinished half.** [`Stance`] is the *body's* sixty-state
/// machine, and the 12 `bike_*` clips a bike archive ships belong to the other one
/// ([`crate::anim::Rig`], whose table is named `bike_<stunt>` and `bike_air_Fwd/Mid/Bck` — exactly
/// those 12). V13 gave the sixty-state machine the set it belongs to, which is the rider's; the bike
/// is still stepped by a stance of its own, because moving it onto the simulation's own
/// [`game_sim::motion`] frame moves what poses the body with it. See the journal's **Not verified**
/// list.
pub struct Chassis {
    /// The bike's clips, the chassis skeleton, the live pose and the machine stepping them.
    pub stance: Stance,
    /// The model to load and to draw the chassis as: the `.mcf`'s own `SLTFile`, `bike` for every
    /// full-size bike on this disc and `bike50` under the fast50s ([`chassis_model`]).
    pub geometry: String,
    /// The six nodes the drives write.
    pub nodes: Nodes,
    /// What the bind pose says about this bike's fork and swingarm.
    pub shape: Geometry,
    /// The two wheels' rolled angles.
    pub front_roll: Roll,
    pub rear_roll: Roll,
    /// The two wheels as they are drawn turning (V59): what the wheel nodes are written from.
    pub front_spin: Spin,
    pub rear_spin: Spin,
    /// Which wheel each node turns with ([`wheels_of`]), for the blur's copies.
    pub wheels: Vec<u8>,
    /// Draw the wheels the original's way (V59): at [`Roll`]'s own angle, however fast, and never
    /// blurred.
    pub original_wheels: bool,
    /// The most front compression the fork is drawn at (V62, [`Chassis::stop_fork_under`]);
    /// infinite until the shell has measured the parts on the bars.
    pub fork_stop: f32,
    /// Every node's transform as the last [`Chassis::frame`] left it — the pose, the drives and the
    /// tree, composed. Empty until the first frame.
    pub world: Vec<M4>,
    /// The last frame's blend named a clip, so the live pose is a pose and not a leftover.
    ///
    /// Not [`crate::anim::Stance::drawn`], which is narrower: a clip that has *run out* writes nothing
    /// and still holds the pose it ended on, and that pose must be kept. This is the other case — a
    /// state with no clip behind it at all, which every grounded row of a bike's table is
    /// ([`crate::chassis::poses_the_chassis`]).
    pub posing: bool,
    /// How much of the pose drawn before a restart is still showing, on a frame whose blend names no
    /// clip: the blend's own `ramp`, which the machine runs whether or not there is a clip to fade
    /// *into*. Zero once it has run out, and on every frame a clip posed.
    pub fading: f32,
    /// The node pose the last frame drew, before the drives: the clip's, the rest shape, or the fade
    /// between them. A restart snapshots this rather than the pose buffer, which on the ground still
    /// holds the last airborne clip.
    pub shown: crate::skin::Pose,
}

/// The cross-fade's weight on a frame: how much of the snapshot is still showing. Mode 0 carries
/// none of its own.
fn ramp_of(blend: va::Blend) -> f32 {
    match blend {
        va::Blend::Free => 0.0,
        va::Blend::One { ramp, .. }
        | va::Blend::Two { ramp, .. }
        | va::Blend::Four { ramp, .. }
        | va::Blend::Pair { ramp, .. } => ramp.0,
    }
}

/// The rest shape faded back towards `under` by `fade`, joint by joint, with the clip samplers'
/// own arithmetic: a shortest-way slerp for the turn and a straight line for the place, 1 being all
/// of `under`.
fn fade_rest(skeleton: &Skeleton, under: &crate::skin::Pose, fade: f32) -> crate::skin::Pose {
    let mut out = skeleton.nodes(&crate::skin::Pose::new(skeleton.len()));
    for j in 0..out.rot.len().min(under.rot.len()) {
        out.rot[j] = crate::skin::slerp(fade, out.rot[j], under.rot[j], true);
        let (p, was) = (out.pos[j], under.pos[j]);
        out.pos[j] = [
            p[0] + (was[0] - p[0]) * fade,
            p[1] + (was[1] - p[1]) * fade,
            p[2] + (was[2] - p[2]) * fade,
            1.0,
        ];
    }
    out
}

impl Chassis {
    /// Read a bike's animation set and bind every clip of it a state names.
    ///
    /// `mcf` is the `.sub`'s own `[Objects] MCF` (`bike.mcf`), which names the chassis `.psg` as its
    /// `SLTFile` — so the skeleton the clips are bound to is the one the shell draws. `prefix` is
    /// what goes in front of a stunt's name: this machine's loader bakes **no** actor word in, so a
    /// bike's is `bike_` and that is what puts `bike_nacnac` in the table.
    ///
    /// The three passes are [`Stance::new`]'s, which is the body's loader; what a bike's set answers
    /// with is the difference. The geometry is read before the table is filled because the set is
    /// moved into the stance.
    pub fn load(
        archives: Vec<Res>,
        mcf: &str,
        prefix: &str,
        stunts: &[Stunt],
        params: Params,
    ) -> Result<Chassis, String> {
        let set = Set::load(archives, mcf)?;
        let geometry = set.set.geometry();
        // The table is filled the body's way, prefix retry and all ([`Stance::new`]): on a bike's set
        // that binds the 9 gesture clips the archive ships and the three `bike_air_*` poses, and
        // leaves the grid's 36 `turn_*`, `hold_*`, `get_ready_*` slots empty — they are the rider's
        // clips and no bike archive has one.
        let stance = Stance::new(set, prefix, stunts, params);
        let nodes = Nodes::of(&stance.set.skeleton);
        let shape = Geometry::of(&stance.set.skeleton, nodes);
        let wheels = wheels_of(&stance.set.skeleton, nodes);
        Ok(Chassis {
            stance,
            geometry,
            nodes,
            shape,
            front_roll: Roll::default(),
            rear_roll: Roll::default(),
            front_spin: Spin::default(),
            rear_spin: Spin::default(),
            wheels,
            original_wheels: false,
            fork_stop: f32::INFINITY,
            world: Vec::new(),
            posing: false,
            fading: 0.0,
            shown: crate::skin::Pose::new(0),
        })
    }

    /// **MD21:** a trick pack's bike clips in the stance's states past the sixty — its extras and the
    /// 360s — bound to this chassis. Answers the names that found no clip.
    pub fn add_pack(&mut self, pack: &crate::anim::PackClips, stunts: &[Stunt]) -> Vec<String> {
        let Stance { set, lib, .. } = &mut self.stance;
        pack.fill(set, lib, stunts, crate::anim::PackClips::bike_members)
    }

    /// [`Chassis::want`] with a trick pack's states (MD21): an extra in progress plays its own clip
    /// and a 360 past its clip angle its own, when this chassis has them; otherwise what
    /// [`Chassis::want`] says, which without a pack is all of it.
    pub fn want_with_pack(&self, ride: &Ride, tricks: &Tricks) -> i32 {
        let lib = &self.stance.lib;
        let has = |state: i32| lib.clip(state).is_some().then_some(state);
        let extra = tricks
            .current
            .extra()
            .filter(|_| tricks.in_progress)
            .and_then(|k| has(va::PACK_EXTRA + k as i32));
        let spin = ride.tricks.as_ref().filter(|t| t.spin.clip != 0).and_then(|t| {
            has(if t.spin.dir >= 1 { va::SPIN_RIGHT } else { va::SPIN_LEFT })
        });
        let want = Chassis::want(ride, tricks);
        match (extra, spin) {
            // a gesture slot outranks the pack, as on the rider
            _ if (0..SLOTS as i32).contains(&want) => want,
            (Some(state), _) | (None, Some(state)) => state,
            _ => want,
        }
    }

    /// **V64:** the state to run beside a rider whose body's machine asked for `body` this frame —
    /// [`game_sim::vehicle_anim::Driver::want`], after [`crate::rider::Body::ride`] — so the bike and
    /// the rider are never in two different tricks.
    ///
    /// A pack state on the body (an extra or a 360 its clip plays) is the bike's state too, whether
    /// or not this chassis has a clip for it. With none the bike is drawn the way a gesture slot with
    /// no bike clip is ([`Chassis::drive`]): the shape it was modelled in, faded to from the air pose.
    /// [`Chassis::want_with_pack`] fell back to [`game_sim::vehicle_anim::AIR`] instead, and the
    /// airborne row is played at the phase axis, which the motion driver stops easing while a trick
    /// runs — so a trick started whipped held the bike crossed up under a rider whose clip is square,
    /// and the pelvis came off the seat by the whip. And a body with no clip for a state does not
    /// play it, so neither does the bike, even with a clip of its own: the two stay on the airborne
    /// row together. Any other state is [`Chassis::want`]'s.
    pub fn want_beside(ride: &Ride, tricks: &Tricks, body: i32) -> i32 {
        match body {
            s if s >= va::PACK_EXTRA => s,
            _ => Chassis::want(ride, tricks),
        }
    }

    /// One frame: the machine's selection, the clock, the clip's pose, the five drives, the tree.
    ///
    /// `want` is the animation state to run — [`Chassis::want`] is the port's reading of it — and
    /// `r` the vehicle's request block. The placement is deliberately left to the caller: the shell
    /// already puts the bike where the simulation's own chassis transform says
    /// ([`crate::mat::M4::placement`]), which is the same node the original's placement writes, so
    /// the request is run as a self-placing actor and [`game_sim::vehicle_anim::Frame::place`] comes
    /// back `None`.
    pub fn frame(&mut self, dt: F, want: i32, r: &Request, drive: Drive) -> va::Frame {
        // What a restart snapshots is the buffer, so put what was last *drawn* in it: after a frame
        // with no clip that is the rest shape (or the fade towards it), not the airborne clip the
        // buffer still holds. Otherwise a landing fades out of nothing and a take-off fades out of
        // the last jump's crossup (S55).
        if !self.posing && self.shown.rot.len() == self.stance.set.pose.rot.len() {
            self.stance.set.pose = self.shown.clone();
        }
        let frame = self.stance.frame(dt, want, r);
        self.posing = crate::chassis::poses_the_chassis(frame.blend, &self.stance.player);
        self.fading = match (self.posing, frame.blend) {
            (true, _) => 0.0,
            // A pack state this chassis has no clip for is out of the table, so the selection
            // answers no blend and no ramp with it — but the machine restarted into it and runs its
            // ramp all the same, and the rider is fading into the trick's clip over that ramp. Fading
            // the rest shape by it keeps the bike under the rider instead of snapping it square on
            // the trick's first frame (V64).
            (false, va::Blend::Free) if self.stance.driver.state >= va::PACK_EXTRA => {
                self.stance.driver.ramp.0.max(0.0)
            }
            (false, blend) => ramp_of(blend),
        };
        self.drive(dt.0, drive);
        frame
    }

    /// The second half of a frame on its own: the five node drives over whatever pose is live, and
    /// the tree composed into [`Chassis::world`].
    ///
    /// Separate from [`Chassis::frame`] because it is separate in the original — the drives run on
    /// the chassis' nodes after the blend, and they run whether or not the blend wrote anything. A
    /// bike whose every clip is missing still has a working suspension.
    pub fn drive(&mut self, dt: f32, d: Drive) {
        // The pose buffer is only a pose when a clip wrote one. A bike's table has no clip for any
        // grounded state, so the rows a ride spends its life in name nothing and write nothing — and
        // what stays in the buffer is the last airborne clip, root channel and all. Drawing the
        // drives over *that* puts the whole bike wherever the crossup threw it and leaves it there
        // (V20). With no clip behind the frame the chassis is the shape it was modelled in, and the
        // drives are the only thing that moves it.
        //
        // But the machine still runs its cross-fade on such a frame: a landing restarts it into a
        // grounded row with the ramp at its top, asking for the airborne pose to ease out over the
        // fade rate. Drawing the rest shape at once threw that away, and a bike crossed up in the air
        // snapped square under its rider on the frame it touched down (S55). So while the ramp runs,
        // the rest shape is faded from the snapshot by it, the way a clip is (`Clip::pose_faded`).
        let rest = crate::skin::Pose::new(self.stance.set.skeleton.len());
        let faded;
        let live = match (self.posing, self.fading > 0.0) {
            (true, _) => &self.stance.set.pose,
            (false, true) => {
                faded = fade_rest(&self.stance.set.skeleton, &self.stance.set.under, self.fading);
                &faded
            }
            (false, false) => &rest,
        };
        self.shown = live.clone();
        let mut local = self.stance.set.skeleton.locals(live);
        self.front_roll.step(dt, d.front_spin);
        self.rear_roll.step(dt, d.rear_spin);
        // The wheels are drawn at their own turn and not the roll's (V59, [`Spin`]).
        let front = self.front_spin.step(dt, d.front_spin);
        let rear = self.rear_spin.step(dt, d.rear_spin);
        let (front, rear) = match self.original_wheels {
            true => (self.front_roll.angle, self.rear_roll.angle),
            false => (front, rear),
        };
        let shape = self.shape;

        // 1 — the steering head, and only with a wheel on the ground: the local rotation is
        // *replaced* by a turn about the fork axis, and the translation kept. Airborne, the bars stay
        // wherever the pose left them, which is what lets a stunt clip take the bars over.
        if let (Some(j), true) = (self.nodes.bars, d.on_ground) {
            if let Some(m) = local.get_mut(j) {
                set_rotation(m, about(shape.fork, d.steer_angle));
            }
        }
        // 2 — the fork's slide. The original works the target out in the `Bike` frame — the rest
        // position, plus the compression up the fork, plus a fixed nudge — and the node's parent is
        // the steering head, so it has to come back out of the frame the turn above just put the
        // bars in.
        //
        // Drawn, the slide stops at the fork stop, so the tyre never comes up into the fender (V62).
        self.slide_fork(&mut local, d.front_compression.min(self.fork_stop));
        // 3 and 5 — the wheels, each about its own `x`.
        for (node, angle) in [(self.nodes.front, front), (self.nodes.rear, rear)] {
            if let Some(m) = node.and_then(|j| local.get_mut(j)) {
                set_rotation(m, about(v3(1.0, 0.0, 0.0), angle));
            }
        }
        // 4 — the swingarm, aimed at the rear axle. Its whole local is rebuilt as a placement: the
        // pivot stays where the pose left it and the arm's own `+z` points from the compressed axle
        // back up to the pivot.
        if let Some(j) = self.nodes.swingarm {
            if let Some(&m) = local.get(j) {
                let pivot = v3(m.0[3][0], m.0[3][1], m.0[3][2]);
                let axle = v3(
                    d.rear_mount.x,
                    d.rear_mount.y + d.rear_compression,
                    d.rear_mount.z,
                );
                let look = shape.swingarm_pivot.sub(axle);
                if look.len() > 0.0 {
                    local[j] = M4::placement(pivot, look, v3(0.0, 1.0, 0.0));
                }
            }
        }
        self.world = self.stance.set.skeleton.compose(&local);
    }

    /// The fork's slide, as the original works it out, at a front compression of `c` (drive 2).
    fn slide_fork(&self, local: &mut [M4], c: f32) {
        let Some(j) = self.nodes.shocks else { return };
        let shape = self.shape;
        let target = shape.shocks_rest.add(shape.fork.scale(shape.inv_sin * c)).add(SHOCKS_NUDGE);
        let bars = self.nodes.bars.and_then(|b| local.get(b).copied());
        let local_pos = match bars {
            Some(bars) => unplace(bars, target),
            None => target,
        };
        if let Some(m) = local.get_mut(j) {
            set_translation(m, local_pos);
        }
    }

    /// **V62.** Stop the drawn fork short of the parts hung off the bars, and answer where.
    ///
    /// `geometry` is the chassis model ([`Chassis::geometry`]) and `parts` the `.sub` parts placed at
    /// `Handlebars`, each in its own model space. The original draws the fork at the simulation's
    /// compression whatever is above it, and the 250's `Front_Fender` hangs off the bars, so a hard
    /// landing (a compression of 1.0) put the tyre 0.1 ft up into it. This finds the most
    /// compression at which the front tyre — its outer radius about the `TireFront` axle, over its
    /// width — is still [`FORK_STOP_CLEAR`] clear of every vertex of every part, in the rest pose
    /// with the bars turned straight, and [`Chassis::drive`] draws the slide no farther. A part the tyre
    /// does not clear even fully extended is not one it can come up into, and is left out; with
    /// nothing to stop under, or no front wheel, the stop stays infinite. Presentation only: the
    /// simulation's compression is untouched.
    pub fn stop_fork_under(&mut self, geometry: &Model, parts: &[&Model]) -> f32 {
        let (Some(front), Some(bars)) = (self.nodes.front, self.nodes.bars) else {
            return self.fork_stop;
        };
        let skeleton = &self.stance.set.skeleton;
        let rest = skeleton.locals(&crate::skin::Pose::new(skeleton.len()));
        let mut stop = f32::INFINITY;
        for part in parts {
            let pose: Vec<M4> = (0..part.nodes.len()).map(|i| part.world(i)).collect();
            let points: Vec<V3> = skin(part, &pose, 0).iter().flat_map(|t| t.v.iter().map(|v| v.pos)).collect();
            let clear = |c: f32| -> f32 {
                // Grounded, as a landing is: the bars turned straight about the fork, then the slide.
                let mut local = rest.clone();
                set_rotation(&mut local[bars], about(self.shape.fork, 0.0));
                self.slide_fork(&mut local, c);
                let world = skeleton.compose(&local);
                let axle = world[front];
                let centre = v3(axle.0[3][0], axle.0[3][1], axle.0[3][2]);
                let axis = axle.dir(v3(1.0, 0.0, 0.0));
                let axis = axis.scale(1.0 / axis.len());
                let (mut outer, mut width) = (0.0f32, 0.0f32);
                for t in skin(geometry, &world, 0).iter().filter(|t| t.v.iter().all(|v| self.wheels.get(v.node) == Some(&1))) {
                    for v in &t.v {
                        let r = v.pos.sub(centre);
                        let h = r.dot(axis);
                        outer = outer.max(r.sub(axis.scale(h)).len());
                        width = width.max(h.abs());
                    }
                }
                let mut least = f32::MAX;
                for &p in &points {
                    let r = world[bars].point(p).sub(centre);
                    let h = r.dot(axis);
                    if h.abs() < width {
                        least = least.min(r.sub(axis.scale(h)).len() - outer);
                    }
                }
                least - FORK_STOP_CLEAR
            };
            if clear(0.0) <= 0.0 {
                continue;
            }
            // March up to the first compression that touches, then halve the step onto it.
            let step = 0.05;
            let mut lo = 0.0f32;
            while lo < FORK_STOP_SEARCH && clear(lo + step) > 0.0 {
                lo += step;
            }
            if lo >= FORK_STOP_SEARCH {
                continue;
            }
            let mut hi = lo + step;
            for _ in 0..16 {
                let mid = 0.5 * (lo + hi);
                match clear(mid) > 0.0 {
                    true => lo = mid,
                    false => hi = mid,
                }
            }
            stop = stop.min(lo);
        }
        self.fork_stop = stop;
        stop
    }

    /// [`Chassis::stop_fork_under`] with what the shell has loaded: the chassis model by its name and
    /// every part of the `.sub` placed at `Handlebars`. A model that did not load is left out.
    pub fn stop_fork_in(&mut self, models: &std::collections::HashMap<String, Loaded>, chassis: &str, parts: &[crate::scene::Part]) -> f32 {
        let Some(geometry) = models.get(chassis) else { return self.fork_stop };
        let on_bars: Vec<&Model> = parts
            .iter()
            .filter(|p| p.placement.eq_ignore_ascii_case(BARS))
            .filter_map(|p| models.get(&p.model).map(|l| &l.model))
            .collect();
        self.stop_fork_under(&geometry.model, &on_bars)
    }

    /// The spin blur's see-through copies of a drawn pose of this chassis (V59): for each wheel
    /// spinning past [`BLUR_FROM`], one copy per [`BLUR_ALPHA`], that wheel alone turned back along
    /// its trail by a share of [`BLUR_ARC`], at that alpha times how blurred it is. None for a
    /// wheel turning slowly, so a bike at rest draws no more than it did.
    ///
    /// `pose` is a pose as drawn — [`Chassis::world`], or the shell's blend of two of them — so
    /// the turn is put on in model space, about each wheel's own axle as that pose has it: a node
    /// under the wheel is carried by `inverse(tire) * turn * tire`. Every other node is a point,
    /// and its triangles have no area to draw.
    pub fn blur_copies(&self, pose: &[M4]) -> Vec<BlurCopy> {
        let mut out = Vec::new();
        if pose.len() != self.wheels.len() || self.original_wheels {
            return out;
        }
        for (wheel, node, spin) in [(1u8, self.nodes.front, self.front_spin), (2, self.nodes.rear, self.rear_spin)] {
            let Some(tire) = node.and_then(|j| pose.get(j).copied()) else { continue };
            if !(spin.blur > 0.0) {
                continue;
            }
            let back = rigid_inverse(tire);
            let n = BLUR_ALPHA.len() as f32;
            for (i, alpha) in BLUR_ALPHA.iter().enumerate() {
                let turn = -spin.dir * BLUR_ARC * (i as f32 + 1.0) / n;
                let carry = back.mul(about(v3(1.0, 0.0, 0.0), turn)).mul(tire);
                let pose = pose
                    .iter()
                    .zip(&self.wheels)
                    .map(|(m, w)| match *w == wheel {
                        true => m.mul(carry),
                        false => {
                            let mut point = M4::IDENTITY;
                            point.0 = [[0.0; 4], [0.0; 4], [0.0; 4], m.0[3]];
                            point
                        }
                    })
                    .collect();
                out.push(BlurCopy { pose, alpha: alpha * spin.blur });
            }
        }
        out
    }

    /// A node's live transform by name, for a `.sub` part to be placed at — the whole point of the
    /// drives on the drawing side. `Bike` answers with the chassis' own root, which is what an
    /// unmoving part asks for.
    pub fn node(&self, name: &str) -> Option<M4> {
        let j = self.stance.set.skeleton.find(name)?;
        self.world.get(j).copied()
    }

    /// Where a named node sits in the chassis' **bind** pose, in model space — which is the frame
    /// the simulation's wheel mounts and steering axis are in.
    ///
    /// The shell hands the ride the chassis model's own `TireFront` and `TireRear` positions rather
    /// than the `.phy`'s `BodyTireForcePosition` (S22, and `main.rs` does it off the loaded model),
    /// and the two are in different frames and different scales. Anything that drives a chassis has
    /// to read its mounts from the same place the shell does, so the set that owns the skeleton
    /// answers for them.
    pub fn node_rest(&self, name: &str) -> Option<V3> {
        self.stance.set.skeleton.at(name)
    }

    /// Write this frame's pose into the loaded chassis model, as the triangles the scene draw reads
    /// — the same thing [`crate::rider::Body::skin_into`] does for the body.
    pub fn skin_into(&self, loaded: &mut Loaded) {
        if !self.world.is_empty() {
            loaded.tris = skin(&loaded.model, &self.world, 0);
        }
    }

    /// Which animation state to run, read off the ride and the trick machine.
    ///
    /// **This is the port's own reading and not a port.** The function that writes
    /// [`game_sim::vehicle_anim::Driver::want`] is the body's own update — ported since V11 as
    /// [`game_sim::vehicle_anim::Update`], which needs the half of the request block nothing in the
    /// port writes (the gate's clock, the wreck, the commanded throttle), so the shell still reads
    /// the state off the ride instead. Three of the machine's
    /// sixty states are reachable from what the ported simulation does keep, which is what this
    /// answers with:
    ///
    /// * a gesture slot `0..24` while the trick machine is running one of them, which is the arm
    ///   that plays the bike's own stunt clips;
    /// * [`game_sim::vehicle_anim::AIR`] with neither wheel down, the airborne row;
    /// * [`game_sim::vehicle_anim::TURN_RIDING`] otherwise, the grounded pose grid.
    ///
    /// The other 56 — the start line, the wreck-and-hold group, the reverse pair, the backflip — are
    /// reached from vehicle state the port does not have.
    pub fn want(ride: &Ride, tricks: &Tricks) -> i32 {
        match tricks.current.slot() {
            Some(slot) if tricks.in_progress && slot < SLOTS => slot as i32,
            _ if !ride.air.on_ground => va::AIR,
            _ => va::TURN_RIDING,
        }
    }

    /// The request block for a frame of riding: where the bike is left to the caller, and the two
    /// axes and the gesture clock read off the simulation.
    ///
    /// `at` is where the shell has put the bike and which way it is facing, for the record's sake —
    /// the placement is skipped (see [`Chassis::frame`]), so nothing downstream reads them.
    ///
    /// The band is the contact velocity's forward lane (S112): in the original the bike and its rider
    /// read one block, the riding request, whose band the fill copies from it.
    pub fn request(ride: &Ride, tricks: &Tricks) -> Request {
        let speed = F(ride.air.contact_vel.z());
        Request {
            base: ride.origin,
            rotation: ride.chassis.quat,
            phase_axis: F::ZERO,
            mix_axis: F::ZERO,
            band: speed,
            progress: tricks.progress,
            seq: tricks.seq,
            ..Request::default()
        }
    }

    /// [`Chassis::request`] with the two axes filled in: the request block a ridden bike's chassis is
    /// stepped from, which is what the shell draws.
    ///
    /// **P29.** The airborne row plays `bike_air_Fwd/Mid/Bck` *at the phase axis* — the crossup sweep,
    /// 0 crossed up one way, 1 the other, 0.5 square — and picks the pair by the mix axis. Left at
    /// [`Chassis::request`]'s zeros, every flight eased the chassis' phase lag to one end of the sweep
    /// and the clip's root channel drew the bike crossed up all the way, whatever the solver did: off
    /// the lip square, down twisted, and square again on the first grounded frame, when the grounded
    /// rows stop posing it. The numbers are the ones [`game_sim::rider::Rider`] already hands the
    /// rider's body (its `body_request`): the motion driver's pose, which in the air is its shown
    /// crossup, and its fore-and-aft. In the original the rider's body is mounted on the *bike's*
    /// request block — the block's owner word is the bike, on every ridden body of the V21 save
    /// states — so the bike and its rider read one pair of axes, which is this. That block is the
    /// riding request (S112), which carries the pose and the fore-aft as the state pack latched them
    /// in front of this tick's header: [`Rider::packed_pose`] and [`Rider::packed_fore_aft`].
    pub fn riding(rider: &Rider) -> Request {
        Request {
            phase_axis: rider.packed_pose,
            mix_axis: rider.packed_fore_aft,
            ..Chassis::request(&rider.ride, &rider.tricks)
        }
    }
}

impl Geometry {
    /// The fork and the swingarm, out of a chassis skeleton's bind pose.
    ///
    /// Every position here is in the `Bike` frame rather than in model space, which is what the
    /// drives want: the root's own transform is taken out by composing the tree with an identity in
    /// its place. On every `.psg` on this disc that root transform *is* an identity and the two agree,
    /// but a bike whose root was offset would otherwise put its forks in the wrong place.
    pub fn of(skeleton: &Skeleton, nodes: Nodes) -> Geometry {
        let mut local: Vec<M4> = skeleton.bones.iter().map(|b| b.local).collect();
        if let Some(m) = nodes.root.and_then(|j| local.get_mut(j)) {
            *m = M4::IDENTITY;
        }
        let world = skeleton.compose(&local);
        let at = |j: Option<usize>| -> V3 {
            j.and_then(|j| world.get(j))
                .map_or(v3(0.0, 0.0, 0.0), |m| m.point(v3(0.0, 0.0, 0.0)))
        };
        let shocks_rest = at(nodes.shocks);
        let fork = at(nodes.bars).sub(shocks_rest);
        let fork = if fork.len() > 0.0 { fork.norm() } else { v3(0.0, 1.0, 0.0) };
        // A vertical travel along a line that leans back by `fork` is longer than the travel: this is
        // one over the sine of the fork's angle to the horizontal, which is the ratio of the line's
        // length to its vertical part.
        let flat = (fork.y * fork.y + fork.z * fork.z).sqrt();
        let inv_sin = if fork.y != 0.0 { flat / fork.y } else { 1.0 };
        Geometry { fork, inv_sin, shocks_rest, swingarm_pivot: at(nodes.swingarm) }
    }
}

/// The sixty-state machine's own numbers, **measured off the running original** (V12).
///
/// V9 had to guess every one of these: the five fade rates were the rider constructor's own 3.0, the
/// stunt cross-fade cap a quarter second, and the pose grid's three thresholds zero because no
/// `turn_*` clip is on a bike archive. V12 recorded the body's own object over PINE for 1500 frames
/// of the human's recording and read them off; all seven are constant for the whole recording.
///
/// | field | what it does | measured | V9's guess |
/// |---|---|---|---|
/// | `rate_ready` | the start-line group's fade rate, per second | 0.25 | 3.0 |
/// | `rate_hold` | the wreck-and-hold group's | 0.25 | 3.0 |
/// | `rate_default` | everything with no rate of its own | 4.0 | 3.0 |
/// | `rate_pose` | the airborne row's and the backflip's | 3.25 | 3.0 |
/// | `rate_stunt` | a gesture slot's, and `nohandedlander`'s | 4.0 | 3.0 |
/// | `cross_cap` | the longest a stunt cross-fade may take, in seconds | 1.0 | 0.25 |
/// | `band` | the pose grid's three thresholds on the forward speed | 7.3333, 14.6666, 21.9999 | 0, 0, 0 |
///
/// The three thresholds are feet per second — `1/1.46666` is the `0.681818` the shell turns a chassis
/// speed into mph with — so the grid's rows change over at 5, 10 and 15 mph exactly. With them at
/// zero every grounded frame took the row's top region; with them measured, the two regions below
/// `band[1]` are the four-clip blend that interpolates between two rows, which is the arm V11 ported
/// and nothing had yet asked for.
///
/// `start_phase` is the one field here that is still not read off the original. V12 found that
/// `+0x8f8`, which S25 read as this constant, is *last frame's progress through the gesture* and
/// takes 135 distinct values over the recording — so whatever the configured start phase is, it is
/// not that field. Zero is the clip's beginning, and the arm that reads it (the stunt cross-fade)
/// never ran in the recording, so nothing has measured it. The two time constants the two eased axes
/// carry are not here either, because the original does not keep them here: they are
/// [`crate::anim::MIX_TAU`] and [`crate::anim::PHASE_TAU`].
pub fn params() -> Params {
    Params {
        rate_stunt: F(4.0),
        rate_default: F(4.0),
        rate_pose: F(3.25),
        rate_hold: F(0.25),
        rate_ready: F(0.25),
        cross_cap: F(1.0),
        start_phase: F::ZERO,
        band: [F(7.3333), F(14.6666), F(21.9999)],
    }
}

/// A turn of `angle` radians about a unit axis, in the row-vector convention the rest of the
/// renderer's matrices are in — which is to say the transpose of the textbook Rodrigues matrix, and
/// the same convention [`crate::skin::rotation`] builds a quaternion's matrix in.
pub fn about(axis: V3, angle: f32) -> M4 {
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    let (x, y, z) = (axis.x, axis.y, axis.z);
    M4([
        [c + x * x * t, y * x * t + z * s, z * x * t - y * s, 0.0],
        [x * y * t - z * s, c + y * y * t, z * y * t + x * s, 0.0],
        [x * z * t + y * s, y * z * t - x * s, c + z * z * t, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

/// Replace a transform's rotation and leave its translation alone — the shape of four of the five
/// drives, and the same three row copies [`crate::skin::Skeleton::locals`] makes when a clip writes
/// a joint's rotation and not its position.
fn set_rotation(m: &mut M4, r: M4) {
    for row in 0..3 {
        m.0[row][..3].copy_from_slice(&r.0[row][..3]);
    }
}

/// Replace a transform's translation and leave its rotation alone.
fn set_translation(m: &mut M4, p: V3) {
    m.0[3][0] = p.x;
    m.0[3][1] = p.y;
    m.0[3][2] = p.z;
}

/// A point out of a transform's frame: the inverse of [`M4::point`] for a rigid transform.
///
/// Rigid is the assumption and it holds for every node of every `.psg` on this disc — a node carries
/// a rotation and a translation and no scale — so the rotation's inverse is its transpose and there
/// is no need for a general inverse the renderer does not otherwise have.
fn unplace(m: M4, p: V3) -> V3 {
    let q = p.sub(v3(m.0[3][0], m.0[3][1], m.0[3][2]));
    v3(
        q.x * m.0[0][0] + q.y * m.0[0][1] + q.z * m.0[0][2],
        q.x * m.0[1][0] + q.y * m.0[1][1] + q.z * m.0[1][2],
        q.x * m.0[2][0] + q.y * m.0[2][1] + q.z * m.0[2][2],
    )
}

/// The chassis model a bike is drawn on, before any [`Chassis`] is loaded: the `SLTFile` of the
/// `.mcf` its `.sub` names, read out of the first archive that has that set (V37). That is `bike` for
/// every full-size bike on this disc and `bike50` for the five fast50s, whose archives have no
/// `bike.psg` at all, so a renderer that always loaded `bike` drew a fast50 as its parts floating on
/// nothing. A `.sub` naming no set, or a set that cannot be read, falls back to `bike`.
pub fn chassis_model(archives: &[Res], vehicle: &crate::scene::Vehicle) -> String {
    // `bike.mcf` would also end `xbike.mcf`, so the name is looked for after a slash first.
    let mcf = vehicle.mcf.rsplit(['\\', '/']).next().unwrap_or("");
    let geometry = match mcf.is_empty() {
        true => String::new(),
        false => archives
            .iter()
            .find_map(|a| a.read_suffix(&format!("\\{mcf}")).or_else(|_| a.read_suffix(mcf)).ok())
            .map(|bytes| crate::rider::MotionSet::parse(&crate::ini::text(&bytes)).geometry())
            .unwrap_or_default(),
    };
    match geometry.is_empty() {
        true => ROOT.to_ascii_lowercase(),
        false => geometry,
    }
}
