//! The cars, drawn (S245): the monster truck, the baja and the dune buggy, each in its livery, its
//! four tires turned by their spin and their steer and carried by their suspension, and the seat its
//! driver sits in.
//!
//! **Where it lives.** All in `machine.res`, beside the car's `.phy`: the models
//! `monstertruck\anim\monstertruck.psg`, `baja\anim\baja.psg`, `buggy\anim\buggy.psg`, each with a
//! `2` livery (and the `_ss` split-screen pair, which the port does not need), and their textures.
//!
//! **Two rigs.** The original poses the cars with two measure steps, one for the truck and one the
//! baja and the buggy share, writing node matrices off the tires' state every frame (the tire's
//! compression, spin angle and steer angle, and the `.phy`'s `MaxCompression` and `MaxTireScale`).
//! [`pose`] tells them apart by the truck's root-level hubs.
//!
//! * **The truck** ([`HUBS`]): each hub, a child of the root, is put at its bind place lifted by its
//!   tire's compression and turned about y by its steer (the rears' by the steer times
//!   `RearTireSteerScale`, which the caller gives); each tire under it spins about x and is
//!   stretched in y toward `MaxTireScale` as it is compressed. Each lower strut is lifted with its
//!   hub. The two axles are lifted by their pair's mean compression and rolled about z to meet both
//!   hubs; each drive line is aimed at its axle node, and each swingarm at its swingarm node, which
//!   rides the lower strut.
//! * **The baja and the buggy** ([`ARMS`]): each front tire hangs under a wheel hub under the upper
//!   arm. The hub is turned about y by the steer, the tire spun about x; the upper arm swings about
//!   z by 1.25 × compression ÷ (the front right hub's distance from its upper arm) radians, up on
//!   each side, and the hub is turned back by the same so it stays upright. The lower arm is aimed
//!   at the hub's bottom node. The rear tires hang under one swing arm, which is aimed at the higher
//!   of the two rear tires' compressions; they spin about x and are stretched as the truck's. Every
//!   strut is a pair of nodes, each aimed at the other.
//!
//! Every aim here is the port's own form of the original's: the node turned, about its own axis
//! where the original turns about one, by the turn that takes the target from where it was in the
//! bind pose to where it is now, so a car at rest is drawn exactly as the model is. The original
//! builds the aims' frames from scratch (an up vector and the target); the two agree by eye.
//!
//! **The driver.** The seat is where the plane's and the helicopter's pilots sit to the model's four
//! limb nodes ([`SEAT`]), carried to the car ([`seat`]). The `.phy`'s `[Animation]`
//! `PelvisBoxPosition` is not that place (as the plane's and the Kiowa's are not, P216b): it is an
//! offset the rider's own body step reads.

use crate::disc::Res;
use crate::mat::{M4, V3, v3};
use crate::psg::Model;
use crate::vehicle::about;
use crate::world::Assets;

/// The archive the cars' models and textures are in.
pub const ARCHIVE: &str = "machine.res";

/// The four tire nodes, front left, front right, rear left, rear right: the car's own wheel order.
pub const TIRES: [&str; 4] = ["FrontLeftTire", "FrontRightTire", "RearLeft_Tire", "RearRight_Tire"];

/// The truck's four hubs, in [`TIRES`]' order, each a child of the root and the parent of its tire.
pub const HUBS: [&str; 4] = ["FrontLeftHub", "FrontRightHub", "RearLeftHub", "RearRightHub"];

/// The baja's and the buggy's front arms, left then right: upper arm, lower arm, wheel hub, the
/// hub's bottom node (the lower arm's target).
pub const ARMS: [[&str; 4]; 2] = [
    ["FrontLeft_UpperARM", "FrontLeft_LowerARM", "FrontLeftWheelHub", "FrontLeftBottomHubNODE"],
    ["FrontRight_UpperARM", "FrontRight_LowerARM", "FrontRightWheelHub", "FrontRightBottomHubNODE"],
];

/// The node the baja's and the buggy's rear tires hang under.
pub const SWING_ARM: &str = "SwingArm";

/// How far the original swings an upper arm for a foot of compression, over its length.
const ARM_SWING: f32 = 1.25;

/// The four nodes the driver's limbs are held to: the two feet, then the two hands, as the plane's
/// and the helicopter's.
pub const SEAT: [&str; 4] = ["Dummy_Foot_L_pos", "Dummy_Foot_R_pos", "Dummy_Hand_L_pos", "Dummy_Hand_R_pos"];

/// Where a pilot's root sits from the middle of the four [`SEAT`] nodes, in the machine's space: the
/// plane's measured seat (`crate::biplane::PILOT_SEAT`, 0, 3.5, -3) less the middle of its four, the
/// same to the thousandth as the Kiowa's ([`crate::kiowa::PILOT_SEAT`] to its four).
pub const SEAT_FROM_LIMBS: V3 = v3(0.061, -0.168, -1.176);

/// One tire as the pose reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tire {
    /// The spin angle, radians, about the tire's x.
    pub spin: f32,
    /// The steer angle, radians, about y.
    pub steer: f32,
    /// How far the suspension is compressed, feet.
    pub compression: f32,
    /// The tire's stretch in y ([`squash`]).
    pub scale: f32,
}

impl Tire {
    /// A tire at rest: unturned, unsprung, unstretched.
    pub const REST: Tire = Tire { spin: 0.0, steer: 0.0, compression: 0.0, scale: 1.0 };
}

/// A tire's stretch in y at `compression` of `max_compression`: one at rest, `max_scale`
/// (`MaxTireScale`) fully compressed, in a straight line between, and never past either.
pub fn squash(compression: f32, max_compression: f32, max_scale: f32) -> f32 {
    if max_compression <= 0.0 {
        return 1.0;
    }
    let s = 1.0 + compression / max_compression * (max_scale - 1.0);
    s.clamp(max_scale.min(1.0), max_scale.max(1.0))
}

/// A loaded car model: the name it is kept under in [`Assets`].
#[derive(Clone, Debug, PartialEq)]
pub struct Car {
    pub model: String,
}

impl Car {
    /// The key a car model `name` (`monstertruck`, `baja2`, …) is kept under: its own, so a path car
    /// of the same name is not taken for it.
    pub fn key(name: &str) -> String {
        format!("car:{name}")
    }

    /// Load the car model `name` into `assets`, from the first of `archives` to hold it. `None` if
    /// it is not there.
    pub fn load(assets: &mut Assets, archives: &[Res], name: &str) -> Option<Car> {
        let key = Car::key(name);
        assets.load_as(archives, name, &key).then_some(Car { model: key })
    }

    /// The car model `name` as [`Car::load`] left it in `assets`.
    pub fn found(assets: &Assets, name: &str) -> Option<Car> {
        let key = Car::key(name);
        assets.models.contains_key(&key).then_some(Car { model: key })
    }

    /// The model's nodes posed by `tires` ([`pose`]); empty if the model is not in `assets`.
    pub fn pose(&self, assets: &Assets, tires: &[Tire; 4]) -> Vec<M4> {
        assets.models.get(&self.model).map_or(Vec::new(), |l| pose(&l.model, tires))
    }

    /// Where the driver's root sits in the car's space ([`seat`]).
    pub fn seat(&self, assets: &Assets) -> Option<V3> {
        seat(&assets.models.get(&self.model)?.model)
    }
}

/// Where the driver's root sits in the car's space: the middle of the four [`SEAT`] nodes and
/// [`SEAT_FROM_LIMBS`]. `None` for a model without all four.
pub fn seat(model: &Model) -> Option<V3> {
    let mut sum = v3(0.0, 0.0, 0.0);
    for name in SEAT {
        sum = sum.add(node_at(model, name)?);
    }
    Some(sum.scale(0.25).add(SEAT_FROM_LIMBS))
}

/// Where a node of the model sits in the model's own space, by name.
pub fn node_at(model: &Model, name: &str) -> Option<V3> {
    let i = find(model, name)?;
    Some(model.world(i).point(v3(0.0, 0.0, 0.0)))
}

fn find(model: &Model, name: &str) -> Option<usize> {
    model.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name))
}

/// Every node's matrix in the model's space, `local` composed down the tree.
fn compose(model: &Model, local: &[M4]) -> Vec<M4> {
    (0..model.nodes.len())
        .map(|node| {
            let (mut m, mut at, mut guard) = (M4::IDENTITY, Some(node), 0);
            while let Some(i) = at.filter(|&i| i < local.len() && guard <= local.len()) {
                m = m.mul(local[i]);
                at = model.nodes[i].parent;
                guard += 1;
            }
            m
        })
        .collect()
}

fn origin(m: M4) -> V3 {
    v3(m.0[3][0], m.0[3][1], m.0[3][2])
}

/// A point of the model's space in the frame `m` places, as the frame's own (its rows may be
/// scaled).
fn into(m: M4, p: V3) -> V3 {
    let d = p.sub(origin(m));
    let row = |i: usize| v3(m.0[i][0], m.0[i][1], m.0[i][2]);
    let along = |i: usize| {
        let r = row(i);
        let l = r.dot(r);
        if l > 1e-12 { d.dot(r) / l } else { 0.0 }
    };
    v3(along(0), along(1), along(2))
}

/// Turn a node about its own `axis` by `angle`, its place kept.
fn turn(local: &mut M4, axis: V3, angle: f32) {
    *local = about(axis, angle).mul(*local);
}

/// The turn that takes the direction `from` to `to`, about `axis` only where one is given (both
/// flattened across it), or the shortest. Nothing for a direction too short to read.
fn turn_between(from: V3, to: V3, axis: Option<V3>) -> Option<M4> {
    let flat = |v: V3| match axis {
        Some(a) => v.sub(a.scale(v.dot(a))),
        None => v,
    };
    let (a, b) = (flat(from), flat(to));
    if a.len() < 1e-6 || b.len() < 1e-6 {
        return None;
    }
    let (a, b) = (a.norm(), b.norm());
    let cross = a.cross(b);
    let angle = cross.len().atan2(a.dot(b));
    if angle.abs() < 1e-7 {
        return None;
    }
    let about_axis = match axis {
        Some(ax) => ax.scale(cross.dot(ax).signum()),
        None => cross.norm(),
    };
    Some(about(about_axis, angle))
}

/// Aim node `i` at `target` as it was aimed at `bind_target` in the bind pose: turned in its own
/// frame (about its own `axis` only, where given) by what takes the one to the other. `now` is every
/// node's matrix as posed so far and `bind` the model's own.
fn aim(local: &mut [M4], i: usize, now: &[M4], bind: &[M4], target: V3, bind_target: V3, axis: Option<V3>) {
    let was = into(bind[i], bind_target);
    let is = into(now[i], target);
    if let Some(r) = turn_between(was, is, axis) {
        local[i] = r.mul(local[i]);
    }
}

/// The tire's matrix spun about its own x and stretched in its parent's y about its own middle.
fn spin_and_stretch(local: &mut M4, tire: &Tire, stretch: bool) {
    turn(local, v3(1.0, 0.0, 0.0), tire.spin);
    if stretch && (tire.scale - 1.0).abs() > 1e-6 {
        let keep = local.0[3];
        let mut s = M4::IDENTITY;
        s.0[1][1] = tire.scale;
        *local = local.mul(s);
        local.0[3] = keep;
    }
}

/// The car's model posed by its four tires (in [`TIRES`]' order), in the model's own space and node
/// order: what the GPU skins the car with and the software path skins a copy into. A node the model
/// does not have is left as it is.
pub fn pose(model: &Model, tires: &[Tire; 4]) -> Vec<M4> {
    let mut local: Vec<M4> = model.nodes.iter().map(|n| n.local).collect();
    let bind = compose(model, &local);
    if find(model, HUBS[0]).is_some() {
        truck(model, &mut local, &bind, tires);
    } else {
        arms(model, &mut local, &bind, tires);
    }
    compose(model, &local)
}

fn lift(local: &mut M4, by: f32) {
    local.0[3][1] += by;
}

/// The truck's rig: hubs and lower struts lifted, axles lifted and rolled, then the drive lines and
/// swingarms aimed.
fn truck(model: &Model, local: &mut [M4], bind: &[M4], tires: &[Tire; 4]) {
    let at = |name: &str| find(model, name);
    for (k, tire) in tires.iter().enumerate() {
        if let Some(h) = at(HUBS[k]) {
            lift(&mut local[h], tire.compression);
            turn(&mut local[h], v3(0.0, 1.0, 0.0), tire.steer);
        }
        if let Some(t) = at(TIRES[k]) {
            spin_and_stretch(&mut local[t], tire, true);
        }
        let side = ["FrontLeft", "FrontRight", "RearLeft", "RearRight"][k];
        if let Some(s) = at(&format!("{side}LowerStrut")) {
            lift(&mut local[s], tire.compression);
        }
    }
    let now = compose(model, local);
    for (axle, pair) in [("FrontAxle", [0, 1]), ("RearAxle", [2, 3])] {
        let (Some(a), Some(right)) = (at(axle), at(HUBS[pair[1]])) else { continue };
        let mean = (tires[pair[0]].compression + tires[pair[1]].compression) / 2.0;
        lift(&mut local[a], mean);
        // Lifted, the axle meets the right hub where its own compression put it.
        let mut lifted = now.to_vec();
        lifted[a].0[3][1] += mean;
        aim(local, a, &lifted, bind, origin(now[right]), origin(bind[right]), Some(v3(0.0, 0.0, 1.0)));
    }
    let now = compose(model, local);
    let mut aims: Vec<(String, String)> = vec![("FrontDriveLine".into(), "FrontAxleNode".into()), ("RearDriveLine".into(), "RearAxleNode".into())];
    for side in ["FrontLeft", "FrontRight", "RearLeft", "RearRight"] {
        for level in ["Upper", "Lower"] {
            let arm = format!("{side}{level}Swingarm");
            aims.push((arm.clone(), format!("{arm}Node")));
        }
    }
    for (node, target) in aims {
        if let (Some(i), Some(t)) = (at(&node), at(&target)) {
            aim(local, i, &now, bind, origin(now[t]), origin(bind[t]), None);
        }
    }
}

/// The baja's and the buggy's rig: the front arms swung and the hubs steered, the lower arms aimed,
/// the swing arm aimed at the rear tires, the tires spun, then every strut pair aimed at each other.
fn arms(model: &Model, local: &mut [M4], bind: &[M4], tires: &[Tire; 4]) {
    let at = |name: &str| find(model, name);
    let z = v3(0.0, 0.0, 1.0);
    // How far an arm swings for a foot: the front right hub's distance from its upper arm.
    let length = match (at(ARMS[1][0]), at(ARMS[1][2])) {
        (Some(u), Some(h)) => origin(bind[h]).sub(origin(bind[u])).len(),
        _ => 0.0,
    };
    for (side, names) in ARMS.iter().enumerate() {
        let tire = &tires[side];
        let (Some(upper), Some(hub)) = (at(names[0]), at(names[2])) else { continue };
        // Up on each side: the way the hub lies from the arm's pivot, across.
        let out = into(bind[upper], origin(bind[hub])).x.signum();
        let swing = if length > 1e-6 { ARM_SWING * tire.compression / length } else { 0.0 } * out;
        turn(&mut local[upper], z, swing);
        turn(&mut local[hub], z, -swing);
        turn(&mut local[hub], v3(0.0, 1.0, 0.0), tire.steer);
        if let Some(t) = at(TIRES[side]) {
            spin_and_stretch(&mut local[t], tire, false);
        }
    }
    let now = compose(model, local);
    for names in ARMS {
        if let (Some(lower), Some(bottom)) = (at(names[1]), at(names[3])) {
            aim(local, lower, &now, bind, origin(now[bottom]), origin(bind[bottom]), Some(z));
        }
    }
    if let (Some(arm), Some(rl)) = (at(SWING_ARM), at(TIRES[2])) {
        let up = tires[2].compression.max(tires[3].compression);
        let rest = origin(bind[rl]);
        let rest = v3(0.0, rest.y, rest.z);
        aim(local, arm, &now, bind, rest.add(v3(0.0, up, 0.0)), rest, Some(v3(1.0, 0.0, 0.0)));
    }
    for k in 2..4 {
        if let Some(t) = at(TIRES[k]) {
            spin_and_stretch(&mut local[t], &tires[k], true);
        }
    }
    let now = compose(model, local);
    for side in ["FrontLeft", "FrontRight", "RearLeft", "RearRight"] {
        for n in ["01", "02"] {
            let (Some(u), Some(l)) = (at(&format!("{side}Strut_{n}_Upper")), at(&format!("{side}Strut_{n}_Lower"))) else { continue };
            aim(local, u, &now, bind, origin(now[l]), origin(bind[l]), None);
            aim(local, l, &now, bind, origin(now[u]), origin(bind[u]), None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::psg::Node;

    fn node(name: &str, parent: Option<usize>, at: V3) -> Node {
        let mut local = M4::IDENTITY;
        local.0[3] = [at.x, at.y, at.z, 1.0];
        Node { name: name.into(), local, parent, center: at, half_extents: v3(1.0, 1.0, 1.0), radius: 1.0 }
    }

    fn model(nodes: Vec<Node>) -> Model {
        Model { nodes, materials: Vec::new(), meshes: Vec::new(), groups: 1, group_distance: vec![f32::MAX] }
    }

    fn close(a: V3, b: V3) -> bool {
        (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4 && (a.z - b.z).abs() < 1e-4
    }

    /// A truck in miniature: two front hubs with their tires, the front axle, a lower strut and its
    /// swingarm node, the swingarm.
    fn truck_model() -> Model {
        model(vec![
            node("MonsterTruck", None, v3(0.0, 0.0, 0.0)),
            node(HUBS[0], Some(0), v3(-4.0, 3.0, 6.0)),
            node(HUBS[1], Some(0), v3(4.0, 3.0, 6.0)),
            node(TIRES[0], Some(1), v3(-1.4, 0.0, 0.0)),
            node(TIRES[1], Some(2), v3(1.4, 0.0, 0.0)),
            node("FrontAxle", Some(0), v3(0.0, 3.0, 6.0)),
            node("FrontLeftLowerStrut", Some(0), v3(0.0, 4.0, 6.0)),
            node("FrontLeftLowerSwingarmNode", Some(6), v3(0.0, -1.0, 0.0)),
            node("FrontLeftLowerSwingarm", Some(0), v3(0.0, 5.0, 1.0)),
        ])
    }

    #[test]
    fn at_rest_a_car_is_its_model() {
        for m in [truck_model(), arms_model()] {
            let p = pose(&m, &[Tire::REST; 4]);
            for (i, posed) in p.iter().enumerate() {
                let w = m.world(i);
                for r in 0..4 {
                    for c in 0..4 {
                        assert!((posed.0[r][c] - w.0[r][c]).abs() < 1e-5, "node {i}: {posed:?} against {w:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_truck_hub_rises_with_its_compression_and_the_axle_rolls_to_meet_it() {
        let m = truck_model();
        let mut tires = [Tire::REST; 4];
        tires[1].compression = 1.0;
        let p = pose(&m, &tires);
        assert!(close(origin(p[2]), v3(4.0, 4.0, 6.0)), "{:?}", origin(p[2]));
        assert!(close(origin(p[1]), v3(-4.0, 3.0, 6.0)));
        // The axle, lifted by the pair's mean, points its x up toward the raised right hub.
        let axle = p[5];
        assert!((origin(axle).y - 3.5).abs() < 1e-4);
        let x = v3(axle.0[0][0], axle.0[0][1], axle.0[0][2]);
        assert!((x.y - (1.0f32 / 8.0).atan().sin()).abs() < 1e-4, "{x:?}");
        // The lower strut rises with nothing (its tire is the left), and the swingarm still aims at
        // its node as it did.
        assert!(close(origin(p[6]), v3(0.0, 4.0, 6.0)));
    }

    #[test]
    fn a_tire_spins_about_x_and_a_hub_steers_about_y() {
        let m = truck_model();
        let mut tires = [Tire::REST; 4];
        tires[0].spin = std::f32::consts::FRAC_PI_2;
        tires[0].steer = 0.3;
        let p = pose(&m, &tires);
        // The tire's place swings round the hub by the steer.
        let t = origin(p[3]);
        let d = t.sub(v3(-4.0, 3.0, 6.0));
        assert!((d.len() - 1.4).abs() < 1e-4 && d.z.abs() > 0.3, "{d:?}");
        // Its own y now lies along the hub's z, a quarter turn about x.
        let y = v3(p[3].0[1][0], p[3].0[1][1], p[3].0[1][2]);
        assert!(y.y.abs() < 1e-4 && (y.len() - 1.0).abs() < 1e-4, "{y:?}");
    }

    #[test]
    fn a_squash_is_one_at_rest_and_the_scale_fully_compressed() {
        assert_eq!(squash(0.0, 2.0, 1.2), 1.0);
        assert!((squash(2.0, 2.0, 1.2) - 1.2).abs() < 1e-6);
        assert!((squash(5.0, 2.0, 1.2) - 1.2).abs() < 1e-6);
        assert!((squash(1.0, 2.0, 0.8) - 0.9).abs() < 1e-6);
        assert_eq!(squash(1.0, 0.0, 1.2), 1.0);
    }

    /// A baja in miniature: the front right arm and hub with its tire and bottom node, the lower
    /// arm, and the swing arm with a rear tire.
    fn arms_model() -> Model {
        model(vec![
            node("Baja", None, v3(0.0, 0.0, 0.0)),
            node(ARMS[1][0], Some(0), v3(0.4, 3.0, 6.0)),
            node(ARMS[1][2], Some(1), v3(1.6, -1.5, 0.0)),
            node(TIRES[1], Some(2), v3(0.8, 0.0, 0.0)),
            node(ARMS[1][3], Some(2), v3(0.0, -0.2, 0.0)),
            node(ARMS[1][1], Some(0), v3(0.3, 2.4, 6.0)),
            node(SWING_ARM, Some(0), v3(0.0, 2.2, 0.0)),
            node(TIRES[2], Some(6), v3(-3.2, -0.7, -5.0)),
        ])
    }

    #[test]
    fn an_arm_swings_its_hub_up_and_keeps_it_upright() {
        let m = arms_model();
        let mut tires = [Tire::REST; 4];
        tires[1].compression = 0.5;
        let p = pose(&m, &tires);
        let rest = m.world(2);
        assert!(origin(p[2]).y > origin(rest).y + 0.3, "{:?} from {:?}", origin(p[2]), origin(rest));
        // Upright: the hub's own axes are the model's.
        let y = v3(p[2].0[1][0], p[2].0[1][1], p[2].0[1][2]);
        assert!(close(y, v3(0.0, 1.0, 0.0)), "{y:?}");
        // And the lower arm turned up after it: its x now climbs.
        let x = v3(p[5].0[0][0], p[5].0[0][1], p[5].0[0][2]);
        assert!(x.y > 0.05, "{x:?}");
    }

    #[test]
    fn the_swing_arm_lifts_the_rear_tires_with_the_higher_compression() {
        let m = arms_model();
        let mut tires = [Tire::REST; 4];
        tires[3].compression = 0.6;
        let p = pose(&m, &tires);
        let (was, is) = (origin(m.world(7)), origin(p[7]));
        assert!((is.y - was.y - 0.6).abs() < 0.1, "{was:?} -> {is:?}");
    }
}
