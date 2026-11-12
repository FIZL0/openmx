// --- the free-ride target seeker (S282a) -------------------------------------------------------------
//
// The seeker runs in the interpreter from ai_tick_flag's first instruction to where it hands on: its
// tail (S282b) or the path branch (S282b), both hooked to return. vt+0x80 and vt+0x94 are the
// original's own reach and hand-off, rand() the original's; the list build is hooked to do nothing, every
// other vtable slot to panic. The world — up to three target sets with pick lists and courses, eight
// targets with per-player done flags, six jumps, and the fixed sequence when there is one — is
// generated around a rider placed near the setup point, the lip, the landing or anywhere, and written
// out the way the original lays it. Compared each tick: what it returned, every field of the seeker,
// the four zeroed controls, every leg of every jump, and the generator's state.

use super::*;

impl Pick {
    /// A vector out to `r` with a `w` in −1..1.
    fn vec_w(&mut self, r: f32) -> Vf {
        let w = self.range(-1.0, 1.0);
        self.vec(r, w)
    }
}

const SK_AI: u32 = 0x0180_0000;
const SK_VT: u32 = 0x0180_1000;
const SK_IFACE: u32 = 0x0180_2000;
const SK_RACE: u32 = 0x0180_3000;
const SK_BIKE: u32 = 0x0180_4000;
const SK_SETS: u32 = 0x0181_0000;
const SK_SET_SIZE: u32 = 0x200;
const SK_LISTS: u32 = 0x0182_0000;
const SK_TARGETS: u32 = 0x0183_0000;
const SK_TARGET_SIZE: u32 = 0x200;
const SK_JUMPS: u32 = 0x0184_0000;
const SK_JUMP_SIZE: u32 = 0x120;
/// Not code: the vtable slots the seeker must not call land here, and panic.
const SK_HOOKS: u32 = 0x0186_0000;
const SK_TAIL: u64 = 0x7a11;
const SK_PATH: u64 = 0x9a7;
const SK_JUMP_COUNT: usize = 6;
const SK_TARGET_COUNT: usize = 8;
const SK_SET_COUNT: usize = 3;
const SK_GROUPS: u32 = 0x0185_0000;
const SK_GROUP_SIZE: u32 = 0x100;
const SK_GROUP_COUNT: usize = 6;
const SK_OWNERS: u32 = 0x0185_1000;
const SK_OWNER_SIZE: u32 = 0x200;
const SK_OWNER_VT: u32 = 0x0185_2000;
/// Not code: the owner's vtable slots land here, and all but the flag question panic.
const SK_OWNER_HOOKS: u32 = 0x0185_3000;
/// The bit the name registry hands out for "HitFreeride" (any one bit does).
const SK_FLAG_BIT: u64 = 0x40;
/// Where an owner keeps its answer to the flag question, for the hook to give back.
const SK_OWNER_ANSWER: u32 = 0xf0;
pub(crate) const SK_ZERO: [u8; 0x4000] = [0; 0x4000];

fn sk_ptr(base: u32, size: u32, i: Option<usize>) -> u32 {
    i.map_or(0, |i| base + size * i as u32)
}

fn sk_index(base: u32, size: u32, a: u32) -> Option<usize> {
    (a != 0).then(|| ((a - base) / size) as usize)
}

pub(crate) fn sk_vf(ee: &ps2_oracle::Ee, at: u32) -> Vf {
    Vf::from_bits(got(ee, at))
}

pub(crate) fn sk_f(ee: &ps2_oracle::Ee, at: u32) -> F {
    F(f32::from_bits(ee.u32_at(at)))
}

/// A list the original reads through a holder: a zero word either side of the entries, so the entry
/// before the first and the one after the last read as no target. Gives the entries' address.
fn sk_list(ee: &mut ps2_oracle::Ee, bump: &mut u32, items: &[Option<usize>]) -> u32 {
    let base = *bump;
    ee.set_u32(base, 0);
    for (k, t) in items.iter().enumerate() {
        ee.set_u32(base + 4 + 4 * k as u32, sk_ptr(SK_TARGETS, SK_TARGET_SIZE, *t));
    }
    ee.set_u32(base + 4 + 4 * items.len() as u32, 0);
    *bump += (8 + 4 * items.len() as u32 + 15) & !15;
    base + 4
}

fn sk_alloc(bump: &mut u32, n: u32) -> u32 {
    let a = *bump;
    *bump += (n + 15) & !15;
    a
}

fn sk_leg(ee: &mut ps2_oracle::Ee, at: u32, l: &sk::Leg) {
    put(ee, at, l.velocity);
    put(ee, at + 0x10, l.aim);
    put(ee, at + 0x20, l.point);
    put(ee, at + 0x30, l.facing);
    ee.set_f32(at + 0x4c, l.learnt.0);
}

fn sk_read_leg(ee: &ps2_oracle::Ee, at: u32) -> sk::Leg {
    sk::Leg {
        velocity: sk_vf(ee, at),
        aim: sk_vf(ee, at + 0x10),
        point: sk_vf(ee, at + 0x20),
        facing: sk_vf(ee, at + 0x30),
        learnt: sk_f(ee, at + 0x4c),
    }
}

/// The leg as bits, so -0 and +0 differ.
fn sk_leg_bits(l: &sk::Leg) -> [u32; 17] {
    let mut out = [0; 17];
    for (k, v) in [l.velocity, l.aim, l.point, l.facing].iter().enumerate() {
        out[4 * k..4 * k + 4].copy_from_slice(&v.to_bits());
    }
    out[16] = l.learnt.0.to_bits();
    out
}

fn sk_seeker_bits(s: &sk::Seeker) -> Vec<(&'static str, u128)> {
    let v = |v: Vf| v.to_bits().iter().enumerate().fold(0u128, |a, (k, b)| a | u128::from(*b) << (32 * k));
    let o = |i: Option<usize>| i.map_or(u128::MAX, |i| i as u128);
    vec![
        ("flags", u128::from(s.flags)),
        ("flags2", u128::from(s.flags2)),
        ("set", o(s.set)),
        ("target", o(s.target)),
        ("last target", o(s.last_target)),
        ("jump", o(s.jump)),
        ("index", u128::from(s.index as u32)),
        ("state", u128::from(s.state as u32)),
        ("tries", u128::from(s.tries as u32)),
        ("tolerance", u128::from(s.tolerance.0.to_bits())),
        ("learnt", u128::from(s.learnt.0.to_bits())),
        ("aim step", u128::from(s.aim_step.0.to_bits())),
        ("speed step", u128::from(s.speed_step.0.to_bits())),
        ("learn step", u128::from(s.learn_step.0.to_bits())),
        ("last miss", v(s.last_miss)),
        ("miss", v(s.miss)),
        ("lead", u128::from(s.lead.0.to_bits())),
        ("distance", u128::from(s.distance.0.to_bits())),
        ("follow", u128::from(s.follow)),
        ("aim", v(s.aim)),
        ("heading", v(s.heading)),
        ("speed", u128::from(s.speed.0.to_bits())),
    ]
}

fn sk_write_seeker(ee: &mut ps2_oracle::Ee, s: &sk::Seeker) {
    ee.write(SK_AI + 0x30, 8, u128::from(s.flags));
    ee.write(SK_AI + 0x38, 8, u128::from(s.flags2));
    ee.set_u32(SK_AI + 0x340, sk_ptr(SK_SETS, SK_SET_SIZE, s.set));
    ee.set_u32(SK_AI + 0x344, sk_ptr(SK_TARGETS, SK_TARGET_SIZE, s.target));
    ee.set_u32(SK_AI + 0x348, sk_ptr(SK_TARGETS, SK_TARGET_SIZE, s.last_target));
    ee.set_u32(SK_AI + 0x34c, sk_ptr(SK_JUMPS, SK_JUMP_SIZE, s.jump));
    ee.set_u32(SK_AI + 0x350, s.index as u32);
    ee.set_u32(SK_AI + 0x354, s.state as u32);
    ee.set_u32(SK_AI + 0x358, s.tries as u32);
    for (at, f) in [
        (0x35c, s.tolerance),
        (0x360, s.learnt),
        (0x364, s.aim_step),
        (0x368, s.speed_step),
        (0x36c, s.learn_step),
        (0x25c, s.lead),
        (0x2fc, s.distance),
        (0x1e0, s.speed),
    ] {
        ee.set_f32(SK_AI + at, f.0);
    }
    put(ee, SK_AI + 0x370, s.last_miss);
    put(ee, SK_AI + 0x380, s.miss);
    ee.set_u32(SK_AI + 0x220, s.follow);
    put(ee, SK_AI + 0x1b0, s.aim);
    put(ee, SK_AI + 0x1d0, s.heading);
}

fn sk_read_seeker(ee: &ps2_oracle::Ee) -> sk::Seeker {
    sk::Seeker {
        flags: ee.read(SK_AI + 0x30, 8) as u64,
        flags2: ee.read(SK_AI + 0x38, 8) as u64,
        set: sk_index(SK_SETS, SK_SET_SIZE, ee.u32_at(SK_AI + 0x340)),
        target: sk_index(SK_TARGETS, SK_TARGET_SIZE, ee.u32_at(SK_AI + 0x344)),
        last_target: sk_index(SK_TARGETS, SK_TARGET_SIZE, ee.u32_at(SK_AI + 0x348)),
        jump: sk_index(SK_JUMPS, SK_JUMP_SIZE, ee.u32_at(SK_AI + 0x34c)),
        index: ee.u32_at(SK_AI + 0x350) as i32,
        state: ee.u32_at(SK_AI + 0x354) as i32,
        tries: ee.u32_at(SK_AI + 0x358) as i32,
        tolerance: sk_f(ee, SK_AI + 0x35c),
        learnt: sk_f(ee, SK_AI + 0x360),
        aim_step: sk_f(ee, SK_AI + 0x364),
        speed_step: sk_f(ee, SK_AI + 0x368),
        learn_step: sk_f(ee, SK_AI + 0x36c),
        last_miss: sk_vf(ee, SK_AI + 0x370),
        miss: sk_vf(ee, SK_AI + 0x380),
        lead: sk_f(ee, SK_AI + 0x25c),
        distance: sk_f(ee, SK_AI + 0x2fc),
        follow: ee.u32_at(SK_AI + 0x220),
        aim: sk_vf(ee, SK_AI + 0x1b0),
        heading: sk_vf(ee, SK_AI + 0x1d0),
        speed: sk_f(ee, SK_AI + 0x1e0),
    }
}

/// Lays the world out the original's way: the targets with their done flags, the jumps, the groups,
/// the sets with their pick lists, courses and owners, and behind the root pointer the set list
/// (`gaps` null entries before each set, and `gaps[sets]` after), the group list and the sequence.
fn sk_write_world(ee: &mut ps2_oracle::Ee, w: &sk::World, gaps: &[u32], seq_on: u32, seq_root: u32) {
    for i in 0..SK_GROUP_COUNT as u32 {
        ee.load(SK_GROUPS + SK_GROUP_SIZE * i, &SK_ZERO[..SK_GROUP_SIZE as usize]);
    }
    for i in 0..SK_SET_COUNT as u32 {
        ee.load(SK_OWNERS + SK_OWNER_SIZE * i, &SK_ZERO[..SK_OWNER_SIZE as usize]);
    }
    let mut bump = SK_LISTS;
    for (i, g) in w.groups.iter().enumerate() {
        let at = SK_GROUPS + SK_GROUP_SIZE * i as u32;
        if let Some(list) = &g.targets {
            let l = sk_alloc(&mut bump, 0x10);
            let items = sk_list(ee, &mut bump, list);
            ee.set_u32(l + 4, list.len() as u32);
            ee.set_u32(l + 0xc, items);
            ee.set_u32(at + 0x48, l);
        }
        // the open bit is bit 0 of a byte per player; the other bits must not count
        let open = sk_alloc(&mut bump, 0x10);
        for (p, o) in g.open.iter().enumerate() {
            ee.write(open + p as u32, 1, if *o { 0xa5 } else { 0x5a });
        }
        ee.set_u32(at + 0x44, open);
        put(ee, at + 0x50, g.point);
        // in play is the word 1 exactly
        let word = if g.active { 1 } else { [0, 2, 0xffff_ffff, 0x101][i % 4] };
        ee.set_u32(at + 0x78, word);
    }
    for (i, t) in w.targets.iter().enumerate() {
        let at = SK_TARGETS + SK_TARGET_SIZE * i as u32;
        ee.set_u32(at + 0x3c, sk_ptr(SK_JUMPS, SK_JUMP_SIZE, t.jump));
        ee.set_u32(at + 0x40, at + 0x80);
        for (p, done) in t.done.iter().enumerate() {
            // other bits of the word around the done bit, which must not count
            let word = if *done { 0x2 | 0x4d } else { 0x4d };
            ee.write(at + 0x80 + 0x30 * p as u32 + 0x20, 2, word);
        }
    }
    for (i, j) in w.jumps.iter().enumerate() {
        let at = SK_JUMPS + SK_JUMP_SIZE * i as u32;
        sk_leg(ee, at, &j.take_off);
        sk_leg(ee, at + 0x60, &j.landing);
        sk_leg(ee, at + 0xc0, &j.setup);
    }
    for (i, s) in w.sets.iter().enumerate() {
        let at = SK_SETS + SK_SET_SIZE * i as u32;
        ee.set_u32(at + 0x11c, sk_ptr(SK_GROUPS, SK_GROUP_SIZE, s.picks));
        // the owner, its model's root node two pointers down, and its answer to the flag question
        let o = SK_OWNERS + SK_OWNER_SIZE * i as u32;
        ee.set_u32(o + 8, SK_OWNER_VT);
        ee.set_u32(o + 0x30, s.owner.player as u32);
        ee.set_u32(o + 0x34, s.owner.kind as u32);
        ee.set_u32(o + 0x3c, o + 0x100);
        ee.set_u32(o + 0x100, o + 0x120);
        ee.set_u32(o + 0x120 + 0x18, o + 0x140);
        put(ee, o + 0x140 + 0x70, s.owner.position);
        // a yes with only high bits set still counts: the original tests all 64
        let yes = if i == 1 { 1u128 << 40 } else { 1 };
        ee.write(o + SK_OWNER_ANSWER, 8, if s.owner.hit_freeride { yes } else { 0 });
        ee.set_u32(at + 0x54, o);
        match &s.course {
            Some(c) => {
                let holder = sk_alloc(&mut bump, 0xa0);
                let l = sk_alloc(&mut bump, 0x10);
                let items = sk_list(ee, &mut bump, &c.targets);
                ee.set_u32(holder + 0x48, l);
                ee.set_f32(holder + 0x94, c.chance.0);
                ee.set_u32(l + 4, c.targets.len() as u32);
                ee.set_u32(l + 0xc, items);
                ee.set_u32(at + 0x114, holder);
            }
            None => ee.set_u32(at + 0x114, 0),
        }
    }
    let x = sk_alloc(&mut bump, 0x50);
    let y = sk_alloc(&mut bump, 0x60);
    let mut entries = Vec::new();
    for i in 0..=w.sets.len() {
        entries.extend((0..gaps[i]).map(|_| 0));
        if i < w.sets.len() {
            entries.push(SK_SETS + SK_SET_SIZE * i as u32);
        }
    }
    let list = sk_alloc(&mut bump, 4 * entries.len() as u32);
    for (k, e) in entries.iter().enumerate() {
        ee.set_u32(list + 4 * k as u32, *e);
    }
    ee.set_u32(x + 0x30, entries.len() as u32);
    ee.set_u32(x + 0x38, list);
    let groups = sk_alloc(&mut bump, 4 * w.groups.len() as u32);
    for i in 0..w.groups.len() {
        ee.set_u32(groups + 4 * i as u32, SK_GROUPS + SK_GROUP_SIZE * i as u32);
    }
    ee.set_u32(x + 0x44, y);
    ee.set_u32(y + 0x30, w.groups.len() as u32);
    ee.set_u32(y + 0x38, groups);
    ee.set_u32(seq_root, x);
    match &w.sequence {
        Some(seq) => {
            let z = sk_alloc(&mut bump, 0x50);
            let g = sk_alloc(&mut bump, 0x40);
            let items = sk_list(ee, &mut bump, seq);
            ee.set_u32(y + 0x50, z);
            ee.set_u32(z + 0x48, g);
            ee.set_u32(g + 0x30, seq.len() as u32);
            ee.set_u32(g + 0x38, items);
            ee.set_u32(seq_on, 1);
        }
        None => ee.set_u32(seq_on, 0),
    }
}

/// Every set's pick list as the original left it.
fn sk_read_picks(ee: &ps2_oracle::Ee, n: usize) -> Vec<Option<usize>> {
    (0..n as u32).map(|i| sk_index(SK_GROUPS, SK_GROUP_SIZE, ee.u32_at(SK_SETS + SK_SET_SIZE * i + 0x11c))).collect()
}

/// The map's groups and every set's owner, around `near`: the owners and the groups' points close
/// enough together that the nearest is often a near thing, and now and then two points the same.
fn sk_gen_groups(p: &mut Pick, targets: &[sk::Target], near: Vf) -> Vec<sk::Group> {
    let n = p.below(SK_GROUP_COUNT as u32 + 1) as usize;
    let mut groups: Vec<sk::Group> = (0..n)
        .map(|_| sk::Group {
            targets: (!p.chance(6)).then(|| sk_gen_list(p, targets, 6, false, 4)),
            point: {
                let r = if p.chance(2) { 40.0 } else { 300.0 };
                sk_offset(near, p.vec_w(r))
            },
            active: p.below(10) < 7,
            open: (0..p.below(5)).map(|_| p.chance(2)).collect(),
        })
        .collect();
    if n >= 2 && p.chance(6) {
        groups[n - 1].point = groups[0].point;
    }
    groups
}

fn sk_gen_owner(p: &mut Pick, near: Vf) -> sk::Owner {
    sk::Owner {
        kind: [0, 1, 1, 1, 2, -1][p.below(6) as usize],
        player: p.below(5) as usize,
        position: {
            let r = if p.chance(2) { 40.0 } else { 300.0 };
            sk_offset(near, p.vec_w(r))
        },
        hit_freeride: p.chance(2),
    }
}

fn sk_gen_gaps(p: &mut Pick) -> Vec<u32> {
    (0..=SK_SET_COUNT).map(|_| if p.chance(5) { 1 + p.below(2) as u32 } else { 0 }).collect()
}

fn sk_write_ride(ee: &mut ps2_oracle::Ee, r: &sk::Ride) {
    put(ee, SK_AI + 0xc0, r.position);
    put(ee, SK_AI + 0x290, r.last_position);
    put(ee, SK_AI + 0xb0, r.forward);
    ee.set_f32(SK_AI + 0x80, r.speed.0);
    ee.set_f32(SK_AI + 0x254, r.min_speed.0);
    ee.set_f32(SK_AI + 0x210, r.reach.base.0);
    ee.set_f32(SK_AI + 0x214, r.reach.max.0);
    ee.set_f32(SK_AI + 0x218, r.reach.gain.0);
    ee.set_u32(SK_AI + 0x40, SK_IFACE);
    ee.set_f32(SK_IFACE + 0x110, r.speed_scale.0);
    ee.set_f32(SK_IFACE + 0x114, r.slow_radius.0);
    ee.set_f32(SK_IFACE + 0x118, r.slow_speed.0);
    ee.set_u32(SK_IFACE + 0x1e4, r.player as u32);
    match r.race_phase {
        Some(phase) => {
            ee.set_u32(SK_RACE + 0x60, phase as u32);
            ee.set_u32(SK_AI + 0xd8, SK_RACE);
        }
        None => ee.set_u32(SK_AI + 0xd8, 0),
    }
    match r.bike {
        Some(b) => {
            ee.write(SK_BIKE + 8, 8, u128::from(b.flags));
            ee.set_f32(SK_BIKE + 0xfc, b.settle.0);
            ee.set_u32(SK_AI + 0xd4, SK_BIKE);
        }
        None => ee.set_u32(SK_AI + 0xd4, 0),
    }
    // the path net is only tested for being there: the branch it starts is hooked
    ee.set_u32(SK_AI + 0xdc, if r.path_net { SK_RACE } else { 0 });
}

/// `v` scaled to a length, on the host: generated data only.
pub(crate) fn sk_along(v: Vf, len: f32) -> Vf {
    let l = (v.x() * v.x() + v.z() * v.z()).sqrt().max(1e-3);
    Vf::new(v.x() / l * len, v.y(), v.z() / l * len, v.w())
}

fn sk_offset(a: Vf, b: Vf) -> Vf {
    Vf::new(a.x() + b.x(), a.y() + b.y(), a.z() + b.z(), a.w())
}

fn sk_gen_jump(p: &mut Pick) -> sk::Jump {
    let setup = Vf::new(p.range(-300.0, 300.0), p.range(-20.0, 20.0), p.range(-300.0, 300.0), p.range(-2.0, 2.0));
    let run = sk_along(p.vec(1.0, 0.0), p.range(30.0, 80.0));
    let lip = sk_offset(Vf::new(setup.x(), p.range(-20.0, 30.0), setup.z(), p.range(-2.0, 2.0)), run);
    let fly = sk_along(sk_offset(sk_along(run, 1.0), p.vec(0.4, 0.0)), p.range(40.0, 120.0));
    let landing = sk_offset(Vf::new(lip.x(), p.range(-20.0, 20.0), lip.z(), p.range(-2.0, 2.0)), fly);
    let velocity = match p.below(5) {
        0 => Vf::new(0.0, 0.0, 0.0, p.range(-1.0, 1.0)),
        1 => p.vec_w(150.0),
        _ => sk_along(Vf::new(fly.x(), p.range(-5.0, 20.0), fly.z(), p.range(-1.0, 1.0)), p.range(10.0, 160.0)),
    };
    let mut aim = sk_offset(lip, p.vec(10.0, 0.0));
    match p.below(10) {
        0 => aim = Vf::new(0.0, aim.y(), aim.z(), aim.w()),
        1 => aim = Vf::new(aim.x(), aim.y(), -0.0, aim.w()),
        _ => {}
    }
    let facing = |p: &mut Pick, along: Vf| {
        if p.chance(4) { p.vec_w(1.0) } else { sk_along(along, p.range(0.5, 1.5)) }
    };
    sk::Jump {
        take_off: sk::Leg {
            velocity,
            aim,
            point: lip,
            facing: facing(p, run),
            learnt: F(if p.chance(6) { 100.0 } else { p.range(-20.0, 115.0) }),
        },
        landing: sk::Leg {
            velocity: if p.chance(4) { Vf::new(0.0, 0.0, 0.0, 0.0) } else { p.vec_w(120.0) },
            aim: p.vec_w(300.0),
            point: landing,
            facing: facing(p, fly),
            learnt: F(p.range(-10.0, 110.0)),
        },
        setup: sk::Leg {
            velocity: if p.chance(3) { p.vec(100.0, 0.0) } else { Vf::new(0.0, 0.0, 0.0, 0.0) },
            aim: p.vec_w(300.0),
            point: setup,
            facing: facing(p, run),
            learnt: F(p.range(-10.0, 110.0)),
        },
    }
}

fn sk_gen_list(p: &mut Pick, targets: &[sk::Target], max: u32, with_jump: bool, none_in: u32) -> Vec<Option<usize>> {
    let ok: Vec<usize> =
        (0..targets.len()).filter(|&t| !with_jump || targets[t].jump.is_some()).collect();
    (0..p.below(max + 1))
        .map(|_| {
            if ok.is_empty() || p.chance(none_in) { None } else { Some(ok[p.below(ok.len() as u32) as usize]) }
        })
        .collect()
}

fn sk_gen(p: &mut Pick) -> (sk::Seeker, sk::Ride, sk::World, Vec<u32>) {
    let jumps: Vec<sk::Jump> = (0..SK_JUMP_COUNT).map(|_| sk_gen_jump(p)).collect();
    let mut targets: Vec<sk::Target> = (0..SK_TARGET_COUNT)
        .map(|_| sk::Target {
            jump: if p.chance(7) { None } else { Some(p.below(SK_JUMP_COUNT as u32) as usize) },
            done: (0..4).map(|_| p.chance(4)).collect(),
        })
        .collect();
    if targets.iter().all(|t| t.jump.is_none()) {
        targets[0].jump = Some(0);
    }
    let near = p.vec_w(300.0);
    let groups = sk_gen_groups(p, &targets, near);
    let sets: Vec<sk::Set> = (0..SK_SET_COUNT)
        .map(|_| sk::Set {
            picks: if groups.is_empty() || p.chance(3) { None } else { Some(p.below(groups.len() as u32) as usize) },
            owner: sk_gen_owner(p, near),
            course: (p.below(10) < 3).then(|| sk::Course {
                targets: sk_gen_list(p, &targets, 6, true, 7),
                chance: F(match p.below(4) {
                    0 => 0.0,
                    1 => 1.0,
                    _ => p.fraction(),
                }),
            }),
        })
        .collect();
    let gaps = sk_gen_gaps(p);
    let sequence = p.chance(4).then(|| {
        let mut seq = sk_gen_list(p, &targets, 6, true, 3);
        if seq.iter().all(|t| t.is_none()) {
            let t = (0..SK_TARGET_COUNT).find(|&t| targets[t].jump.is_some());
            seq.push(t);
        }
        seq
    });
    let world = sk::World { sets, targets, jumps, sequence, groups };

    let sequenced = world.sequence.is_some();
    let set = if sequenced || !p.chance(10) { Some(p.below(SK_SET_COUNT as u32) as usize) } else { None };
    let state = [0, 1, 2, 3, 3, 3][p.below(6) as usize];
    // a rider in the air mostly has its target
    let keep = if state == 3 { 15 } else { 9 };
    let target = if set.is_none() || p.below(20) >= keep { None } else { Some(p.below(SK_TARGET_COUNT as u32) as usize) };
    let jump = match (target.and_then(|t| world.targets[t].jump), p.below(8)) {
        (Some(j), 0..=4) => Some(j),
        (_, 7) => None,
        _ => Some(p.below(SK_JUMP_COUNT as u32) as usize),
    };
    let index = match &world.sequence {
        Some(seq) => p.below(seq.len() as u32),
        None => p.below(8),
    };
    let mut flags = 0u64;
    for (bit, odds) in [(flag::ON_GROUND, 6), (sk::CLOSING, 5), (sk::NO_LEARN, 2), (0x1, 5), (0x80, 5), (0x1_0000_0000, 5)] {
        if p.below(10) < odds {
            flags |= bit;
        }
    }
    let mut flags2 = 0u64;
    for (bit, odds) in [(sk::HEADLONG, 3), (sk::LEG_DONE, 5), (0x1, 5), (0x2_0000_0000, 5)] {
        if p.below(10) < odds {
            flags2 |= bit;
        }
    }
    let last_miss = if p.below(10) < 3 {
        sk::MISS_START
    } else {
        let y = match p.below(4) {
            0 | 1 => 0.0,
            2 => -p.range(0.0, 2.0),
            _ => p.range(0.0, 2.0),
        };
        Vf::new(p.range(-40.0, 40.0), y, p.range(-40.0, 40.0), p.range(-1.0, 1.0))
    };
    let seeker = sk::Seeker {
        flags,
        flags2,
        set,
        target,
        last_target: if p.chance(3) { None } else { Some(p.below(SK_TARGET_COUNT as u32) as usize) },
        jump,
        index,
        state,
        tries: if p.chance(3) { p.below(12) } else { p.below(3) },
        tolerance: F(if p.chance(2) { p.range(0.0, 40.0) } else { p.range(0.0, 8.0) }),
        learnt: F(p.range(-10.0, 110.0)),
        aim_step: F(if p.chance(3) { 0.05 } else { p.range(0.0, 0.5) }),
        speed_step: F(p.range(0.0, 20.0)),
        learn_step: F(p.range(0.0, 20.0)),
        last_miss,
        miss: p.vec_w(50.0),
        lead: F(p.range(0.0, 80.0)),
        distance: F(p.range(0.0, 300.0)),
        follow: p.next() as u32,
        aim: p.vec(300.0, 1.0),
        heading: p.vec(1.0, 0.0),
        speed: F(p.range(0.0, 150.0)),
    };

    // the rider somewhere that sets a state off
    let j = &world.jumps[jump.unwrap_or(p.below(SK_JUMP_COUNT as u32) as usize)];
    let run = sk_along(Vf::new(j.take_off.point.x() - j.setup.point.x(), 0.0, j.take_off.point.z() - j.setup.point.z(), 0.0), 1.0);
    // half the time where the rider's own state moves on
    let kind = match (p.chance(2), seeker.state) {
        (true, 1) => 3,
        (true, 2) => 1,
        (true, 3) => 2,
        _ => p.below(6),
    };
    let near = if p.chance(2) { 12.0 } else { 30.0 };
    let position = match kind {
        0 => sk_offset(j.setup.point, p.vec(25.0, 0.0)),
        1 => sk_offset(j.take_off.point, p.vec(near, 0.0)),
        2 => sk_offset(j.landing.point, p.vec(near, 0.0)),
        3 => sk_offset(j.setup.point, sk_offset(sk_along(run, p.range(-5.0, 25.0)), p.vec(4.0, 0.0))),
        _ => p.vec_w(300.0),
    };
    let step = if p.chance(2) { sk_along(j.landing.facing, p.range(-5.0, 15.0)) } else { p.vec(10.0, 0.0) };
    let mut last_position = Vf::new(position.x() - step.x(), position.y() - step.y(), position.z() - step.z(), p.range(-1.0, 1.0));
    if p.chance(10) {
        // on the landing point: the crossing test's "before" is exactly 0
        last_position = j.landing.point;
    }
    let forward = if p.chance(2) { sk_offset(run, p.vec(0.5, 0.0)) } else { p.vec(1.0, 0.0) };
    let race_phase = (p.below(10) < 3).then(|| [4, 0, 2][p.below(3) as usize]);
    let mut bike = (!p.chance(7)).then(|| sk::Mount {
        flags: [0x0, 0x2, 0x100, 0x102][p.below(4) as usize] | if p.chance(2) { 0x8_0001 } else { 0 },
        settle: F(if p.chance(5) { 0.5 } else { p.range(0.2, 1.0) }),
    });
    if race_phase == Some(4) && bike.is_none() {
        bike = Some(sk::Mount { flags: 0x100, settle: F(1.0) });
    }
    let ride = sk::Ride {
        position,
        last_position,
        forward,
        speed: F(p.range(0.0, 100.0)),
        min_speed: F(p.range(0.0, 60.0)),
        speed_scale: F(match p.below(10) {
            0 => 0.0,
            1..=5 => 1.0,
            _ => p.range(0.0, 2.0),
        }),
        slow_radius: F(p.range(0.0, 150.0)),
        slow_speed: F(p.range(0.0, 60.0)),
        player: p.below(4) as usize,
        reach: if p.chance(2) {
            sk::Reach { base: F(10.0), max: F(60.0), gain: F(0.2) }
        } else {
            sk::Reach { base: F(p.range(0.0, 30.0)), max: F(p.range(0.0, 80.0)), gain: F(p.range(0.0, 1.0)) }
        },
        race_phase,
        bike,
        path_net: p.chance(7),
    };
    (seeker, ride, world, gaps)
}

fn sk_controls() -> ai::Controls {
    let z = F(0.0);
    ai::Controls {
        steer: F(0.25),
        pitch: F(-0.5),
        throttle: F(0.75),
        brake: F(0.125),
        heading: z,
        aim: z,
        roll: z,
        flat_forward: Vf::new(0.0, 0.0, 0.0, 0.0),
        target_speed: z,
    }
}

/// The name registry (it allocates for a name it has not seen) hands out [`SK_FLAG_BIT`], and the
/// owners' vtable answers the flag question from [`SK_OWNER_ANSWER`]; every other owner slot panics.
fn sk_owner_hooks(ee: &mut ps2_oracle::Ee, name_bit: u32) {
    ee.hooks.insert(name_bit, Box::new(|ee| ee.set(2, SK_FLAG_BIT)));
    for slot in (0..0x100).step_by(4) {
        let h = SK_OWNER_HOOKS + slot;
        if slot == 0xac {
            ee.hooks.insert(h, Box::new(|ee| {
                assert_eq!((ee.r(5), ee.r(6)), (SK_FLAG_BIT, 0), "the flag question's arguments");
                let answer = ee.read(ee.r(4) as u32 + SK_OWNER_ANSWER, 8) as u64;
                ee.set(2, answer);
            }));
        } else {
            ee.hooks.insert(h, Box::new(move |_| panic!("the build called the owner's vtable slot {slot:#x}")));
        }
        ee.set_u32(SK_OWNER_VT + slot, h);
    }
}

/// Sets up the hooks and the vtable for the seeker; gives (ai_tick_flag, rand_state, sequence word,
/// sequence root).
fn sk_setup(ee: &mut ps2_oracle::Ee) -> Option<[u32; 4]> {
    let at = named(&[
        "ai_tick_flag",
        "rand_state",
        "ai_seek_reach",
        "ai_seek_hand_off",
        "name_bit",
        "ai_seek_path_branch",
        "ai_seek_tail",
        "ai_seek_sequence_on",
        "ai_seek_sequence_root",
    ])?;
    let [tick, rand_state, reach, hand_off, name_bit, path, tail, seq_on, seq_root] = at[..] else { unreachable!() };
    for slot in (0..0x100).step_by(4) {
        let target = match slot {
            0x80 => reach,
            0x94 => hand_off,
            _ => {
                let h = SK_HOOKS + slot;
                ee.hooks.insert(h, Box::new(move |_| panic!("the seeker called vtable slot {slot:#x}")));
                h
            }
        };
        ee.set_u32(SK_VT + slot, target);
    }
    sk_owner_hooks(ee, name_bit);
    ee.hooks.insert(path, Box::new(|ee| {
        ee.set(2, SK_PATH);
        ee.set(31, ps2_oracle::DONE as i32 as i64 as u64);
    }));
    ee.hooks.insert(tail, Box::new(|ee| {
        ee.set(2, SK_TAIL);
        ee.set(31, ps2_oracle::DONE as i32 as i64 as u64);
    }));
    Some([tick, rand_state, seq_on, seq_root])
}

#[test]
fn the_target_seeker_matches_over_ten_thousand_ticks() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some([tick, rand_state, seq_on, seq_root]) = sk_setup(&mut ee) else { return };

    let mut p = Pick(0x5282_a7a2_93e7_5eed);
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    for case in 0..10_000 {
        let (mut seeker, ride, mut world, gaps) = sk_gen(&mut p);
        let state = p.next();
        let before = (seeker, world.clone());

        for (base, len) in [(SK_AI, 0x1000), (SK_LISTS, 0x4000), (SK_RACE, 0x200), (SK_BIKE, 0x200)] {
            ee.load(base, &SK_ZERO[..len]);
        }
        for i in 0..SK_TARGET_COUNT as u32 {
            ee.load(SK_TARGETS + SK_TARGET_SIZE * i, &SK_ZERO[..SK_TARGET_SIZE as usize]);
        }
        ee.set_u32(SK_AI + 8, SK_VT);
        sk_write_world(&mut ee, &world, &gaps, seq_on, seq_root);
        sk_write_ride(&mut ee, &ride);
        sk_write_seeker(&mut ee, &seeker);
        let mut out = sk_controls();
        for (at, f) in [(0xf0, out.steer), (0xf4, out.pitch), (0xe0, out.throttle), (0xe4, out.brake)] {
            ee.set_f32(SK_AI + at, f.0);
        }
        ee.write(rand_state, 8, u128::from(state));

        ee.call(tick, &[u64::from(SK_AI)], &[1.0 / 30.0], BUDGET);

        let mut rng = Rng::from_state(state);
        let how = sk::seek(&mut seeker, &ride, &mut world, &mut rng, &mut out);
        let what = format!("case {case}: {how:?}, state {} -> {}", before.0.state, seeker.state);
        let want = match how {
            sk::Seek::Still => 1,
            sk::Seek::Lost => 0,
            sk::Seek::Path => SK_PATH,
            sk::Seek::Steered => SK_TAIL,
        };
        assert_eq!(ee.v0() as u32 as u64, want, "{what}: how it ended");
        let theirs = sk_read_seeker(&ee);
        for ((name, a), (_, b)) in sk_seeker_bits(&theirs).iter().zip(sk_seeker_bits(&seeker).iter()) {
            assert_eq!(a, b, "{what}: {name}: original {a:#x} port {b:#x}");
        }
        for (at, f, name) in [(0xf0, out.steer, "steer"), (0xf4, out.pitch, "pitch"), (0xe0, out.throttle, "throttle"), (0xe4, out.brake, "brake")] {
            assert_eq!(ee.u32_at(SK_AI + at), f.0.to_bits(), "{what}: {name}");
        }
        for (i, j) in world.jumps.iter().enumerate() {
            let at = SK_JUMPS + SK_JUMP_SIZE * i as u32;
            for (off, leg, name) in [(0, &j.take_off, "take-off"), (0x60, &j.landing, "landing"), (0xc0, &j.setup, "setup")] {
                assert_eq!(sk_leg_bits(&sk_read_leg(&ee, at + off)), sk_leg_bits(leg), "{what}: jump {i} {name}");
            }
        }
        assert_eq!(ee.read(rand_state, 8) as u64, rng.state(), "{what}: the generator");
        let picks: Vec<_> = world.sets.iter().map(|s| s.picks).collect();
        assert_eq!(sk_read_picks(&ee, world.sets.len()), picks, "{what}: the pick lists");

        // what the tick went through
        let (b, bw) = &before;
        let mut count = |k: &'static str| *seen.entry(k).or_default() += 1;
        count(match how {
            sk::Seek::Still => "still",
            sk::Seek::Lost => "lost",
            sk::Seek::Path => "path",
            sk::Seek::Steered => "steered",
        });
        if rng.state() != state {
            count("drew");
        }
        if seeker.target != b.target && seeker.target.is_some() {
            count(if bw.sequence.is_some() {
                "picked from the sequence"
            } else if b.set.is_some_and(|s| bw.sets[s].course.is_some()) {
                "picked off the course"
            } else {
                "picked off the list"
            });
        }
        match (b.state, seeker.state) {
            (1, 2) => count("1 -> 2"),
            (2, 3) => count("2 -> 3"),
            (3, 0) if b.target.is_some() && seeker.target.is_none() => count("3: dropped"),
            (3, 1) if world.jumps.iter().zip(&bw.jumps).any(|(a, c)| a.take_off.velocity != c.take_off.velocity) => {
                count("3: learnt")
            }
            (3, 1) if seeker.flags2 & sk::LEG_DONE != 0 && b.flags2 & sk::LEG_DONE == 0 => count("3: landed"),
            (3, 1) => count("3 -> 1"),
            _ => {}
        }
        if seeker.speed == F(150.0) && how == sk::Seek::Steered && seeker.state != 0 {
            count("flat out");
        }
        if seeker.flags & sk::CLOSING != b.flags & sk::CLOSING {
            count("closing flipped");
        }
        let halved = seeker.aim_step.0 != b.aim_step.0 || seeker.speed_step.0 != b.speed_step.0;
        if halved && seeker.state == 1 {
            count("steps halved");
        }
        let moved = |a: &sk::Jump, c: &sk::Jump| {
            a.take_off.aim != c.take_off.aim && c.take_off.aim.x() != 0.0 && c.take_off.aim.z() != 0.0
        };
        if b.state == 3 && world.jumps.iter().zip(&bw.jumps).any(|(a, c)| moved(a, c)) {
            count("lip re-aimed");
        }
        if world.sets.iter().zip(&bw.sets).any(|(a, c)| a.picks != c.picks) {
            count(if world.sets[0].picks.is_some() { "built" } else { "built none" });
        }
    }
    eprintln!("target seeker: {seen:?}");
    for (k, n) in [
        ("still", 500),
        ("lost", 200),
        ("path", 200),
        ("steered", 3000),
        ("picked from the sequence", 200),
        ("picked off the course", 200),
        ("picked off the list", 200),
        ("1 -> 2", 100),
        ("2 -> 3", 100),
        ("3: dropped", 100),
        ("3: learnt", 100),
        ("3 -> 1", 50),
        ("flat out", 500),
        ("closing flipped", 500),
        ("steps halved", 30),
        ("lip re-aimed", 100),
        ("built", 100),
        ("built none", 20),
    ] {
        assert!(seen.get(k).copied().unwrap_or(0) >= n, "only {:?} ticks went through {k}", seen.get(k));
    }
}

// The pick-list build (S306) on its own: the sets' owners and the map's groups generated around one
// spot, so the nearest group is often a near thing, with null entries in the set list, groups out of
// play or closed to the player, an owner that asks for open groups and one that does not, no owner of
// kind 1, no groups, and two groups at the same point. The original's build is called on the root;
// every set's pick list is compared.
#[test]
fn the_pick_list_build_matches_over_ten_thousand_worlds() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_seek_list_build", "name_bit", "ai_seek_sequence_on", "ai_seek_sequence_root"]) else { return };
    let [build, name_bit, seq_on, seq_root] = at[..] else { unreachable!() };
    sk_owner_hooks(&mut ee, name_bit);

    let mut p = Pick(0x5306_b1d1_57a7_e5ed);
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    for case in 0..10_000 {
        let targets: Vec<sk::Target> = (0..SK_TARGET_COUNT)
            .map(|t| sk::Target { jump: Some(t % SK_JUMP_COUNT), done: Vec::new() })
            .collect();
        let near = p.vec_w(300.0);
        let groups = sk_gen_groups(&mut p, &targets, near);
        let sets: Vec<sk::Set> = (0..SK_SET_COUNT)
            .map(|_| sk::Set {
                picks: if groups.is_empty() || p.chance(2) { None } else { Some(p.below(groups.len() as u32) as usize) },
                course: None,
                owner: sk_gen_owner(&mut p, near),
            })
            .collect();
        let gaps = sk_gen_gaps(&mut p);
        let mut world = sk::World { sets, targets, jumps: Vec::new(), sequence: None, groups };

        ee.load(SK_LISTS, &SK_ZERO);
        sk_write_world(&mut ee, &world, &gaps, seq_on, seq_root);
        ee.call(build, &[u64::from(ee.u32_at(seq_root))], &[], BUDGET);

        let human = world.sets.iter().find(|s| s.owner.kind == 1).map(|s| s.owner);
        sk::build(&mut world);
        let picks: Vec<_> = world.sets.iter().map(|s| s.picks).collect();
        assert_eq!(sk_read_picks(&ee, world.sets.len()), picks, "case {case}: the pick lists");

        let mut count = |k: &'static str| *seen.entry(k).or_default() += 1;
        count(match (human, picks[0]) {
            (None, _) => "no owner of kind 1",
            (Some(_), None) => "no group",
            (Some(o), Some(_)) if o.hit_freeride => "an open group",
            (Some(_), Some(_)) => "any group in play",
        });
        if let (Some(o), Some(g)) = (human, picks[0]) {
            let skipped_nearer = world.groups.iter().enumerate().any(|(i, x)| {
                i != g && {
                    let d = |v: Vf| (v.x() - o.position.x()).powi(2) + (v.y() - o.position.y()).powi(2) + (v.z() - o.position.z()).powi(2);
                    d(x.point) < d(world.groups[g].point)
                }
            });
            if skipped_nearer {
                count("a nearer group passed over");
            }
        }
    }
    eprintln!("pick-list build: {seen:?}");
    for (k, n) in [
        ("no owner of kind 1", 300),
        ("no group", 300),
        ("an open group", 1000),
        ("any group in play", 1000),
        ("a nearer group passed over", 500),
    ] {
        assert!(seen.get(k).copied().unwrap_or(0) >= n, "only {:?} worlds went through {k}", seen.get(k));
    }
}
