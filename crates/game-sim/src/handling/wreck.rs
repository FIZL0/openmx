//! **SLUS-21104's bike wreck around the sim frame** (F31b4b): the wreck test the vehicle update asks
//! every frame the bike is up ([`super::tail::Out::WreckTest`]), the wreck's entry and the unwreck it
//! calls, and the put-back to the bike's `+0xd38` spot ([`super::tail::Out::PutBack`]). Where the
//! original keeps them and their calls: research/oracle.txt `# F31b4b`.
//!
//! The wreck test ([`wreck_test`]), for a bike someone drives (the AI driver's flag, or a state of
//! kind 1) with wrecks on (the game's word and the bike's):
//!
//! 1. out of bounds ([`Out::OutOfBounds`], or [`Out::Bounds`] for a position handed in) is a wreck
//!    13 unless the bike is already down for it, and the test stops there;
//! 2. with the bike not yet down, the clauses ([`clauses`]): reason 15, then the loads, the pitch and
//!    roll, the crossup and the trick endings, in [`super::step::wreck_test`]'s order — the first
//!    that holds enters its wreck;
//! 3. **then the hits and the striker whether or not a clause entered** ([`hits`]): the collision
//!    record is not swapped by the entry, so one frame can enter a wreck two or three times, and the
//!    last reason and throw are the ones that stay;
//! 4. the striker word is cleared;
//! 5. the wreck clock ([`clock`]): with a reason set, `dt` on; past the settle window (2.5 s for 13,
//!    else 0.5 s) the count arms and posts its event; counting and past `MaxWreckTime`, the unwreck.
//!
//! A bike nobody drives is wrecked once with 14 (no control), its clock at `MaxWreckTime` and
//! counting. Either way, with a reason set the run's riding bit goes, and a wreck that is new this
//! frame counts on the bike's wrecks.
//!
//! The entry ([`enter`]) draws the out-of-bounds throw (three `rand`s) for 13, turns the riding part
//! off ([`part`]), swaps the collision record's part for the wreck one ([`swap`]), and in its base
//! sets the reason and clock, rumbles the pad and hands the body its wreck damping. The unwreck
//! ([`unwreck`]) undoes it: the loads and the body's rotation back to rest, the rider's stunt ids to
//! none, the riding part back, the put-back ([`put_back`]), and in its base for 13 the throw's
//! divide, the reason and clock cleared and the body's rest damping back.
//!
//! **MD63f's wreck gravity is not here.** The two virtuals that rewrite gravity on entry and unwreck
//! sit in another class's vtable, which no object in state 4 uses; the state-4 bikes' entry and
//! unwreck touch gravity nowhere. The one word the
//! unwreck rescales is the body's `+0xe8` target's `+4`, divided for a wreck 13.
//!
//! Every word goes through [`Wrecks`] as it is read or written, in the original's order, and every
//! call out is an [`Out`]. The bike's own virtuals the wreck calls among themselves (the entry, the
//! unwreck, the part, the swaps, the put-back) are [`Out`]s too, so that each is proved on its own; a
//! caller that wants the port serves them with this module's functions.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air::normalize;
use ps2_float::vu::Dest;
use ps2_float::{F, Vf, fpu};

/// The quadword the wreck test compares a handed-in position with, and the unwreck's rest (0, 0, 0,
/// 1), and the body's rest damping the unwreck's base hands it: rodata.
pub const ORIGIN: u32 = 0x0039_6fc0;
pub const REST_DAMPING: u32 = 0x0039_6fd0;
/// The game's "wrecks on" word, the struck class the reason-15 test masks the record's object with,
/// and the divisor the unwreck's base rescales a wreck 13's throw by.
pub const WRECKS_ON: u32 = 0x0052_5350;
pub const STRUCK_CLASS: u32 = 0x0054_2db0;
pub const THROW_SCALE: u32 = 0x0052_6598;

/// The wreck's reasons the frame itself enters: out of bounds, no control, and the striker's and
/// flat's 5, scenery 6, joust 7, struck 15.
pub const OUT_OF_BOUNDS: u32 = 13;
pub const NO_CONTROL: u32 = 14;

/// The bike's words, as offsets.
pub const REASON: u32 = 0xc80;
pub const CLOCK: u32 = 0xc84;
pub const MAX_WRECK_TIME: u32 = 0xc88;
pub const COUNTING: u32 = 0xc8c;
pub const SETTLES: u32 = 0x920;
pub const RECORD: u32 = 0xc8;
pub const BODY: u32 = 0xb8;
pub const RUN: u32 = 0x680;
pub const STATE: u32 = 0xbc4;
/// The record's parts: riding (`+0x1438`) and wrecked (`+0x1434`); the record's own (`+0xcc`, `+0xd8`
/// and the record's `+0x200`) is one of the two.
pub const WRECK_PART: u32 = 0x1434;
pub const RIDE_PART: u32 = 0x1438;
/// The run's riding bit (word `+8`), cleared while a reason is set.
pub const RIDING_BIT: u32 = 0x0004_0000;

/// A call the wreck makes outside its own words, with the arguments that vary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Out {
    /// The bike's own virtuals the wreck calls among themselves: the entry (vt `+0x368`, the reason),
    /// the unwreck (`+0x36c`, the kind), the riding part on or off (`+0x398`, [`part`]), the record's
    /// part to the wrecked one or back (`+0x48c` / `+0x490`, [`swap`]), the put-back (`+0x3a8`, snap,
    /// kind).
    Enter(u32),
    Unwreck(u32),
    Part(bool),
    ToWreck,
    ToRide,
    PutBack { snap: u32, kind: u32 },
    /// The bike's out-of-bounds test (vt `+0x16c`); the handed-in position's own (the world's bounds
    /// test: the bike's `+0x3b0`, the position, two words out, 0, 150 ft). Each answers yes or no.
    OutOfBounds,
    Bounds { node: u32, at: u32 },
    /// The bike's vt `+0x4a8` and `+0x4b0` (−1), first in the entry and the unwreck; the settle's
    /// ([`settle`]) word to each of the player's listeners (its vt at `+0x20`, `+0xa0`: the listener,
    /// 5, a zero word, 0, 0).
    Settle,
    Stance(i32),
    Told(u32),
    /// `rand`: its answer.
    Rand,
    /// The event post (on the game's event queue, from the bike's `+0x674`, category 8):
    /// the id and the word after it.
    Post { id: u32, arg: u32 },
    /// The state's rumble test (the pad's, on the state's `+0x6ac`, an answer) and the rumbler's
    /// vt `+0x4c` (0, 200, 1.0).
    RumbleReady(u32),
    Rumble(u32),
    /// The body's damping virtuals (its vt at `+0x110`): `+0x20` and `+0x68` with a quadword in RAM,
    /// `+0x60` with one in RAM or the one handed.
    BodyAt { slot: u32, at: u32 },
    BodyQuad { slot: u32, q: Vf },
    /// A part's virtuals (its vt at `+0x28`, on its `+0x24`): on (`+0x20`) and off (`+0x18`); a node's
    /// unhook from its parent; a part's attach in the world (the part, the bike's `+0xb4`, `+0xc0`, 1).
    PartOn(u32),
    PartOff(u32),
    Unhook(u32),
    Attach { part: u32, a: u32, b: u32 },
    /// The engine's reset (on the bike's `+0x142c`).
    Engine(u32),
    /// The put-back's: each wheel's reset; the rider's reattach (the
    /// bike's `+0x1480`, `+0x1284`, 1, 1); the two bones' poses into the put-back's two buffers
    /// (the node's frame: the bone, 0, the buffer); a part's frame set from one; the ground reset;
    /// vt `+0x3a0` (kind set) or `+0x39c`; the trail's reset (on `+0x10dc`); vt `+0x378`; the
    /// camera's "actor moved" (on the bike's `+0x43c`).
    Wheel(u32),
    Rider { a: u32, b: u32 },
    Pose { bone: u32, buf: u32 },
    Place { part: u32, buf: u32 },
    Ground,
    Kind(bool),
    Trail(u32),
    Sync,
    Camera(u32),
    /// The terrain's normal under a spot (the ground query: the terrain, the spot, out, 0, 0), asked
    /// through [`Wrecks::normal`].
    Normal { terrain: u32, at: u32 },
}

/// The wreck's outside world: words by address (the bike is `bike`), and its calls. A call's answer
/// is its `v0`; only [`Out::OutOfBounds`], [`Out::Bounds`], [`Out::Rand`] and
/// [`Out::RumbleReady`] have one.
pub trait Wrecks {
    fn bike(&self) -> u32;
    fn word(&mut self, at: u32) -> u32;
    fn set(&mut self, at: u32, w: u32);
    fn out(&mut self, c: Out) -> u32;
    fn normal(&mut self, terrain: u32, at: u32) -> Vf;
}

fn w(m: &mut impl Wrecks, o: u32) -> u32 {
    let b = m.bike();
    m.word(b + o)
}

fn put(m: &mut impl Wrecks, o: u32, v: u32) {
    let b = m.bike();
    m.set(b + o, v);
}

fn f(m: &mut impl Wrecks, at: u32) -> F {
    F(f32::from_bits(m.word(at)))
}

fn fb(m: &mut impl Wrecks, o: u32) -> F {
    let b = m.bike();
    f(m, b + o)
}

fn quad(m: &mut impl Wrecks, at: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|k| m.word(at + k)))
}

fn set_quad(m: &mut impl Wrecks, at: u32, v: Vf) {
    for (k, x) in v.to_bits().into_iter().enumerate() {
        m.set(at + 4 * k as u32, x);
    }
}

fn copy_quad(m: &mut impl Wrecks, to: u32, from: u32) {
    let v = quad(m, from);
    set_quad(m, to, v);
}

/// The byte at `at`, and a byte written there (the word around it read and written back).
fn byte(m: &mut impl Wrecks, at: u32) -> u32 {
    m.word(at & !3) >> (8 * (at & 3)) & 0xff
}

fn set_byte(m: &mut impl Wrecks, at: u32, b: u32) {
    let s = 8 * (at & 3);
    let x = m.word(at & !3) & !(0xff << s) | (b & 0xff) << s;
    m.set(at & !3, x);
}

/// The g loads within the back, front and side limits (`<=`: on a limit is within), as reason 15
/// and the scenery hit ask it.
fn loads_within(m: &mut impl Wrecks) -> bool {
    let fore_aft = fb(m, 0x1258);
    if !(fore_aft <= fb(m, 0x13ec)) || fore_aft < F(fpu::neg(fb(m, 0x13e8).0)) {
        return false;
    }
    let (limit, side) = (fb(m, 0x13f0), fb(m, 0x1250));
    side <= limit && !(side < F(fpu::neg(limit.0)))
}

/// **Reason 15's test** (the steep-flip test, the backflip's too): the record touching an object of the
/// struck class, and the bike airborne or its loads past the limits.
pub fn struck(m: &mut impl Wrecks) -> bool {
    let rec = w(m, RECORD);
    if rec == 0 || m.word(rec + 0x34) == 0 {
        return false;
    }
    let obj = m.word(rec + 0x20c);
    if obj == 0 || m.word(obj + 0x94) & m.word(STRUCK_CLASS) == 0 {
        return false;
    }
    w(m, 0xde0) != 0 || !loads_within(m)
}

/// The record touching (its `+0x34`), the record read again.
fn touching(m: &mut impl Wrecks) -> bool {
    let rec = w(m, RECORD);
    m.word(rec + 0x34) != 0
}

/// **The wreck test's clauses** for a driven bike not yet down: the reason of the first that holds,
/// with the joust's throw written to `+0x1260`, or none. [`super::step::wreck_test`]'s clauses, read
/// off the words with the original's branch senses.
pub fn clauses(m: &mut impl Wrecks) -> Option<u32> {
    if struck(m) {
        return Some(15);
    }
    let fore_aft = fb(m, 0x1258);
    if !(fore_aft <= fb(m, 0x13ec)) {
        return Some(4);
    }
    if fore_aft < F(fpu::neg(fb(m, 0x13e8).0)) {
        return Some(1);
    }
    let (limit, side) = (fb(m, 0x13f0), fb(m, 0x1250));
    if !(side <= limit) {
        return Some(2);
    }
    if side < F(fpu::neg(limit.0)) {
        return Some(3);
    }
    let v = fb(m, 0x1254);
    if !(v <= fb(m, 0x1410)) && w(m, 0x1524) == w(m, 0x152c) {
        return Some(5);
    }
    if !(v <= fb(m, 0x13f4)) && w(m, 0x146c) != 0 {
        return Some(5);
    }
    if !(v <= fb(m, 0x13f8)) && w(m, 0x146c) == 0 {
        return Some(5);
    }
    let on_ground = w(m, 0x324) != 0;
    if on_ground && fb(m, 0x360) < F(fpu::neg(fb(m, 0x13fc).0)) {
        return Some(1);
    }
    if on_ground && !(fb(m, 0x360) <= fb(m, 0x1400)) {
        return Some(0xb);
    }
    let airborne = w(m, 0xde0) != 0;
    if airborne && w(m, 0x1194) == 0 && w(m, 0x11d0) == 0 {
        if fb(m, 0x360) < F(fpu::neg(fb(m, 0x1404).0)) || !(F(fpu::abs(fb(m, 0x368).0)) <= fb(m, 0x140c)) {
            return Some(8);
        }
    }
    if airborne && w(m, 0x1194) == 0 && w(m, 0x11d0) == 0 && !(fb(m, 0x360) <= fb(m, 0x1408)) {
        return Some(0xb);
    }
    if (w(m, 0x1194) != 0 || w(m, 0x11d0) != 0) && touching(m) {
        return Some(0xb);
    }
    if w(m, 0xde4) != 0 && !airborne && !(fb(m, 0x1190) <= fb(m, 0x1424)) {
        let (limit, c) = (fb(m, 0x1420), fb(m, 0x141c));
        if !(c <= limit) || c < F(fpu::sub(1.0, limit.0)) {
            return Some(if c <= F(0.5) { 3 } else { 2 });
        }
    }
    if (on_ground || touching(m)) && (w(m, 0x1524) as i32) < 41 && w(m, 0x14cc) == 0 {
        return Some(0xc);
    }
    let id = w(m, 0x1524);
    if id == w(m, 0x1530) && !(fb(m, 0x14e4) <= F(f32::from_bits(0x3f73_3333))) {
        let body = w(m, BODY);
        let v = quad(m, body + 0x80);
        let throw = v.merge(Dest::XYZ, v.sub(normalize(v).mul_q(10.0)));
        let b = m.bike();
        set_quad(m, b + 0x1260, throw);
        return Some(7);
    }
    if id == w(m, 0x152c) && fb(m, 0xbf4) < fb(m, 0x1414) {
        return Some(0xc);
    }
    None
}

/// How far up the bike a contact's ray must reach for the hits: 2.9 with the size word (`+0xbc0`) 0,
/// else 4.5 ([`super::step::HIT_REACH_SMALL`], [`super::step::HIT_REACH`]).
fn reach(m: &mut impl Wrecks) -> F {
    if w(m, 0xbc0) == 0 { F(f32::from_bits(0x4039_999a)) } else { F(4.5) }
}

/// **The hits and the striker**, after the clauses whatever they did: the first of the record's own
/// contacts (not mirrored) reaching past [`reach`] decides — an actor standing higher than this bike
/// by more than the joust height is a joust (its velocity the throw), scenery with the loads outside
/// their limits is a scenery wreck — and ends the search; then a striker of the bike's mask is a 5.
pub fn hits(m: &mut impl Wrecks) {
    let rec = w(m, RECORD);
    if m.word(rec + 0x34) != 0 {
        let limit = reach(m);
        let (list, n) = (m.word(rec + 0xf8), m.word(rec + 0xf0) as i32);
        for i in 0..n.max(0) as u32 {
            let e = list + 0x90 * i;
            if m.word(e) != 0 || f(m, e + 0x24) <= limit {
                continue;
            }
            let actor = m.word(e + 0x84);
            if actor != 0 {
                let theirs = m.word(actor + 0xe4);
                let height = fb(m, 0x1418);
                let ours = w(m, BODY);
                let up = F(fpu::sub(f(m, theirs + 0x74).0, f(m, ours + 0x74).0));
                if !(up <= height) {
                    let b = m.bike();
                    copy_quad(m, b + 0x1260, theirs + 0x80);
                    m.out(Out::Enter(7));
                }
            } else if !loads_within(m) {
                m.out(Out::Enter(6));
            }
            break;
        }
    }
    let striker = w(m, 0x1440);
    if striker != 0 && m.word(striker + 0x94) & w(m, 0x1110) != 0 {
        m.out(Out::Enter(5));
    }
}

/// **The wreck clock**, with a reason set: `dt` on; not yet counting and settled, past the window
/// (2.5 s for 13, else 0.5 s; "clock at most the window" skips) the count arms, the clock goes to 0
/// and the event (id 12) is posted; counting and past `MaxWreckTime`, the unwreck (kind 1).
pub fn clock(m: &mut impl Wrecks, dt: F) {
    if w(m, REASON) == 0 {
        return;
    }
    let c = fpu::add(fb(m, CLOCK).0, dt.0);
    put(m, CLOCK, c.to_bits());
    if w(m, COUNTING) == 0 && w(m, SETTLES) != 0 {
        let window = if w(m, REASON) == OUT_OF_BOUNDS { F(2.5) } else { F(0.5) };
        if !(fb(m, CLOCK) <= window) {
            put(m, COUNTING, 1);
            put(m, CLOCK, 0);
            m.out(Out::Post { id: 12, arg: 0 });
        }
    }
    if w(m, COUNTING) != 0 && !(fb(m, CLOCK) <= fb(m, MAX_WRECK_TIME)) {
        m.out(Out::Unwreck(1));
    }
}

/// **The wreck test** (vt `+0x49c`): `at` the position handed in (0: none; the vehicle
/// update hands none), `dt` the frame's.
pub fn wreck_test(m: &mut impl Wrecks, at: u32, dt: F) {
    let prev = w(m, REASON);
    let driver = w(m, 0x48c);
    let state = w(m, STATE);
    let driven = (driver != 0 && m.word(driver + 0xe4) != 0) || (state != 0 && m.word(state + 0x28) == 1);
    if !driven {
        if prev == 0 {
            m.out(Out::Enter(NO_CONTROL));
            let t = w(m, MAX_WRECK_TIME);
            put(m, CLOCK, t);
            put(m, COUNTING, 1);
        }
    } else {
        if m.word(WRECKS_ON) != 0 && w(m, 0xc1c) != 0 {
            let mut free = true;
            if w(m, 0x308) != 0 && prev != OUT_OF_BOUNDS {
                let mut off = 0;
                if at != 0 && (0..4).any(|k| m.word(at + 4 * k) != m.word(ORIGIN + 4 * k)) {
                    let node = w(m, 0x3b0);
                    off = m.out(Out::Bounds { node, at });
                }
                if m.out(Out::OutOfBounds) != 0 || off != 0 {
                    m.out(Out::Enter(OUT_OF_BOUNDS));
                    free = false;
                }
            }
            if free && w(m, REASON) == 0 {
                if let Some(r) = clauses(m) {
                    m.out(Out::Enter(r));
                }
                hits(m);
            }
            put(m, 0x1440, 0);
        }
        clock(m, dt);
    }
    if w(m, REASON) != 0 {
        let run = w(m, RUN);
        if run != 0 {
            let x = m.word(run + 8);
            m.set(run + 8, x & !RIDING_BIT);
        }
    }
    if prev == 0 && w(m, REASON) != 0 {
        let n = w(m, 0x6c4).wrapping_add(1);
        put(m, 0x6c4, n);
    }
}

/// **The settle** (vt `+0x4a8`, [`Out::Settle`]), the stunts' end with a player on the bike (the run
/// `+0x684` and the player `+0x680`): the stance's (vt `+0x4b0`, −1), the wheelie's and stoppie's
/// clocks (`+0x1744`, `+0x1748`) and their four words past (`+0x704..+0x710`) to 0, each of the
/// player's listeners (the list at its `+0x1d4`, on through each one's `+0x1c`) told, and the flip's
/// clip and the word after it (`+0x1754`, `+0x1758`) to 0: the bike's vt `+0x53c` and `+0x540`, a
/// store each.
pub fn settle(m: &mut impl Wrecks) {
    if w(m, 0x684) == 0 || w(m, 0x680) == 0 {
        return;
    }
    m.out(Out::Stance(-1));
    for o in [0x1744, 0x1748, 0x704, 0x708, 0x70c, 0x710] {
        put(m, o, 0);
    }
    let player = w(m, 0x680);
    let mut l = m.word(player.wrapping_add(0x1d4));
    while l != 0 {
        m.out(Out::Told(l));
        l = m.word(l.wrapping_add(0x1c));
    }
    put(m, 0x1754, 0);
    put(m, 0x1758, 0);
}

/// The event post with the bike's word `+0xd20` after the id.
fn post(m: &mut impl Wrecks, id: u32) {
    let arg = w(m, 0xd20);
    m.out(Out::Post { id, arg });
}

/// Whether the bike's size word (`+0xbbc`) is one the body's damping is handed for: neither 0 nor 1.
fn damps(m: &mut impl Wrecks) -> bool {
    let s = w(m, 0xbbc);
    s != 1 && s != 0
}

/// **The wreck's entry** (vt `+0x368`, and its base class's), reason `r`.
pub fn enter(m: &mut impl Wrecks, r: u32) {
    m.out(Out::Settle);
    m.out(Out::Stance(-1));
    if r == OUT_OF_BOUNDS {
        for k in 0..3 {
            let x = m.out(Out::Rand) as i32;
            let v = fpu::mul(f32::from_bits(0x3000_0000), fpu::from_i32(x));
            let body = w(m, BODY);
            m.set(body + 0x1d0 + 4 * k, v.to_bits());
        }
    }
    m.out(Out::Part(false));
    m.out(Out::ToWreck);
    put(m, 0xc54, 0);
    put(m, 0xc50, 0);
    put(m, 0xde8, 0);
    put(m, 0x16d4, 1);
    // The base.
    put(m, REASON, r);
    put(m, CLOCK, 0);
    put(m, COUNTING, 0);
    let state = w(m, STATE);
    if state != 0 && m.word(state + 0x28) == 1 && m.word(state + 0x5c) != 0 {
        let at = m.word(state + 0x6ac);
        if m.out(Out::RumbleReady(at)) != 0 {
            let state = w(m, STATE);
            let rumbler = m.word(state + 0x5c);
            m.out(Out::Rumble(rumbler));
        }
    }
    if damps(m) {
        let b = m.bike();
        let (a, s) = (quad(m, b + 0xcb0), quad(m, b + 0xcc0));
        let q = a.merge(Dest::XYZ, a.mul(s));
        m.out(Out::BodyAt { slot: 0x20, at: b + 0xca0 });
        m.out(Out::BodyAt { slot: 0x68, at: b + 0xca0 });
        m.out(Out::BodyQuad { slot: 0x60, q });
    }
    post(m, 11);
    put(m, 0x1784, u32::from(r != 0));
    let n = w(m, 0x6e4).wrapping_add(1);
    put(m, 0x6e4, n);
}

/// **The unwreck** (vt `+0x36c`, and its base class's), `kind` handed to the
/// put-back.
pub fn unwreck(m: &mut impl Wrecks, kind: u32) {
    m.out(Out::Settle);
    let b = m.bike();
    copy_quad(m, b + 0x1250, ORIGIN);
    let body = w(m, BODY);
    copy_quad(m, body + 0xc0, ORIGIN);
    m.out(Out::Stance(-1));
    put(m, 0x1524, 255);
    put(m, 0x1528, 255);
    put(m, 0x13e4, 1);
    for o in [0x1650, 0x119c, 0x11a0, 0x1194] {
        put(m, o, 0);
    }
    m.out(Out::ToRide);
    m.out(Out::PutBack { snap: 0, kind });
    let engine = w(m, 0x142c);
    m.out(Out::Engine(engine));
    let run = w(m, RUN);
    if run != 0 {
        let x = m.word(run + 8);
        m.set(run + 8, x | RIDING_BIT);
    }
    put(m, 0x1784, 0);
    let g = w(m, 0x1778);
    if g != 0 {
        let h = m.word(g + 0x34);
        if h != 0 {
            let rec = w(m, RECORD);
            m.set(h + 0x204, rec.wrapping_add(0x34));
        }
    }
    // The base.
    if w(m, REASON) == OUT_OF_BOUNDS {
        let p = w(m, 0xe8);
        let x = fpu::div(f(m, p + 4).0, f(m, THROW_SCALE).0);
        m.set(p + 4, x.to_bits());
    }
    put(m, REASON, 0);
    put(m, CLOCK, 0);
    if damps(m) {
        m.out(Out::BodyAt { slot: 0x20, at: REST_DAMPING });
        m.out(Out::BodyAt { slot: 0x68, at: REST_DAMPING });
        m.out(Out::BodyAt { slot: 0x60, at: b + 0xcc0 });
    }
    post(m, 13);
}

/// **The riding part on or off** (vt `+0x398`): the part at `+0xd0`. On, with the
/// bike's byte `+0x44` bit 0: the part's on, then its attach while its `+0x20` is 0. Off: its node
/// unhooked, then the part's off.
pub fn part(m: &mut impl Wrecks, on: bool) {
    if on {
        let b = m.bike();
        if byte(m, b + 0x44) & 1 == 0 || w(m, 0xd0) == 0 {
            return;
        }
        let p = w(m, 0xd0);
        m.out(Out::PartOn(p));
        let p = w(m, 0xd0);
        if m.word(p + 0x20) == 0 {
            let (a, c) = (w(m, 0xb4), w(m, 0xc0));
            m.out(Out::Attach { part: p, a, b: c });
        }
    } else {
        let p = w(m, 0xd0);
        if p == 0 {
            return;
        }
        m.out(Out::Unhook(p + 0x18));
        let p = w(m, 0xd0);
        m.out(Out::PartOff(p));
    }
}

/// **The record's part swapped** (vt `+0x48c` to the wrecked one, `+0x490`
/// back): the record's words (`+0xcc`, `+0xd8`, the record's `+0x200`) to the new part, the
/// old one off and unhooked, the new one on and attached unless its `+0x20` is set. The record
/// itself (`+0xc8`) stays.
pub fn swap(m: &mut impl Wrecks, to_wreck: bool) {
    let (new, old) = if to_wreck { (WRECK_PART, RIDE_PART) } else { (RIDE_PART, WRECK_PART) };
    let p = w(m, new);
    put(m, 0xcc, p);
    let p = w(m, new);
    put(m, 0xd8, p);
    let p = w(m, new);
    let rec = w(m, RECORD);
    m.set(rec + 0x200, p);
    let o = w(m, old);
    m.out(Out::PartOff(o));
    let o = w(m, old);
    m.out(Out::Unhook(o + 0x18));
    let p = w(m, new);
    if m.word(p + 0x20) == 0 {
        m.out(Out::PartOn(p));
        let p = w(m, new);
        let (a, c) = (w(m, 0xb4), w(m, 0xc0));
        m.out(Out::Attach { part: p, a, b: c });
    }
}

/// **The put-back** (vt `+0x3a8`, and its base class's): the bike to its spot
/// (`+0xd40` position, `+0xd50`, `+0xd60` the up; with `snap` and a terrain, lifted 0.3 ft along the
/// terrain's normal there, which is the up), the run's spot from `+0xd70` and later `+0x1738`, the
/// wheels, rider and bones reset, on the ground and not airborne, the crossup to 0.5. `kind` set, the
/// bike's `+0xde8` is `+0xdec` and vt `+0x3a0` runs, else 0 and `+0x39c`.
pub fn put_back(m: &mut impl Wrecks, snap: u32, kind: u32) {
    let b = m.bike();
    for o in [0x17b0, 0x17b4] {
        let wheel = w(m, o);
        m.out(Out::Wheel(wheel));
    }
    let rider = w(m, 0x1480);
    m.out(Out::Unhook(rider));
    let mut held = false;
    let flags = byte(m, b + 0x45);
    if flags & 2 != 0 {
        set_byte(m, b + 0x45, flags & !2);
        held = true;
    }
    // The base.
    copy_quad(m, b + 0x50, b + 0xd40);
    let run = w(m, RUN);
    if run != 0 {
        let at = w(m, 0xd70);
        if at != 0 && w(m, 0xd74) != 0 {
            m.set(run + 0x94, at);
            let x = w(m, 0xd74);
            m.set(run + 0x98, x);
            let x = w(m, 0xd78);
            m.set(run + 0x9c, x);
        }
    }
    let terrain = w(m, 0x3b4);
    if snap != 0 && terrain != 0 {
        let n = m.normal(terrain, b + 0x50);
        let lift = n.merge(Dest::XYZ, n.mul_q(f32::from_bits(0x3e99_999a)));
        let at = quad(m, b + 0x50);
        set_quad(m, b + 0x50, at.merge(Dest::XYZ, at.add(lift)));
        copy_quad(m, b + 0x60, b + 0xd50);
        set_quad(m, b + 0x70, n);
    } else {
        copy_quad(m, b + 0x60, b + 0xd50);
        copy_quad(m, b + 0x70, b + 0xd60);
    }
    m.out(Out::Ground);
    put(m, 0x31c, 1);
    if kind != 0 {
        let x = w(m, 0xdec);
        put(m, 0xde8, x);
        m.out(Out::Kind(true));
    } else {
        put(m, 0xde8, 0);
        m.out(Out::Kind(false));
    }
    let trail = w(m, 0x10dc);
    if trail != 0 {
        m.out(Out::Trail(trail));
    }
    post(m, 21);
    if held {
        let flags = byte(m, b + 0x45);
        set_byte(m, b + 0x45, flags & !2 | 2);
    }
    let (a, c) = (w(m, 0x1480), w(m, 0x1284));
    m.out(Out::Rider { a, b: c });
    for (o, buf) in [(0x1104, 0), (0x1108, 1)] {
        let bone = w(m, o);
        m.out(Out::Pose { bone, buf });
    }
    let mut i = 0;
    while (i as i32) < w(m, 0x1470) as i32 {
        let kinds = w(m, 0x1478);
        let k = m.word(kinds.wrapping_add(4 * i));
        if k == 0 || k == 1 {
            let parts = w(m, 0x1474);
            let part = m.word(parts.wrapping_add(4 * i));
            m.out(Out::Place { part, buf: k });
        }
        i += 1;
    }
    put(m, 0x324, 1);
    put(m, 0xde0, 0);
    put(m, 0x141c, 0x3f00_0000);
    put(m, 0x330, 0);
    put(m, 0x31c, 1);
    m.out(Out::Sync);
    let run = w(m, RUN);
    if run != 0 {
        let at = w(m, 0x1738);
        if at != 0 {
            m.set(run + 0x94, at);
            let x = w(m, 0x173c);
            m.set(run + 0x98, x);
            let x = w(m, 0x1740);
            m.set(run + 0x9c, x);
        }
    }
    m.out(Out::Camera(b + 0x43c));
}

/// The wheel's rest frame the wheel reset puts back on its four frames that are not the origin
/// (0, 1, 0, 1): rodata.
pub const WHEEL_REST: u32 = 0x0039_7000;

/// **The engine's reset** ([`Out::Engine`], on the engine): the revs to idle (`+0xa0` from `+0x54`),
/// `+0xa4` and `+0xa8` zero, then [`engine_gear`] 1.
pub fn engine_reset(m: &mut impl Wrecks, engine: u32) {
    let idle = m.word(engine + 0x54);
    m.set(engine + 0xa0, idle);
    m.set(engine + 0xa4, 0);
    m.set(engine + 0xa8, 0);
    engine_gear(m, engine, 1);
}

/// The engine put in gear `g`: for `0 < g <= gears` (`+0x6c`, signed), the gear (`+0x5c`), the
/// overall ratio (`+0x74`: the final drive `+0x34` times the gear's ratio in the table at `+0x70`)
/// and its reciprocal (`+0x78`). Out of range it does nothing. [`crate::r#move::set_gear`] on
/// the engine's words.
pub fn engine_gear(m: &mut impl Wrecks, engine: u32, g: i32) {
    if g <= 0 || (m.word(engine + 0x6c) as i32) < g {
        return;
    }
    m.set(engine + 0x5c, g as u32);
    let table = m.word(engine + 0x70);
    let drive = f(m, engine + 0x34);
    let gear = m.word(engine + 0x5c);
    let r = drive * f(m, (gear << 2).wrapping_add(table).wrapping_sub(4));
    m.set(engine + 0x74, r.to_bits());
    m.set(engine + 0x78, (F::ONE / r).to_bits());
}

/// **A wheel's reset** ([`Out::Wheel`], on the wheel): its ray, touch and roughness words zero
/// (`+0xd0`, `+0xd4`, `+0x1d4`, `+0x224`, `+0x228`, `+0x210`, `+0xf0`, `+0xf8`), six frames to the
/// origin (`+0x140`, `+0x170`, `+0x180`, `+0x190`, `+0x840`, `+0x850`) and two to [`WHEEL_REST`]
/// (`+0x130`, `+0x200`).
pub fn wheel_reset(m: &mut impl Wrecks, wheel: u32) {
    for o in [0xd0, 0xd4, 0x1d4, 0x224, 0x228, 0x210, 0xf0, 0xf8] {
        m.set(wheel + o, 0);
    }
    for o in [0x140, 0x170, 0x180, 0x190, 0x840, 0x850] {
        copy_quad(m, wheel + o, ORIGIN);
    }
    for o in [0x130, 0x200] {
        copy_quad(m, wheel + o, WHEEL_REST);
    }
}
