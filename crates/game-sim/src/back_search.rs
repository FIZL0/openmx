//! **The put-back's mesh query** (S83): what the original asks when its record update looks for
//! anything solid over a spot or in the way of the ring round it ([`crate::ride::Ride::keep_back`]).
//!
//! It is not a sweep against the scene's props. The original sweeps the bundle only against the
//! bike's own search list — gathered by the broad phase ([`broad_phase::gather`]) around the bundle,
//! and gathered again only once the bundle has moved more than ten feet from where it last was — and
//! of that list only what is nearer than the bundle's radius:
//!
//! - a collidable that a nearby search may strike and that is not in the categories the query
//!   masks out (the bikes'), swept one at a time, in its own space, into a record each sweep clears
//!   but for its points and normals;
//! - a placed group with parts, which refuses the spot when its cell is within twenty feet of the
//!   bundle's centre on the ground plane;
//! - and, in a race, each checkpoint, which refuses it when the centre is within the checkpoint's
//!   quarter radius times its radius, plus a hundred, squared on the ground plane.
//!
//! An object that met the bundle lifts the query's answer to probe 0's point, whichever object left
//! it there — the sweep does not clear the points, so an object whose probe 0 missed hands over the
//! point the object before it left (or the zero the query started with). Any other probe meeting
//! anything refuses. What the query hands back is in the record the port's [`World::solid`] fills,
//! so [`crate::ride`] reads it the same way: probe 0 met with the highest point and its normal, the
//! other probes by whether any object met them.
//!
//! The broad phase's world is the plane's ([`flight_search::world_wooded`]) with the bundle in the
//! ray probe's slot; it is built only when the list is gathered.
//!
//! [`World::solid`]: crate::world::World::solid

#![deny(clippy::float_arithmetic)]

use crate::broad_phase::{self, Search};
use crate::collide::{self, Bundle, MeshRef, Probe, Record};
use crate::eco::Trees;
use crate::flight_search;
use crate::math::Mat4;
use crate::world::Prop;
use crate::world_search::Collidable;
use ps2_float::vu::{Dest, Field};
use ps2_float::{Acc, F, Vf};

/// How far, squared, the bundle may move from where the list was last gathered before it is
/// gathered again: the bike's constructor's ten feet, never written again.
pub const REACH: f32 = 100.0;

/// The categories the query never sweeps (the bikes'), a word of the game's own data.
pub const MASK: u32 = 0x1_0000;

/// A placed group refuses the spot within this, squared, of its cell.
pub const CROWDED: f32 = 400.0;

/// The bundle's collidable as the gather sees it: enabled, struck by nothing it filters, colliding
/// with every category.
const BUNDLE_FLAGS: u8 = 0x46;

/// The broad phase's kinds of structure the gather asks: static and moving, or in a race whose
/// object says so, static only.
const BOTH: u32 = 3;
const STATIC: u32 = 1;

/// The query's own state on the bike: where the bundle was when the list was last gathered, how far
/// the last query found it had moved since (squared), and the list.
#[derive(Clone, Debug, PartialEq)]
pub struct Cache {
    pub at: Vf,
    pub moved: F,
    pub list: Search,
    /// **The port's own:** how many props and placed groups the world the list was gathered in had.
    /// The original has one world; a port world that changes under the bike gathers again.
    pub world: [u32; 2],
}

impl Default for Cache {
    /// As the bike is built: at the origin, nothing moved, the plane's empty list (the same three
    /// numbers: ten feet least, thirty extra, 128 kept).
    fn default() -> Cache {
        Cache { at: Vf::ZERO, moved: F::ZERO, list: flight_search::search(), world: [0; 2] }
    }
}

/// One of a race's checkpoints, as the query reads it: its centre (`x` and `z`) and radius.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Checkpoint {
    pub at: Vf,
    pub radius: F,
}

/// The race the bike is in, when it is: whether its gather is of the static structures only, and
/// whether its checkpoints refuse (the race running, with its checkpoint flag up), and which.
#[derive(Clone, Copy, Debug, Default)]
pub struct Race<'a> {
    pub static_only: bool,
    pub checkpoints: Option<&'a [Checkpoint]>,
}

/// What the query sweeps: the track's props, the ecosystem's plants, and the field's box the broad
/// phase's tree is laid over.
#[derive(Clone, Copy)]
pub struct Scene<'a> {
    pub props: &'a [Prop],
    pub trees: Option<&'a Trees>,
    pub field: Option<([F; 2], [F; 2])>,
}

/// What one object that met the bundle left: which prop, and the record as the sweep left it.
pub type Met<'a> = (usize, &'a Record);

/// Ask the query of `bundle`: `true` when it refuses. `found` gets the answer as the port's
/// [`crate::world::World::solid`] gives one (see the module's notes).
pub fn query(scene: Scene<'_>, bundle: &Bundle, cache: &mut Cache, race: Race<'_>, found: &mut Record) -> bool {
    query_each(scene, bundle, cache, race, found, |_| {})
}

/// [`query`], telling `each` about every object that met the bundle, in the order it swept them.
pub fn query_each(
    scene: Scene<'_>,
    bundle: &Bundle,
    cache: &mut Cache,
    race: Race<'_>,
    found: &mut Record,
    mut each: impl FnMut(Met<'_>),
) -> bool {
    gather(scene, bundle, cache, race);
    let groups = flight_search::FIRST + scene.props.len();
    let n = bundle.count.min(collide::MAX_PROBES);
    // The query's record, cleared once: everything the per-object sweep leaves alone starts at zero.
    let mut record = Record::new();
    let mut y = None::<F>;
    let mut refused = false;
    for c in &cache.list.nearby.found {
        if !(c.distance < bundle.radius) {
            continue;
        }
        if let Some(i) = c.object.checked_sub(flight_search::FIRST).filter(|&i| i < scene.props.len()) {
            // A prop is struck by nearby searches and is scenery, which the mask lets through; the
            // bikes it masks are not in the port's world.
            if !sweep_object(bundle, &scene.props[i], &mut record) {
                continue;
            }
            each((i, &record));
            if y.is_none_or(|y| !(F(record.point[0].y()) <= y)) {
                y = Some(F(record.point[0].y()));
                found.point[0] = record.point[0];
                found.normal[0] = record.normal[0];
            }
            found.hit[0] = true;
            for k in 1..n {
                if record.hit[k] {
                    found.hit[k] = true;
                    refused = true;
                }
            }
        } else if let Some(g) = scene.trees.and_then(|t| Some((t, t.groups().get(c.object.checked_sub(groups)?)?)))
            && g.1.solid
        {
            let (trees, g) = g;
            let at = |c: u16| F::from(i32::from(c)) * trees.cell();
            let (dx, dz) = (at(g.cell[0]) - F(bundle.centre.x()), at(g.cell[2]) - F(bundle.centre.z()));
            if Acc::mula(dx, dx).madd(dz, dz) <= F(CROWDED) {
                refused = true;
            }
        }
    }
    if let Some(checkpoints) = race.checkpoints {
        refused |= checkpoints.iter().any(|c| near(c, bundle.centre));
    }
    refused
}

/// Whether a checkpoint refuses a spot at `centre`: within `(r / 4) · r + 100`, squared, of it on
/// the ground plane, in the original's order.
pub fn near(c: &Checkpoint, centre: Vf) -> bool {
    let (dx, dz) = (F(c.at.x()) - F(centre.x()), F(c.at.z()) - F(centre.z()));
    let d = Acc::mula(dx, dx).madd(dz, dz);
    d <= F(100.0) + F(0.25) * c.radius * c.radius
}

/// The list, gathered again around the bundle when it has moved far enough (or into another world);
/// the bundle's move measured either way.
fn gather(scene: Scene<'_>, bundle: &Bundle, cache: &mut Cache, race: Race<'_>) {
    let groups = scene.trees.map_or(0, |t| t.groups().len());
    let world = [scene.props.len() as u32, groups as u32];
    let d = bundle.centre.sub(cache.at);
    cache.moved = Acc::mula(d.x(), d.x()).madda(d.y(), d.y()).madd(d.z(), d.z());
    if cache.moved <= F(REACH) && cache.world == world {
        return;
    }
    if cache.world != world {
        cache.list.built = false;
        cache.list.nearby.found.clear();
        cache.world = world;
    }
    let (mut broad, mut objects) =
        flight_search::world_wooded(scene.props, scene.trees, None, scene.field, bundle.centre, bundle.radius);
    objects[flight_search::RAY] = Collidable {
        flags: BUNDLE_FLAGS,
        category: 0,
        ..objects[flight_search::RAY].clone()
    };
    let kind = if race.static_only { STATIC } else { BOTH };
    broad_phase::gather(&mut objects, &mut broad, &mut cache.list, flight_search::RAY, kind);
    cache.at = bundle.centre;
}

/// The bundle against one prop, as the original's per-object sweep runs it: the two spheres, then
/// the probes into the prop's space, the mesh's sweep, and every probe that met something back out.
/// `record` keeps its points and normals from before; everything else is cleared first. `true` when
/// any probe met the mesh.
pub fn sweep_object(bundle: &Bundle, prop: &Prop, record: &mut Record) -> bool {
    let (at, _) = flight_search::prop_box(prop);
    let at = Vf::new(at[0].0, at[1].0, at[2].0, at[3].0);
    let d = bundle.centre.sub(at);
    let reach = prop.radius + bundle.radius;
    if !(Acc::mula(d.x(), d.x()).madda(d.y(), d.y()).madd(d.z(), d.z()) < reach * reach) {
        return false;
    }
    let n = bundle.count.min(collide::MAX_PROBES);
    record.hit = [false; collide::MAX_PROBES];
    record.fraction = [F(-1.0); collide::MAX_PROBES];
    record.hits = 0;
    record.nearest = F::ONE;
    let into = inverse(&prop.to_world);
    let mut local = Bundle { count: n, centre: point(&into, bundle.centre), radius: bundle.radius, ..*bundle };
    for (l, p) in local.probes[..n].iter_mut().zip(&bundle.probes[..n]) {
        *l = Probe { origin: point(&into, p.origin), dir: direction(&into, p.dir), length: p.length };
    }
    collide::sweep(&local, &MeshRef::of(&prop.mesh), record);
    let mut met = false;
    for i in 0..n {
        if record.hit[i] {
            met = true;
            record.point[i] = point(&prop.to_world, record.point[i]);
            record.normal[i] = direction(&prop.to_world, record.normal[i]);
        }
    }
    met
}

/// A point through a matrix, the translation row first (`vaddax` of it, then `vmadda` rows 0 and 1
/// and `vmadd` row 2); `w` as it came.
fn point(m: &Mat4, v: Vf) -> Vf {
    let acc = m.0[3].add(Vf::ZERO);
    let acc = Vf::madd(acc, m.0[0], v.bc(Field::X));
    let acc = Vf::madd(acc, m.0[1], v.bc(Field::Y));
    v.merge(Dest::XYZ, Vf::madd(acc, m.0[2], v.bc(Field::Z)))
}

/// A direction through a matrix's three rotation rows (`vmulax`, `vmadday`, `vmaddz`); `w` as it came.
fn direction(m: &Mat4, v: Vf) -> Vf {
    let acc = m.0[0].mul(v.bc(Field::X));
    let acc = Vf::madd(acc, m.0[1], v.bc(Field::Y));
    v.merge(Dest::XYZ, Vf::madd(acc, m.0[2], v.bc(Field::Z)))
}

/// A rigid matrix's inverse as the sweep builds it: the rotation transposed in place (each row keeps
/// its `w`), and the translation row's `xyz` taken back through it from zero (`vmsubax`, `vmsubay`,
/// `vmsubz`).
fn inverse(m: &Mat4) -> Mat4 {
    let [r0, r1, r2, t] = m.0;
    let c0 = Vf::new(r0.x(), r1.x(), r2.x(), r0.w());
    let c1 = Vf::new(r0.y(), r1.y(), r2.y(), r1.w());
    let c2 = Vf::new(r0.z(), r1.z(), r2.z(), r2.w());
    let acc = Vf::msub(Vf::ZERO, c0, t.bc(Field::X));
    let acc = Vf::msub(acc, c1, t.bc(Field::Y));
    Mat4([c0, c1, c2, t.merge(Dest::XYZ, Vf::msub(acc, c2, t.bc(Field::Z)))])
}
