// MD128 — what SLUS-21104's own calls hand out in turn, unhooked: the map-edge test and throw, the
// body's reset, the spatial update, the node's bounds, the collision sync, the record's gravity call
// and the subtree's marking. MD106b's test (`own_calls.rs`) runs with these unhooked, and their own
// calls out hooked here instead.

use super::*;
use game_sim::atv_moto::Collidables;
use game_sim::atv_sync::{Collidable, Kind, slot};
use game_sim::quad::Part;
use game_sim::rng::Rng as GameRng;

/// What MD128's calls hand out in turn: the game's random numbers (the throw's), the spatial
/// database's move, the nearby list's gather, the world's three list removals, the riding probe's
/// owner cleared and its list added to, and the record handed back (research/oracle.txt `# MD128`).
const RAND: u32 = 0x0035_61a8;
pub(crate) const SPATIAL_MOVE: u32 = 0x0018_d690;
pub(crate) const GATHER: u32 = 0x001d_6a60;
const LEAVE_WORLD: [u32; 3] = [0x0049_5170, 0x0049_5030, 0x0049_4db0];
const RIDING_OWNER: u32 = 0x0011_5890;
const RIDING_ADD: u32 = 0x0011_58a0;
const RELEASE: u32 = 0x0020_3f40;
/// The two collidable classes' vtables: the ray probe and the mesh.
const PART_VTABLE: u32 = 0x003b_6c20;
const MESH_VTABLE: u32 = 0x003b_6d00;

/// The words MD128's calls keep: the car's (its database entry, the flag word, the post element the
/// world lists, the reset byte, the vehicle state's word the world lists, the actors the gather reads,
/// the three collidables, the record's owner, the physics world, the terrain), the terrain's extent,
/// the record's contact count, the world's category word, the physics world's three lists.
mod hand {
    pub const ENTRY: u32 = 8;
    pub const FLAG_WORD: u32 = 0x24;
    pub const POST: u32 = 0x34;
    pub const RESET_BYTE: u32 = 0x44;
    pub const SECOND: u32 = 0xbc;
    pub const ACTORS: u32 = 0xc4;
    pub const RIDING: u32 = 0xcc;
    pub const FRAME: u32 = 0xd0;
    pub const ACTIVE: u32 = 0xd8;
    pub const OWNER: u32 = 0xdc;
    pub const PHYSICS: u32 = 0xe0;
    pub const TERRAIN: u32 = 0x3b0;
    pub const EXTENT: u32 = 0x43c;
    pub const WIDTH: u32 = 0x40;
    pub const DEPTH: u32 = 0x48;
    pub const COUNT: u32 = 0xf0;
    pub const CATEGORY: u32 = 0xa4;
    pub const LISTS: [u32; 3] = [0x50, 0x68, 0xb0];
    /// A collidable's words: its entry, centre, reach, frame; a mesh's stood and before frames.
    pub const PART_ENTRY: u32 = 0x20;
    pub const PART_CENTRE: u32 = 0x50;
    pub const PART_REACH: u32 = 0x70;
    pub const PART_FRAME: u32 = 0x130;
    pub const STOOD: u32 = 0x1b0;
    pub const BEFORE: u32 = 0x1f0;
    /// A scene node's database words: its entry, box centre and half extent, second flag byte, reach.
    pub const NODE_ENTRY: u32 = 8;
    pub const NODE_CENTRE: u32 = 0xd0;
    pub const NODE_HALF: u32 = 0xe0;
    pub const NODE_REACH: u32 = 0x108;
}

/// The image's car's MD128 pointers: the terrain's extent, the riding probe and the frame mesh, the
/// node's and the collidables' database entries (laid back when a generated state has one), the
/// record's owner, and the node's subtree, its first child first.
pub(crate) struct HandPtrs {
    extent: u32,
    parts: [(Part, u32); 2],
    node_entry: u32,
    part_entries: [u32; 2],
    owner: u32,
    below: Vec<u32>,
}

pub(crate) fn hand_ptrs(ee: &Ee, p: &Ptrs) -> HandPtrs {
    let terrain = ee.u32_at(car_at() + hand::TERRAIN);
    let parts = [(Part::Riding, ee.u32_at(car_at() + hand::RIDING)), (Part::Frame, ee.u32_at(car_at() + hand::FRAME))];
    let mut below = Vec::new();
    let mut todo = vec![ee.u32_at(p.node + 0xf4)];
    while let Some(n) = todo.pop() {
        if n == 0 {
            continue;
        }
        assert!(!below.contains(&n), "a loop in the subtree");
        below.push(n);
        todo.push(ee.u32_at(n + 0xf8));
        todo.push(ee.u32_at(n + 0xf4));
    }
    HandPtrs {
        extent: if terrain == 0 { 0 } else { ee.u32_at(terrain + hand::EXTENT) },
        parts,
        node_entry: ee.u32_at(p.node + hand::NODE_ENTRY),
        part_entries: parts.map(|(_, a)| ee.u32_at(a + hand::PART_ENTRY)),
        owner: ee.u32_at(car_at() + hand::OWNER),
        below,
    }
}

/// A collidable as the sync sees it (as `tests/atv_quad/image.rs` reads one).
fn collidable(ee: &Ee, a: u32) -> Collidable {
    let xyz = |a: u32| Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), 0.0);
    let rows = |a: u32| std::array::from_fn(|k| get_v(ee, a + 0x10 * k as u32));
    let kind = match ee.u32_at(a + 0xc) {
        PART_VTABLE => {
            let (tree, bound) = (ee.u32_at(a + 0x17c), ee.u32_at(a + 0x180));
            Kind::Rays { tree: (tree != 0).then(|| (xyz(tree + 0xc), xyz(tree + 0x18))), bound: (get_v(ee, bound), get_v(ee, bound + 0x10)) }
        }
        MESH_VTABLE => {
            let mesh = ee.u32_at(a + 0x244);
            Kind::Mesh { local: rows(a + 0x170), centre: xyz(mesh + 0xc), half: xyz(mesh + 0x18), stood: rows(a + hand::STOOD), before: rows(a + hand::BEFORE) }
        }
        vt => panic!("a collidable at {a:#x} of vtable {vt:#010x}"),
    };
    Collidable {
        kind,
        frame: rows(a + hand::PART_FRAME),
        centre: get_v(ee, a + hand::PART_CENTRE),
        reach: F(ee.f32_at(a + hand::PART_REACH)),
        spatial: ee.u32_at(a + hand::PART_ENTRY) != 0,
    }
}

/// MD128's words of the vehicle read out of RAM, into a [`read_vehicle`]'s.
pub(crate) fn read_hand_outs(ee: &Ee, p: &Ptrs, o: &OwnPtrs, v: &mut atv_car::Vehicle) {
    let h = hand_ptrs(ee, p);
    let reset = ee.u8_at(car_at() + hand::RESET_BYTE);
    assert_eq!(h.extent, o.area, "the world's box is not the terrain's extent ([`generate_hand_outs`] draws one for both)");
    v.area = (h.extent != 0).then(|| game_sim::biplane::Area {
        width: F(ee.f32_at(h.extent + hand::WIDTH)),
        depth: F(ee.f32_at(h.extent + hand::DEPTH)),
    });
    v.spatial = ee.u32_at(car_at() + hand::ENTRY) != 0;
    let part = |a: u32| h.parts.iter().find(|&&(_, x)| a != 0 && x == a).map(|&(q, _)| q);
    let mut c = Collidables {
        sync: reset & 1 != 0,
        riding: part(ee.u32_at(car_at() + hand::RIDING)),
        frame: part(ee.u32_at(car_at() + hand::FRAME)),
        active: part(ee.u32_at(car_at() + hand::ACTIVE)),
        ..Default::default()
    };
    for &(q, a) in &h.parts {
        c.states[slot(q)] = collidable(ee, a);
    }
    v.collidables = c;
    v.reset_flags = reset & !1;
    v.flag_word = ee.u32_at(car_at() + hand::FLAG_WORD);
    v.owned = h.owner != 0;
    // Only an empty list is read: the resolution's contacts are not this test's (MD75e1's).
    v.held = (ee.u32_at(p.record + hand::COUNT) == 0).then(game_sim::atv_contact::Held::default);
    let index = |a: u32| h.below.iter().position(|&n| n == a && a != 0);
    v.below = h
        .below
        .iter()
        .map(|&n| atv_car::Below { child: index(ee.u32_at(n + 0xf4)), sibling: index(ee.u32_at(n + 0xf8)), flags: ee.u8_at(n + 0x100) })
        .collect();
    for (n, &a) in v.scene.nodes.iter_mut().zip(&o.nodes) {
        n.spatial = ee.u32_at(a + hand::NODE_ENTRY) != 0;
        n.centre = get_v(ee, a + hand::NODE_CENTRE);
        n.half = get_v(ee, a + hand::NODE_HALF);
        n.more = ee.u8_at(a + 0x101);
        n.reach = F(ee.f32_at(a + hand::NODE_REACH));
    }
}

/// Every word [`read_hand_outs`] reads but the reset byte and the nodes' second flag byte (which
/// [`vehicle_words`] lays with the bytes beside them), where it lives; entries the port holds as
/// switches take the image's (`ee`'s) pointer back.
pub(crate) fn hand_words(ee: &Ee, v: &atv_car::Vehicle, p: &Ptrs, o: &OwnPtrs) -> Vec<(u32, u32)> {
    let h = hand_ptrs(ee, p);
    let mut out = Vec::new();
    let mut vs: Vec<(u32, Vf)> = Vec::new();
    if let Some(a) = v.area {
        out.push((h.extent + hand::WIDTH, a.width.to_bits()));
        out.push((h.extent + hand::DEPTH, a.depth.to_bits()));
    }
    // The car's own entry: the image has none, so a generated one is the node's (the hooked move
    // is all that reads it).
    let entry = match ee.u32_at(car_at() + hand::ENTRY) {
        0 => h.node_entry,
        e => e,
    };
    out.push((car_at() + hand::ENTRY, if v.spatial { entry } else { 0 }));
    out.push((car_at() + hand::FLAG_WORD, v.flag_word));
    out.push((car_at() + hand::OWNER, if v.owned { h.owner } else { 0 }));
    let at = |q: Option<Part>| q.map_or(0, |q| h.parts.iter().find(|&&(x, _)| x == q).expect("a held part").1);
    let c = &v.collidables;
    out.push((car_at() + hand::RIDING, at(c.riding)));
    out.push((car_at() + hand::FRAME, at(c.frame)));
    out.push((car_at() + hand::ACTIVE, at(c.active)));
    for (k, &(q, a)) in h.parts.iter().enumerate() {
        let s = &c.states[slot(q)];
        out.push((a + hand::PART_ENTRY, if s.spatial { h.part_entries[k] } else { 0 }));
        out.push((a + hand::PART_REACH, s.reach.to_bits()));
        vs.push((a + hand::PART_CENTRE, s.centre));
        let mut rows = vec![(a + hand::PART_FRAME, s.frame)];
        if let Kind::Mesh { stood, before, .. } = s.kind {
            rows.push((a + hand::STOOD, stood));
            rows.push((a + hand::BEFORE, before));
        }
        for (b, m) in rows {
            vs.extend((0..4).map(|r| (b + 0x10 * r as u32, m[r])));
        }
    }
    for (b, &n) in v.below.iter().zip(&h.below) {
        let w = ee.u32_at(n + 0x100);
        out.push((n + 0x100, (w & !0xff) | u32::from(b.flags)));
    }
    for (n, &a) in v.scene.nodes.iter().zip(&o.nodes) {
        let entry = match ee.u32_at(a + hand::NODE_ENTRY) {
            0 => h.node_entry,
            e => e,
        };
        out.push((a + hand::NODE_ENTRY, if n.spatial { entry } else { 0 }));
        out.push((a + hand::NODE_REACH, n.reach.to_bits()));
        vs.push((a + hand::NODE_CENTRE, n.centre));
        vs.push((a + hand::NODE_HALF, n.half));
    }
    for (a, x) in vs {
        for (k, b) in x.to_bits().into_iter().enumerate() {
            out.push((a + 4 * k as u32, b));
        }
    }
    out
}

/// The reset byte (`+0x44`: the sync's switch and the reset's bits) and the flag byte after it, the
/// word's other halves the image's.
pub(crate) fn flag_bytes_word(ee: &Ee, v: &atv_car::Vehicle) -> u32 {
    let reset = v.reset_flags | u8::from(v.collidables.sync);
    (ee.u32_at(car_at() + hand::RESET_BYTE) & 0xffff_0000) | u32::from(v.flags) << 8 | u32::from(reset)
}

/// The blocks MD128's calls write in, beyond MD106b's: the two collidables and each subtree node's
/// flag word.
pub(crate) fn hand_blocks(ee: &Ee, p: &Ptrs) -> Vec<(u32, u32)> {
    let h = hand_ptrs(ee, p);
    h.parts.iter().map(|&(_, a)| (a, 0x260)).chain(h.below.iter().map(|&n| (n + 0x100, 4))).collect()
}

/// A generated state of MD128's words: the map's area round the body (in, or out past one edge), the
/// throw's seed, the database entries, node 0's stale and origin-reach bits, the sync's switch and
/// active probe, the reset's bits, owner and flag word, and the subtree's current bits. Without a
/// look the area holds the body (a wreck would call through the missing look).
pub(crate) fn generate_hand_outs(r: &mut Rng, v: &mut atv_car::Vehicle, body: &Body) {
    if let Some(a) = &mut v.area {
        // On the image the world's box is the terrain's extent: its far corner is the map's width
        // and depth. Half the time (always without a look) the corner is past the margin, else it is
        // the one MD106b drew round the body (as often outside the world as in it).
        let (px, pz) = (body.position.x(), body.position.z());
        let near = v.bounds.area.filter(|_| v.look && r.chance(0.5)).map(|(_, hi)| (hi.x(), hi.z()));
        let (width, depth) = near.unwrap_or_else(|| {
            let inside = !v.look || r.chance(0.6);
            let mut d = [r.range(20.0, 2500.0), r.range(20.0, 2500.0)];
            if !inside {
                d[(r.next() % 2) as usize] = r.range(-80.0, -20.0);
            } else if v.look && r.chance(0.3) {
                // On the margin: a strict test one way, the other a unit off.
                d[(r.next() % 2) as usize] = [-1.0, -0.5, 0.0, 0.5, 1.0][(r.next() % 5) as usize];
            }
            (px + 150.0 + d[0], pz + 150.0 + d[1])
        });
        a.width = F(width);
        a.depth = F(depth);
        if let Some((_, hi)) = &mut v.bounds.area {
            *hi = Vf::new(width, hi.y(), depth, hi.w());
        }
    }
    v.rng = GameRng::from_seed(r.word());
    v.spatial = r.chance(0.5);
    let n = &mut v.scene.nodes[0];
    n.spatial = r.chance(0.75);
    n.flags = (n.flags & !0xc8) | (r.word() as u8 & 0xc8);
    if r.chance(0.5) {
        n.more ^= 4;
    }
    v.collidables.sync = r.chance(0.85);
    if r.chance(0.2) {
        v.collidables.active = None;
    }
    v.reset_flags = (v.reset_flags & !0xc0) | (r.word() as u8 & 0xc0);
    v.owned = r.chance(0.8);
    v.flag_word = (v.flag_word & !2) | (r.word() & 2);
    for b in &mut v.below {
        if r.chance(0.5) {
            b.flags ^= 1;
        }
    }
}

/// Hook what MD128's calls hand out in turn (MD106b's hooks leave those calls unhooked): each
/// asserts what it was handed and logs it; the random numbers come from `rng`, the port's.
pub(crate) fn hook_hand_outs(ee: &mut Ee, p: Ptrs, o: &OwnPtrs, log: Shared<Vec<Call>>, rng: Shared<GameRng>) {
    let h = hand_ptrs(ee, &p);
    ee.hooks.insert(RAND, Box::new(move |ee| {
        let x = rng.borrow_mut().rand();
        ee.set(2, x as u32 as u64);
    }));
    let parts = h.parts;
    let part_of = move |a: u32| parts.iter().find(|&&(_, x)| x == a).unwrap_or_else(|| panic!("no collidable at {a:#x}")).0;
    let l = log.clone();
    ee.hooks.insert(SPATIAL_MOVE, Box::new(move |ee| {
        let v = |a: u64| get_v(ee, a as u32);
        let (centre, half, reach) = (v(ee.r(5)), v(ee.r(6)), F(ee.f(12)));
        let c = match ee.r(4) as u32 {
            CAR => Call::SpatialMove { node: false, centre, half, reach },
            n if n == p.node => Call::SpatialMove { node: true, centre, half, reach },
            n => Call::PartMove { part: part_of(n - 0x18), centre, half, reach },
        };
        l.borrow_mut().push(c);
    }));
    let l = log.clone();
    ee.hooks.insert(GATHER, Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(6)), (ee.u32_at(CAR + hand::ACTORS), 3), "the gather's list and kind");
        l.borrow_mut().push(Call::Gather(part_of(ee.r(5) as u32)));
    }));
    let rebound = ee.u32_at(ee.u32_at(p.node + 4) + 0x94);
    let l = log.clone();
    ee.hooks.insert(rebound, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, p.node, "the rebound on something else, from {:#x}", ee.r(31));
        l.borrow_mut().push(Call::NodeRebound);
    }));
    let physics = ee.u32_at(CAR + hand::PHYSICS);
    let second = ee.u32_at(CAR + hand::SECOND);
    for (k, f) in LEAVE_WORLD.into_iter().enumerate() {
        let l = log.clone();
        let x = [p.body, second, CAR + hand::POST][k];
        ee.hooks.insert(f, Box::new(move |ee| {
            assert_eq!((ee.r(4) as u32, ee.r(5) as u32), (physics + hand::LISTS[k], x), "list {k}'s removal");
            l.borrow_mut().push(Call::LeaveWorld(k));
            ee.set(2, 1);
        }));
    }
    let riding = h.parts[0].1;
    let l = log.clone();
    ee.hooks.insert(RIDING_OWNER, Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5)), (riding, 0), "the riding probe's owner cleared");
        l.borrow_mut().push(Call::RidingLetGo);
    }));
    let (l, world) = (log.clone(), o.world);
    ee.hooks.insert(RIDING_ADD, Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5) as u32), (riding, ee.u32_at(world + hand::CATEGORY)), "the riding probe's list added to");
        assert_eq!(l.borrow().last(), Some(&Call::RidingLetGo), "the list added to without the owner cleared");
    }));
    let (l, owner) = (log, h.owner);
    ee.hooks.insert(RELEASE, Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5) as u32), (owner, p.record), "the record handed back");
        l.borrow_mut().push(Call::ReleaseRecord);
    }));
}
