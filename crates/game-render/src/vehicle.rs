//! **The bike that moves: the chassis skeleton posed from the vehicle's own animation machine, and
//! the five nodes the simulation drives instead of a clip.**
//!
//! Until this module the shell drew a statue. Every part of a `.sub` was placed at its chassis node's
//! *bind* transform, read once per frame off the model the archive shipped
//! (`chassis.model.world(i)`), so the forks never compressed although the ground step prints their
//! travel every frame, the wheels never turned although their angular velocity is right there, the
//! front end never steered with the stick and the swingarm never moved. And S25 — the vehicle's own
//! sixty-state animation machine — asked for the bike's clips every frame with nothing to consume
//! the pose it answered.
//!
//! [`Chassis`] is the other side of both. It is [`crate::anim::Set`] with the *vehicle's* driver,
//! clip table and clock beside it, exactly as [`crate::rider::Body`] is that set with the rider's —
//! and then the half a rider does not have.
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

use crate::anim::Set;
use crate::disc::Res;
use crate::mat::{M4, V3, v3};
use crate::skin::{Skeleton, skin};
use crate::world::Loaded;
use game_sim::ride::Ride;
use game_sim::trick::{SLOTS, Stunt, Tricks};
use game_sim::vehicle_anim::{self as va, Driver, Library, Params, Request};
use game_sim::motion::{Animations, Player, Snapshot};
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

/// A chassis ready to be drawn: the bike's clips bound to its own skeleton, the vehicle's animation
/// machine, and the transforms the last frame left on every node.
///
/// This is to the bike what [`crate::rider::Body`] is to the rider, with two differences. The
/// driving is *not* the simulation's — nothing in the ported ride runs a
/// [`game_sim::vehicle_anim::Driver`], because the function that decides its state is the vehicle's
/// own update and is not ported (see [`Chassis::want`]) — and the pose a clip writes is not the last
/// word, because the drives come after it.
pub struct Chassis {
    /// The clips, the chassis skeleton and the live pose.
    pub set: Set,
    /// The sixty-state clip table, as [`Library::load`] filled it.
    pub lib: Library,
    /// The vehicle's animation machine.
    pub driver: Driver,
    /// The motion object's clock — the same one the rider's animation runs on, because it is the
    /// same library underneath.
    pub player: Player,
    /// The pose a cross-fade fades from, as the simulation records it.
    pub snapshot: Snapshot,
    /// The machine's own numbers. See [`Chassis::load`] for where they come from, which is not the
    /// disc.
    pub params: Params,
    /// The model to load and to draw the chassis as: the `.mcf`'s own `SLTFile`, which is `bike` for
    /// every bike on this disc.
    pub geometry: String,
    /// The six nodes the drives write.
    pub nodes: Nodes,
    /// What the bind pose says about this bike's fork and swingarm.
    pub shape: Geometry,
    /// The two wheels' rolled angles.
    pub front_roll: Roll,
    pub rear_roll: Roll,
    /// Every node's transform as the last [`Chassis::frame`] left it — the pose, the drives and the
    /// tree, composed. Empty until the first frame.
    pub world: Vec<M4>,
    /// A clip has written a pose. Unlike the rider's ([`crate::rider::Body::posed`]) this is not
    /// what decides whether the chassis can be drawn — a bike's bind pose is the bike, where a
    /// rider's is a body with its arms out — so the shell draws [`Chassis::world`] from the first
    /// frame either way. It is kept because "no clip of the sixty states bound" and "the state
    /// machine is asking for a clip that is not on this disc" look identical on screen otherwise.
    pub posed: bool,
}

impl Chassis {
    /// Read a bike's animation set and bind every clip its sixty states name.
    ///
    /// `mcf` is the `.sub`'s own `[Objects] MCF` (`bike.mcf`), which names the chassis `.psg` as its
    /// `SLTFile` — so the skeleton the clips are bound to is the one the shell draws. `prefix` is
    /// what goes in front of a stunt's name: the vehicle's loader bakes **no** actor word in, unlike
    /// the rider's, so a bike's is `bike_` and that is what puts `bike_nacnac` in the table.
    ///
    /// Three passes, in the loader's own order: the `.mcf`'s own motions (the airborne poses), then
    /// the 24 gesture slots with no `UsedForBike` gate, then the 36 named pose clips — the last two
    /// being [`Library::load`] through this crate's [`Set`].
    pub fn load(
        archives: Vec<Res>,
        mcf: &str,
        prefix: &str,
        stunts: &[Stunt],
        params: Params,
    ) -> Result<Chassis, String> {
        let mut set = Set::load(archives, mcf)?;
        set.load_listed();
        let mut lib = Library::load(prefix, stunts, &mut set);
        // The 36 pose clips, by the prefix the gesture slots already use.
        //
        // [`Library::load`] asks for them by bare name — `air_Mid`, `turn_riding_Fwd` — and a bike's
        // `.mcf` names its own motions with the actor's word in front: `NAME=bike_air_Mid`. So the
        // ported loader's `bound("air_Mid")` finds nothing on a bike set, and the airborne row, which
        // is the one row of the grid this disc *has* clips for, comes out empty. The prefix is the
        // difference and it is the same `bike_` the gesture slots are named with
        // ([`Library::stunt_name`] bakes no actor word in, which is why the vehicle's prefix carries
        // one), so each empty slot is asked for a second time with it. Only the empty ones: a set
        // that answered to the bare name keeps what it bound.
        //
        // This is the port's own reading of a seam the decompilation was not re-read for; see the
        // journal's **Not verified** list.
        for (i, name) in va::POSE_CLIPS.iter().enumerate() {
            let slot = SLOTS + i;
            if lib.clip[slot].is_none() {
                let prefixed = format!("{prefix}{name}");
                set.load(&prefixed);
                lib.clip[slot] = set.bound(&prefixed);
            }
        }
        let geometry = set.set.geometry();
        let nodes = Nodes::of(&set.skeleton);
        let shape = Geometry::of(&set.skeleton, nodes);
        Ok(Chassis {
            set,
            lib,
            // `extra` is "which added record the selection takes instead of the state machine", and
            // **negative is none** — so a zeroed driver asks for record 0 on its very first frame and
            // the machine never makes a selection at all. The original's vehicle constructor sets
            // this; `Driver::default()` cannot, because `Default` for an `i32` is zero.
            driver: Driver { extra: -1, ..Driver::default() },
            player: Player::default(),
            snapshot: Snapshot::default(),
            params,
            geometry,
            nodes,
            shape,
            front_roll: Roll::default(),
            rear_roll: Roll::default(),
            world: Vec::new(),
            posed: false,
        })
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
        // The machine is `Copy`, which is what lets the clip-length closure borrow the set while the
        // step runs.
        let mut driver = self.driver;
        driver.want = want;
        let frame = {
            let set = &self.set;
            driver.frame(dt, &self.params, r, &self.lib, true, |id| {
                set.timing(Some(id)).duration
            })
        };
        self.driver = driver;

        if frame.restart {
            self.set.rewind(&mut self.snapshot);
        }
        // The two played-once groups put a clip on with a cross out of whatever was playing. A state
        // of those groups with no clip of its own still reaches the engine's play in the original,
        // which stores the null handle — and a null handle in the clock is "nothing is playing".
        if let Some(play) = frame.play {
            match play.put() {
                Some(put) => {
                    let playing = self.set.timing(self.player.clip).duration;
                    if self.player.play(put, playing) {
                        self.set.rewind(&mut self.snapshot);
                    }
                }
                None => {
                    self.player.clip = None;
                    self.player.finished = false;
                    self.player.fade.on = false;
                    self.player.time = F::ZERO;
                }
            }
        }
        let clip = self.set.timing(frame.blend.clock_clip(&self.player));
        frame.blend.run(&mut self.player, dt, &clip);
        let posed = self.set.apply_vehicle(frame.blend, &self.player);
        self.posed |= posed;
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
        let mut local = self.set.skeleton.locals(&self.set.pose);
        let front = self.front_roll.step(dt, d.front_spin);
        let rear = self.rear_roll.step(dt, d.rear_spin);
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
        if let Some(j) = self.nodes.shocks {
            let target = shape
                .shocks_rest
                .add(shape.fork.scale(shape.inv_sin * d.front_compression))
                .add(SHOCKS_NUDGE);
            let bars = self.nodes.bars.and_then(|b| local.get(b).copied());
            let local_pos = match bars {
                Some(bars) => unplace(bars, target),
                None => target,
            };
            if let Some(m) = local.get_mut(j) {
                set_translation(m, local_pos);
            }
        }
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
        self.world = self.set.skeleton.compose(&local);
    }

    /// A node's live transform by name, for a `.sub` part to be placed at — the whole point of the
    /// drives on the drawing side. `Bike` answers with the chassis' own root, which is what an
    /// unmoving part asks for.
    pub fn node(&self, name: &str) -> Option<M4> {
        let j = self.set.skeleton.find(name)?;
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
        self.set.skeleton.at(name)
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
    /// [`game_sim::vehicle_anim::Driver::want`] is the vehicle's own update, which S25 explicitly did
    /// not port — the module is handed the state and answers with the call. Three of the machine's
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
    pub fn request(ride: &Ride, tricks: &Tricks) -> Request {
        let speed = F(ride.chassis.local_vel.z());
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

/// The animation machine's own numbers, as the port runs them.
///
/// **None of these is off the disc and none is read out of the original.** They are the vehicle
/// constructor's, and the vehicle's constructor is not ported — S25 ports the step and is handed its
/// `Params`. What they do and why these values:
///
/// * the five fade rates are how fast the cross-fade ramp falls, per second, and they are all the
///   rider constructor's own `3` ([`game_sim::motion::Params::blend_rate`]) — the one rate in the
///   ported animation code that *was* read. A rate of zero would be worse than a wrong rate: the ramp
///   is put to 1 on every restart and only the rate brings it down, so a zero leaves every state
///   showing all of the pose it started from and the bike would never animate at all.
/// * `cross_cap` caps the stunt cross-fade at a quarter second. It must be above zero: the arm
///   divides by `min(cross_cap, half the clip)`, and a zero makes that weight a non-number.
/// * `start_phase` is where the clip being crossed out of is picked up on the frame the cross starts,
///   and zero is its beginning.
/// * the three `band` thresholds cut forward speed up for the grounded pose grid. They are left at
///   zero because **no `turn_*` clip is on this disc** — every row of that grid is empty on every
///   bike archive, so the thresholds cannot change what is drawn, and inventing three speeds would
///   be three numbers nobody could check.
///
/// Every one of these is in the journal's **Not verified** list.
pub fn params() -> Params {
    Params {
        rate_stunt: F(3.0),
        rate_default: F(3.0),
        rate_pose: F(3.0),
        rate_hold: F(3.0),
        rate_ready: F(3.0),
        cross_cap: F(0.25),
        start_phase: F::ZERO,
        band: [F::ZERO; 3],
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
