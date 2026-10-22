//! How the world fills the broad phase (S96): the static quadtree laid over a track's field, and a
//! collidable added to it.
//!
//! The order a [`crate::broad_phase::gather`] hands candidates out in is the order the tree's walk
//! meets them — node by node, and within a node chunk by chunk, slot by slot — so where each collidable
//! went in, and in what order, is what decides the candidate order. Three steps, the original's:
//!
//! 1. **the tree** ([`tree_over`]) — a box on the ground plane, sized to a power of two of cells
//!    (32768 across its longer side), and as many levels as keep the smallest node at least a given
//!    width across;
//! 2. **the key** ([`key`]) — a collidable's box clipped to the tree, in cells: the smallest level
//!    whose nodes are wider than the box both ways, the corner cell, and which of the three neighbours
//!    to the right, below and diagonally the box spills into. A box off the tree has no key;
//! 3. **the insertion** ([`insert`]) — the collidable put in the node at that level under each corner
//!    its key names, the nodes on the way made as needed, and in each node into the first empty slot
//!    of its chunks, or a new chunk at the end.
//!
//! The box itself is the class's: a mesh's is its root node's box through its frame ([`mesh_box`]).

#![deny(clippy::float_arithmetic)]

use crate::broad_phase::{Node, Quadtree};
use crate::math::Mat4;
use ps2_float::{F, Vf};

/// The side of the tree's root, in cells, and its first level's.
const ROOT: i32 = 0x8000;
const FIRST: i32 = 0x4000;
/// The most levels a tree is given.
const LEVELS: i32 = 12;

/// An empty tree over the ground-plane box `lo`..`hi` (`x`, `z`), its smallest node at least `least`
/// world units across, where the level stops: the cells per world unit are 32768 over the longer side
/// (its whole units), and the levels halve from 16384 cells until one is no wider than `least` is in
/// cells — or twelve levels.
///
/// The second value is the world size of a cell, which the tree's queries report their boxes in
/// ([`crate::broad_phase::Broad::cell_size`]).
pub fn tree_over(lo: [F; 2], hi: [F; 2], least: F) -> (Quadtree, F) {
    let extent = [hi[0] - lo[0], hi[1] - lo[1]];
    let longer = if extent[1] < extent[0] { extent[0].to_i32() } else { extent[1].to_i32() };
    let scale = F::from(ROOT) / F::from(longer);
    let cell_size = F::ONE / scale;
    let (mut levels, mut size) = (0, ROOT);
    loop {
        size >>= 1;
        levels += 1;
        if LEVELS - 1 < levels || !((least * scale).to_i32() < size) {
            break;
        }
    }
    let tree = Quadtree {
        origin: lo,
        extent,
        cell: FIRST >> (levels - 1),
        scale,
        root: Some(0),
        nodes: vec![Node::default()],
        ..Quadtree::default()
    };
    (tree, cell_size)
}

/// The deepest level, and a level's node side in cells.
fn deepest(tree: &Quadtree) -> u32 {
    (FIRST / tree.cell).trailing_zeros()
}

fn side(level: u32) -> i32 {
    FIRST >> level
}

/// Where in `tree` the box `centre` ± `half` goes: its level (bits 0..4), the corners it spills into
/// (0x10 always, 0x20 the next node in `z`, 0x40 in `x`, 0x80 both), and its corner cell, `z` from bit
/// 5 and `x` from bit 17. `hi` is the far corner the tree was laid to ([`tree_over`]'s). 0 for a box
/// off the tree.
pub fn key(tree: &Quadtree, hi: [F; 2], centre: [F; 4], half: [F; 4]) -> u32 {
    let [ox, oz] = tree.origin;
    let x0 = (centre[0] - half[0]).max(ox) - ox;
    let x1 = (centre[0] + half[0]).min(hi[0]) - ox;
    let z0 = (centre[2] - half[2]).max(oz) - oz;
    let z1 = (centre[2] + half[2]).min(hi[1]) - oz;
    if x1 < 0.0 || !(x0 <= tree.extent[0]) || z1 < 0.0 || !(z0 <= tree.extent[1]) {
        return 0;
    }
    let s = tree.scale;
    let (cx0, cz0) = ((x0 * s).to_i32(), (z0 * s).to_i32());
    let (cx1, cz1) = ((x1 * s).to_i32().wrapping_add(1), (z1 * s).to_i32().wrapping_add(1));
    let (width, depth) = (cx1.wrapping_sub(cx0), cz1.wrapping_sub(cz0));
    let max = deepest(tree);
    let mut level = 0;
    while level <= max && width < side(level) && depth < side(level) {
        level += 1;
    }
    let level = level.min(max);
    let mask = |size: i32| !(size - 1);
    let size = side(level);
    let m = mask(size);
    let (mut x0, mut x1, mut z0, mut z1) = (cx0 & m, cx1 & m, cz0 & m, cz1 & m);
    let mut level = level;
    if (size < x1.wrapping_sub(x0) || size < z1.wrapping_sub(z0)) && level != 0 {
        level -= 1;
        let m = mask(size + size);
        (x0, x1, z0, z1) = (x0 & m, x1 & m, z0 & m, z1 & m);
    }
    let mut corners = 0x10;
    if z1 != z0 {
        corners |= 0x20;
    }
    if x1 != x0 {
        corners |= 0x40;
    }
    if corners & 0x60 == 0x60 {
        corners |= 0x80;
    }
    let x0 = if x0 < 0x8000 { x0 } else { 0x7fff };
    let z0 = if z0 < 0x8000 { z0 } else { 0x7fff };
    level | corners | (z0 as u32) << 5 | (x0 as u32) << 17
}

/// `o` put in `tree` where `key` says: under each corner the key names, down from the root to the
/// key's level (a node made wherever one is missing), into that node's first empty slot, or a new chunk
/// of four at the end of its chunks. Nothing for a key of 0.
pub fn insert(tree: &mut Quadtree, o: usize, key: u32) {
    if key == 0 {
        return;
    }
    let level = key & 0xf;
    let size = side(level);
    let x = ((key & 0xfff0_0000) >> 17) as i32;
    let z = ((key & 0xfff00) >> 5) as i32;
    for corner in 0..4 {
        if key & (0x10 << corner) == 0 {
            continue;
        }
        let x = x.wrapping_add(if corner & 2 != 0 { size } else { 0 });
        let z = z.wrapping_add(if corner & 1 != 0 { size } else { 0 });
        let mut node = match tree.root {
            Some(r) => r,
            None => {
                tree.nodes.push(Node::default());
                tree.root = Some(tree.nodes.len() - 1);
                tree.nodes.len() - 1
            }
        };
        let mut bit = FIRST;
        for _ in 0..=level {
            let i = 2 * usize::from(x & bit != 0) + usize::from(z & bit != 0);
            node = match tree.nodes[node].children[i] {
                Some(c) => c,
                None => {
                    tree.nodes.push(Node::default());
                    let c = tree.nodes.len() - 1;
                    tree.nodes[node].children[i] = Some(c);
                    c
                }
            };
            bit >>= 1;
        }
        let chunks = &mut tree.nodes[node].chunks;
        match chunks.iter_mut().flat_map(|c| c.iter_mut()).find(|s| s.is_none()) {
            Some(slot) => *slot = Some(o),
            None => chunks.push([Some(o), None, None, None]),
        }
    }
}

/// A mesh's box in the world, as the mesh class gives it: its root node's box, `centre` through the
/// frame `to` (the translation, then the three rows, on the vector unit) and `half` the sum of the rows'
/// magnitudes, each row scaled by its half. Both have `w` 1.
pub fn mesh_box(to: &Mat4, centre: [F; 3], half: [F; 3]) -> ([F; 4], [F; 4]) {
    let [r0, r1, r2, t] = to.0;
    let lane = |v: F| Vf([v.0; 4]);
    let a = r0.mul(lane(half[0])).abs();
    let b = r1.mul(lane(half[1])).abs();
    let c = r2.mul(lane(half[2])).abs();
    let acc = Vf::madd(t.add(Vf::ZERO), r0, lane(centre[0]));
    let acc = Vf::madd(acc, r1, lane(centre[1]));
    let mid = Vf::madd(acc, r2, lane(centre[2]));
    let size = Vf::madd(b.add(a), c, Vf([1.0; 4]));
    let four = |v: Vf| [F(v.x()), F(v.y()), F(v.z()), F::ONE];
    (four(mid), four(size))
}
