//! **The bike's put-back spot, kept every frame** (F31e): the run's update, outside the sim frame,
//! keeps where the bike was last on ground that faces up, and the put-back ([`super::wreck`]) moves
//! the bike there. Where the original keeps them: research/oracle.txt `# F31e`.
//!
//! The bike keeps the spot at `+0xd40` (a point), `+0xd50` (the heading, flat and normalised) and
//! `+0xd60` (the ground's normal there). Its own sphere (`+0xd80`, [`crate::atv_world::VehicleSphere`])
//! goes to the spot first, and gathers its list (`+0xd84`) round it; a spot with something in the way
//! is not kept.
//!
//! - [`frame`]: the run's update as it reaches the bike: the run's heading from the bike's node, then
//!   [`keep`] while the run is in and not out.
//! - [`keep`]: the spot picked (the body's centre of gravity, unless the run or the bike holds
//!   another), the sphere put there ([`place`]), and the spot kept where the ground faces up.
//! - [`place`]: the sphere put at the spot, the ground under it, and the gather round it ([`gather`]).
//!
//! Every word goes through [`Spots`], with its calls out.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use super::node;
use super::wreck::turned;
use ps2_float::vu::Dest;
use ps2_float::{F, Vf, fpu::Acc, vu};

/// A quadword as its four words.
pub type Quad = [u32; 4];

/// The bike's words: its body and node, its actor and run, the ground it rides, the spot it is
/// held at (`+0xc00`, with `+0xc10` set) and whether it keeps none (`+0xc20`), the last up handed
/// (`+0xd70`), the spot kept, the sphere's moved and limit, the sphere, its list, and where it
/// last gathered.
pub const BODY: u32 = 0xb8;
pub const NODE: u32 = 0xe4;
pub const GROUND: u32 = 0x3b4;
pub const RUN: u32 = 0x680;
pub const ACTOR: u32 = 0x684;
pub const HELD_AT: u32 = 0xc00;
pub const HELD: u32 = 0xc10;
pub const NO_KEEP: u32 = 0xc20;
pub const MOVED: u32 = 0xd38;
pub const LIMIT: u32 = 0xd3c;
pub const SPOT: u32 = 0xd40;
pub const HEADING: u32 = 0xd50;
pub const UP: u32 = 0xd60;
pub const LAST_UP: u32 = 0xd70;
pub const SPHERE: u32 = 0xd80;
pub const LIST: u32 = 0xd84;
pub const GATHERED: u32 = 0xd90;

/// The run's flags (a doubleword; every bit here is in its low word): in, out, and the two that
/// make it hand its own spot.
const IN: u32 = 0x2;
const OUT: u32 = 0x1;
const OWN_SPOT: u32 = 0x20_0000;
const HANDED: u32 = 0x40_0000;

/// How far up the ground under a kept spot must face.
const KEEP_UP: f32 = 0.8;
/// The sphere's reach with the rider on and off, and how high its sweep's top is over it.
const REACH: f32 = 40.0;
const OFF_REACH: f32 = 5.0;
const TOP: f32 = 10.0;
/// How near a plant (squared, on the ground plane) is in the way.
const PLANT_NEAR: f32 = 400.0;

/// The keep's outside world: words by address (the bike is `bike`), and its calls.
pub trait Spots {
    fn bike(&self) -> u32;
    fn word(&mut self, at: u32) -> u32;
    fn set(&mut self, at: u32, w: u32);
    /// The ground query (the bike's ground, a point; no record): the point with its `y` on the
    /// ground, and the ground's normal.
    fn ground(&mut self, ground: u32, point: Quad) -> (Quad, Quad);
    /// The sphere filed again in the world's database at its `+0x160` (the spatial move of its entry
    /// `+0x18`, [`super::attach::spatial_move`], half 40 each way, reach 40).
    fn refile(&mut self, sphere: u32);
    /// The list gathered round the sphere (kind 1, static structures only; the list's flat word
    /// set: [`crate::atv_world::gather`]).
    fn gather(&mut self, list: u32, sphere: u32);
    /// The sweep test of the sphere against an object (its collidable), into the sphere's buffers
    /// (`+0x7c`): whether it hit. Not ported.
    fn sweep(&mut self, sphere: u32, object: u32) -> bool;
    /// A plant's part (its index) stood at the plant ([`crate::atv_plant::stand`], which writes the
    /// part), and the bit of the soft foliage parts are not in the way with.
    fn plant_part(&mut self, plant: u32, i: u32) -> u32;
    fn soft_foliage(&mut self) -> u32;
    /// The categories an object out of the way has (a word of the game's), and the ecosystem (the
    /// placed plants' cell size `+0xc4` and models `+0x84`).
    fn mask(&mut self) -> u32;
    fn ecosystem(&mut self) -> u32;
}

fn quad(m: &mut impl Spots, at: u32) -> Quad {
    [0, 4, 8, 12].map(|k| m.word(at + k))
}

fn set_quad(m: &mut impl Spots, at: u32, q: Quad) {
    for (k, w) in q.into_iter().enumerate() {
        m.set(at + 4 * k as u32, w);
    }
}

fn f(m: &mut impl Spots, at: u32) -> F {
    F(f32::from_bits(m.word(at)))
}

fn lanes(q: Quad) -> Vf {
    Vf(q.map(f32::from_bits))
}

fn byte(m: &mut impl Spots, at: u32) -> u32 {
    m.word(at & !3) >> (8 * (at & 3)) & 0xff
}

fn half(m: &mut impl Spots, at: u32) -> u32 {
    m.word(at & !3) >> (8 * (at & 2)) & 0xffff
}

/// The node calls on the keep's words.
struct Nodes<'a, M: Spots>(&'a mut M);

impl<M: Spots> node::Ram for Nodes<'_, M> {
    fn word(&mut self, at: u32) -> u32 {
        self.0.word(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.0.set(at, w);
    }
}

/// **The run's update as it reaches the bike** (the actor's, in its running state, 3): the run's
/// point (`+0x60`) and heading (`+0x120`) are the node's world position and `z` row as they are,
/// and its `+0x130` the `y` row; then, with the run in and not out, the spot kept ([`keep`]: the
/// run's point, heading and `+0x88`).
///
/// The run's other words the update keeps (its velocity and speed, its height over the ground, its
/// clocks), its event post and the actor's other parts are not ported: none reaches the bike.
pub fn frame(m: &mut impl Spots) {
    let b = m.bike();
    let actor = m.word(b + ACTOR);
    let run = m.word(b + RUN);
    if actor == 0 || m.word(actor + 0x58) != 3 || run == 0 {
        return;
    }
    let n = m.word(b + NODE);
    for (to, from) in [(0x60, 0xa0), (0x120, 0x90), (0x130, 0x80)] {
        for k in [0, 4, 8] {
            let w = m.word(n + from + k);
            m.set(run + to + k, w);
        }
    }
    let flags = m.word(run + 8);
    if flags & IN != 0 && flags & OUT == 0 {
        keep(m, run + 0x60, run + 0x120, run + 0x88);
    }
}

/// **The spot kept** (the bike's own virtual, `at` a point, `heading`, `up` a normal or 0), always
/// gathering. With no run, nothing (0). The spot: the run's point where the run holds its own spot,
/// else the bike's held spot while it is held, else the node's position plus the body's centre of
/// gravity (`+0x1a0`) turned by its orientation (`+0x190`, [`turned`]), unless the run hands the
/// spot (`at`, and `up` into `+0xd70` if there is one). Unless the bike keeps none, the sphere is
/// put there ([`place`]), and with nothing in the way and the ground's normal up past
/// [`KEEP_UP`] the spot is kept: the point, the heading (its `y` made 0, then `xyz` normalised by
/// the VU0's `vrsqrt`) and the normal.
pub fn keep(m: &mut impl Spots, at: u32, heading: u32, up: u32) -> bool {
    let b = m.bike();
    let run = m.word(b + RUN);
    if run == 0 {
        return false;
    }
    let flags = m.word(run + 8);
    let spot = if flags & OWN_SPOT != 0 && flags & HANDED != 0 {
        quad(m, run + 0x60)
    } else if m.word(b + HELD) != 0 {
        quad(m, b + HELD_AT)
    } else if flags & HANDED == 0 {
        let body = m.word(b + BODY);
        let (q, cofg) = (lanes(quad(m, body + 0x190)), lanes(quad(m, body + 0x1a0)));
        let off = turned(q, cofg);
        let n = m.word(b + NODE);
        let mut p = [0; 4];
        node::update(&mut Nodes(m), n);
        for (k, w) in p.iter_mut().take(3).enumerate() {
            *w = m.word(n + 0xa0 + 4 * k as u32);
        }
        p[3] = F::ONE.to_bits();
        let s = lanes(p);
        let s = s.merge(Dest::XYZ, s.add(off));
        [s.0[0].to_bits(), s.0[1].to_bits(), s.0[2].to_bits(), p[3]]
    } else {
        if up != 0 {
            for k in [0, 4, 8] {
                let w = m.word(up + k);
                m.set(b + LAST_UP + k, w);
            }
        }
        quad(m, at)
    };
    if m.word(b + NO_KEEP) != 0 {
        return true;
    }
    let (spot, normal, clear) = place(m, spot, heading);
    if clear && !(F(f32::from_bits(normal[1])) <= F(KEEP_UP)) {
        set_quad(m, b + SPOT, spot);
        let h = quad(m, heading);
        let v = lanes([h[0], 0, h[2], h[3]]);
        let sq = v.mul(v);
        let len = vu::madd(vu::add(sq.x(), sq.y()), 1.0, sq.z());
        let n = v.mul_q(vu::rsqrt(1.0, len));
        set_quad(m, b + HEADING, [n.0[0].to_bits(), n.0[1].to_bits(), n.0[2].to_bits(), h[3]]);
        set_quad(m, b + UP, normal);
    }
    true
}

/// **The sphere put at the spot** (the bike's `+0xd80`): its centre the spot, its sweep's top
/// [`TOP`] over it, its reach [`REACH`] ([`OFF_REACH`] with the rider off: the actor in state 2 and
/// the run's `+0x40` set); filed again ([`Spots::refile`]) if the last gather found it past its
/// limit (`!(moved <= limit)`); the ground under the spot, then the gather round it ([`gather`]).
/// The heading's `y` made its absolute. Answers the spot (on the ground, or lifted), the normal and
/// whether nothing was in the way.
pub fn place(m: &mut impl Spots, spot: Quad, heading: u32) -> (Quad, Quad, bool) {
    let b = m.bike();
    let sphere = m.word(b + SPHERE);
    set_quad(m, sphere + 0x160, spot);
    let top = m.word(sphere + 0x148);
    m.set(top, spot[0]);
    m.set(top + 4, (F(TOP) + F(f32::from_bits(spot[1]))).to_bits());
    m.set(top + 8, spot[2]);
    let (actor, run) = (m.word(b + ACTOR), m.word(b + RUN));
    let off = actor != 0 && m.word(actor + 0x58) == 2 && run != 0 && m.word(run + 0x40) != 0;
    m.set(sphere + 0x170, F(if off { OFF_REACH } else { REACH }).to_bits());
    if !(f(m, b + MOVED) <= f(m, b + LIMIT)) {
        m.refile(sphere);
    }
    let g = m.word(b + GROUND);
    let (mut spot, mut normal) = m.ground(g, spot);
    let blocked = gather(m, sphere, &mut spot, &mut normal, off);
    let y = m.word(heading + 4);
    m.set(heading + 4, F(f32::from_bits(y)).abs().to_bits());
    (spot, normal, !blocked)
}

/// **The gather round the sphere**: its sweep's buffers cleared (`+0x7c`'s `+0xa8`, `+0x98`,
/// `+0x94`, `+0xa4`, for its `+0x130` segments); its centre (`+0x50`) and radius (`+0x70`) where it
/// was put and its reach; how far it moved, squared, from where it last gathered (`+0xd38`, a vector
/// unit subtract, then `mula`/`madda`/`madd`), and past the limit the list gathered afresh
/// ([`Spots::gather`]) and the place kept. Then each of the list's candidates within the reach:
///
/// - an object (enabled, none of its categories in [`Spots::mask`]) is in the way, unless the rider
///   is off: then it is swept, and a hit higher than the spot lifts the spot to it and its normal
///   becomes the normal;
/// - a plant's parts are each asked for (which places them), and the plant is in the way if a part
///   not soft foliage is within 20 ft on the ground plane (its cell-unit place times the
///   ecosystem's cell size).
///
/// Answers whether anything was in the way.
pub fn gather(m: &mut impl Spots, sphere: u32, spot: &mut Quad, normal: &mut Quad, off: bool) -> bool {
    let b = m.bike();
    let s = m.word(sphere + 0x7c);
    let n = m.word(sphere + 0x130);
    for (buf, size) in [(0xa8, 4), (0x98, 16), (0x94, 16), (0xa4, 4)] {
        let at = m.word(s + buf);
        for k in (0..n.wrapping_mul(size)).step_by(4) {
            m.set(at + k, 0);
        }
    }
    let c = quad(m, sphere + 0x160);
    set_quad(m, sphere + 0x50, c);
    let r = m.word(sphere + 0x170);
    m.set(sphere + 0x70, r);
    let last = quad(m, b + GATHERED);
    let d: [F; 3] = std::array::from_fn(|i| F(vu::sub(f32::from_bits(c[i]), f32::from_bits(last[i]))));
    let moved = Acc::mula(d[0], d[0]).madda(d[1], d[1]).madd(d[2], d[2]);
    m.set(b + MOVED, moved.to_bits());
    let list = m.word(b + LIST);
    if !(moved <= f(m, b + LIMIT)) {
        m.set(list + 0x3c, 1);
        m.gather(list, sphere);
        m.set(list + 0x3c, 0);
        let c = quad(m, sphere + 0x50);
        set_quad(m, b + GATHERED, c);
    }
    let mut blocked = false;
    let count = m.word(list + 4) as i32;
    for i in 0..count.max(0) as u32 {
        let e = m.word(list) + 8 * i;
        if !(f(m, e + 4) < f(m, sphere + 0x170)) {
            continue;
        }
        let item = m.word(e);
        match byte(m, item + 2) {
            2 => {
                let o = if item != 0 { item - 0x18 } else { 0 };
                if byte(m, o + 0x45) & 1 == 0 || m.word(o + 0x94) & m.mask() != 0 {
                    continue;
                }
                if !off {
                    blocked = true;
                    continue;
                }
                if !m.sweep(sphere, o) {
                    continue;
                }
                let s = m.word(sphere + 0x7c);
                let hit = m.word(s + 0x98);
                let y = f(m, hit + 4);
                if y <= F(f32::from_bits(spot[1])) {
                    continue;
                }
                spot[1] = y.to_bits();
                let at = m.word(s + 0x94);
                *normal = quad(m, at);
            }
            3 => {
                let eco = m.ecosystem();
                let models = m.word(eco + 0x84);
                let kind = byte(m, item + 0x12);
                let model = m.word(models + 4 * kind);
                let parts = m.word(model + 0x194) as i32;
                let soft = m.soft_foliage();
                // Each part is asked for in turn (the ask places the part's spheres), and the
                // distance taken again for each part not soft foliage.
                for p in 0..parts.max(0) as u32 {
                    let part = m.plant_part(item, p);
                    if m.word(part + 0x94) == soft {
                        continue;
                    }
                    let size = f(m, eco + 0xc4);
                    let dx = F::from(half(m, item + 0xc) as i32) * size - f(m, sphere + 0x160);
                    let dz = F::from(half(m, item + 0x10) as i32) * size - f(m, sphere + 0x168);
                    if Acc::mula(dz, dz).madd(dx, dx) <= F(PLANT_NEAR) {
                        blocked = true;
                    }
                }
            }
            _ => {}
        }
    }
    blocked
}
