//! **The vehicle's effects** (F31b5n): one of the leaves the bike's vehicle update reaches in its
//! part walk ([`super::tail::walk`]), the one that turns the bike's effect items on and off. Where the
//! original keeps it: research/oracle.txt `# F31b5n`.
//!
//! The effects listener holds a table of conditions (its `+0x24`: a count, then entries [`ENTRY`]
//! bytes apart from the table's `+4`) and a list of items (`+0x28`: a count, the items' array at
//! `+0x30`). Each frame, [`step`]:
//!
//! 1. gives each entry a flags byte (`+0x28`) from the two words it points at (`+8`, `+0xc`): bit 0
//!    with both zero, [`FIRST_SET`] and [`SECOND_SET`] with each set, and [`HIDDEN`] or [`SHOWN`] by
//!    the word the table's `+0x24` points at;
//! 2. when the key an entry points at (`+0x24`, a byte) is no longer the one it kept (`+0x29`), keeps
//!    it and tells each item bound to the entry (the item's `+4`) of it ([`Out::Key`]);
//! 3. turns each item on ([`Out::On`]) or off ([`Out::Off`]) by its mask (`+0xc`) against its entry's
//!    flags and the table's bit, and steps it while it is on (`+8` reads 1, [`Out::Step`]).
//!
//! It ends with the walk of its own listeners, which its caller makes. The items' own work (the
//! particle emitters they turn on and off among it) is theirs, handed out.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use ps2_float::F;

/// The listener's table (`+0x24`) and its items' count (`+0x28`) and array (`+0x30`).
pub const TABLE: u32 = 0x24;
pub const ITEMS: u32 = 0x28;
pub const ITEM_ARRAY: u32 = 0x30;
/// An entry's size, and its words: the two it tests (`+8`, `+0xc`, each a pointer to the word), the
/// key's pointer (`+0x24`), its flags (`+0x28`) and the key it kept (`+0x29`), both bytes.
pub const ENTRY: u32 = 0x2c;
pub const FIRST: u32 = 8;
pub const SECOND: u32 = 0xc;
pub const KEY: u32 = 0x24;
pub const FLAGS: u32 = 0x28;
pub const KEPT: u32 = 0x29;
/// The flags' bits: both words zero, the first set, the second set; and of the table's word (its
/// `+0x24` points at it), [`HIDDEN`] when it is set and [`SHOWN`] when it is not.
pub const NONE_SET: u8 = 1;
pub const FIRST_SET: u8 = 2;
pub const SECOND_SET: u8 = 4;
pub const HIDDEN: u8 = 8;
pub const SHOWN: u8 = 0x10;
/// An item's entry (`+4`, 0 for none), its state byte (`+8`, 1 on: bit 0 read as "on" when turning)
/// and its mask (`+0xc`).
pub const ITEM_ENTRY: u32 = 4;
pub const ITEM_STATE: u32 = 8;
pub const ITEM_MASK: u32 = 0xc;

/// What the effects hand out: each an item's virtual.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Out {
    /// The item's key changed (its vt `+8`, the new key as a byte).
    Key { item: u32, key: u8 },
    /// The item turned on (vt `+0x18`) or off (vt `+0x1c`).
    On(u32),
    Off(u32),
    /// The item stepped (vt `+0x14`, `dt`).
    Step(u32),
}

/// The effects' world: RAM words, a byte store, and the items' virtuals.
pub trait Effects {
    fn word(&mut self, at: u32) -> u32;
    fn set_byte(&mut self, at: u32, b: u8);
    fn out(&mut self, c: Out, dt: F);
}

/// The table's bit: [`HIDDEN`] with the word its `+0x24` points at set, [`SHOWN`] without.
fn shown_bit(m: &mut impl Effects, table: u32) -> u8 {
    let at = m.word(table + TABLE);
    if m.word(at) != 0 { HIDDEN } else { SHOWN }
}

/// The word the word at `at` points at.
fn through(m: &mut impl Effects, at: u32) -> u32 {
    let p = m.word(at);
    m.word(p)
}

fn byte(m: &mut impl Effects, at: u32) -> u8 {
    (m.word(at & !3) >> (8 * (at & 3))) as u8
}

/// The effects listener `e`'s item `k`.
fn item_at(m: &mut impl Effects, e: u32, k: u32) -> u32 {
    let items = m.word(e + ITEM_ARRAY);
    m.word(items + 4 * k)
}

/// The effects listener `e`'s own work for a frame of `dt` (without its closing walk). Counts and
/// the table are read again where the original reads them again: an item's call may change them.
pub fn step(m: &mut impl Effects, e: u32, dt: F) {
    let table = m.word(e + TABLE);
    let mask = shown_bit(m, table);
    let mut table = table;
    let mut i = 0;
    while (i as i32) < m.word(table) as i32 {
        let entry = m.word(table + 4) + ENTRY * i;
        let first = through(m, entry + FIRST);
        let mut flags = u8::from(first == 0 && through(m, entry + SECOND) == 0) * NONE_SET;
        let second = through(m, entry + SECOND);
        flags |= shown_bit(m, table);
        if first != 0 {
            flags |= FIRST_SET;
        }
        if second != 0 {
            flags |= SECOND_SET;
        }
        m.set_byte(entry + FLAGS, flags);
        let at = m.word(entry + KEY);
        let key = byte(m, at);
        if key != byte(m, entry + KEPT) {
            m.set_byte(entry + KEPT, key);
            let mut k = 0;
            while (k as i32) < m.word(e + ITEMS) as i32 {
                let item = item_at(m, e, k);
                if m.word(item + ITEM_ENTRY) == entry {
                    let key = byte(m, entry + KEPT);
                    m.out(Out::Key { item, key }, dt);
                }
                k += 1;
            }
        }
        table = m.word(e + TABLE);
        i += 1;
    }
    let mut k = 0;
    while (k as i32) < m.word(e + ITEMS) as i32 {
        let item = item_at(m, e, k);
        let link = m.word(item + ITEM_ENTRY);
        let wants = m.word(item + ITEM_MASK);
        let off = (link != 0 && wants & u32::from(byte(m, link + FLAGS)) != 0) || wants & u32::from(mask) != 0;
        let on = byte(m, item + ITEM_STATE) & 1 != 0;
        if off && on {
            m.out(Out::Off(item), dt);
        } else if !off && !on {
            m.out(Out::On(item), dt);
        }
        if byte(m, item + ITEM_STATE) == 1 {
            m.out(Out::Step(item), dt);
        }
        k += 1;
    }
}
