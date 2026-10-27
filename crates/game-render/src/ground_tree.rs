//! A ground piece's chunks in a quadtree (V65), so a frame's choice of what to draw costs what is in
//! view, not what the piece holds.
//!
//! Every node of the tree covers a square of the piece's chunks, a power of two a side, and carries a
//! sphere round them all and, for each level of detail, the worst and the least height error any of
//! them has there ([`crate::terrain::lod_errors`]). A frame walks it from the root:
//!
//! - a node the view does not see is culled whole, with nothing under it looked at;
//! - a node wholly in view whose worst errors at its near side pick the same level as its least
//!   errors at its far side is drawn *whole* at that level: every chunk under it would pick that
//!   level alone, so all of them are drawn at it at once and none is looked at. Far ground, where
//!   everything is at the coarsest level, goes whole a large branch at a time;
//! - anything else is split into its four quarters, down to the chunks.
//!
//! Then the levels are balanced, as [`crate::terrain::lod_balance`] balances a grid: two neighbours
//! drawn differ by one level at most, and a node drawn whole has no coarser neighbour, so only a
//! single chunk ever bends an edge onto a coarser one ([`crate::terrain::lod_indices`]) and a whole
//! node's chunks all draw unstitched. A balance that would make a whole node finer splits it instead,
//! so the finer levels stay where they are needed. Neighbours that are culled or not there hold
//! nobody back: nothing is drawn across that side to crack against.

use crate::mat::V3;

/// One chunk as the tree is built from it.
pub struct TreeChunk<'a> {
    /// Which chunk of the piece it is, across and along.
    pub at: (u32, u32),
    pub centre: V3,
    pub radius: f32,
    /// Its height error at each level (with its holes' edges, [`crate::terrain::lod_hole_errors`]).
    pub errors: &'a [f32],
}

struct Node {
    /// The square it covers, in chunks: its first corner and its side.
    x: u32,
    z: u32,
    size: u32,
    centre: V3,
    radius: f32,
    errors: Vec<f32>,
    /// The least height error any of its chunks has at each level.
    least: Vec<f32>,
    /// Its quarters, `NONE` where a quarter has no chunk; all `NONE` for a chunk's own node.
    kids: [u32; 4],
    /// The chunks under it, as a run of [`GroundTree::order`].
    first: u32,
    count: u32,
    /// The node it is a quarter of; `NONE` for the root.
    parent: u32,
}

const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    Culled,
    /// Drawn whole at a level.
    Whole(u32),
    /// A chunk's own node, drawn at a level.
    Leaf(u32),
    Split,
}

/// What a frame draws of a node: the node, the level its chunks draw at and, for a chunk's own node,
/// which of its sides border a coarser neighbour (bit `side` of [`crate::terrain::lod_indices`]'s
/// `coarser`). A whole node's mask is always 0.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pick {
    pub node: u32,
    pub level: u32,
    pub mask: u32,
}

/// A piece's chunks in a quadtree ([`GroundTree::select`]).
pub struct GroundTree {
    nodes: Vec<Node>,
    root: u32,
    /// The chunks, by their index in the list the tree was made from, in the tree's order: each node's
    /// are one run.
    order: Vec<u32>,
    /// Each node's state in the last [`GroundTree::select`], valid where its stamp is the current one.
    state: Vec<(u32, State)>,
    stamp: u32,
    /// The walk's own lists, kept from frame to frame.
    scratch: [Vec<u32>; 3],
}

/// How a view sees: `look(centre, radius)` is `None` for a sphere it does not draw, else how far its
/// eye is from the sphere's surface (0 inside it); `level(errors, distance)` is the coarsest level of
/// those errors that is fine enough that far ([`crate::terrain::lod_level`]); `holds(centre, radius)`
/// is whether the sphere is wholly in view, frustum and reach, and only a node it holds is drawn
/// whole, so nothing behind the camera or past the fog is drawn for it.
pub struct Look<'a> {
    pub look: &'a dyn Fn(V3, f32) -> Option<f32>,
    pub level: &'a dyn Fn(&[f32], f32) -> u32,
    pub holds: &'a dyn Fn(V3, f32) -> bool,
}

impl GroundTree {
    pub fn new(chunks: &[TreeChunk]) -> GroundTree {
        let across = chunks.iter().map(|c| c.at.0 + 1).max().unwrap_or(0);
        let along = chunks.iter().map(|c| c.at.1 + 1).max().unwrap_or(0);
        let mut grid = vec![NONE; (across * along) as usize];
        for (i, c) in chunks.iter().enumerate() {
            grid[(c.at.1 * across + c.at.0) as usize] = i as u32;
        }
        let mut t = GroundTree { nodes: Vec::new(), root: NONE, order: Vec::with_capacity(chunks.len()), state: Vec::new(), stamp: 0, scratch: Default::default() };
        let side = across.max(along).max(1).next_power_of_two();
        t.root = t.build(chunks, &grid, across, along, 0, 0, side);
        t.state = vec![(0, State::Culled); t.nodes.len()];
        t
    }

    #[allow(clippy::too_many_arguments)]
    fn build(&mut self, chunks: &[TreeChunk], grid: &[u32], across: u32, along: u32, x: u32, z: u32, size: u32) -> u32 {
        if x >= across || z >= along {
            return NONE;
        }
        let first = self.order.len() as u32;
        if size == 1 {
            let i = grid[(z * across + x) as usize];
            if i == NONE {
                return NONE;
            }
            let c = &chunks[i as usize];
            self.order.push(i);
            self.nodes.push(Node { x, z, size, centre: c.centre, radius: c.radius, errors: c.errors.to_vec(), least: c.errors.to_vec(), kids: [NONE; 4], first, count: 1, parent: NONE });
            return self.nodes.len() as u32 - 1;
        }
        let h = size / 2;
        let kids = [(0, 0), (h, 0), (0, h), (h, h)].map(|(dx, dz)| self.build(chunks, grid, across, along, x + dx, z + dz, h));
        let real: Vec<&Node> = kids.iter().filter(|&&k| k != NONE).map(|&k| &self.nodes[k as usize]).collect();
        if real.is_empty() {
            return NONE;
        }
        // A sphere round the kids' spheres: their box's middle, out to the farthest of them.
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for k in &real {
            let c = [k.centre.x, k.centre.y, k.centre.z];
            for a in 0..3 {
                lo[a] = lo[a].min(c[a] - k.radius);
                hi[a] = hi[a].max(c[a] + k.radius);
            }
        }
        let centre = V3 { x: (lo[0] + hi[0]) / 2.0, y: (lo[1] + hi[1]) / 2.0, z: (lo[2] + hi[2]) / 2.0 };
        let radius = real.iter().map(|k| k.centre.sub(centre).len() + k.radius).fold(0.0, f32::max);
        let levels = real.iter().map(|k| k.errors.len()).max().unwrap_or(0);
        let errors = (0..levels).map(|l| real.iter().map(|k| k.errors.get(l).copied().unwrap_or(f32::MAX)).fold(0.0, f32::max)).collect();
        let least = (0..levels).map(|l| real.iter().map(|k| k.least.get(l).copied().unwrap_or(f32::MAX)).fold(f32::MAX, f32::min)).collect();
        let count = self.order.len() as u32 - first;
        self.nodes.push(Node { x, z, size, centre, radius, errors, least, kids, first, count, parent: NONE });
        let me = self.nodes.len() as u32 - 1;
        for k in kids.into_iter().filter(|&k| k != NONE) {
            self.nodes[k as usize].parent = me;
        }
        me
    }

    /// The chunks under `node`, by their index in the list the tree was made from.
    pub fn chunks(&self, node: u32) -> &[u32] {
        let n = &self.nodes[node as usize];
        &self.order[n.first as usize..(n.first + n.count) as usize]
    }

    /// The square `node` covers, in chunks: its first corner, across and along, and its side.
    pub fn square(&self, node: u32) -> (u32, u32, u32) {
        let n = &self.nodes[node as usize];
        (n.x, n.z, n.size)
    }

    /// How many nodes the tree has.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    fn get(&self, n: u32) -> Option<State> {
        let (stamp, s) = self.state[n as usize];
        (stamp == self.stamp).then_some(s)
    }

    fn set(&mut self, n: u32, s: State) {
        self.state[n as usize] = (self.stamp, s);
    }

    /// What one view draws of the piece, into `picks` in the tree's order, balanced and stitched as
    /// the module's doc says. Returns how many nodes it looked at: the frame's cost, which grows with
    /// what is in view rather than with the piece.
    pub fn select(&mut self, look: &Look, picks: &mut Vec<Pick>) -> usize {
        picks.clear();
        if self.root == NONE {
            return 0;
        }
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.state.iter_mut().for_each(|s| s.0 = 0);
            self.stamp = 1;
        }
        let mut visits = 0;
        let [mut queue, mut stack, mut near] = std::mem::take(&mut self.scratch);
        queue.clear();
        stack.clear();
        stack.push(self.root);
        // Nothing is coarser than the coarsest level.
        let top = (self.nodes[self.root as usize].errors.len() as u32).saturating_sub(1);
        while let Some(n) = stack.pop() {
            visits += 1;
            let node = &self.nodes[n as usize];
            let Some(d) = (look.look)(node.centre, node.radius) else {
                self.set(n, State::Culled);
                continue;
            };
            if node.kids == [NONE; 4] {
                let level = (look.level)(&node.errors, d);
                self.set(n, State::Leaf(level));
                queue.push(n);
                continue;
            }
            let level = (look.level)(&node.errors, d);
            let beyond = d + 2.0 * node.radius;
            if level == (look.level)(&node.least, beyond) && (look.holds)(node.centre, node.radius) {
                self.set(n, State::Whole(level));
                queue.push(n);
                continue;
            }
            let kids = node.kids;
            self.set(n, State::Split);
            stack.extend(kids.iter().rev().filter(|&&k| k != NONE));
        }
        // The balance: lower whatever is too coarse beside something, until nothing is.
        while let Some(n) = queue.pop() {
            let (level, whole) = match self.get(n) {
                Some(State::Leaf(l)) => (l, false),
                Some(State::Whole(l)) => (l, true),
                _ => continue,
            };
            let cap = if whole { level } else { level + 1 };
            if cap >= top {
                continue;
            }
            for side in 0..4 {
                near.clear();
                let r = self.beside(n, side);
                self.drawn_in(self.holding(n, r), r, &mut near);
                for &m in &near {
                    match self.get(m) {
                        Some(State::Leaf(l)) if l > cap => {
                            self.set(m, State::Leaf(cap));
                            queue.push(m);
                        }
                        Some(State::Whole(l)) if l > cap => {
                            // Split it rather than draw all of it finer: its quarters keep its level
                            // and only the ones beside `n` go on being lowered.
                            self.set(m, State::Split);
                            let kids = self.nodes[m as usize].kids;
                            for k in kids.into_iter().filter(|&k| k != NONE) {
                                visits += 1;
                                let node = &self.nodes[k as usize];
                                let s = match (look.look)(node.centre, node.radius) {
                                    None => State::Culled,
                                    Some(_) if node.kids == [NONE; 4] => State::Leaf(l),
                                    Some(_) => State::Whole(l),
                                };
                                self.set(k, s);
                                queue.push(k);
                            }
                            queue.push(n);
                        }
                        _ => {}
                    }
                }
            }
        }
        // What is drawn, in the tree's order, a chunk's own node with its coarser sides.
        stack.push(self.root);
        while let Some(n) = stack.pop() {
            match self.get(n) {
                Some(State::Split) => stack.extend(self.nodes[n as usize].kids.iter().rev().filter(|&&k| k != NONE)),
                Some(State::Whole(level)) => picks.push(Pick { node: n, level, mask: 0 }),
                Some(State::Leaf(level)) => {
                    let mut mask = 0;
                    for side in (0..4).filter(|_| level < top) {
                        let [x0, z0, x1, z1] = self.beside(n, side);
                        if x0 < x1 && z0 < z1 && self.level_at(self.holding(n, [x0, z0, x1, z1]), x0, z0).is_some_and(|l| l > level) {
                            mask |= 1 << side;
                        }
                    }
                    picks.push(Pick { node: n, level, mask });
                }
                _ => {}
            }
        }
        self.scratch = [queue, stack, near];
        visits
    }

    /// The chunks just past side `side` of node `n` ([`crate::terrain::SIDE_Z0`] and on), as
    /// `[x0, z0, x1, z1)`; empty past the tree's first edges.
    fn beside(&self, n: u32, side: usize) -> [u32; 4] {
        let Node { x, z, size, .. } = self.nodes[n as usize];
        match side {
            crate::terrain::SIDE_Z0 if z > 0 => [x, z - 1, x + size, z],
            crate::terrain::SIDE_X1 => [x + size, z, x + size + 1, z + size],
            crate::terrain::SIDE_Z1 => [x, z + size, x + size, z + size + 1],
            crate::terrain::SIDE_X0 if x > 0 => [x - 1, z, x, z + size],
            _ => [0; 4],
        }
    }

    /// The nearest node above `n` whose square holds `r` (`[x0, z0, x1, z1)`), or the root: where a
    /// look beside `n` starts, rather than at the root every time.
    fn holding(&self, n: u32, r: [u32; 4]) -> u32 {
        let mut a = self.nodes[n as usize].parent;
        while a != NONE {
            let node = &self.nodes[a as usize];
            if node.x <= r[0] && r[2] <= node.x + node.size && node.z <= r[1] && r[3] <= node.z + node.size {
                return a;
            }
            a = node.parent;
        }
        self.root
    }

    /// The nodes drawn whole or as chunks that overlap `[x0, z0, x1, z1)`, under `n`.
    fn drawn_in(&self, n: u32, r: [u32; 4], out: &mut Vec<u32>) {
        if n == NONE || r[0] >= r[2] || r[1] >= r[3] {
            return;
        }
        let node = &self.nodes[n as usize];
        if node.x >= r[2] || node.x + node.size <= r[0] || node.z >= r[3] || node.z + node.size <= r[1] {
            return;
        }
        match self.get(n) {
            Some(State::Whole(_) | State::Leaf(_)) => out.push(n),
            Some(State::Split) => {
                for k in node.kids {
                    self.drawn_in(k, r, out);
                }
            }
            _ => {}
        }
    }

    /// The level chunk `(x, z)` is drawn at, if it is drawn, looking from `n` down.
    fn level_at(&self, from: u32, x: u32, z: u32) -> Option<u32> {
        let mut n = from;
        while n != NONE {
            let node = &self.nodes[n as usize];
            if x < node.x || x >= node.x + node.size || z < node.z || z >= node.z + node.size {
                return None;
            }
            match self.get(n)? {
                State::Whole(l) | State::Leaf(l) => return Some(l),
                State::Culled => return None,
                State::Split => {
                    let h = node.size / 2;
                    let q = usize::from(x >= node.x + h) + 2 * usize::from(z >= node.z + h);
                    n = node.kids[q];
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mat::v3;
    use crate::terrain::{SIDE_X0, SIDE_X1, SIDE_Z0, SIDE_Z1, lod_errors, lod_hole_errors, lod_level};

    const CELLS: u32 = 16;
    const FEET: f32 = 16.0 * 9.0;

    /// A ground `n` chunks a side of rolling hills, with every `holes`-th chunk holed.
    fn ground(n: u32, holes: u32) -> (Vec<(u32, u32)>, Vec<V3>, Vec<Vec<f32>>, Vec<bool>) {
        let (mut at, mut centre, mut errors, mut holed) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for cz in 0..n {
            for cx in 0..n {
                let h = |i: u32, j: u32| {
                    let (x, z) = ((cx * CELLS + i) as f32 * 9.0, (cz * CELLS + j) as f32 * 9.0);
                    20.0 * (x / 300.0).sin() * (z / 410.0).cos() + 3.0 * (x / 37.0).sin()
                };
                let heights: Vec<f32> = (0..=CELLS).flat_map(|j| (0..=CELLS).map(move |i| (i, j))).map(|(i, j)| h(i, j)).collect();
                at.push((cx, cz));
                centre.push(v3((cx as f32 + 0.5) * FEET, 0.0, (cz as f32 + 0.5) * FEET));
                let holed_here = holes > 0 && (cx * 7 + cz * 3) % holes == 0;
                let mut e = lod_errors(&heights, CELLS);
                if holed_here {
                    lod_hole_errors(&mut e, 9.0);
                }
                errors.push(e);
                holed.push(holed_here);
            }
        }
        (at, centre, errors, holed)
    }

    fn tree(g: &(Vec<(u32, u32)>, Vec<V3>, Vec<Vec<f32>>, Vec<bool>)) -> GroundTree {
        let chunks: Vec<TreeChunk> = (0..g.0.len()).map(|i| TreeChunk { at: g.0[i], centre: g.1[i], radius: FEET * 0.75, errors: &g.2[i] }).collect();
        GroundTree::new(&chunks)
    }

    /// A camera at `eye` looking along +x with a 90° view, drawing out to `reach`.
    fn select(t: &mut GroundTree, eye: V3, reach: f32) -> (Vec<Pick>, usize) {
        let look = move |c: V3, r: f32| {
            let d = c.sub(eye);
            let far = d.len() - r;
            // In front of the eye, within 45° either side of +x.
            let inside = d.x + r * std::f32::consts::SQRT_2 >= d.z.abs();
            (far <= reach && inside).then_some(far.max(0.0))
        };
        let holds = move |c: V3, r: f32| {
            let d = c.sub(eye);
            d.len() + r <= reach && d.x - r * std::f32::consts::SQRT_2 >= d.z.abs()
        };
        let level = |e: &[f32], d: f32| lod_level(e, d, 1000.0, 0.75);
        let mut picks = Vec::new();
        let visits = t.select(&Look { look: &look, level: &level, holds: &holds }, &mut picks);
        (picks, visits)
    }

    /// Every drawn chunk's level and coarser sides.
    fn per_chunk(t: &GroundTree, g: &(Vec<(u32, u32)>, Vec<V3>, Vec<Vec<f32>>, Vec<bool>), picks: &[Pick]) -> std::collections::HashMap<(u32, u32), (u32, u32)> {
        let mut out = std::collections::HashMap::new();
        for p in picks {
            for &c in t.chunks(p.node) {
                assert!(out.insert(g.0[c as usize], (p.level, p.mask)).is_none(), "a chunk drawn twice");
            }
        }
        out
    }

    #[test]
    fn the_nodes_looked_at_grow_with_the_view_not_the_ground() {
        // Far Fog's finite reach over a ground and one four times as wide, from the same place.
        let eye = v3(10.0 * FEET, 30.0, 10.5 * FEET);
        let reach = 30.0 * FEET;
        let small = ground(64, 0);
        let wide = ground(256, 0);
        let (sp, sv) = select(&mut tree(&small), eye, reach);
        let (wp, wv) = select(&mut tree(&wide), eye, reach);
        assert!(!sp.is_empty() && !wp.is_empty());
        // The wide ground's extra branches are culled at their roots: a few more nodes, not sixteen
        // times as many.
        assert!(wv <= sv + 4 * 8, "the small ground {sv} nodes, the wide {wv}");
        assert!(wv < 64 * 64, "{wv} nodes looked at");
        // With no reach, the far branches go whole: the whole wide ground costs about as many
        // nodes as the piece of it in view, far fewer than its chunks.
        let (fp, fv) = select(&mut tree(&wide), eye, f32::INFINITY);
        assert!(fv < 256 * 256 / 8, "{fv} nodes looked at for {} picks", fp.len());
        assert!(fp.iter().any(|p| p.mask == 0 && tree(&wide).chunks(p.node).len() > 16), "no branch drawn whole");
    }

    #[test]
    fn no_crack_between_the_levels_drawn() {
        for (n, holes, reach) in [(64, 0, f32::INFINITY), (64, 5, f32::INFINITY), (128, 0, 40.0 * FEET), (40, 3, 20.0 * FEET)] {
            let g = ground(n, holes);
            let mut t = tree(&g);
            let mut seen = 0;
            for eye in [v3(2.0 * FEET, 10.0, 20.0 * FEET), v3(20.3 * FEET, 200.0, 20.7 * FEET), v3(-30.0 * FEET, 900.0, 5.0 * FEET)] {
                let (picks, _) = select(&mut t, eye, reach);
                let drawn = per_chunk(&t, &g, &picks);
                seen += drawn.len();
                for (&(x, z), &(level, mask)) in &drawn {
                    // Every side: a drawn neighbour differs by one at most, and a coarser one is
                    // stitched to by exactly this chunk's mask (V32's lod_indices does the rest).
                    for (side, other) in [(SIDE_Z0, z.checked_sub(1).map(|z| (x, z))), (SIDE_X1, Some((x + 1, z))), (SIDE_Z1, Some((x, z + 1))), (SIDE_X0, x.checked_sub(1).map(|x| (x, z)))] {
                        let Some(&(theirs, their_mask)) = other.and_then(|o| drawn.get(&o)) else {
                            assert_eq!(mask & (1 << side), 0, "({x},{z}) stitched to nothing on side {side}");
                            continue;
                        };
                        assert!(level.abs_diff(theirs) <= 1, "({x},{z}) at {level} beside {theirs} on side {side}");
                        assert_eq!(mask & (1 << side) != 0, theirs > level, "({x},{z}) at {level} side {side} beside {theirs}");
                        let back = (side + 2) % 4;
                        assert_eq!(their_mask & (1 << back) != 0, level > theirs);
                    }
                }
            }
            assert!(seen > 0, "{n} chunks a side: nothing drawn");
        }
    }

    #[test]
    fn a_whole_node_is_never_coarser_than_its_chunks_would_be_alone() {
        let g = ground(64, 0);
        let mut t = tree(&g);
        let eye = v3(3.0 * FEET, 40.0, 30.0 * FEET);
        let (picks, _) = select(&mut t, eye, f32::INFINITY);
        for (&(x, z), &(level, _)) in &per_chunk(&t, &g, &picks) {
            let i = g.0.iter().position(|&a| a == (x, z)).expect("there");
            let d = (g.1[i].sub(eye).len() - FEET * 0.75).max(0.0);
            assert!(level <= lod_level(&g.2[i], d, 1000.0, 0.75), "({x},{z}) drawn coarser than it would pick");
        }
    }
}
