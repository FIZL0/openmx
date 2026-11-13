//! **The part walk's first particle emitter kind** (F57): the eleven listeners state 4's part walk
//! ([`super::tail::walk`]) steps with the emitter's thunk (each at its object's `+0xc`), as a port on
//! RAM. Where the original keeps it: research/oracle.txt `# F57`.
//!
//! Each frame, [`step`]:
//!
//! 1. runs the emitter's [`gate`]: brings its spot (`+0x30`, last frame's at `+0x40`) to its node, if
//!    it has one (`+0x80`), and runs its clocks; a non-zero answer ends the frame;
//! 2. with its definition's (`+0xd8`) bit [`DEF_OTHER`] set, does the other kind's work instead
//!    ([`Call::Other`], not ported: F57a);
//! 3. from the spot's move over `dt` (its speed) and a draw of `rand`, takes a number of particles to
//!    spawn ([`count`]), and from a second draw a walk through the spread table ([`SPREAD`]): a start
//!    and a step of 1 to 3 entries, either way;
//! 4. spawns them into its definition's pool (`*def`: count `+0x108`, room `+0x110`, the particles'
//!    pointers at `+0x118`), each placed along the spot's move, given an offset and a push off the
//!    table's next three entries (turned into the emitter's frame or by its turn), its look
//!    ([`look`]), size and life ([`size`]), spin, fade and flags from the entries after.
//!
//! The calls it makes that are not the emitter's own work are handed out ([`Call`]): `rand`, the
//! object's lighting virtuals, and the ones no C15 frame reaches.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use super::node::{self, Ram};
use super::update::Quad;
use ps2_float::vu::{self, Field};
use ps2_float::{F, Vf, fpu, fpu::Acc};

/// The emitter's words: its spot (`+0x30`) and last frame's (`+0x40`), the spot's offset from its
/// node (`+0x50`), the base's two parts (`+0x60`, `+0x70`), its node (`+0x80`), its turn (`+0x90`,
/// a quaternion), its frame (`+0xa0`, a pointer to three rows), the particles' look byte (`+0xa4`),
/// its age (`+0xac`), flag bytes (`+0xb0`, `+0xb1`), hold and mode clocks (`+0xb8`, `+0xbc`), its
/// rate (`+0xc4`), its bind (`+0xc8`), definition (`+0xd8`), carried fraction (`+0xe0`), its life
/// and size by speed (`+0xe4`, `+0xe8`), its count by distance and speed (`+0xec`, `+0xf0`) and its
/// two speeds: below the first nothing spawns (`+0xf4`), above the second it is fast (`+0xf8`).
pub const SPOT: u32 = 0x30;
pub const LAST: u32 = 0x40;
pub const OFFSET: u32 = 0x50;
pub const BASE: u32 = 0x60;
pub const BASE_ADD: u32 = 0x70;
pub const NODE: u32 = 0x80;
pub const TURN: u32 = 0x90;
pub const FRAME: u32 = 0xa0;
pub const LOOK: u32 = 0xa4;
pub const AGE: u32 = 0xac;
pub const FLAGS: u32 = 0xb0;
pub const FLAGS2: u32 = 0xb1;
pub const HOLD: u32 = 0xb8;
pub const WAIT: u32 = 0xbc;
pub const RATE: u32 = 0xc4;
pub const BIND: u32 = 0xc8;
pub const DEF: u32 = 0xd8;
pub const CARRY: u32 = 0xe0;
pub const LIFE_BY_SPEED: u32 = 0xe4;
pub const SIZE_BY_SPEED: u32 = 0xe8;
pub const COUNT_BY_DISTANCE: u32 = 0xec;
pub const COUNT_BY_SPEED: u32 = 0xf0;
pub const SLOWEST: u32 = 0xf4;
pub const FAST_ABOVE: u32 = 0xf8;
/// The flags byte's bits: skip a frame (cleared as read), on (the hold clock runs, the mode clocks
/// douse), spawned (set by a frame that spawns, cleared by one that idles), the spot's offset turned
/// by the node, in its frame, the base's add on the base (not on the velocity), turned, lit.
pub const SKIP: u8 = 0x01;
pub const ON: u8 = 0x02;
pub const SPAWNED: u8 = 0x04;
pub const IN_FRAME: u8 = 0x08;
pub const OFFSET_TURNED: u8 = 0x10;
pub const ADD_ON_BASE: u8 = 0x20;
pub const TURNED: u8 = 0x40;
pub const LIT: u8 = 0x80;
/// The definition's words this kind reads: its pool (`+0`), flags (`+0x28`, also the particles'),
/// mode (`+0x58`: 1, 2 and 3 run clocks that light and douse), and its option byte (`+0x4c`).
pub const POOL: u32 = 0;
pub const DEF_FLAGS: u32 = 0x28;
pub const DEF_OPTIONS: u32 = 0x4c;
pub const DEF_MODE: u32 = 0x58;
/// The option byte's bits: fade on the eighth entry, on the ninth, a velocity from the move, the
/// pool's push on the base, settle each particle ([`Call::Settle`]), the other kind.
pub const DEF_FADE_A: u8 = 0x02;
pub const DEF_FADE_B: u8 = 0x04;
pub const DEF_VELOCITY: u8 = 0x08;
pub const DEF_POOL_PUSH: u8 = 0x10;
pub const DEF_SETTLE: u8 = 0x20;
pub const DEF_OTHER: u8 = 0x40;
/// The definition flags' bits: the look by steps ([`look`]) rather than the emitter's byte, and spin.
pub const LOOK_STEPPED: u32 = 0x400;
pub const SPINS: u32 = 0x20_0000;
/// The pool's count, room and particles' array.
pub const POOL_COUNT: u32 = 0x108;
pub const POOL_ROOM: u32 = 0x110;
pub const POOL_ARRAY: u32 = 0x118;

/// The spread table: 600 floats the spawns walk through.
pub const SPREAD: u32 = 0x0054_2410;
pub const SPREAD_LEN: i32 = 600;
/// The quad the table's entries are centred on.
pub const SPREAD_CENTRE: u32 = 0x0052_5450;
/// The zero quad the velocity is without [`DEF_VELOCITY`].
pub const ZERO_QUAD: u32 = 0x0039_6fc0;
/// The game's state (a pointer: its byte `+0x225`'s bit 3 pauses every emitter), a word that holds
/// the ones with bit 0 of [`FLAGS2`], and the frame's id (a halfword) a node's owner must match.
pub const GAME: u32 = 0x003e_ab60;
pub const GAME_PAUSED: u32 = 0x225;
pub const HELD: u32 = 0x0052_53c8;
pub const FRAME_ID: u32 = 0x003d_3b90;
/// What the notice hands its listener (`+8`): the kind's word.
pub const NOTICE_KIND: u32 = 0x0052_b9e0;

/// `rand`'s scale, 2⁻³¹.
const RAND_SCALE: u32 = 0x3000_0000;
/// The second draw's steps: (below, direction, step) — at or under each bound, that walk; above the
/// last, back by three.
const WALKS: [(u32, i32, i32); 5] = [(0x3e19_999a, 1, 1), (0x3e99_999a, 2, 2), (0x3ee6_6666, 3, 3), (0x3f33_3333, -1, 2), (0x3f59_999a, -2, 2)];
const PI: u32 = 0x4049_0fd0;

/// What the emitter hands out, each the original's own (the [`Call::Rand`] answer is `rand`'s; the
/// [`Call::Other`] and [`Call::Probe`] answers are their returns; the rest answer nothing).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    /// The C library's `rand`.
    Rand,
    /// The emitter's listener (`+8`) told: `$f12` 2.0, the emitter, its spot, [`NOTICE_KIND`].
    Notice(u32),
    /// The emitter lit (vt `+0xa4`) and doused (vt `+0xa8`).
    Light(u32),
    Douse(u32),
    /// The other kind's step ([`DEF_OTHER`]): not ported (F57a).
    Other { emitter: u32, dt: F },
    /// With the definition's `+0x168` above 0: the owner (`+0xd0`) placed at `at` with the word
    /// after `+0xcc`'s `+0x20`, its own virtual (vt at its `+0xc`, `+0xc8`), the emitter's own
    /// follow-up, and a probe of the world at the spot answering a float's bits.
    Place { owner: u32, at: u32, with: u32 },
    Owner(u32),
    Follow(u32),
    Probe(u32),
    /// A particle settled ([`DEF_SETTLE`]), and one bound to the emitter (its `+0xc8` 1).
    Settle(u32),
    Bind { emitter: u32, particle: u32 },
}

/// The emitter's world: RAM (byte and halfword stores beside the words), and its calls out.
pub trait Emitters: Ram {
    fn set_byte(&mut self, at: u32, b: u8);
    fn set_half(&mut self, at: u32, h: u16);
    fn out(&mut self, c: Call) -> u32;
}

fn byte(m: &mut impl Emitters, at: u32) -> u8 {
    (m.word(at & !3) >> (8 * (at & 3))) as u8
}

fn half(m: &mut impl Emitters, at: u32) -> u16 {
    (m.word(at & !3) >> (8 * (at & 2))) as u16
}

fn f(m: &mut impl Emitters, at: u32) -> F {
    F::from_bits(m.word(at))
}

fn set_f(m: &mut impl Emitters, at: u32, x: F) {
    m.set(at, x.to_bits());
}

fn quad(m: &mut impl Emitters, at: u32) -> Quad {
    [0, 4, 8, 12].map(|k| m.word(at + k))
}

fn put_quad(m: &mut impl Emitters, at: u32, q: Quad) {
    for (k, w) in q.into_iter().enumerate() {
        m.set(at + 4 * k as u32, w);
    }
}

fn v(q: Quad) -> Vf {
    Vf::from_bits(q)
}

/// A `.xyz` result: `r`'s three lanes, `w` the raw word `of` holds.
fn xyz(r: Vf, of: Quad) -> Quad {
    let b = r.to_bits();
    [b[0], b[1], b[2], of[3]]
}

fn def(m: &mut impl Emitters, o: u32) -> u32 {
    m.word(o + DEF)
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

/// `rand` times 2⁻³¹ (`mul.s` of the scale by the converted draw).
fn draw(m: &mut impl Emitters) -> F {
    let r = m.out(Call::Rand) as i32;
    mul(F(f32::from_bits(RAND_SCALE)), F(fpu::from_i32(r)))
}

/// `v`'s `xyz` through the three rows at `rows` (`vmulax`, `vmadday`, `vmaddz`), `w` its own.
fn in_frame(m: &mut impl Emitters, rows: u32, q: Quad) -> Quad {
    let r = [0, 0x10, 0x20].map(|k| v(quad(m, rows + k)));
    let x = v(q);
    xyz(Vf::madd(Vf::madd(r[0].mul(x.bc(Field::X)), r[1], x.bc(Field::Y)), r[2], x.bc(Field::Z)), q)
}

/// `v` turned by the quaternion at `at` (`q v q*`, two `vopmula`/`vmadd*`/`vopmsub` rounds), `w`
/// its own.
fn turned(m: &mut impl Emitters, at: u32, x: Quad) -> Quad {
    let q = v(quad(m, at));
    let c = v(x);
    let acc = Vf::madd(Vf::opmula(q, c), c, q.bc(Field::W));
    let t = Vf::opmsub(acc, c, q);
    let p = q.mul(c);
    let acc = Vf::madd(Vf::madd(Vf::madd(Vf::opmula(q, t), q, p.bc(Field::X)), q, p.bc(Field::Y)), q, p.bc(Field::Z));
    let acc = Vf::madd(acc, t, q.bc(Field::W));
    xyz(Vf::opmsub(acc, t, q), x)
}

/// Last frame's spot kept, the age run on and the wait flag cleared: what a frame that spawns
/// nothing does. Answers 1.
fn idle(m: &mut impl Emitters, o: u32, dt: F) -> u32 {
    let age = add(f(m, o + AGE), dt);
    set_f(m, o + AGE, age);
    let spot = quad(m, o + SPOT);
    put_quad(m, o + LAST, spot);
    let b = byte(m, o + FLAGS);
    m.set_byte(o + FLAGS, b & !SPAWNED);
    1
}

/// The mode clock ([`WAIT`]) run down by `dt`: `None` while it was already out, else whether it ran out.
fn wait(m: &mut impl Emitters, o: u32, dt: F) -> Option<bool> {
    let w = f(m, o + WAIT);
    if w <= 0.0 {
        return None;
    }
    let w = sub(w, dt);
    set_f(m, o + WAIT, w);
    Some(w <= 0.0)
}

/// **The emitter's gate** (its first call each frame): the spot brought to the node, the clocks run.
/// Answers 0 when the emitter spawns this frame.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn gate(m: &mut impl Emitters, o: u32, dt: F) -> u32 {
    let game = m.word(GAME);
    if byte(m, game + GAME_PAUSED) & 0x08 != 0 {
        return 1;
    }
    let n = m.word(o + NODE);
    if n != 0 {
        let spot = quad(m, o + SPOT);
        put_quad(m, o + LAST, spot);
        node::position(m, n, o + SPOT);
        let off = if byte(m, o + FLAGS) & OFFSET_TURNED != 0 {
            let q = quad(m, o + OFFSET);
            let n = m.word(o + NODE);
            node::rotate(m, n, q)
        } else {
            quad(m, o + OFFSET)
        };
        let spot = quad(m, o + SPOT);
        put_quad(m, o + SPOT, xyz(v(spot).add(v(off)), spot));
    }
    if m.word(o + 8) != 0 {
        m.out(Call::Notice(o));
    }
    let b = byte(m, o + FLAGS);
    if b & SKIP != 0 {
        m.set_byte(o + FLAGS, b & !SKIP);
        let spot = quad(m, o + SPOT);
        put_quad(m, o + LAST, spot);
        return 1;
    }
    let hold = f(m, o + HOLD);
    if !(hold <= 0.0) {
        if b & ON != 0 {
            set_f(m, o + HOLD, sub(hold, dt));
        }
        return 1;
    }
    if byte(m, o + FLAGS2) & 1 != 0 && m.word(HELD) != 0 {
        return 1;
    }
    let d = def(m, o);
    let mode = m.word(d + DEF_MODE);
    let holding = byte(m, o + FLAGS) & ON != 0;
    match mode {
        1 if holding => {
            let w = f(m, o + WAIT);
            if !(w <= 0.0) {
                set_f(m, o + WAIT, sub(w, dt));
                return 0;
            }
            if byte(m, o + FLAGS) & LIT != 0 {
                m.out(Call::Douse(o));
                return 1;
            }
        }
        1 => return idle(m, o, dt),
        2 | 3 => {
            let (on, off) = if mode == 2 { (0x60, 0x68) } else { (0x64, 0x6c) };
            match (holding, wait(m, o, dt)) {
                (_, None) => {}
                (true, Some(false)) => return 0,
                (false, Some(false)) => return idle(m, o, dt),
                (true, Some(true)) => {
                    let w = if mode == 2 { span(m, o, off) } else { fixed(m, o, off) };
                    set_f(m, o + WAIT, w);
                    m.out(Call::Douse(o));
                    return 1;
                }
                (false, Some(true)) => {
                    let w = if mode == 2 { span(m, o, on) } else { fixed(m, o, on) };
                    set_f(m, o + WAIT, w);
                    m.out(Call::Light(o));
                    m.set(o + AGE, 0);
                    return 0;
                }
            }
        }
        _ => {}
    }
    let b = byte(m, o + FLAGS);
    if b & ON == 0 {
        return idle(m, o, dt);
    }
    if b & SPAWNED == 0 {
        m.set_byte(o + FLAGS, b | SPAWNED);
    }
    0
}

/// A clock's next span, the definition's word at `at`.
fn fixed(m: &mut impl Emitters, o: u32, at: u32) -> F {
    let d = def(m, o);
    f(m, d + at)
}

/// A clock's next span: a `rand` draw between the definition's words at `at` and `at + 4`.
fn span(m: &mut impl Emitters, o: u32, at: u32) -> F {
    let u = draw(m);
    let d = def(m, o);
    let (lo, hi) = (f(m, d + at), f(m, d + at + 4));
    Acc::adda(0.0, lo).madd(sub(hi, lo), u)
}

/// **How many particles a frame spawns**: by the emitter's distance and speed when fast (or by
/// the draw `u` between the definition's two counts per second), times its rate, plus the fraction
/// carried from last frame; the whole part (no less than the definition's `+0x24`) spawns.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn count(m: &mut impl Emitters, o: u32, fast: bool, dt: F, dist: F, speed: F, u: F) -> i32 {
    let d = def(m, o);
    let lo = mul(f(m, d + 0x110), dt);
    let hi = mul(f(m, d + 0x114), dt);
    let by_dist = f(m, o + COUNT_BY_DISTANCE);
    let by_speed = f(m, o + COUNT_BY_SPEED);
    let mut c = if fast && (!(by_dist <= 0.0) || !(by_speed <= 0.0)) {
        let c = Acc::mula(by_speed, speed).madd(by_dist, dist);
        if c < lo {
            lo
        } else if c <= hi {
            c
        } else {
            hi
        }
    } else {
        Acc::adda(0.0, lo).madd(u, sub(hi, lo))
    };
    c = mul(c, f(m, o + RATE));
    if byte(m, d + DEF_OPTIONS) & DEF_OTHER != 0 {
        let pool = m.word(d + POOL);
        let s = m.word(pool + 0x174);
        if s != 0 {
            c = mul(c, f(m, s + 0x84));
        }
    }
    c = add(c, f(m, o + CARRY));
    let n = if c <= 1.0 {
        set_f(m, o + CARRY, c);
        0
    } else {
        let n = fpu::to_i32(c.0);
        set_f(m, o + CARRY, sub(c, F(fpu::from_i32(n))));
        n
    };
    let least = f(m, d + 0x24);
    fpu::to_i32(fpu::max(least.0, fpu::from_i32(n)))
}

/// **The particles' base velocity**: the base (`+0x60`) and its add (`+0x70`, on the base or on
/// `vel` by [`ADD_ON_BASE`]) in the emitter's frame added to `vel`, or turned (into `vel`), or as
/// they are added to `vel`. `vel` is changed as the original changes it.
pub fn base(m: &mut impl Emitters, o: u32, vel: &mut Quad) -> Quad {
    let b = byte(m, o + FLAGS);
    if b & (IN_FRAME | TURNED) != 0 {
        let mut t = quad(m, o + BASE);
        let add = quad(m, o + BASE_ADD);
        if byte(m, o + FLAGS) & ADD_ON_BASE != 0 {
            t = xyz(v(t).add(v(add)), t);
        } else {
            *vel = xyz(v(*vel).add(v(add)), *vel);
        }
        if b & IN_FRAME != 0 {
            let rows = m.word(o + FRAME);
            let t = in_frame(m, rows, t);
            *vel = xyz(v(*vel).add(v(t)), *vel);
        } else {
            *vel = turned(m, o + TURN, t);
        }
    } else {
        let t = quad(m, o + BASE);
        let s = xyz(v(t).add(v(quad(m, o + BASE_ADD))), t);
        *vel = xyz(v(*vel).add(v(s)), *vel);
    }
    *vel
}

/// **A particle's look** (its `+0x26`): with [`LOOK_STEPPED`] the definition's first look (`+0x38`)
/// plus its step (`+0x44`) times how many of its spans (`+0xb8`) it takes to pass the entry `t`;
/// else the emitter's look byte.
pub fn look(m: &mut impl Emitters, o: u32, t: F, stepped: bool) -> u32 {
    if !stepped {
        return u32::from(byte(m, o + LOOK));
    }
    let d = def(m, o);
    let s = f(m, d + 0xb8);
    let mut n = 0u32;
    if !(t <= s) {
        let mut x = s;
        loop {
            x = add(x, s);
            n += 1;
            if t <= x {
                break;
            }
        }
    }
    m.word(d + 0x38).wrapping_add(n.wrapping_mul(m.word(d + 0x44)))
}

/// `x` held between the definition's words at `lo` and `lo + 4`.
fn clamp(m: &mut impl Emitters, d: u32, lo: u32, x: F) -> F {
    let a = f(m, d + lo);
    if x < a {
        return a;
    }
    let b = f(m, d + lo + 4);
    if x <= b { x } else { b }
}

/// **A particle's life and size** (into `life_at`, `size_at`): when fast, the emitter's by-speed
/// factors times `speed` held to the definition's ranges; else the ranges' starts plus their spans
/// times the entries `a` and `b`. The size is scaled by the pool's scale (at least a half) with
/// [`DEF_OTHER`].
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
#[allow(clippy::too_many_arguments)] // the original's
pub fn size(m: &mut impl Emitters, o: u32, life_at: u32, size_at: u32, fast: bool, speed: F, a: F, b: F) {
    let d = def(m, o);
    let by = f(m, o + LIFE_BY_SPEED);
    let life = if fast && !(by <= 0.0) {
        let x = mul(by, speed);
        clamp(m, d, 0xbc, x)
    } else {
        Acc::adda(0.0, f(m, d + 0xbc)).madd(f(m, d + 0xc4), a)
    };
    let by = f(m, o + SIZE_BY_SPEED);
    let mut size = if fast && !(by <= 0.0) {
        let x = mul(by, speed);
        clamp(m, d, 0x100, x)
    } else {
        Acc::adda(0.0, f(m, d + 0x100)).madd(f(m, d + 0x108), b)
    };
    if byte(m, d + DEF_OPTIONS) & DEF_OTHER != 0 {
        let pool = m.word(d + POOL);
        let s = m.word(pool + 0x174);
        if s != 0 {
            let k = mul(f(m, o + RATE), f(m, s + 0x84));
            size = mul(size, F(fpu::max(0.5, k.0)));
        }
    }
    set_f(m, life_at, life);
    set_f(m, size_at, size);
}

fn entry(m: &mut impl Emitters, i: i32) -> F {
    f(m, SPREAD.wrapping_add((i as u32).wrapping_mul(4)))
}

/// **The emitter's step** for a frame of `dt` (the thunk's target, on the object `o`): the
/// [`gate`], then the spawns. Answers what the original answers (1, or [`Call::Other`]'s).
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn step(m: &mut impl Emitters, o: u32, dt: F) -> u32 {
    if gate(m, o, dt) != 0 {
        return 1;
    }
    let d = def(m, o);
    if byte(m, d + DEF_OPTIONS) & DEF_OTHER != 0 {
        return m.out(Call::Other { emitter: o, dt });
    }
    let n = m.word(o + NODE);
    if n != 0 {
        let owner = m.word(n + 8);
        if owner != 0 {
            let at = m.word(owner + 4) + 0xa4;
            if half(m, FRAME_ID) != half(m, at) {
                return 1;
            }
        }
    }
    let spot = quad(m, o + SPOT);
    let moved = xyz(v(spot).sub(v(quad(m, o + LAST))), spot);
    let (x, y, z) = (F::from_bits(moved[0]), F::from_bits(moved[1]), F::from_bits(moved[2]));
    let dist = F(fpu::sqrt(Acc::mula(x, x).madda(y, y).madd(z, z).0));
    let speed = div(dist, dt);
    let d = def(m, o);
    let mut vel = if byte(m, d + DEF_OPTIONS) & DEF_VELOCITY != 0 {
        let q = vu::div(1.0, dt.0);
        xyz(v(moved).mul_q(q), moved)
    } else {
        quad(m, ZERO_QUAD)
    };
    let fast = !(speed <= f(m, o + FAST_ABOVE));
    if speed < f(m, o + SLOWEST) {
        return 1;
    }
    let u = draw(m);
    let count = count(m, o, fast, dt, dist, speed, u);
    let u2 = draw(m);
    let (mut dir, mut stride) = WALKS.iter().find(|w| u2 <= f32::from_bits(w.0)).map_or((-3, 3), |w| (w.1, w.2));
    if byte(m, o + FLAGS) & IN_FRAME != 0 {
        let n = m.word(o + NODE);
        if n != 0 {
            node::update(m, n);
        }
    }
    if count <= 0 {
        return 1;
    }
    let d = def(m, o);
    if m.word(d + DEF_MODE) == 1 {
        let b = byte(m, o + FLAGS);
        m.set_byte(o + FLAGS, (b & !LIT) | LIT);
    }
    let mut base = base(m, o, &mut vel);
    let d = def(m, o);
    if byte(m, d + DEF_OPTIONS) & DEF_POOL_PUSH != 0 {
        let pool = m.word(d + POOL);
        base = xyz(v(base).add(v(quad(m, pool + 0x140))), base);
    }
    let mut pos = quad(m, o + SPOT);
    let q = vu::div(1.0, fpu::from_i32(count));
    let step = xyz(v(moved).mul_q(q), moved);
    let d = def(m, o);
    let mut idx = fpu::to_i32(fpu::mul(u.0, fpu::from_i32(599 - 13 * stride)));
    let stepped = m.word(d + DEF_FLAGS) & LOOK_STEPPED != 0;
    if m.word(d + 0x168) as i32 > 0 {
        place(m, o);
    }
    for _ in 0..count {
        let d = def(m, o);
        let pool = m.word(d + POOL);
        let k = m.word(pool + POOL_COUNT);
        if !((k as i32) < m.word(pool + POOL_ROOM) as i32) {
            return 1;
        }
        if (dir > 0 && idx + 13 * stride >= SPREAD_LEN) || (dir < 0 && idx < 13 * stride) {
            dir = -dir;
            if dir >= 2 {
                dir -= 1;
                stride -= 1;
            } else if dir < -1 {
                dir += 1;
                stride += 1;
            }
        }
        let array = m.word(pool + POOL_ARRAY);
        let p = m.word(array.wrapping_add(k.wrapping_mul(4)));
        let at = |i: i32| SPREAD.wrapping_add((i as u32).wrapping_mul(4));
        let raw = [m.word(at(idx + dir)), m.word(at(idx + 2 * dir)), m.word(at(idx + 3 * dir)), 1.0f32.to_bits()];
        idx += 3 * dir;
        let c = quad(m, SPREAD_CENTRE);
        let off = xyz(Vf::from_bits(raw).sub(v(c)), raw);
        let o3 = v(off);
        let most = fpu::max(fpu::max(fpu::abs(o3.x()), fpu::abs(o3.y())), fpu::abs(o3.z()));
        let sq = o3.mul(o3);
        let s = vu::madd(vu::add(sq.x(), sq.y()), 1.0, sq.z());
        let q = vu::rsqrt(1.0, s);
        let spread = xyz(o3.mul_q(q), off);
        let spread = xyz(v(spread).mul(Vf::splat(most)), spread);
        // the offset: the definition's `+0xd0`..`+0xe0` box's middle plus its `+0xf0` by the spread
        let d = def(m, o);
        let r = quad(m, d + 0xf0);
        let along = xyz(v(r).mul(v(spread)), r);
        let hi = quad(m, d + 0xe0);
        let mid = xyz(v(hi).add(v(quad(m, d + 0xd0))), hi);
        let mid = xyz(v(mid).mul(Vf::splat(0.5)), mid);
        let mut offset = xyz(v(mid).add(v(along)), mid);
        // the push: `+0x130`..`+0x140`'s middle plus `+0x150` by the spread
        let r = quad(m, d + 0x150);
        let along = xyz(v(r).mul(v(spread)), r);
        let hi = quad(m, d + 0x140);
        let mid = xyz(v(hi).add(v(quad(m, d + 0x130))), hi);
        let mid = xyz(v(mid).mul(Vf::splat(0.5)), mid);
        let mut push = xyz(v(mid).add(v(along)), mid);
        let b = byte(m, o + FLAGS);
        if b & IN_FRAME != 0 {
            let rows = m.word(o + FRAME);
            push = in_frame(m, rows, push);
            let rows = m.word(o + FRAME);
            offset = in_frame(m, rows, offset);
        } else if b & TURNED != 0 {
            push = turned(m, o + TURN, push);
            offset = turned(m, o + TURN, offset);
        }
        idx += dir;
        let t = entry(m, idx);
        let lk = look(m, o, t, stepped);
        m.set_half(p + 0x26, lk as u16);
        idx += dir;
        let a = entry(m, idx);
        idx += dir;
        let b = entry(m, idx);
        size(m, o, p + 0x3c, p + 0x34, fast, speed, a, b);
        put_quad(m, p + 0x10, xyz(v(base).add(v(push)), base));
        let d = def(m, o);
        let s = mul(f(m, p + 0x34), f(m, d + 0x10c));
        set_f(m, p + 0x30, s);
        put_quad(m, p, xyz(v(pos).add(v(offset)), pos));
        idx += dir;
        let life = m.word(p + 0x3c);
        m.set(p + 0x40, life);
        m.set(p + 0x54, o);
        let r = div(F(1.0), f(m, p + 0x34));
        set_f(m, p + 0x38, r);
        let d = def(m, o);
        let flags = m.word(d + DEF_FLAGS);
        m.set(p + 0x48, flags);
        let t = entry(m, idx);
        let d = def(m, o);
        let fade = Acc::adda(0.0, f(m, d + 0x118)).madd(f(m, d + 0x120), t);
        set_f(m, p + 0x44, fade);
        if fade == 0.0 {
            let w = m.word(p + 0x48);
            m.set(p + 0x48, w & !8);
        }
        let d = def(m, o);
        if byte(m, d + DEF_OPTIONS) & DEF_SETTLE != 0 {
            m.out(Call::Settle(p));
            let y = add(f(m, p + 4), F::from_bits(offset[1]));
            set_f(m, p + 4, y);
        }
        let d = def(m, o);
        if m.word(d + DEF_FLAGS) & SPINS != 0 {
            let spin = m.word(d + 0xa0);
            m.set(p + 0x4c, spin);
            let t = entry(m, idx);
            let d = def(m, o);
            let r = div(mul(F(f32::from_bits(PI)), mul(f(m, d + 0xa4), t)), F(180.0));
            set_f(m, p + 0x50, r);
        } else {
            m.set(p + 0x4c, 0);
            m.set(p + 0x50, 0);
        }
        if m.word(o + BIND) == 1 {
            m.out(Call::Bind { emitter: o, particle: p });
        }
        idx += dir;
        let fade_a = !(entry(m, idx) <= 0.5);
        idx += dir;
        let fade_b = !(entry(m, idx) <= 0.5);
        let d = def(m, o);
        let w = m.word(p + 0x48);
        if fade_a && byte(m, d + DEF_OPTIONS) & DEF_FADE_A != 0 {
            m.set(p + 0x48, w | 0x4000);
        } else {
            m.set(p + 0x48, w & !0x4000);
        }
        let d = def(m, o);
        let w = m.word(p + 0x48);
        if fade_b && byte(m, d + DEF_OPTIONS) & DEF_FADE_B != 0 {
            m.set(p + 0x48, w | 0x8000);
        } else {
            m.set(p + 0x48, w & !0x8000);
        }
        let d = def(m, o);
        let pool = m.word(d + POOL);
        let k = m.word(pool + POOL_COUNT);
        m.set(pool + POOL_COUNT, k.wrapping_add(1));
        pos = xyz(v(pos).sub(v(step)), pos);
        if !stepped {
            idx += dir;
            let t = entry(m, idx);
            let d = def(m, o);
            let (lo, hi) = (m.word(d + 0x38), m.word(d + 0x3c));
            let span = fpu::from_i32(hi.wrapping_sub(lo) as i32);
            let k = fpu::to_i32(fpu::mul(span, t.0));
            m.set_byte(o + LOOK, lo.wrapping_add(k as u32) as u8);
        }
    }
    1
}

/// With the definition's `+0x168` above 0, before the spawns: the owner placed and stepped, the
/// emitter's follow-up, and with a limit at the definition's `+0x160`, the bind (`+0xc8`) set by a
/// probe of the world at the spot. All handed out; no C15 frame reaches it.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
fn place(m: &mut impl Emitters, o: u32) {
    let owner = m.word(o + 0xd0);
    let with = m.word(o + 0xcc).wrapping_add(0x20);
    m.out(Call::Place { owner, at: o + SPOT, with });
    let owner = m.word(o + 0xd0);
    m.out(Call::Owner(owner));
    m.out(Call::Follow(o));
    let d = def(m, o);
    let limit = f(m, d + 0x160);
    if limit == 0.0 {
        return;
    }
    let got = F::from_bits(m.out(Call::Probe(o + SPOT)));
    let d = def(m, o);
    let mode = m.word(d + 0x168);
    if got <= f(m, d + 0x160) {
        m.set(o + BIND, mode);
    } else if mode == 2 {
        m.set(o + BIND, 1);
    } else if mode == 1 {
        m.set(o + BIND, 0);
    }
}
