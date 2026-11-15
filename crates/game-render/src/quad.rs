//! **The quad's chassis: the nodes MX vs. ATV Unleashed's ATV models carry where a bike has six.**
//!
//! A bike's chassis is one fork and one swingarm ([`crate::vehicle::Nodes`]). The three ATV chassis
//! models on SLUS_211.04 (`ATVS_R_Chasis`, `atv_s_chasis`, `250R_atv_chasis`, each named by its own
//! motion set's `SLTFile`, MD37c) share one skeleton of 43 nodes, rooted at a node spelt
//! `250r_atv_chasis` in all three:
//!
//! ```text
//!   root ─┬─ AarmUpperLeft ─┬─ AarmUpperLeftNode
//!         │                 └─ LeftHub ── WheelLeftFront, LeftHubNode, LeftUpperHubNode,
//!         │                               TieRodLeftNode, FrontLeftParticle01
//!         ├─ AarmUpperRight ── (the same, Right)
//!         ├─ AarmLowerLeft/Right ── AarmLower…Node, FrontLower…Strut
//!         ├─ SwingArm ─┬─ RearAxle ── WheelLeftRear, WheelRightRear
//!         │            └─ RearLowerStrut, RearLeft/RightParticle01
//!         ├─ HandleBars ── BrakeHandle, ClutchHandle, Dummy_Hand_L/R
//!         └─ TieRodLeft/Right, FrontUpper…Strut, RearUpperStrut, Dummy_Foot_L/R, ShadowBody
//! ```
//!
//! (the tree as openmx-mods' `import-atv check` prints it, the same in all three; which of these the
//! original drives from its step, and how, is MD37b's). [`Nodes::of`] finds them by name, as the
//! game does, case-insensitively.
//!
//! **MD138:** [`pose`] poses them from the step — the port's own rig, matched by eye, as the cars'
//! ([`crate::car::pose`]): the original's ATV measure step is not ported. [`seat`] puts the rider on
//! the pack's ATV motion set ([`RIDING`], [`STOPPED`], [`AIR`]).

use crate::anim::{Bound, Set};
use crate::car::{Tire, aim, compose, find, origin, spin_and_stretch, turn};
use crate::mat::{M4, v3};
use crate::psg::Model;
use crate::rider::{Anchors, Body};
use crate::skin::{Clip, Pose, Skeleton};
use ps2_float::F;

pub const BARS: &str = "HandleBars";
pub const SWINGARM: &str = "SwingArm";
pub const REAR_AXLE: &str = "RearAxle";
/// The four wheels: front left, front right, rear left, rear right.
pub const WHEELS: [&str; 4] = ["WheelLeftFront", "WheelRightFront", "WheelLeftRear", "WheelRightRear"];
/// The front hubs, left and right: each carries its wheel and turns with the tie rod.
pub const HUBS: [&str; 2] = ["LeftHub", "RightHub"];
/// The A-arms: upper left, upper right, lower left, lower right.
pub const A_ARMS: [&str; 4] = ["AarmUpperLeft", "AarmUpperRight", "AarmLowerLeft", "AarmLowerRight"];
pub const TIE_RODS: [&str; 2] = ["TieRodLeft", "TieRodRight"];

/// Where each of the quad's moving parts is in a chassis skeleton; `None` for a name it lacks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Nodes {
    pub bars: Option<usize>,
    pub swingarm: Option<usize>,
    pub rear_axle: Option<usize>,
    pub wheels: [Option<usize>; 4],
    pub hubs: [Option<usize>; 2],
    pub a_arms: [Option<usize>; 4],
    pub tie_rods: [Option<usize>; 2],
}

impl Nodes {
    pub fn of(skeleton: &Skeleton) -> Nodes {
        Nodes {
            bars: skeleton.find(BARS),
            swingarm: skeleton.find(SWINGARM),
            rear_axle: skeleton.find(REAR_AXLE),
            wheels: WHEELS.map(|n| skeleton.find(n)),
            hubs: HUBS.map(|n| skeleton.find(n)),
            a_arms: A_ARMS.map(|n| skeleton.find(n)),
            tie_rods: TIE_RODS.map(|n| skeleton.find(n)),
        }
    }

    /// Every name [`Nodes::of`] looked for and did not find.
    pub fn missing(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        let mut one = |at: Option<usize>, name: &'static str| {
            if at.is_none() {
                out.push(name);
            }
        };
        one(self.bars, BARS);
        one(self.swingarm, SWINGARM);
        one(self.rear_axle, REAR_AXLE);
        for (at, name) in self.wheels.iter().zip(WHEELS) {
            one(*at, name);
        }
        for (at, name) in self.hubs.iter().zip(HUBS) {
            one(*at, name);
        }
        for (at, name) in self.a_arms.iter().zip(A_ARMS) {
            one(*at, name);
        }
        for (at, name) in self.tie_rods.iter().zip(TIE_RODS) {
            one(*at, name);
        }
        out
    }
}

/// The front struts, left and right: the upper on the chassis, the lower on the lower A-arm.
pub const FRONT_STRUTS: [[&str; 2]; 2] = [["FrontUpperLeftStrut", "FrontLowerLeftStrut"], ["FrontUpperRightStrut", "FrontLowerRightStrut"]];
/// The rear strut: the upper on the chassis, the lower on the swingarm.
pub const REAR_STRUT: [&str; 2] = ["RearUpperStrut", "RearLowerStrut"];
/// Where each lower A-arm meets its hub: under the hub, so the lower arm follows it up.
pub const LOWER_JOINTS: [&str; 2] = ["LeftHubNode", "RightHubNode"];
/// Where each tie rod meets its hub.
pub const TIE_ROD_ENDS: [&str; 2] = ["TieRodLeftNode", "TieRodRightNode"];

/// The chassis posed by its four wheels (in [`WHEELS`]' order: spin, steer, compression; the scale
/// is not read — a quad's tires do not stretch) and the bars turned by `bars` radians, in the model's
/// own space and node order. At rest, the model as it is.
///
/// The port's own rig, as the baja's ([`crate::car::pose`]): each upper A-arm swung about its own z
/// so its hub rises by the wheel's compression, the hub turned back level and steered about y, the
/// lower arm aimed at the hub's lower joint, the tie rod at its end on the hub, the swingarm about
/// its x so the rear axle rises by the more compressed rear wheel, the wheels spun, the handlebars
/// turned, then each strut pair aimed at each other. A node the model lacks is left as it is.
pub fn pose(model: &Model, wheels: &[Tire; 4], bars: f32) -> Vec<M4> {
    pose_from(model, model.nodes.iter().map(|n| n.local).collect(), wheels, bars)
}

/// [`pose`] from `local`, the chassis' locals as its clip leaves them (MD138a), in place of the
/// model's own: the rig measures from that pose as [`pose`] does from the model's.
pub fn pose_from(model: &Model, mut local: Vec<M4>, wheels: &[Tire; 4], bars: f32) -> Vec<M4> {
    let bind = compose(model, &local);
    let at = |name: &str| find(model, name);
    let (x, y, z) = (v3(1.0, 0.0, 0.0), v3(0.0, 1.0, 0.0), v3(0.0, 0.0, 1.0));
    for side in 0..2 {
        let wheel = &wheels[side];
        let (Some(upper), Some(hub)) = (at(A_ARMS[side]), at(HUBS[side])) else { continue };
        let reach = crate::car::into(bind[upper], origin(bind[hub]));
        // Up on each side: the hub lies out along the arm's x one way or the other.
        let length = reach.len();
        let swing = if length > 1e-6 { (wheel.compression / length).clamp(-1.0, 1.0).asin() } else { 0.0 } * reach.x.signum();
        turn(&mut local[upper], z, swing);
        turn(&mut local[hub], z, -swing);
        turn(&mut local[hub], y, wheel.steer);
        if let Some(w) = at(WHEELS[side]) {
            spin_and_stretch(&mut local[w], wheel, false);
        }
    }
    if let (Some(arm), Some(axle)) = (at(SWINGARM), at(REAR_AXLE)) {
        let up = wheels[2].compression.max(wheels[3].compression);
        // Risen by `up` on the circle the axle swings on about the swingarm's pivot.
        let (pivot, rest) = (origin(bind[arm]), origin(bind[axle]));
        let (r, dy) = ((rest.y - pivot.y).hypot(rest.z - pivot.z), rest.y + up - pivot.y);
        let dz = (r * r - dy * dy).max(0.0).sqrt() * (rest.z - pivot.z).signum();
        let now = compose(model, &local);
        aim(&mut local, arm, &now, &bind, v3(rest.x, pivot.y + dy, pivot.z + dz), rest, Some(x));
    }
    for k in 2..4 {
        if let Some(w) = at(WHEELS[k]) {
            spin_and_stretch(&mut local[w], &wheels[k], false);
        }
    }
    if let Some(b) = at(BARS) {
        turn(&mut local[b], y, bars);
    }
    let now = compose(model, &local);
    for side in 0..2 {
        if let (Some(lower), Some(joint)) = (at(A_ARMS[2 + side]), at(LOWER_JOINTS[side])) {
            aim(&mut local, lower, &now, &bind, origin(now[joint]), origin(bind[joint]), Some(z));
        }
        if let (Some(rod), Some(end)) = (at(TIE_RODS[side]), at(TIE_ROD_ENDS[side])) {
            aim(&mut local, rod, &now, &bind, origin(now[end]), origin(bind[end]), None);
        }
    }
    let now = compose(model, &local);
    for [u, l] in [FRONT_STRUTS[0], FRONT_STRUTS[1], REAR_STRUT] {
        let (Some(u), Some(l)) = (at(u), at(l)) else { continue };
        aim(&mut local, u, &now, &bind, origin(now[l]), origin(bind[l]), None);
        aim(&mut local, l, &now, &bind, origin(now[u]), origin(bind[u]), None);
    }
    compose(model, &local)
}

/// The ATV motion set's riding row, forward, middle and back: each clip a sweep of the turn's lean
/// over its length.
pub const RIDING: [&str; 3] = ["ATV_turn_riding_Fwd", "ATV_turn_riding_Mid", "ATV_turn_riding_Bck"];
/// Its row for a quad standing still.
pub const STOPPED: [&str; 3] = ["ATV_turn_stop_Fwd", "ATV_turn_stop_Mid", "ATV_turn_stop_Bck"];
/// Its row in the air.
pub const AIR: [&str; 3] = ["ATV_air_Fwd", "ATV_air_Mid", "ATV_air_Bck"];

/// Bind `clip` into the rider's `set` under `name`, unless a clip of that name is bound already. The
/// ATV motion set names the MX rider's skeleton (`mrider.Slt`), so its clips drive the bike's rider
/// as they are.
pub fn bind(set: &mut Set, name: &str, clip: &Clip) {
    if set.id(name).is_none() {
        let joints = clip.joints(&set.skeleton);
        set.clips.push(Bound { name: name.to_owned(), member: name.to_owned(), clip: clip.clone(), joints });
    }
}

/// Pose the rider's `set` on one `row` ([`RIDING`], [`STOPPED`], [`AIR`]): at `phase` (0 to 1) of
/// the clips' length for the turn's lean, mixed from the middle clip towards the forward one as
/// `fore_aft` goes to -1 and the back one as it goes to 1. False if the row is not bound ([`bind`]).
pub fn seat(set: &mut Set, row: [&str; 3], phase: f32, fore_aft: f32) -> bool {
    let (Some(fwd), Some(mid), Some(bck)) = (set.id(row[0]), set.id(row[1]), set.id(row[2])) else {
        return false;
    };
    let lean = fore_aft.clamp(-1.0, 1.0);
    let toward = if lean < 0.0 { fwd } else { bck };
    let (a, b) = (&set.clips[mid.0 as usize], &set.clips[toward.0 as usize]);
    let time = phase.clamp(0.0, 1.0) * a.clip.duration();
    a.clip.pose_mixed(&b.clip, time, 1.0 - lean.abs(), &a.joints, &mut set.pose);
    true
}

/// The rider's node matrices on the quad: [`seat`]ed on `row`, held to the chassis' `anchors` (its
/// own pegs and grips, in the chassis' space, which is the body's) at full strength with both feet
/// on, and the body's own pose, hold and feet put back afterwards, as the plane's pilot
/// ([`crate::biplane::pilot_palette`]). `None` if the row is not bound.
pub fn rider_palette(body: &mut Body, anchors: &Anchors, row: [&str; 3], phase: f32, fore_aft: f32) -> Option<Vec<M4>> {
    rider_palette_stunt(body, anchors, row, phase, fore_aft, None)
}

/// [`rider_palette`] with a stunt's clip (MD138a: bound by name, at a time into it, held on its
/// last frame past its end) laid over the row on the joints it names.
pub fn rider_palette_stunt(body: &mut Body, anchors: &Anchors, row: [&str; 3], phase: f32, fore_aft: f32, stunt: Option<(&str, f32)>) -> Option<Vec<M4>> {
    let (pose, hold, feet) = (body.rig.set.pose.clone(), body.hold, body.feet);
    body.hold = 1.0;
    body.feet = Default::default();
    let seated = seat(&mut body.rig.set, row, phase, fore_aft);
    if let (true, Some((name, time))) = (seated, stunt) {
        let set = &mut body.rig.set;
        if let Some(id) = set.id(name) {
            let b = &set.clips[id.0 as usize];
            b.clip.pose(time.min(b.clip.duration()), &b.joints, &mut set.pose);
        }
    }
    let palette = seated.then(|| body.palette(Some(anchors)).0);
    body.rig.set.pose = pose;
    body.hold = hold;
    body.feet = feet;
    palette
}

// --- MD138a: the quad's own clips and the rider's stunt clips -----------------------------------------

/// One row of the rider's stunt table as the quad's clips read it: its name, how long it runs, and the
/// length of the quad's own clip for it when the quad binds one (`ATV_Bike_<name>`, for a row
/// `UsedForBike` whose clip the pack has).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stunt {
    pub name: String,
    pub duration: f32,
    pub machine: Option<f32>,
}

/// The stunts the start keeps a running stunt through: the flips, the 360s and the one-pair stunts.
pub const FLIPS: [&str; 5] = ["backflip", "threesixtyl", "threesixtyr", "wheelie", "stoppie"];
/// Of those, the ones on one pair of wheels: they never replace the stunt the quad is on.
pub const ONE_PAIR: [&str; 2] = ["wheelie", "stoppie"];
/// The stunt the spawn leaves as the balance stunt, and the id it is at in MX vs. ATV's table.
pub const BALANCE: &str = "nohandedlander";
const BALANCE_ID: i32 = 4;
/// No stunt: what the quad is on, on the ground.
pub const NO_STUNT: i32 = 255;

/// One frame's clip on the chassis, as the animation driver hands it out ([`game_sim::quad::animate`]'s
/// [`Call::PlayClip`] and [`Call::BlendClips`]): places in the quad's clip table (the 46 stunts', then
/// [`game_sim::quad::POSE_CLIPS`]), the phase into the clip, the mix, and the fade back to the pose
/// the clip came in over.
///
/// [`Call::PlayClip`]: game_sim::quad::Call::PlayClip
/// [`Call::BlendClips`]: game_sim::quad::Call::BlendClips
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Played {
    One { clip: i32, weight: f32, fade: f32 },
    Two { clips: [i32; 2], weight: f32, mix: f32, fade: f32 },
}

impl Played {
    pub fn fade(&self) -> f32 {
        match *self {
            Played::One { fade, .. } | Played::Two { fade, .. } => fade,
        }
    }

    /// The clip that leads ([`Played::Two`]'s first).
    pub fn clip(&self) -> i32 {
        match *self {
            Played::One { clip, .. } => clip,
            Played::Two { clips, .. } => clips[0],
        }
    }
}

/// **The quad's clips, ticked beside its ride** (MD138a): which stunt the quad is on, and the
/// original's animation driver run on it ([`game_sim::quad::animate`], MD65c's port) for the chassis'
/// clip, with the rider's stunt clip started and stopped by the ride's own calls.
///
/// The stunt word follows what the original's quad does around its steps (research/oracle.txt
/// `# MD138a`): a [`Call::StartStunt`] puts the stunt on in the air, or for a flip, a 360 or a
/// one-pair stunt at any time, but a flip-class stunt replaces only a riding pose (above 40) and the
/// one-pair stunts none; the stunt's clock restarts and the rider's clip with it. Each frame the stunt
/// runs its clock and ends at the row's `Duration` (a flip or 360 held while it is still going); with
/// none on, the quad is on the riding pose in the air and on no stunt on the ground; a wrecked quad is
/// on none. [`Call::EndStunt`] stops the rider's clip.
///
/// [`Call::StartStunt`]: game_sim::quad::Call::StartStunt
/// [`Call::EndStunt`]: game_sim::quad::Call::EndStunt
#[derive(Clone, Debug)]
pub struct Driver {
    pub stunts: Vec<Stunt>,
    pub anim: game_sim::quad::Anim,
    /// The stunt the quad is on, and how long it has been.
    pub current: i32,
    pub clock: f32,
    /// The rider's stunt clip (a row of [`Driver::stunts`]) and how far into it, while one plays.
    pub rider: Option<(i32, f32)>,
    /// This frame's clip on the chassis, and the one it fades from (the clip before the last stop).
    pub played: Option<Played>,
    pub under: Option<Played>,
}

/// The quad's clips as [`game_sim::quad::animate`] asks for them.
struct Lengths<'a>(&'a [Stunt]);

impl game_sim::quad::StuntClips for Lengths<'_> {
    fn ready(&mut self, stunt: i32) -> bool {
        usize::try_from(stunt).ok().and_then(|i| self.0.get(i)).is_some_and(|s| s.machine.is_some())
    }

    fn length(&mut self, stunt: i32) -> F {
        F(usize::try_from(stunt).ok().and_then(|i| self.0.get(i)).and_then(|s| s.machine).unwrap_or(0.0))
    }
}

impl Driver {
    /// The driver as a quad is spawned: the spawn's fade rate (3 a second), crossup range (a quarter
    /// turn's half) and blends (a half each), vehicle state 1, on no stunt.
    pub fn new(stunts: Vec<Stunt>) -> Driver {
        let balance = stunts.iter().position(|s| s.name.eq_ignore_ascii_case(BALANCE)).map_or(BALANCE_ID, |i| i as i32);
        let anim = game_sim::quad::Anim {
            vehicle_state: 1,
            fade: F::ZERO,
            fade_rate: F(3.0),
            stunt: NO_STUNT,
            next_stunt: NO_STUNT,
            balance_stunt: balance,
            restart: 0,
            stunt_time: F::ZERO,
            crossup_range: F(core::f32::consts::FRAC_PI_4),
            crossup_blend: F(0.5),
            fore_aft_blend: F(0.5),
        };
        Driver { stunts, anim, current: NO_STUNT, clock: 0.0, rider: None, played: None, under: None }
    }

    fn named(&self, id: i32, names: &[&str]) -> bool {
        usize::try_from(id).ok().and_then(|i| self.stunts.get(i)).is_some_and(|s| names.iter().any(|n| s.name.eq_ignore_ascii_case(n)))
    }

    /// A stunt's name, `None` for no row.
    pub fn name(&self, id: i32) -> Option<&str> {
        usize::try_from(id).ok().and_then(|i| self.stunts.get(i)).map(|s| s.name.as_str())
    }

    /// The stunt start: see [`Driver`].
    fn start(&mut self, id: i32, airborne: bool, down: bool) {
        let flip = self.named(id, &FLIPS);
        if !(airborne && !down) && !flip {
            return;
        }
        if !flip || (self.current > 40 && !self.named(id, &ONE_PAIR)) {
            self.current = id;
        }
        self.clock = 0.0;
        self.anim.restart = 1;
        self.rider = Some((id, 0.0));
    }

    /// One frame after the ride's step: its calls this tick (`ride.rest`), the stunt's clock and end,
    /// then the animation driver on copies of the quad (nothing of the ride is written).
    pub fn tick(&mut self, ride: &game_sim::quad::Ride, dt: f32) {
        let held = ride.bike.air.flip_active || ride.bike.spin_active;
        let pb = ride.tailed.as_ref().map(|t| t.put_back.clone()).unwrap_or_default();
        self.tick_on(&ride.rest, &ride.q, ride.down(), held, pb, dt);
    }

    /// [`Driver::tick`] on its parts: the tick's calls, the quad, whether it is down, whether a flip or
    /// a 360 is still going, and its put-back.
    pub fn tick_on(&mut self, calls: &[game_sim::quad::Call], q: &game_sim::quad::Quad, down: bool, going: bool, mut pb: game_sim::quad::PutBack, dt: f32) {
        use game_sim::quad::Call;
        let airborne = !q.on_ground;
        if let Some((_, t)) = &mut self.rider {
            *t += dt;
        }
        for c in calls {
            match *c {
                Call::StartStunt(id) => self.start(id as i32, airborne, down),
                Call::EndStunt => self.rider = None,
                _ => {}
            }
        }
        self.current = if down {
            NO_STUNT
        } else {
            let on = usize::try_from(self.current).is_ok_and(|i| i < self.stunts.len());
            let held = going && self.named(self.current, &FLIPS[..3]);
            self.clock += dt;
            let ended = on && !held && self.clock >= self.stunts[self.current as usize].duration;
            if ended {
                self.rider = None;
            }
            if on && !ended {
                self.current
            } else if airborne {
                game_sim::quad::RIDING_POSE
            } else {
                NO_STUNT
            }
        };
        self.anim.next_stunt = self.current;
        self.anim.stunt_time = F(self.clock);
        let (mut q, mut body) = (*q, game_sim::quad::Body::default());
        let mut out: Vec<Call> = Vec::new();
        game_sim::quad::animate(&mut self.anim, &mut pb, &mut q, &mut body, &game_sim::quad::NetClock::default(), F(dt), &mut Lengths(&self.stunts), &mut out);
        for c in out {
            match c {
                Call::AnimStop => self.under = self.played,
                Call::PlayClip { clip, weight, fade } => self.played = Some(Played::One { clip, weight: weight.0, fade: fade.0 }),
                Call::BlendClips { clips, weight, mix, fade } => self.played = Some(Played::Two { clips, weight: weight.0, mix: mix.0, fade: fade.0 }),
                _ => {}
            }
        }
        self.anim.restart = 0;
    }
}

/// The quad's clip table resolved to clips: `stunt(id)` the quad's own clip for a stunt below
/// [`game_sim::quad::RIDING_POSE`], `pose(k)` the riding pose's `k`th (fore, middle, back).
pub fn clip_of<'a>(id: i32, stunt: impl Fn(usize) -> Option<&'a Clip>, pose: impl Fn(usize) -> Option<&'a Clip>) -> Option<&'a Clip> {
    let p = game_sim::quad::POSE_CLIPS;
    match usize::try_from(id).ok()? {
        i if (i as i32) < p[0] => stunt(i),
        i if (i as i32) <= p[2] => pose(i - p[0] as usize),
        _ => None,
    }
}

/// The chassis' joints as one [`Played`] lays them, over `under` (what it fades back to): one clip at
/// its phase, or two at the first's phase mixed (`mix` 1 the first, 0 the second). `None` for a clip
/// the table lacks.
pub fn lay<'a>(played: Played, skeleton: &Skeleton, under: &Pose, clip: impl Fn(i32) -> Option<&'a Clip>) -> Option<Pose> {
    let mut out = under.clone();
    out.hold();
    match played {
        Played::One { clip: c, weight, fade } => {
            let c = clip(c)?;
            c.pose_faded(weight * c.duration(), &c.joints(skeleton), fade, under, &mut out);
        }
        Played::Two { clips, weight, mix, fade } => {
            let (a, b) = (clip(clips[0])?, clip(clips[1])?);
            a.pose_mixed_faded(b, weight * a.duration(), mix, &a.joints(skeleton), fade, under, &mut out);
        }
    }
    Some(out)
}

/// The chassis' locals off a pose of its skeleton: the model's own where the pose wrote nothing.
pub fn locals_of(model: &Model, pose: &Pose) -> Vec<M4> {
    Skeleton::of(model).locals(pose)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mat::V3;
    use crate::psg::Node;

    fn node(name: &str, parent: Option<usize>, at: V3) -> Node {
        let mut local = M4::IDENTITY;
        local.0[3] = [at.x, at.y, at.z, 1.0];
        Node { name: name.into(), local, parent, center: at, half_extents: v3(1.0, 1.0, 1.0), radius: 1.0 }
    }

    /// The chassis in miniature: one front corner, the swingarm and rear axle, the bars, a strut pair.
    fn chassis() -> Model {
        let nodes = vec![
            node("250r_atv_chasis", None, v3(0.0, 0.0, 0.0)),
            node(A_ARMS[1], Some(0), v3(0.3, 1.0, 1.8)),
            node(HUBS[1], Some(1), v3(1.05, 0.0, 0.0)),
            node(WHEELS[1], Some(2), v3(0.3, -0.2, 0.0)),
            node(LOWER_JOINTS[1], Some(2), v3(0.0, -0.4, 0.0)),
            node(A_ARMS[3], Some(0), v3(0.2, 0.6, 1.8)),
            node(TIE_RODS[1], Some(0), v3(0.2, 0.9, 1.6)),
            node(TIE_ROD_ENDS[1], Some(2), v3(0.0, 0.0, -0.2)),
            node(SWINGARM, Some(0), v3(0.0, 1.21, -0.66)),
            node(REAR_AXLE, Some(8), v3(0.0, -0.63, -1.33)),
            node(WHEELS[3], Some(9), v3(0.9, 0.0, 0.0)),
            node(BARS, Some(0), v3(0.0, 2.32, 1.36)),
            node(FRONT_STRUTS[1][0], Some(0), v3(0.5, 1.6, 1.8)),
            node(FRONT_STRUTS[1][1], Some(5), v3(0.6, 0.0, 0.0)),
        ];
        Model { nodes, materials: Vec::new(), meshes: Vec::new(), groups: 1, group_distance: vec![f32::MAX] }
    }

    fn at(p: &[M4], i: usize) -> V3 {
        p[i].point(v3(0.0, 0.0, 0.0))
    }

    #[test]
    fn at_rest_the_quad_is_its_model() {
        let m = chassis();
        let p = pose(&m, &[Tire::REST; 4], 0.0);
        for (i, posed) in p.iter().enumerate() {
            let w = m.world(i);
            for r in 0..4 {
                for c in 0..4 {
                    assert!((posed.0[r][c] - w.0[r][c]).abs() < 1e-5, "node {i}: {posed:?} against {w:?}");
                }
            }
        }
    }

    #[test]
    fn compression_lifts_the_wheels_and_the_lower_arm_follows() {
        let m = chassis();
        let mut wheels = [Tire::REST; 4];
        wheels[1].compression = 0.3;
        wheels[3].compression = 0.25;
        let (rest, p) = (pose(&m, &[Tire::REST; 4], 0.0), pose(&m, &wheels, 0.0));
        let hub = at(&p, 2).sub(at(&rest, 2));
        assert!((hub.y - 0.3).abs() < 1e-3, "the hub rose {hub:?}");
        let axle = at(&p, 9).sub(at(&rest, 9));
        assert!((axle.y - 0.25).abs() < 1e-3, "the rear axle rose {axle:?}");
        // The lower arm points at the hub's lower joint as it did at rest.
        let along = p[5].point(crate::car::into(rest[5], at(&rest, 4))).sub(at(&p, 5)).norm();
        let is = at(&p, 4).sub(at(&p, 5)).norm();
        assert!(along.dot(is) > 0.9999, "the lower arm points {along:?}, the joint is {is:?}");
        assert!(p[2].0[0][1].abs() < 1e-4, "the hub tipped: {:?}", p[2].0[0]);
    }

    /// MX vs. ATV's table in miniature: 46 rows, the taunt (0) with a clip of the quad's own, the
    /// backflip (41) without, as the original's `UsedForBike` has them.
    fn table() -> Vec<Stunt> {
        let mut t: Vec<Stunt> = (0..46).map(|i| Stunt { name: format!("stunt{i}"), duration: 1.0, machine: None }).collect();
        t[0] = Stunt { name: "Taunt".into(), duration: 2.0, machine: Some(1.5) };
        t[4].name = BALANCE.into();
        t[41] = Stunt { name: "backflip".into(), duration: 0.5, machine: None };
        t[42].name = "wheelie".into();
        t
    }

    fn quad(on_ground: bool) -> game_sim::quad::Quad {
        game_sim::quad::Quad { on_ground, ..Default::default() }
    }

    const DT: f32 = 1.0 / 30.0;

    #[test]
    fn on_the_ground_the_quad_plays_the_middle_pose_at_a_half() {
        let mut d = Driver::new(table());
        assert_eq!(d.anim.balance_stunt, 4);
        d.tick_on(&[], &quad(true), false, false, Default::default(), DT);
        assert_eq!(d.current, NO_STUNT);
        // The spawn is on no stunt already: nothing stops, nothing fades.
        assert_eq!(d.played, Some(Played::One { clip: game_sim::quad::POSE_CLIPS[1], weight: 0.5, fade: 0.0 }));
    }

    #[test]
    fn in_the_air_the_riding_pose_blends() {
        let mut d = Driver::new(table());
        d.tick_on(&[], &quad(false), false, false, Default::default(), DT);
        assert_eq!(d.current, game_sim::quad::RIDING_POSE);
        let Some(Played::Two { clips, weight, mix, .. }) = d.played else { panic!("{:?}", d.played) };
        // The stick and the crossup at rest: the middle and back poses, all the middle, halfway.
        assert_eq!((clips, weight, mix), ([48, 47], 0.5, 0.0));
    }

    #[test]
    fn a_backflip_plays_the_riders_clip_and_the_quads_air_pose() {
        use game_sim::quad::Call;
        let mut d = Driver::new(table());
        let air = quad(false);
        d.tick_on(&[], &air, false, false, Default::default(), DT);
        d.tick_on(&[Call::StartStunt(41)], &air, false, true, Default::default(), DT);
        assert_eq!(d.current, 41, "a flip replaces the riding pose");
        assert_eq!(d.rider.map(|r| r.0), Some(41));
        // The quad binds no clip for a stunt not `UsedForBike`: the middle pose at a half.
        assert_eq!(d.played, Some(Played::One { clip: game_sim::quad::POSE_CLIPS[1], weight: 0.5, fade: 1.0 }));
        for _ in 0..30 {
            d.tick_on(&[], &air, false, true, Default::default(), DT);
        }
        assert_eq!(d.current, 41, "held past its duration while the flip goes on");
        d.tick_on(&[], &air, false, false, Default::default(), DT);
        assert_eq!((d.current, d.rider), (game_sim::quad::RIDING_POSE, None), "ended when the flip stops");
        assert!(matches!(d.played, Some(Played::Two { .. })));
    }

    #[test]
    fn a_stunt_with_the_quads_own_clip_plays_it_by_its_clock() {
        use game_sim::quad::Call;
        let mut d = Driver::new(table());
        let air = quad(false);
        d.tick_on(&[Call::StartStunt(0)], &air, false, false, Default::default(), DT);
        assert_eq!(d.played, Some(Played::One { clip: 0, weight: 0.0, fade: 1.0 }), "stopped on the change: from its start");
        d.tick_on(&[], &air, false, false, Default::default(), DT);
        let Some(Played::One { clip: 0, weight, fade }) = d.played else { panic!("{:?}", d.played) };
        assert!((weight - 2.0 * DT / 1.5).abs() < 1e-6, "weight {weight}");
        assert!((fade - (1.0 - 3.0 * DT)).abs() < 1e-6, "fade {fade}");
        // On the ground a trick does not start; a one-pair stunt plays the rider's clip only.
        let mut d = Driver::new(table());
        d.tick_on(&[Call::StartStunt(0)], &quad(true), false, false, Default::default(), DT);
        assert_eq!((d.current, d.rider), (NO_STUNT, None));
        d.tick_on(&[Call::StartStunt(42)], &quad(true), false, false, Default::default(), DT);
        assert_eq!((d.current, d.rider.map(|r| r.0)), (NO_STUNT, Some(42)));
    }
}
