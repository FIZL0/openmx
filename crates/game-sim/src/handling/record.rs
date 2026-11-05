//! **SLUS-21104's bike collision record, its pass before the world steps** (F31b4): what fills the
//! list of contacts the bike's body is pushed by, once a frame, before the physics world's step. The
//! record is the object the sim keeps on its list of colliders (the bike's `+0xc8`); where the
//! original keeps it and its calls: research/oracle.txt `# F31b4`.
//!
//! The sim runs two of the record's virtuals over its list, every record's first and then every
//! record's second:
//!
//! 1. [`clear`]: empty the list and the four touch flags, and give the body back its rest damping when
//!    the record lowers it;
//! 2. [`pass`]: the *sweep back* ([`sweep_back`]), then, with the active part's rays on, the
//!    *heightfield rays* and the *world rays*.
//!
//! It is MX Unleashed's collision record ([`crate::contact_search`]) at SLUS-21104's offsets, with
//! three differences: the heightfield's answer comes back through the terrain service (its surface the
//! corner with the largest weight, [`super::ground::surface`], its depth 0); a world-ray hit raises
//! "touched" and "on an object" even when its category is masked out (`+0x108`), and only an
//! unmasked hit files a contact; a struck mover is filed only when its kind is [`MOVER_KIND`].
//!
//! The probes' searches, the scene node's transforms and the body's damping setters are the outside
//! world's, through [`Collides`]. The terrain query itself is [`super::ground::under`], called through
//! [`Collides::under`].
//!
//! [`held`] and [`held_words`] (F31b5g) read and write the record as its third virtual, the
//! contacts' resolution the gravity calls every substep (vt `+0x88`), holds it:
//! [`crate::atv_contact::Held`].
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use super::ground;
use super::seed::Words;
use crate::chassis::turn;
use crate::contact::{dot, minus, negated, plus, scaled, unit, unturn};
use crate::contact_search::Ground;
use crate::quad::Body;
use ps2_float::{F, Vf};

/// Up, as the record keeps it (`w` 1): the heightfield rays point down against the body's up.
pub const UP: Vf = Vf::new(0.0, 1.0, 0.0, 1.0);
/// The two quadwords a filed contact starts with (its `+0x50`, `+0x60`).
pub const BLANK: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);
/// The surface word of a contact the world rays file: none.
pub const NO_SURFACE: u32 = 0xffff_ffff;
/// A mover's kind (`+0x1a0`) that the record files as a mover; any other is not.
pub const MOVER_KIND: u32 = 2;
/// A contact's size in the record's list.
pub const CONTACT: u32 = 0x90;

/// One filed contact: what it is (0 the record's own, 1 handed to it by another record's ray), the
/// ray (`+0x10`, `+0x20`), the normal (`+0x30`) and point (`+0x40`), the surface word (`+0x80`), the
/// other record (`+0x84`) and the mover (`+0x88`), both 0 for none. `+0x50` and `+0x60` are filed
/// as [`BLANK`]; the rest of the entry is left as it was.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contact {
    pub kind: u32,
    pub from: Vf,
    pub to: Vf,
    pub normal: Vf,
    pub point: Vf,
    pub surface: u32,
    pub other: u32,
    pub mover: u32,
}

/// The collision record, as its two virtuals read and write it. The pointers it holds are kept as
/// what they are to the record: there or not (`body`, `terrain`, `nearby`, `part`, `probe`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    /// `+0x24` the heightfield rays, `+0x28` the world rays, `+0x2c` the rays at all, `+0x30` the
    /// sweep back.
    pub heightfield: bool,
    pub world_rays: bool,
    pub rays: bool,
    pub sweep: bool,
    /// `+0x34` touched anything, `+0x38` another record, `+0x3c` the ground, `+0x40` an object.
    pub touching: bool,
    pub touching_other: bool,
    pub on_ground: bool,
    pub on_object: bool,
    /// `+0x44`: give the body its damping back when the list is emptied; `+0x48`, `+0x50` what to.
    pub damp: bool,
    pub rest_linear: F,
    pub rest_angular: F,
    /// `+0xa0`: the sweep's reference point, in the body's frame.
    pub offset: Vf,
    /// `+0xb0`: how far back the sweep starts, in seconds of the velocity; `+0xb4` how far along the
    /// hit's normal the body is put; `+0xb8` the cosine past which two records' directions count as
    /// flying apart.
    pub lead: F,
    pub push_out: F,
    pub facing: F,
    /// `+0xc0`, `+0xd0`: the last sweep's segment.
    pub swept_from: Vf,
    pub swept_to: Vf,
    /// `+0xe4`: the record has a body (else it rides a chassis, `+0xe0`, through its node `+0xe8`).
    pub body: bool,
    /// `+0xec`: the terrain the heightfield rays ask.
    pub terrain: bool,
    /// `+0xf0`..`+0xf8`: the contacts, at most `capacity` of them.
    pub contacts: Vec<Contact>,
    pub capacity: i32,
    /// `+0xfc`: the near-field list the searches go through (else the global search).
    pub nearby: bool,
    /// `+0x108`: the categories whose world-ray hits are not filed.
    pub mask: u32,
    /// `+0x200` the active part (the ray probe), `+0x204` the sweep's sphere probe.
    pub part: bool,
    pub probe: bool,
    /// `+0x20c`: the last object the world rays filed a contact off. Nothing empties it.
    pub struck: u32,
}

/// The record at `at` in RAM: its flags, tuning, last sweep, and its `+0xf0` contacts.
pub fn read(w: &impl Words, at: u32) -> Record {
    let list = w.word(at + 0xf8);
    let count = w.i(at + 0xf0).max(0) as u32;
    let contacts = (0..count)
        .map(|i| {
            let c = list + CONTACT * i;
            Contact {
                kind: w.word(c),
                from: w.v(c + 0x10),
                to: w.v(c + 0x20),
                normal: w.v(c + 0x30),
                point: w.v(c + 0x40),
                surface: w.word(c + 0x80),
                other: w.word(c + 0x84),
                mover: w.word(c + 0x88),
            }
        })
        .collect();
    Record {
        heightfield: w.b(at + 0x24),
        world_rays: w.b(at + 0x28),
        rays: w.b(at + 0x2c),
        sweep: w.b(at + 0x30),
        touching: w.b(at + 0x34),
        touching_other: w.b(at + 0x38),
        on_ground: w.b(at + 0x3c),
        on_object: w.b(at + 0x40),
        damp: w.b(at + 0x44),
        rest_linear: w.f(at + 0x48),
        rest_angular: w.f(at + 0x50),
        offset: w.v(at + 0xa0),
        lead: w.f(at + 0xb0),
        push_out: w.f(at + 0xb4),
        facing: w.f(at + 0xb8),
        swept_from: w.v(at + 0xc0),
        swept_to: w.v(at + 0xd0),
        body: w.b(at + 0xe4),
        terrain: w.b(at + 0xec),
        contacts,
        capacity: w.i(at + 0xf4),
        nearby: w.b(at + 0xfc),
        mask: w.word(at + 0x108),
        part: w.b(at + 0x200),
        probe: w.b(at + 0x204),
        struck: w.word(at + 0x20c),
    }
}

/// The words the two virtuals write, address and value, for the record `r` at `at` whose list is at
/// `list` (`+0xf8`): the touch flags, the last sweep, the count and each contact's filed words, and
/// the struck object.
pub fn words(r: &Record, at: u32, list: u32) -> Vec<(u32, u32)> {
    let b = u32::from;
    let mut out = vec![
        (at + 0x34, b(r.touching)),
        (at + 0x38, b(r.touching_other)),
        (at + 0x3c, b(r.on_ground)),
        (at + 0x40, b(r.on_object)),
        (at + 0xf0, r.contacts.len() as u32),
        (at + 0x20c, r.struck),
    ];
    let mut v = |a: u32, x: Vf| out.extend(x.to_bits().into_iter().enumerate().map(|(k, w)| (a + 4 * k as u32, w)));
    v(at + 0xc0, r.swept_from);
    v(at + 0xd0, r.swept_to);
    for (i, c) in r.contacts.iter().enumerate() {
        let e = list + CONTACT * i as u32;
        v(e + 0x10, c.from);
        v(e + 0x20, c.to);
        v(e + 0x30, c.normal);
        v(e + 0x40, c.point);
        v(e + 0x50, BLANK);
        v(e + 0x60, BLANK);
    }
    for (i, c) in r.contacts.iter().enumerate() {
        let e = list + CONTACT * i as u32;
        out.extend([(e, c.kind), (e + 0x80, c.surface), (e + 0x84, c.other), (e + 0x88, c.mover)]);
    }
    out
}

/// One contact onto the list, if there is room (`count < capacity`, signed).
pub fn file(rec: &mut Record, c: Contact) {
    if (rec.contacts.len() as i32) < rec.capacity {
        rec.contacts.push(c);
    }
}

/// What a probe struck: the object (`0` for none), its record (`+0x90`) and its mover (`+0x8c`, only
/// when the mover's kind is [`MOVER_KIND`]), `0` for none; and its category (`+0x94`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Struck {
    pub object: u32,
    pub other: u32,
    pub mover: u32,
    pub category: u32,
}

/// One of the ray probe's hits (its record `+0x7c`): the ray (`+0x90`, two quadwords a hit), the
/// normal (`+0x94`), the point (`+0x98`) and what was struck (`+0x9c`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hit {
    pub from: Vf,
    pub to: Vf,
    pub normal: Vf,
    pub point: Vf,
    pub struck: Struck,
}

/// The sphere probe's answer when its hit bit is up: its record's first normal and point, and its
/// first object (read whatever the record's count).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Swept {
    pub normal: Vf,
    pub point: Vf,
    pub struck: Struck,
}

/// The record's outside: its node, its probes and their searches, the terrain, the other records.
pub trait Collides {
    /// The body's linear and angular damping set (its virtuals `+0x20` and `+0x68`), `(d, d, d, 1)`.
    fn damp(&mut self, linear: Vf, angular: Vf);
    /// The node (`+0xe8`): a direction turned into the world, its position, a point placed into the
    /// world, and its position set.
    fn node_rotate(&mut self, v: Vf) -> Vf;
    fn node_position(&mut self) -> Vf;
    fn node_place(&mut self, v: Vf) -> Vf;
    fn node_move(&mut self, at: Vf);
    /// The node's rows' `y` (its `+0x74`, `+0x84`, `+0x94`), read as they are: the up of a record
    /// with no body.
    fn node_up(&self) -> [F; 3];
    /// The chassis' velocity (`+0xe0`'s `+0x250`), for a record with no body.
    fn chassis_velocity(&self) -> Vf;
    /// The sphere probe set to the segment, placed and searched (near-field or global); its answer
    /// when its hit bit (`+0x126` bit 5) is up.
    fn sweep(&mut self, from: Vf, to: Vf, nearby: bool) -> Option<Swept>;
    /// The active part placed again at the node, after the sweep moved the body.
    fn place_part(&mut self);
    /// The active part's rays (`+0x178` of them at `+0x180`): start and end, in the node's frame.
    fn part_rays(&self) -> Vec<[Vf; 2]>;
    /// The active part searched (near-field or global), and its hits in its order.
    fn search(&mut self, nearby: bool) -> Vec<Hit>;
    /// The terrain's box (`+0x30`, `+0x40`; `x` and `z` are read).
    fn terrain_box(&self) -> [Vf; 2];
    /// The terrain query on the buffer ([`ground::under`]).
    fn under(&mut self, g: &mut Ground);
    /// The velocity of what the sweep struck: its record's body's (`+0x80`) or chassis' (`+0x250`),
    /// or, with no record, its mover's (the mover's virtual `+0xc0`).
    fn velocity(&mut self, s: &Struck) -> Vf;
    /// Another record, by what [`Struck::other`] holds.
    fn other(&mut self, id: u32) -> &mut Record;
}

/// The record's first virtual (`+0x7c`): the list and the touch flags emptied, and with a body and
/// [`Record::damp`] the body's damping given back.
pub fn clear(rec: &mut Record, w: &mut impl Collides) {
    rec.contacts.clear();
    rec.touching = false;
    rec.on_ground = false;
    rec.on_object = false;
    rec.touching_other = false;
    if rec.body && rec.damp {
        let (l, a) = (rec.rest_linear.0, rec.rest_angular.0);
        w.damp(Vf::new(l, l, l, 1.0), Vf::new(a, a, a, 1.0));
    }
}

/// The sweep back (the record's virtual `+0x80`): a segment from where the reference point was `lead`
/// seconds of velocity ago to where it is, searched with the sphere probe. On a hit the body (and the
/// node) is put out along the hit's normal, unless what was struck is another record or a mover and
/// the two are flying apart (both faster than 1, their directions' cosine past [`Record::facing`]).
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn sweep_back(rec: &mut Record, mut body: Option<&mut Body>, dt: F, w: &mut impl Collides) {
    if !rec.sweep || !rec.probe {
        return;
    }
    let (r, ours) = match body.as_deref() {
        Some(b) => {
            let r = turn(b.orientation, rec.offset);
            rec.swept_to = plus(b.position, r);
            (r, b.velocity)
        }
        None => {
            let r = w.node_rotate(rec.offset);
            rec.swept_to = plus(w.node_position(), r);
            (r, w.chassis_velocity())
        }
    };
    let back = scaled(scaled(ours, rec.lead), dt);
    rec.swept_from = minus(rec.swept_to, back);
    let Some(hit) = w.sweep(rec.swept_from, rec.swept_to, rec.nearby) else {
        return;
    };
    let s = hit.struck;
    if s.other != 0 || s.mover != 0 {
        let theirs = w.velocity(&s);
        if !(dot(ours, ours).sqrt() <= F::ONE) && !(dot(theirs, theirs).sqrt() <= F::ONE) {
            let (ours, theirs) = (unit(ours), unit(theirs));
            if !(dot(ours, theirs) <= rec.facing) {
                return;
            }
        }
    }
    let at = minus(plus(hit.point, scaled(hit.normal, rec.push_out)), r);
    if let Some(b) = body.as_deref_mut() {
        b.position = at;
    }
    w.node_move(at);
    if rec.part {
        w.place_part();
    }
    rec.swept_to = at;
}

/// The record's second virtual (`+0x84`): the sweep back, then with the active part and its rays on,
/// the heightfield rays (each of the part's rays pointing down against the up whose end, placed by
/// the node, is inside the terrain's box: a contact where the end is under the ground) and the world
/// rays (the part's search: a contact per unmasked hit, and one handed back to a struck record with
/// the normal turned round). `me` is what a contact handed to another record names this one by.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn pass(rec: &mut Record, me: u32, mut body: Option<&mut Body>, dt: F, w: &mut impl Collides) {
    sweep_back(rec, body.as_deref_mut(), dt, w);
    if !rec.part || !rec.rays {
        return;
    }
    if rec.heightfield && rec.terrain {
        let up = match body.as_deref() {
            Some(b) => unturn(b.orientation, UP),
            None => {
                let [x, y, z] = w.node_up();
                Vf::new(x.0, y.0, z.0, 1.0)
            }
        };
        for [start, end] in w.part_rays() {
            if !(dot(minus(end, start), up) < F::ZERO) {
                continue;
            }
            let tip = w.node_place(end);
            let [lo, hi] = w.terrain_box();
            let inside = tip.x() < hi.x() && !(tip.x() <= lo.x()) && tip.z() < hi.z() && !(tip.z() <= lo.z());
            if !inside {
                continue;
            }
            let mut g = Ground { point: tip, weights: Vf::new(1.0, 1.0, 1.0, 1.0), ..Ground::default() };
            w.under(&mut g);
            let surface = u32::from(ground::surface(&g));
            let point = minus(g.point, scaled(g.normal, F::ZERO));
            if !(point.y() <= tip.y()) {
                rec.touching = true;
                rec.on_ground = true;
                file(rec, Contact { kind: 0, from: start, to: end, normal: g.normal, point, surface, other: 0, mover: 0 });
            }
        }
    }
    if !rec.world_rays {
        return;
    }
    for hit in w.search(rec.nearby) {
        rec.touching = true;
        rec.on_object = true;
        let s = hit.struck;
        if s.category & rec.mask != 0 {
            continue;
        }
        let (from, to, point) = (hit.from, hit.to, hit.point);
        file(rec, Contact { kind: 0, from, to, normal: hit.normal, point, surface: NO_SURFACE, other: s.other, mover: s.mover });
        rec.struck = s.object;
        if s.other != 0 {
            rec.touching_other = true;
            // A hit on the record's own object hands the contact back to the record itself.
            let o = if s.other == me { &mut *rec } else { w.other(s.other) };
            o.touching = true;
            o.on_object = true;
            o.touching_other = true;
            let normal = negated(hit.normal);
            file(o, Contact { kind: 1, from, to, normal, point, surface: NO_SURFACE, other: me, mover: 0 });
        }
    }
}

/// The resolution's fields of the record (vt `+0x88`, [`crate::atv_contact::resolve`]; research/
/// oracle.txt `# MD75e1`): the damping it lowers to, the mass and inertia it shares, its centre of
/// mass (`+0x80`) and that turned into the world (`+0x90`), the collidable flags, the deepest
/// contact, the scenery's and the actors' responses, the friction by surface, the last contact's
/// normal speed (`+0x1e8`) and relative velocity (`+0x1f0`), the own impulse's mask (`+0x150`).
fn collider(w: &impl Words, at: u32) -> crate::atv_contact::Record {
    use crate::contact::{Collider, Response};
    let response = |s: u32, angular: u32, spring: u32, friction_mask: u32, friction: u32| Response {
        stiffness: w.f(at + s),
        damping: w.f(at + s + 4),
        restitution: w.f(at + s + 8),
        angular: w.b(at + angular),
        friction: w.f(at + friction),
        spring_mask: w.v(at + spring),
        friction_mask: w.v(at + friction_mask),
    };
    let c = w.word(at + 0x208);
    let collider = Collider {
        damp: w.b(at + 0x44),
        damp_linear: w.f(at + 0x4c),
        damp_angular: w.f(at + 0x54),
        mass: w.f(at + 0x58),
        inv_mass: w.f(at + 0x5c),
        inv_inertia: w.v(at + 0x70),
        cofg: w.v(at + 0x80),
        cofg_world: w.v(at + 0x90),
        actors: w.word(at + 0x100),
        collides_with: w.word(at + 0x104),
        collidable: (c != 0).then(|| w.word(c + 0x94)),
        max_depth: w.f(at + 0x10c),
        scenery: response(0x118, 0x13c, 0x160, 0x170, 0x1a0),
        actor: response(0x130, 0x140, 0x180, 0x190, 0x1a4),
        surfaces: core::array::from_fn(|k| w.f(at + 0x1a8 + 4 * k as u32)),
        normal_speed: w.f(at + 0x1e8),
        relative: w.v(at + 0x1f0),
    };
    crate::atv_contact::Record { collider, impulse_mask: w.v(at + 0x150) }
}

/// **The record at `at` as its resolution holds it** (F31b5g; the gravity's call, vt `+0x88` handed
/// 1, answered by [`crate::atv_contact::Held::resolve`]): its fields, its `+0xf0` contacts, the
/// body at `+0xe4` whole and its mass (`+0x18`). `None` when a contact is with another record
/// (`+0x84`) or a mover (`+0x88`): the mover's velocity and spin are its virtuals', and another
/// record is not held; the call is then not answered. A record with no body is held with none, and
/// [`crate::atv_contact::Held::resolve`] does not answer it either.
pub fn held(w: &impl Words, at: u32) -> Option<crate::atv_contact::Held> {
    let list = w.word(at + 0xf8);
    let mut entries = Vec::new();
    for i in 0..w.i(at + 0xf0).max(0) as u32 {
        let e = list + CONTACT * i;
        if w.word(e + 0x84) != 0 || w.word(e + 0x88) != 0 {
            return None;
        }
        let contact = crate::contact::Contact {
            kind: w.word(e),
            from: w.v(e + 0x10),
            on_body: w.v(e + 0x20),
            normal: w.v(e + 0x30),
            point: w.v(e + 0x40),
            arm: w.v(e + 0x50),
            other_arm: w.v(e + 0x60),
            surface: w.i(e + 0x80),
            other: None,
            mover: None,
        };
        entries.push(crate::atv_contact::Entry { contact, relative: w.v(e + 0x70) });
    }
    let b = w.word(at + 0xe4);
    Some(crate::atv_contact::Held {
        record: collider(w, at),
        entries,
        movers: Vec::new(),
        mass: if b != 0 { w.f(b + 0x18) } else { F::ZERO },
        body: (b != 0).then(|| super::seed::body(w, b)),
    })
}

/// [`held`]'s words the resolution writes, address and value, for the record at `at` (its list and
/// body pointers read off `w`): the centre of mass in the world, the normal speed and the relative
/// velocity; each contact's two arms and its relative velocity; and every word of the body
/// [`super::seed::body`] reads.
pub fn held_words(h: &crate::atv_contact::Held, at: u32, w: &impl Words) -> Vec<(u32, u32)> {
    let mut out = vec![(at + 0x1e8, h.record.collider.normal_speed.to_bits())];
    let mut v = |a: u32, x: Vf| out.extend(x.to_bits().into_iter().enumerate().map(|(k, x)| (a + 4 * k as u32, x)));
    v(at + 0x90, h.record.collider.cofg_world);
    v(at + 0x1f0, h.record.collider.relative);
    let list = w.word(at + 0xf8);
    for (i, e) in h.entries.iter().enumerate() {
        let a = list + CONTACT * i as u32;
        v(a + 0x50, e.contact.arm);
        v(a + 0x60, e.contact.other_arm);
        v(a + 0x70, e.relative);
    }
    if let Some(b) = &h.body {
        out.extend(super::seed::body_words(b, w.word(at + 0xe4)));
    }
    out
}
