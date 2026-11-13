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
//!
//! F56: the items' turning virtuals ([`on`], [`off`], [`key`], and the [`pause`] and [`unpause`] a
//! key reaches) are ported too, by the item's [`Class`]; what they hand their objects ([`Call`]) is
//! the objects' own.
//!
//! F58: the items' steps (`+0x14`, [`item_step`]) too, by the item's [`Kind`]: each lights or
//! douses its object by what the bike's words say, and sets the object's rates, spans or spots
//! before it lights; the object's own step at a new spot ([`Call::Move`]) is the object's.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use super::update::Quad;
use ps2_float::{F, Vf, fpu, fpu::Acc};

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

// --- F56: the items' turning virtuals ------------------------------------------------------------

/// An item's own words beyond the effects' ([`ITEM_ENTRY`], [`ITEM_STATE`], [`ITEM_MASK`]): its
/// object (`+0x14`, the thing it lights and douses, its vtable at the object's `+4`), and its
/// table of picks (a byte per key from `+0x18`, [`UNPICKED`] for a key that pauses it).
pub const ITEM_OBJECT: u32 = 0x14;
pub const ITEM_PICKS: u32 = 0x18;
pub const UNPICKED: u8 = 0xff;
/// The state byte's bits: on ([`Out::On`]) and paused (a key with no pick).
pub const ON: u8 = 1;
pub const PAUSED: u8 = 2;
/// A [`Class::Clocked`] item's clock (`+0x44`, cleared) and its count (`+0x4c`, set back to the
/// start at `+0x48`) when its object halts.
pub const CLOCK: u32 = 0x44;
pub const COUNT_START: u32 = 0x48;
pub const COUNT: u32 = 0x4c;
/// The word a [`Class::Picked`] item clears when it douses (`+0x28`), and the one a
/// [`Class::Latched`] item clears after it douses on turning off (`+0x90`).
pub const PICKED_WORD: u32 = 0x28;
pub const LATCH: u32 = 0x90;

/// An effects item's class, as its vtable (`+0x10`) says: what its turning virtuals (on `+0x18`,
/// off `+0x1c`, key `+8`, and pause `+0x20` and unpause `+0x24`, which only a key reaches) do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Turning on only sets [`ON`]; turning off and pausing halt its object and reset its clocks
    /// ([`CLOCK`], [`COUNT`]); unpausing only clears [`PAUSED`]; a key pauses or unpauses it by its
    /// pick, and hands the pick on to nobody.
    Clocked,
    /// Turning on lights its object unless paused, turning off and pausing douse it, unpausing
    /// lights it again if on; a key does nothing.
    Lit,
    /// As [`Class::Lit`], but a key with a pick hands it to the object ([`Call::Pick`]) and
    /// unpauses, one without pauses; and turning off and pausing clear [`PICKED_WORD`] before the
    /// douse.
    Picked,
    /// As [`Class::Picked`], but pausing clears nothing and turning off clears [`LATCH`] after the
    /// douse.
    Latched,
}

/// What an item hands its object (`+0x14`), each the object's virtual or its own function.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    /// Lit (vt `+0xa4`) and doused (vt `+0xa8`).
    Light(u32),
    Douse(u32),
    /// Halted (vt `+0x84`).
    Halt(u32),
    /// Given a pick (the object's own, not a virtual): the byte the key picked.
    Pick { object: u32, pick: u8 },
    /// F58: moved to a new spot and stepped there (vt `+0x98`, `dt`): the object's spot (`+0x30`)
    /// and the spot it came from (`+0x40`) are set before it.
    Move { object: u32, dt: F },
}

/// The items' world: RAM, each item's class, and the calls they hand their objects.
pub trait Items {
    fn word(&mut self, at: u32) -> u32;
    fn set(&mut self, at: u32, w: u32);
    fn set_byte(&mut self, at: u32, b: u8);
    fn class(&mut self, item: u32) -> Class;
    fn call(&mut self, c: Call);
}

fn item_byte(m: &mut impl Items, at: u32) -> u8 {
    (m.word(at & !3) >> (8 * (at & 3))) as u8
}

/// The item's state byte with `set` set and `clear` cleared, stored and read back.
fn restate(m: &mut impl Items, item: u32, set: u8, clear: u8) -> u8 {
    let s = (item_byte(m, item + ITEM_STATE) | set) & !clear;
    m.set_byte(item + ITEM_STATE, s);
    item_byte(m, item + ITEM_STATE)
}

fn object(m: &mut impl Items, item: u32) -> u32 {
    m.word(item + ITEM_OBJECT)
}

/// A clocked item's object halted, its clock cleared and its count set back to the start.
fn halt(m: &mut impl Items, item: u32) {
    let o = object(m, item);
    m.call(Call::Halt(o));
    m.set(item + CLOCK, 0);
    let start = m.word(item + COUNT_START);
    m.set(item + COUNT, start);
}

/// The item turned on (its vt `+0x18`).
pub fn on(m: &mut impl Items, item: u32) {
    let class = m.class(item);
    let s = restate(m, item, ON, 0);
    if class != Class::Clocked && s & PAUSED == 0 {
        let o = object(m, item);
        m.call(Call::Light(o));
    }
}

/// The item turned off (its vt `+0x1c`).
pub fn off(m: &mut impl Items, item: u32) {
    let class = m.class(item);
    restate(m, item, 0, ON);
    match class {
        Class::Clocked => halt(m, item),
        Class::Lit => {
            let o = object(m, item);
            m.call(Call::Douse(o));
        }
        Class::Picked => {
            m.set(item + PICKED_WORD, 0);
            let o = object(m, item);
            m.call(Call::Douse(o));
        }
        Class::Latched => {
            let o = object(m, item);
            m.call(Call::Douse(o));
            m.set(item + LATCH, 0);
        }
    }
}

/// The item paused (its vt `+0x20`).
pub fn pause(m: &mut impl Items, item: u32) {
    let class = m.class(item);
    restate(m, item, PAUSED, 0);
    match class {
        Class::Clocked => halt(m, item),
        Class::Picked => {
            m.set(item + PICKED_WORD, 0);
            let o = object(m, item);
            m.call(Call::Douse(o));
        }
        Class::Lit | Class::Latched => {
            let o = object(m, item);
            m.call(Call::Douse(o));
        }
    }
}

/// The item unpaused (its vt `+0x24`).
pub fn unpause(m: &mut impl Items, item: u32) {
    let class = m.class(item);
    let s = restate(m, item, 0, PAUSED);
    if class != Class::Clocked && s & ON != 0 {
        let o = object(m, item);
        m.call(Call::Light(o));
    }
}

/// The item told its entry's new key (its vt `+8`): its pick for the key ([`ITEM_PICKS`]) unpauses
/// it, [`UNPICKED`] pauses it. A picked or latched item hands the pick to its object first.
pub fn key(m: &mut impl Items, item: u32, key: u8) {
    let class = m.class(item);
    if class == Class::Lit {
        return;
    }
    let pick = item_byte(m, item + ITEM_PICKS + u32::from(key));
    if pick == UNPICKED {
        return pause(m, item);
    }
    if class != Class::Clocked {
        let object = object(m, item);
        m.call(Call::Pick { object, pick });
    }
    unpause(m, item);
}

// --- F58: the items' steps ------------------------------------------------------------------------

/// What an item's step (its vt `+0x14`) does, as its vtable says. Each reads the bike's words through
/// the effects' table (the word at the item's `+0`, its `+0x24`: [`TABLE`]), whose words point at
/// them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Nothing (a [`Class::Clocked`] item's step returns at once).
    Still,
    /// Lights its object at a rate ([`OBJECT_RATE`]) by a level from two of the table's words, and
    /// douses it when the level is not above zero.
    Gauge,
    /// Latches by its entry's two words ([`LATCH`]) and, latched and above its floor, lights its
    /// object with the definition's counts by a ramp on the table's word; douses it otherwise.
    Ramp,
    /// Swings its object's angle and spans by the table's word while the bike is on it (or the word
    /// is out of its dead band), lighting it; inside the band it fades its counts out over a quarter
    /// of a second and douses it.
    Swing,
    /// Lights or douses its object by the flags of the part a word points at, and moves the object
    /// to each of that part's picked spots, less the part's motion over the frame.
    Spots,
    /// Lights or douses its object by the table's bit, and moves it to each linked part's spot its
    /// mask lets through and whose flags it does not mask, less the part's motion over the frame.
    Linked,
}

/// The table's words the steps read, each a pointer to the word: [`Kind::Gauge`]'s level and its
/// zero (`+0x18`, `+0x20`), [`Kind::Swing`]'s gate (`+0x18`, `+0x1c`) and swing (`+0x20`), and
/// [`Kind::Linked`]'s parts (a count at `+0x10`, their array at `+0x14`).
pub const TABLE_LEVEL: u32 = 0x18;
pub const TABLE_GATE: u32 = 0x1c;
pub const TABLE_SWING: u32 = 0x20;
pub const TABLE_PARTS: u32 = 0x10;
pub const TABLE_PART_ARRAY: u32 = 0x14;
/// The entry's words a [`Kind::Ramp`] and a [`Kind::Swing`] item read (each a pointer to the word):
/// the ramp's word (`+0x10`), the swing's bike-on word (`+0x14`) and its swing (`+0x20`).
pub const ENTRY_RAMP: u32 = 0x10;
pub const ENTRY_ON: u32 = 0x14;
pub const ENTRY_SWING: u32 = 0x20;
/// The object's words the steps write: the definition (`+0xd8`), its angle (`+0x60`), its rate
/// (`+0x68`), and its spot and the spot it came from (`+0x30`, `+0x40`) before it moves.
pub const OBJECT_DEF: u32 = 0xd8;
pub const OBJECT_ANGLE: u32 = 0x60;
pub const OBJECT_RATE: u32 = 0x68;
pub const OBJECT_SPOT: u32 = 0x30;
pub const OBJECT_CAME_FROM: u32 = 0x40;
/// The object's flags byte (`+0xb1`): bit 0 clears [`DEF_SPIN`] in its definition's flags (`+0x28`)
/// when a [`Kind::Swing`] item lights it, and sets it without. The definition's pick (`+0x40`): a
/// [`Kind::Spots`] or [`Kind::Linked`] item hands its object a pick only when it differs.
pub const OBJECT_FLAGS2: u32 = 0xb1;
pub const DEF_FLAGS: u32 = 0x28;
pub const DEF_SPIN: u32 = 0x200;
pub const DEF_PICK: u32 = 0x40;
/// A [`Kind::Swing`] item's fade (`+0x28`): set to a quarter second each frame it swings.
pub const SWING_FADE: u32 = 0x28;
const QUARTER: u32 = 0x3e80_0000;

fn sf(m: &mut impl Items, at: u32) -> F {
    F::from_bits(m.word(at))
}

fn set_sf(m: &mut impl Items, at: u32, x: F) {
    m.set(at, x.to_bits());
}

/// The word the word at `at` points at, as a float.
fn sf_through(m: &mut impl Items, at: u32) -> F {
    let p = m.word(at);
    sf(m, p)
}

fn copy(m: &mut impl Items, from: u32, to: u32) {
    let w = m.word(from);
    m.set(to, w);
}

fn item_quad(m: &mut impl Items, at: u32) -> Quad {
    [0, 4, 8, 12].map(|k| m.word(at + k))
}

fn put_item_quad(m: &mut impl Items, at: u32, q: Quad) {
    for (k, w) in q.into_iter().enumerate() {
        m.set(at + 4 * k as u32, w);
    }
}

/// A `.xyz` result: `r`'s three lanes, `w` the raw word `of` holds.
fn lanes_xyz(r: Vf, of: Quad) -> Quad {
    let b = r.to_bits();
    [b[0], b[1], b[2], of[3]]
}

/// `vmulx.xyz` of the quad by `dt`.
fn scaled(q: Quad, dt: F) -> Quad {
    lanes_xyz(Vf::from_bits(q).mul(Vf::splat(dt.0)), q)
}

/// `vsub.xyz`: `a - b`, `w` the raw word `a` holds.
fn less(a: Quad, b: Quad) -> Quad {
    lanes_xyz(Vf::from_bits(a).sub(Vf::from_bits(b)), a)
}

fn add(a: F, b: F) -> F {
    F(fpu::add(a.0, b.0))
}
fn sub(a: F, b: F) -> F {
    F(fpu::sub(a.0, b.0))
}
fn mul(a: F, b: F) -> F {
    F(fpu::mul(a.0, b.0))
}
fn div(a: F, b: F) -> F {
    F(fpu::div(a.0, b.0))
}

/// The effects' table an item reads (its `+0`'s [`TABLE`]).
fn item_table(m: &mut impl Items, item: u32) -> u32 {
    let e = m.word(item);
    m.word(e + TABLE)
}

fn object_def(m: &mut impl Items, item: u32) -> u32 {
    let o = object(m, item);
    m.word(o + OBJECT_DEF)
}

fn light(m: &mut impl Items, item: u32) {
    let o = object(m, item);
    m.call(Call::Light(o));
}

fn douse(m: &mut impl Items, item: u32) {
    let o = object(m, item);
    m.call(Call::Douse(o));
}

/// [`HIDDEN`] with the table's `+0x24` word set, [`SHOWN`] without.
fn table_bit(m: &mut impl Items, table: u32) -> u32 {
    let p = m.word(table + TABLE);
    if m.word(p) != 0 { u32::from(HIDDEN) } else { u32::from(SHOWN) }
}

/// The flags two words make (as an entry's, [`step`]): [`NONE_SET`] with both zero, [`FIRST_SET`]
/// and [`SECOND_SET`] with each set.
fn pair_flags(m: &mut impl Items, first: u32, second: u32) -> u32 {
    let (a, b) = (m.word(first), m.word(second));
    u32::from(a == 0 && b == 0) * u32::from(NONE_SET) | u32::from(a != 0) * u32::from(FIRST_SET) | u32::from(b != 0) * u32::from(SECOND_SET)
}

/// The steps' world: the items' and a kind for each.
pub trait Steps: Items {
    fn kind(&mut self, item: u32) -> Kind;
}

/// The item stepped for a frame of `dt` (its vt `+0x14`).
pub fn item_step(m: &mut impl Steps, item: u32, dt: F) {
    match m.kind(item) {
        Kind::Still => {}
        Kind::Gauge => gauge(m, item),
        Kind::Ramp => ramp(m, item),
        Kind::Swing => swing(m, item, dt),
        Kind::Spots => spots(m, item, dt),
        Kind::Linked => linked(m, item, dt),
    }
}

/// [`Kind::Gauge`]: the level `0.5 * level * (span - zero) / span` (its span `+0x18`); above zero
/// the object's rate is its scale (`+0x1c`) times the level plus a half, and it lights.
fn gauge(m: &mut impl Items, item: u32) {
    let t = item_table(m, item);
    let span = sf(m, item + 0x18);
    let level = sf_through(m, t + TABLE_LEVEL);
    let zero = sf_through(m, t + TABLE_SWING);
    let x = div(mul(mul(F(0.5), level), sub(span, zero)), span);
    if x <= 0.0 {
        return douse(m, item);
    }
    let rate = mul(sf(m, item + 0x1c), add(F(0.5), x));
    let o = object(m, item);
    set_sf(m, o + OBJECT_RATE, rate);
    light(m, item);
}

/// [`Kind::Ramp`]: latched (its entry's first word sets [`LATCH`], the second alone clears it) and
/// the entry's word (`+0x10`) above its floor (`+0x28`), the definition's counts (`+0x110`,
/// `+0x114`) are its own (`+0x48`, `+0x4c`) times a ramp, `+0x3c + w * +0x40`; with its spans on
/// (`+0x30`) and the word above its knee (`+0x2c`) the ramp adds `w * +0x44`, the definition's
/// rates and spans are its own scaled (`+0x34`, `+0x38`), else they are its own as they are.
fn ramp(m: &mut impl Items, item: u32) {
    let entry = m.word(item + ITEM_ENTRY);
    if through_word(m, entry + FIRST) != 0 {
        m.set(item + LATCH, 1);
    } else if through_word(m, entry + SECOND) != 0 {
        m.set(item + LATCH, 0);
    }
    if m.word(item + LATCH) == 0 {
        return douse(m, item);
    }
    let entry = m.word(item + ITEM_ENTRY);
    let w = sf_through(m, entry + ENTRY_RAMP);
    if w <= sf(m, item + 0x28) {
        return douse(m, item);
    }
    let mut r = Acc::adda(F::ZERO, sf(m, item + 0x3c)).madd(sf(m, item + 0x40), w);
    if m.word(item + 0x30) != 0 {
        if w <= sf(m, item + 0x2c) {
            let d = object_def(m, item);
            copy(m, item + 0x58, d + 0x118);
            let d = object_def(m, item);
            copy(m, item + 0x5c, d + 0x11c);
            let d = object_def(m, item);
            let x = sub(sf(m, d + 0xc0), sf(m, d + 0xbc));
            set_sf(m, d + 0xc4, x);
            for (from, to) in [(0x70, 0x130), (0x80, 0x140)] {
                let d = object_def(m, item);
                let q = item_quad(m, item + from);
                put_item_quad(m, d + to, q);
            }
        } else {
            let a = Acc::adda(F::ZERO, r);
            let x = mul(sf(m, item + 0x58), sf(m, item + 0x34));
            let by = sf(m, item + 0x44);
            let d = object_def(m, item);
            set_sf(m, d + 0x118, x);
            r = a.madd(by, w);
            for (from, to) in [(0x5c, 0x11c)] {
                let x = mul(sf(m, item + from), sf(m, item + 0x34));
                let d = object_def(m, item);
                set_sf(m, d + to, x);
            }
            for (from, to) in [(0x70, 0x130), (0x80, 0x140), (0x88, 0x144)] {
                let x = mul(sf(m, item + from), sf(m, item + 0x38));
                let d = object_def(m, item);
                set_sf(m, d + to, x);
            }
        }
        let d = object_def(m, item);
        let span = less(item_quad(m, d + 0x140), item_quad(m, d + 0x130));
        let d = object_def(m, item);
        put_item_quad(m, d + 0x150, span);
    }
    for (from, to) in [(0x48, 0x110), (0x4c, 0x114)] {
        let x = mul(sf(m, item + from), r);
        let d = object_def(m, item);
        set_sf(m, d + to, x);
    }
    light(m, item);
}

fn through_word(m: &mut impl Items, at: u32) -> u32 {
    let p = m.word(at);
    m.word(p)
}

/// [`Kind::Swing`]: swings when the table's gate is open (its level set, its gate clear) and the
/// entry's word (`+0x14`) set, or the swing word is outside `±` its dead band (`+0x30`). Then the
/// object's angle is `-(+0x2c) * swing`, the definition's spans (`+0x130`/`+0x140` x by the angle's
/// half from `+0x50`/`+0x60`, y by a clamped `1 - (swing + 1) * +0x44` from `+0x54`/`+0x64` over
/// `+0x3c`/`+0x40`) and counts (`+0x110`/`+0x114`, its own `+0x34`/`+0x38` and half again by the
/// clamp), the fade set ([`SWING_FADE`]), the spin bit by the object's ([`OBJECT_FLAGS2`]) when
/// `+0x70` says so, and it lights. Inside the band it runs the fade down by `dt`, the counts by it
/// over a quarter, lit; out of fade it douses.
fn swing(m: &mut impl Items, item: u32, dt: F) {
    let t = item_table(m, item);
    let entry = m.word(item + ITEM_ENTRY);
    let gate = sf_through(m, t + TABLE_LEVEL) != 0.0 && sf_through(m, t + TABLE_GATE) == 0.0 && through_word(m, entry + ENTRY_ON) != 0;
    if !gate {
        let band = sf(m, item + 0x30);
        let s = sf_through(m, entry + ENTRY_SWING);
        let neg = F(fpu::neg(band.0));
        if !(s < neg) && s <= band {
            let fade = sf(m, item + SWING_FADE);
            if fade <= 0.0 {
                m.set(item + SWING_FADE, 0);
                return douse(m, item);
            }
            let fade = sub(fade, dt);
            set_sf(m, item + SWING_FADE, fade);
            let q = div(fade, F::from_bits(QUARTER));
            if fade <= 0.0 {
                m.set(item + SWING_FADE, 0);
                return douse(m, item);
            }
            for (from, to) in [(0x34, 0x110), (0x38, 0x114)] {
                let x = mul(sf(m, item + from), q);
                let d = object_def(m, item);
                set_sf(m, d + to, x);
            }
            return light(m, item);
        }
    }
    let s = sf_through(m, entry + ENTRY_SWING);
    let angle = mul(F(fpu::neg(sf(m, item + 0x2c).0)), s);
    let o = object(m, item);
    set_sf(m, o + OBJECT_ANGLE, angle);
    let o = object(m, item);
    let half = mul(F(0.5), F(fpu::abs(sf(m, o + OBJECT_ANGLE).0)));
    let x = sub(sf(m, item + 0x50), half);
    let d = m.word(o + OBJECT_DEF);
    set_sf(m, d + 0x130, x);
    let x = add(sf(m, item + 0x60), half);
    let d = object_def(m, item);
    set_sf(m, d + 0x140, x);
    let d = object_def(m, item);
    let x = sub(sf(m, d + 0x140), sf(m, d + 0x130));
    set_sf(m, d + 0x150, x);
    let s = sf_through(m, t + TABLE_SWING);
    let c = Acc::adda(F::ZERO, F::ONE).msub(add(F::ONE, s), sf(m, item + 0x44));
    let c = F(fpu::max(0.0, fpu::min(1.0, c.0)));
    let h = mul(F(0.5), c);
    for (base, by, to) in [(0x54, 0x3c, 0x134), (0x64, 0x40, 0x144)] {
        let x = Acc::adda(F::ZERO, sf(m, item + base)).madd(c, sf(m, item + by));
        let d = object_def(m, item);
        set_sf(m, d + to, x);
    }
    let d = object_def(m, item);
    let x = sub(sf(m, d + 0x144), sf(m, d + 0x134));
    set_sf(m, d + 0x154, x);
    for (from, to) in [(0x34, 0x110), (0x38, 0x114)] {
        let d = object_def(m, item);
        copy(m, item + from, d + to);
    }
    for (from, to) in [(0x34, 0x110), (0x38, 0x114)] {
        let k = sf(m, item + from);
        let d = object_def(m, item);
        let x = Acc::adda(F::ZERO, sf(m, d + to)).madd(k, h);
        set_sf(m, d + to, x);
    }
    m.set(item + SWING_FADE, QUARTER);
    if m.word(item + 0x70) != 0 {
        let o = object(m, item);
        let d = m.word(o + OBJECT_DEF);
        let flags = m.word(d + DEF_FLAGS);
        let spun = if item_byte(m, o + OBJECT_FLAGS2) & 1 != 0 { flags & !DEF_SPIN } else { flags | DEF_SPIN };
        m.set(d + DEF_FLAGS, spun);
    }
    light(m, item);
}

/// The object handed `pick` (when its definition's pick differs) and moved to `spot` from `spot`
/// less `motion` (both `.xyz`).
fn move_to<M: Items>(m: &mut M, item: u32, pick: u8, spot: u32, motion: impl FnOnce(&mut M) -> Quad, dt: F) {
    let o = object(m, item);
    let d = m.word(o + OBJECT_DEF);
    if u32::from(pick) != m.word(d + DEF_PICK) {
        m.call(Call::Pick { object: o, pick });
    }
    let motion = motion(m);
    let from = less(item_quad(m, spot), motion);
    let o = object(m, item);
    put_item_quad(m, o + OBJECT_CAME_FROM, from);
    let at = item_quad(m, spot);
    let o = object(m, item);
    put_item_quad(m, o + OBJECT_SPOT, at);
    let o = object(m, item);
    m.call(Call::Move { object: o, dt });
}

/// [`Kind::Spots`]: the part the word at `+0x28` points at (none: nothing) lights the object
/// unless the item's mask (`+0xc`) meets the part's flags (its `+0x3c`/`+0x40` words as an entry's,
/// and the table's bit), which douse it; lit, each of its spots (a count at `+0xf0`, re-read, the
/// array at `+0xf8`, 0x90 bytes apart) with a pick (the item's pick byte at `+0x18` plus the spot's
/// `+0x80`) moves the object to the spot (`+0x40`) less the part's motion (`+0x1f0`) times `dt`.
fn spots(m: &mut impl Items, item: u32, dt: F) {
    let p = m.word(item + 0x28);
    let part = m.word(p);
    if part == 0 {
        return;
    }
    let mask = m.word(item + ITEM_MASK);
    let t = item_table(m, item);
    let flags = pair_flags(m, part + 0x3c, part + 0x40) | table_bit(m, t);
    if mask & flags != 0 {
        return douse(m, item);
    }
    light(m, item);
    let motion = scaled(item_quad(m, part + 0x1f0), dt);
    let mut k = 0u32;
    while (k as i32) < m.word(part + 0xf0) as i32 {
        let spot = m.word(part + 0xf8).wrapping_add(k.wrapping_mul(0x90));
        let at = m.word(spot + 0x80);
        let pick = item_byte(m, item.wrapping_add(at).wrapping_add(ITEM_PICKS));
        if pick != UNPICKED {
            move_to(m, item, pick, spot + 0x40, |_| motion, dt);
        }
        k += 1;
    }
}

/// [`Kind::Linked`]: the object lit, or doused when the item's mask (`+0xc`) meets the table's bit;
/// then each of the table's parts ([`TABLE_PARTS`], read once) that is there, that the item's links
/// (`+0x18`, a bit per part) let through, with a pick (the item's pick byte at `+0x1c` plus the
/// part's byte `+0x3c`), and whose flags (its `+0x18`/`+0x1c` words as an entry's) the mask does
/// not meet, moves the object to the part's spot (`+0x40`) less its motion (the quad at its
/// `+0x80`'s `+0x80`) times `dt`.
fn linked<M: Items>(m: &mut M, item: u32, dt: F) {
    let t = item_table(m, item);
    let parts = m.word(t + TABLE_PART_ARRAY);
    let n = m.word(t + TABLE_PARTS) as i32;
    let bit = table_bit(m, t);
    if m.word(item + ITEM_MASK) & bit != 0 {
        douse(m, item);
    } else {
        light(m, item);
    }
    let mut k = 0u32;
    while (k as i32) < n {
        let part = m.word(parts.wrapping_add(4 * k));
        if part != 0 && m.word(item + 0x18) & (1 << (k & 31)) != 0 {
            let flags = pair_flags(m, part + 0x18, part + 0x1c);
            let at = item_byte(m, part + 0x3c);
            let pick = item_byte(m, item.wrapping_add(u32::from(at)).wrapping_add(0x1c));
            if pick != UNPICKED && m.word(item + ITEM_MASK) & flags == 0 {
                let motion = |m: &mut M| {
                    let by = m.word(part + 0x80);
                    scaled(item_quad(m, by + 0x80), dt)
                };
                move_to(m, item, pick, part + 0x40, motion, dt);
            }
        }
        k += 1;
    }
}
