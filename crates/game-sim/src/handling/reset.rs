//! **SLUS-21104's bike out of the world** (F47): what the bike's PostProcess calls when its origin
//! leaves the world ([`super::bike::air_state`], [`super::bike::vehicle_post`]), as word-level ports
//! over RAM. C15's rides never leave the world, so these are proved by oracle alone. Where the
//! original keeps them: research/oracle.txt `# F47`.
//!
//! - [`body_reset`] (the bike's vt `+0x13c`): with the body on, the body, the bike's `+0xbc` and its
//!   post element taken off the physics world's three lists, the body turned off (its virtual,
//!   [`Out::BodyOn`], served by [`body_on`]), and the collision record let go of: with bit 7 of the
//!   byte `+0x44` the riding part's owner cleared and the world's word added to its list
//!   ([`add_unique`]), else the record handed back to its owner ([`Out::Release`]). With bit 6, bit 1
//!   of `+0x24` cleared.
//! - [`respawn`] (vt `+0xe8`): the ground reset ([`Out::Ground`], [`super::wreck::ground`]) and the
//!   respawned word `+0x31c` set.
//! - [`post`]: an event posted on a queue (the out-of-world message: category 8, id 13): the
//!   poster's registration found, and each of its live listeners told ([`Out::Tell`]).
//!
//! Every word goes through [`Resets`] as it is read or written, in the original's order, and every
//! call out is an [`Out`].
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

/// A call the reset makes outside its own words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    /// The body's on word set (its second vtable, at `+0x110`, slot `+0xc`): [`body_on`].
    BodyOn { body: u32, on: u32 },
    /// The collision record handed back to its owner (the bike's `+0xdc`, the record `+0xc8`).
    Release { owner: u32, record: u32 },
    /// A list's storage grown to `bytes` (the allocator's resize of `at`): its new address.
    Grow { at: u32, bytes: u32 },
    /// The ground reset (the put-back's, [`super::wreck::ground`]) on the bike.
    Ground,
    /// The message laid out where the listeners read it (the poster's frame): its address.
    Message([u32; 4]),
    /// A listener (`to`, its vtable at `+8`, slot `+0xc`) told the poster's word and the message.
    Tell { to: u32, from: u32, message: u32 },
}

/// The reset's outside world: words by address, and its calls (the answer is read only from
/// [`Out::Grow`] and [`Out::Message`]).
pub trait Resets {
    fn word(&mut self, at: u32) -> u32;
    fn set(&mut self, at: u32, w: u32);
    fn out(&mut self, c: Out) -> u32;
}

/// The bike's words, as offsets: its body, the word after it the world lists hold, the physics
/// world, the post element, the collision record and its owner, the riding part, the world bounds,
/// the flag bytes and words, and the respawned word.
pub const BODY: u32 = 0xb8;
pub const SECOND: u32 = 0xbc;
pub const WORLD: u32 = 0xe0;
pub const POST: u32 = 0x34;
pub const RECORD: u32 = 0xc8;
pub const OWNER: u32 = 0xdc;
pub const RIDING: u32 = 0xcc;
pub const BOUNDS: u32 = 0xe8;
pub const FLAG_BYTE: u32 = 0x44;
pub const FLAG_WORD: u32 = 0x24;
pub const RESPAWNED: u32 = 0x31c;
/// The physics world's lists: the bodies, the bikes' second words, the post elements.
pub const LISTS: [u32; 3] = [0x50, 0x68, 0xb0];

fn byte(m: &mut impl Resets, at: u32) -> u32 {
    m.word(at & !3) >> (8 * (at & 3)) & 0xff
}

/// **`x` taken off a list** (a count, then the array's address at `+8`): the first entry that is `x`
/// replaced by the last, the count one less. Whether it was there.
pub fn remove(m: &mut impl Resets, list: u32, x: u32) -> bool {
    let count = m.word(list) as i32;
    if count <= 0 {
        return false;
    }
    let items = m.word(list + 8);
    for k in 0..count as u32 {
        if m.word(items.wrapping_add(4 * k)) == x {
            let last = m.word(items.wrapping_add(4 * count as u32).wrapping_sub(4));
            m.set(items.wrapping_add(4 * k), last);
            let c = m.word(list);
            m.set(list, c.wrapping_sub(1));
            return true;
        }
    }
    false
}

/// **`x` added to a part's list once** (its array `+0xfc`, `+0x100` long): nothing if it is there,
/// else into the first empty slot, else the array grown by one ([`Out::Grow`]) and `x` at its end.
pub fn add_unique(m: &mut impl Resets, part: u32, x: u32) {
    let count = m.word(part + 0x100) as i32;
    let n = count.max(0) as u32;
    let items = m.word(part + 0xfc);
    if (0..n).any(|k| m.word(items.wrapping_add(4 * k)) == x) {
        return;
    }
    let items = m.word(part + 0xfc);
    for k in 0..n {
        if m.word(items.wrapping_add(4 * k)) == 0 {
            m.set(items.wrapping_add(4 * k), x);
            return;
        }
    }
    let at = m.word(part + 0xfc);
    let grown = m.out(Out::Grow { at, bytes: (count.wrapping_add(1) as u32).wrapping_mul(4) });
    m.set(part + 0xfc, grown);
    let (c, items) = (m.word(part + 0x100), m.word(part + 0xfc));
    m.set(items.wrapping_add(c.wrapping_mul(4)), x);
    let c = m.word(part + 0x100);
    m.set(part + 0x100, c.wrapping_add(1));
}

/// **The body's on word** ([`Out::BodyOn`]): its word 0.
pub fn body_on(m: &mut impl Resets, body: u32, on: u32) {
    m.set(body, on);
}

/// **The bike's body reset** (its vt `+0x13c`, the vehicle post's out of the world).
pub fn body_reset(m: &mut impl Resets, bike: u32) {
    let body = m.word(bike + BODY);
    if m.word(body) == 0 {
        return;
    }
    let world = m.word(bike + WORLD);
    remove(m, world.wrapping_add(LISTS[0]), body);
    let (world, second) = (m.word(bike + WORLD), m.word(bike + SECOND));
    remove(m, world.wrapping_add(LISTS[1]), second);
    let world = m.word(bike + WORLD);
    remove(m, world.wrapping_add(LISTS[2]), if bike == 0 { 0 } else { bike + POST });
    let body = m.word(bike + BODY);
    m.out(Out::BodyOn { body, on: 0 });
    let owner = m.word(bike + OWNER);
    if owner != 0 {
        let record = m.word(bike + RECORD);
        if record != 0 {
            if byte(m, bike + FLAG_BYTE) & 0x80 != 0 {
                let riding = m.word(bike + RIDING);
                if riding != 0 {
                    m.set(riding + 0xf8, 0);
                    let bounds = m.word(bike + BOUNDS);
                    let x = m.word(bounds.wrapping_add(0xa4));
                    let riding = m.word(bike + RIDING);
                    add_unique(m, riding, x);
                }
            } else {
                m.out(Out::Release { owner, record });
            }
        }
    }
    if byte(m, bike + FLAG_BYTE) & 0x40 != 0 {
        let w = m.word(bike + FLAG_WORD);
        m.set(bike + FLAG_WORD, w & !2);
    }
}

/// **The bike's respawn** (its vt `+0xe8`, after the body reset): the ground reset, then `+0x31c` 1.
pub fn respawn(m: &mut impl Resets, bike: u32) {
    m.out(Out::Ground);
    m.set(bike + RESPAWNED, 1);
}

/// **An event posted** on `queue` by `poster`, the message `[kind, id, last, data]` (the order the
/// poster lays them out in): the queue's registrations (`+0x24` long, the array at `+0x2c`) walked
/// for the first live one (word 0 clear) of `poster` (`+0x10`) with its tag (`+4`) the poster's
/// word 0; with one, each of its listeners (`+0x30` long, re-read each time, the array at `+0x38`)
/// that is live (word 0 clear) told ([`Out::Tell`]: the listener `+0x10`, the registration's
/// `+0x10`). Nothing is posted without a registration.
pub fn post(m: &mut impl Resets, queue: u32, poster: u32, message: [u32; 4]) {
    let message = m.out(Out::Message(message));
    let mut found = None;
    let mut k = 0u32;
    while (k as i32) < m.word(queue + 0x24) as i32 {
        let items = m.word(queue + 0x2c);
        let e = m.word(items.wrapping_add(4 * k));
        if m.word(e) == 0 && m.word(e + 0x10) == poster && m.word(e + 4) == m.word(poster) {
            found = Some(e);
            break;
        }
        k += 1;
    }
    let Some(e) = found else { return };
    let mut k = 0u32;
    while (k as i32) < m.word(e + 0x30) as i32 {
        let items = m.word(e + 0x38);
        let l = m.word(items.wrapping_add(4 * k));
        if m.word(l) == 0 {
            let (to, from) = (m.word(l + 0x10), m.word(e + 0x10));
            m.out(Out::Tell { to, from, message });
        }
        k += 1;
    }
}
