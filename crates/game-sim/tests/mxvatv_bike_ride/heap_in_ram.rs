//! **MD140 — the game's heap's resize in Rust**, on the heap's words in RAM. The heap's manager (the
//! pointer at [`MANAGER_AT`]) holds eight pools `0x4c` bytes apart; a pool's words: `+0x1c` the
//! root of its free tree (by size), `+0x20..+0x28` the heads of three lists of free blocks of the
//! three small sizes ([`SMALL_SIZES_AT`]), `+0x2c` (a halfword) its id, `+0x48` its lock.
//! A block's 16-byte header: `+2` its pool's id, `+4` 0 while free, `+8` the size of the block
//! before it, `+0xc` its size (bit 31 while free); a free block's data `+0x10`, `+0x14`, `+0x18` are
//! its tree's left, right and parent (a small list's previous, next). The lock's calls and the
//! statistics calls (empty) are left out: they write nothing outside the stack.
//! Shared by `mxvatv_bike_ride.rs` and `atv_quad/record_outside.rs`, whose pair logs grow through it.
#![allow(dead_code)]

use ps2_oracle::Ee;

/// The pointer to the heap's manager, and the three small sizes that have lists of their own.
pub const MANAGER_AT: u32 = 0x0038_f750;
pub const SMALL_SIZES_AT: u32 = 0x0039_0638;
const FREE: u32 = 0x8000_0000;
/// A pool's free tree's root, its small lists' heads, its id.
const ROOT: u32 = 0x1c;
const LISTS: u32 = 0x20;
const ID: u32 = 0x2c;
const SIZE: u32 = 0x7fff_ffff;

fn size(ee: &Ee, b: u32) -> u32 {
    ee.u32_at(b + 0xc) & SIZE
}
fn set_u16(ee: &mut Ee, a: u32, v: u16) {
    ee.write(a, 2, u128::from(v));
}
/// Which small list `size` has (none for any other size).
fn small(ee: &Ee, size: u32) -> Option<u32> {
    (0..3).find(|&i| ee.u32_at(SMALL_SIZES_AT + 4 * i) == size)
}
fn round(bytes: u32) -> u32 {
    // the original's signed rounding up to 16; a size is never negative here
    let b = bytes as i32;
    if b == 0 {
        return 0x10;
    }
    let mut r = b & 0xf;
    if b < 0 && r != 0 {
        r -= 0x10;
    }
    (if r != 0 { b + 0x10 - r } else { b }) as u32
}

/// The pool a resize of `block` (or a new block, 0) goes to: the block's own, by the id in its
/// header; for a new one the pool whose id is the manager's current one.
fn pool_of(ee: &Ee, block: u32) -> u32 {
    let m = ee.u32_at(MANAGER_AT);
    assert_ne!(ee.u32_at(m + 4), 0, "the heap is not set up: its setup is not Rust's");
    let p = (0..8).map(|i| m + 0x1c + 0x4c * i).find(|&q| {
        let id = ee.u16_at(q + ID) as i16;
        if block == 0 { ee.i32_at(q - 4) == i32::from(id) } else { id == ee.u16_at(block - 0xe) as i16 }
    });
    p.expect("no pool has the block's id")
}

/// The best fitting free block of `size` under `node`: the small list's head for a small size
/// when it has one, else the tree's exact fit, else its smallest bigger block (0 for none).
fn find(ee: &Ee, p: u32, node: u32, size: u32) -> u32 {
    if let Some(i) = small(ee, size) {
        let head = ee.u32_at(p + LISTS + 4 * i);
        if head != 0 {
            return head;
        }
    }
    if node == 0 {
        return 0;
    }
    let n = self::size(ee, node);
    if n == size {
        node
    } else if n < size {
        find(ee, p, ee.u32_at(node + 0x14), size)
    } else {
        match find(ee, p, ee.u32_at(node + 0x10), size) {
            0 => node,
            b => b,
        }
    }
}

/// The free block `b` (of `size`, which names its list) taken off its small list or the tree.
fn unlink(ee: &mut Ee, p: u32, b: u32, size: u32) {
    let w = |ee: &Ee, a: u32| ee.u32_at(a);
    if let Some(i) = small(ee, size) {
        let head = p + LISTS + 4 * i;
        if w(ee, head) == b {
            let next = w(ee, b + 0x14);
            ee.set_u32(head, next);
            if next != 0 {
                ee.set_u32(next + 0x10, 0);
            }
        } else {
            let (prev, next) = (w(ee, b + 0x10), w(ee, b + 0x14));
            ee.set_u32(prev + 0x14, next);
            if next != 0 {
                ee.set_u32(next + 0x10, prev);
            }
        }
        return;
    }
    let (l, r) = (w(ee, b + 0x10), w(ee, b + 0x14));
    // the child that takes the block's place (none for a leaf)
    let up = match (l, r) {
        (0, 0) => {
            let parent = w(ee, b + 0x18);
            if parent != 0 {
                if w(ee, parent + 0x10) == b {
                    ee.set_u32(parent + 0x10, 0);
                }
                if w(ee, w(ee, b + 0x18) + 0x14) == b {
                    ee.set_u32(w(ee, b + 0x18) + 0x14, 0);
                }
            }
            if w(ee, p + ROOT) == b {
                ee.set_u32(p + ROOT, 0);
            }
            return;
        }
        (0, r) => {
            let parent = w(ee, b + 0x18);
            if parent != 0 {
                if w(ee, parent + 0x10) == b {
                    ee.set_u32(parent + 0x10, r);
                }
                if w(ee, w(ee, b + 0x18) + 0x14) == b {
                    ee.set_u32(w(ee, b + 0x18) + 0x14, w(ee, b + 0x14));
                }
                ee.set_u32(w(ee, b + 0x14) + 0x18, w(ee, b + 0x18));
            }
            w(ee, b + 0x14)
        }
        (l, 0) => {
            let parent = w(ee, b + 0x18);
            if parent != 0 {
                if w(ee, parent + 0x10) == b {
                    ee.set_u32(parent + 0x10, l);
                }
                if w(ee, w(ee, b + 0x18) + 0x14) == b {
                    ee.set_u32(w(ee, b + 0x18) + 0x14, w(ee, b + 0x10));
                }
                ee.set_u32(w(ee, b + 0x10) + 0x18, w(ee, b + 0x18));
            }
            w(ee, b + 0x10)
        }
        (l, r) => {
            // the left subtree hung under the right one's leftmost block, the right one moved up
            let mut m = r;
            while w(ee, m + 0x10) != 0 {
                m = w(ee, m + 0x10);
            }
            ee.set_u32(m + 0x10, l);
            ee.set_u32(w(ee, b + 0x10) + 0x18, m);
            let parent = w(ee, b + 0x18);
            if parent != 0 {
                if w(ee, parent + 0x10) == b {
                    ee.set_u32(parent + 0x10, w(ee, b + 0x14));
                } else if w(ee, parent + 0x14) == b {
                    ee.set_u32(parent + 0x14, w(ee, b + 0x14));
                }
                if w(ee, b + 0x14) != 0 {
                    ee.set_u32(w(ee, b + 0x14) + 0x18, w(ee, b + 0x18));
                }
            }
            w(ee, b + 0x14)
        }
    };
    if w(ee, p + ROOT) == b {
        ee.set_u32(p + ROOT, up);
        if up != 0 {
            ee.set_u32(up + 0x18, 0);
        }
    }
}

/// The block `b` filed free with `size` bytes: on its small list's head, or as a leaf of the tree.
fn insert(ee: &mut Ee, p: u32, b: u32, size: u32) {
    let mark = |ee: &mut Ee, size: u32, parent: u32| {
        ee.set_u32(b + 4, 0);
        let w = ee.u32_at(b + 0xc);
        ee.set_u32(b + 0xc, (w & FREE) | size | FREE);
        ee.set_u32(b + 0x18, parent);
        ee.set_u32(b + 0x10, 0);
        ee.set_u32(b + 0x14, 0);
        ee.set_u32(b + size + 0x18, size);
    };
    if let Some(i) = small(ee, size) {
        let head = p + LISTS + 4 * i;
        let first = ee.u32_at(head);
        mark(ee, ee.u32_at(SMALL_SIZES_AT + 4 * i), 0);
        ee.set_u32(b + 0x10, 0);
        if first == 0 {
            ee.set_u32(head, b);
            ee.set_u32(b + 0x14, 0);
        } else {
            ee.set_u32(b + 0x14, first);
            ee.set_u32(first + 0x10, b);
            ee.set_u32(head, b);
        }
        return;
    }
    let mut n = ee.u32_at(p + ROOT);
    if n == 0 {
        mark(ee, size, 0);
        ee.set_u32(p + ROOT, b);
        return;
    }
    let slot = loop {
        let slot = if size < self::size(ee, n) { n + 0x10 } else { n + 0x14 };
        match ee.u32_at(slot) {
            0 => break slot,
            next => n = next,
        }
    };
    mark(ee, size, n);
    ee.set_u32(slot, b);
}

/// The block at header `b` given `size` bytes and the pool's id.
fn take(ee: &mut Ee, p: u32, b: u32, size: u32) {
    ee.set_u32(b + 0xc, size);
    set_u16(ee, b + 2, ee.u16_at(p + ID));
    ee.set_u32(b + size + 0x18, size);
}

/// A new block of `bytes` from pool `p` (0 when none fits): the best fit, split when bigger (a
/// bigger one searched again with room for the rest's header and 16 bytes).
fn alloc(ee: &mut Ee, p: u32, bytes: u32) -> u32 {
    let bytes = round(bytes);
    let mut b = find(ee, p, ee.u32_at(p + ROOT), bytes);
    if b == 0 {
        return 0;
    }
    let mut have = size(ee, b);
    let mut rest = None;
    if (bytes as i32) < have as i32 {
        b = find(ee, p, ee.u32_at(p + ROOT), bytes + 0x20);
        if b == 0 {
            return 0;
        }
        have = size(ee, b);
        rest = Some((b + bytes + 0x10, have.wrapping_sub(bytes) - 0x10));
    }
    unlink(ee, p, b, have);
    match rest {
        None => take(ee, p, b, have),
        Some((r, n)) => {
            insert(ee, p, r, n);
            take(ee, p, b, bytes);
        }
    }
    b + 0x10
}

/// The block `block` freed, joined with a free block before and after it.
fn free(ee: &mut Ee, p: u32, block: u32) {
    let mut b = block - 0x10;
    let size = self::size(ee, b);
    let mut joined = size;
    if ee.u32_at(p + ROOT) == 0 {
        insert(ee, p, b, size);
        return;
    }
    let before = ee.i32_at(b + 8);
    let prev = if before > 0 { b.wrapping_sub(before as u32 + 0x10) } else { 0 };
    let next = b + size + 0x10;
    if prev != 0 && ee.u32_at(prev + 0xc) & FREE != 0 {
        joined = size.wrapping_add(before as u32) + 0x10;
        unlink(ee, p, prev, ee.u32_at(prev + 0xc) & SIZE);
        b = prev;
    }
    if next != 0 && ee.u32_at(next + 0xc) & FREE != 0 {
        let n = ee.u32_at(next + 0xc) & SIZE;
        joined = joined.wrapping_add(n) + 0x10;
        unlink(ee, p, next, n);
    }
    ee.set_u32(b + 0x18, 0);
    ee.set_u32(b + 0x10, 0);
    ee.set_u32(b + 0x14, 0);
    insert(ee, p, b, joined);
}

/// `block` (0 for a new one) resized to `bytes`: grown in place into a free block after it when
/// that has the room (all of it taken unless 0x21 bytes or more are left over), else moved to a
/// new block, its bytes copied and the old one freed; as the original, answers the block (0 when
/// the heap has no room).
pub fn resize(ee: &mut Ee, block: u32, bytes: u32) -> u32 {
    let p = pool_of(ee, block);
    if block == 0 {
        return alloc(ee, p, bytes);
    }
    let mut bytes = round(bytes);
    let b = block - 0x10;
    let have = size(ee, b);
    if bytes == have {
        return block;
    }
    let next = b + have + 0x10;
    let nw = ee.u32_at(next + 0xc);
    let need = bytes.wrapping_sub(have) as i32;
    if (have as i32) < bytes as i32 && nw & FREE != 0 && need <= (nw & SIZE) as i32 {
        let n = nw & SIZE;
        let left = n as i32 - need;
        let split = if left < 0x21 {
            bytes = have + n + 0x10;
            0
        } else {
            b + bytes + 0x10
        };
        unlink(ee, p, next, n);
        take(ee, p, b, bytes);
        if split != 0 {
            insert(ee, p, split, left as u32);
        }
        return block;
    }
    let moved = alloc(ee, p, bytes);
    let n = bytes.min(have) as i32;
    for k in 0..n.max(0) as u32 {
        let v = ee.u8_at(block + k);
        ee.write(moved + k, 1, u128::from(v));
    }
    free(ee, p, block);
    moved
}

/// **MD140 — a grow in Rust, checked**: the original's resize (at `original`, called with `block`,
/// `bytes`, the caller's `file` and `line`) run on the RAM as it is and put aside, then [`resize`]
/// on the same RAM; every word outside the stack and the answer must equal the original's. Answers
/// the port's block.
pub fn resize_checked(ee: &mut Ee, original: u32, gp: u64, budget: u64, block: u32, bytes: u32, file: u32, line: u32) -> u32 {
    let before = ee.ram.clone();
    ee.set(28, gp);
    ee.call(original, &[u64::from(block), u64::from(bytes), u64::from(file), u64::from(line)], &[], budget);
    let want = ee.v0() as u32;
    let theirs = std::mem::replace(&mut ee.ram, before);
    let got = resize(ee, block, bytes);
    let stack = (ee.stack as usize).saturating_sub(0x10_0000);
    let word = |r: &[u8], i: usize| u32::from_le_bytes(r[i..i + 4].try_into().unwrap());
    let differ: Vec<String> = (0..stack)
        .step_by(4)
        .filter(|&i| theirs[i..i + 4] != ee.ram[i..i + 4])
        .map(|i| format!("{i:#010x}: original {:08x}, port {:08x}", word(&theirs, i), word(&ee.ram, i)))
        .collect();
    assert!(differ.is_empty(), "the heap's resize ({block:#x} to {bytes:#x} bytes): {} words differ:\n  {}", differ.len(), differ.join("\n  "));
    assert_eq!(got, want, "the heap's resized block");
    got
}
