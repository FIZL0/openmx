//! P103: the lot's machines at their bays — drawn see-through, colliding with nothing, offered by
//! the original's own pop-up when the rider stops beside one, and gone, all of it, with the setting
//! off.
//!
//! The rule the pop-up follows is the original's trigger (`research/oracle.txt`'s
//! `freeride_near_machine` and `freeride_switch_prompt`): the nearest of each kind in the original's
//! order, closer than 30 ft in three dimensions, to a rider who is not wrecked and not above
//! 58.667 ft/s. Its picture is the one PCSX2 draws on free09 (journal 2026-10-22-p103), measured
//! into `overlay.rs`; the drawing tests load the disc's art and skip without it.
//!
//! `src/vehicle.rs`, `src/overlay.rs` and what they lean on are compiled in by path, the way
//! `tests/overlay.rs` takes them.

// the browser's own clock, threads and files, which are std's here (P196)
#[allow(unused)]
#[path = "../src/files.rs"]
mod files;
#[path = "../src/menu.rs"]
#[allow(dead_code)]
mod menu;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;
#[path = "../src/roster.rs"]
#[allow(dead_code)]
mod roster;
#[path = "../src/chase.rs"]
#[allow(dead_code)]
mod chase;
#[path = "../src/join.rs"]
#[allow(dead_code)]
mod join;
#[path = "../src/split.rs"]
#[allow(dead_code)]
mod split;
#[path = "../src/overlay.rs"]
#[allow(dead_code)]
mod overlay;

use game_render::{Disc, Frame};
use game_sim::Sim;
use game_sim::ghost::Ghost;
use game_sim::input::{Control, Report};
use game_sim::ride::Controls;
use game_sim::world::Flat;
use ps2_float::{F, Vf};
use vehicle::{Answer, Bay, Garage, Kind, Lot, Machine, Plane};

/// A `machine.txt` in the disc's shape: two of every machine, the planes 30 ft apart on flat ground.
const LOT: &str = "[Machines]\nNumberOfMachines = 1\n\n\
    [Biplane1]\nPosition = 100, 0, 0\nDirection = 0, 0, 1\n\
    [Biplane2]\nPosition = 100, 0, 300\nDirection = 0, 0, 1\n\
    [Helicopter1]\nPosition = 0, 0, 20\nDirection = 1, 0, 0\n[Helicopter2]\nPosition = 0, 0, 80\nDirection = 1, 0, 0\n\
    [Buggy1]\nPosition = 0, 0, -20\nDirection = 0, 0, 1\n[Buggy2]\nPosition = 20, 0, -20\nDirection = 0, 0, 1\n\
    [Monster1]\nPosition = -20, 0, 0\nDirection = 0, 0, 1\n[Monster2]\nPosition = -40, 0, 0\nDirection = 0, 0, 1\n\
    [Baja1]\nPosition = 0, 0, 200\nDirection = 0, 0, 1\n[Baja2]\nPosition = 20, 0, 200\nDirection = 0, 0, 1\n";

fn lot(players: usize) -> Lot {
    Lot::new(LOT.as_bytes(), players, &mut |_| 0.0)
}

fn at(x: f32, y: f32, z: f32) -> Vf {
    Vf::new(x, y, z, 1.0)
}

/// The bays are the lot's machines the port has — the planes, one a player — drawn see-through at
/// the put-back ghost's own alpha; with the setting off there are none, so nothing is drawn there,
/// marked or offered.
#[test]
fn a_bay_is_a_see_through_plane_and_off_is_nothing() {
    for players in [1, 2, 4] {
        let lot = lot(players);
        let shown = vehicle::bays_shown(&lot, true);
        assert_eq!(shown.len(), players, "one plane a player: {shown:?}");
        assert!(shown.iter().all(|b| b.machine == Machine::Biplane));
        assert_eq!(shown, lot.of(Machine::Biplane).copied().collect::<Vec<Bay>>());
        for bay in &shown {
            let plane = vehicle::bay_plane(bay);
            assert_eq!((plane.at, plane.facing, plane.livery), (bay.spawn.at, bay.spawn.facing, bay.livery));
            assert!(plane.flight.is_none(), "a bay's plane is never flown");
        }
        assert!(vehicle::bays_shown(&lot, false).is_empty(), "the setting off shows no bay");
        assert_eq!(vehicle::offer(&vehicle::bays_shown(&lot, false), Kind::Bike, at(100.0, 0.0, 10.0), 0.0, false), None);
    }
    assert!(vehicle::BAY_ALPHA < 1.0 && vehicle::BAY_ALPHA > 0.0);
    assert_eq!(vehicle::BAY_ALPHA, Ghost::START.0, "the put-back ghost's alpha");
}

/// Nothing collides with a bay: a bike ridden flat out straight through one rides exactly as it does
/// on a track with no bays at all, tick for tick, and comes out the other side. The plane's bays
/// are in no world the bike asks (and no world the plane asks: it flies over the heightfield alone).
#[test]
fn a_bike_rides_straight_through_a_bay() {
    let ride = |with_lot: bool| -> Vec<(f32, f32, f32)> {
        let mut sim = Sim::new(0);
        let start = sim.rider.ride.position();
        let (forward, _) = sim.rider.ride.axes();
        // A bay 40 ft dead ahead of the bike, at its height.
        let txt = format!(
            "[Biplane1]\nPosition = {}, {}, {}\nDirection = 0, 0, 1\n",
            start.x() + forward.x() * 40.0,
            start.y(),
            start.z() + forward.z() * 40.0
        );
        let lot = Lot::new(txt.as_bytes(), 1, &mut |_| 0.0);
        let bays = vehicle::bays_shown(&lot, with_lot);
        assert_eq!(bays.len(), usize::from(with_lot));
        let mut garage = Garage::new(Kind::Bike, lot.plane(0, None)).unwrap();
        let mut world = Flat::default();
        let gas = Controls { gas: F(1.0), ..Controls::default() };
        (0..240)
            .map(|_| {
                garage.step(&mut sim, F(1.0 / 30.0), &gas, &mut world);
                let p = sim.rider.ride.position();
                (p.x(), p.y(), p.z())
            })
            .collect()
    };
    let (through, alone) = (ride(true), ride(false));
    assert_eq!(through, alone, "a bay changed the ride");
    let start = Sim::new(0).rider.ride.position();
    let (fx, fz) = (through[239].0 - start.x(), through[239].2 - start.z());
    assert!(fx.hypot(fz) > 80.0, "the bike only got {:.1} ft, not past the bay", fx.hypot(fz));
}

/// The original's trigger: the nearest bay of the first ported kind, closer than 30 ft in three
/// dimensions and not on top of the rider, to a rider upright and no faster than 40 mph — and never
/// the machine the rider is on.
#[test]
fn the_pop_up_offers_as_the_originals_trigger_does() {
    let lot = lot(2);
    let bays = vehicle::bays_shown(&lot, true);
    let offer = |p: Vf, speed: f32, wrecked: bool| vehicle::offer(&bays, Kind::Bike, p, speed, wrecked);
    // Player 1's plane stands at 100, 0, 0.
    assert_eq!(offer(at(100.0, 0.0, 29.9), 0.0, false), Some(Machine::Biplane));
    assert_eq!(offer(at(100.0, 0.0, 30.0), 0.0, false), None, "30 ft is out: the original's < 30");
    assert_eq!(offer(at(118.0, 18.0, 18.0), 0.0, false), None, "31 ft in three dimensions, 25 on the ground");
    assert_eq!(offer(at(100.0, 0.0, 0.0), 0.0, false), None, "the nearest search skips what is on top of it");
    // Player 2's copy, 300 ft off, is offered beside it the same way: any copy, the nearest.
    assert_eq!(offer(at(100.0, 2.0, 310.0), 0.0, false), Some(Machine::Biplane));
    // Speed either way, up to 58.667 ft/s and not past it; and never to a wrecked rider.
    assert_eq!(offer(at(100.0, 0.0, 10.0), 58.6, false), Some(Machine::Biplane));
    assert_eq!(offer(at(100.0, 0.0, 10.0), -58.6, false), Some(Machine::Biplane));
    assert_eq!(offer(at(100.0, 0.0, 10.0), 58.7, false), None);
    assert_eq!(offer(at(100.0, 0.0, 10.0), 0.0, true), None);
    // The other machines are not ported, so standing beside them offers nothing.
    assert_eq!(offer(at(-20.0, 0.0, 5.0), 0.0, false), None, "a monster truck's bay");
    // And the plane is not offered to its own pilot.
    assert_eq!(vehicle::offer(&bays, Kind::Biplane, at(100.0, 0.0, 10.0), 0.0, false), None);
    assert_eq!(vehicle::offer_text(Machine::Biplane), "Would you like to switch to the Biplane?");
}

/// Circle answers the pop-up on the frame it goes down: one press, one switch, and the switch is
/// P102's, the player's own plane spawned where the bike was. A Circle held into range does not
/// take it, nor one pressed with nothing offered.
#[test]
fn circle_takes_the_offered_plane_onto_the_player() {
    let lot = lot(1);
    let bays = vehicle::bays_shown(&lot, true);
    let mut sim = Sim::new(0);
    let mut floor = Flat::default();
    let mut garage = Garage::new(Kind::Bike, lot.plane(0, None)).unwrap();
    let bay = lot.bay(Machine::Biplane, 0).unwrap().spawn;
    // Stand the bike 20 ft from the bay, stopped.
    let near = at(bay.at.x() + 12.0, 0.0, bay.at.z() + 16.0);
    sim.rider.teleport_to(game_sim::markers::Spot::still(near, Vf::new(0.0, 0.0, 1.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0)));
    let here = sim.rider.ride.position();
    let offered = vehicle::offer(&bays, garage.up, here, sim.rider.ride.chassis.speed.0, sim.rider.crash.is_some());
    assert_eq!(offered, Some(Machine::Biplane));

    // Circle down while nothing was offered, then held into range: no switch.
    let mut answer = Answer::default();
    assert_eq!(answer.take(true, false, None), None);
    assert_eq!(answer.take(true, false, offered), None, "a held Circle is not a press");
    assert_eq!(answer.take(false, false, offered), None);
    // Pressed: the plane, once.
    let to = answer.take(true, false, offered);
    assert_eq!(to, Some(Kind::Biplane));
    assert_eq!(answer.take(true, false, offered), None, "held on, it is not taken again");
    // A tap between two frames is a press whatever the frames say.
    assert_eq!(answer.take(false, true, offered), Some(Kind::Biplane));

    assert_eq!(garage.switch(to.unwrap(), &mut sim, &mut floor), Ok(Kind::Biplane));
    let plane = garage.plane_flown().expect("the plane is up");
    assert_eq!((plane.at.x(), plane.at.z()), (here.x(), here.z()), "spawned on the player, not at the bay");
    assert_eq!(plane.bay, bay, "the bay stays where it is, and is drawn there still");
    assert_eq!(vehicle::bays_shown(&lot, true).len(), 1);
    // In the plane the bay offers nothing.
    assert_eq!(vehicle::offer(&bays, garage.up, plane.at, 0.0, false), None);
    // Back on the bike the plane flown is drawn nowhere; the bay's is.
    assert_eq!(garage.switch(Kind::Bike, &mut sim, &mut floor), Ok(Kind::Bike));
    assert!(garage.plane_flown().is_none());
}

// ---- the pop-up, drawn off the disc --------------------------------------------------------------

fn art() -> Option<overlay::Art> {
    match Disc::open_configured() {
        Ok(mut disc) => Some(overlay::Art::load(&mut disc).expect("fstyleui.res's HUD art")),
        Err(e) => {
            eprintln!("skipped, no disc: {e}");
            None
        }
    }
}

fn gauges() -> overlay::Gauges {
    overlay::Gauges { bike: true, ..Default::default() }
}

/// The pop-up in a full 640x448 view, where PCSX2's picture was measured: the question in the
/// original's two lines, the panel darkening what is under it, the black pill, the Circle in the
/// original's red. Not offered, or with the setting off (no bays, so nothing offered), none of it.
#[test]
fn in_range_the_pop_up_is_drawn_and_off_it_is_not() {
    let Some(mut art) = art() else { return };
    let sim = Sim::new(0);
    let riders = vec![chase::bike(&sim.rider.ride)];
    let grey = [96u8, 96, 96];

    // The original's own break, on the truck it was seen on, and the plane's the same way.
    let mut lines = |m: Machine| {
        art.offers = vec![Some(m)];
        let mut frame = Frame::new(640, 448, grey);
        let drawn = art.draw(&mut frame, (640, 448), &riders, 0, 100.0, &gauges());
        (drawn, frame)
    };
    let (truck, truck_frame) = lines(Machine::Monster);
    assert_eq!(truck.offer, Some(vec!["Would you like to switch".to_string(), "to the Monster Truck?".to_string()]));
    let rgb = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("p103-pop-up.rgb");
    std::fs::write(&rgb, &truck_frame.color).expect("the picture is written");
    let (drawn, frame) = lines(Machine::Biplane);
    eprintln!("the pop-up's picture: {} (640x448 RGB)", rgb.display());
    assert_eq!(drawn.offer, Some(vec!["Would you like to switch".to_string(), "to the Biplane?".to_string()]));
    let (bx, by) = drawn.offer_button.expect("the button");
    assert!((bx - 286.0).abs() < 0.5 && (by - 403.0).abs() < 0.5, "the Circle at {bx}, {by}");
    let px = |x: usize, y: usize| -> [u8; 3] { frame.color[3 * (y * 640 + x)..][..3].try_into().unwrap() };
    // The panel, see-through: the grey under it at 0.42.
    let panel = px(200, 380);
    assert!(panel.iter().all(|&c| (36..=44).contains(&c)), "the panel is {panel:?}");
    // Its rounded corner leaves the very corner alone.
    assert_eq!(px(181, 317), grey);
    // The pill is black, and the Circle red.
    assert_eq!(px(380, 390), [0, 0, 0]);
    let ring = (393..413).flat_map(|y| (278..294).map(move |x| (x, y))).map(|(x, y)| px(x, y)).filter(|c| c[0] > 150 && c[1] < 100).count();
    assert!(ring > 20, "{ring} red pixels in the button");
    // The question is written in the original's grey over the panel.
    let text = (330..380).flat_map(|y| (200..440).map(move |x| (x, y))).filter(|&(x, y)| px(x, y)[0] > 150).count();
    assert!(text > 400, "{text} light pixels of question");

    // Nothing offered: none of it.
    art.offers = vec![None];
    let mut bare = Frame::new(640, 448, grey);
    let drawn = art.draw(&mut bare, (640, 448), &riders, 0, 100.0, &gauges());
    assert_eq!((drawn.offer, drawn.offer_button), (None, None));
    assert_eq!(&bare.color[3 * (380 * 640 + 200)..][..3], &grey);

    // The setting off: no bays, so the rider beside where one would be is offered nothing to draw.
    let lot = lot(1);
    let beside = at(100.0, 0.0, 10.0);
    for (on, want) in [(true, Some(Machine::Biplane)), (false, None)] {
        let offer = vehicle::offer(&vehicle::bays_shown(&lot, on), Kind::Bike, beside, 0.0, false);
        assert_eq!(offer, want);
        art.offers = vec![offer];
        let mut frame = Frame::new(640, 448, grey);
        let drawn = art.draw(&mut frame, (640, 448), &riders, 0, 100.0, &gauges());
        assert_eq!(drawn.offer.is_some(), on);
    }
}

/// The GPU draws the pop-up as the CPU does (V30's sprites): the panel's pieces, the pill, the
/// button and the text, in a full view and in each half of a split.
#[test]
fn the_gpu_draws_the_pop_up_as_the_cpu_does() {
    let Some(mut art) = art() else { return };
    let mut gpu = match game_render::gpu::Renderer::headless() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    gpu.set_images(&art.images());
    let sims = [Sim::new(0), Sim::new(0)];
    let riders: Vec<_> = sims.iter().map(|s| chase::bike(&s.rider.ride)).collect();
    let (w, h) = (1280usize, 720usize);
    let texture = gpu.offscreen(w as u32, h as u32);
    let view = texture.create_view(&Default::default());
    let target = game_render::gpu::Target { view: &view, size: (w as u32, h as u32), frame: [0, 0, w as u32, h as u32], scale: 1 };
    art.offers = vec![Some(Machine::Biplane), Some(Machine::Biplane)];
    let halves: Vec<_> = (0..2).map(|v| (v * w / 2, 0, w / 2, h)).collect();
    for (riders, views) in [(&riders[..1], vec![(0, 0, w, h)]), (&riders[..], halves)] {
        let mut frame = Frame::new(w, h, [0, 0, 0]);
        let mut sprites = Vec::new();
        for (v, &(x, y, vw, vh)) in views.iter().enumerate() {
            let mut part = Frame::new(vw, vh, [0, 0, 0]);
            assert!(art.draw(&mut part, (w, h), riders, v, 100.0, &gauges()).offer.is_some());
            for row in 0..vh {
                frame.color[3 * ((y + row) * w + x)..][..3 * vw].copy_from_slice(&part.color[3 * row * vw..][..3 * vw]);
            }
            art.sprites(&mut sprites, (x, y, vw, vh), (w, h), riders, v, 100.0, &gauges());
        }
        gpu.draw_sprites(&target, &sprites);
        let got = gpu.read(&texture);
        let sum: u64 = frame.color.iter().zip(&got).map(|(&x, &y)| x.abs_diff(y) as u64).sum();
        let mean = sum as f64 / got.len() as f64;
        let off = frame.color.chunks(3).zip(got.chunks(3)).filter(|(x, y)| x.iter().zip(y.iter()).any(|(&p, &q)| p.abs_diff(q) > 48)).count();
        let off = off as f64 / (w * h) as f64;
        eprintln!("{} view(s): {} sprites, {mean:.3} apart on average, {:.4}% of pixels off", views.len(), sprites.len(), off * 100.0);
        assert!(mean < 0.5 && off < 0.001, "{} view(s): {mean:.3} apart, {:.3}% off", views.len(), off * 100.0);
    }
}

/// The report of a pad holding Circle, for the shell's own read of it: the game's bitmask, as
/// `main.rs` tests it.
#[test]
fn circle_is_the_reports_circle() {
    let mut report = Report::idle();
    let low = !Control::Circle.mask();
    report.0[2] = (low >> 8) as u8;
    report.0[3] = low as u8;
    assert_ne!(report.buttons() & Control::Circle.mask(), 0);
    assert_eq!(Report::idle().buttons() & Control::Circle.mask(), 0);
    let _ = Plane::parked; // the type the bays draw
}
