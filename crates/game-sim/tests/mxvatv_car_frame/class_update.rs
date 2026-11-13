use super::*;
use game_sim::atv_car::Updated;
use game_sim::handling::tail;

/// The car classes' update slots (vt `+0x10c`): the buggy's, the golf cart's and the truck's, one
/// code over three copies of the same rodata. Each runs on the buggy image's car.
const SLOTS: [(&str, u32); 3] = [("buggy", 0x004c_6ce0), ("golf cart", 0x004d_0250), ("truck", 0x004d_7f40)];
/// What stays hooked: the class's per-frame step (MD148).
const CLASS_STEP: u32 = 0x004c_5510;
/// The ghost step's calls: the collision probe's mode switch (a1 0, then 1) round its search, and
/// the node's alpha (f12).
const PROBE_MODE: u32 = 0x0011_5890;
const PROBE_SEARCH: u32 = 0x0011_48e0;
const NODE_ALPHA: u32 = 0x0018_d170;
/// The base update's plain calls: the network's step and the vehicle's parts and listeners.
const NETWORK: u32 = 0x0025_e8d0;
const PARTS: u32 = 0x0049_4370;
/// The slot's plain calls: the list's take-off, the rumble's switch, the cue and its queue.
const UNLIST: u32 = 0x0020_3ff0;
const RUMBLE_ON: u32 = 0x0051_a250;
const CUE: u32 = 0x0027_2300;
const CUE_QUEUE: u32 = 0x0044_2eb0;
/// The buggy's virtuals the update and the slot hand out.
const SLOT_CALLS: [(u32, Call); 9] = [
    (0x39c, Call::GhostOver),
    (0x3e4, Call::Prepare),
    (0x35c, Call::Ride(F::ZERO)),
    (0x378, Call::Sync),
    (0x37c, Call::Snapshot),
    (0x3a8, Call::PutBack),
    (0x138, Call::CollisionOn),
    (0x124, Call::CollisionSync),
    (0x3ac, Call::Place),
];
/// The settings pointer and its still-camera bit (MD106d's).
const SETTINGS: u32 = 0x003e_ab60;
const STILL_BYTE: u32 = 0x225;
const STILL_BIT: u8 = 0x08;
/// Free RAM (zero in the image) for a carrying object and its node.
const CARRIER: u32 = 0x01fe_6000;
const CARRIER_NODE: u32 = 0x01fe_6400;

/// The words of the slot, the ghost step and the base update (research/oracle.txt `# MD106e`).
mod ue {
    pub const GHOST: u32 = 0xde8;
    pub const GHOST_LENGTH: u32 = 0xdec;
    pub const PROBE: u32 = 0xcc;
    /// The probe's flags byte: bit `0x20` inside something.
    pub const PROBE_FLAGS: u32 = 0x126;
    pub const INSIDE: u8 = 0x20;
    pub const DISTANCE: u32 = 0x724;
    pub const CARRIER: u32 = 0x48c;
    pub const CARRIER_NODE: u32 = 0xe4;
    pub const NODE_FLAGS: u32 = 8;
    pub const COLLISION_WORD: u32 = 0x11bc;
    pub const LIST: u32 = 0xdc;
    pub const COLLISION_FLAGS: u32 = 0x44;
    pub const RUN: u32 = 0x680;
    pub const PLAYER: u32 = 0x684;
    pub const ID: u32 = 0xd20;
    pub const CUE_SOURCE: u32 = 0x674;
    pub const RIDE_CLOCK: u32 = 0xcf8;
    pub const DOWN: [u32; 2] = [0xd2c, 0xd30];
    pub const UP: u32 = 0xd34;
    pub const FREE: u32 = 0xc14;
    pub const STUCK: u32 = 0xc10;
    pub const STUCK_TIME: u32 = 0xdf0;
    pub const LAST_FREE: u32 = 0xc00;
    /// The node's side and ahead rows.
    pub const SIDE: u32 = 0x80;
    pub const AHEAD: u32 = 0x90;
    /// The vehicle state's rumbler and its switch's word.
    pub const RUMBLER: u32 = 0x5c;
    pub const RUMBLE_WORD: u32 = 0x6ac;
}

/// What a case pokes beyond MD106d's: the slot, the probe's answer, the carrier's node flags, the
/// rumble's switch.
struct Case {
    slot: usize,
    carrier: u8,
    ready: bool,
}

/// Hook what stays a call: the class step, the ghost step's, the base update's, the hazard clock's
/// put-back, and the slot's. Every one logs in the port's terms; the probe's mode switch logs to
/// `modes` (its a1, and 9 for the search between), the rumbler's vt `+0x4c` its level to `rumbled`.
fn hook_class(ee: &mut Ee, p: &Ptrs, state: u32, log: Shared<Vec<Call>>, modes: Shared<Vec<u64>>, rumbled: Shared<Vec<i32>>, ready: Shared<bool>) {
    let node = p.node;
    for (slot, c) in SLOT_CALLS {
        let l = log.clone();
        ee.hooks.insert(ee.u32_at(BUGGY_VTABLE + slot), Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, CAR, "{c:?} on something else, from {:#x}", ee.r(31));
            let c = match c {
                Call::Ride(_) => Call::Ride(F(ee.f(12))),
                Call::PutBack => {
                    assert_eq!((ee.r(5), ee.r(6)), (1, 1), "the put-back's words");
                    c
                }
                Call::Place => {
                    let args = (ee.r(5) as u32, ee.r(6) as u32, ee.r(7), ee.r(8));
                    assert_eq!(args, (node + 0xa0, node + 0x90, 0, 1), "the place's arguments");
                    c
                }
                c => c,
            };
            l.borrow_mut().push(c);
        }));
    }
    let l = log.clone();
    ee.hooks.insert(CLASS_STEP, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, CAR, "the class step on something else");
        l.borrow_mut().push(Call::ClassStep);
    }));
    let l = log.clone();
    ee.hooks.insert(NETWORK, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, CAR + 0x400, "the network's step on something else");
        l.borrow_mut().push(Call::Network(F(ee.f(12))));
    }));
    let l = log.clone();
    ee.hooks.insert(PARTS, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, CAR, "the parts on something else");
        l.borrow_mut().push(Call::Parts(F(ee.f(12))));
    }));
    let probe = ee.u32_at(CAR + ue::PROBE);
    let m = modes.clone();
    ee.hooks.insert(PROBE_MODE, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, probe, "the probe's mode on something else");
        m.borrow_mut().push(ee.r(5));
    }));
    let (l, m) = (log.clone(), modes);
    ee.hooks.insert(PROBE_SEARCH, Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5)), (probe, 0), "the probe's search");
        m.borrow_mut().push(9);
        l.borrow_mut().push(Call::GhostSearch);
    }));
    let l = log.clone();
    ee.hooks.insert(NODE_ALPHA, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, node, "the alpha on another node");
        l.borrow_mut().push(Call::GhostAlpha(F(ee.f(12))));
    }));
    let l = log.clone();
    ee.hooks.insert(UNLIST, Box::new(move |ee| {
        let args = (ee.r(4) as u32, ee.r(5) as u32);
        assert_eq!(args, (ee.u32_at(CAR + ue::LIST), CAR + 0x40), "the list's take-off");
        l.borrow_mut().push(Call::Unlist);
    }));
    let l = log.clone();
    ee.hooks.insert(RUMBLE_ON, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, ee.u32_at(state + ue::RUMBLE_WORD), "the rumble's switch word");
        // The level is in s1 by then.
        l.borrow_mut().push(Call::RideRumble(ee.r(17) as i32));
        ee.set(2, u64::from(*ready.borrow()));
    }));
    let rumbler = ee.u32_at(state + ue::RUMBLER);
    assert_ne!(rumbler, 0, "the image's vehicle state has a rumbler");
    ee.hooks.insert(ee.u32_at(ee.u32_at(rumbler + 0x1c) + 0x4c), Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5)), (rumbler, 0), "the rumbler and its softness");
        assert_eq!(F(ee.f(12)), F(atv_car::RIDE_RUMBLE_TIME), "the rumble's time");
        rumbled.borrow_mut().push(ee.r(6) as i32);
    }));
    let l = log;
    ee.hooks.insert(CUE, Box::new(move |ee| {
        let args = (ee.r(4) as u32, ee.r(5) as u32, ee.r(6), ee.r(7), ee.r(8) as u32, ee.r(9));
        let want = (ee.u32_at(CUE_QUEUE), CAR + ue::CUE_SOURCE, 8, 4, ee.u32_at(CAR + ue::ID), 0);
        assert_eq!(args, want, "the frame's cue");
        l.borrow_mut().push(Call::Cue(4));
    }));
}

/// A float that is now and then a signed zero.
fn zeroish(r: &mut Rng, a: f32, b: f32) -> F {
    F(if r.chance(0.08) { if r.chance(0.5) { -0.0 } else { 0.0 } } else { r.range(a, b) })
}

/// A generated slot case over MD106d's update: the ghost, the odometer's words and the node's two
/// axes, the collision words, the player, the base update's clocks, and the body.
fn generate_class(r: &mut Rng, image: &Updated, image_body: &Body, state: Option<u32>) -> (Updated, Body, Case) {
    let mut w = image.clone();
    w.ghost = match r.next() % 5 {
        0 => zeroish(r, -1.0, 0.0),
        1 => F(r.range(0.0, 0.05)),
        _ => F(r.range(0.0, 3.0)),
    };
    w.ghost_length = match r.next() % 6 {
        0 => if r.chance(0.5) { F(-0.0) } else { F::ZERO },
        1 => w.ghost,
        2 => F(r.range(-3.0, 0.0)),
        _ => F(r.range(0.5, 3.0)),
    };
    w.probe = r.chance(0.85);
    w.inside = r.chance(0.3);
    let v = &mut w.vehicle;
    v.state = state.unwrap_or(0);
    v.distance = F(r.range(0.0, 5000.0));
    let axis = |r: &mut Rng| {
        let y = match r.next() % 6 {
            0 => if r.chance(0.5) { -1.0 } else { 1.0 },
            1 => r.range(-1.3, 1.3),
            2 if r.chance(0.3) => 0.0,
            _ => r.range(-0.99, 0.99),
        };
        let flat = if r.chance(0.05) { 0.0 } else { 1.0 };
        Vf::new(r.range(-1.0, 1.0) * flat, y, r.range(-1.0, 1.0) * flat, r.range(-1.0, 1.0))
    };
    (v.side, v.ahead) = (axis(r), axis(r));
    v.player = if r.chance(0.7) { image.vehicle.player.max(1) } else { 0 };
    v.rumbler = r.chance(0.8);
    v.ride_clock = F(r.range(0.0, 100.0));
    v.down = [zeroish(r, -1.0, 2.0), zeroish(r, -1.0, 2.0)];
    v.up = zeroish(r, -1.0, 2.0);
    v.free = u32::from(r.chance(0.5));
    v.stuck = u32::from(r.chance(0.5));
    v.stuck_time = F(r.range(0.0, 4.0));
    v.last_free = r.vec(500.0, 1.0);
    v.run_flags ^= if r.chance(0.3) { tail::NO_STUCK } else { 0 };
    w.collision_word = r.word() % 3;
    w.listed = r.chance(0.7);
    w.collision_flags = r.word() as u8;
    let carrier = [0, 1, 2, 3][(r.next() % 4) as usize];
    w.carried = carrier == 3;

    let mut body = *image_body;
    body.enabled = r.chance(0.8);
    body.velocity = r.vec(60.0, 0.0);
    let d = r.vec(50.0, 0.0);
    body.position = Vf(core::array::from_fn(|k| image_body.position.0[k] + d.0[k]));
    let peak = [0.0, 3.0, 8.0, 30.0][(r.next() % 4) as usize];
    body.correction_peak = r.vec(peak, 1.0);
    let slot = (r.next() % 3) as usize;
    (w, body, Case { slot, carrier, ready: r.chance(0.6) })
}

/// The slot's and the base update's words, as the port leaves them.
fn class_words(w: &Updated, flags_word: u32) -> Vec<(u32, u32)> {
    let v = &w.vehicle;
    let c = |o: u32| CAR + o;
    let mut out = vec![
        (c(ue::GHOST), w.ghost.to_bits()),
        (c(ue::DISTANCE), v.distance.to_bits()),
        (c(ue::COLLISION_WORD), w.collision_word),
        (c(ue::COLLISION_FLAGS), flags_word & !0xff | u32::from(w.collision_flags)),
        (c(ue::RIDE_CLOCK), v.ride_clock.to_bits()),
        (c(ue::DOWN[0]), v.down[0].to_bits()),
        (c(ue::DOWN[1]), v.down[1].to_bits()),
        (c(ue::UP), v.up.to_bits()),
        (c(ue::FREE), v.free),
        (c(ue::STUCK), v.stuck),
        (c(ue::STUCK_TIME), v.stuck_time.to_bits()),
    ];
    out.extend(v.last_free.to_bits().into_iter().enumerate().map(|(k, x)| (c(ue::LAST_FREE) + 4 * k as u32, x)));
    out
}

/// The image's words of the slot and the base update.
fn read_class(ee: &Ee, p: &Ptrs) -> Updated {
    let c = |o: u32| CAR + o;
    let f = |o: u32| F(ee.f32_at(c(o)));
    let run = ee.u32_at(c(ue::RUN));
    Updated {
        ghost: f(ue::GHOST),
        ghost_length: f(ue::GHOST_LENGTH),
        probe: ee.u32_at(c(ue::PROBE)) != 0,
        inside: ee.u8_at(ee.u32_at(c(ue::PROBE)) + ue::PROBE_FLAGS) & ue::INSIDE != 0,
        vehicle: tail::Vehicle {
            state: ee.u32_at(c(update::up::VEHICLE_STATE)),
            side: get_v(ee, p.node + ue::SIDE),
            ahead: get_v(ee, p.node + ue::AHEAD),
            body_position: get_v(ee, p.body + 0x70),
            distance: f(ue::DISTANCE),
            player: ee.u32_at(c(ue::PLAYER)),
            run,
            run_flags: ee.u32_at(run + 8),
            ride_clock: f(ue::RIDE_CLOCK),
            down: ue::DOWN.map(f),
            up: f(ue::UP),
            free: ee.u32_at(c(ue::FREE)),
            stuck: ee.u32_at(c(ue::STUCK)),
            stuck_time: f(ue::STUCK_TIME),
            last_free: get_v(ee, c(ue::LAST_FREE)),
            ..tail::Vehicle::default()
        },
        carried: false,
        collision_word: ee.u32_at(c(ue::COLLISION_WORD)),
        listed: ee.u32_at(c(ue::LIST)) != 0,
        collision_flags: ee.u8_at(c(ue::COLLISION_FLAGS)),
    }
}

/// **The car classes' update slots are the originals'** (MD106e): the buggy's, golf cart's and
/// truck's vt `+0x10c` on the buggy image's car with the update's ghost step, hazard clock and base
/// update unhooked (only what they and the slot hand out hooked, and the class's per-frame step),
/// against [`atv_car::class_update`] over 10,000 generated cases on MD106d's: every compared word of
/// the car, its tires and the slot's and base update's words equal, the same calls in the same order
/// with the same arguments, nothing else written in the car, its tires or its body, the rumbler
/// rung exactly when the switch says so. Each branch is seen.
#[test]
fn the_car_classes_update_slots_are_the_originals() {
    let Some(pristine) = image() else { return };
    let Some(mut ee) = image() else { return };
    let p = ptrs(&ee);
    let image_car = read_car(&ee, &p);
    let image_body = seed::body(&|a| ee.u32_at(a), p.body);
    let image_w = read_class(&ee, &p);
    let state = ee.u32_at(CAR + update::up::VEHICLE_STATE);
    assert_ne!(state, 0, "the image's car has a vehicle state");
    assert!(image_w.probe, "the image's car has a collision probe");
    let rumbler = ee.u32_at(state + ue::RUMBLER);
    let still_at = ee.u32_at(SETTINGS) + STILL_BYTE;
    for a in (CARRIER..CARRIER_NODE + 0x100).step_by(4) {
        assert_eq!(ee.u32_at(a), 0, "the scratch at {a:#x} is not free");
    }
    let log: Shared<Vec<Call>> = Rc::default();
    let modes: Shared<Vec<u64>> = Rc::default();
    let rumbled: Shared<Vec<i32>> = Rc::default();
    let ready: Shared<bool> = Rc::default();
    hook_class(&mut ee, &p, state, log.clone(), modes.clone(), rumbled.clone(), ready.clone());
    let mut r = Rng(0x0106_e0da_7e00_0001);
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    let mut see = |what: &'static str, yes: bool| {
        if yes {
            *seen.entry(what).or_insert(0) += 1;
        }
    };
    let blocks: Vec<(u32, u32)> = [(CAR, 0x1500), (p.body, 0x230)].into_iter().chain(p.tires.map(|t| (t, 0x200))).collect();
    let probe = ee.u32_at(CAR + ue::PROBE);
    const CASES: usize = 10_000;
    for case in 0..CASES {
        let (u, hazard_words, mut car, dt) = update::generate_update(&mut r, &image_car);
        for (t, &k) in car.tires.iter_mut().zip(&u.tire_surface) {
            t.surface = k;
        }
        car.grounded = r.chance(0.5);
        // Now and then wrecked (13, 14) or another word: the slot's rumble waits for riding.
        car.state = [atv_car::RIDING, atv_car::RIDING, atv_car::RIDING, 13, 14, r.word()][(r.next() % 6) as usize];
        if r.chance(0.1) {
            car.ground_velocity.0[2] = if r.chance(0.5) { -0.0 } else { -car.ground_velocity.z() };
        }
        let (w, body, k) = generate_class(&mut r, &image_w, &image_body, u.vehicle_state.map(|_| state));

        // The original.
        poke(&mut ee, &car_words(&car, &p));
        poke(&mut ee, &seed::body_words(&body, p.body));
        let still = ee.u8_at(still_at) & !STILL_BIT | if u.still_camera { STILL_BIT } else { 0 };
        ee.write(still_at, 1, u128::from(still));
        ee.set_u32(CAR + update::up::VEHICLE_STATE, w.vehicle.state);
        ee.set_u32(state + update::up::MODE, u.vehicle_state.unwrap_or(0));
        ee.set_u32(CAR + update::up::ON_HAZARD, word(u.on_hazard));
        for (j, (s, hw)) in u.surfaces.iter().zip(hazard_words).enumerate() {
            let a = CAR + update::up::SURFACES + 0xc * j as u32;
            ee.set_u32(a, hw);
            ee.set_u32(a + 4, s.time.to_bits());
        }
        for (t, &s) in p.tires.iter().zip(&u.tire_surface) {
            ee.write(t + update::up::TIRE_SURFACE, 1, u128::from(s));
        }
        let v = &w.vehicle;
        let c = |o: u32| CAR + o;
        ee.set_u32(c(ue::GHOST), w.ghost.to_bits());
        ee.set_u32(c(ue::GHOST_LENGTH), w.ghost_length.to_bits());
        ee.set_u32(c(ue::PROBE), if w.probe { probe } else { 0 });
        let flags = pristine.u8_at(probe + ue::PROBE_FLAGS) & !ue::INSIDE | if w.inside { ue::INSIDE } else { 0 };
        ee.write(probe + ue::PROBE_FLAGS, 1, u128::from(flags));
        ee.set_u32(c(ue::DISTANCE), v.distance.to_bits());
        put_v(&mut ee, p.node + ue::SIDE, v.side);
        put_v(&mut ee, p.node + ue::AHEAD, v.ahead);
        ee.set_u32(c(ue::CARRIER), if k.carrier == 0 { 0 } else { CARRIER });
        ee.set_u32(CARRIER + ue::CARRIER_NODE, if k.carrier < 2 { 0 } else { CARRIER_NODE });
        ee.set_u32(CARRIER_NODE + ue::NODE_FLAGS, if k.carrier == 3 { 0x2 | r.word() & !0x2 } else { r.word() & !0x2 });
        ee.set_u32(c(ue::COLLISION_WORD), w.collision_word);
        ee.set_u32(c(ue::LIST), if w.listed { pristine.u32_at(c(ue::LIST)).max(CARRIER + 0x200) } else { 0 });
        ee.write(c(ue::COLLISION_FLAGS), 1, u128::from(w.collision_flags));
        ee.set_u32(c(ue::PLAYER), v.player);
        ee.set_u32(state + ue::RUMBLER, if v.rumbler { rumbler } else { 0 });
        if v.run != 0 {
            ee.set_u32(v.run + 8, v.run_flags);
        }
        ee.set_u32(c(ue::RIDE_CLOCK), v.ride_clock.to_bits());
        ee.set_u32(c(ue::DOWN[0]), v.down[0].to_bits());
        ee.set_u32(c(ue::DOWN[1]), v.down[1].to_bits());
        ee.set_u32(c(ue::UP), v.up.to_bits());
        ee.set_u32(c(ue::FREE), v.free);
        ee.set_u32(c(ue::STUCK), v.stuck);
        ee.set_u32(c(ue::STUCK_TIME), v.stuck_time.to_bits());
        put_v(&mut ee, c(ue::LAST_FREE), v.last_free);
        let flags_word = ee.u32_at(c(ue::COLLISION_FLAGS));
        *ready.borrow_mut() = k.ready;
        log.borrow_mut().clear();
        modes.borrow_mut().clear();
        rumbled.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(SLOTS[k.slot].1, &[u64::from(CAR)], &[dt.0], 1_000_000);
        let written = ee.written_ranges();

        // The port.
        let (mut pu, mut pw, mut pc, mut pb) = (u, w.clone(), car, body);
        let mut port: Vec<Call> = Vec::new();
        atv_car::class_update(&mut pu, &mut pw, &mut pc, &mut pb, dt, &mut port);
        let mut after = car_words(&pc, &p);
        after.push((CAR + update::up::ON_HAZARD, word(pu.on_hazard)));
        after.extend(class_words(&pw, flags_word));

        let bad: Vec<String> = after
            .iter()
            .filter(|&&(a, x)| ee.u32_at(a) != x)
            .map(|&(a, x)| format!("  {a:#x}: port {x:#010x}, original {:#010x}", ee.u32_at(a)))
            .collect();
        assert!(bad.is_empty(), "case {case} ({}, {u:?}, {w:?}, dt {dt:?}): {} words differ\n{}", SLOTS[k.slot].0, bad.len(), bad.join("\n"));
        let theirs = log.borrow().iter().map(|c| format!("{c:?}")).collect::<Vec<_>>();
        let ours = port.iter().map(|c| format!("{c:?}")).collect::<Vec<_>>();
        assert_eq!(ours, theirs, "case {case} ({}, {u:?}, {w:?}): the calls", SLOTS[k.slot].0);
        let searched = port.contains(&Call::GhostSearch);
        assert_eq!(*modes.borrow(), if searched { vec![0, 9, 1] } else { vec![] }, "case {case}: the probe's modes");
        let rung = port.iter().find_map(|c| if let Call::RideRumble(l) = c { Some(*l) } else { None });
        let want: Vec<i32> = rung.filter(|_| k.ready).into_iter().collect();
        assert_eq!(*rumbled.borrow(), want, "case {case}: the rumbler");
        let compared: std::collections::BTreeSet<u32> = after.iter().map(|&(a, _)| a).collect();
        for &(from, to) in &written {
            for a in (from & !3..to).step_by(4) {
                let ours = blocks.iter().any(|&(b, n)| (b..b + n).contains(&a));
                assert!(!ours || compared.contains(&a), "case {case}: the original wrote {a:#x}, which is not compared");
            }
        }

        let has = |c: &Call| port.contains(c);
        let calls = |f: fn(&Call) -> bool| port.iter().any(f);
        let measured = u.vehicle_state.is_some();
        let riding = u.vehicle_state == Some(1);
        see(SLOTS[k.slot].0, measured);
        see("no vehicle state", !measured);
        see("still camera", u.still_camera);
        see("odometer on the ground", measured && car.grounded);
        let y = w.vehicle.ahead.y();
        see("odometer along the ahead axis", measured && !car.grounded && y > -1.0 && y < 1.0);
        see("odometer along the side axis", measured && !car.grounded && !(y > -1.0 && y < 1.0));
        see("odometer held at zero", measured && pw.vehicle.distance == w.vehicle.distance && dt != F::ZERO);
        see("collision on, carried", measured && k.carrier == 3 && !riding);
        see("collision on, riding", measured && riding);
        see("collision off, carrier without bit 1", measured && !riding && (1..3).contains(&k.carrier));
        see("collision off, unlisted", has(&Call::Unlist));
        see("collision off, body out", measured && !has(&Call::CollisionOn) && !body.enabled);
        see("placed", has(&Call::Place));
        see("rumble rung", rung.is_some() && k.ready);
        see("rumble switched off", rung.is_some() && !k.ready);
        see("rumble under its shock", measured && riding && car.state == atv_car::RIDING && rung.is_none() && w.vehicle.rumbler);
        see("rumble without a rumbler", measured && riding && car.state == atv_car::RIDING && !w.vehicle.rumbler);
        see("rumble wrecked", measured && riding && car.state != atv_car::RIDING);
        see("ghost counted", searched && !w.inside && pw.ghost != w.ghost);
        see("ghost held inside", searched && w.inside);
        see("ghost without a probe", !u.still_camera && w.ghost > F::ZERO && !w.probe);
        see("ghost over", has(&Call::GhostOver));
        see("ghost at the floor of a zero length", calls(|c| matches!(c, Call::GhostAlpha(a) if *a == F(0.1))) && w.ghost_length == F::ZERO);
        see("ghost fading", calls(|c| matches!(c, Call::GhostAlpha(a) if *a > F(0.1) && *a < F::ONE)));
        see("base update, no state", !measured && has(&Call::Parts(dt)) && !calls(|c| matches!(c, Call::Ride(_))));
        see("base update, free kept", measured && w.vehicle.free != 0);
        see("hazard clock put back", has(&Call::PutBack));
        see("hazard clock stuck counting", pw.vehicle.stuck_time > w.vehicle.stuck_time);

        if case == 0 {
            let (a, _) = after[after.len() - 1];
            let red: Vec<u32> = after
                .iter()
                .map(|&(b, y)| if b == a { (b, y ^ 1) } else { (b, y) })
                .filter(|&(b, y)| ee.u32_at(b) != y)
                .map(|(b, _)| b)
                .collect();
            assert_eq!(red, [a], "a flipped bit");
        }

        for (from, to) in written {
            for a in from..to {
                ee.write(a, 1, u128::from(pristine.u8_at(a)));
            }
        }
    }
    eprintln!("MD106e: {CASES} slot updates, seen {seen:?}");
    for (what, &n) in &seen {
        assert!(n >= 30, "{what} seen only {n} times");
    }
    assert_eq!(seen.len(), 30, "a branch was never seen: {seen:?}");
}
