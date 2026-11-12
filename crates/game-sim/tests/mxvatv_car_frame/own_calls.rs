// MD106b — SLUS-21104's own calls in the frame, unhooked.

use super::*;

/// What the five calls MD106b ports hand out in turn: the map-edge test, the rumble's switch, the
/// sound cue, the wrecked look on and off, the car's respawn, the body's reset, the spatial update,
/// the collision sync and the node's bounds.
pub(crate) const EDGE_TEST: u32 = 0x004a_4f20;
const RUMBLE_ON: u32 = 0x0051_a250;
const RUMBLE_RA: u32 = 0x004a_841c;
const LOOK_ON: u32 = 0x0019_86c0;
const LOOK_OFF: u32 = 0x004b_82c0;
const RESPAWN: u32 = 0x004c_ab10;
const BODY_RESET: u32 = 0x0049_4ae0;
pub(crate) const SPATIAL_UPDATE: u32 = 0x0049_2c40;
pub(crate) const COLLISION_SYNC: u32 = 0x0049_42c0;
pub(crate) const NODE_BOUNDS: u32 = 0x0019_6e80;
pub(crate) const SUBTREE_OUT_OF_DATE: u32 = 0x0019_3d70;

/// The vehicle's words MD106b's calls keep (research/oracle.txt `# MD106b`).
mod own {
    pub const FLAGS_WORD: u32 = 0x44;
    pub const WORLD: u32 = 0xe8;
    pub const BOUNDED: u32 = 0x308;
    pub const KIND: u32 = 0xbbc;
    pub const VEHICLE_STATE: u32 = 0xbc4;
    pub const CLOCK: u32 = 0xc84;
    pub const MAX_WRECK_TIME: u32 = 0xc88;
    pub const COUNTING: u32 = 0xc8c;
    pub const WRECK_DAMPING: u32 = 0xca0;
    pub const WRECK_INERTIA: u32 = 0xcb0;
    pub const INERTIA: u32 = 0xcc0;
    pub const CUE_TAG: u32 = 0xd20;
    pub const CUE_SOURCE: u32 = 0x674;
    pub const TIPPED_WRECK: u32 = 0x1118;
    pub const WRECKS_ON: u32 = 0x111c;
    pub const LOOK: u32 = 0x12b0;
    /// The vehicle state's mode (1 the player's) and pad.
    pub const MODE: u32 = 0x28;
    pub const PAD: u32 = 0x5c;
    /// The record's touching word; the body's inertia; the world's drag, gravity, floor and box.
    pub const TOUCHING: u32 = 0x34;
    pub const BODY_INERTIA: u32 = 0x130;
    pub const DRAG: u32 = 4;
    pub const GRAVITY: u32 = 0x40;
    pub const FLOOR: u32 = 0x98;
    pub const AREA: u32 = 0xa8;
}

/// The car's node and its parents.
fn car_scene_nodes(ee: &Ee, first: u32) -> Vec<u32> {
    let mut out = vec![first];
    while let n @ 1.. = ee.u32_at(out[out.len() - 1] + 0xf0) {
        assert!(!out.contains(&n), "a loop of parents");
        out.push(n);
    }
    out
}

/// The image's car's MD106b pointers.
#[derive(Clone)]
pub(crate) struct OwnPtrs {
    pub(crate) world: u32,
    pub(crate) state: u32,
    pub(crate) look: u32,
    pub(crate) area: u32,
    pub(crate) nodes: Vec<u32>,
}

pub(crate) fn own_ptrs(ee: &Ee, p: &Ptrs) -> OwnPtrs {
    let world = ee.u32_at(CAR + own::WORLD);
    let nodes = car_scene_nodes(ee, p.node);
    assert!(nodes.len() <= 4, "the car's node has {} parents, past the 3 a Scene holds", nodes.len() - 1);
    OwnPtrs {
        world,
        state: ee.u32_at(CAR + own::VEHICLE_STATE),
        look: ee.u32_at(CAR + own::LOOK),
        area: ee.u32_at(world + own::AREA),
        nodes,
    }
}

pub(crate) fn read_vehicle(ee: &Ee, p: &Ptrs, o: &OwnPtrs) -> atv_car::Vehicle {
    let b = |a: u32| ee.u32_at(a) != 0;
    let index = |a: u32| o.nodes.iter().position(|&n| n == a);
    let mut nodes = [game_sim::atv_moto::Node::default(); 4];
    for (n, &a) in nodes.iter_mut().zip(&o.nodes) {
        *n = game_sim::atv_moto::Node {
            local: std::array::from_fn(|r| get_v(ee, a + 0x30 + 0x10 * r as u32)),
            world: std::array::from_fn(|r| get_v(ee, a + 0x70 + 0x10 * r as u32)),
            parent: index(ee.u32_at(a + 0xf0)),
            child: index(ee.u32_at(a + 0xf4)),
            sibling: index(ee.u32_at(a + 0xf8)),
            flags: ee.u8_at(a + 0x100),
            ..Default::default()
        };
    }
    let mode = ee.u32_at(o.state + own::MODE);
    atv_car::Vehicle {
        wrecks_on: b(CAR + own::WRECKS_ON),
        bounded: b(CAR + own::BOUNDED),
        clock: F(ee.f32_at(CAR + own::CLOCK)),
        max_wreck_time: F(ee.f32_at(CAR + own::MAX_WRECK_TIME)),
        counting: b(CAR + own::COUNTING),
        tipped_wreck: b(CAR + own::TIPPED_WRECK),
        kind: ee.u32_at(CAR + own::KIND),
        wreck_damping: get_v(ee, CAR + own::WRECK_DAMPING),
        wreck_inertia: get_v(ee, CAR + own::WRECK_INERTIA),
        inertia: get_v(ee, CAR + own::INERTIA),
        body_inertia: get_v(ee, p.body + own::BODY_INERTIA),
        rumble: o.state != 0 && mode == 1 && b(o.state + own::PAD),
        look: b(CAR + own::LOOK),
        touching: b(p.record + own::TOUCHING),
        record: p.record != 0,
        gravity: get_v(ee, o.world + own::GRAVITY),
        drag: F(ee.f32_at(o.world + own::DRAG)),
        flags: ee.u8_at(CAR + own::FLAGS_WORD + 1),
        scene: game_sim::atv_moto::Scene { nodes },
        subtree: ee.u32_at(p.node + 0xf4) != 0,
        bounds: game_sim::atv_moto::Bounds {
            floor: F(ee.f32_at(o.world + own::FLOOR)),
            area: (o.area != 0).then(|| (get_v(ee, o.area + 0x30), get_v(ee, o.area + 0x40))),
        },
        out_of_bounds: false,
    }
}

/// Every word [`read_vehicle`] reads, where it lives; words a byte of which is the port's take their
/// other bytes from `ee` (the image).
pub(crate) fn vehicle_words(ee: &Ee, v: &atv_car::Vehicle, p: &Ptrs, o: &OwnPtrs) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut w = |a: u32, x: u32| out.push((a, x));
    let byte = |a: u32, b: u8| (ee.u32_at(a & !3) & !(0xff << (8 * (a & 3)))) | u32::from(b) << (8 * (a & 3));
    w(CAR + own::WRECKS_ON, word(v.wrecks_on));
    w(CAR + own::BOUNDED, word(v.bounded));
    w(CAR + own::CLOCK, v.clock.to_bits());
    w(CAR + own::MAX_WRECK_TIME, v.max_wreck_time.to_bits());
    w(CAR + own::COUNTING, word(v.counting));
    w(CAR + own::TIPPED_WRECK, word(v.tipped_wreck));
    w(CAR + own::KIND, v.kind);
    w(CAR + own::LOOK, if v.look { o.look } else { 0 });
    w(o.state + own::MODE, if v.rumble { 1 } else { 0 });
    w(p.record + own::TOUCHING, word(v.touching));
    w(o.world + own::DRAG, v.drag.to_bits());
    w(o.world + own::FLOOR, v.bounds.floor.to_bits());
    w(CAR + own::FLAGS_WORD, byte(CAR + own::FLAGS_WORD + 1, v.flags));
    let mut vs = vec![
        (CAR + own::WRECK_DAMPING, v.wreck_damping),
        (CAR + own::WRECK_INERTIA, v.wreck_inertia),
        (CAR + own::INERTIA, v.inertia),
        (p.body + own::BODY_INERTIA, v.body_inertia),
        (o.world + own::GRAVITY, v.gravity),
    ];
    if let Some((lo, hi)) = v.bounds.area {
        vs.push((o.area + 0x30, lo));
        vs.push((o.area + 0x40, hi));
    }
    for (n, &a) in v.scene.nodes.iter().zip(&o.nodes) {
        for r in 0..4u32 {
            vs.push((a + 0x30 + 0x10 * r, n.local[r as usize]));
            vs.push((a + 0x70 + 0x10 * r, n.world[r as usize]));
        }
        w(a + 0x100, byte(a + 0x100, n.flags));
    }
    for (a, x) in vs {
        for (k, b) in x.to_bits().into_iter().enumerate() {
            w(a + 4 * k as u32, b);
        }
    }
    out
}

/// Unhook MD106b's five calls (the vehicle force and post, the wreck timer, the node-local turn and
/// the node's position) and hook what they hand out instead; the landing sound now logs what it is
/// handed. A call from anything but the car (or its node, record or look) fails the test: these
/// hooks run nothing, for every caller.
fn hook_own(ee: &mut Ee, p: Ptrs, o: &OwnPtrs, log: Shared<Vec<Call>>, out: Shared<bool>) {
    for a in [VEHICLE_FORCE, VEHICLE_POST, WRECK_TIMER, NODE_LOCAL, NODE_POSITION] {
        ee.hooks.remove(&a).expect("hooked");
    }
    let record_tell = ee.u32_at(ee.u32_at(p.record + 4) + 0x88);
    let on: [(u32, u32, Call); 8] = [
        (SUBTREE_OUT_OF_DATE, ee.u32_at(p.node + 0xf4), Call::SubtreeOutOfDate),
        (record_tell, p.record, Call::GravityRecord),
        (BODY_RESET, CAR, Call::BodyReset),
        (RESPAWN, CAR, Call::Respawn),
        (SPATIAL_UPDATE, CAR, Call::SpatialUpdate),
        (NODE_BOUNDS, p.node, Call::NodeBounds),
        (COLLISION_SYNC, CAR, Call::CollisionSync),
        (LOOK_OFF, o.look, Call::WreckLook(false)),
    ];
    for (addr, this, c) in on {
        let l = log.clone();
        ee.hooks.insert(addr, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, this, "{c:?} ({addr:#x}) on something else, from {:#x}", ee.r(31));
            if c == Call::GravityRecord {
                assert_eq!(ee.r(5), 1, "the record's gravity word");
            }
            l.borrow_mut().push(c);
        }));
    }
    let (l, look) = (log.clone(), o.look);
    ee.hooks.insert(LOOK_ON, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, look, "the wrecked look on something else, from {:#x}", ee.r(31));
        l.borrow_mut().push(Call::WreckLook(true));
    }));
    let l = log.clone();
    ee.hooks.insert(EDGE_TEST, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, CAR, "the map-edge test on something else");
        l.borrow_mut().push(Call::OutOfBounds);
        ee.set(2, u64::from(*out.borrow()));
    }));
    let l = log.clone();
    ee.hooks.insert(RUMBLE_ON, Box::new(move |ee| {
        assert_eq!(ee.r(31) as u32, RUMBLE_RA, "the rumble switch from elsewhere");
        l.borrow_mut().push(Call::Rumble);
        ee.set(2, 0);
    }));
    let l = log.clone();
    ee.hooks.insert(CUE, Box::new(move |ee| {
        assert_eq!((ee.r(5) as u32, ee.r(6)), (CAR + own::CUE_SOURCE, 8), "a cue from something else, from {:#x}", ee.r(31));
        let kind = ee.r(7) as u32;
        let tag = if kind == 12 { 0 } else { ee.u32_at(CAR + own::CUE_TAG) };
        assert_eq!(ee.r(8) as u32, tag, "cue {kind}'s tag");
        l.borrow_mut().push(Call::Cue(kind));
    }));
    let l = log;
    ee.hooks.insert(LANDING_SOUND, Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5)), (p.landing_sound, 0), "the landing sound's object and kind");
        l.borrow_mut().push(Call::LandingSound(get_v(ee, ee.r(6) as u32)));
    }));
}

/// A generated state of MD106b's words: wrecks on or off, the clock about `MaxWreckTime`, the kinds,
/// the flag byte's bits, the world's box round the body (in and out), the rumble and the look.
fn generate_own(r: &mut Rng, image: &atv_car::Vehicle, car: &mut Car, body: &Body, s: &mut Script) -> (atv_car::Vehicle, bool) {
    let mut v = *image;
    v.wrecks_on = r.chance(0.9);
    v.bounded = r.chance(0.7);
    v.max_wreck_time = F(r.range(0.2, 6.0));
    v.clock = if r.chance(0.5) { F(v.max_wreck_time.0 - r.range(-0.08, 0.06)) } else { F(r.range(0.0, v.max_wreck_time.0)) };
    v.counting = r.chance(0.4);
    v.tipped_wreck = r.chance(0.3);
    v.kind = [0, 1, 4, 4, 2][(r.next() % 5) as usize];
    v.wreck_damping = Vf::new(r.range(0.9, 1.0), r.range(0.9, 1.0), r.range(0.9, 1.0), 1.0);
    v.wreck_inertia = Vf::new(r.range(0.05, 1.0), r.range(0.05, 1.0), r.range(0.05, 1.0), r.range(0.5, 2.0));
    let mut lane = |r: &mut Rng| if r.chance(0.08) { [0.0, -0.0, -3.0][(r.next() % 3) as usize] } else { r.range(100.0, 1500.0) };
    v.inertia = Vf::new(lane(r), lane(r), lane(r), r.range(0.5, 2.0));
    v.body_inertia = Vf::new(lane(r), lane(r), lane(r), r.range(0.5, 2.0));
    v.rumble = r.chance(0.5);
    v.look = r.chance(0.85);
    v.touching = r.chance(0.7);
    v.gravity = Vf::new(r.range(-50.0, 50.0), r.range(-3000.0, -1000.0), r.range(-50.0, 50.0), 1.0);
    v.drag = F(r.range(0.0, 0.2));
    v.flags = (image.flags & !0x67) | (r.word() as u8 & 0x67);
    v.bounds.floor = F(body.position.y() + r.range(-8.0, 8.0));
    if v.bounds.area.is_some() {
        let (px, pz) = (body.position.x(), body.position.z());
        let lo = Vf::new(px - r.range(-10.0, 60.0), r.range(-100.0, 100.0), pz - r.range(-10.0, 60.0), 1.0);
        let hi = Vf::new(px + r.range(-10.0, 60.0), r.range(-100.0, 100.0), pz + r.range(-10.0, 60.0), 1.0);
        v.bounds.area = Some((lo, hi));
    }
    for n in &mut v.scene.nodes {
        if r.chance(0.5) {
            n.flags ^= 1;
        }
    }
    let mut out = r.chance(0.25);
    if !v.look {
        // The wreck calls the look without testing it is there: no wreck can start.
        car.state = [atv_car::WRECK_EDGE, atv_car::WRECK_TIPPED][(r.next() % 2) as usize];
        out = false;
    } else if r.chance(0.35) {
        car.state = atv_car::RIDING;
    }
    if r.chance(0.5) {
        // Slow, for the still tests.
        for t in &mut s.steps {
            for j in t.iter_mut() {
                j.3 = Vf::new(j.3.x() * 0.1, j.3.y() * 0.1, j.3.z() * 0.1, j.3.w());
            }
        }
    }
    (v, out)
}

/// **The car's frame with SLUS-21104's own calls in it is the original's** (MD106b): the frame test
/// with the vehicle force and post, the wreck timer, the landing's node-local turn and the node's
/// position no longer hooked, against [`atv_car::Own`] answering them over the same script, 3,000
/// generated states: the car, its tires and body, the vehicle's wreck words, the world's drag and the
/// scene's nodes word for word, the same calls handed out, nothing else written in those blocks, and
/// every branch of the five seen.
#[test]
fn the_cars_own_calls_are_the_originals() {
    let Some(pristine) = image() else { return };
    let Some(mut ee) = image() else { return };
    let p = ptrs(&ee);
    let o = own_ptrs(&ee, &p);
    let image_car = read_car(&ee, &p);
    let image_body = seed::body(&|a| ee.u32_at(a), p.body);
    let image_v = read_vehicle(&ee, &p, &o);
    assert!(image_v.look && image_v.record && o.state != 0 && ee.u32_at(o.state + own::PAD) != 0, "the image's look, record and pad");
    let log: Shared<Vec<Call>> = Rc::default();
    let script: Shared<Script> = Rc::default();
    let out: Shared<bool> = Rc::default();
    hook(&mut ee, p, log.clone(), script.clone());
    hook_own(&mut ee, p, &o, log.clone(), out.clone());
    let world = World { iterations: 1, rate: F(115.0) };
    let table = AcosTable::build();
    let mut r = Rng(0x0106_b0b0_0000_0001);
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    let mut see = |what: &'static str, yes: bool| {
        if yes {
            *seen.entry(what).or_insert(0) += 1;
        }
    };
    let mut blocks: Vec<(u32, u32)> = [(CAR, 0x1500), (p.body, 0x230), (p.record + at::TIPPED, 4), (p.record + own::TOUCHING, 4), (o.world, 0xb0)]
        .into_iter()
        .chain(p.tires.map(|t| (t, 0x200)))
        .collect();
    blocks.extend(o.nodes.iter().map(|&n| (n, 0x110)));
    const CASES: usize = 3000;
    for case in 0..CASES {
        let (mut car, body, dt) = generate(&mut r, &image_car, &image_body, &p);
        let mut s = Script::new(&mut r);
        let (v, edge) = generate_own(&mut r, &image_v, &mut car, &body, &mut s);
        let before = [car_words(&car, &p), seed::body_words(&body, p.body), vehicle_words(&pristine, &v, &p, &o)].concat();

        // The original.
        poke(&mut ee, &before);
        *script.borrow_mut() = s.clone();
        *out.borrow_mut() = edge;
        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(WORLD_STEP, &[u64::from(WORLD)], &[dt.0], 20_000_000);
        let written = ee.written_ranges();

        // The port.
        let (mut pc, mut pb, mut pv) = (car, body, v);
        pv.out_of_bounds = edge;
        let mut port = Port { log: Vec::new(), script: s };
        atv_car::frame(&world, &mut pc, &mut pb, dt, &table, &mut atv_car::Own { v: &mut pv, rest: &mut port });
        let after = [car_words(&pc, &p), seed::body_words(&pb, p.body), vehicle_words(&pristine, &pv, &p, &o)].concat();

        let bad: Vec<String> = after
            .iter()
            .filter(|&&(a, x)| ee.u32_at(a) != x)
            .map(|&(a, x)| format!("  {a:#x}: port {x:#010x} ({}), original {:#010x} ({})", f32::from_bits(x), ee.u32_at(a), ee.f32_at(a)))
            .collect();
        let theirs = log.borrow().iter().map(|c| format!("{c:?}")).collect::<Vec<_>>();
        let ours = port.log.iter().map(|c| format!("{c:?}")).collect::<Vec<_>>();
        assert!(bad.is_empty(), "case {case} (dt {dt:?}, state {} -> {}): {} words differ\n{}\ncalls {ours:?}", car.state, pc.state, bad.len(), bad.join("\n"));
        assert_eq!(ours, theirs, "case {case}: the calls");
        let compared: std::collections::BTreeSet<u32> = after.iter().map(|&(a, _)| a).collect();
        for &(from, to) in &written {
            for a in (from & !3..to).step_by(4) {
                let ours = blocks.iter().any(|&(b, n)| (b..b + n).contains(&a));
                assert!(!ours || compared.contains(&a), "case {case}: the original wrote {a:#x}, which is not compared");
            }
        }

        let has = |what: &str| ours.iter().any(|c| c.starts_with(what));
        let wreck = car.state == atv_car::RIDING && pc.state != atv_car::RIDING || has("OutOfBounds") && edge;
        let unwreck = car.state != atv_car::RIDING && pc.state == atv_car::RIDING;
        see("wrecks off", !v.wrecks_on);
        see("edge asked", has("OutOfBounds"));
        see("edge wreck", has("OutOfBounds") && edge);
        see("tipped wreck", pc.state == atv_car::WRECK_TIPPED && car.state == atv_car::RIDING);
        see("riding, not tipped", v.wrecks_on && car.state == atv_car::RIDING && pc.state == atv_car::RIDING);
        see("edge clock started", has("Cue(12)"));
        see("edge unwreck", unwreck && car.state == atv_car::WRECK_EDGE);
        see("tipped unwreck", unwreck && car.state == atv_car::WRECK_TIPPED);
        see("wrecked, waiting", v.wrecks_on && car.state != atv_car::RIDING && pc.state == car.state);
        see("wreck of kind 0 or 1", wreck && v.kind < 2);
        see("wreck of another kind", wreck && v.kind >= 2);
        see("unwreck of kind 0 or 1", unwreck && v.kind < 2);
        see("unwreck of another kind", unwreck && v.kind >= 2);
        see("unwreck without a look", unwreck && !v.look);
        see("rumble", has("Rumble"));
        see("outside the world", has("BodyReset"));
        see("inside the world", !has("BodyReset"));
        see("node bounds", has("NodeBounds"));
        see("landing sound", has("LandingSound"));
        see("inertia lane not above 0", (wreck || unwreck) && v.kind >= 2 && pv.body_inertia.0[..3].iter().any(|&x| x <= 0.0));

        // Put the image back.
        for (from, to) in written {
            for a in from..to {
                ee.write(a, 1, u128::from(pristine.u8_at(a)));
            }
        }
    }
    eprintln!("MD106b: {CASES} frames, seen {seen:?}");
    for (what, &k) in &seen {
        assert!(k >= 30, "{what} seen only {k} times");
    }
    assert_eq!(seen.len(), 20, "a branch was never seen: {seen:?}");
}
