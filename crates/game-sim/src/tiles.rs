//! Worlds bigger than one terrain (MD8): several tracks' terrains laid side by side at offsets, with
//! join ground between them, ridden as one ground.
//!
//! The layout is openmx-mods' world standard (`standard/OPENMX-TRACK-STANDARD.md` §3). Each **tile**
//! is a track's terrain whole, as its source disc has it — its own spacing, its own height scale,
//! its own tree — with its sample (0, 0) at `at` in the world and the cells it owns given by `keep`
//! (the interior left once the edge berm a neighbour meets is cut). Each **join** is a generated
//! terrain at its own `at`, owning every cell it has. A world point belongs to the first tile whose
//! `keep` holds it, then to the first join that covers it, and otherwise to nothing: that is off the
//! world.
//!
//! **Each tile is queried in its own coordinates.** The point the ride asks about is moved into the
//! tile's frame (`world − at`) and handed to the tile's own [`Ground`] — the same query, the same
//! arithmetic and the same terrain bytes as the tile ridden alone — and the answer's height and
//! normal come back unchanged. The move loses nothing: `at` is a whole number of feet (the standard
//! asks for a whole number of leaves, `16 × spacing`, which is whole for every spacing a disc has),
//! the point is at or past it on both axes, and below 2²⁴ feet a whole number is a multiple of the
//! point's own last bit, so the difference is a multiple of that bit no larger than the point —
//! which a float holds exactly, and which the EE's truncating subtract therefore gives exactly.
//! [`Tiled::new`] refuses a world whose offsets are not whole, negative or past 2²⁴ feet.
//! `tests/tiles.rs` checks the move against exact arithmetic and every tile's ground against the
//! tile alone, bit for bit.
//!
//! What a world is *not* exact to is a ride on the tile alone. The rider's own position is in world
//! feet, and far from the origin a float holds fewer of its bits: 1/128 ft at the ultra map's
//! 116,000 ft against 1/2048 ft at 5,000. The ground under a point is the tile's to the bit, but a
//! bike moving through the same local spot of the same tile is a different sum of floats when the
//! tile is far out, and can part from the tile-alone ride on any tick (the journal measures it).
//!
//! **Loading and dropping.** A tile's terrain bytes and its collision shapes are loaded the first
//! time anything asks about ground it owns or shapes that could reach a probe, and [`Tiled::stream`]
//! drops what is far from every rider and loads ahead of them. What the simulation is answered never
//! depends on what happens to be loaded — a query on a dropped tile loads it again first — so the
//! streaming is memory, not behaviour.
//!
//! **The edge.** The world's [`World::area`] is the rectangle from the origin to its far corner, so
//! the original's own edge test ([`crate::off_map`]) shoots a rider off its outer edge with its own
//! margin, and nowhere between tiles. Inside that rectangle, a hole (no tile and no join) is asked
//! the same way through [`World::off_world`].
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::biplane::Area;
use crate::camera::{Obstacle, Obstacles, Surface};
use crate::collide::{Bundle, Probe, Record};
use crate::contact_search::{Cell, Heightfield};
use crate::track::{Mesh, Placement, Terrain, TerrainLayout};
use crate::world::{self, Ground, Hit, Place, Prop, Props, World, NO_GROUND};
use ps2_float::{Dest, F, Vf};
use std::cell::RefCell;
use std::collections::HashMap;

pub mod generate;

/// How many samples a terrain tree's leaf holds along one side.
const LEAF_SAMPLES: u32 = 16;

/// The side of one square of the index that finds the tiles under a point, in feet, as a shift: 1024.
const BUCKET_SHIFT: i32 = 10;

/// The largest offset a tile may have: past 2²⁴ feet a whole number of feet is no longer a multiple
/// of every point's last bit, and the move into a tile's coordinates could round.
const MAX_AT: f32 = 16_777_216.0;

/// How far past its own terrain a tile's collision shapes are looked for: a shape stands on the
/// tile, and the largest on the disc reaches well inside this.
const SHAPE_REACH: F = F(256.0);

/// One terrain of a world: a tile or a join, as the world's file places it.
#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    /// The tile's or join's folder name, for the log.
    pub name: String,
    /// Where the terrain's sample (0, 0) sits in the world, `x` and `z`, in feet.
    pub at: [F; 2],
    /// The terrain's root cells across and along, as the tile's scene states them (a join's are read
    /// off its tree), which with the tree's levels is its samples ([`Ground::new`]).
    pub layout: TerrainLayout,
    /// Feet a sample: the terrain file's own, read from its header.
    pub spacing: F,
    /// How many levels the terrain's tree has.
    pub levels: u32,
    /// The cells it owns, `[x0, z0, x1, z1]` in its own samples, half-open. A join owns every cell it
    /// has.
    pub keep: [u32; 4],
    /// Where its collision shapes' owner numbers start: the count of every collision placement in the
    /// tiles before it, so a shape is the same number whatever else is loaded. Its own first shape is
    /// numbered as it is on the tile alone when this is 0.
    pub first_owner: i32,
}

impl Tile {
    /// Samples across and along: root cells × 16 per level.
    pub fn samples(&self) -> [u32; 2] {
        let per_root = LEAF_SAMPLES.pow(self.levels);
        [self.layout.width.max(1) * per_root, self.layout.breadth.max(1) * per_root]
    }

    /// The whole terrain's cells, as a join's `keep`.
    pub fn every_cell(&self) -> [u32; 4] {
        let [x, z] = self.samples();
        [0, 0, x.saturating_sub(1), z.saturating_sub(1)]
    }

    /// The world rectangle its terrain covers, `[x0, z0, x1, z1]` in feet.
    fn span(&self) -> [F; 4] {
        let [x, z] = self.samples();
        [
            self.at[0],
            self.at[1],
            self.at[0] + F::from(x as i32) * self.spacing,
            self.at[1] + F::from(z as i32) * self.spacing,
        ]
    }

    /// The world rectangle of the cells it owns, `[x0, z0, x1, z1]` in feet.
    fn owned(&self) -> [F; 4] {
        let [x0, z0, x1, z1] = self.keep.map(|k| F::from(k as i32) * self.spacing);
        [self.at[0] + x0, self.at[1] + z0, self.at[0] + x1, self.at[1] + z1]
    }

    /// A world point in the tile's own coordinates, and whether the tile owns it: the cell it falls
    /// in by the same division [`Ground::at`] makes, inside `keep` and inside the terrain.
    fn local(&self, x: F, z: F) -> Option<(F, F)> {
        let (lx, lz) = (x - self.at[0], z - self.at[1]);
        if !(lx >= F::ZERO && lz >= F::ZERO) {
            return None;
        }
        let (ix, iz) = ((lx / self.spacing).to_i32(), (lz / self.spacing).to_i32());
        let [sx, sz] = self.samples();
        let [x0, z0, x1, z1] = self.keep;
        let inside = |i: i32, lo: u32, hi: u32, n: u32| i >= 0 && (i as u32) >= lo && (i as u32) < hi && (i as u32) + 1 < n;
        (inside(ix, x0, x1, sx) && inside(iz, z0, z1, sz)).then_some((lx, lz))
    }
}

/// What a world's [`Source`] hands over for one tile: its terrain file, and its collision shapes
/// placed in the tile's own coordinates (none for a join).
#[derive(Default)]
pub struct Piece {
    pub terrain: Vec<u8>,
    pub shapes: Vec<(Mesh, Placement)>,
}

/// Where a world's tiles come from: the shell reads a mod's folders, a test writes them. `Send`, so
/// a ride can be loaded on a thread of its own (P127).
pub trait Source: Send {
    /// Tile `index`'s terrain and shapes.
    fn load(&mut self, index: usize) -> Result<Piece, String>;
}

/// A tile in memory: its terrain's bytes and its shapes, stood in the world.
struct Loaded {
    bytes: Box<[u8]>,
    props: Props,
}

/// A tile's slot: not loaded, loaded, or refused when it was (then it has no ground, and says so
/// once).
enum Slot {
    Out,
    In(Loaded),
    Failed,
}

/// How far from a rider [`Tiled::stream`] keeps tiles loaded, in feet: within `load` of any rider
/// it loads them ahead of time, past `drop` of every rider it lets them go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reach {
    pub load: F,
    pub drop: F,
}

impl Reach {
    /// A desktop's: the GPU's ground draws to the fog, a few thousand feet, and tiles are tens of
    /// megabytes at most.
    pub const DESKTOP: Reach = Reach { load: F(6000.0), drop: F(9000.0) };
    /// A browser's, where the page's memory is the tighter limit: the tile a rider is on and its
    /// neighbours.
    pub const WEB: Reach = Reach { load: F(2000.0), drop: F(3000.0) };
}

/// A world of several terrains, as one ground: see the module note.
pub struct Tiled {
    /// Tiles first, then joins: the order a point's owner is looked for in.
    pub tiles: Vec<Tile>,
    /// The rectangle from the origin to the far corner of everything owned.
    pub area: Area,
    /// The finest spacing of any tile: the camera's march steps by half of it.
    pub finest: F,
    pub reach: Reach,
    source: RefCell<Box<dyn Source>>,
    slots: RefCell<Vec<Slot>>,
    /// Each 1024-ft square's tiles, in owner order.
    index: HashMap<(i32, i32), Vec<usize>>,
    /// What loading and dropping did since the shell last took the lines ([`Tiled::take_log`]).
    log: RefCell<Vec<String>>,
}

/// The index square a world point is in.
fn bucket(x: F, z: F) -> (i32, i32) {
    (x.to_i32() >> BUCKET_SHIFT, z.to_i32() >> BUCKET_SHIFT)
}

impl Tiled {
    /// A world of `tiles` (tiles, then joins, in the world file's order), read through `source`.
    ///
    /// Refuses an offset that is not a whole number of feet, is negative or is past 2²⁴ feet — the
    /// three things that would let the move into a tile's coordinates round — a spacing that is not
    /// positive, and a `keep` larger than its terrain.
    pub fn new(tiles: Vec<Tile>, source: Box<dyn Source>, reach: Reach) -> Result<Tiled, String> {
        if tiles.is_empty() {
            return Err("a world with no tiles".into());
        }
        let mut index: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        let (mut width, mut depth, mut finest) = (F::ZERO, F::ZERO, F(f32::MAX));
        for (i, t) in tiles.iter().enumerate() {
            for a in t.at {
                if !(a >= F::ZERO && a < F(MAX_AT)) || F::from(a.to_i32()) != a {
                    return Err(format!("{}: at {} is not a whole number of feet from 0 to 2^24", t.name, a.0));
                }
            }
            if !(t.spacing > F::ZERO) {
                return Err(format!("{}: spacing {}", t.name, t.spacing.0));
            }
            let [sx, sz] = t.samples();
            let [x0, z0, x1, z1] = t.keep;
            if !(x0 < x1 && z0 < z1 && x1 < sx && z1 < sz) {
                return Err(format!("{}: keep {:?} in a terrain of {sx} x {sz} samples", t.name, t.keep));
            }
            let owned = t.owned();
            width = width.max(owned[2]);
            depth = depth.max(owned[3]);
            finest = finest.min(t.spacing);
            // Every square the shapes could reach from, one square wider all round than the F sums
            // say, so no rounding at a square's edge can leave a tile out of one.
            let span = t.span();
            let (lo, hi) = (
                bucket((span[0] - SHAPE_REACH).max(F::ZERO), (span[1] - SHAPE_REACH).max(F::ZERO)),
                bucket(span[2] + SHAPE_REACH, span[3] + SHAPE_REACH),
            );
            for bz in lo.1 - 1..=hi.1 + 1 {
                for bx in lo.0 - 1..=hi.0 + 1 {
                    index.entry((bx, bz)).or_default().push(i);
                }
            }
        }
        let slots = tiles.iter().map(|_| Slot::Out).collect();
        Ok(Tiled {
            tiles,
            area: Area { width, depth },
            finest,
            reach,
            source: RefCell::new(source),
            slots: RefCell::new(slots),
            index,
            log: RefCell::new(Vec::new()),
        })
    }

    /// The tile that owns a world point, and the point in its coordinates.
    pub fn owner(&self, x: F, z: F) -> Option<(usize, F, F)> {
        if !(x >= F::ZERO && z >= F::ZERO && x < F(MAX_AT) && z < F(MAX_AT)) {
            return None;
        }
        let near = self.index.get(&bucket(x, z))?;
        near.iter().find_map(|&i| self.tiles[i].local(x, z).map(|(lx, lz)| (i, lx, lz)))
    }

    /// Make sure tile `i` is in memory, loading it if it is not. `false` if it cannot be.
    fn ensure(&self, i: usize) -> bool {
        let mut slots = self.slots.borrow_mut();
        match slots[i] {
            Slot::In(_) => return true,
            Slot::Failed => return false,
            Slot::Out => {}
        }
        let tile = &self.tiles[i];
        let piece = self.source.borrow_mut().load(i).and_then(|piece| {
            Terrain::parse(&piece.terrain).map(|_| ()).map_err(|e| format!("terrain: {e}"))?;
            Ok(piece)
        });
        match piece {
            Ok(piece) => {
                let props = Props {
                    props: piece.shapes.into_iter().map(|(mesh, at)| Prop::place(mesh, &moved(&at, tile.at))).collect(),
                };
                self.log.borrow_mut().push(format!("world: {} loaded, {} shapes", tile.name, props.props.len()));
                slots[i] = Slot::In(Loaded { bytes: piece.terrain.into_boxed_slice(), props });
                true
            }
            Err(e) => {
                self.log.borrow_mut().push(format!("world: {} has no ground: {e}", tile.name));
                slots[i] = Slot::Failed;
                false
            }
        }
    }

    /// Run `f` on tile `i`'s ground, as the tile alone has it.
    fn with_ground<R>(&self, i: usize, f: impl FnOnce(&Ground<'_>) -> R) -> Option<R> {
        if !self.ensure(i) {
            return None;
        }
        let slots = self.slots.borrow();
        let Slot::In(loaded) = &slots[i] else { return None };
        let terrain = Terrain::parse(&loaded.bytes).ok()?;
        let ground = Ground::new(&terrain, &self.tiles[i].layout);
        Some(f(&ground))
    }

    /// The ground under a world point: the owning tile's own [`Ground::at`], asked in its coordinates.
    pub fn at(&self, x: F, z: F) -> Option<Hit> {
        let (i, lx, lz) = self.owner(x, z)?;
        self.with_ground(i, |g| g.at(lx, lz)).flatten()
    }

    /// Whether a world point is owned by nothing.
    pub fn hole(&self, x: F, z: F) -> bool {
        self.owner(x, z).is_none()
    }

    /// Which tiles are loaded now, by index.
    pub fn loaded(&self) -> Vec<usize> {
        let slots = self.slots.borrow();
        (0..slots.len()).filter(|&i| matches!(slots[i], Slot::In(_))).collect()
    }

    /// The bytes held by the loaded tiles' terrains.
    pub fn loaded_bytes(&self) -> usize {
        self.slots.borrow().iter().map(|s| if let Slot::In(l) = s { l.bytes.len() } else { 0 }).sum()
    }

    /// Load the tiles within [`Reach::load`] of any of `near` and drop those past [`Reach::drop`] of
    /// all of them. Changes nothing the simulation is answered (see the module note).
    pub fn stream(&mut self, near: &[Vf]) {
        for i in 0..self.tiles.len() {
            let gap = near.iter().map(|p| self.tiles[i].gap(F(p.x()), F(p.z()))).fold(F(f32::MAX), F::min);
            let held = matches!(self.slots.borrow()[i], Slot::In(_));
            if !held && gap <= self.reach.load {
                self.ensure(i);
            } else if held && gap > self.reach.drop {
                self.slots.borrow_mut()[i] = Slot::Out;
                self.log.borrow_mut().push(format!("world: {} dropped", self.tiles[i].name));
            }
        }
    }

    /// The lines loading and dropping wrote since the last call.
    pub fn take_log(&self) -> Vec<String> {
        std::mem::take(&mut self.log.borrow_mut())
    }

    /// The tiles whose shapes could reach a bundle's sphere, in owner order, loaded.
    fn shaped(&self, centre: Vf, radius: F) -> Vec<usize> {
        let (x, z) = (F(centre.x()), F(centre.z()));
        let mut out: Vec<usize> = Vec::new();
        let lo = bucket((x - radius).max(F::ZERO), (z - radius).max(F::ZERO));
        let hi = bucket((x + radius).max(F::ZERO), (z + radius).max(F::ZERO));
        for bz in lo.1..=hi.1 {
            for bx in lo.0..=hi.0 {
                for &i in self.index.get(&(bx, bz)).map_or(&[][..], Vec::as_slice) {
                    if !out.contains(&i) && self.tiles[i].gap(x, z) <= radius + SHAPE_REACH {
                        out.push(i);
                    }
                }
            }
        }
        out.sort_unstable();
        out.retain(|&i| self.ensure(i));
        out
    }

    /// Sweep a world-space bundle against the shapes of every tile it could reach, tile by tile into
    /// one record, each tile's shapes numbered from its [`Tile::first_owner`].
    pub fn sweep(&self, bundle: &Bundle, record: &mut Record) -> bool {
        let mut looked = false;
        for i in self.shaped(bundle.centre, bundle.radius) {
            let slots = self.slots.borrow();
            if let Slot::In(loaded) = &slots[i] {
                looked |= loaded.props.sweep_owned(bundle, record, self.tiles[i].first_owner);
            }
        }
        looked
    }

    /// The nearest shape on one segment: [`Props::along`]'s one-probe bundle, over every tile.
    fn shapes_along(&self, start: Vf, end: Vf) -> Option<(F, Obstacle)> {
        let probe = Probe::between(start, end);
        if probe.length <= F::ZERO {
            return None;
        }
        let mut bundle = Bundle { count: 1, ..Bundle::default() };
        bundle.probes[0] = probe;
        let span = end.sub(start);
        bundle.centre = start.merge(Dest::XYZ, start.add(span.mul_q(0.5)));
        bundle.radius = probe.length / F(2.0);
        let mut record = Record::new();
        self.sweep(&bundle, &mut record);
        if !record.hit[0] || record.fraction[0] < F::ZERO {
            return None;
        }
        Some((record.fraction[0], Obstacle { point: record.point[0], normal: record.normal[0] }))
    }

    /// The world as the collision objects' heightfield search reads it ([`World::heightfield`]).
    pub fn field(&self) -> TiledField<'_> {
        TiledField(self)
    }
}

impl Tile {
    /// How far a world point is from the terrain's rectangle, in feet, the larger of the two axes'
    /// gaps: 0 inside it.
    fn gap(&self, x: F, z: F) -> F {
        let [x0, z0, x1, z1] = self.span();
        let dx = (x0 - x).max(x - x1).max(F::ZERO);
        let dz = (z0 - z).max(z - z1).max(F::ZERO);
        dx.max(dz)
    }
}

/// A placement in a tile's coordinates moved by the tile's offset into the world's.
fn moved(at: &Placement, by: [F; 2]) -> Placement {
    let mut out = at.clone();
    out.position[0] = at.position[0] + by[0];
    out.position[2] = at.position[2] + by[1];
    out
}

impl World for Tiled {
    /// As a track's [`crate::world::Ground`] answers since S64: the original's own query
    /// ([`crate::contact_search::ground`]) over the owning tile's field, and no ground off the world.
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

    fn solid(&mut self, bundle: &Bundle, record: &mut Record) -> bool {
        self.sweep(bundle, record)
    }

    fn area(&self) -> Option<Area> {
        Some(self.area)
    }

    fn off_world(&self, at: Vf, margin: F) -> bool {
        let (x, z) = (F(at.x()), F(at.z()));
        // Only inside the rectangle: its own edge is the area's test, with the original's margin.
        let inside = |x: F, z: F| x >= F::ZERO && z >= F::ZERO && x < self.area.width && z < self.area.depth;
        [(x, z), (x - margin, z), (x + margin, z), (x, z - margin), (x, z + margin)]
            .into_iter()
            .any(|(x, z)| inside(x, z) && self.hole(x, z))
    }

    fn heightfield(&mut self) -> Option<Box<dyn Heightfield + '_>> {
        Some(Box::new(self.field()))
    }

    fn collision(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>) {
        // The plane's search takes one set of shapes in one space; a world's are a set a tile.
        (Some(Box::new(self.field())), None)
    }
}

impl Surface for Tiled {
    fn on_surface(&mut self, point: Vf) -> Vf {
        match self.at(F(point.x()), F(point.z())) {
            Some(hit) => Vf::new(point.x(), hit.y.0, point.z(), point.w()),
            None => point,
        }
    }
}

impl Obstacles for Tiled {
    fn nearest(&mut self, start: Vf, end: Vf) -> Option<Obstacle> {
        let dirt = world::march(self.finest, start, end, |x, z| self.at(x, z));
        match (dirt, self.shapes_along(start, end)) {
            (Some((d, on_dirt)), Some((p, on_prop))) => Some(if d <= p { on_dirt } else { on_prop }),
            (Some((_, hit)), None) | (None, Some((_, hit))) => Some(hit),
            (None, None) => None,
        }
    }
}

/// A world as a heightfield: the box is the whole world's, and every query goes to the tile under
/// the point through [`Heightfield::tile`], in that tile's coordinates, on that tile's own field
/// ([`Ground::field`]). It has no cells of its own.
pub struct TiledField<'a>(&'a Tiled);

impl Heightfield for TiledField<'_> {
    fn min(&self) -> Vf {
        Vf::new(0.0, 0.0, 0.0, 1.0)
    }
    fn max(&self) -> Vf {
        Vf::new(self.0.area.width.0, 0.0, self.0.area.depth.0, 1.0)
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
        None
    }
    fn tile(&self, at: Vf, query: &mut dyn FnMut(&dyn Heightfield, Vf)) -> Option<bool> {
        let Some((i, lx, lz)) = self.0.owner(F(at.x()), F(at.z())) else { return Some(false) };
        let local = Vf::new(lx.0, at.y(), lz.0, at.w());
        Some(self.0.with_ground(i, |g| query(&g.field(), local)).is_some())
    }
}

/// The ground a ride is on: one track, or a world of several ([`Tiled`]). Every question goes to
/// whichever it is, unchanged, so a disc track rides exactly as a [`Place`] does.
pub enum Land<'a> {
    Track(Place<'a>),
    Tiles(Box<Tiled>),
}

impl Land<'_> {
    fn world(&mut self) -> &mut dyn World {
        match self {
            Land::Track(p) => p,
            Land::Tiles(t) => &mut **t,
        }
    }

    /// The world of tiles, when this is one.
    pub fn tiled(&mut self) -> Option<&mut Tiled> {
        match self {
            Land::Track(_) => None,
            Land::Tiles(t) => Some(&mut **t),
        }
    }
}

impl World for Land<'_> {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.world().ground(at)
    }
    fn surface(&mut self, at: Vf) -> u8 {
        self.world().surface(at)
    }
    fn wheel_surface(&mut self, at: Vf) -> Option<u8> {
        self.world().wheel_surface(at)
    }
    fn solid(&mut self, bundle: &Bundle, record: &mut Record) -> bool {
        self.world().solid(bundle, record)
    }
    fn groups(&mut self, bundle: &Bundle, record: &mut Record) -> bool {
        self.world().groups(bundle, record)
    }
    fn crowded(&mut self, centre: Vf, radius: F) -> bool {
        self.world().crowded(centre, radius)
    }
    fn area(&self) -> Option<Area> {
        match self {
            Land::Track(p) => p.area(),
            Land::Tiles(t) => t.area(),
        }
    }
    fn off_world(&self, at: Vf, margin: F) -> bool {
        match self {
            Land::Track(p) => p.off_world(at, margin),
            Land::Tiles(t) => t.off_world(at, margin),
        }
    }
    fn heightfield(&mut self) -> Option<Box<dyn Heightfield + '_>> {
        match self {
            Land::Track(p) => p.heightfield(),
            Land::Tiles(t) => t.heightfield(),
        }
    }
    fn collision(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>) {
        match self {
            Land::Track(p) => p.collision(),
            Land::Tiles(t) => t.collision(),
        }
    }
}

impl Surface for Land<'_> {
    fn on_surface(&mut self, point: Vf) -> Vf {
        match self {
            Land::Track(p) => p.on_surface(point),
            Land::Tiles(t) => t.on_surface(point),
        }
    }
}

impl Obstacles for Land<'_> {
    fn nearest(&mut self, start: Vf, end: Vf) -> Option<Obstacle> {
        match self {
            Land::Track(p) => p.nearest(start, end),
            Land::Tiles(t) => t.nearest(start, end),
        }
    }
}
