//! **What hears the bike's riding part hit** (F54): SLUS-21104's two listeners on the riding part
//! (its `+0x104` list), each called by the search on a hit (vt `+8`, with the probe and the struck,
//! [`crate::world_search::Narrow::heard_hit`]). Neither reads the probe; both read the struck's
//! category (`+0x94`).
//!
//! - **The bike's own** (the bike `+0x38` in): [`bike_hears`] — with the bike's flag bit `0x02` at
//!   `+0x44` up and the category within the mask of the bike's `+0xe8` object (`+0xa4`), the bike's
//!   vt `+0x138`, the caller's ([`Heard::Bike`]); else nothing.
//! - **The pair log** (an object `+0x28` in): the pair pushed onto its log (`+0x3c8`, [`Log`]) as
//!   `{probe, struck, 0, kind}`, the kind 1 for a category within its `+0x2c` mask, else 2 within its
//!   `+0x30` mask, else 0 ([`kind`]). A full log grows by its step through the heap's resize
//!   ([`push`]).
//!
//! Field map: research/oracle.txt, `# F54`.

/// The bike's flag bit at `+0x44` that lets it hear a hit.
pub const BIKE_HEARS: u8 = 0x02;

/// **The bike's own listener**: whether it calls its vt `+0x138`.
pub fn bike_hears(flags: u8, mask: u32, category: u32) -> bool {
    flags & BIKE_HEARS != 0 && category & mask != 0
}

/// What a listener asks of its caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heard {
    /// Nothing more.
    Nothing,
    /// The bike's vt `+0x138`: `a0` the bike, `a1` the struck's category (the listener's own read of
    /// it left there, not the probe), `a2` the struck.
    Bike,
}

/// **The pair log's kind** of a struck with `category`, by the log's two masks (`+0x2c`, `+0x30`).
pub fn kind(category: u32, masks: [u32; 2]) -> u32 {
    if category & masks[0] != 0 {
        1
    } else if category & masks[1] != 0 {
        2
    } else {
        0
    }
}

/// The pair log's words: its count (`+0`), entries (`+8`, 16 bytes each), step (`+0xc`), capacity
/// (`+0x10`) and the most it may grow to (`+0x14`), all signed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Log {
    pub count: i32,
    pub entries: u32,
    pub step: i32,
    pub capacity: i32,
    pub most: i32,
}

/// **The push**: below capacity, the entry's slot (the count before; the count up one). At capacity,
/// the log grows to `count + step` entries when that is above its capacity and no more than its most
/// (`resize(entries, bytes)`, the heap's, answers the entries' new address), and the entry goes in;
/// else nothing is pushed (`None`).
pub fn push(log: &mut Log, resize: &mut dyn FnMut(u32, u32) -> u32) -> Option<u32> {
    if log.count >= log.capacity {
        let want = log.count.wrapping_add(log.step);
        if !(log.capacity < want && want <= log.most) {
            return None;
        }
        log.entries = resize(log.entries, (want as u32).wrapping_shl(4));
        log.capacity = want;
    }
    let slot = log.count as u32;
    log.count += 1;
    Some(slot)
}

/// The entry the pair log holds for a pair: the probe, the struck, 0, the kind.
pub fn entry(probe: u32, struck: u32, kind: u32) -> [u32; 4] {
    [probe, struck, 0, kind]
}
