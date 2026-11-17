//! **SLUS-21104's camera controller told of the wreck** (F67): what the camera controller does when
//! the bike's wreck posts its events ([`super::reset::post`], message kind 8), as word-level ports over
//! RAM. Where the original keeps them: research/oracle.txt `# F67`.
//!
//! The controller's receive ([`receive`]) acts on the event's id:
//!
//! - `0x15`: the current camera's `+0x78` set, and the player's fader (by the slot that holds the
//!   controller's actor) started;
//! - `7`, `6`: back to the chase camera, or on to the follow camera, when the current one asks;
//! - `0xd` (the put-back): back to the chase camera, the screen fade started once the wreck has lasted,
//!   and the hold `+0x74` set while the bike's `+0xde8` is above 0;
//! - `0xc`: the wreck's length into `+0x70`;
//! - `0xb` (the wreck): the wreck camera aimed ([`aim`]) and switched to.
//!
//! A switch ([`controller_switch`], [`switch`]): the old camera left (its vt `+0xe0`: [`leave_a`],
//! [`leave_b`]), its interface taken off the connector's list ([`unlist`]), the new one put on that
//! sorted list ([`list_add`]) and every interface connected again ([`connect`]: the cameras'
//! [`connect_a`] / [`connect_b`], the second keeping its targets in two heap arrays,
//! [`targets_add`]); the new camera entered (its vt `+0xdc`: [`enter_a`], [`enter_b`]: its aim set
//! ([`set_offset`], [`set_target`]) and spread over its targets ([`spread`], [`distances`])); the
//! controller's actor taken out of its world and put back in its cell ([`Out::Remove`],
//! [`Out::Insert`]) with its record's spot kept.
//!
//! Every word goes through [`Cameras`] as it is read or written, in the original's order, and every
//! virtual call is an [`Out::Call`] with the function read off the object's vtable: the caller serves
//! it with the port of that function.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use ps2_float::fpu;
use ps2_float::vu::{self, Dest, Vf};
use ps2_float::F;

/// Where the original's heap calls come from: each its own file and line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    /// The connector's list: an interface appended, or put before a node.
    ListAppend,
    ListInsert,
    /// An interface's pair (its mask and the other interface), and its node on the pair list.
    Pair,
    PairAppend,
    /// The second camera's target array and its spot array, grown by one.
    Targets,
    Spots,
}

/// A call outside the controller's words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    /// A virtual: the function `f` read off the object's vtable, with its arguments (the object first).
    Call { f: u32, args: [u32; 4] },
    /// The heap: `bytes` new (the allocator, or `new`) or `at` grown to `bytes`, from `site`; `at`
    /// freed.
    Alloc { bytes: u32, site: Site },
    New { bytes: u32, site: Site },
    Realloc { at: u32, bytes: u32, site: Site },
    Free(u32),
    /// The actor `entry` taken out of its world ([`super::attach::remove`]).
    Remove(u32),
    /// The actor `entry` put back in `cell` under `kind`, its box at the quad at `centre`, `half` the
    /// quad at `half`, with no reach and no overlaps ([`super::attach::insert_with`]).
    Insert { entry: u32, cell: u32, kind: u32, centre: u32, half: u32 },
    /// The interface `item` joined to the controller's `+0x34` (the switch's other arm).
    Join { item: u32, with: u32 },
}

/// The words the controller reads that are the game's, not its own.
#[derive(Clone, Copy, Debug)]
pub struct Globals {
    /// The word holding the players' table (the actor of player `i` at `+0x25c + 4i`).
    pub players: u32,
    /// The word holding the screen (its two faders at `+0x394`, `+0x398`).
    pub screen: u32,
    /// The quad `(0, 0, 0, 1)`.
    pub unit: u32,
    /// The quad a spread's sum starts at.
    pub origin: u32,
}

pub trait Cameras {
    fn word(&mut self, at: u32) -> u32;
    fn set(&mut self, at: u32, w: u32);
    fn out(&mut self, c: Out) -> u32;
    /// A free word for an answer handed back through a pointer (the original's is on its stack).
    fn local(&mut self) -> u32;
}

const ONE: u32 = 0x3f80_0000;
const MAX: u32 = 0x7f7f_ffff;

fn fw(m: &mut impl Cameras, at: u32) -> f32 {
    f32::from_bits(m.word(at))
}

fn quad(m: &mut impl Cameras, at: u32) -> [u32; 4] {
    [m.word(at), m.word(at + 4), m.word(at + 8), m.word(at + 0xc)]
}

fn put_quad(m: &mut impl Cameras, at: u32, q: [u32; 4]) {
    for (k, w) in q.into_iter().enumerate() {
        m.set(at + 4 * k as u32, w);
    }
}

/// The three lanes `xyz` of `v` written at `at`, its `w` word left.
fn put_xyz(m: &mut impl Cameras, at: u32, v: Vf) {
    for k in 0..3 {
        m.set(at + 4 * k, v.to_bits()[k as usize]);
    }
}

/// The word at `off` past the pointer held at `at`.
fn through(m: &mut impl Cameras, at: u32, off: u32) -> u32 {
    let p = m.word(at);
    m.word(p + off)
}

fn call(m: &mut impl Cameras, f: u32, a: &[u32]) -> u32 {
    let mut args = [0; 4];
    args[..a.len()].copy_from_slice(a);
    m.out(Out::Call { f, args })
}

/// The object's virtual at `slot` (its vtable at `+4`), called with `this` first.
fn virt(m: &mut impl Cameras, this: u32, slot: u32, rest: &[u32]) -> u32 {
    let f = through(m, this + 4, slot);
    let mut a = vec![this];
    a.extend_from_slice(rest);
    call(m, f, &a)
}

/// An interface's virtual at `slot` (its vtable at `+0x48`).
fn item_virt(m: &mut impl Cameras, item: u32, slot: u32, rest: &[u32]) -> u32 {
    let f = through(m, item + 0x48, slot);
    let mut a = vec![item];
    a.extend_from_slice(rest);
    call(m, f, &a)
}

// --- the receive ---------------------------------------------------------------------------------------

/// The player slot (0 to 3) whose actor is the controller's `+0x2c`, or -1. The table is read once,
/// the actor again for every slot.
fn player_of(m: &mut impl Cameras, g: &Globals, ctl: u32) -> i32 {
    let table = m.word(g.players);
    for i in 0..4 {
        if m.word(table + 4 * i + 0x25c) == m.word(ctl + 0x2c) {
            return i as i32;
        }
    }
    -1
}

/// **The controller told of a kind-8 event** (the receive, behind its thunk at the controller's
/// `+0x38`): `from` the poster's registration (the bike's `+0x674`), `msg` the four words. Answers 1.
pub fn receive(m: &mut impl Cameras, g: &Globals, ctl: u32, from: u32, msg: u32) -> u32 {
    let bike = if from != 0 { from.wrapping_sub(0x674) } else { 0 };
    match m.word(msg + 4) {
        0x15 => {
            let cur = m.word(ctl + 0x28);
            m.set(cur + 0x78, 1);
            let i = player_of(m, g, ctl);
            if i >= 0 {
                let fader = through(m, g.screen, 0x394);
                let on = m.word(fader + 0x40) + 4 * i as u32;
                m.set(on, 1);
            }
        }
        7 => {
            let cur = m.word(ctl + 0x28);
            let chase = m.word(ctl + 0x78);
            if cur == chase && m.word(chase + 0x110) == m.word(ctl + 0x58) && cur != m.word(ctl + 0x4c) {
                let to = m.word(ctl + 0x4c);
                virt(m, ctl, 0x84, &[to, 0]);
                back_to(m, ctl);
            }
        }
        6 => {
            let cur = m.word(ctl + 0x28);
            if cur == m.word(ctl + 0x84) && m.word(ctl + 0x58) as i32 >= 0 && {
                let e = through(m, msg + 0xc, 0x14);
                (m.word(e + 4) as i32) < 0x29
            } {
                let to = m.word(ctl + 0x78);
                virt(m, ctl, 0x84, &[to, 0]);
                let chase = m.word(ctl + 0x78);
                let at = m.word(ctl + 0x58);
                virt(m, chase, 0x100, &[at]);
            }
        }
        0xd => put_back(m, g, ctl, bike),
        0xc => {
            let d = fpu::sub(fw(m, bike + 0xc88), fw(m, bike + 0xc84));
            m.set(ctl + 0x70, d.to_bits());
        }
        0xb => {
            let cur = m.word(ctl + 0x28);
            if cur == m.word(ctl + 0x78) || cur == m.word(ctl + 0x84) {
                let p = m.word(bike + 0xc80);
                aim(m, g, ctl, p);
            }
        }
        _ => {}
    }
    1
}

/// After a switch back: the chase camera's vt `+0x100` with the controller's `+0x50`, when it is the
/// current one and its `+0x110` is not that.
fn back_to(m: &mut impl Cameras, ctl: u32) {
    let chase = m.word(ctl + 0x78);
    if m.word(ctl + 0x28) == chase {
        let at = m.word(ctl + 0x50);
        if m.word(chase + 0x110) != at {
            virt(m, chase, 0x100, &[at]);
        }
    }
}

/// The current camera is one of the three the wreck uses (`+0x88`, `+0x78`, `+0x84`).
fn wreck_camera(m: &mut impl Cameras, ctl: u32) -> bool {
    let cur = m.word(ctl + 0x28);
    cur == m.word(ctl + 0x88) || cur == m.word(ctl + 0x78) || cur == m.word(ctl + 0x84)
}

/// Id `0xd`: back to `+0x4c`; past 0.34 s the player's screen fade (the second fader's slot started
/// at 150 and 150, when not already running) and the clock held at 0.34; the hold `+0x74` while the
/// bike's `+0xde8` is above 0.
fn put_back(m: &mut impl Cameras, g: &Globals, ctl: u32, bike: u32) {
    if !wreck_camera(m, ctl) {
        return;
    }
    let (cur, to) = (m.word(ctl + 0x28), m.word(ctl + 0x4c));
    if cur != to {
        virt(m, ctl, 0x84, &[to, 0]);
        back_to(m, ctl);
    }
    const LONG: u32 = 0x3eae_147b; // 0.34
    if !(F(fw(m, ctl + 0x70)) <= F(f32::from_bits(LONG))) {
        let i = player_of(m, g, ctl);
        if i >= 0 && wreck_camera(m, ctl) {
            let fader = through(m, g.screen, 0x398);
            if !fade_running(m, fader, i as u32) {
                fade_start(m, fader, i as u32, 150, 150);
            }
        }
        m.set(ctl + 0x70, LONG);
    }
    if !(F(fw(m, bike + 0xde8)) <= F::ZERO) {
        m.set(ctl + 0x74, 2);
    }
}

/// A fader's slot `i` is running (bit 0 or 1 of its `+0x38`).
fn fade_running(m: &mut impl Cameras, fader: u32, i: u32) -> bool {
    let e = m.word(fader + 4 * i + 0x34);
    e != 0 && m.word(e + 0x38) & 3 != 0
}

/// A fader's slot `i` started, unless it has none or is on.
fn fade_start(m: &mut impl Cameras, fader: u32, i: u32, ms: u32, hold: u32) {
    let e = m.word(fader + 4 * i + 0x34);
    if e != 0 && m.word(e + 0x38) & 1 == 0 {
        fade_on(m, e, ms, hold);
    }
}

/// A fade switched on, once: its flags (bits 2 and 0 set, bit 1 cleared), the hold, the clock, the
/// rate (`ms` / 1000) and the level (1 for an instant fade), then shown off (its vt `+0x20`).
fn fade_on(m: &mut impl Cameras, e: u32, ms: u32, hold: u32) {
    if m.word(e + 0x38) & 4 != 0 {
        return;
    }
    let f = m.word(e + 0x38) | 4;
    m.set(e + 0x38, f);
    m.set(e + 0x3c, hold);
    let f = m.word(e + 0x38) | 1;
    m.set(e + 0x38, f);
    let f = m.word(e + 0x38) & !2;
    m.set(e + 0x38, f);
    m.set(e + 0x28, 0);
    m.set(e + 0x2c, fpu::div(fpu::from_i32(ms as i32), 1000.0).to_bits());
    m.set(e + 0x30, if ms == 0 { ONE } else { 0 });
    virt(m, e, 0x20, &[]);
}

// --- the wreck camera aimed ------------------------------------------------------------------------

/// **The wreck camera aimed** (id `0xb`, `p` the bike's `+0xc80`): its offset (`+0xb0`) the chase
/// camera's when that is current, else `(0, 0, 0, 1)`; its target and offset rates (the 13 kind's
/// tighter); then switched to, flagged unless the follow camera is current or the chase camera's
/// current shot has its `+0x7c`.
pub fn aim(m: &mut impl Cameras, g: &Globals, ctl: u32, p: u32) {
    let unit = quad(m, g.unit);
    let cur = m.word(ctl + 0x28);
    let mut flag = 0;
    if cur != m.word(ctl + 0x84) {
        let chase = m.word(ctl + 0x78);
        let held = cur == chase && {
            let shot = m.word(chase + 0x124) + m.word(chase + 0x110).wrapping_mul(0xa0);
            m.word(shot + 0x7c) != 0
        };
        if !held {
            flag = 1;
        }
    }
    if cur == m.word(ctl + 0x78) {
        for k in 0..3 {
            let offs = through(m, ctl + 0x78, 0xc4);
            let w = through(m, offs + 4 * k, 0);
            let to = m.word(ctl + 0x88) + 0xb0 + 4 * k;
            m.set(to, w);
        }
    } else {
        let cam = m.word(ctl + 0x88);
        put_quad(m, cam + 0xb0, unit);
    }
    let (a, b) = if p == 0xd {
        let s = f32::from_bits(0x3e6b_851f); // 0.23
        let y = fpu::mul(f32::from_bits(0x3f66_6666), fpu::mul(s, f32::from_bits(through(m, ctl + 0x88, 0x94))));
        ([0x3e4c_cccd; 3], [s.to_bits(), y.to_bits(), s.to_bits()])
    } else {
        let s = f32::from_bits(0x3e99_999a); // 0.3
        let y = fpu::mul(s, f32::from_bits(through(m, ctl + 0x88, 0x94)));
        ([0x3e8f_5c29; 3], [s.to_bits(), y.to_bits(), s.to_bits()])
    };
    let cam = m.word(ctl + 0x88);
    let at = m.word(ctl + 0x2c) + 0x140;
    set_target(m, cam, Src::Ram(at), Src::Words(a));
    let cam = m.word(ctl + 0x88);
    set_offset(m, cam, Src::Ram(cam + 0xb0), Src::Words(b));
    let cam = m.word(ctl + 0x88);
    virt(m, ctl, 0x84, &[cam, flag]);
}

/// Where a camera's three limits come from: none, three words in RAM, or three words of the caller's.
#[derive(Clone, Copy, Debug)]
pub enum Src {
    None,
    Ram(u32),
    Words([u32; 3]),
}

fn src(m: &mut impl Cameras, s: Src, k: u32) -> u32 {
    match s {
        Src::None => MAX,
        Src::Ram(at) => m.word(at + 4 * k),
        Src::Words(w) => w[k as usize],
    }
}

/// The three limits at the camera's pointers `+base`: each lane's `a` into the limit's first word, and
/// `b` into its second with its third 1, where not the largest float.
fn limits(m: &mut impl Cameras, cam: u32, base: u32, a: Src, b: Src) {
    for k in 0..3 {
        let p = m.word(cam + base + 4 * k);
        if p == 0 {
            continue;
        }
        let fb = src(m, b, k);
        let fa = src(m, a, k);
        if F(f32::from_bits(fa)) != F(f32::from_bits(MAX)) {
            m.set(p, fa);
        }
        if F(f32::from_bits(fb)) != F(f32::from_bits(MAX)) {
            m.set(p + 4, fb);
            m.set(p + 8, ONE);
        }
    }
}

/// The camera's target limits (its pointers at `+0xe0`).
pub fn set_target(m: &mut impl Cameras, cam: u32, a: Src, b: Src) {
    limits(m, cam, 0xe0, a, b);
}

/// The camera's offset limits (its pointers at `+0xec`).
pub fn set_offset(m: &mut impl Cameras, cam: u32, a: Src, b: Src) {
    limits(m, cam, 0xec, a, b);
}

// --- the switch ----------------------------------------------------------------------------------------

/// **The controller's switch** (its vt `+0x84`): the chase slot `+0x78` taken by `cam` when it is one
/// of the two chase cameras, then [`switch`].
pub fn controller_switch(m: &mut impl Cameras, g: &Globals, ctl: u32, cam: u32, flag: u32) -> u32 {
    if cam == m.word(ctl + 0x7c) || cam == m.word(ctl + 0x80) {
        m.set(ctl + 0x78, cam);
    }
    switch(m, g, ctl, cam, flag)
}

/// The interface of a camera (`+0x24`), or none.
fn item_of(cam: u32) -> u32 {
    if cam != 0 { cam + 0x24 } else { 0 }
}

/// **A camera switch**: the old one left and its interface unlisted from the connector (`+0x30`), the
/// previous `+0x24` kept, `cam` current, its interface listed and connected, `cam` entered with
/// `flag`, and the controller's actor (`+0x2c`) taken out of its world and put back in its cell under
/// kind 2 at its position, its record's spot kept. Answers 1.
pub fn switch(m: &mut impl Cameras, g: &Globals, ctl: u32, cam: u32, flag: u32) -> u32 {
    let old = m.word(ctl + 0x28);
    if old != 0 {
        virt(m, old, 0xe0, &[]);
        if m.word(ctl + 0x34) != 0 {
            let it = item_of(m.word(ctl + 0x28));
            let obj = m.word(ctl + 0x30);
            virt(m, obj, 0x88, &[it]);
            for slot in [0x3c, 0x4c] {
                let it = m.word(ctl + 0x28) + 0x24;
                item_virt(m, it, slot, &[]);
            }
        } else {
            let obj = m.word(ctl + 0x30);
            if obj != 0 {
                let it = item_of(m.word(ctl + 0x28));
                virt(m, obj, 0x88, &[it]);
            }
        }
    }
    let cur = m.word(ctl + 0x28);
    if cur != 0 && cur != cam {
        m.set(ctl + 0x24, cur);
    } else if cur == 0 {
        m.set(ctl + 0x24, cam);
    }
    m.set(ctl + 0x28, cam);
    if m.word(ctl + 0x34) == 0 {
        let obj = m.word(ctl + 0x30);
        if obj != 0 {
            let it = item_of(m.word(ctl + 0x28));
            virt(m, obj, 0x84, &[it]);
            let obj = m.word(ctl + 0x30);
            virt(m, obj, 0x94, &[]);
        }
    } else {
        let it = item_of(m.word(ctl + 0x28));
        let obj = m.word(ctl + 0x30);
        virt(m, obj, 0x84, &[it]);
        let (item, with) = (m.word(ctl + 0x28) + 0x24, m.word(ctl + 0x34));
        m.out(Out::Join { item, with });
    }
    let cur = m.word(ctl + 0x28);
    virt(m, cur, 0xdc, &[flag]);
    let actor = m.word(ctl + 0x2c);
    let rec = m.word(actor + 8);
    let spot = quad(m, rec + 0x10);
    let cell = m.word(rec + 4);
    m.out(Out::Remove(actor));
    let actor = m.word(ctl + 0x2c);
    m.set(actor + 0x30, cell);
    m.out(Out::Insert { entry: actor, cell, kind: 2, centre: actor + 0x140, half: g.unit });
    let rec = through(m, ctl + 0x2c, 8);
    put_quad(m, rec + 0x10, spot);
    1
}

/// A camera shown or not (`+0x78` set when `flag` is 0), then its vt `+0x20`.
pub fn show(m: &mut impl Cameras, cam: u32, flag: u32) {
    m.set(cam + 0x78, u32::from(flag == 0));
    virt(m, cam, 0x20, &[]);
}

/// **The first camera class left** (vt `+0xe0`) with no part at `+0xbc`: its vt `+0x18`.
pub fn leave_a(m: &mut impl Cameras, cam: u32) {
    virt(m, cam, 0x18, &[]);
}

/// **The first camera class entered** (vt `+0xdc`) with no part at `+0xbc`: [`show`].
pub fn enter_a(m: &mut impl Cameras, cam: u32, flag: u32) {
    show(m, cam, flag);
}

/// **The second camera class left** (vt `+0xe0`): every target's flag cleared, the count copied, the
/// offset `(0, 0, 0, 1)`, the blend 0, then its vt `+0x18`.
pub fn leave_b(m: &mut impl Cameras, g: &Globals, cam: u32) {
    let mut n = m.word(cam + 0x300);
    let mut k = 0;
    if 0 < n as i32 {
        loop {
            m.set(cam + 0x310 + 4 * k, 0);
            k += 1;
            n = m.word(cam + 0x300);
            if (k as i32) >= n as i32 {
                break;
            }
        }
    }
    m.set(cam + 0x304, n);
    let unit = quad(m, g.unit);
    put_quad(m, cam + 0xb0, unit);
    m.set(cam + 0x2a4, 0);
    virt(m, cam, 0x18, &[]);
}

/// **The second camera class entered** (vt `+0xdc`). With `flag`: its range from its target's
/// `+0x170`, the distance kept, the blend 1, and its offset limits from its `+0xb0` and its target
/// limits from its target's position (the arm for an offset of `(0, 0, 0, 1)` is not here); without,
/// shown with its own range and unflagged. Then its vt `+0xe8` and [`show`].
pub fn enter_b(m: &mut impl Cameras, cam: u32, flag: u32) {
    if flag == 0 {
        m.set(cam + 0x78, 1);
        let r = m.word(cam + 0x2f8);
        m.set(cam + 0x90, r);
        m.set(cam + 0x308, 0);
    } else {
        let r = through(m, cam + 0x70, 0x170);
        m.set(cam + 0x90, r);
        let d = m.word(cam + 0x2a0);
        m.set(cam + 0x29c, d);
        m.set(cam + 0x2a4, ONE);
        let o = quad(m, cam + 0xb0);
        set_offset(m, cam, Src::Words([o[0], o[1], o[2]]), Src::None);
        let at = m.word(cam + 0x70) + 0x140;
        set_target(m, cam, Src::Ram(at), Src::None);
        m.set(cam + 0x308, flag);
    }
    virt(m, cam, 0xe8, &[]);
    show(m, cam, flag);
}

/// **The second camera spread over its targets** (vt `+0xe8`): the targets' distances
/// ([`distances`]); their mean spot; the direction from it to the camera's target (`+0x2b0`, its `w`
/// the target's), its length the range `+0x84` and the direction made unit; the offset `+0xa0` from
/// the three offset limits, `w` 1; the range past its least `+0x294`, held between the least and the
/// most when unflagged; `+0x298` 0.
pub fn spread(m: &mut impl Cameras, g: &Globals, cam: u32) {
    let mut s = Vf::from_bits(quad(m, g.origin));
    distances(m, cam);
    let n = m.word(cam + 0x300);
    if 0 < n as i32 {
        let list = m.word(cam + 0x80);
        let mut k = 0u32;
        loop {
            let t = m.word(list + 4 * k) + 0x10;
            s = s.merge(Dest::XYZ, s.add(Vf::from_bits(quad(m, t))));
            k += 1;
            if (k as i32) >= m.word(cam + 0x300) as i32 {
                break;
            }
        }
    }
    let n = m.word(cam + 0x300);
    let q = vu::div(1.0, fpu::from_i32(n as i32));
    s = s.merge(Dest::XYZ, s.mul_q(q));
    let to = m.word(cam + 0x70) + 0x140;
    let raw = quad(m, to);
    let d = Vf::from_bits(raw);
    let d = d.merge(Dest::XYZ, d.sub(s));
    put_xyz(m, cam + 0x2b0, d);
    m.set(cam + 0x2bc, raw[3]);
    let [x, y, z] = [fw(m, cam + 0x2b0), fw(m, cam + 0x2b4), fw(m, cam + 0x2b8)];
    let len = fpu::sqrt(fpu::madd(fpu::madda(fpu::mula(x, x), y, y), z, z));
    m.set(cam + 0x84, len.to_bits());
    let q = vu::div(1.0, len);
    let v = Vf::from_bits(quad(m, cam + 0x2b0));
    put_xyz(m, cam + 0x2b0, v.mul_q(q));
    for k in 0..3 {
        let w = through(m, cam + 0xec + 4 * k, 0);
        m.set(cam + 0xa0 + 4 * k, w);
    }
    m.set(cam + 0xac, ONE);
    let past = fpu::sub(fw(m, cam + 0x84), fw(m, cam + 0x2f0));
    m.set(cam + 0x294, past.to_bits());
    if m.word(cam + 0x308) == 0 {
        let past = F(fw(m, cam + 0x294));
        if past < F::ZERO {
            let least = m.word(cam + 0x2f0);
            m.set(cam + 0x84, least);
            m.set(cam + 0x294, 0);
        } else {
            let most = m.word(cam + 0x2f4);
            let span = fpu::sub(f32::from_bits(most), fw(m, cam + 0x2f0));
            if !(past <= F(span)) {
                m.set(cam + 0x84, most);
                let span = fpu::sub(fw(m, cam + 0x2f4), fw(m, cam + 0x2f0));
                m.set(cam + 0x294, span.to_bits());
            }
        }
    }
    m.set(cam + 0x298, 0);
}

/// **The distances between the second camera's targets**: the table at `+0x104` (ten to a row)
/// cleared, and for every pair of unflagged targets `i`, `j` (`j` from 1) the distance between their
/// spots, the table's other half copied where it has one.
pub fn distances(m: &mut impl Cameras, cam: u32) {
    for k in 0..100 {
        m.set(cam + 0x104 + 4 * k, 0);
    }
    let mut n = m.word(cam + 0x300);
    let mut i = 0u32;
    if 0 < n as i32 {
        loop {
            let mut j = 1u32;
            if 1 < n as i32 {
                loop {
                    if m.word(cam + 4 * i + 0x310) == 0 && m.word(cam + 4 * j + 0x310) == 0 {
                        let ji = cam + j * 0x28 + 4 * i + 0x104;
                        let ij = cam + i * 0x28 + 4 * j + 0x104;
                        let v = m.word(ji);
                        if F(f32::from_bits(v)) != F::ZERO {
                            m.set(ij, v);
                        } else if i != j {
                            let list = m.word(cam + 0x80);
                            let a = {
                                let t = m.word(list + 4 * j);
                                Vf::from_bits(quad(m, t + 0x10))
                            };
                            let list = m.word(cam + 0x80);
                            let b = {
                                let t = m.word(list + 4 * i);
                                Vf::from_bits(quad(m, t + 0x10))
                            };
                            let d = a.sub(b);
                            let [x, y, z] = [d.x(), d.y(), d.z()];
                            let f = fpu::sqrt(fpu::madd(fpu::madda(fpu::mula(x, x), y, y), z, z)).to_bits();
                            m.set(ij, f);
                            m.set(ji, f);
                        }
                    }
                    n = m.word(cam + 0x300);
                    j += 1;
                    if (j as i32) >= n as i32 {
                        break;
                    }
                }
            }
            i += 1;
            if (i as i32) >= n as i32 {
                break;
            }
        }
    }
}

// --- the connector's list -----------------------------------------------------------------------------

/// A list's node at `node` (`[value, prev, next]`) taken out of the list at `list` (`[count, head,
/// tail]`), cleared and freed.
fn unlink(m: &mut impl Cameras, list: u32, node: u32) {
    if node == m.word(list + 4) {
        let next = m.word(node + 8);
        m.set(list + 4, next);
    }
    if node == m.word(list + 8) {
        let prev = m.word(node + 4);
        m.set(list + 8, prev);
    }
    let prev = m.word(node + 4);
    if prev != 0 {
        let next = m.word(node + 8);
        m.set(prev + 8, next);
    }
    let next = m.word(node + 8);
    if next != 0 {
        let prev = m.word(node + 4);
        m.set(next + 4, prev);
    }
    if node != 0 {
        for k in 0..3 {
            m.set(node + 4 * k, 0);
        }
        m.out(Out::Free(node));
    }
    let n = m.word(list).wrapping_sub(1);
    m.set(list, n);
}

/// The first node holding `v` unlinked: `v`, or 0 when none holds it.
fn find_remove(m: &mut impl Cameras, list: u32, v: u32) -> u32 {
    let mut node = m.word(list + 4);
    while node != 0 {
        if m.word(node) == v {
            unlink(m, list, node);
            return v;
        }
        node = m.word(node + 8);
    }
    0
}

/// `v` appended to the list at `list`, its node from `site`.
fn append(m: &mut impl Cameras, list: u32, v: u32, site: Site) -> u32 {
    let node = m.out(Out::Alloc { bytes: 0xc, site });
    if node == 0 {
        return 0;
    }
    m.set(node, v);
    let tail = m.word(list + 8);
    m.set(node + 4, tail);
    m.set(node + 8, 0);
    if m.word(list + 8) == 0 {
        m.set(list + 4, node);
        m.set(list + 8, node);
    } else {
        let tail = m.word(list + 8);
        m.set(tail + 8, node);
        m.set(list + 8, node);
    }
    let n = m.word(list) + 1;
    m.set(list, n);
    1
}

/// `v` put before the node `at`, its node from `site`.
fn insert_before(m: &mut impl Cameras, list: u32, at: u32, v: u32, site: Site) -> u32 {
    let node = m.out(Out::Alloc { bytes: 0xc, site });
    if node == 0 {
        return 0;
    }
    m.set(node, v);
    let prev = m.word(at + 4);
    m.set(node + 4, prev);
    m.set(node + 8, at);
    let prev = m.word(at + 4);
    if prev != 0 {
        m.set(prev + 8, node);
    }
    m.set(at + 4, node);
    if at == m.word(list + 4) {
        m.set(list + 4, node);
    }
    let n = m.word(list) + 1;
    m.set(list, n);
    1
}

/// The `k`th node's value, or 0.
fn nth(m: &mut impl Cameras, list: u32, k: i32) -> u32 {
    let mut node = m.word(list + 4);
    let mut i = 0;
    while i < k {
        if node == 0 {
            return 0;
        }
        i += 1;
        node = m.word(node + 8);
    }
    if node == 0 { 0 } else { m.word(node) }
}

/// Every node freed (its value cleared first) and the list left empty.
fn clear_list(m: &mut impl Cameras, list: u32) {
    m.set(list, 0);
    let mut node = m.word(list + 4);
    while node != 0 {
        m.set(node, 0);
        let next = m.word(node + 8);
        m.out(Out::Free(node));
        node = next;
    }
    m.set(list + 4, 0);
    m.set(list + 8, 0);
}

/// **An interface taken off the connector** (the connector's vt `+0x88`): when on its list, dropped
/// (its vt `+0xc4`, [`drop_item`]).
pub fn unlist(m: &mut impl Cameras, obj: u32, item: u32) {
    if item != 0 && find_remove(m, obj + 0x2c, item) != 0 {
        virt(m, obj, 0xc4, &[item]);
    }
}

/// **An interface dropped** (the connector's vt `+0xc4`): its vt `+0x3c` and `+0x4c`, and its
/// connector cleared.
pub fn drop_item(m: &mut impl Cameras, item: u32) {
    item_virt(m, item, 0x3c, &[]);
    item_virt(m, item, 0x4c, &[]);
    m.set(item + 0x20, 0);
}

/// **An interface put on the connector's sorted list** (the connector's vt `+0x84`): before the first
/// with a greater key (`+0xc`), unless it is on the list already among the lesser or equal; then its
/// connector set (the connector's vt `+0xc0`, [`own`]). Answers 1 when put on.
pub fn list_add(m: &mut impl Cameras, obj: u32, item: u32) -> u32 {
    let mut p = m.word(obj + 0x30);
    while p != 0 {
        if through(m, p, 0xc) as i32 > m.word(item + 0xc) as i32 {
            break;
        }
        if item == m.word(p) {
            return 0;
        }
        p = m.word(p + 8);
    }
    if p == 0 {
        append(m, obj + 0x2c, item, Site::ListAppend);
    } else {
        insert_before(m, obj + 0x2c, p, item, Site::ListInsert);
    }
    virt(m, obj, 0xc0, &[item]);
    1
}

/// **An interface's connector set** (the connector's vt `+0xc0`).
pub fn own(m: &mut impl Cameras, obj: u32, item: u32) {
    m.set(item + 0x20, obj);
}

/// **Every interface on the connector connected** (the connector's vt `+0x94`): each of each one's
/// offers (vt `+0x2c` many, vt `+0x30` each) handed to every interface on the list (vt `+0x40`), and
/// where one takes it, the two connected (the offerer's vt `+0x34`, the taker's vt `+0x44`).
pub fn connect(m: &mut impl Cameras, obj: u32) {
    let mut a = m.word(obj + 0x30);
    while a != 0 {
        let item = m.word(a);
        let n = item_virt(m, item, 0x2c, &[]) as i32;
        let mut k = 0;
        while k < n {
            let out = m.local();
            if item_virt(m, item, 0x30, &[k as u32, out]) != 0 {
                let mut b = m.word(obj + 0x30);
                while b != 0 {
                    let other = m.word(b);
                    let mask = m.word(out);
                    let o = item_virt(m, other, 0x40, &[mask]);
                    if o != 0 {
                        let mask = m.word(out);
                        item_virt(m, item, 0x34, &[mask, o]);
                        let mask = m.word(out);
                        item_virt(m, other, 0x44, &[mask, item]);
                    }
                    b = m.word(b + 8);
                }
            }
            k += 1;
        }
        a = m.word(a + 8);
    }
}

// --- the interfaces ---------------------------------------------------------------------------------

/// An interface's one offer (vt `+0x30`, `base` the object): offer 0 the word at `+off` into `out`.
pub fn offer(m: &mut impl Cameras, base: u32, k: u32, out: u32, off: u32) -> u32 {
    if k != 0 {
        return 0;
    }
    let w = m.word(base + off);
    m.set(out, w);
    1
}

/// The rider's takes (vt `+0x40`): its mask word's offset and the part handed back.
pub const RIDER_TAKES: [(u32, u32); 3] = [(0x9d4, 0x9d0), (0x904, 0x900), (0xa94, 0xa90)];
/// The bike's takes.
pub const BIKE_TAKES: [(u32, u32); 7] = [(0x734, 0x730), (0x9c4, 0x9c0), (0xa94, 0xa90), (0xb54, 0xb50), (0x6b4, 0x6b0), (0x498, 0x494), (0x6a8, 0x6a4)];

/// An offer taken (vt `+0x40`, `base` the object): the part of the first mask the offer meets, or 0.
pub fn take(m: &mut impl Cameras, base: u32, mask: u32, takes: &[(u32, u32)]) -> u32 {
    for &(at, part) in takes {
        if mask & m.word(base + at) != 0 {
            return if base != 0 { base + part } else { 0 };
        }
    }
    0
}

/// **The first camera class connected** (vt `+0x34`): with none yet and the offer its own, `o` taken
/// when it has all of `+0x84`'s bits in its `+0xbc`, and its `+0xb8` into `+0x108`.
pub fn connect_a(m: &mut impl Cameras, cam: u32, mask: u32, o: u32) {
    if m.word(cam + 0x80) != 0 || mask & m.word(cam + 0x7c) == 0 {
        return;
    }
    let want = m.word(cam + 0x84);
    if want & m.word(o + 0xbc) != want {
        return;
    }
    m.set(cam + 0x80, o);
    let w = through(m, cam + 0x80, 0xb8);
    m.set(cam + 0x108, w);
}

/// **The first camera class disconnected** (vt `+0x3c`).
pub fn disconnect_a(m: &mut impl Cameras, cam: u32) {
    m.set(cam + 0x80, 0);
}

/// **The second camera class connected** (vt `+0x34`): with the offer its own, `o` one more target.
pub fn connect_b(m: &mut impl Cameras, cam: u32, mask: u32, o: u32) {
    if mask & m.word(cam + 0x7c) != 0 {
        targets_add(m, cam, o);
    }
}

/// A target added to the second camera's two arrays (`+0x80` the targets, `+0x2a8` their spots),
/// the first made the one with a `+0xc0` (swapped in), and the count `+0x300` and its copy grown.
pub fn targets_add(m: &mut impl Cameras, cam: u32, o: u32) {
    let n = m.word(cam + 0x300);
    let at = m.word(cam + 0x80);
    let a = m.out(Out::Realloc { at, bytes: (n + 1) * 4, site: Site::Targets });
    m.set(cam + 0x80, a);
    let n = m.word(cam + 0x300);
    let at = m.word(cam + 0x80) + 4 * n;
    m.set(at, o);
    let n = m.word(cam + 0x300);
    let at = m.word(cam + 0x2a8);
    let a = m.out(Out::Realloc { at, bytes: (n + 1) * 0x10, site: Site::Spots });
    m.set(cam + 0x2a8, a);
    let n = m.word(cam + 0x300);
    let spot = quad(m, o + 0x10);
    let at = m.word(cam + 0x2a8) + 0x10 * n;
    put_quad(m, at, spot);
    if m.word(cam + 0x300) != 0 && m.word(o + 0xc0) != 0 {
        let list = m.word(cam + 0x80);
        let first = m.word(list);
        let n = m.word(cam + 0x300);
        let last = m.word(list + 4 * n);
        m.set(list, last);
        let n = m.word(cam + 0x300);
        let at = m.word(cam + 0x80) + 4 * n;
        m.set(at, first);
        let spots = m.word(cam + 0x2a8);
        let first = quad(m, spots);
        let n = m.word(cam + 0x300);
        let last = quad(m, spots + 0x10 * n);
        put_quad(m, spots, last);
        let n = m.word(cam + 0x300);
        let at = m.word(cam + 0x2a8) + 0x10 * n;
        put_quad(m, at, first);
    }
    let n = m.word(cam + 0x300) + 1;
    m.set(cam + 0x300, n);
    let n = m.word(cam + 0x300);
    m.set(cam + 0x304, n);
}

/// **The second camera class disconnected** (vt `+0x3c`): its two arrays freed and its count 0.
pub fn targets_drop(m: &mut impl Cameras, cam: u32) {
    for at in [0x80, 0x2a8] {
        let a = m.word(cam + at);
        if a != 0 {
            m.out(Out::Free(a));
        }
    }
    m.set(cam + 0x80, 0);
    m.set(cam + 0x2a8, 0);
    m.set(cam + 0x300, 0);
    let n = m.word(cam + 0x300);
    m.set(cam + 0x304, n);
}

/// **An interface paired** (vt `+0x44`): `mask` into its `+8`, and `(mask, other)` on its pair list
/// (`+0x24`) unless there already.
pub fn pair(m: &mut impl Cameras, item: u32, mask: u32, other: u32) {
    let w = m.word(item + 8) | mask;
    m.set(item + 8, w);
    let mut k = 0;
    while k < m.word(item + 0x24) as i32 {
        let e = nth(m, item + 0x24, k);
        if e != 0 && m.word(e) == mask && m.word(e + 4) == other {
            return;
        }
        k += 1;
    }
    let pr = m.out(Out::New { bytes: 8, site: Site::Pair });
    m.set(pr, mask);
    m.set(pr + 4, other);
    append(m, item + 0x24, pr, Site::PairAppend);
}

/// **An interface's pairs cleared** (vt `+0x4c`): its mask 0, each pair's other told (its vt `+0x38`
/// with the mask) and the pair freed, and the list emptied.
pub fn clear(m: &mut impl Cameras, item: u32) {
    m.set(item + 8, 0);
    let mut node = m.word(item + 0x28);
    while node != 0 {
        let pr = m.word(node);
        let (other, mask) = (m.word(pr + 4), m.word(pr));
        item_virt(m, other, 0x38, &[mask]);
        m.out(Out::Free(pr));
        node = m.word(node + 8);
    }
    clear_list(m, item + 0x24);
}
