// --- MD159: MD39b's five drives through the shell's car ----------------------------------------------
//
// The shell's own MX vs. ATV car (openmx's `MxvatvCarRide`, `crates/openmx/src/vehicle.rs` compiled in
// by path, the way openmx's tests take it), built off MD39's files as the shell builds it off a pack,
// seeded once from each drive's RAM image and then carried: each frame it takes the drive's tape
// through a pad and the shell's bindings and steps its own words, beside the interpreter's frame.

use super::drives::{Beside, HANDED_OUT, sort_answer, world_step_inside};
use super::whole::{NOISE_G1, NOISE_PERM, NOISE_UNBUILT, Oasis, terrain_file};
use super::*;
use game_sim::atv_contact;
use game_sim::atv_ground;
use game_sim::bindings::{Command, Table};
use game_sim::handling::record;
use game_sim::input::{Axis, AxisRange, Manager, Report};
use game_sim::noise::SmoothNoise;
use std::collections::BTreeMap;

#[path = "../mxvatv_car_replay/drive.rs"]
#[allow(dead_code)]
mod drive;
use drive::Drive;

#[path = "../../../openmx/src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;
use vehicle::MxvatvCarRide;

/// The commands the car's input step asks for (MD107's).
const ASKED: [Command; 8] = [Command::GAS, Command::BRAKE, Command(0x18), Command::CLUTCH, Command::LEFT, Command::RIGHT, Command::UP, Command::DOWN];

/// The shell's car for drive `d`, off MD39's `.phy` and chassis model (`context/mxvatv-ps2/md39`) as
/// the shell reads them off a car pack, or `None` (and a skip) without them.
fn shell_car(d: &Drive) -> Option<MxvatvCarRide> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let dir = kit.ctx("mxvatv-ps2/md39");
    let word = match d.name {
        "golf" => "golfcart",
        "truck" => "monstertruck",
        n => n,
    };
    let (phy, psg) = (format!("phy/{word}.phy"), format!("model/{word}.psg"));
    if !dir.join(&phy).exists() || !dir.join(&psg).exists() {
        skip(&format!("{} has no {phy} or {psg}: MD39's files", dir.display()));
        return None;
    }
    Some(MxvatvCarRide::read("md39", word, &phy, &psg, &mut |n: &str| std::fs::read(dir.join(n)).ok()).unwrap())
}

/// The pad report for tape tick `t`: each command the input step asks for on the tape's button for
/// it in MX vs. ATV's bindings ([`drive::button`]) held on the control the shell's bindings put it
/// on, at full pressure; the left stick's byte at the end the tape's stick names. Nothing held, the
/// stick centred, with no tick.
fn report(t: Option<&drive::Row>, table: &Table) -> Report {
    let Some(t) = t else { return Report::idle() };
    let held: Vec<&str> = t["buttons"].split(' ').collect();
    let mask = ASKED
        .into_iter()
        .filter(|&c| drive::button(c).is_some_and(|b| held.contains(&b)))
        .map(|c| table.control(0, c).unwrap_or_else(|| panic!("the shell binds no control to {c:?}")).mask())
        .fold(0, |m, b| m | b);
    let mut r = Report::from_buttons(mask);
    for (k, at) in [("lx", 6), ("ly", 7)] {
        r.0[at] = match t[k].as_str() {
            "-1.0" => 0x00,
            "1.0" => 0xff,
            "0.0" => r.0[at],
            s => panic!("the tape's stick {k} {s} is no end and not centred"),
        };
    }
    r
}

/// What a drive through the shell's car came to.
#[derive(Default)]
struct Seen {
    frames: usize,
    words: usize,
    handed: BTreeMap<String, usize>,
    sort_struck: usize,
    touching: usize,
    wrecks: u32,
}

/// Drive `d` replayed as MD135's test replays it in the interpreter, with the shell's car seeded from
/// the drive's image once (its keys, state, input keys and words, car, body and vehicle read off the
/// image into the car the shell makes) and then left to carry its own words: each frame, at the
/// original's world step, the car is stepped by [`MxvatvCarRide::step_with`] on a pad playing the
/// frame's tape tick (two back, as MD107's input step reads it) through the shell's shipped bindings,
/// its frame answered by MD135's world (Desert Oasis's ground, the image's noise tables, the sort's
/// search as the original's left it), and only the collision record's answers (its touch and its
/// held contacts, the world's records, run outside the car's frame) laid in from the machine. Every
/// word MD135 compares, and the input step's, must equal the original's after it, every frame.
fn through_shell(d: &Drive) -> Option<Seen> {
    let pre = drive::image(d.name)?;
    let ticks = drive::tape()?;
    let rows = drive::rows(d.name)?;
    let trn = terrain_file()?;
    let mut shell = shell_car(d)?;
    assert_eq!(pre.u32_at(d.car + 4), d.vtable, "{}: no car at {:#x}", d.name, d.car);
    assert_eq!(pre.u32_at(NOISE_UNBUILT), 0, "{}: the noise's tables are built", d.name);
    THE_CAR.with(|c| c.set(d.car));
    let trn: &'static [u8] = Box::leak(trn.into_boxed_slice());
    let map: &'static game_sim::track::Terrain<'static> = Box::leak(Box::new(game_sim::track::Terrain::parse(trn).unwrap()));
    let terrain: &'static atv_ground::Terrain<'static> = Box::leak(Box::new(atv_ground::Terrain::of(map)));
    let noise = SmoothNoise::from_tables(&std::array::from_fn(|k| pre.u32_at(NOISE_PERM + 4 * k as u32)), &std::array::from_fn(|k| F(pre.f32_at(NOISE_G1 + 4 * k as u32))));

    // The seed: the shell's car made, then the image's words laid in, once.
    {
        let p = ptrs(&pre);
        let o = own_ptrs(&pre, &p);
        let m = read_mx(&pre, &p);
        let mut r = shell.made(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0));
        (r.keys, r.state, r.car, r.body, r.vehicle) = (m.keys(), m.state, read_car(&pre, &p), seed::body(&|a| pre.u32_at(a), p.body), read_vehicle(&pre, &p, &o));
        (r.input_keys, r.input) = (drive::keys_of(&pre, d.car), drive::input_of(&pre, d.car));
        shell.seed(r);
    }
    let s = drive::sim(&pre);
    let mut ee = drive::hooked(pre);
    let table = Table::SHIPPED;
    let dt = F::from_bits(drive::DT);
    let seen: Shared<Seen> = Rc::default();
    let frame_no: Shared<usize> = Rc::default();
    let shell: Shared<MxvatvCarRide> = Rc::new(RefCell::new(shell));
    let input = Rc::new(RefCell::new(Manager::connected()));
    let (keep, at_frame, name, car_word) = (seen.clone(), frame_no.clone(), d.name, d.car);
    let (pad, ticks_in) = (input.clone(), Rc::new(ticks.clone()));
    let shell_in = shell.clone();
    ee.hooks.insert(WORLD_STEP, Box::new(move |ee: &mut Ee| {
        let k = *at_frame.borrow();
        let p = ptrs(ee);
        let o = own_ptrs(ee, &p);
        let w = ee.r(4) as u32;
        let listeners: Vec<u32> = (0..ee.u32_at(w + 0x98)).map(|i| ee.u32_at(ee.u32_at(w + 0xa0) + 4 * i)).collect();
        if !listeners.contains(&(car_at() + 0x30)) {
            world_step_inside(ee);
            return;
        }
        // The collision record's answers as the world's records left them for this frame: its touch
        // (the vehicle's word and the pieces' copy) and its held contacts.
        let contacts = ee.u32_at(p.record + 0xf0);
        let fresh = read_vehicle(ee, &p, &o);
        let held = if contacts != 0 { record::held(&|a| ee.u32_at(a), p.record) } else { fresh.held.clone() };
        let m = read_mx(ee, &p);
        let (_, step_dt) = world_step_inside(ee);
        assert_eq!(step_dt.to_bits(), dt.to_bits(), "{name} frame {k}: the world's tick is the shell's");
        let sort = sort_answer(ee, &p);
        // Frame k's pad: tape tick k - 2, as the image's own words stand before it.
        let tick = k.checked_sub(2).and_then(|t| ticks_in.get(t));
        let mut mgr = pad.borrow_mut();
        mgr.tick(dt, &[report(tick, &table), Report::idle()]);
        let pad0 = mgr.pad(0);
        if let Some(t) = tick {
            let held_names: Vec<&str> = t["buttons"].split(' ').collect();
            for c in ASKED {
                let want = if drive::button(c).is_some_and(|b| held_names.contains(&b)) { F::ONE } else { F::ZERO };
                assert_eq!(table.value(pad0, c).to_bits(), want.to_bits(), "{name} frame {k}: {c:?} through the shell's bindings");
            }
            for (axis, key) in [(Axis::LeftX, "lx"), (Axis::LeftY, "ly")] {
                let want = F(t[key].parse::<f32>().unwrap());
                assert_eq!(pad0.axis_through(axis, &AxisRange::PLAYER).to_bits(), want.to_bits(), "{name} frame {k}: the stick's {key}");
            }
        }
        let mut sh = shell_in.borrow_mut();
        {
            let r = sh.ridden.as_deref_mut().expect("the shell's car is out");
            (r.vehicle.touching, r.vehicle.held, r.state.touching) = (fresh.touching, held, m.state.touching);
        }
        let oasis = Oasis { terrain, noise: noise.clone(), knot: None, asked: Vec::new(), rest: Vec::new() };
        let rest = sh.step_with(pad0, &table, dt, Beside { oasis, sort }).expect("the shell's car is out");
        let r = sh.ridden.as_deref().unwrap();
        let pm = Mx { keys: Keys { tires: r.keys.tires.clone(), ..m.keys.clone() }, state: r.state, ..m };
        let resolved: Vec<(u32, u32)> = r.vehicle.held.as_ref().filter(|_| contacts != 0).map_or(Vec::new(), |h| record::held_words(&atv_contact::Held { body: None, ..h.clone() }, p.record, &|a| ee.u32_at(a)));
        let parts = [car_words(&r.car, &p), seed::body_words(&r.body, p.body), mx_words(&pm, &p), vehicle_words(ee, &r.vehicle, &p, &o), resolved];
        let counts = parts.clone().map(|ws| ws.iter().filter(|&&(a, x)| ee.u32_at(a) != x).count());
        let after: Vec<(u32, u32)> = parts.concat();
        let bad: Vec<String> = after
            .iter()
            .filter(|&&(a, x)| ee.u32_at(a) != x)
            .take(40)
            .map(|&(a, x)| format!("  {a:#x} (car +{:#x}): shell {x:#010x} ({}), original {:#010x} ({})", a.wrapping_sub(car_at()), f32::from_bits(x), ee.u32_at(a), ee.f32_at(a)))
            .collect();
        assert!(
            bad.is_empty(),
            "{name} frame {k}: words of the shell's car differ from the original's (car, body, pieces, vehicle, record: {counts:?}; {contacts} contacts, sort struck {}, state {}, handed out {:?}):\n{}",
            sort.found,
            r.car.state,
            rest.oasis.rest,
            bad.join("\n")
        );
        assert_eq!(format!("{:?}", r.input), format!("{:?}", drive::input_of(ee, car_word)), "{name} frame {k}: the shell's input step, then the original's words");
        let mut s = keep.borrow_mut();
        for c in &rest.oasis.rest {
            let n = format!("{c:?}").split([' ', '(', '{']).next().unwrap().to_string();
            assert!(HANDED_OUT.contains(&n.as_str()), "{name} frame {k}: the shell's car handed out {c:?}");
            *s.handed.entry(n).or_default() += 1;
        }
        s.frames += 1;
        s.words += after.len();
        s.sort_struck += usize::from(sort.found);
        s.touching += usize::from(contacts != 0);
        s.wrecks = sh.wrecks;
    }));
    let mut j = 0;
    for _ in 0..rows.len() {
        *frame_no.borrow_mut() = j + 1;
        drive::step(&mut ee, s, d, &ticks, &mut j, 2);
    }
    let seen = std::mem::take(&mut *seen.borrow_mut());
    assert_eq!(seen.frames, rows.len(), "{}: the world step ran once a frame", d.name);
    let sh = shell.borrow();
    let report = sh.report().expect("the shell's car is out");
    eprintln!(
        "MD159 {}: {} frames through the shell's car, carried from the image, {} words in all equal to the original's; handed out {:?}; the sort struck on {} frames, the record held contacts on {}; {}",
        d.name, seen.frames, seen.words, seen.handed, seen.sort_struck, seen.touching, report
    );
    Some(seen)
}

fn shell_drive(d: &Drive) {
    let Some(seen) = through_shell(d) else { return };
    assert_eq!(seen.wrecks, 0, "{}: the shell's car wrecked on a drive the original never wrecks on", d.name);
    assert!(!seen.handed.contains_key("Respawn"), "{}: the shell's car answers the respawn itself", d.name);
}

/// **MD159: the buggy's drive through the shell's car**: MD39b's drive's tape fed to the shell's
/// `MxvatvCarRide` seeded from the drive's image, every word equal to the interpreter's each frame.
#[test]
fn the_buggys_drive_through_the_shells_car() {
    shell_drive(&drive::BUGGY);
}

/// **The baja's.**
#[test]
fn the_bajas_drive_through_the_shells_car() {
    shell_drive(&drive::BAJA);
}

/// **The sandrail's.**
#[test]
fn the_sandrails_drive_through_the_shells_car() {
    shell_drive(&drive::SANDRAIL);
}

/// **The golf cart's.**
#[test]
fn the_golf_carts_drive_through_the_shells_car() {
    shell_drive(&drive::GOLF);
}

/// **The monster truck's.**
#[test]
fn the_trucks_drive_through_the_shells_car() {
    shell_drive(&drive::TRUCK);
}
