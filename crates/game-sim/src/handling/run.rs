//! **The player's run** (F31b5h): the scoring the bike's stunt start and trick end
//! ([`super::update::stunt_start`], [`super::update::trick_end`]) hand out as
//! [`super::update::Out::RunOpen`] and the rest, and the listeners state 4's run tells. Where the
//! original keeps them and their calls: research/oracle.txt `# F31b5h`.
//!
//! A run keeps a free list of stunt nodes (`+0x3c`), the group it tells (`+0x24`: the group itself,
//! then its members from the group's `+4`, next at `+0x18`, each with its vtable at `+0x1c`), its
//! flags (`+0x50`: bit 0 scoring, bit 3 held off, bit 15 a history kept), the score's step
//! (`+0x2a8`) and the history's length (`+0x300`, negative for none). A player keeps its flags as a
//! doubleword (`+8`: bit 0 out, bit 1 in, bit 2 the run's kind, bit 5 the run open, bit 6 a stunt
//! running), its watcher (`+4`) and its list of watchers (`+0x1d4`, next at `+0x1c`, each with its
//! vtable at `+0x20`), its stunts (`+0xc8`, next at `+0x24`, counted at `+0xcc`), the history
//! (`+0xd0`, counted at `+0xd4`), the run's score (`+0xdc`) and the stunts' sum (`+0xe0`).
//!
//! A stunt node is ten words: `+0` flags (bit 0 scored, bit 1 rounded to its step, bit 2 floored to
//! it, bit 4 running, bit 5 marked), `+4` the stunt's id, `+8` its score, `+0xc` the base, `+0x10`
//! the second score, `+0x14` the multiplier, `+0x18` the base's partner, `+0x20` the step and `+0x24`
//! the next node.
//!
//! - [`open`], [`score`], [`mark`], [`close`], [`tally`] and [`end`]: the run's six calls.
//! - The listeners state 4 reaches: the bike's watcher ([`watch_open`], [`watch_score`],
//!   [`watch_end`], [`watch_tally`]: the bike's event record filled and posted, the tally's best
//!   and the stunt table's repeat scales, [`repeat`]); the group's score ([`group_score`], each
//!   gauge it holds kicked, [`kick`]) and its tally ([`group_tally`], a post). The group's open and
//!   end answer 0 and do nothing.
//!
//! Every word goes through [`Runs`]; a listener told is an [`Out::Tell`] and an event post an
//! [`Out::Post`].
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use ps2_float::{F, fpu};

/// Where a listener's vtable is: the run's group and its members', the player's watchers'.
pub const GROUP_VT: u32 = 0x1c;
pub const WATCH_VT: u32 = 0x20;

/// What a listener is told, with the arguments past the listener and the player: the run opened;
/// a stunt scored (its node); a stunt ended (`x` in `$f12`, the end's flag, the node or 0); the run
/// tallied (`x`, the tally's flag).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Heard {
    Open,
    Score(u32),
    End { x: u32, flag: u32, node: u32 },
    Tally { x: u32, flag: u32 },
}

impl Heard {
    /// The vtable slot a listener whose vtable is at `vt` ([`GROUP_VT`] or [`WATCH_VT`]) is told
    /// through.
    pub fn slot(self, vt: u32) -> u32 {
        let group = vt == GROUP_VT;
        match self {
            Heard::Open => if group { 0x3c } else { 0x40 },
            Heard::Tally { .. } => if group { 0x40 } else { 0x44 },
            Heard::Score(_) => if group { 0x44 } else { 0x48 },
            Heard::End { .. } => if group { 0x48 } else { 0x4c },
        }
    }
}

/// A call the run and its listeners make outside their words.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Out {
    /// A listener `to` (its vtable at `to + vt`) told `heard`, with the player.
    Tell { to: u32, vt: u32, player: u32, heard: Heard },
    /// An event posted to the listeners registered on `on` (the engine's event queue, its owner
    /// word the caller's): `kind`, `id`, its data word and its last word.
    Post { on: u32, kind: u32, id: u32, data: u32, x: u32 },
}

/// The run's outside world: words by address, and its calls (their answers are unread).
pub trait Runs {
    fn word(&mut self, at: u32) -> u32;
    fn set(&mut self, at: u32, w: u32);
    fn out(&mut self, c: Out);
}

fn f(m: &mut impl Runs, at: u32) -> F {
    F(f32::from_bits(m.word(at)))
}

fn setf(m: &mut impl Runs, at: u32, v: F) {
    m.set(at, v.to_bits());
}

fn byte(m: &mut impl Runs, at: u32) -> u32 {
    m.word(at & !3) >> (8 * (at & 3)) & 0xff
}

/// A stunt node's words.
const NODE_WORDS: u32 = 10;
const NEXT: u32 = 0x24;

/// The player's flag bits.
const RUN_OPEN: u32 = 0x20;
const RUNNING: u32 = 0x40;
/// A node's flag bits.
const SCORED: u32 = 1;
const ROUNDED: u32 = 2;
const FLOORED: u32 = 4;
const LIVE: u32 = 0x10;
const MARKED: u32 = 0x20;

/// Every node of the chain at `head` back onto the run's free list, front first.
fn free(m: &mut impl Runs, run: u32, head: u32) {
    let mut n = m.word(head);
    while n != 0 {
        let next = m.word(n.wrapping_add(NEXT));
        m.set(head, next);
        let free = m.word(run.wrapping_add(0x3c));
        m.set(n.wrapping_add(NEXT), free);
        m.set(run.wrapping_add(0x3c), n);
        n = m.word(head);
    }
}

/// The group (the run's `+0x24`, then its members) and the player's watchers told `heard`; with
/// `nested`, the watchers' list only when the player has a watcher (the tally's shape).
fn tell(m: &mut impl Runs, run: u32, player: u32, heard: Heard, nested: bool) {
    let g = m.word(run.wrapping_add(0x24));
    if g != 0 {
        m.out(Out::Tell { to: g, vt: GROUP_VT, player, heard });
        let g = m.word(run.wrapping_add(0x24));
        let mut e = m.word(g.wrapping_add(4));
        while e != 0 {
            m.out(Out::Tell { to: e, vt: GROUP_VT, player, heard });
            e = m.word(e.wrapping_add(0x18));
        }
    }
    let w = m.word(player.wrapping_add(4));
    if w != 0 {
        m.out(Out::Tell { to: w, vt: WATCH_VT, player, heard });
    } else if nested {
        return;
    }
    let mut l = m.word(player.wrapping_add(0x1d4));
    while l != 0 {
        m.out(Out::Tell { to: l, vt: WATCH_VT, player, heard });
        l = m.word(l.wrapping_add(0x1c));
    }
}

/// **The run opened** for `player`: with the run scoring and not held off and the player in and
/// not out, the run open flag set, `+0xd8` and the count cleared and the player's stunts freed;
/// then everyone told. Answers 0 for no player, else 1.
pub fn open(m: &mut impl Runs, run: u32, player: u32) -> u32 {
    if player == 0 {
        return 0;
    }
    let rf = m.word(run.wrapping_add(0x50));
    if rf & 1 != 0 && rf & 8 == 0 {
        let pf = m.word(player.wrapping_add(8));
        if pf & 2 != 0 && pf & 1 == 0 {
            m.set(player.wrapping_add(8), pf | RUN_OPEN);
            m.set(player.wrapping_add(0xd8), 0);
            m.set(player.wrapping_add(0xcc), 0);
            free(m, run, player.wrapping_add(0xc8));
            m.set(player.wrapping_add(0xcc), 0);
        }
    }
    tell(m, run, player, Heard::Open, false);
    1
}

/// **A stunt scored** (`id`, the node's flags `kind`; `base`, `partner`, `second`, `times` and
/// `step` the node's `+0xc`, `+0x18`, `+0x10`, `+0x14` and `+0x20`): with the player's run open, a
/// node off the free list, cleared and filled (its score the base times the multiplier, its flags
/// `kind` running and scored), put at the front of the player's stunts, counted and the player's
/// stunt flag set; then everyone told. Answers 0 for no player, no run open or no free node, else 1.
#[allow(clippy::too_many_arguments)]
pub fn score(m: &mut impl Runs, run: u32, player: u32, id: u32, kind: u32, base: F, partner: F, second: F, times: F, step: F) -> u32 {
    if player == 0 || m.word(player.wrapping_add(8)) & RUN_OPEN == 0 {
        return 0;
    }
    let n = m.word(run.wrapping_add(0x3c));
    if n == 0 {
        return 0;
    }
    let next = m.word(n.wrapping_add(NEXT));
    m.set(run.wrapping_add(0x3c), next);
    for i in 0..NODE_WORDS {
        m.set(n.wrapping_add(4 * i), 0);
    }
    m.set(n.wrapping_add(NEXT), 0);
    m.set(n.wrapping_add(4), id);
    setf(m, n.wrapping_add(0xc), base);
    setf(m, n.wrapping_add(0x18), partner);
    setf(m, n.wrapping_add(0x10), second);
    setf(m, n.wrapping_add(0x14), times);
    let s = f(m, n.wrapping_add(0xc)) * times;
    setf(m, n.wrapping_add(8), s);
    setf(m, n.wrapping_add(0x20), step);
    m.set(n, kind | SCORED | LIVE);
    let head = m.word(player.wrapping_add(0xc8));
    m.set(n.wrapping_add(NEXT), head);
    m.set(player.wrapping_add(0xc8), n);
    let count = m.word(player.wrapping_add(0xcc)).wrapping_add(1);
    m.set(player.wrapping_add(0xcc), count);
    let pf = m.word(player.wrapping_add(8));
    m.set(player.wrapping_add(8), pf | RUNNING);
    tell(m, run, player, Heard::Score(n), false);
    1
}

/// **A stunt's mark** set (`on`) or cleared: the player's last running node of `id` not marked
/// (for `on`) or marked; for a negative `id`, `node`, or the first running one. The run is not read.
/// Answers 0 for no player, else 1.
pub fn mark(m: &mut impl Runs, _run: u32, player: u32, on: u32, id: u32, node: u32) -> u32 {
    if player == 0 {
        return 0;
    }
    let mut hit = 0;
    if (id as i32) < 0 {
        hit = if node != 0 { node } else { first_live(m, player) };
    } else {
        let mut n = m.word(player.wrapping_add(0xc8));
        while n != 0 {
            if m.word(n.wrapping_add(4)) == id {
                let fl = m.word(n);
                if fl & LIVE != 0 && (fl & MARKED == 0) == (on != 0) {
                    hit = n;
                }
            }
            n = m.word(n.wrapping_add(NEXT));
        }
    }
    if hit != 0 {
        let fl = m.word(hit);
        m.set(hit, if on != 0 { fl | MARKED } else { fl & !MARKED });
    }
    1
}

/// The player's first running node, or 0.
fn first_live(m: &mut impl Runs, player: u32) -> u32 {
    let mut n = m.word(player.wrapping_add(0xc8));
    while n != 0 && m.word(n) & LIVE == 0 {
        n = m.word(n.wrapping_add(NEXT));
    }
    n
}

/// **A stunt ended** (`x`, `flag`, `id`, `node` as [`mark`]'s): with the player's run open and a
/// stunt running, the node (the last running one of `id`, or for a negative `id` `node` or the
/// first running one) stopped and unmarked, and either unscored (no `flag`) or its score times `x`,
/// then floored or rounded to its step when the step is over 0; everyone told; and the player's
/// stunt flag kept only while a node still runs. Answers 0 for no player or nothing open, else 1.
pub fn end(m: &mut impl Runs, x: F, run: u32, player: u32, flag: u32, id: u32, node: u32) -> u32 {
    if player == 0 {
        return 0;
    }
    let pf = m.word(player.wrapping_add(8));
    if pf & RUNNING == 0 || pf & RUN_OPEN == 0 {
        return 0;
    }
    let mut hit = 0;
    if (id as i32) < 0 {
        hit = if node != 0 { node } else { first_live(m, player) };
    } else {
        let mut n = m.word(player.wrapping_add(0xc8));
        while n != 0 {
            if m.word(n.wrapping_add(4)) == id && m.word(n) & LIVE != 0 {
                hit = n;
            }
            n = m.word(n.wrapping_add(NEXT));
        }
    }
    if hit != 0 {
        let fl = m.word(hit) & !(LIVE | MARKED);
        m.set(hit, fl);
        if flag == 0 {
            let fl = m.word(hit);
            m.set(hit, fl & !SCORED);
        } else {
            let s = f(m, hit.wrapping_add(8)) * x;
            setf(m, hit.wrapping_add(8), s);
            let step = f(m, hit.wrapping_add(0x20));
            if !(step <= F::ZERO) {
                let fl = m.word(hit);
                if fl & FLOORED != 0 {
                    let step = f(m, hit.wrapping_add(0x20));
                    let q = f(m, hit.wrapping_add(8)) / step;
                    setf(m, hit.wrapping_add(8), F(fpu::from_i32(q.to_i32())) * step);
                } else if fl & ROUNDED != 0 {
                    let q = F(0.5) + f(m, hit.wrapping_add(8)) / step;
                    setf(m, hit.wrapping_add(8), F(fpu::from_i32(q.to_i32())) * step);
                }
            }
        }
    }
    tell(m, run, player, Heard::End { x: x.to_bits(), flag, node: hit }, false);
    let pf = m.word(player.wrapping_add(8));
    m.set(player.wrapping_add(8), pf & !RUNNING);
    if first_live(m, player) != 0 {
        let pf = m.word(player.wrapping_add(8));
        m.set(player.wrapping_add(8), pf | RUNNING);
    }
    1
}

/// **The run closed** (`x`, `flag`): each of the player's running nodes ended ([`end`], id −1).
/// Answers 0 for no player, else 1.
pub fn close(m: &mut impl Runs, x: F, run: u32, player: u32, flag: u32) -> u32 {
    if player == 0 {
        return 0;
    }
    let mut n = m.word(player.wrapping_add(0xc8));
    while n != 0 {
        if m.word(n) & LIVE != 0 {
            end(m, x, run, player, flag, u32::MAX, n);
        }
        n = m.word(n.wrapping_add(NEXT));
    }
    1
}

/// **The run tallied** (`x`, `flag`): with the player's run open, the run and stunt flags cleared,
/// the stunts' scores summed (each stopped and unmarked), the sum times `x` added to the run's score
/// (with `flag`), the score floored to the run's step; everyone told (the watchers' list only with a
/// watcher); the sum cleared; and the stunts freed, or with a history kept, put in front of it, the
/// history cut to its length. Answers 0 for no player or no run open, else 1.
pub fn tally(m: &mut impl Runs, x: F, run: u32, player: u32, flag: u32) -> u32 {
    if player == 0 {
        return 0;
    }
    let pf = m.word(player.wrapping_add(8));
    if pf & RUN_OPEN == 0 {
        return 0;
    }
    m.set(player.wrapping_add(8), pf & !(RUN_OPEN | RUNNING));
    m.set(player.wrapping_add(0xe0), 0);
    let mut n = m.word(player.wrapping_add(0xc8));
    while n != 0 {
        let sum = f(m, player.wrapping_add(0xe0)) + f(m, n.wrapping_add(8));
        setf(m, player.wrapping_add(0xe0), sum);
        let fl = m.word(n);
        m.set(n, fl & !(LIVE | MARKED));
        n = m.word(n.wrapping_add(NEXT));
    }
    if flag != 0 {
        let (score, sum) = (f(m, player.wrapping_add(0xdc)), f(m, player.wrapping_add(0xe0)));
        let s = fpu::Acc::adda(F::ZERO, score).madd(sum, x);
        setf(m, player.wrapping_add(0xdc), s);
    }
    let (score, step) = (f(m, player.wrapping_add(0xdc)), f(m, run.wrapping_add(0x2a8)));
    let q = score / step;
    setf(m, player.wrapping_add(0xdc), F(fpu::from_i32(q.to_i32())) * step);
    tell(m, run, player, Heard::Tally { x: x.to_bits(), flag }, true);
    m.set(player.wrapping_add(0xe0), 0);
    if m.word(run.wrapping_add(0x50)) & 0x8000 == 0 {
        if player.wrapping_add(0xc8) != 0 {
            free(m, run, player.wrapping_add(0xc8));
        }
        m.set(player.wrapping_add(0xcc), 0);
        return 1;
    }
    if (m.word(run.wrapping_add(0x300)) as i32) < 0 {
        if player.wrapping_add(0xd0) != 0 {
            free(m, run, player.wrapping_add(0xd0));
        }
        m.set(player.wrapping_add(0xd4), 0);
    }
    let mut last = m.word(player.wrapping_add(0xc8));
    while last != 0 {
        let next = m.word(last.wrapping_add(NEXT));
        if next == 0 {
            break;
        }
        last = next;
    }
    if last != 0 {
        let history = m.word(player.wrapping_add(0xd0));
        m.set(last.wrapping_add(NEXT), history);
        let head = m.word(player.wrapping_add(0xc8));
        m.set(player.wrapping_add(0xd0), head);
        let kept = m.word(player.wrapping_add(0xd4)).wrapping_add(m.word(player.wrapping_add(0xcc)));
        m.set(player.wrapping_add(0xd4), kept);
        m.set(player.wrapping_add(0xc8), 0);
        m.set(player.wrapping_add(0xcc), 0);
    }
    fn len(m: &mut impl Runs, run: u32) -> i32 {
        m.word(run.wrapping_add(0x300)) as i32
    }
    fn kept(m: &mut impl Runs, player: u32) -> i32 {
        m.word(player.wrapping_add(0xd4)) as i32
    }
    if len(m, run) < kept(m, player) {
        m.set(player.wrapping_add(0xd4), 0);
        let mut p = player.wrapping_add(0xd0);
        while kept(m, player) < len(m, run) {
            let k = m.word(player.wrapping_add(0xd4));
            let n = m.word(p);
            m.set(player.wrapping_add(0xd4), k.wrapping_add(1));
            p = n.wrapping_add(NEXT);
        }
        if m.word(p) != 0 && p != 0 {
            free(m, run, p);
        }
    }
    1
}

/// The bike's event record (`+0xd24`) cleared: five words 0, three −1.
fn record_cleared(m: &mut impl Runs, bike: u32) -> u32 {
    let r = m.word(bike.wrapping_add(0xd24));
    for o in [4, 8, 0xc, 0x10, 0x14] {
        m.set(r.wrapping_add(o), 0);
    }
    for o in [0x18, 0x1c, 0x20] {
        m.set(r.wrapping_add(o), u32::MAX);
    }
    r
}

/// The record word `o` set to `v`, the record re-read.
fn record_set(m: &mut impl Runs, bike: u32, o: u32, v: u32) {
    let r = m.word(bike.wrapping_add(0xd24));
    m.set(r.wrapping_add(o), v);
}

/// The bike's event `id` posted on its listeners (`+0x674`), the record its data.
fn post(m: &mut impl Runs, bike: u32, id: u32, x: u32) {
    let on = if bike != 0 { bike.wrapping_add(0x674) } else { 0 };
    let data = m.word(bike.wrapping_add(0xd24));
    m.out(Out::Post { on, kind: 8, id, data, x });
}

/// **The bike's watcher told the run opened**: the record's player, event 5. Answers 1.
pub fn watch_open(m: &mut impl Runs, bike: u32, player: u32) -> u32 {
    record_cleared(m, bike);
    record_set(m, bike, 8, player);
    post(m, bike, 5, 0);
    1
}

/// **The bike's watcher told a stunt scored**: the record's player, node and the bike's first kept
/// stunt, event 6. Answers 1.
pub fn watch_score(m: &mut impl Runs, bike: u32, player: u32, node: u32) -> u32 {
    record_cleared(m, bike);
    record_set(m, bike, 8, player);
    record_set(m, bike, 0x14, node);
    let k = m.word(bike.wrapping_add(0x13b8));
    record_set(m, bike, 0x18, k);
    post(m, bike, 6, 0);
    1
}

/// **The bike's watcher told a stunt ended**: the record's player, flag, node and the bike's first
/// three kept stunts, event 7. Answers 1.
pub fn watch_end(m: &mut impl Runs, bike: u32, player: u32, flag: u32, node: u32) -> u32 {
    record_cleared(m, bike);
    record_set(m, bike, 8, player);
    record_set(m, bike, 0x10, flag);
    record_set(m, bike, 0x14, node);
    for (o, k) in [(0x18, 0x13b8), (0x1c, 0x13bc), (0x20, 0x13c0)] {
        let k = m.word(bike.wrapping_add(k));
        record_set(m, bike, o, k);
    }
    post(m, bike, 7, 0);
    1
}

/// The run's score at which the bike keeps its time (`+0x1750` into `+0x6f4`).
const MILLION: F = F(1_000_000.0);

/// **The bike's watcher told the run tallied** (`x`, `flag`): with a player and `flag`, the best
/// combo (`+0x6f0`) raised to the sum times `x` for two stunts or more, the time (`+0x6f4`, none
/// when negative) lowered to the bike's `+0x1750` once the score reaches a million, and each stunt's
/// repeat scale lowered ([`repeat`]); then the record's player and flag, event 8 with `x` as an int.
/// Answers 1.
pub fn watch_tally(m: &mut impl Runs, bike: u32, x: F, player: u32, flag: u32) -> u32 {
    if player != 0 && flag != 0 {
        let combo = f(m, player.wrapping_add(0xe0)) * x;
        if m.word(player.wrapping_add(0xcc)) as i32 >= 2 && !(combo <= f(m, bike.wrapping_add(0x6f0))) {
            setf(m, bike.wrapping_add(0x6f0), combo);
        }
        if !(f(m, player.wrapping_add(0xdc)) < MILLION) {
            let t = f(m, bike.wrapping_add(0x6f4));
            if t < F::ZERO || f(m, bike.wrapping_add(0x1750)) < t {
                let now = m.word(bike.wrapping_add(0x1750));
                m.set(bike.wrapping_add(0x6f4), now);
            }
        }
        let mut n = m.word(player.wrapping_add(0xc8));
        while n != 0 {
            let id = m.word(n.wrapping_add(4));
            let table = m.word(bike.wrapping_add(0x14f0));
            repeat(m, table, id);
            n = m.word(n.wrapping_add(NEXT));
        }
    }
    record_cleared(m, bike);
    record_set(m, bike, 8, player);
    record_set(m, bike, 0x10, flag);
    post(m, bike, 8, x.to_i32() as u32);
    1
}

/// The stunt table's entry count (as [`super::update`]'s).
const STUNTS: u32 = 46;

/// **A stunt's repeat scale lowered** (the stunt table, its id): with the table in use, once the
/// scale is back at 1 every scale is first divided by the table's `+4` (at most 1); then the scale
/// times the table's `+8`, at least its `+0xc`.
pub fn repeat(m: &mut impl Runs, table: u32, id: u32) {
    if m.word(table) == 0 {
        return;
    }
    let at = table.wrapping_add(id.wrapping_mul(12)).wrapping_add(0x1c);
    if f(m, at) == F::ONE {
        for i in 0..STUNTS {
            let s = table.wrapping_add(0x1c + 12 * i);
            let v = (f(m, s) / f(m, table.wrapping_add(4))).min(F::ONE);
            setf(m, s, v);
        }
    }
    let rate = f(m, table.wrapping_add(8));
    let v = (f(m, at) * rate).max(f(m, table.wrapping_add(0xc)));
    setf(m, at, v);
}

/// **The group told a stunt scored** (the group's owner): with gauges (`+0x3dc`, counted at
/// `+0x3e0`), each shown one (byte `+0x21` bit 0) kicked ([`kick`]) by the owner's settings
/// (`+0x3d4`: the kick `+0x58c`, the level `+0x594` for a run of kind bit 2, else `+0x59c`). Answers
/// 1.
pub fn group_score(m: &mut impl Runs, owner: u32, player: u32, _node: u32) -> u32 {
    if m.word(owner.wrapping_add(0x3dc)) == 0 {
        return 1;
    }
    let s = m.word(owner.wrapping_add(0x3d4));
    let level = if m.word(player.wrapping_add(8)) & 4 != 0 { f(m, s.wrapping_add(0x594)) } else { f(m, s.wrapping_add(0x59c)) };
    let mut i = 0;
    while i < m.word(owner.wrapping_add(0x3e0)) as i32 {
        let gauges = m.word(owner.wrapping_add(0x3dc));
        let g = m.word(gauges.wrapping_add(4 * i as u32));
        if byte(m, g.wrapping_add(0x21)) & 1 != 0 {
            let k = f(m, s.wrapping_add(0x58c));
            kick(m, g, k, level);
        }
        i += 1;
    }
    1
}

/// **A gauge kicked** (`k`, the level `to`): its peak (`+0x58`) at least `k`, its half-life
/// (`+0x60`) half the peak, its rate (`+0x5c`) from its level (`+0x50`) to `to` over the half-life.
pub fn kick(m: &mut impl Runs, g: u32, k: F, to: F) {
    let peak = k.max(f(m, g.wrapping_add(0x58)));
    setf(m, g.wrapping_add(0x58), peak);
    let half = F(0.5) * f(m, g.wrapping_add(0x58));
    setf(m, g.wrapping_add(0x60), half);
    let rate = (to - f(m, g.wrapping_add(0x50))) / f(m, g.wrapping_add(0x60));
    setf(m, g.wrapping_add(0x5c), rate);
}

/// **The group told the run tallied** (the group's owner): event 10 on the owner's listeners
/// (`+0x384`), the player its data and the flag its last word. Answers 1.
pub fn group_tally(m: &mut impl Runs, owner: u32, player: u32, flag: u32) -> u32 {
    let on = if owner != 0 { owner.wrapping_add(0x384) } else { 0 };
    m.out(Out::Post { on, kind: 1, id: 10, data: player, x: flag });
    1
}
