//! **SLUS-21104's heap** (F31b5r): the allocator behind the game's `new`, `delete` and `realloc`,
//! so a frame that allocates needs no interpreter. Where the original keeps it: research/oracle.txt
//! `# F31b5r`.
//!
//! A heap manager holds up to eight heaps; the current one (the manager's id word) serves new
//! blocks, and a block goes back to the heap whose id its header carries. A heap is a range of
//! blocks, each a 16-byte header then its bytes (sizes are multiples of 16): the header's `+2` is the
//! heap's id (a short), `+8` the size of the block before it (0 for the first), `+0xc` its own size
//! with the top bit set while it is free. The heap keeps its free blocks in a binary tree by size
//! ([`ROOT`]; the free block's `+0x10` and `+0x14` the smaller and larger sides, `+0x18` its parent),
//! except free blocks of exactly 16, 32 or 48 bytes, which go on a list each ([`SMALL`]; `+0x10`
//! and `+0x14` the list's previous and next).
//!
//! - [`alloc`] takes the exact size's list head if there is one, else the smallest tree block at
//!   least as big (or one of the size, met on the way down). A bigger block is split only if one 32
//!   bytes over the request is found, which is then split instead; otherwise the whole block is used.
//! - [`free`] joins the block with a free neighbour on either side and files the whole — except into
//!   a heap whose tree is empty, where it is filed alone.
//! - [`realloc`] grows in place into a free block after it when that holds the growth (splitting off
//!   the rest when over 32 bytes remain, else taking all of it); any other change of size, shrinking
//!   too, is a new block, the smaller of the two sizes copied over, and the old block freed.
//!
//! Each of the original's calls takes the heap's lock (a thread-owner count, back where it was when
//! the call returns) and reports to a statistics hook that does nothing; neither changes RAM, and
//! neither is here.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

pub use super::node::Ram;

/// The manager's: whether it is made, the current heap's id, and its first heap (the rest follow,
/// [`HEAP_BYTES`] apart).
pub const MADE: u32 = 0x4;
pub const CURRENT: u32 = 0x18;
pub const HEAPS: u32 = 0x1c;
pub const HEAP_BYTES: u32 = 0x4c;
const SLOTS: u32 = 8;

/// A heap's: its free tree's root, the small sizes' list heads (one word each, from [`LISTS`]), its
/// id (a short) and its lock object.
pub const ROOT: u32 = 0x1c;
pub const LISTS: u32 = 0x20;
pub const ID: u32 = 0x2c;
pub const LOCK: u32 = 0x48;

/// The free block sizes kept on lists rather than in the tree (a table in the original's data).
pub const SMALL: [u32; 3] = [0x10, 0x20, 0x30];

/// A block's header (16 bytes before the block's address).
pub const HEADER: u32 = 0x10;
const TAG: u32 = 0x4;
const PREV_SIZE: u32 = 0x8;
const SIZE: u32 = 0xc;
const LEFT: u32 = 0x10;
const RIGHT: u32 = 0x14;
const PARENT: u32 = 0x18;
const FREE: u32 = 0x8000_0000;

fn size_of(w: u32) -> u32 {
    w & !FREE
}

fn small(size: u32) -> Option<u32> {
    SMALL.iter().position(|&s| s == size).map(|k| k as u32)
}

/// The short at `at` (2-aligned), sign-extended.
fn short(m: &mut impl Ram, at: u32) -> i32 {
    let w = m.word(at & !3);
    i32::from((w >> (8 * (at & 2))) as u16 as i16)
}

fn set_short(m: &mut impl Ram, at: u32, v: i32) {
    let (a, s) = (at & !3, 8 * (at & 2));
    let w = m.word(a);
    m.set(a, w & !(0xffff << s) | (v as u32 & 0xffff) << s);
}

/// A request rounded up to the heap's 16-byte grain (0 asks for 16), as the original's signed
/// remainder does it.
pub fn rounded(bytes: u32) -> u32 {
    if bytes == 0 {
        return 0x10;
    }
    let mut r = bytes & 0xf;
    if (bytes as i32) < 0 && r != 0 {
        r = r.wrapping_sub(0x10);
    }
    if r != 0 { bytes.wrapping_add(0x10u32.wrapping_sub(r)) } else { bytes }
}

/// **The current heap** of the manager at `mgr`: the slot whose id is the manager's current id, or
/// 0 if none is.
pub fn current(m: &mut impl Ram, mgr: u32) -> u32 {
    let id = m.word(mgr + CURRENT) as i32;
    heap_with(m, mgr, id)
}

/// **The heap a block belongs to**: the slot whose id its header carries, or 0 if none is.
pub fn of_block(m: &mut impl Ram, mgr: u32, block: u32) -> u32 {
    let id = short(m, block.wrapping_sub(HEADER) + 2);
    heap_with(m, mgr, id)
}

fn heap_with(m: &mut impl Ram, mgr: u32, id: i32) -> u32 {
    (0..SLOTS).map(|k| mgr + HEAPS + k * HEAP_BYTES).find(|&h| short(m, h + ID) == id).unwrap_or(0)
}

/// The free block of a size (its list's head), else the tree's smallest at least as big below `node`
/// (one of the size met on the way down ends the search).
fn fit(m: &mut impl Ram, heap: u32, mut node: u32, size: u32) -> u32 {
    if let Some(k) = small(size) {
        let head = m.word(heap + LISTS + 4 * k);
        if head != 0 {
            return head;
        }
    }
    let mut best = 0;
    while node != 0 {
        let s = size_of(m.word(node + SIZE));
        if s == size {
            return node;
        }
        if s < size {
            node = m.word(node + RIGHT);
        } else {
            best = node;
            node = m.word(node + LEFT);
        }
    }
    best
}

/// Marks the block at `b` (its header) free with `size`, the block after's back size too, links
/// cleared.
fn mark_free(m: &mut impl Ram, b: u32, size: u32, parent: u32) {
    m.set(b + TAG, 0);
    let w = m.word(b + SIZE);
    m.set(b + SIZE, w & FREE | size);
    let w = m.word(b + SIZE);
    m.set(b + SIZE, w | FREE);
    m.set(b + PARENT, parent);
    m.set(b + LEFT, 0);
    m.set(b + RIGHT, 0);
    m.set(b.wrapping_add(size) + HEADER + PREV_SIZE, size);
}

/// Files the free block at `b` (its header) of `size`: at the front of its size's list, or as a leaf
/// of the tree (smaller to the left, the same or larger to the right).
fn insert(m: &mut impl Ram, heap: u32, b: u32, size: u32) {
    if let Some(k) = small(size) {
        let slot = heap + LISTS + 4 * k;
        let head = m.word(slot);
        mark_free(m, b, SMALL[k as usize], 0);
        m.set(b + LEFT, 0);
        if head == 0 {
            m.set(slot, b);
            m.set(b + RIGHT, 0);
        } else {
            m.set(b + RIGHT, head);
            let next = m.word(b + RIGHT);
            m.set(next + LEFT, b);
            m.set(slot, b);
        }
        return;
    }
    let mut node = m.word(heap + ROOT);
    if node == 0 {
        mark_free(m, b, size, 0);
        m.set(heap + ROOT, b);
        return;
    }
    let link = loop {
        let side = if size < size_of(m.word(node + SIZE)) { LEFT } else { RIGHT };
        let next = m.word(node + side);
        if next == 0 {
            break node + side;
        }
        node = next;
    };
    mark_free(m, b, size, node);
    m.set(link, b);
}

/// Takes the free block at `b` (its header) of `size` out of its list or the tree. A tree node with
/// both sides is replaced by its larger side, its smaller side hung under the larger's smallest.
fn unlink(m: &mut impl Ram, heap: u32, b: u32, size: u32) {
    if let Some(k) = small(size) {
        let slot = heap + LISTS + 4 * k;
        if m.word(slot) == b {
            let next = m.word(b + RIGHT);
            m.set(slot, next);
            let next = m.word(b + RIGHT);
            if next != 0 {
                m.set(next + LEFT, 0);
            }
        } else {
            let (prev, next) = (m.word(b + LEFT), m.word(b + RIGHT));
            m.set(prev + RIGHT, next);
            if next != 0 {
                m.set(next + LEFT, prev);
            }
        }
        return;
    }
    let left = m.word(b + LEFT);
    let right = m.word(b + RIGHT);
    // The child that takes the block's place (both sides: the larger, the smaller moved under it).
    let child_at = match (left != 0, right != 0) {
        (false, false) => {
            let p = m.word(b + PARENT);
            if p != 0 {
                if m.word(p + LEFT) == b {
                    m.set(p + LEFT, 0);
                }
                let p = m.word(b + PARENT);
                if m.word(p + RIGHT) == b {
                    m.set(p + RIGHT, 0);
                }
            }
            if m.word(heap + ROOT) == b {
                m.set(heap + ROOT, 0);
            }
            return;
        }
        (false, true) => {
            let p = m.word(b + PARENT);
            if p != 0 {
                if m.word(p + LEFT) == b {
                    m.set(p + LEFT, right);
                }
                let p = m.word(b + PARENT);
                if m.word(p + RIGHT) == b {
                    let r = m.word(b + RIGHT);
                    m.set(p + RIGHT, r);
                }
                let (p, r) = (m.word(b + PARENT), m.word(b + RIGHT));
                m.set(r + PARENT, p);
            }
            RIGHT
        }
        (true, false) => {
            let p = m.word(b + PARENT);
            if p != 0 {
                if m.word(p + LEFT) == b {
                    m.set(p + LEFT, left);
                }
                let p = m.word(b + PARENT);
                if m.word(p + RIGHT) == b {
                    let l = m.word(b + LEFT);
                    m.set(p + RIGHT, l);
                }
                let (p, l) = (m.word(b + PARENT), m.word(b + LEFT));
                m.set(l + PARENT, p);
            }
            LEFT
        }
        (true, true) => {
            let mut least = right;
            loop {
                let next = m.word(least + LEFT);
                if next == 0 {
                    break;
                }
                least = next;
            }
            m.set(least + LEFT, left);
            let l = m.word(b + LEFT);
            m.set(l + PARENT, least);
            let p = m.word(b + PARENT);
            if p != 0 {
                if m.word(p + LEFT) == b {
                    let r = m.word(b + RIGHT);
                    m.set(p + LEFT, r);
                } else if m.word(p + RIGHT) == b {
                    let r = m.word(b + RIGHT);
                    m.set(p + RIGHT, r);
                }
                let r = m.word(b + RIGHT);
                if r != 0 {
                    let p = m.word(b + PARENT);
                    m.set(r + PARENT, p);
                }
            }
            RIGHT
        }
    };
    if m.word(heap + ROOT) == b {
        let c = m.word(b + child_at);
        m.set(heap + ROOT, c);
        let root = m.word(heap + ROOT);
        if root != 0 {
            m.set(root + PARENT, 0);
        }
    }
}

/// Marks the block at `b` (its header) taken, `size` bytes, from `heap`.
fn mark_taken(m: &mut impl Ram, heap: u32, b: u32, size: u32) {
    let id = short(m, heap + ID);
    let w = size_of(m.word(b + SIZE));
    m.set(b + SIZE, w);
    let w = m.word(b + SIZE);
    m.set(b + SIZE, w & FREE | size);
    set_short(m, b + 2, id);
    m.set(b.wrapping_add(size) + HEADER + PREV_SIZE, size);
}

/// **A block of `bytes`** (rounded, [`rounded`]) from `heap`: its address, or 0 with nothing free
/// that fits.
pub fn alloc(m: &mut impl Ram, heap: u32, bytes: u32) -> u32 {
    let size = rounded(bytes);
    let root = m.word(heap + ROOT);
    let mut b = fit(m, heap, root, size);
    if b == 0 {
        return 0;
    }
    let mut whole = size_of(m.word(b + SIZE));
    let mut rest = None;
    if (size as i32) < (whole as i32) {
        let root = m.word(heap + ROOT);
        b = fit(m, heap, root, size.wrapping_add(0x20));
        if b == 0 {
            return 0;
        }
        whole = size_of(m.word(b + SIZE));
        rest = Some((b.wrapping_add(size) + HEADER, whole.wrapping_sub(size).wrapping_sub(HEADER)));
    }
    unlink(m, heap, b, whole);
    match rest {
        Some((at, n)) => {
            insert(m, heap, at, n);
            mark_taken(m, heap, b, size);
        }
        None => mark_taken(m, heap, b, whole),
    }
    b + HEADER
}

/// **The block at `block` back to `heap`**, joined with a free block before or after it (none if
/// the heap's tree is empty). Nothing for a null block.
pub fn free(m: &mut impl Ram, heap: u32, block: u32) {
    if block == 0 {
        return;
    }
    let mut b = block.wrapping_sub(HEADER);
    let mut size = size_of(m.word(b + SIZE));
    if m.word(heap + ROOT) == 0 {
        insert(m, heap, b, size);
        return;
    }
    let back = m.word(b + PREV_SIZE);
    let prev = if (back as i32) > 0 { b.wrapping_sub(back.wrapping_add(HEADER)) } else { 0 };
    let next = b.wrapping_add(size) + HEADER;
    if prev != 0 {
        let w = m.word(prev + SIZE);
        if w & FREE != 0 {
            size = size.wrapping_add(back).wrapping_add(HEADER);
            unlink(m, heap, prev, size_of(w));
            b = prev;
        }
    }
    if next != 0 {
        let w = m.word(next + SIZE);
        if w & FREE != 0 {
            size = size.wrapping_add(size_of(w)).wrapping_add(HEADER);
            unlink(m, heap, next, size_of(w));
        }
    }
    m.set(b + PARENT, 0);
    m.set(b + LEFT, 0);
    m.set(b + RIGHT, 0);
    insert(m, heap, b, size);
}

/// **The block at `block` resized to `bytes`** (rounded) in `heap`: grown in place into a free block
/// after it that holds the growth, else moved to a new block (the smaller size copied, the old
/// freed). A null block is a new block; the same rounded size changes nothing. Answers the block's
/// address.
pub fn realloc(m: &mut impl Ram, heap: u32, block: u32, bytes: u32) -> u32 {
    if block == 0 {
        return alloc(m, heap, bytes);
    }
    let mut size = rounded(bytes);
    let b = block.wrapping_sub(HEADER);
    let old = size_of(m.word(b + SIZE));
    if size == old {
        return block;
    }
    let next = b.wrapping_add(old) + HEADER;
    let w = m.word(next + SIZE);
    let spare = size_of(w);
    let grow = size.wrapping_sub(old);
    if (old as i32) < (size as i32) && w & FREE != 0 && (spare as i32) >= (grow as i32) {
        let left = spare.wrapping_sub(grow);
        let split = if (left as i32) < 33 {
            size = old.wrapping_add(spare).wrapping_add(HEADER);
            0
        } else {
            b.wrapping_add(size) + HEADER
        };
        unlink(m, heap, next, spare);
        mark_taken(m, heap, b, size);
        if split != 0 {
            insert(m, heap, split, left);
        }
        return block;
    }
    let to = alloc(m, heap, size);
    let n = if (old as i32) < (size as i32) { old } else { size };
    copy(m, to, block, n);
    free(m, heap, block);
    to
}

/// The C library's forward copy of `n` bytes.
fn copy(m: &mut impl Ram, to: u32, from: u32, n: u32) {
    let words = if (to | from) & 3 == 0 { n / 4 } else { 0 };
    for k in 0..words {
        let w = m.word(from + 4 * k);
        m.set(to + 4 * k, w);
    }
    for k in 4 * words..n {
        let (s, d) = (from + k, to + k);
        let byte = m.word(s & !3) >> (8 * (s & 3)) & 0xff;
        let w = m.word(d & !3);
        m.set(d & !3, w & !(0xff << (8 * (d & 3))) | byte << (8 * (d & 3)));
    }
}

/// **`new`** (the game's allocation with a file and line, which only a debug build kept): a block of
/// `bytes` from the manager's current heap.
pub fn new(m: &mut impl Ram, mgr: u32, bytes: u32) -> u32 {
    let heap = current(m, mgr);
    alloc(m, heap, bytes)
}

/// **`delete`**: the block back to the heap its header names. Nothing for a null block.
pub fn delete(m: &mut impl Ram, mgr: u32, block: u32) {
    if block != 0 {
        let heap = of_block(m, mgr, block);
        free(m, heap, block);
    }
}

/// **`realloc`**: [`realloc`] in the heap the block's header names, or a new block from the current
/// heap for a null one.
pub fn resize(m: &mut impl Ram, mgr: u32, block: u32, bytes: u32) -> u32 {
    let heap = if block == 0 { current(m, mgr) } else { of_block(m, mgr, block) };
    realloc(m, heap, block, bytes)
}
