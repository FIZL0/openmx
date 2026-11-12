//! **SLUS-21104's attachment move on RAM** (F43b): the call the bike's vehicle update
//! ([`super::update::prepare`]) ends on, the rider's attachment moved in the world's spatial
//! database to where its node now is, as a word-level port over the database in memory.
//! [`crate::atv_spatial`] holds the same move as structs for the quad; the rider's database, its
//! sub-databases' blocks and nodes and the link pool are where the original keeps them, and grow and
//! shrink through its heap, so these read and write them there. Where the original keeps them:
//! research/oracle.txt `# F43b`.
//!
//! - [`moved`]: the attachment's box ([`bounds`]) from its node, then [`spatial_move`] with its
//!   reach. An attachment with no record does nothing.
//! - [`bounds`]: the node's world frame brought up to date ([`node::update`]); the box's centre is
//!   the attachment's local centre carried out of it, its half the local half's three axes'
//!   magnitudes summed.
//! - [`spatial_move`]: the walk through the portals, the record moved in its cell's sub-database or
//!   into another cell's, the extra cells brought up to date, and the centre kept — as
//!   [`crate::atv_spatial::spatial_move`] says, word for word.
//!
//! An attachment's words: its record (`+8`), its node (`+0x34`), its local centre (`+0x40`) and half
//! (`+0x50`), its reach (`+0x60`). A record's: the extras (`+0`), the cell (`+4`), the sub-database
//! (`+8`) and the place in it (`+0xc`), the last centre (`+0x10`). A cell's: its links (`+0x24`: the
//! next, the cell beyond, the portal), its two sub-databases (`+0x28`, `+0x2c`). A pool's: links in a
//! chunk (`+8`), the chunks in use (`+0xc` to `+0x10`) and set aside (`+0x14`), links handed out of
//! the last (`+0x18`), the stride (`+0x1c`), a chunk's bytes (`+0x20`) and header (`+0x38`), the
//! freed links (`+0x24`), the next link (`+0x28`), the links' constructor (`+0x2c`), the heap the
//! chunks come out of (`+0x30`, 0 the main one).
//!
//! **What is not the original's to the word.** The sub-databases' calls go through their vtables in
//! the original; here [`Space::class`] names the class and the port runs that class's. A tree node's
//! delete goes through the node's vtable too: the port runs the one class the tree makes.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use super::node::{self, Ram};
use super::update::Quad;
use crate::atv_spatial::{self, Portal, Tree};
use ps2_float::vu::{Dest, Field};
use ps2_float::{F, Vf};

/// A sub-database's class, as its vtable (`+4`) says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// The moving entries' slots in blocks.
    List,
    /// The quadtree (MD89).
    Tree,
    /// One not ported.
    Other,
}

/// Which of the original's heap calls an allocation is: each its own file and line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    /// A list's block table, reallocated one longer.
    Blocks,
    /// A list's new block of slots.
    Block,
    /// A pool's new chunk off the main heap.
    Chunk,
    /// A pool's new chunk off its own heap (the pool's `+0x30`).
    ChunkIn(u32),
    /// A tree node reallocated to take its four children.
    Grow,
    /// A new tree node.
    Node,
    /// A collidable's new record, as it is put in a world ([`insert`]).
    Record,
    /// The link pool, made by the first record when there is none.
    Pool,
}

/// What the database's memory needs beyond words.
pub trait Space: Ram {
    /// The class of the sub-database at `sub`.
    fn class(&mut self, sub: u32) -> Class;
    /// The original's heap: `bytes` from `site` (0 when it has none).
    fn alloc(&mut self, bytes: u32, site: Site) -> u32;
    /// `at` grown or shrunk to `bytes`, its words kept, from `site`.
    fn realloc(&mut self, at: u32, bytes: u32, site: Site) -> u32;
    /// `at` handed back.
    fn free(&mut self, at: u32);
    /// A pool's constructor `ctor` run on a new chunk's link at `link`.
    fn construct(&mut self, ctor: u32, link: u32);
    /// The link pool at `pool` deleted (its class's deleting destructor, its vtable's `+0x10`), when
    /// the last record lets go of it.
    fn delete_pool(&mut self, pool: u32);
}

/// The original's globals and classes the database's calls read and write.
#[derive(Clone, Copy, Debug)]
pub struct Globals {
    /// Where the pointer to the database's link pool is.
    pub links: u32,
    /// Where the pointer to the tree being worked on is (a tree's put or take-out sets it; its new and
    /// freed nodes are counted in it).
    pub current: u32,
    /// A tree node's vtable, and the one under it its constructor and destructor pass through.
    pub node_class: u32,
    pub node_base: u32,
}

fn word_f(m: &mut impl Ram, at: u32) -> F {
    F(f32::from_bits(m.word(at)))
}

fn set_f(m: &mut impl Ram, at: u32, x: F) {
    m.set(at, x.to_bits());
}

fn quad(m: &mut impl Ram, at: u32) -> Quad {
    [0, 4, 8, 12].map(|k| m.word(at.wrapping_add(k)))
}

fn v(m: &mut impl Ram, at: u32) -> Vf {
    Vf::from_bits(quad(m, at))
}

fn set_v(m: &mut impl Ram, at: u32, x: Vf) {
    for (k, w) in x.to_bits().into_iter().enumerate() {
        m.set(at.wrapping_add(4 * k as u32), w);
    }
}

fn add(m: &mut impl Ram, at: u32, by: u32) {
    let w = m.word(at);
    m.set(at, w.wrapping_add(by));
}

/// **The attachment's box**: its node brought up to date, the centre `p + R·c` (the local centre `c`
/// carried by the node's world rows `R` and position `p`, on VU0's accumulator from `p + 0`) and the
/// half `|R₀·hx| + |R₁·hy| + |R₂·hz|` (its `x`, `y`, `z` rows scaled by the local half's). Each keeps
/// its local quadword's `w` (the original's stores of 1 there are overwritten by the whole quadword).
pub fn bounds(m: &mut impl Ram, entry: u32) -> (Vf, Vf) {
    let n = m.word(entry + 0x34);
    node::update(m, n);
    let n = m.word(entry + 0x34);
    let (local, half) = (v(m, entry + 0x40), v(m, entry + 0x50));
    let rows = [0, 1, 2, 3].map(|r| v(m, n + 0x70 + 0x10 * r));
    let a = rows[0].mul(half.bc(Field::X)).abs();
    let b = rows[1].mul(half.bc(Field::Y)).abs();
    let c = rows[2].mul(half.bc(Field::Z)).abs();
    let acc = rows[3].add(Vf::splat(0.0));
    let acc = Vf::madd(acc, rows[0], local.bc(Field::X));
    let acc = Vf::madd(acc, rows[1], local.bc(Field::Y));
    let centre = Vf::madd(acc, rows[2], local.bc(Field::Z));
    let size = Vf::madd(b.add(a), c, Vf::splat(1.0));
    (local.merge(Dest::XYZ, centre), half.merge(Dest::XYZ, size))
}

/// **The attachment moved**: nothing without a record (`+8`); else its box ([`bounds`]) moved in the
/// database with its reach (`+0x60`).
pub fn moved(m: &mut impl Space, g: &Globals, entry: u32) {
    if m.word(entry + 8) == 0 {
        return;
    }
    let (centre, half) = bounds(m, entry);
    let reach = word_f(m, entry + 0x60);
    spatial_move(m, g, entry, centre, half, reach);
}

// --- the pools ------------------------------------------------------------------------------------------

/// A link out of pool `p`: the last chunk's next while it has any, else the last one freed, else a
/// new chunk's first ([`chunk`]).
fn take(m: &mut impl Space, p: u32) -> u32 {
    if (m.word(p + 0x18) as i32) < (m.word(p + 8) as i32) {
        let l = m.word(p + 0x28);
        let stride = m.word(p + 0x1c);
        m.set(p + 0x28, l.wrapping_add(stride));
        add(m, p + 0x18, 1);
        return l;
    }
    let head = m.word(p + 0x24);
    if head != 0 {
        let next = m.word(head);
        m.set(p + 0x24, next);
        return head;
    }
    chunk(m, p)
}

/// Pool `p`'s chunk `c` chained last and ended.
fn chain(m: &mut impl Space, p: u32, c: u32) {
    if m.word(p + 0xc) != 0 {
        let last = m.word(p + 0x10);
        m.set(last, c);
        m.set(p + 0x10, c);
    } else {
        m.set(p + 0x10, c);
        m.set(p + 0xc, c);
    }
}

/// A new chunk's first link: one set aside if there is one, else one off the heap (each of its links
/// constructed if the pool has a constructor; 0 if the heap has none).
fn chunk(m: &mut impl Space, p: u32) -> u32 {
    let spare = m.word(p + 0x14);
    if spare != 0 {
        chain(m, p, spare);
        let s = m.word(p + 0x14);
        let next = m.word(s);
        m.set(p + 0x14, next);
        let last = m.word(p + 0x10);
        m.set(last, 0);
    } else {
        let bytes = m.word(p + 0x20).wrapping_add(m.word(p + 0x38));
        let heap = m.word(p + 0x30);
        let c = m.alloc(bytes, if heap == 0 { Site::Chunk } else { Site::ChunkIn(heap) });
        if c == 0 {
            return 0;
        }
        let ctor = m.word(p + 0x2c);
        if ctor != 0 {
            let mut at = c.wrapping_add(m.word(p + 0x38));
            let mut k = 0;
            while k < m.word(p + 8) as i32 {
                m.construct(ctor, at);
                k += 1;
                at = at.wrapping_add(m.word(p + 0x1c));
            }
        }
        chain(m, p, c);
        let last = m.word(p + 0x10);
        m.set(last, 0);
    }
    let first = m.word(p + 0x10).wrapping_add(m.word(p + 0x38));
    let next = first.wrapping_add(m.word(p + 0x1c));
    m.set(p + 0x28, next);
    m.set(p + 0x18, 1);
    first
}

/// Link `l` handed back to pool `p`, first on its free chain (a pool of links no bigger than the
/// chain's word keeps none).
fn give(m: &mut impl Ram, p: u32, l: u32) {
    if m.word(p + 0x1c) > 4 {
        let head = m.word(p + 0x24);
        m.set(l, head);
        m.set(p + 0x24, l);
    }
}

/// Every link of pool `p` handed back at once: the chunks in use set aside in front of those set
/// aside already, the free chain dropped.
fn reset(m: &mut impl Ram, p: u32) {
    if m.word(p + 0xc) != 0 {
        let spare = m.word(p + 0x14);
        if spare != 0 {
            let last = m.word(p + 0x10);
            if last != 0 {
                m.set(last, spare);
            }
        }
        let first = m.word(p + 0xc);
        m.set(p + 0x14, first);
    }
    m.set(p + 0xc, 0);
    m.set(p + 0x10, 0);
    let n = m.word(p + 8);
    m.set(p + 0x18, n);
    m.set(p + 0x24, 0);
}

// --- the list -------------------------------------------------------------------------------------------

/// vt `+0x20`: the slot an entry goes in — `current` if it has one, else the first free one, else the
/// last block's next (a new block when it is full).
fn list_pick(m: &mut impl Space, s: u32, current: u32) -> u32 {
    if current != 0 {
        return current;
    }
    let free = m.word(s + 0x38);
    if free != 0 {
        return free;
    }
    if m.word(s + 0x30) == m.word(s + 0x28) {
        let (count, blocks) = (m.word(s + 0x2c), m.word(s + 0x34));
        let blocks = m.realloc(blocks, count.wrapping_add(1).wrapping_mul(4), Site::Blocks);
        m.set(s + 0x34, blocks);
        let size = m.word(s + 0x28);
        let block = m.alloc(size.wrapping_mul(4), Site::Block);
        let (count, blocks) = (m.word(s + 0x2c), m.word(s + 0x34));
        m.set(blocks.wrapping_add(count.wrapping_mul(4)), block);
        add(m, s + 0x2c, 1);
        m.set(s + 0x30, 0);
    }
    let (count, blocks, last) = (m.word(s + 0x2c), m.word(s + 0x34), m.word(s + 0x30));
    let block = m.word(blocks.wrapping_add(count.wrapping_mul(4)).wrapping_sub(4));
    block.wrapping_add(last.wrapping_mul(4))
}

/// vt `+0x24`: `entry` in `slot`, the first free one (the chain moves on) or the last block's next.
fn list_put(m: &mut impl Ram, s: u32, entry: u32, slot: u32) {
    let free = m.word(s + 0x38);
    if free != 0 && slot == free {
        let next = m.word(free) & !3;
        m.set(s + 0x38, next);
    } else {
        add(m, s + 0x30, 1);
    }
    m.set(slot, entry);
}

/// vt `+0x2c`: `slot` freed, first on the chain (tagged with bit 1).
fn list_out(m: &mut impl Ram, s: u32, slot: u32) {
    let free = m.word(s + 0x38);
    m.set(slot, free | 2);
    m.set(s + 0x38, slot);
}

// --- the tree -------------------------------------------------------------------------------------------

/// The tree at `t` as [`Tree::pick`] reads it.
fn tree_view(m: &mut impl Ram, t: u32) -> Tree {
    let deepest = m.word(t + 0x4c) as i32;
    let sides = (0..=deepest).map(|l| m.word(t + 0x58 + 4 * l as u32) as i32).collect();
    let (origin, extent) = ([word_f(m, t + 0x28), word_f(m, t + 0x30)], [word_f(m, t + 0x40), word_f(m, t + 0x44)]);
    let (kind, scale) = (m.word(t + 8), word_f(m, t + 0x54));
    Tree::new(kind, origin, extent, sides, scale, Default::default(), Default::default())
}

/// vt `+0x40`: an open query closed — its pool's links handed back, its walk and cells cleared.
fn close(m: &mut impl Ram, t: u32) {
    if m.word(t + 0xc) & 1 == 0 {
        return;
    }
    let q = m.word(t + 0x94);
    reset(m, q);
    for o in [0x9c, 0xa0, 0xa4, 0xac] {
        m.set(t + o, 0);
    }
    for o in [0x8a0, 0x8a4, 0x8a8, 0x8ac] {
        m.set(t + o, 0x8000_0000);
    }
    let f = m.word(t + 0xc);
    m.set(t + 0xc, f & !1);
}

/// Node `n`'s height (`+8` the middle, `+0xc` half of it) widened to take in `lo`..`hi` (nothing when
/// `hi` is below `lo`).
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
fn widen(m: &mut impl Ram, n: u32, lo: F, hi: F) {
    if !(lo <= hi) {
        return;
    }
    let (mid, half) = (word_f(m, n + 8), word_f(m, n + 0xc));
    let mut a = mid - half;
    if lo < a {
        a = lo;
    }
    let mut b = mid + half;
    if !(hi <= b) {
        b = hi;
    }
    set_f(m, n + 8, (a + b) / F(2.0));
    set_f(m, n + 0xc, (b - a) / F(2.0));
}

/// The corner cells a key names (0x10 its own, 0x20 the next in `z`, 0x40 in `x`, 0x80 both), at its
/// level's node side; and its level.
fn corners(m: &mut impl Ram, t: u32, key: u32) -> (u32, Vec<(i32, i32)>) {
    let level = key & 0xf;
    let side = m.word(t + 0x58 + 4 * level) as i32;
    let (x, z) = (((key & 0xfff0_0000) >> 17) as i32, ((key & 0xfff00) >> 5) as i32);
    let cells = (0..4)
        .filter(|c| key & (0x10 << c) != 0)
        .map(|c| {
            let dx = if c & 2 != 0 { side } else { 0 };
            let dz = if c & 1 != 0 { side } else { 0 };
            (x.wrapping_add(dx), z.wrapping_add(dz))
        })
        .collect();
    (level, cells)
}

/// Which child the cell (`x`, `z`) is under at the level whose bit is `bit`.
fn child_of(x: i32, z: i32, bit: i32) -> u32 {
    2 * u32::from(x & bit != 0) + u32::from(z & bit != 0)
}

/// The box's height: its centre's `y` less and plus its half's.
fn height(centre: Vf, half: Vf) -> (F, F) {
    let (c, h) = (F(centre.y()), F(half.y()));
    (c - h, c + h)
}

/// vt `+0x4c`: a new node (the heap's), constructed and counted in the current tree.
fn make(m: &mut impl Space, g: &Globals) -> u32 {
    let n = m.alloc(0x14, Site::Node);
    if n != 0 {
        m.set(n + 0x10, g.node_base);
        m.set(n + 4, 0);
        m.set(n, 0);
        let cur = m.word(g.current);
        add(m, cur + 0x880, 1);
        m.set(n + 8, 0);
        m.set(n + 0xc, (-f32::MAX).to_bits());
        m.set(n + 0x10, g.node_class);
    }
    n
}

/// A node with no children reallocated to 36 bytes, its four children (none yet) after it.
fn grow(m: &mut impl Space, n: u32) -> u32 {
    let n = m.realloc(n, 0x24, Site::Grow);
    m.set(n, n.wrapping_add(0x14));
    for k in 0..4 {
        m.set(n.wrapping_add(0x14 + 4 * k), 0);
    }
    n
}

/// `entry` in node `n`'s first empty slot, else in a new chunk (the current tree's pool's) at the end
/// of its chain.
fn file(m: &mut impl Space, g: &Globals, n: u32, entry: u32) {
    let (mut prev, mut c) = (0, m.word(n + 4));
    while c != 0 {
        for k in 0..4 {
            if m.word(c + 4 + 4 * k) == 0 {
                m.set(c + 4 + 4 * k, entry);
                return;
            }
        }
        prev = c;
        c = m.word(c);
    }
    let cur = m.word(g.current);
    let pool = m.word(cur + 0x90);
    let l = take(m, pool);
    for k in 0..5 {
        m.set(l.wrapping_add(4 * k), 0);
    }
    m.set(l.wrapping_add(4), entry);
    if m.word(n + 4) == 0 {
        m.set(n + 4, l);
    } else {
        m.set(prev, l);
    }
}

/// vt `+0x24`: `entry` filed under `key` (nothing for 0). The open query is closed, the put counted
/// and the tree made current, the entry's first halfword cleared; under each corner the key names,
/// the root and every node down to the key's level widened to the box's height, a node with no
/// children grown them and a missing child made on the way, and the entry filed in the last.
fn tree_put(m: &mut impl Space, g: &Globals, t: u32, entry: u32, key: u32, centre: Vf, half: Vf) {
    let (lo, hi) = height(centre, half);
    if key == 0 {
        return;
    }
    close(m, t);
    add(m, t + 0x888, 1);
    m.set(g.current, t);
    let w = m.word(entry);
    m.set(entry, w & 0xffff_0000);
    let (level, cells) = corners(m, t, key);
    for (x, z) in cells {
        let mut node = m.word(t + 0x8c);
        let (mut parent, mut was) = (0, 0);
        widen(m, node, lo, hi);
        let mut bit = 0x4000;
        for _ in 0..=level {
            let i = child_of(x, z, bit);
            let kids = m.word(node);
            let mut child = 0;
            if kids == 0 {
                if node == m.word(t + 0x8c) {
                    node = grow(m, node);
                    m.set(t + 0x8c, node);
                } else {
                    node = grow(m, node);
                }
                if parent != 0 {
                    let at = m.word(parent).wrapping_add(4 * was);
                    m.set(at, node);
                }
            } else {
                child = m.word(kids.wrapping_add(4 * i));
            }
            if child == 0 {
                child = make(m, g);
                let at = m.word(node).wrapping_add(4 * i);
                m.set(at, child);
            }
            widen(m, child, lo, hi);
            (parent, was, node) = (node, i, child);
            bit >>= 1;
        }
        file(m, g, node, entry);
    }
}

/// vt `+0x28`: the entry still under `key`, its box now `centre` ± `half`: the nodes down to each
/// corner (while they have children) widened to its height.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
fn tree_moved(m: &mut impl Ram, t: u32, key: u32, centre: Vf, half: Vf) {
    let (lo, hi) = height(centre, half);
    if key == 0 || !(lo <= hi) {
        return;
    }
    let (level, cells) = corners(m, t, key);
    for (x, z) in cells {
        let mut node = m.word(t + 0x8c);
        widen(m, node, lo, hi);
        let mut bit = 0x4000;
        for _ in 0..=level {
            let kids = m.word(node);
            if kids == 0 {
                break;
            }
            node = m.word(kids.wrapping_add(4 * child_of(x, z, bit)));
            widen(m, node, lo, hi);
            bit >>= 1;
        }
    }
}

/// `entry` cleared from the first of node `n`'s chunks that holds it (every slot of that chunk that
/// does), and each chunk met that this leaves empty unlinked and freed to the current tree's pool.
/// The walk goes on from a freed chunk's first word, which the free has made the pool's free chain.
/// Whether the node still holds anything: a chunk, or a child.
fn unfile(m: &mut impl Ram, g: &Globals, n: u32, entry: u32) -> bool {
    let mut c = m.word(n + 4);
    let mut prev = 0;
    while c != 0 {
        let (mut empty, mut found) = (true, false);
        for k in 0..4 {
            let w = m.word(c + 4 + 4 * k);
            if w == entry {
                m.set(c + 4 + 4 * k, 0);
                found = true;
            } else if w != 0 {
                empty = false;
            }
        }
        if empty {
            let next = m.word(c);
            if m.word(n + 4) == c {
                m.set(n + 4, next);
            } else {
                m.set(prev, next);
            }
            let cur = m.word(g.current);
            let pool = m.word(cur + 0x90);
            give(m, pool, c);
        }
        if found {
            break;
        }
        prev = c;
        c = m.word(c);
    }
    if m.word(n + 4) != 0 {
        return true;
    }
    let kids = m.word(n);
    kids != 0 && (0..4).any(|k| m.word(kids.wrapping_add(4 * k)) != 0)
}

/// A node's deleting destructor (its vt `+8` with 1): its children deleted, its chunks freed to the
/// current tree's pool unless the tree keeps them (`+0x98` bit 0), the count down, the node freed.
fn delete(m: &mut impl Space, g: &Globals, n: u32) {
    if n == 0 {
        return;
    }
    m.set(n + 0x10, g.node_class);
    m.set(n + 0x10, g.node_base);
    let first = m.word(n + 4);
    if m.word(n) != 0 {
        for k in 0..4 {
            let kids = m.word(n);
            let c = m.word(kids.wrapping_add(4 * k));
            delete(m, g, c);
        }
    }
    let cur = m.word(g.current);
    if m.word(cur + 0x98) & 1 == 0 {
        let mut c = first;
        while c != 0 {
            let next = m.word(c);
            let cur = m.word(g.current);
            let pool = m.word(cur + 0x90);
            give(m, pool, c);
            c = next;
        }
    }
    let cur = m.word(g.current);
    add(m, cur + 0x880, u32::MAX);
    m.free(n);
}

/// vt `+0x2c`: `entry` taken out from under `key` (nothing for 0). The open query is closed, the
/// take-out counted and the tree made current; under each corner whose nodes down to the key's level
/// are all there, the entry unfiled from the last, which is deleted if that leaves it nothing and its
/// parent's child cleared.
fn tree_out(m: &mut impl Space, g: &Globals, t: u32, entry: u32, key: u32) {
    if key == 0 {
        return;
    }
    close(m, t);
    add(m, t + 0x888, u32::MAX);
    m.set(g.current, t);
    let (level, cells) = corners(m, t, key);
    'corner: for (x, z) in cells {
        let (mut node, mut parent, mut i) = (m.word(t + 0x8c), 0, 0);
        let mut bit = 0x4000;
        for _ in 0..=level {
            i = child_of(x, z, bit);
            let kids = m.word(node);
            parent = node;
            if kids == 0 {
                continue 'corner;
            }
            node = m.word(kids.wrapping_add(4 * i));
            if node == 0 {
                continue 'corner;
            }
            bit >>= 1;
        }
        if !unfile(m, g, node, entry) {
            delete(m, g, node);
            if parent != 0 {
                let at = m.word(parent).wrapping_add(4 * i);
                m.set(at, 0);
            }
        }
    }
}

// --- a sub-database's calls, by class -------------------------------------------------------------------

fn other(sub: u32) -> ! {
    panic!("an entry in sub-database {sub:#x}, of a class not ported")
}

/// vt `+0x20`: the place for the box.
fn pick(m: &mut impl Space, s: u32, centre: Vf, half: Vf, current: u32) -> u32 {
    match m.class(s) {
        Class::List => list_pick(m, s, current),
        Class::Tree => tree_view(m, s).pick(centre, half),
        Class::Other => other(s),
    }
}

/// vt `+0x24`.
fn put(m: &mut impl Space, g: &Globals, s: u32, entry: u32, at: u32, centre: Vf, half: Vf) {
    match m.class(s) {
        Class::List => list_put(m, s, entry, at),
        Class::Tree => tree_put(m, g, s, entry, at, centre, half),
        Class::Other => other(s),
    }
}

/// vt `+0x28` (a list's does nothing).
fn shifted(m: &mut impl Space, s: u32, at: u32, centre: Vf, half: Vf) {
    match m.class(s) {
        Class::List => {}
        Class::Tree => tree_moved(m, s, at, centre, half),
        Class::Other => other(s),
    }
}

/// vt `+0x2c`.
fn out(m: &mut impl Space, g: &Globals, s: u32, entry: u32, at: u32) {
    match m.class(s) {
        Class::List => list_out(m, s, at),
        Class::Tree => tree_out(m, g, s, entry, at),
        Class::Other => other(s),
    }
}

/// vt `+0x20`, then `+0x28` if the place is the one at `slot`, else `+0x2c` and `+0x24` and the
/// place kept at `slot`.
fn shift(m: &mut impl Space, g: &Globals, s: u32, entry: u32, slot: u32, centre: Vf, half: Vf) {
    let was = m.word(slot);
    let to = pick(m, s, centre, half, was);
    if to == m.word(slot) {
        shifted(m, s, to, centre, half);
    } else {
        let was = m.word(slot);
        out(m, g, s, entry, was);
        put(m, g, s, entry, to, centre, half);
        m.set(slot, to);
    }
}

/// A cell's sub-database for `kind`: bit 0 the first, else bit 1 the second, else none.
fn sub_of(m: &mut impl Ram, cell: u32, kind: u32) -> u32 {
    if kind & 1 != 0 {
        m.word(cell + 0x28)
    } else if kind & 2 != 0 {
        m.word(cell + 0x2c)
    } else {
        0
    }
}

fn portal(m: &mut impl Ram, p: u32) -> Portal {
    Portal {
        corners: [0, 1, 2, 3].map(|i| v(m, p + 0x30 + 0x10 * i)),
        normal: v(m, p + 0x70),
        plane: word_f(m, p + 0xc4),
        edges: [0, 1, 2, 3].map(|i| v(m, p + 0x80 + 0x10 * i)),
    }
}

/// The cells beyond `cell`'s links (never `home`, nor one gathered already) whose portal the sphere
/// touches, each followed by those beyond it.
fn reach(m: &mut impl Ram, home: u32, cell: u32, centre: Vf, r: F, out: &mut Vec<u32>) {
    let mut l = m.word(cell + 0x24);
    while l != 0 {
        let to = m.word(l + 4);
        if to != home && !out.contains(&to) {
            let p = m.word(l + 8);
            if atv_spatial::touches(&portal(m, p), centre, r) {
                out.push(to);
                reach(m, home, to, centre, r, out);
            }
        }
        l = m.word(l);
    }
}

/// The extra cells brought up to date with the cells the sphere reaches: an extra still reached moved
/// in place, one no longer reached left (its link back to the pool), each newly reached one entered
/// with a link from the pool, newest first. The kind a new cell's sub-database is picked by is the
/// last one touched's; a cell whose sub-database has no place for the box is not entered.
fn overlaps(m: &mut impl Space, g: &Globals, rec: u32, entry: u32, centre: Vf, half: Vf, r: F) {
    let home = m.word(rec + 4);
    let mut cells = Vec::new();
    reach(m, home, home, centre, r, &mut cells);
    let mut carried = m.word(rec + 8);
    let (mut prev, mut e) = (rec, m.word(rec));
    while e != 0 {
        let cell = m.word(e + 4);
        if let Some(k) = cells.iter().position(|&c| c == cell) {
            carried = m.word(e + 8);
            shift(m, g, carried, entry, e + 0xc, centre, half);
            cells[k] = 0;
            prev = e;
            e = m.word(e);
        } else {
            let (s, at) = (m.word(e + 8), m.word(e + 0xc));
            out(m, g, s, entry, at);
            let next = m.word(e);
            m.set(prev, next);
            let pool = m.word(g.links);
            give(m, pool, e);
            e = m.word(prev);
        }
    }
    for cell in cells {
        if cell == 0 {
            continue;
        }
        let kind = m.word(carried + 8);
        let s = sub_of(m, cell, kind);
        carried = s;
        let slot = pick(m, s, centre, half, 0);
        if slot == 0 {
            continue;
        }
        put(m, g, s, entry, slot, centre, half);
        let pool = m.word(g.links);
        let l = take(m, pool);
        m.set(l.wrapping_add(0xc), slot);
        m.set(l.wrapping_add(4), cell);
        m.set(l.wrapping_add(8), s);
        let head = m.word(rec);
        m.set(l, head);
        m.set(rec, l);
    }
}

/// **The spatial move**: `entry`'s record (`+8`) moved to the box at `centre`, `half` either way,
/// with the sphere reaching `reach` for the overlaps; nothing if the record is in no cell.
pub fn spatial_move(m: &mut impl Space, g: &Globals, entry: u32, centre: Vf, half: Vf, reach: F) {
    let rec = m.word(entry + 8);
    let home = m.word(rec + 4);
    if home == 0 {
        return;
    }
    let (mut cell, mut from) = (home, 0);
    let mut l = m.word(cell + 0x24);
    while l != 0 {
        let to = m.word(l + 4);
        if to != from {
            let p = m.word(l + 8);
            let (gate, last) = (portal(m, p), v(m, rec + 0x10));
            if atv_spatial::crosses(&gate, last, centre) {
                (from, cell) = (cell, to);
                l = m.word(cell + 0x24);
                continue;
            }
        }
        l = m.word(l);
    }
    if cell == m.word(rec + 4) {
        let s = m.word(rec + 8);
        shift(m, g, s, entry, rec + 0xc, centre, half);
    } else {
        let old = m.word(rec + 8);
        let at = m.word(rec + 0xc);
        out(m, g, old, entry, at);
        let mut e = m.word(rec);
        while e != 0 {
            let (s, at) = (m.word(e + 8), m.word(e + 0xc));
            out(m, g, s, entry, at);
            let next = m.word(e);
            let pool = m.word(g.links);
            give(m, pool, e);
            e = next;
        }
        m.set(rec, 0);
        m.set(rec + 4, cell);
        let kind = m.word(old + 8);
        let s = sub_of(m, cell, kind);
        m.set(rec + 8, s);
        let at = pick(m, s, centre, half, 0);
        m.set(rec + 0xc, at);
        put(m, g, s, entry, at, centre, half);
    }
    overlaps(m, g, rec, entry, centre, half, reach);
    set_v(m, rec + 0x10, centre);
}

// --- a collidable put in a world (F31b5o) --------------------------------------------------------------

/// The link pool's class, which [`insert`] gives a pool it has to make (the vtable at its `+4`).
#[derive(Clone, Copy, Debug)]
pub struct PoolClass(pub u32);

/// A record taken up (`atv_record_new`): the link pool made first if there is none (a pool of
/// 16-byte links, 4096 bytes a chunk), the record's four words cleared, and the records counted
/// (the count kept two words after the pool's pointer).
fn record_new(m: &mut impl Space, g: &Globals, class: PoolClass, rec: u32) {
    if m.word(g.links) == 0 {
        let p = m.alloc(0x3c, Site::Pool);
        if p != 0 {
            let (stride, bytes) = (0x10u32, 0x1000u32);
            m.set(p, 1);
            m.set(p + 4, class.0);
            m.set(p + 0x30, 0);
            m.set(p + 0x34, 0);
            m.set(p + 8, ((bytes - stride) as i32 / stride as i32) as u32);
            m.set(p + 0xc, 0);
            m.set(p + 0x10, 0);
            let n = m.word(p + 8);
            m.set(p + 0x18, n);
            m.set(p + 0x1c, stride);
            m.set(p + 0x20, bytes);
            m.set(p + 0x24, 0);
            m.set(p + 0x14, 0);
            m.set(p + 0x2c, 0);
            m.set(p + 0x38, 4);
        }
        m.set(g.links, p);
    }
    for k in 0..4 {
        m.set(rec + 4 * k, 0);
    }
    add(m, g.links + 8, 1);
}

/// A record let go of (`atv_record_drop`, never freeing the record itself): the count down, and the
/// link pool deleted with the last.
fn record_drop(m: &mut impl Space, g: &Globals, rec: u32) {
    if rec == 0 {
        return;
    }
    add(m, g.links + 8, u32::MAX);
    if m.word(g.links + 8) == 0 {
        let p = m.word(g.links);
        if p != 0 {
            m.delete_pool(p);
            m.set(g.links, 0);
        }
    }
}

/// **A collidable's entry put in a world** (`atv_spatial_insert`): `entry` (a collidable's `+0x18`)
/// filed in cell `cell` under `kind` with its box at `centre`, `half` either way, and its sphere
/// reaching `reach` for the overlaps. The call takes up a record of its own at `temp` (the
/// original's is on its stack) and lets it go at the end; unless `keep_temp`, the entry gets a new
/// record off the heap (its `+8`) and is filed with that, else it is filed with the call's own.
/// The filing: the cell's sub-database for the kind, its place for the box ([`pick`], none held)
/// and the entry put there ([`put`]); the record's cell, sub-database and place, no extras, the
/// centre (all four lanes); then the overlaps ([`overlaps`]).
#[allow(clippy::too_many_arguments)]
pub fn insert(m: &mut impl Space, g: &Globals, class: PoolClass, entry: u32, cell: u32, kind: u32, centre: Vf, half: Vf, reach: F, temp: u32, keep_temp: bool) {
    record_new(m, g, class, temp);
    let rec = if keep_temp {
        temp
    } else {
        let r = m.alloc(0x20, Site::Record);
        if r != 0 {
            record_new(m, g, class, r);
        }
        m.set(entry + 8, r);
        r
    };
    let s = sub_of(m, cell, kind);
    let at = pick(m, s, centre, half, 0);
    put(m, g, s, entry, at, centre, half);
    m.set(rec + 4, cell);
    m.set(rec + 8, s);
    m.set(rec + 0xc, at);
    m.set(rec, 0);
    set_v(m, rec + 0x10, centre);
    overlaps(m, g, rec, entry, centre, half, reach);
    record_drop(m, g, temp);
}

/// **A collidable put in a world** (`atv_part_in_world`, with its last argument 1): its box in the
/// world (its class's vt `+0xc4`, handed in as `centre` and `half`), the reach the half's length
/// (`mula`, `madda`, `madd`, `sqrt.s`), and its entry (`+0x18`) [`insert`]ed in `cell` under
/// `kind`, filed with the call's own record when its flag byte (`+0x126`) has bit 3.
#[allow(clippy::too_many_arguments)]
pub fn part_in_world(m: &mut impl Space, g: &Globals, class: PoolClass, part: u32, cell: u32, kind: u32, centre: Vf, half: Vf, temp: u32) {
    let [x, y, z] = [half.x(), half.y(), half.z()];
    let reach = F(ps2_float::fpu::sqrt(ps2_float::fpu::madd(ps2_float::fpu::madda(ps2_float::fpu::mula(x, x), y, y), z, z)));
    let flags = m.word(part + 0x124) >> 16 & 0xff;
    insert(m, g, class, part + 0x18, cell, kind, centre, half, reach, temp, flags & 8 != 0);
}
