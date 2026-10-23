//! The world a bike rides over, as the ride asks about it: where the ground is under a point, and what
//! the collision sweep runs into.
//!
//! Two things in the ride's frame reach outside the bike, and this is both of them:
//!
//! * **The ground query.** [`wheel::suspension`] walks a ray down from each wheel's mount and asks the
//!   world where the surface under the ray's end is and which way it faces. The original's answer
//!   comes out of the terrain object's own query, which D6 ported ([`track::Terrain::cell`]) down to
//!   the four corners of a cell; what is *not* ported is how the game picks a point inside the cell,
//!   because that lives in the terrain collision class nobody has read yet. [`Ground::at`] blends the
//!   cell bilinearly, which is this module's own arithmetic and is marked as such. The ride's query
//!   itself, [`World::ground`] on a [`Ground`], is not that since S64: it is the original's own
//!   triangle split ([`crate::contact_search::ground`]), and `Ground::at` only says whether a point
//!   is on the field.
//! * **The sweep.** The scene lists a collision mesh for every solid thing on the track — three
//!   hundred and fifty-three of them on `free06` — each with a position and a pair of axes. S4 ported
//!   the sweep itself ([`collide::sweep`]), which works in one mesh's own space, so [`Props`] is the
//!   bookkeeping on top: the place of each mesh in the world, the cheap sphere reject that keeps the
//!   other three hundred and fifty-two out of the frame, and the merge of one mesh's hits into the
//!   bundle's record in world space.
//!
//! A third thing asks, and it is not the ride: the follow camera. It asks twice, and both are answered
//! at the end of this file. Its obstacle probe wants the nearest thing on one segment
//! ([`crate::camera::Obstacles`]) and its ground probe wants the surface under one point
//! ([`crate::camera::Surface`]) — the same two answers the ride gets, the terrain and the collision
//! shapes, because in the original it is the same collision world the bike is swept against and the
//! same ground query the wheels use.
//!
//! The merge is worth a word, because it is the one place this module does the original's bookkeeping
//! rather than its own: a probe keeps its *nearest* hit, the bundle latches its nearest across all
//! probes, and the push-out is recomputed from the world-space direction and normal — the same three
//! rules `collide`'s leaf applies, applied again one level up. Sweeping every mesh into one record
//! directly would be wrong, because the record's points and normals would be in whichever mesh was
//! swept last.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::camera::{Obstacle, Obstacles, Surface};
use crate::collide::{self, Bundle, MeshRef, Probe, Record};
use crate::contact_search::{Cell, Heightfield, TerrainField};
use crate::math::{self, Mat4};
use crate::track::{self, Mesh, Placement, Terrain, TerrainLayout};
use ps2_float::{Dest, F, Vf, vu};

/// How many terrain samples one unit of the scene's `TerrainWidth` (or `TerrainBreadth`) is: the game's
/// map edge is width × 256 × scale whatever the tree's depth. On a two-level tree that is one root cell;
/// a one-level tree (MX vs. ATV's supercross and freestyle maps) is 16 root cells of 16 samples and
/// says `TerrainWidth=1`, so counting `16^levels` would make it 16 samples a side.
pub const SAMPLES_PER_WIDTH: u32 = 256;

/// The height a ground query answers with where there is no ground: far enough below anything a wheel
/// can reach that [`wheel::suspension`]'s "the ground is below the probe's end" test refuses it.
const NO_GROUND: f32 = -1.0e6;

/// Everything outside the bike that one frame of the ride asks about.
///
/// Two methods, so that a test can ride a flat plane with nothing on it ([`Flat`]) and the shell can
/// ride a track ([`Place`]) without the ride knowing which it has.
pub trait World {
    /// Where the surface under a world point is, and which way it faces: the answer
    /// [`crate::wheel::suspension`]'s ray wants. The point comes back moved onto the surface.
    fn ground(&mut self, at: Vf) -> (Vf, Vf);

    /// Which friction surface the ground under a point is, as an index into the tire's own
    /// [`crate::phy::Tire::surfaces`] table. The original keeps it in the contact record the ground
    /// query fills in; here it is a second question, so that [`World::ground`] stays the shape
    /// [`crate::wheel::suspension`]'s closure wants. A world with one kind of dirt answers zero.
    fn surface(&mut self, at: Vf) -> u8 {
        let _ = at;
        0
    }

    /// The surface the wheel's ray leaves in the wheel (S89): the original's heightfield query writes
    /// it at the end of every pass of the ray, hit or miss, from the terrain alone. `None` where the
    /// query writes nothing, so the wheel keeps what it had. A world with no terrain of its own
    /// answers [`World::surface`].
    fn wheel_surface(&mut self, at: Vf) -> Option<u8> {
        Some(self.surface(at))
    }

    /// Sweep a bundle of probes against everything solid, in world space. `true` when anything was
    /// looked at — the record says what was hit.
    fn solid(&mut self, bundle: &Bundle, record: &mut Record) -> bool;

    /// Sweep a bundle against the placed groups only — an ecosystem's trunks ([`crate::eco`], S97):
    /// the shapes the bike's body rays search and its swept probe and the put-back's probes do not.
    /// The record is the one [`World::solid`] filled, so the nearer hit of the two stands. A world
    /// with no placed groups looks at nothing.
    fn groups(&mut self, bundle: &Bundle, record: &mut Record) -> bool {
        let _ = (bundle, record);
        false
    }

    /// Whether the put-back's clearance query refuses a spot with none of its probes meeting anything
    /// (S75): the original's query also refuses when a placed group it has gathered within `radius` of
    /// `centre` has a part whose cell, on the ground plane, is within twenty feet of it — and, in a
    /// race, when a checkpoint is near. A world with no placed groups and no race answers no.
    fn crowded(&mut self, centre: Vf, radius: F) -> bool {
        let _ = (centre, radius);
        false
    }

    /// How big the map is, as the bike's map-edge test measures it ([`crate::off_map`]): the
    /// terrain's root cells times 256 times its sample spacing. `None` for a world with no edge, which
    /// is every world but a track's.
    fn area(&self) -> Option<crate::biplane::Area> {
        None
    }

    /// The terrain as a collision object's search reads it: the wreck bike's heightfield rays
    /// ([`crate::wreck_bike`]). A world that has no terrain of its own is sampled through
    /// [`World::ground`], a cell a unit wide ([`Sampled`]).
    fn heightfield(&mut self) -> Option<Box<dyn Heightfield + '_>> {
        Some(Box::new(Sampled(std::cell::RefCell::new(self))))
    }

    /// What a collision object's search reads, together: the terrain ([`World::heightfield`]) and the
    /// collision meshes standing on it, which the biplane's probes search ([`crate::flight_search`]).
    /// A world with no props of its own has none.
    fn collision(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>) {
        (self.heightfield(), None)
    }

    /// [`World::collision`] and the ecosystem's plants standing in the world, which the biplane's and
    /// the wreck bike's ray probes search as placed groups (S102, [`crate::eco::Wooded`]). A world
    /// without an ecosystem has none.
    fn wooded(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>, Option<&crate::eco::Trees>) {
        let (field, props) = self.collision();
        (field, props, None)
    }
}

/// Any world as a heightfield: each cell's corners a unit apart, at the height and with the normal
/// [`World::ground`] gives there. The port's own, for the worlds tests build; a track is read as the
/// terrain object reads it ([`Ground::field`]).
pub struct Sampled<'a, W: World + ?Sized>(pub std::cell::RefCell<&'a mut W>);

impl<W: World + ?Sized> Heightfield for Sampled<'_, W> {
    fn min(&self) -> Vf {
        Vf::new(-f32::MAX, 0.0, -f32::MAX, 1.0)
    }
    fn max(&self) -> Vf {
        Vf::new(f32::MAX, 0.0, f32::MAX, 1.0)
    }
    fn per_sample(&self) -> F {
        F::ONE
    }
    fn height_scale(&self) -> F {
        F::ONE
    }
    fn coarse(&self) -> bool {
        false
    }
    fn split_one_way(&self) -> bool {
        true
    }
    fn cell(&self, ix: i32, iz: i32, _step: i32) -> Option<Cell> {
        let mut w = self.0.borrow_mut();
        let mut heights = [0.0; 4];
        let mut normals = [Vf::ZERO; 4];
        for (k, (dx, dz)) in [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().enumerate() {
            let at = Vf::new(fpu_i32(ix.wrapping_add(dx)), 0.0, fpu_i32(iz.wrapping_add(dz)), 1.0);
            let (on, n) = w.ground(at);
            heights[k] = on.y();
            normals[k] = n;
        }
        Some(Cell { heights: Vf(heights), normals, surfaces: [0; 4] })
    }
}

fn fpu_i32(i: i32) -> f32 {
    F::from(i).0
}

/// A flat plane at a height, with nothing standing on it. What a fixture rides.
#[derive(Clone, Copy, Debug)]
pub struct Flat {
    pub y: F,
    pub normal: Vf,
}

impl Default for Flat {
    fn default() -> Flat {
        Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 1.0) }
    }
}

impl World for Flat {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (Vf::new(at.x(), self.y.0, at.z(), 1.0), self.normal)
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }

    fn heightfield(&mut self) -> Option<Box<dyn Heightfield + '_>> {
        Some(Box::new(*self))
    }
}

/// The plane as a heightfield of one-unit cells reaching as far as a float does, every corner at its
/// height and facing its normal.
impl Heightfield for Flat {
    fn min(&self) -> Vf {
        Vf::new(-f32::MAX, 0.0, -f32::MAX, 1.0)
    }
    fn max(&self) -> Vf {
        Vf::new(f32::MAX, 0.0, f32::MAX, 1.0)
    }
    fn per_sample(&self) -> F {
        F::ONE
    }
    fn height_scale(&self) -> F {
        F::ONE
    }
    fn coarse(&self) -> bool {
        false
    }
    fn split_one_way(&self) -> bool {
        true
    }
    fn cell(&self, _ix: i32, _iz: i32, _step: i32) -> Option<Cell> {
        Some(Cell { heights: Vf::splat(self.y.0), normals: [self.normal; 4], surfaces: [0; 4] })
    }
}

// --- the ground ------------------------------------------------------------------------------------

/// What the ground is doing under one point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// World height.
    pub y: F,
    /// The surface normal, normalised — the stored one is not.
    pub normal: Vf,
    /// The surface id, which is the index of a friction surface in the bike's `.phy`.
    pub surface: u8,
}

/// One track's terrain, and what the scene says about how big it is.
///
/// The same shape as the shell's own `ground.rs`, in [`F`] rather than host floats, because what comes
/// out of it reaches the simulation here: a height under a wheel decides a spring force.
pub struct Ground<'a> {
    terrain: &'a Terrain<'a>,
    /// World units between neighbouring samples, which is also local height to world height.
    pub spacing: F,
    /// How many samples the field holds across and along.
    pub samples_x: u32,
    pub samples_z: u32,
    /// The map's extent as the game's own edge test computes it, from the scene's numbers rather than
    /// the samples: [`World::area`].
    pub area: crate::biplane::Area,
}

impl<'a> Ground<'a> {
    /// The ground of one track: its terrain file, and the scene's own statement of how many root cells
    /// it has. A field is `width × 256` samples ([`SAMPLES_PER_WIDTH`]), whatever the tree's depth.
    pub fn new(terrain: &'a Terrain<'a>, layout: &TerrainLayout) -> Ground<'a> {
        Ground {
            spacing: terrain.scale,
            samples_x: layout.width.max(1) * SAMPLES_PER_WIDTH,
            samples_z: layout.breadth.max(1) * SAMPLES_PER_WIDTH,
            area: crate::biplane::Area::new(
                F(layout.width as f32),
                F(layout.breadth as f32),
                layout.width_scale,
                layout.breadth_scale,
            ),
            terrain,
        }
    }

    /// How big the field is in world units.
    pub fn extent(&self) -> (F, F) {
        (
            F::from((self.samples_x - 1) as i32) * self.spacing,
            F::from((self.samples_z - 1) as i32) * self.spacing,
        )
    }

    /// The terrain as the terrain object's own query reads it: the box it covers (a sample's width
    /// past the last sample, as the game's object has it), a sample per [`Ground::spacing`], heights
    /// scaled by the same, one sample to a cell and every cell split the same way.
    pub fn field(&self) -> TerrainField<'_> {
        let (x, z) = (F::from(self.samples_x as i32) * self.spacing, F::from(self.samples_z as i32) * self.spacing);
        TerrainField {
            terrain: self.terrain,
            min: Vf::new(0.0, 0.0, 0.0, 1.0),
            max: Vf::new(x.0, 0.0, z.0, 1.0),
            per_sample: F::ONE / self.spacing,
            height_scale: self.spacing,
            coarse: false,
            split_one_way: true,
        }
    }

    /// The ground under a world point: the cell's four corners, blended.
    ///
    /// The blend is **this module's own**, not the game's — see the module note. Off the edge of the
    /// field there is no answer at all, which is what keeps a rider on the map.
    pub fn at(&self, x: F, z: F) -> Option<Hit> {
        if x < F::ZERO || z < F::ZERO {
            return None;
        }
        let (fx, fz) = (x / self.spacing, z / self.spacing);
        let (ix, iz) = (fx.to_i32(), fz.to_i32());
        if ix < 0 || iz < 0 {
            return None;
        }
        let (sx, sz) = (ix as u32, iz as u32);
        if sx + 1 >= self.samples_x || sz + 1 >= self.samples_z {
            return None;
        }
        let quad = self.terrain.cell(sx, sz, 1)?;
        let (tx, tz) = (fx - F::from(ix), fz - F::from(iz));
        // The corners come back as the sample, +x, +z, then both: a bilinear blend in that order.
        let weight = [
            (F::ONE - tx) * (F::ONE - tz),
            tx * (F::ONE - tz),
            (F::ONE - tx) * tz,
            tx * tz,
        ];
        let mut y = F::ZERO;
        let mut normal = Vf::ZERO;
        for i in 0..4 {
            y = y + weight[i] * quad.height[i];
            let n = quad.normal[i];
            let n = Vf::new(n[0].0, n[1].0, n[2].0, 0.0);
            normal = normal.add(n.mul_q(weight[i].0));
        }
        // The surface is the nearest corner's: a surface id is a choice of friction table, and
        // averaging two of them would name a third.
        let nearest = usize::from(tx > F(0.5)) + 2 * usize::from(tz > F(0.5));
        Some(Hit {
            y: self.spacing * y,
            normal: normalize(normal),
            surface: quad.surface[nearest],
        })
    }
}

impl World for Ground<'_> {
    /// The world's ground query as the original's is (`ground_query`, S64): the terrain object's own
    /// heightfield arithmetic over [`Ground::field`] — the cell split into two triangles, the corner
    /// normals blended by weight and normalised — which is [`crate::contact_search::ground`]. The
    /// point keeps its `x`, `z` and `w` and takes the ground's `y`.
    ///
    /// Off the field the original's query leaves the point where it was (and so a wheel past the edge
    /// finds ground at its own probe's end); this keeps [`Ground::at`]'s answer there instead, no
    /// ground at all, which is what keeps a rider on the map. See S64's journal entry.
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        if self.at(F(at.x()).max(F::ZERO), F(at.z()).max(F::ZERO)).is_none() {
            return (Vf::new(at.x(), NO_GROUND, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 1.0));
        }
        let g = crate::contact_search::ground(&self.field(), at);
        (g.point, g.normal)
    }

    fn surface(&mut self, at: Vf) -> u8 {
        self.at(F(at.x()), F(at.z())).map_or(0, |hit| hit.surface)
    }

    fn wheel_surface(&mut self, at: Vf) -> Option<u8> {
        crate::contact_search::nearest_surface(&self.field(), at)
    }

    fn area(&self) -> Option<crate::biplane::Area> {
        Some(self.area)
    }

    fn heightfield(&mut self) -> Option<Box<dyn Heightfield + '_>> {
        Some(Box::new(self.field()))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

// --- the props -------------------------------------------------------------------------------------

/// One solid thing on the track: its collision mesh, where it stands, and the sphere that gets it out
/// of a frame it is nowhere near.
pub struct Prop {
    pub mesh: Mesh,
    /// The mesh's own space into the world: the mesh file's transform followed by the scene's
    /// placement.
    pub to_world: Mat4,
    /// And back, which is what a bundle of world-space probes has to go through before the sweep sees
    /// it — [`collide::sweep`] works in the mesh's own space.
    pub from_world: Mat4,
    /// The root node's box, as a sphere in world space.
    pub centre: Vf,
    pub radius: F,
}

impl Prop {
    /// A parsed mesh, stood where the scene puts it.
    ///
    /// The placement is a position and two axes, which go through the engine's own look-at
    /// ([`math::basis`], the two axes as given, which is what the scene's collision reader asks for:
    /// orthogonalising leaves an upright prop's up a bit short of 1, S96); the mesh carries a transform
    /// of its own, which goes first.
    pub fn place(mesh: Mesh, at: &Placement) -> Prop {
        let place = math::basis(
            Vf::new(at.position[0].0, at.position[1].0, at.position[2].0, 1.0),
            Vf::new(at.look[0].0, at.look[1].0, at.look[2].0, 0.0),
            Vf::new(at.up[0].0, at.up[1].0, at.up[2].0, 0.0),
            false,
            true,
        );
        let own = Mat4([
            row(mesh.transform[0], 0.0),
            row(mesh.transform[1], 0.0),
            row(mesh.transform[2], 0.0),
            row(mesh.transform[3], 1.0),
        ]);
        let to_world = math::mul(&own, &place);
        let (centre, radius) = match mesh.nodes.first() {
            Some(node) => {
                let c = row(node.centre, 1.0);
                let h = row(node.half_extent, 0.0);
                (math::transform_point(&to_world, c), length(h))
            }
            None => (Vf::new(0.0, 0.0, 0.0, 1.0), F::ZERO),
        };
        Prop { mesh, from_world: invert_rigid(&to_world), to_world, centre, radius }
    }
}

/// Everything solid on a track, and the sweep against the lot.
#[derive(Default)]
pub struct Props {
    pub props: Vec<Prop>,
}

impl Props {
    /// Read the scene's collision list, asking `read` for each mesh's bytes. A file the archive does
    /// not have, or one that does not parse, is skipped with its reason — a track is rideable without
    /// one of its fences.
    pub fn load(
        scene: &track::Scene,
        mut read: impl FnMut(&str) -> Result<Vec<u8>, String>,
    ) -> (Props, Vec<String>) {
        let mut props = Props::default();
        let mut missed = Vec::new();
        for at in &scene.collision {
            match read(&at.file).and_then(|b| Mesh::parse(&b)) {
                Ok(mesh) => props.props.push(Prop::place(mesh, at)),
                Err(e) => missed.push(format!("{}: {e}", at.file)),
            }
        }
        (props, missed)
    }

    /// Sweep a world-space bundle against every prop it could reach.
    ///
    /// Each prop is swept in its own space into a scratch record and merged back in world space, by
    /// the three rules [`collide`]'s leaf uses: a probe keeps its nearest hit, the bundle latches its
    /// nearest, and the push-out is the travel left over along the normal.
    pub fn sweep(&self, bundle: &Bundle, record: &mut Record) -> bool {
        let mut looked = false;
        for (owner, prop) in self.props.iter().enumerate() {
            if prop.radius == F::ZERO && prop.mesh.nodes.is_empty() {
                continue;
            }
            // The cheap reject: two spheres. The bundle's own centre and radius are the caller's
            // statement of where its probes are.
            let gap = length(sub3(prop.centre, bundle.centre));
            if gap > prop.radius + bundle.radius {
                continue;
            }
            let local = into(prop, bundle);
            let mut scratch = Record::new();
            if !collide::sweep(&local, &MeshRef::of(&prop.mesh), &mut scratch) {
                continue;
            }
            looked = true;
            merge(record, bundle, prop, &scratch, owner as i32);
        }
        looked
    }
}

/// A bundle of world-space probes in one prop's space.
fn into(prop: &Prop, bundle: &Bundle) -> Bundle {
    let mut out = Bundle {
        count: bundle.count,
        probes: [Probe { origin: Vf::ZERO, dir: Vf::ZERO, length: F::ZERO }; collide::MAX_PROBES],
        centre: math::transform_point(&prop.from_world, bundle.centre),
        radius: bundle.radius,
    };
    for i in 0..bundle.count.min(collide::MAX_PROBES) {
        let p = &bundle.probes[i];
        out.probes[i] = Probe {
            origin: math::transform_point(&prop.from_world, p.origin),
            dir: direction(&prop.from_world, p.dir),
            length: p.length,
        };
    }
    out
}

/// One prop's hits into the bundle's record, in world space.
fn merge(record: &mut Record, bundle: &Bundle, prop: &Prop, found: &Record, owner: i32) {
    for i in 0..bundle.count.min(collide::MAX_PROBES) {
        if !found.hit[i] {
            continue;
        }
        let nearer = found.fraction[i] < record.fraction[i] || record.fraction[i] < F::ZERO;
        if nearer {
            record.fraction[i] = found.fraction[i];
            record.point[i] = math::transform_point(&prop.to_world, found.point[i]);
            record.normal[i] = direction(&prop.to_world, found.normal[i]);
            record.owner[i] = owner;
        }
        record.hit[i] = true;
        record.hits += 1;
        if record.fraction[i] < record.nearest {
            record.nearest = record.fraction[i];
            record.nearest_probe = Some(i);
            record.nearest_normal = record.normal[i];
            record.nearest_point = record.point[i];
            // The travel left over, along the normal, in units of one unit of travel — `collide`'s
            // own rule, with the world-space direction this time.
            let left = bundle.probes[i].dir.mul_q((F::ONE - record.fraction[i]).0);
            let n = record.normal[i];
            let depth = vu::dot3(left.0, n.0);
            record.push = n.merge(Dest::XYZ, n.mul_q(depth));
        }
    }
}

/// A whole track: the ground and the props on it.
pub struct Place<'a> {
    pub ground: Ground<'a>,
    pub props: Props,
}

impl World for Place<'_> {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.ground.ground(at)
    }

    fn surface(&mut self, at: Vf) -> u8 {
        self.ground.surface(at)
    }

    fn wheel_surface(&mut self, at: Vf) -> Option<u8> {
        self.ground.wheel_surface(at)
    }

    fn area(&self) -> Option<crate::biplane::Area> {
        self.ground.area()
    }

    fn solid(&mut self, bundle: &Bundle, record: &mut Record) -> bool {
        self.props.sweep(bundle, record)
    }

    fn heightfield(&mut self) -> Option<Box<dyn Heightfield + '_>> {
        self.ground.heightfield()
    }

    fn collision(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>) {
        (self.ground.heightfield(), Some(&self.props))
    }
}

// --- the small arithmetic --------------------------------------------------------------------------

/// A scene's three-float row as a quadword.
fn row(v: [F; 3], w: f32) -> Vf {
    Vf::new(v[0].0, v[1].0, v[2].0, w)
}

/// A *direction* through a matrix: the three rotation rows, with the translation left out.
fn direction(m: &Mat4, v: Vf) -> Vf {
    let acc = m.0[0].mul_q(v.x());
    let acc = Vf::madd(acc, m.0[1], v.bc(vu::Field::Y));
    let acc = Vf::madd(acc, m.0[2], v.bc(vu::Field::Z));
    v.merge(Dest::XYZ, acc)
}

/// The inverse of a rigid transform: the rotation transposed, and the translation back through it.
/// A mesh transform with scale in it would need more than this, and none of the disc's have.
fn invert_rigid(m: &Mat4) -> Mat4 {
    let r = [m.0[0], m.0[1], m.0[2]];
    let t = m.0[3];
    let inv = Mat4([
        Vf::new(r[0].x(), r[1].x(), r[2].x(), 0.0),
        Vf::new(r[0].y(), r[1].y(), r[2].y(), 0.0),
        Vf::new(r[0].z(), r[1].z(), r[2].z(), 0.0),
        Vf::new(0.0, 0.0, 0.0, 1.0),
    ]);
    let back = direction(&inv, t);
    Mat4([inv.0[0], inv.0[1], inv.0[2], Vf::new(-back.x(), -back.y(), -back.z(), 1.0)])
}

/// `a - b` over `xyz`.
fn sub3(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// The length of a vector's `xyz`, by a real square root: nothing here is the original's arithmetic,
/// and a sphere reject that is a bit out rejects nothing.
fn length(v: Vf) -> F {
    F(vu::dot3(v.0, v.0)).sqrt()
}

/// Normalise `xyz` through VU0's *approximate* reciprocal square root, as everything else in the port
/// that normalises a vector does. `w` comes through untouched.
fn normalize(v: Vf) -> Vf {
    v.merge(Dest::XYZ, v.mul_q(vu::rsqrt(1.0, vu::dot3(v.0, v.0))))
}

// --- what the camera asks --------------------------------------------------------------------------

/// How many steps of the terrain march there may be per sample of the heightfield, and the ceiling on
/// the whole march: a camera probe is a dozen feet long, so the ceiling is never the binding one.
const STEPS_PER_SAMPLE: i32 = 2;
const MAX_STEPS: i32 = 64;
/// How many times the crossing is halved once the march has bracketed it. Eight halvings put the
/// point within a four-hundredth of a sample of the real crossing, which is a fraction of an inch.
const HALVINGS: u32 = 8;

impl Ground<'_> {
    /// Where a segment first goes under the terrain: the fraction of the way along it, and the
    /// crossing point with the surface normal there.
    ///
    /// **The port's own arithmetic, not the original's.** The game has a terrain collision class that
    /// answers a swept probe directly and nobody has read it; this walks the segment in steps of half
    /// a sample — short enough that no cell can be stepped over — and halves the first step that ends
    /// up underground. What it shares with the original is the rule that decides whether there is a
    /// hit at all: a segment that *starts* under the surface comes out of it unobstructed, because the
    /// original's triangle test culls back faces.
    ///
    /// Off the edge of the field there is no ground and so no hit, which is the same answer
    /// [`Ground::at`] gives the ride's suspension there.
    pub fn under(&self, start: Vf, end: Vf) -> Option<(F, Obstacle)> {
        let span = sub3(end, start);
        let len = length(span);
        if len <= F::ZERO || self.clearance(start)? <= F::ZERO {
            return None;
        }
        let per_step = self.spacing / F::from(STEPS_PER_SAMPLE);
        let steps = (len / per_step).to_i32().clamp(1, MAX_STEPS);
        let mut lo = F::ZERO;
        for i in 1..=steps {
            let hi = F::from(i) / F::from(steps);
            let Some(clear) = self.clearance(along(start, span, hi)) else {
                // Off the field: nothing to hit, and nothing to bracket a crossing against either.
                lo = hi;
                continue;
            };
            if clear > F::ZERO {
                lo = hi;
                continue;
            }
            // Bracketed: the segment was above the dirt at `lo` and is under it at `hi`.
            let (mut lo, mut hi) = (lo, hi);
            for _ in 0..HALVINGS {
                let mid = (lo + hi) / F(2.0);
                match self.clearance(along(start, span, mid)) {
                    Some(c) if c > F::ZERO => lo = mid,
                    _ => hi = mid,
                }
            }
            let point = along(start, span, hi);
            let hit = self.at(F(point.x()), F(point.z()))?;
            return Some((hi, Obstacle { point: Vf::new(point.x(), hit.y.0, point.z(), 1.0), normal: hit.normal }));
        }
        None
    }

    /// How far a world point is above the dirt, or `None` where there is no dirt under it.
    fn clearance(&self, at: Vf) -> Option<F> {
        self.at(F(at.x()), F(at.z())).map(|hit| F(at.y()) - hit.y)
    }
}

impl Obstacles for Ground<'_> {
    fn nearest(&mut self, start: Vf, end: Vf) -> Option<Obstacle> {
        self.under(start, end).map(|(_, hit)| hit)
    }
}

impl Surface for Ground<'_> {
    /// The point dropped onto the dirt: the terrain's own height at its `x` and `z`, with the other
    /// three lanes left alone. Off the edge of the field the point comes back where it was, which is
    /// what the original's query does when it finds nothing — and what the camera's ground probe
    /// reads as "no lift".
    fn on_surface(&mut self, point: Vf) -> Vf {
        match self.at(F(point.x()), F(point.z())) {
            Some(hit) => Vf::new(point.x(), hit.y.0, point.z(), point.w()),
            None => point,
        }
    }
}

impl Props {
    /// The nearest thing on one segment: the camera's probe, as a one-probe bundle swept against
    /// everything solid. The fraction comes back with it so that a caller with a second answer — the
    /// ground — can take whichever is nearer.
    pub fn along(&self, start: Vf, end: Vf) -> Option<(F, Obstacle)> {
        let probe = Probe::between(start, end);
        if probe.length <= F::ZERO {
            return None;
        }
        let mut bundle = Bundle { count: 1, ..Bundle::default() };
        bundle.probes[0] = probe;
        // The sphere every tree node is tested against first has to cover the whole segment: the
        // midpoint and half the length, which is the smallest one that does.
        bundle.centre = along(start, sub3(end, start), F(0.5));
        bundle.radius = probe.length / F(2.0);
        let mut record = Record::new();
        self.sweep(&bundle, &mut record);
        if !record.hit[0] || record.fraction[0] < F::ZERO {
            return None;
        }
        Some((
            record.fraction[0],
            Obstacle { point: record.point[0], normal: record.normal[0] },
        ))
    }
}

impl Obstacles for Props {
    fn nearest(&mut self, start: Vf, end: Vf) -> Option<Obstacle> {
        self.along(start, end).map(|(_, hit)| hit)
    }
}

impl Surface for Place<'_> {
    /// The ground, which is the terrain and nothing else: the original's query walks the terrain
    /// collision tree, and a collision shape standing on it is not ground. So a camera looking over a
    /// ramp is lifted by the hill the ramp sits on and not by the ramp — which is the obstacle probe's
    /// job, one step later in the frame.
    fn on_surface(&mut self, point: Vf) -> Vf {
        self.ground.on_surface(point)
    }
}

impl Obstacles for Place<'_> {
    /// The nearer of the two answers: the terrain's crossing and the collision shapes' hit, compared
    /// as fractions of the same segment. This is what a [`crate::camera::Follow`] with a track under it
    /// probes, and between them they are the whole of what the original's camera could have hit.
    fn nearest(&mut self, start: Vf, end: Vf) -> Option<Obstacle> {
        match (self.ground.under(start, end), self.props.along(start, end)) {
            (Some((dirt, on_dirt)), Some((prop, on_prop))) => {
                Some(if dirt <= prop { on_dirt } else { on_prop })
            }
            (Some((_, hit)), None) | (None, Some((_, hit))) => Some(hit),
            (None, None) => None,
        }
    }
}

/// A point a fraction of the way along `span` from `start`, over `xyz`, with `start`'s own `w`.
fn along(start: Vf, span: Vf, t: F) -> Vf {
    start.merge(Dest::XYZ, start.add(span.mul_q(t.0)))
}
