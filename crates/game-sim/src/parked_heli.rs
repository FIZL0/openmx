//! **The parked helicopter in the riders' world** (S494a): free06's Kiowa, parked with nobody aboard
//! (F80), as the bikes meet it.
//!
//! S494's capture `heli1.csv` (`research/s494/index.md`): the original's bike ridden into free06's
//! parked helicopter strikes its hull, and the helicopter's body, awake all along (R52: its sleep
//! thresholds are never met), takes the push: v (−0.94, −0.27, 2.22) ft/s on the strike, then the
//! bike, pressed against it, pushes it ~4.2 ft. The same as the plane ([`crate::parked_plane`]).
//!
//! So a parked helicopter is a [`ParkedHeli`]: the helicopter's frame unflown
//! ([`crate::kiowa::Kiowa::tick`] over [`crate::kiowa::Grounded`], its collision object searched
//! first, as the shell settles it), held in [`crate::Sim::parked_helis`], stepped every frame ahead
//! of the bikes after the parked planes, its hull (`kiowaf.col`) in the bikes' list after theirs
//! ([`ParkedHeli::hull`]), and its body on each bike's resolve after theirs ([`ParkedHeli::flown`]),
//! written back after the bike's frame ([`ParkedHeli::pushed`]). What its crash test asks for is
//! left undone, as the settle leaves it: nobody aboard, nothing to wreck.

use crate::kiowa::{Collision, Kiowa, Loop, Measure, Phy, Ride};
use crate::rng::Rng;
use crate::world::World;
use ps2_float::{F, Vf};
use std::sync::Arc;

/// **A parked helicopter**: its flight, the frame's state round it, its collision object, the
/// `rand()` its crash test draws from, the rotor's wreck rays its strike test asks, and its hull.
#[derive(Clone, Debug)]
pub struct ParkedHeli {
    pub kiowa: Kiowa,
    pub ride: Ride,
    pub collision: Collision,
    pub rng: Rng,
    /// The rotor's wreck rays (`[Wrecks]`, [`crate::kiowa::WreckRotor::rays`]): a constant of the
    /// file, kept off the wire.
    pub rotor: [[Vf; 2]; 8],
    /// Its hull as the bikes meet it, `kiowaf.col` ([`crate::kiowa::KIOWA_HULL`]): kept off the wire,
    /// so `None` only on one just read off it, before this machine's is put back.
    pub hull: Option<Arc<crate::track::Mesh>>,
}

/// The helicopter's `rand()` as the shell's built one seeds it.
pub const RNG_SEED: u32 = 0x0216_6b69;

impl ParkedHeli {
    /// **The helicopter as the shell builds and parks it** (F80's `parked_heli`): the constructor's
    /// helicopter ([`crate::kiowa::BUILT_INERTIA`] and on) with its model's origin put so its body's
    /// centre is on `body_at` along `facing`, upright and at rest, unflown; its collision object off
    /// `rays` (`kiowam.col`), its nearby list gathered where it stands; then `frames` frames of `dt`
    /// ([`ParkedHeli::frame`]). The placement's arithmetic is the port's own (host floats), as the
    /// shell's is: no frame of the original's reads it.
    #[allow(clippy::too_many_arguments)]
    pub fn settled(
        phy: &Phy,
        rays: crate::flight_search::Rays,
        hull: crate::track::Mesh,
        body_at: Vf,
        facing: Vf,
        world: &mut dyn World,
        tables: &crate::ride::Tables,
        dt: F,
        frames: usize,
    ) -> ParkedHeli {
        let part = &phy.mass_properties;
        let up = Vf::new(0.0, 1.0, 0.0, 0.0);
        let quat = crate::math::mat4_to_quat(&crate::math::basis(body_at, facing, up, true, false));
        let c = rotate(quat, part.cofg);
        let origin = Vf::new(body_at.x() - c[0], body_at.y() - c[1], body_at.z() - c[2], 1.0);
        let quat = crate::math::mat4_to_quat(&crate::math::basis(origin, facing, up, true, false));
        let c = rotate(quat, part.cofg);
        let position = Vf::new(origin.x() + c[0], origin.y() + c[1], origin.z() + c[2], 1.0);
        let clear = Vf::new(0.0, 0.0, 0.0, 1.0);
        let mut body = crate::biplane::Body {
            enabled: true,
            inv_mass: if part.mass.0 > 0.0 { F(1.0 / part.mass.0) } else { F::ZERO },
            damping: Vf::new(1.0, 1.0, 1.0, 1.0),
            last_position: position,
            position,
            velocity: clear,
            last_force: clear,
            force: clear,
            shock_peak: clear,
            shock: clear,
            inv_inertia: crate::kiowa::BUILT_INV_INERTIA,
            ang_damping: Vf::new(1.0, 1.0, 1.0, 1.0),
            last_quat: quat,
            quat,
            cofg: part.cofg,
            moment: clear,
            last_moment: clear,
            rates: clear,
            twist_peak: clear,
            twist: clear,
            ..Default::default()
        };
        let mut measure = Measure::default();
        measure.flight(&body, &tables.acos);
        let kiowa = Kiowa {
            controller: phy.controller,
            rotor: phy.rotor,
            aero: phy.aero,
            control: Loop { filter: crate::kiowa::BUILT_FILTER, hold_altitude: F(position.y()), ..Loop::default() },
            ..Kiowa::default()
        };
        let mut collision = Collision::new(part, &body, crate::kiowa::BUILT_INERTIA, rays);
        collision.refresh(&body, world);
        // The parked one's body is put on the spot itself, after the build.
        (body.position, body.last_position) = (body_at, body_at);
        let ride = Ride {
            body,
            measure,
            crash: phy.crash(),
            flown: false,
            gravity_scale: part.gravity_scale,
            throw_scale: F::ZERO,
            gravity: part.gravity,
            area: world.area(),
            touchdown_sound: false,
            rotor_angle: F::ZERO,
            rotor_hidden: false,
        };
        let mut heli = ParkedHeli { kiowa, ride, collision, rng: Rng::from_seed(RNG_SEED), rotor: phy.wreck_rotor.rays, hull: Some(Arc::new(hull)) };
        heli.collision.refresh(&heli.ride.body, world);
        for _ in 0..frames {
            heli.frame(dt, world, tables);
        }
        heli
    }

    /// **One frame of the parked helicopter**: its collision object searched first, as the level's
    /// collision node searches every object before any dynamics world runs, then its frame unflown
    /// on the pad the last frame left ([`Kiowa::tick`] over [`crate::kiowa::Grounded`]). What its
    /// crash test asks for is left undone.
    pub fn frame(&mut self, dt: F, world: &mut dyn World, tables: &crate::ride::Tables) {
        let movers = self.collision.search(&mut self.ride.body, world, dt);
        let mut ground = crate::kiowa::Grounded { collision: &mut self.collision, world, movers: &movers, rotor: &self.rotor, asked: Vec::new() };
        self.kiowa.tick(&mut self.ride, &crate::kiowa::WORLD, None, &tables.curve, &tables.acos, &mut self.rng, dt, &mut ground);
    }

    /// Its hull as the riders' bikes meet it: `kiowaf.col` where its collision node stands it (the
    /// body's pose), the room its record has, and the helicopter's category
    /// ([`crate::kiowa::COLLIDABLE`]), for seat `seat`. `None` without its hull.
    pub fn hull(&self, seat: usize) -> Option<crate::rider_contact::Hull<'_>> {
        let mesh = self.hull.as_deref()?;
        let n = crate::wreck_bike::node_of(&self.ride.body);
        let at = crate::math::Mat4([n.rows[0], n.rows[1], n.rows[2], n.origin]);
        Some(crate::rider_contact::Hull { seat, mesh, at, capacity: self.collision.record.capacity, category: crate::kiowa::COLLIDABLE })
    }

    /// Its body as a bike's resolve pushes it ([`crate::rider_contact::Bump::planes`]): the copy
    /// nearest `from` (the bike) on a world that wraps, as a parked plane's is
    /// ([`crate::flight::Flight::flown`]).
    pub fn flown(&self, seat: usize, from: Vf, world: &dyn World) -> crate::rider_contact::Flown {
        let mut body = self.ride.body;
        let near = crate::nearest_copy(world, from, body.position);
        if near != body.position {
            let by = near.sub(body.position);
            body.position = near;
            body.last_position = body.last_position.add(by);
        }
        crate::rider_contact::Flown { seat, collider: self.collision.collider, body, mass: self.collision.mass, quad: None }
    }

    /// The body a bike's resolve pushed, written back: its speeds, spins and what the resolve wrote,
    /// where the helicopter really is, as a parked plane's is ([`crate::flight::Flight::pushed`]).
    pub fn pushed(&mut self, p: &crate::rider_contact::Flown) {
        let (position, last_position) = (self.ride.body.position, self.ride.body.last_position);
        self.ride.body = p.body;
        (self.ride.body.position, self.ride.body.last_position) = (position, last_position);
        self.collision.collider = p.collider;
    }
}

impl crate::Sim {
    /// Whether the Sim holds anything parked a lone bike can strike: a car (S485), a plane (S494) or
    /// a helicopter (S494a). A lone rider among them rides [`crate::Sim::step_all`].
    pub(crate) fn parks_fliers_or_cars(&self) -> bool {
        !self.parked_cars.is_empty() || !self.parked_planes.is_empty() || !self.parked_helis.is_empty()
    }

    /// The parked helicopters' bodies in player `me`'s bump, after the parked planes' (S494a): each
    /// one the copy nearest `me`'s bike.
    pub fn parked_helis_flown(&self, me: usize, world: &dyn World) -> Vec<crate::rider_contact::Flown> {
        let from = self.rider_of(me).ride.transform().0[3];
        let first = self.players() + self.parked_cars.len() + self.parked_planes.len();
        self.parked_helis.iter().enumerate().map(|(k, h)| h.flown(first + k, from, world)).collect()
    }

    /// **The map's parked helicopters as the riders' bikes meet them** (S494a, the module's
    /// comment), held in [`crate::Sim::parked_helis`]. [`crate::Sim::parked`], the AI riders' hulls,
    /// is the caller's ([`crate::Sim::park_cars_and_fliers`]).
    pub fn park_helis(&mut self, helis: Vec<ParkedHeli>) {
        self.parked_helis = helis.into_iter().filter(|h| h.hull.is_some()).collect();
    }

    /// The parked helicopters' frame, ahead of the bikes', after the parked planes' (S494a).
    pub(crate) fn parked_helis_frame(&mut self, dt: F, world: &mut dyn World) {
        let tables = &self.tables.ride;
        for h in &mut self.parked_helis {
            h.frame(dt, world, tables);
        }
    }
}

/// `v` turned by the quaternion `q`, in host floats: the shell's placement arithmetic.
fn rotate(q: Vf, v: Vf) -> [f32; 3] {
    let (qx, qy, qz, qw) = (q.x(), q.y(), q.z(), q.w());
    let (vx, vy, vz) = (v.x(), v.y(), v.z());
    let (tx, ty, tz) = (2.0 * (qy * vz - qz * vy), 2.0 * (qz * vx - qx * vz), 2.0 * (qx * vy - qy * vx));
    [vx + qw * tx + (qy * tz - qz * ty), vy + qw * ty + (qz * tx - qx * tz), vz + qw * tz + (qx * ty - qy * tx)]
}
