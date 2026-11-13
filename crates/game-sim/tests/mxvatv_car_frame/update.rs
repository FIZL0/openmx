use super::*;

/// The car's update (the car class's vt `+0x10c`; the buggy's own slot runs its odometer, collision
/// and rumble first, then this) and its calls: the ghost step (the buggy's vt `+0x3a4`, a jump), the
/// hazard clock (vt `+0x45c`), the buggy's own step (vt `+0x488`) and the vehicle's base update.
const UPDATE: u32 = 0x004c_c4e0;
const GHOST: u32 = 0x004c_d4e0;
const HAZARD_CLOCK: u32 = 0x004a_ad60;
const CLASS_STEP: u32 = 0x004c_5510;
const VEHICLE_UPDATE: u32 = 0x004a_79c0;
/// The game's settings pointer, and the still-camera switch's byte and bit there.
const SETTINGS: u32 = 0x003e_ab60;
const STILL_BYTE: u32 = 0x225;
const STILL_BIT: u8 = 0x08;

/// The update's words (research/oracle.txt `# MD106d`).
pub(crate) mod up {
    pub const VEHICLE_STATE: u32 = 0xbc4;
    pub const MODE: u32 = 0x28;
    pub const ON_HAZARD: u32 = 0xc20;
    pub const SURFACES: u32 = 0xdf4;
    /// A tire's surface byte.
    pub const TIRE_SURFACE: u32 = 0xf0;
}

/// Hook the update's four calls: log them in the port's terms. Each must be on the player's car.
fn hook_update(ee: &mut Ee, log: Shared<Vec<Call>>) {
    type Make = fn(&Ee) -> Call;
    let calls: [(u32, Make); 4] = [
        (GHOST, |ee| Call::Ghost(F(ee.f(12)))),
        (HAZARD_CLOCK, |ee| {
            assert!(ee.r(5) < 2, "the hazard clock's sunk word {:#x}", ee.r(5));
            Call::HazardClock { dt: F(ee.f(12)), time: F(ee.f(13)), sunk: ee.r(5) != 0 }
        }),
        (CLASS_STEP, |_| Call::ClassStep),
        (VEHICLE_UPDATE, |ee| Call::VehicleUpdate(F(ee.f(12)))),
    ];
    for (addr, make) in calls {
        let l = log.clone();
        ee.hooks.insert(addr, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, CAR, "a call of the update on another object");
            l.borrow_mut().push(make(ee));
        }));
    }
}

/// A generated update: the image's car with its latch, tires' contact flags and surfaces stirred,
/// the still camera and the vehicle state's mode chosen, and a surface table (with the hazard words
/// as written: any set word is a hazard).
pub(crate) fn generate_update(r: &mut Rng, image_car: &Car) -> (Update, [u32; 16], Car, F) {
    let mut car = *image_car;
    car.latch = r.chance(0.6);
    car.latched = r.chance(0.5);
    car.latch_count = (r.next() % 8) as i32 - 2;
    let solid = [0.0, 0.2, 0.6][(r.next() % 3) as usize];
    for t in &mut car.tires {
        t.solid = r.chance(solid);
    }
    let mut u = Update {
        still_camera: r.chance(0.15),
        vehicle_state: if r.chance(0.15) { None } else { Some([3, 4, 1, 1, 0, 2, r.word()][(r.next() % 7) as usize]) },
        on_hazard: r.chance(0.5),
        ..Update::default()
    };
    let hazards = [0.0, 0.05, 0.4][(r.next() % 3) as usize];
    let mut words = [0u32; 16];
    for (w, s) in words.iter_mut().zip(&mut u.surfaces) {
        *w = if r.chance(hazards) { [1, 0x8000_0000, r.word() | 1][(r.next() % 3) as usize] } else { 0 };
        s.hazard = *w != 0;
        s.time = F(if r.chance(0.1) {
            -1.0
        } else if r.chance(0.15) {
            if r.chance(0.5) { -0.0 } else { 0.0 }
        } else {
            r.range(-1.0, 6.0)
        });
    }
    // Now and then all four on one surface, so the longest time is a tie.
    let one = (r.next() % 16) as u8;
    u.tire_surface = core::array::from_fn(|_| if r.chance(0.3) { one } else { (r.next() % 16) as u8 });
    if r.chance(0.1) {
        // The four tires' times all zeros of either sign: the longest is a `max.s` of signed zeros.
        for k in u.tire_surface {
            u.surfaces[usize::from(k)].time = F(if r.chance(0.5) { -0.0 } else { 0.0 });
        }
    }
    let dt = if r.chance(0.7) { F(1.0 / 30.0) } else { F(r.range(0.002, 0.05)) };
    (u, words, car, dt)
}

/// **The car's update is the original's**: SLUS-21104's update on the buggy image's car, its four
/// calls hooked, against [`atv_car::update`] over 10,000 generated updates: every word of the car and
/// its tires and the hazard flag equal, the same calls in the same order with the same arguments, and
/// nothing else written in those blocks. Each branch is seen.
#[test]
fn the_cars_update_is_the_originals() {
    let Some(pristine) = image() else { return };
    let Some(mut ee) = image() else { return };
    let p = ptrs(&ee);
    let image_car = read_car(&ee, &p);
    let state = ee.u32_at(CAR + up::VEHICLE_STATE);
    assert_ne!(state, 0, "the image's car has a vehicle state");
    let still_at = ee.u32_at(SETTINGS) + STILL_BYTE;
    let log: Shared<Vec<Call>> = Rc::default();
    hook_update(&mut ee, log.clone());
    let mut r = Rng(0x0106_d0da_7e00_0001);
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    let mut see = |what: &'static str, yes: bool| {
        if yes {
            *seen.entry(what).or_insert(0) += 1;
        }
    };
    let blocks: Vec<(u32, u32)> = std::iter::once((CAR, 0x1500)).chain(p.tires.map(|t| (t, 0x200))).collect();
    const CASES: usize = 10_000;
    for case in 0..CASES {
        let (u, hazard_words, mut car, dt) = generate_update(&mut r, &image_car);
        // The update's surface index is the tire's surface byte (MD127's [`Tire::surface`]).
        for (t, &k) in car.tires.iter_mut().zip(&u.tire_surface) {
            t.surface = k;
        }

        // The original.
        poke(&mut ee, &car_words(&car, &p));
        let still = ee.u8_at(still_at) & !STILL_BIT | if u.still_camera { STILL_BIT } else { 0 };
        ee.write(still_at, 1, u128::from(still));
        ee.set_u32(CAR + up::VEHICLE_STATE, if u.vehicle_state.is_some() { state } else { 0 });
        ee.set_u32(state + up::MODE, u.vehicle_state.unwrap_or(0));
        ee.set_u32(CAR + up::ON_HAZARD, word(u.on_hazard));
        for (k, (s, w)) in u.surfaces.iter().zip(hazard_words).enumerate() {
            let a = CAR + up::SURFACES + 0xc * k as u32;
            ee.set_u32(a, w);
            ee.set_u32(a + 4, s.time.to_bits());
        }
        for (t, &k) in p.tires.iter().zip(&u.tire_surface) {
            ee.write(t + up::TIRE_SURFACE, 1, u128::from(k));
        }
        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(UPDATE, &[u64::from(CAR)], &[dt.0], 1_000_000);
        let written = ee.written_ranges();

        // The port.
        let (mut pu, mut pc, mut pb) = (u, car, Body::default());
        let mut port = Port { log: Vec::new(), script: Script::default() };
        atv_car::update(&mut pu, &mut pc, &mut pb, dt, &mut port);
        let mut after = car_words(&pc, &p);
        after.push((CAR + up::ON_HAZARD, word(pu.on_hazard)));

        let bad: Vec<String> = after
            .iter()
            .filter(|&&(a, x)| ee.u32_at(a) != x)
            .map(|&(a, x)| format!("  {a:#x}: port {x:#010x}, original {:#010x}", ee.u32_at(a)))
            .collect();
        assert!(bad.is_empty(), "case {case} ({u:?}, dt {dt:?}): {} words differ\n{}", bad.len(), bad.join("\n"));
        let theirs = log.borrow().iter().map(|c| format!("{c:?}")).collect::<Vec<_>>();
        let ours = port.log.iter().map(|c| format!("{c:?}")).collect::<Vec<_>>();
        assert_eq!(ours, theirs, "case {case} ({u:?}): the calls");
        let compared: std::collections::BTreeSet<u32> = after.iter().map(|&(a, _)| a).collect();
        for &(from, to) in &written {
            for a in (from & !3..to).step_by(4) {
                let ours = blocks.iter().any(|&(b, n)| (b..b + n).contains(&a));
                assert!(!ours || compared.contains(&a), "case {case}: the original wrote {a:#x}, which is not compared");
            }
        }

        let latching = !u.still_camera && matches!(u.vehicle_state, None | Some(3 | 4));
        let measuring = !u.still_camera && !latching;
        let sunk = port.log.iter().any(|c| matches!(c, Call::HazardClock { sunk: true, .. }));
        see("still camera", u.still_camera);
        see("no vehicle state", latching && u.vehicle_state.is_none());
        see("mode 3", latching && u.vehicle_state == Some(3));
        see("mode 4", latching && u.vehicle_state == Some(4));
        see("latch counted", latching && car.latch && pc.latch);
        see("latch counted up from below zero", latching && car.latch && car.latch_count < 0);
        see("latch let go", latching && car.latch && !pc.latch);
        see("no latch", latching && !car.latch);
        see("measured", measuring);
        see("on a hazard, sunk", measuring && pu.on_hazard && sunk);
        see("on a hazard, not sunk", measuring && pu.on_hazard && !sunk);
        see("on no hazard, a tire solid", measuring && !pu.on_hazard && pc.tires.iter().any(|t| t.solid));
        see("a sign-bit hazard word", measuring && hazard_words.contains(&0x8000_0000) && pu.on_hazard);
        see("longest time a tie of signed zeros", measuring && {
            let t = u.tire_surface.map(|k| u.surfaces[usize::from(k)].time);
            t.iter().all(|x| *x == 0.0) && t.iter().any(|x| x.to_bits() != t[0].to_bits())
        });

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
    eprintln!("MD106d: {CASES} updates, seen {seen:?}");
    for (what, &k) in &seen {
        assert!(k >= 30, "{what} seen only {k} times");
    }
    assert_eq!(seen.len(), 14, "a branch was never seen: {seen:?}");
}
