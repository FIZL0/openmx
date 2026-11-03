//! The cars as the shell draws them (S245): each car's livery model posed by its tires — their spin,
//! their steer and their suspension's compression off the car's `game_sim::car::Machine` — at the
//! car's matrix, and its driver in the seat. The rigs are `game_render::car`'s; this is where they
//! meet the car's state and the draw.
//!
//! A car stands in its bay where the port parks it (its own track, `vehicle::Machine::home`),
//! see-through as every bay is ([`crate::vehicle::BAY_ALPHA`]) and at rest; while a player drives it
//! it is drawn solid at its matrix, by the GPU ([`actors`]) and by the software path ([`skinned`]).

use game_render::car::{Car, Tire, squash};
use game_render::gpu::Actor;
use game_render::mat::M4;
use game_render::world::Assets;

use crate::vehicle::{Bay, CarRide, Machine};

/// The model a car `machine` in `livery` is drawn with, as `Car::load` keeps it.
pub fn model_name(machine: Machine, livery: usize) -> &'static str {
    machine.models()[livery % crate::vehicle::LIVERIES]
}

/// Car `machine` in `livery` among the models the load put in `assets` (`drawn`), or in its first
/// livery where that one was not loaded (one player loads one), as the Kiowa's bays fall back.
pub fn find(drawn: &[Car], machine: Machine, livery: usize) -> Option<&Car> {
    let of = |l| {
        let key = Car::key(model_name(machine, l));
        drawn.iter().find(|c| c.model == key)
    };
    of(livery).or_else(|| of(0))
}

/// The four tires as the pose reads them off a driven car: each tire's spin angle and its
/// compression, its stretch toward the `.phy`'s `MaxTireScale` by that compression of its
/// `MaxCompression`, and its steer (the truck's rears' times `RearTireSteerScale`, as the
/// original's truck pose turns them). At rest for a car not out.
pub fn tires(ride: &CarRide) -> [Tire; 4] {
    let Some(m) = ride.driven.as_ref() else { return [Tire::REST; 4] };
    let rear = ride.car.animation.rear_tire_steer_scale.map_or(1.0, |s| s.0);
    std::array::from_fn(|k| {
        let (s, spec) = (&m.wheels[k].s, &ride.car.tires[k]);
        Tire {
            spin: s.angle.0,
            steer: s.steer.0 * if k >= 2 { rear } else { 1.0 },
            compression: s.compression.0,
            scale: squash(s.compression.0, spec.max_compression.0, spec.max_tire_scale.0),
        }
    })
}

/// Where a driven car is drawn: its matrix, which is the model's placement (rows x, y, z, origin).
pub fn placed(m: &game_sim::car::Machine) -> M4 {
    M4(std::array::from_fn(|r| std::array::from_fn(|c| m.matrix.0[r].0[c])))
}

/// Where a parked car stands in its bay: the model's origin on the bay's spot, nose along its
/// heading, upright.
pub fn bay_at(bay: &Bay) -> M4 {
    let v = |p: ps2_float::Vf| game_render::mat::v3(p.x(), p.y(), p.z());
    M4::placement(v(bay.spawn.at), v(bay.spawn.facing), game_render::mat::v3(0.0, 1.0, 0.0))
}

/// A car placed at `at` as the GPU draws it: the model skinned by its tires' pose, pushed on the end
/// of `poses`, which the actor names. A car casts no shadow, as no machine does.
pub fn actors<'a>(drawn: &'a Car, assets: &Assets, tires: &[Tire; 4], at: M4, alpha: f32, poses: &mut Vec<Vec<M4>>) -> Vec<Actor<'a>> {
    let pose = drawn.pose(assets, tires);
    let posed = (!pose.is_empty()).then(|| {
        poses.push(pose);
        poses.len() - 1
    });
    vec![Actor { model: drawn.model.as_str(), at, pose: posed, lod_from: None, alpha, casts: false }]
}

/// The driven car as the software path draws it, as `kiowa_draw::skinned` the helicopter: the model
/// skinned by its tires' pose into `copy`, the seat's own copy of it (made here the first time).
pub fn skinned<'a>(drawn: &'a Car, copy: &'a str, assets: &mut Assets, tires: &[Tire; 4], at: M4) -> (&'a str, M4) {
    let pose = drawn.pose(assets, tires);
    if !assets.models.contains_key(copy)
        && let Some(own) = assets.models.get(&drawn.model).cloned()
    {
        assets.models.insert(copy.to_string(), own);
    }
    match (assets.models.get_mut(copy), pose.is_empty()) {
        (Some(loaded), false) => {
            loaded.tris = game_render::skin::skin(&loaded.model, &pose, 0);
            (copy, at)
        }
        _ => (drawn.model.as_str(), at),
    }
}

/// `--trace-picture`'s line for a driven car drawn this frame: the renderer, the model and where its
/// origin stands.
pub fn trace(p: usize, renderer: &str, drawn: &Car, at: M4, tires: &[Tire; 4]) {
    let o = at.0[3];
    let t: Vec<String> = tires.iter().map(|t| format!("{:.2}/{:.2}/{:.2}/{:.2}", t.spin, t.steer, t.compression, t.scale)).collect();
    println!("picture car: seat {p} {renderer} {} at {:.2} {:.2} {:.2} tires {}", drawn.model, o[0], o[1], o[2], t.join(" "));
}

/// `--trace-picture`'s line for a parked car's bay drawn this frame: the model and where it stands.
pub fn trace_bay(drawn: &Car, at: M4) {
    let o = at.0[3];
    println!("picture bay: {} at {:.2} {:.2} {:.2} alpha {}", drawn.model, o[0], o[1], o[2], crate::vehicle::BAY_ALPHA);
}
