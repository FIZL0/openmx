//! What the chase camera is handed each frame: the machine with the controller, seen the way the
//! original's per-frame target writer sees it (P34).
//!
//! The race camera does not read the vehicle. It reads a small target block the vehicle fills once a
//! frame (`research/oracle.txt`, `vehicle_camera_target`): the seat position, the chassis' forward
//! and up rows, the velocity, the lean, a flag and the speed. The flag is the one that is easy to get
//! backwards. It is stored as `on_ground == 0` — the rider is in the *air* — and it is what tells the
//! camera to head along the velocity rather than along the bike's forward row. Handed the on-ground
//! flag instead, the camera took its heading off the forward row through every jump, and a backflip
//! turns that row over: the view swung round to the front of the bike at the top of the flip and
//! back again on the way down.
//!
//! This file is std and `game_sim` only, for the same reason `pad.rs` is: the ride test compiles it
//! in by path, because a binary crate has nothing a test can `use`.

use game_sim::camera::{Ridden, Target};
use game_sim::ride::Ride;
use ps2_float::{F, Vf};

use crate::vehicle::{Garage, Kind};

/// The camera's target for whichever machine is up: the bike's ride, or the parked plane.
pub fn ridden(garage: &Garage, ride: &Ride) -> Ridden {
    match (garage.up, &garage.plane) {
        // The plane, as the camera's target: where it is, its nose, world up, and no lean — a plane
        // has no rider's stick to lean on. S34 gives it an attitude and a speed. Its flag is the same
        // one: in the recording2 biplane save state the plane stands on its wheels and the block's
        // flag reads 0.
        (Kind::Biplane, Some(plane)) => Ridden {
            target: Target {
                body_point: plane.at,
                forward: plane.facing,
                up: Vf::new(0.0, 1.0, 0.0, 0.0),
                vel: plane.vel,
                lean: F::ZERO,
            },
            airborne: !plane.on_ground(),
            speed: plane.speed(),
        },
        _ => bike(ride),
    }
}

/// The bike's target block. The rows are the chassis quaternion's, which is the transform the
/// original's writer reads them from (the vehicle's `+0x18` node, P32's `*(bike+0x18) ==
/// *(chassis+0x254)`); the speed is the chassis' own `+0x34`, floored at zero as the writer floors
/// it (a length never goes below).
pub fn bike(ride: &Ride) -> Ridden {
    let (forward, up) = ride.axes();
    Ridden {
        target: Target {
            body_point: ride.position(),
            forward,
            up,
            vel: ride.chassis.vel,
            lean: ride.held.lean.0,
        },
        airborne: !ride.air.on_ground,
        speed: ride.chassis.speed,
    }
}
