// --- MD156: MD107's five drives, the port's update beside the interpreter's ----------------------------

use super::class_update::{SETTINGS, SLOTS, STILL_BIT, STILL_BYTE, class_words, hook_class, read_class, ue};
use super::update::up;
use super::*;
use game_sim::atv_car::Updated;
use std::collections::BTreeMap;

/// The car class's own per-frame step (vt `+0x488`), its update slot (vt `+0x10c`).
const CLASS_STEP_SLOT: u32 = 0x488;
const UPDATE_SLOT: u32 = 0x10c;

/// What the drive's update came to: the player's car's updates stepped beside the interpreter, the
/// words compared, how often each call was handed out, the class steps whose part words were
/// compared, and the slot's runs on other objects.
#[derive(Default)]
pub(crate) struct UpdateSeen {
    pub frames: usize,
    pub words: usize,
    pub handed: BTreeMap<String, usize>,
    pub posed: usize,
    pub others: usize,
    /// Updates with the car off the ground (the odometer along the node's heading), and on a hazard.
    pub airborne: usize,
    pub hazard: usize,
}

/// The update's words off the machine (research/oracle.txt `# MD106d`).
fn read_update(ee: &Ee, p: &Ptrs) -> Update {
    let c = |o: u32| car_at() + o;
    let state = ee.u32_at(c(up::VEHICLE_STATE));
    Update {
        still_camera: ee.u8_at(ee.u32_at(SETTINGS) + STILL_BYTE) & STILL_BIT != 0,
        vehicle_state: (state != 0).then(|| ee.u32_at(state + up::MODE)),
        surfaces: core::array::from_fn(|j| {
            let a = c(up::SURFACES) + 0xc * j as u32;
            game_sim::quad::Surface { hazard: ee.u32_at(a) != 0, time: F(ee.f32_at(a + 4)) }
        }),
        tire_surface: p.tires.map(|t| ee.u8_at(t + up::TIRE_SURFACE)),
        on_hazard: ee.u32_at(c(up::ON_HAZARD)) != 0,
        untamed: false,
    }
}

/// The slot's and the base update's words off the machine ([`read_class`]), with what MD106e's
/// cases poke rather than read: the vehicle state's rumbler, and whether the car's carrier's node
/// has its flag bit 1.
fn read_updated(ee: &Ee, p: &Ptrs) -> Updated {
    let c = |o: u32| car_at() + o;
    let mut w = read_class(ee, p);
    let state = w.vehicle.state;
    w.vehicle.rumbler = state != 0 && ee.u32_at(state + ue::RUMBLER) != 0;
    let carrier = ee.u32_at(c(ue::CARRIER));
    let node = if carrier != 0 { ee.u32_at(carrier + ue::CARRIER_NODE) } else { 0 };
    w.carried = node != 0 && ee.u32_at(node + ue::NODE_FLAGS) & 2 != 0;
    w
}

/// Runs `addr` on `car` and `dt` from inside its hook, leaving the machine as a return from it would.
fn inside(ee: &mut Ee, addr: u32, car: u32, dt: F) {
    let (sp, ra, stack) = (ee.r(29), ee.r(31), ee.stack);
    ee.stack = (sp as u32 - 0x100) & !0xf;
    ee.call(addr, &[u64::from(car)], &[dt.0], 400_000_000);
    (ee.stack, ee.pc) = (stack, addr);
    ee.set(29, sp);
    ee.set(31, ra);
}

/// Hooks the drive's car's update slot: each time it runs on the player's car, the slot's words, the
/// update's, the car and its body are read off the machine and [`atv_car::class_update`] runs on
/// them; then the original's slot runs once on a copy of the machine with what the port hands out
/// hooked (MD106e's [`hook_class`]: each logged, none run), the buggy class's own step run as the
/// original's (its part words against [`atv_car::class_step`] over the RAM the slot started on, as
/// MD106e's test), the golf cart's and the monster truck's likewise against
/// [`atv_car::golf_class_step`] (MD156a) and [`atv_car::truck_class_step`] (MD156b); every word the
/// port keeps must equal the
/// original's, the calls the same in the same order, and the original must write nothing else in the
/// car, its tires or its body. The machine is put back and the original's slot runs for real, so the
/// drive goes on as the original's.
pub(crate) fn hook_update(ee: &mut Ee, name: &'static str, frame_no: Shared<usize>, seen: Shared<UpdateSeen>) {
    let vtable = ee.u32_at(car_at() + 4);
    let slot = ee.u32_at(vtable + UPDATE_SLOT);
    assert!(SLOTS.iter().any(|&(_, s)| s == slot), "{name}: the car's update slot {slot:#x} is not one of MD106e's");
    let class_step = ee.u32_at(vtable + CLASS_STEP_SLOT);
    let buggy = class_step == super::class_step::CLASS_STEP;
    // MD156a: the golf cart's own step, ported too.
    let golf = class_step == super::golf_step::GOLF_CLASS_STEP;
    // MD156b: and the monster truck's.
    let truck = class_step == super::truck_step::TRUCK_CLASS_STEP;
    let curve = super::class_step::curve_table(ee);
    let acos = super::class_step::acos_table(ee);
    let mut saved: Vec<u8> = Vec::new();
    let mut saved_spr: Vec<u8> = Vec::new();
    ee.hooks.insert(slot, Box::new(move |ee: &mut Ee| {
        let (car_addr, dt) = (ee.r(4) as u32, F(ee.f(12)));
        if car_addr != car_at() {
            inside(ee, slot, car_addr, dt);
            seen.borrow_mut().others += 1;
            return;
        }
        let k = *frame_no.borrow();
        let p = ptrs(ee);
        let car = read_car(ee, &p);
        let body = seed::body(&|a| ee.u32_at(a), p.body);
        let u = read_update(ee, &p);
        let w = read_updated(ee, &p);
        let flags_word = ee.u32_at(car_addr + ue::COLLISION_FLAGS);

        // The port.
        let (mut pu, mut pw, mut pc, mut pb) = (u, w.clone(), car, body);
        let mut port: Vec<Call> = Vec::new();
        atv_car::class_update(&mut pu, &mut pw, &mut pc, &mut pb, dt, &mut port);
        let stepped = port.contains(&Call::ClassStep);
        let parts = super::class_step::read_parts(ee);
        let posed = if buggy && stepped {
            let mut over = super::class_step::Over { ee, words: BTreeMap::new() };
            atv_car::class_step(&mut over, &parts, &super::class_step::read_posing(ee), &curve, &acos);
            over.words
        } else if golf && stepped {
            super::golf_step::golf_port(ee, &curve)
        } else if truck && stepped {
            super::truck_step::truck_port(ee, &curve, &acos)
        } else {
            BTreeMap::new()
        };
        let mut after = car_words(&pc, &p);
        after.push((car_addr + up::ON_HAZARD, word(pu.on_hazard)));
        after.extend(class_words(&pw, flags_word));

        // The original, on a copy of the machine, with the port's hand-outs hooked.
        saved.resize(ee.ram.len(), 0);
        saved.copy_from_slice(&ee.ram);
        saved_spr.resize(ee.spr.len(), 0);
        saved_spr.copy_from_slice(&ee.spr);
        let before: std::collections::BTreeSet<u32> = ee.hooks.keys().copied().collect();
        let log: Shared<Vec<Call>> = Rc::default();
        let modes: Shared<Vec<u64>> = Rc::default();
        let state = ee.u32_at(car_addr + up::VEHICLE_STATE);
        hook_class(ee, &p, state, log.clone(), modes.clone(), Rc::default(), Rc::default());
        let l = log.clone();
        if buggy || golf || truck {
            ee.hooks.insert(class_step, Box::new(move |ee: &mut Ee| {
                assert_eq!(ee.r(4) as u32, car_at(), "the class step on something else");
                l.borrow_mut().push(Call::ClassStep);
                let written = ee.written.clone();
                inside(ee, class_step, car_at(), F::ZERO);
                ee.written.extend(written);
            }));
        } else {
            ee.hooks.insert(class_step, Box::new(move |ee: &mut Ee| {
                assert_eq!(ee.r(4) as u32, car_at(), "the class step on something else");
                l.borrow_mut().push(Call::ClassStep);
            }));
        }
        let hooks: Vec<u32> = ee.hooks.keys().copied().filter(|a| !before.contains(a)).collect();
        assert!(!before.contains(&class_step), "{name}: the class step was hooked already");
        inside(ee, slot, car_addr, dt);
        let written = ee.written_ranges();

        let bad: Vec<String> = after
            .iter()
            .filter(|&&(a, x)| ee.u32_at(a) != x)
            .take(40)
            .map(|&(a, x)| format!("  {a:#x} (car +{:#x}): port {x:#010x}, original {:#010x}", a.wrapping_sub(car_addr), ee.u32_at(a)))
            .collect();
        assert!(bad.is_empty(), "{name} frame {k}: words of the port's update differ from the original's ({u:?}, {w:?}, dt {dt:?}):\n{}", bad.join("\n"));
        let theirs: Vec<String> = log.borrow().iter().map(|c| format!("{c:?}")).collect();
        let ours: Vec<String> = port.iter().map(|c| format!("{c:?}")).collect();
        assert_eq!(ours, theirs, "{name} frame {k}: the update's calls");
        let searched = port.contains(&Call::GhostSearch);
        assert_eq!(*modes.borrow(), if searched { vec![0, 9, 1] } else { vec![] }, "{name} frame {k}: the probe's modes");
        if buggy {
            let bad = super::class_step::compare_nodes(ee, &posed, &super::class_step::nodes(&parts));
            assert!(bad.is_empty(), "{name} frame {k}: the class step's parts (stepped {stepped}): {} words differ\n{}", bad.len(), bad.join("\n"));
        }
        if golf {
            let nodes = super::golf_step::golf_nodes(&super::golf_step::read_golf_parts(ee));
            let bad = super::class_step::compare_nodes(ee, &posed, &nodes);
            assert!(bad.is_empty(), "{name} frame {k}: the golf cart's class step's parts (stepped {stepped}): {} words differ\n{}", bad.len(), bad.join("\n"));
        }
        if truck {
            let nodes = super::truck_step::truck_nodes(&super::truck_step::read_truck_parts(ee));
            let bad = super::class_step::compare_nodes(ee, &posed, &nodes);
            assert!(bad.is_empty(), "{name} frame {k}: the monster truck's class step's parts (stepped {stepped}): {} words differ\n{}", bad.len(), bad.join("\n"));
        }
        // The class step's own car words (the golf cart's steering frame) are compared with its parts.
        let compared: std::collections::BTreeSet<u32> = after.iter().map(|&(a, _)| a).chain(posed.keys().copied()).collect();
        let blocks: Vec<(u32, u32)> = [(car_addr, 0x1500), (p.body, 0x230)].into_iter().chain(p.tires.map(|t| (t, 0x200))).collect();
        for &(from, to) in &written {
            for a in (from & !3..to).step_by(4) {
                let ours = blocks.iter().any(|&(b, n)| (b..b + n).contains(&a));
                assert!(!ours || compared.contains(&a), "{name} frame {k}: the original's update wrote {a:#x} (car +{:#x}), which is not compared", a.wrapping_sub(car_addr));
            }
        }

        // Put the machine back and run the original's slot for real.
        for a in hooks {
            ee.hooks.remove(&a);
        }
        ee.ram.copy_from_slice(&saved);
        ee.spr.copy_from_slice(&saved_spr);
        inside(ee, slot, car_addr, dt);

        let mut s = seen.borrow_mut();
        for c in &port {
            let n = format!("{c:?}").split([' ', '(', '{']).next().unwrap().to_string();
            *s.handed.entry(n).or_default() += 1;
        }
        s.frames += 1;
        s.words += after.len();
        s.posed += usize::from((buggy || golf || truck) && stepped);
        s.airborne += usize::from(!car.grounded);
        s.hazard += usize::from(pu.on_hazard);
    }));
}
