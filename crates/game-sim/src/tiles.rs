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
//!
//! **The put-back** (MD14). The original's put-back record keeps any spot it finds fit, and on one
//! big terrain a thrown bike comes down inside it. A world is narrower than its throw (an arc for
//! half its width, whichever way it is thrown) and has holes, so a wreck can come down over no
//! ground at all: there the record keeps nothing ([`Tiled::keeps`]) — no spot no tile owns, and none
//! the edge test would throw from — and the bike goes back to the last spot it held on ground.
//!
//! **The plane** searches one set of shapes in one space, so it is handed every tile's together
//! ([`Tiled::every_shape`]), read once and kept; the bike's own sweeps go tile by tile as above.
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

    /// Tile `index`'s shapes alone, for the plane's search ([`Tiled::every_shape`]): [`Source::load`]'s,
    /// which a source that can read them without the terrain does.
    fn shapes(&mut self, index: usize) -> Result<Vec<(Mesh, Placement)>, String> {
        self.load(index).map(|piece| piece.shapes)
    }

    /// Start reading tile `index` where [`Source::load`] will not have to wait for it, if the source
    /// can (MD28: a tile read on the ride's thread was 4 to 8 ms, and several came at once).
    fn want(&mut self, _index: usize) {}

    /// Whether [`Source::load`] of tile `index` answers without reading: always, for a source that
    /// cannot read ahead.
    fn ready(&mut self, _index: usize) -> bool {
        true
    }

    /// Tile `index` is not wanted any more ([`Source::want`]).
    fn forget(&mut self, _index: usize) {}
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
    /// Which tiles anything asked about since the last [`Tiled::stream`] (MD28): those stay, however
    /// far from the riders, so a query that goes on asking about a far tile every tick — a parked
    /// plane's dust — does not read it again every tick.
    asked: RefCell<Vec<bool>>,
    /// Each 1024-ft square's tiles, in owner order.
    index: HashMap<(i32, i32), Vec<usize>>,
    /// What loading and dropping did since the shell last took the lines ([`Tiled::take_log`]).
    log: RefCell<Vec<String>>,
    /// Every tile's shapes in one set, in owner order, once the plane's search has asked
    /// ([`Tiled::every_shape`]).
    every: Option<Props>,
    /// Which axes wrap round, `[x, z]` (MD29, [`Tiled::wrapped`]).
    wrap: [bool; 2],
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
        let asked = vec![false; tiles.len()];
        Ok(Tiled {
            tiles,
            area: Area { width, depth },
            finest,
            reach,
            source: RefCell::new(source),
            slots: RefCell::new(slots),
            asked: RefCell::new(asked),
            index,
            log: RefCell::new(Vec::new()),
            every: None,
            wrap: [false, false],
        })
    }

    /// **The world wrapped round** (MD29, the port's own): on each axis `wrap` names, the far edge
    /// is a seam onto the near one. Every question about a point past a seam is asked of the point
    /// one width back ([`Tiled::fold`]), the edge test is not asked on that axis
    /// ([`crate::off_map::outside_wrapped`]), and the ride moves a rider who crosses a seam back by
    /// one width ([`crate::rider::Rider::shift`]). The width is the world's rectangle, so its far
    /// edge must meet its near one — the world's file puts a seam join there
    /// ([`generate::seam_joins`]). Refuses a wrapped side that is not a whole number of feet below
    /// 2²⁴, the same rule as an offset's: a shift by it is then exact for every point past it.
    pub fn wrapped(mut self, wrap: [bool; 2]) -> Result<Tiled, String> {
        for (axis, on) in wrap.into_iter().enumerate() {
            let side = self.extent()[axis];
            if on && (!(side > F::ZERO && side < F(MAX_AT)) || F::from(side.to_i32()) != side) {
                return Err(format!("a world {} feet {} is not a whole number of feet from 1 to 2^24, and cannot wrap", side.0, ["wide", "deep"][axis]));
            }
        }
        self.wrap = wrap;
        self.every = None;
        Ok(self)
    }

    /// Which axes wrap, `[x, z]`.
    pub fn wraps(&self) -> [bool; 2] {
        self.wrap
    }

    /// The world's rectangle's sides, `[width, depth]`: on a wrapped axis, the shift a seam makes.
    pub fn extent(&self) -> [F; 2] {
        [self.area.width, self.area.depth]
    }

    /// One coordinate brought inside the world on a wrapped axis: less a width while it is at or
    /// past the far seam, which is exact (the point is at or past a whole number of feet below 2²⁴,
    /// as the module note argues for an offset), and plus a width while it is short of the near
    /// one, which rounds to the far side's last bit (`1/256` ft at 100,000 ft) and stays short of
    /// the far seam. Unchanged on an axis that does not wrap, and for `NaN`.
    pub fn fold_axis(&self, axis: usize, v: F) -> F {
        if !self.wrap[axis] {
            return v;
        }
        let side = self.extent()[axis];
        let mut v = v;
        for _ in 0..64 {
            if v >= side {
                v = v - side;
            } else if v < F::ZERO {
                v = v + side;
                if v >= side {
                    // A point a hair short of the near seam rounded onto the far one.
                    v = F(f32::from_bits(side.0.to_bits() - 1));
                }
            } else {
                break;
            }
        }
        v
    }

    /// A world point brought inside the world on its wrapped axes ([`Tiled::fold_axis`]).
    pub fn fold(&self, x: F, z: F) -> (F, F) {
        (self.fold_axis(0, x), self.fold_axis(1, z))
    }

    /// The whole widths a point is past the world's seams, `[x, z]`: what [`Tiled::fold`] takes off
    /// it, in widths. 0 on an axis that does not wrap, and for a point inside.
    pub fn laps(&self, x: F, z: F) -> [i32; 2] {
        let lap = |axis: usize, v: F| -> i32 {
            if !self.wrap[axis] || !(v >= F::ZERO || v < F::ZERO) {
                return 0;
            }
            let side = self.extent()[axis];
            if v >= side { 1 } else if v < F::ZERO { -1 } else { 0 }
        };
        [lap(0, x), lap(1, z)]
    }

    /// The tile that owns a world point, and the point in its coordinates. On a world that wraps,
    /// a point past a seam is the one a width back ([`Tiled::fold`]).
    pub fn owner(&self, x: F, z: F) -> Option<(usize, F, F)> {
        let (x, z) = self.fold(x, z);
        if !(x >= F::ZERO && z >= F::ZERO && x < F(MAX_AT) && z < F(MAX_AT)) {
            return None;
        }
        let near = self.index.get(&bucket(x, z))?;
        near.iter().find_map(|&i| self.tiles[i].local(x, z).map(|(lx, lz)| (i, lx, lz)))
    }

    /// Make sure tile `i` is in memory, loading it if it is not. `false` if it cannot be.
    fn ensure(&self, i: usize) -> bool {
        self.asked.borrow_mut()[i] = true;
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
    /// all of them, but for those asked about since the last call. Changes nothing the simulation is
    /// answered (see the module note).
    ///
    /// A tile the source reads ahead ([`Source::want`]) is loaded once it is read; one the
    /// simulation asks for first is loaded then, as a tile past reach is.
    pub fn stream(&mut self, near: &[Vf]) {
        let asked = std::mem::replace(&mut *self.asked.borrow_mut(), vec![false; self.tiles.len()]);
        for i in 0..self.tiles.len() {
            let gap = near.iter().map(|p| self.gap(i, F(p.x()), F(p.z()))).fold(F(f32::MAX), F::min);
            let (held, out) = match self.slots.borrow()[i] {
                Slot::In(_) => (true, false),
                Slot::Out => (false, true),
                Slot::Failed => (false, false),
            };
            if out && gap <= self.reach.load {
                let ready = {
                    let mut source = self.source.borrow_mut();
                    source.want(i);
                    source.ready(i)
                };
                if ready {
                    self.ensure(i);
                }
            } else if held && gap > self.reach.drop && !asked[i] {
                self.slots.borrow_mut()[i] = Slot::Out;
                self.log.borrow_mut().push(format!("world: {} dropped", self.tiles[i].name));
            } else if out && gap > self.reach.drop {
                self.source.borrow_mut().forget(i);
            }
        }
    }

    /// Load every tile within [`Reach::load`] of any of `near` now, read ahead or not: before a ride
    /// (MD28), where waiting costs no frame.
    pub fn load_near(&mut self, near: &[Vf]) {
        for i in 0..self.tiles.len() {
            let gap = near.iter().map(|p| self.gap(i, F(p.x()), F(p.z()))).fold(F(f32::MAX), F::min);
            if gap <= self.reach.load {
                self.ensure(i);
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
    ///
    /// On a world that wraps (MD29), a bundle within reach of a seam is swept again one width over,
    /// against the shapes by the far edge, and what that finds is moved back to where the bundle is.
    pub fn sweep(&self, bundle: &Bundle, record: &mut Record) -> bool {
        let mut looked = self.sweep_here(bundle, record);
        for by in self.copies(bundle.centre, bundle.radius + SHAPE_REACH) {
            let mut moved = *bundle;
            moved.centre = shifted(bundle.centre, by);
            for p in moved.probes.iter_mut().take(moved.count) {
                p.origin = shifted(p.origin, by);
            }
            let before = *record;
            looked |= self.sweep_here(&moved, record);
            let back = [F::ZERO - by[0], F::ZERO - by[1]];
            for k in 0..record.point.len() {
                if !same(record.point[k], before.point[k]) || record.fraction[k] != before.fraction[k] {
                    record.point[k] = shifted(record.point[k], back);
                }
            }
            if !same(record.nearest_point, before.nearest_point) || record.nearest != before.nearest {
                record.nearest_point = shifted(record.nearest_point, back);
            }
        }
        looked
    }

    /// The shifts, in feet, that bring a point within `reach` of a seam to the copy of the world
    /// on the seam's far side: none on a world that does not wrap, and up to three by a corner.
    fn copies(&self, at: Vf, reach: F) -> Vec<[F; 2]> {
        if self.wrap == [false, false] {
            return Vec::new();
        }
        let ways = |axis: usize, v: F| -> Vec<F> {
            let side = self.extent()[axis];
            let mut out = vec![F::ZERO];
            if self.wrap[axis] {
                if v + reach >= side {
                    out.push(F::ZERO - side);
                }
                if v - reach < F::ZERO {
                    out.push(side);
                }
            }
            out
        };
        let (xs, zs) = (ways(0, F(at.x())), ways(1, F(at.z())));
        let mut out = Vec::new();
        for &dz in &zs {
            for &dx in &xs {
                if dx != F::ZERO || dz != F::ZERO {
                    out.push([dx, dz]);
                }
            }
        }
        out
    }

    /// How far a world point is from tile `i`'s rectangle ([`Tile::gap`]), the nearest of its copies
    /// on a world that wraps.
    fn gap(&self, i: usize, x: F, z: F) -> F {
        let t = &self.tiles[i];
        let mut gap = t.gap(x, z);
        for by in self.copies(Vf::new(x.0, 0.0, z.0, 1.0), gap) {
            gap = gap.min(t.gap(x + by[0], z + by[1]));
        }
        gap
    }

    /// [`Tiled::sweep`] with the bundle where it is.
    fn sweep_here(&self, bundle: &Bundle, record: &mut Record) -> bool {
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

    /// **Every tile's shapes as one set** (MD14), for the searches that take one set of shapes in one
    /// space — the plane's ([`crate::flight_search`]) and the wreck bike's: tile by tile in owner
    /// order, each in world feet, so a shape's place in the set is its [`Tile::first_owner`] plus its
    /// place in its tile whatever is loaded. Read once from the source, the first time it is asked,
    /// and kept: the plane's search numbers what it gathered by place in the set, so the set must not
    /// change as tiles stream in and out. A tile whose shapes do not read adds none, and says so.
    pub fn every_shape(&mut self) -> &Props {
        if self.every.is_none() {
            let mut all = Props::default();
            let mut placed: Vec<(Mesh, Placement)> = Vec::new();
            for (i, tile) in self.tiles.iter().enumerate() {
                match self.source.get_mut().shapes(i) {
                    Ok(shapes) => all.props.extend(shapes.into_iter().map(|(mesh, at)| {
                        let at = moved(&at, tile.at);
                        if self.wrap != [false, false] {
                            placed.push((mesh.clone(), at.clone()));
                        }
                        Prop::place(mesh, &at)
                    })),
                    Err(e) => self.log.get_mut().push(format!("world: {} has no shapes for the plane: {e}", tile.name)),
                }
            }
            // A world that wraps (MD29): the shapes by a seam stand again one width over, after
            // every tile's own, so the plane's search meets them from the far side and every
            // shape keeps its number.
            let mut copied = Vec::new();
            for (prop, (mesh, at)) in all.props.iter().zip(&placed) {
                for by in self.copies(prop.centre, prop.radius) {
                    copied.push(Prop::place(mesh.clone(), &moved(at, by)));
                }
            }
            all.props.extend(copied);
            self.every = Some(all);
        }
        self.every.get_or_insert_with(Props::default)
    }

    /// Whether the put-back record may keep a spot ([`World::keeps`], MD14): a tile owns the ground
    /// under it, and the edge test would not throw from it — [`crate::off_map::MARGIN`] inside the
    /// world's rectangle and from every hole in it. Anywhere else a bike put back is on no ground
    /// (`y` −10⁶, and the camera's angles go with it), or thrown again on its first tick.
    pub fn keeps(&self, at: Vf) -> bool {
        let margin = crate::off_map::MARGIN;
        self.at(F(at.x()), F(at.z())).is_some()
            && !crate::off_map::outside_axes(&self.area, self.wrap, at, margin)
            && !self.off_world(at, margin)
    }

    /// **Every hole inside the world** (MD27): the rectangles of [`Tiled::area`] that no tile or join
    /// owns, `[x0, z0, x1, z1]` in world feet. The edge test throws a rider within
    /// [`crate::off_map::MARGIN`] of any of them ([`World::off_world`]), so a world with none throws
    /// only at its outer edge — never at a join, whatever its tiles' own edges were.
    ///
    /// Exact rather than sampled: the rectangle is cut on every piece's owned edges as
    /// [`Tile::local`] draws them (its `keep`, less the terrain's last cell, which no cell of the
    /// field starts), and the owner of each cut's middle is asked the way a ride asks it. A strip a
    /// few feet wide between two pieces is a cut of its own, which a grid of points can step over.
    pub fn holes(&self) -> Vec<[F; 4]> {
        let edges = |axis: usize| {
            let mut e: Vec<F> = vec![F::ZERO, if axis == 0 { self.area.width } else { self.area.depth }];
            for t in &self.tiles {
                let n = t.samples()[axis].saturating_sub(1);
                for k in [t.keep[axis], t.keep[axis + 2].min(n)] {
                    e.push(t.at[axis] + F::from(k as i32) * t.spacing);
                }
            }
            e.retain(|v| *v >= F::ZERO && *v <= if axis == 0 { self.area.width } else { self.area.depth });
            e.sort_by(|a, b| a.0.total_cmp(&b.0));
            e.dedup();
            e
        };
        let (xs, zs) = (edges(0), edges(1));
        let mut out = Vec::new();
        for z in zs.windows(2) {
            for x in xs.windows(2) {
                let (mx, mz) = ((x[0] + x[1]) / F(2.0), (z[0] + z[1]) / F(2.0));
                if self.hole(mx, mz) {
                    out.push([x[0], z[0], x[1], z[1]]);
                }
            }
        }
        out
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

/// A point moved along `x` and `z`.
fn shifted(v: Vf, by: [F; 2]) -> Vf {
    Vf::new((F(v.x()) + by[0]).0, v.y(), (F(v.z()) + by[1]).0, v.w())
}

/// Two vectors the same, word for word.
fn same(a: Vf, b: Vf) -> bool {
    a.0.iter().zip(b.0.iter()).all(|(a, b)| a.to_bits() == b.to_bits())
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

    fn wrap(&self) -> [bool; 2] {
        self.wrap
    }

    fn off_world(&self, at: Vf, margin: F) -> bool {
        let (x, z) = (F(at.x()), F(at.z()));
        // Only inside the rectangle: its own edge is the area's test, with the original's margin.
        // On a world that wraps, a point past a seam is the one a width back (MD29).
        let inside = |x: F, z: F| x >= F::ZERO && z >= F::ZERO && x < self.area.width && z < self.area.depth;
        [(x, z), (x - margin, z), (x + margin, z), (x, z - margin), (x, z + margin)]
            .into_iter()
            .map(|(x, z)| self.fold(x, z))
            .any(|(x, z)| inside(x, z) && self.hole(x, z))
    }

    fn heightfield(&mut self) -> Option<Box<dyn Heightfield + '_>> {
        Some(Box::new(self.field()))
    }

    fn keeps(&mut self, at: Vf) -> bool {
        Tiled::keeps(self, at)
    }

    fn collision(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>) {
        // The plane's search takes one set of shapes in one space: every tile's, together.
        self.every_shape();
        let (this, every) = (&*self, self.every.as_ref());
        (Some(Box::new(this.field())), every)
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
    /// The world's rectangle, and on a wrapped axis (MD29) a width more each way, so a search near a
    /// seam asks past it and [`Heightfield::tile`] folds the point back.
    fn min(&self) -> Vf {
        let [w, d] = self.0.wrap;
        let lo = |on: bool, side: F| if on { (F::ZERO - side).0 } else { 0.0 };
        Vf::new(lo(w, self.0.area.width), 0.0, lo(d, self.0.area.depth), 1.0)
    }
    fn max(&self) -> Vf {
        let [w, d] = self.0.wrap;
        let hi = |on: bool, side: F| if on { (side + side).0 } else { side.0 };
        Vf::new(hi(w, self.0.area.width), 0.0, hi(d, self.0.area.depth), 1.0)
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
    /// A track with its ecosystem's trunks standing in it (V50): the shell's ride of a scene that
    /// names an `.esb`. The trunks answer only what [`crate::eco::Wooded`] gives them; the camera's
    /// questions go to the track, as the original's camera asks only the terrain.
    Woods(Box<crate::eco::Wooded<Place<'a>>>),
}

impl Land<'_> {
    fn world(&mut self) -> &mut dyn World {
        match self {
            Land::Track(p) => p,
            Land::Tiles(t) => &mut **t,
            Land::Woods(w) => &mut **w,
        }
    }

    /// The world of tiles, when this is one.
    pub fn tiled(&mut self) -> Option<&mut Tiled> {
        match self {
            Land::Track(_) | Land::Woods(_) => None,
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
            Land::Woods(w) => w.area(),
        }
    }
    fn off_world(&self, at: Vf, margin: F) -> bool {
        match self {
            Land::Track(p) => p.off_world(at, margin),
            Land::Tiles(t) => t.off_world(at, margin),
            Land::Woods(w) => w.off_world(at, margin),
        }
    }
    fn wrap(&self) -> [bool; 2] {
        match self {
            Land::Track(p) => p.wrap(),
            Land::Tiles(t) => World::wrap(&**t),
            Land::Woods(w) => w.wrap(),
        }
    }
    fn keeps(&mut self, at: Vf) -> bool {
        self.world().keeps(at)
    }
    fn heightfield(&mut self) -> Option<Box<dyn Heightfield + '_>> {
        match self {
            Land::Track(p) => p.heightfield(),
            Land::Tiles(t) => t.heightfield(),
            Land::Woods(w) => w.heightfield(),
        }
    }
    fn collision(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>) {
        match self {
            Land::Track(p) => p.collision(),
            Land::Tiles(t) => t.collision(),
            Land::Woods(w) => w.collision(),
        }
    }
    fn wooded(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>, Option<&crate::eco::Trees>) {
        match self {
            Land::Track(p) => p.wooded(),
            Land::Tiles(t) => t.wooded(),
            Land::Woods(w) => w.wooded(),
        }
    }
    fn moving(&mut self) -> Option<&mut Props> {
        match self {
            Land::Track(p) => p.moving(),
            Land::Tiles(t) => t.moving(),
            Land::Woods(w) => w.moving(),
        }
    }
}

impl Surface for Land<'_> {
    fn on_surface(&mut self, point: Vf) -> Vf {
        match self {
            Land::Track(p) => p.on_surface(point),
            Land::Tiles(t) => t.on_surface(point),
            Land::Woods(w) => w.world.on_surface(point),
        }
    }
}

impl Obstacles for Land<'_> {
    fn nearest(&mut self, start: Vf, end: Vf) -> Option<Obstacle> {
        match self {
            Land::Track(p) => p.nearest(start, end),
            Land::Tiles(t) => t.nearest(start, end),
            Land::Woods(w) => w.world.nearest(start, end),
        }
    }
}
