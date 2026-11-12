//! P287 — MX vs. ATV's quad's Game Modifiers ([`Category::Atv`]) on the shell's quad: each knob off
//! 100% moves the loaded `.phy` and the ride the way it is named ([`vehicle::AtvRide::step`] on a flat
//! floor), and every knob at its default, or any other machine's moved, leaves the quad's load, and
//! so its step, bit for bit.
//!
//! The quad takes the knobs as the shell's [`vehicle::Garage::modify`] hands them: kept on the
//! [`vehicle::AtvRide`] and read as it is made ([`Mods::atv`], [`Mods::atv_body`]). Each test skips
//! without openmx-mods' `mxvatv_atv` machine pack in this checkout's `mods/machines/`.
//!
//! `src/vehicle.rs` is compiled in by path, the way `tests/machine_modifiers.rs` takes it.

#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use game_sim::markers::Spot;
use game_sim::modifiers::{Category, Knob, Mods};
use game_sim::ride::Controls;
use game_sim::world::Flat;
use ps2_float::{F, Vf};
use std::path::Path;

const DT: f32 = 1.0 / 30.0;
const PHY: &str = "Veh/atv_Race250P.phy";

/// The quad off the pack's Race 250 P handling file, or `None` without the pack.
fn quad() -> Option<vehicle::AtvRide> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/machines/mxvatv_atv");
    if !dir.join("mod.json").is_file() {
        println!("SKIPPED: no mods/machines/mxvatv_atv (openmx-mods' import-atv makes it)");
        return None;
    }
    let files = game_render::disc::Res::folder(&dir).expect("the pack's folder reads");
    Some(vehicle::AtvRide::read("mxvatv_atv", PHY, &mut |n| files.read(&n.replace('/', "\\")).ok()).expect("the quad builds"))
}

fn with(knob: Knob, pct: u16) -> Mods {
    let mut m = Mods::original();
    m.set(0, knob, pct);
    m
}

/// The quad's load as [`vehicle::AtvRide`] makes it, on `mods`.
fn load(atv: &vehicle::AtvRide, mods: &Mods) -> game_sim::quad::Load {
    let curve = game_sim::camera::CurveTable::build();
    let mut load = game_sim::quad::Load::read_model(&atv.ini, &atv.model, Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 1.0), &curve).unwrap();
    mods.atv(0, &mut load);
    load
}

/// `ticks` ticks of the quad on `mods`, taken at rest on a flat floor, `held` its controls each
/// tick: its body after each.
fn ride(atv: &vehicle::AtvRide, mods: &Mods, ticks: usize, held: &dyn Fn(usize) -> Controls) -> Vec<game_sim::atv_body::Body> {
    let mut atv = atv.clone();
    (atv.mods, atv.seat) = (*mods, 0);
    let mut floor = Flat::default();
    let spot = Spot::still(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0));
    atv.take(&spot, &mut floor).expect("the floor is under it");
    (0..ticks)
        .map(|t| {
            atv.step(&held(t), F(DT), &mut floor);
            atv.quad().unwrap().body
        })
        .collect()
}

fn gas(_: usize) -> Controls {
    Controls { gas: F::ONE, ..Controls::default() }
}

fn speed(b: &game_sim::atv_body::Body) -> f32 {
    b.velocity.x().hypot(b.velocity.z())
}

fn bits(bodies: &[game_sim::atv_body::Body]) -> Vec<u32> {
    bodies.iter().flat_map(|b| [b.position, b.velocity, b.quat]).flat_map(|v| [v.x(), v.y(), v.z(), v.w()]).map(f32::to_bits).collect()
}

#[test]
fn the_atv_knobs_are_their_own_category() {
    assert_eq!(Category::Atv.knobs(), [Knob::AtvSpeed, Knob::AtvAccel, Knob::AtvGrip, Knob::AtvSuspension]);
    assert!(!Category::ALL.contains(&Category::Atv), "an addition's, not the disc's");
    for k in Category::Atv.knobs() {
        assert!(k.per_seat());
        assert_eq!(Knob::from_word(k.word()), Some(k));
        assert_eq!(Mods::parse(&with(k, 150).text()).unwrap(), with(k, 150));
    }
}

#[test]
fn every_knob_at_its_default_leaves_the_quad_bit_exact() {
    let Some(atv) = quad() else { return };
    let mut all = Mods::original();
    for k in Knob::ALL {
        all.set(0, k, 100);
    }
    assert!(all.is_original());
    // Every other machine's and the world's moved: none of them reaches the quad.
    let mut others = Mods::original();
    for k in Knob::ALL.into_iter().filter(|k| k.category() != Category::Atv) {
        others.set(0, k, k.step(k.most(), false));
    }
    let stock = format!("{:?}", load(&atv, &Mods::original()));
    for mods in [all, others] {
        assert_eq!(format!("{:?}", load(&atv, &mods)), stock, "{}", mods.text());
    }
    let turn = |t: usize| Controls { gas: F::ONE, stick: (if t < 60 { F::ZERO } else { F(-1.0) }, F::ZERO), ..Controls::default() };
    let plain = bits(&ride(&atv, &Mods::original(), 120, &turn));
    assert_eq!(bits(&ride(&atv, &all, 120, &turn)), plain);
    assert_eq!(bits(&ride(&atv, &others, 120, &turn)), plain);
}

#[test]
fn an_atv_knob_moves_its_keys() {
    let Some(atv) = quad() else { return };
    let stock = load(&atv, &Mods::original());
    let at = |k: Knob, pct: u16| load(&atv, &with(k, pct));
    let (lr, ff) = (game_sim::quad::LEFT_REAR, game_sim::quad::LEFT_FRONT);

    let fast = at(Knob::AtvSpeed, 200);
    assert_eq!(fast.engine.final_drive, stock.engine.final_drive / F(2.0));
    assert_eq!(fast.engine.curve_torque[0], stock.engine.curve_torque[0] * F(2.0));
    assert_eq!(fast.mv.drag, stock.mv.drag / F(4.0));
    assert_eq!(fast.mv.reverse_speed, stock.mv.reverse_speed * F(2.0));

    let quick = at(Knob::AtvAccel, 200);
    assert_eq!(quick.engine.final_drive, stock.engine.final_drive);
    assert_eq!(quick.mv.drag, stock.mv.drag * F(2.0));
    assert_eq!(quick.tires.wheels[lr].surfaces[0].drive[2], stock.tires.wheels[lr].surfaces[0].drive[2] * F(2.0));
    assert_eq!(quick.tires.wheels[ff].surfaces[0].drive[2], stock.tires.wheels[ff].surfaces[0].drive[2], "only the driven pair's grip");

    let slick = at(Knob::AtvGrip, 50);
    assert_eq!(slick.tires.wheels[ff].surfaces[0].drive, stock.tires.wheels[ff].surfaces[0].drive.map(|f| f * F(0.5)));

    let stiff = at(Knob::AtvSuspension, 200);
    assert_eq!(stiff.tires.wheels[lr].spring, stock.tires.wheels[lr].spring * F(2.0));
    assert_eq!(stiff.tires.front_spring.0, stock.tires.front_spring.0 * F(2.0));
    assert_eq!(stiff.mv.rear_spring, stock.mv.rear_spring * F(2.0));
    assert_eq!(stiff.tires.wheels[lr].damping_in, stock.tires.wheels[lr].damping_in * F(2.0).sqrt());
}

#[test]
fn an_atv_knob_changes_the_quads_ride() {
    let Some(atv) = quad() else { return };
    let run = |k: Knob, pct: u16, ticks: usize, held: &dyn Fn(usize) -> Controls| ride(&atv, &with(k, pct), ticks, held);

    // Top speed: the fastest it goes in twenty seconds of gas.
    let top = |pct: u16| run(Knob::AtvSpeed, pct, 600, &gas).iter().map(speed).fold(0.0, f32::max);
    let (stock, fast, slow) = (top(100), top(150), top(50));
    println!("top speed: {slow} / {stock} / {fast} ft/s at 50/100/150%");
    assert!(fast > stock * 1.2 && slow < stock * 0.8, "top speed {slow} {stock} {fast}");

    // Acceleration: the speed two seconds off the line, the rider leaning forward (stick down the
    // pad's y) as a launch on more torque needs: at 150% with the stick centred the Race 250 P loops
    // over backwards in a second.
    let lean = |_: usize| Controls { gas: F::ONE, stick: (F::ZERO, F(-1.0)), ..Controls::default() };
    let at = |pct: u16| speed(&run(Knob::AtvAccel, pct, 60, &lean)[59]);
    let (stock, quick, slow) = (at(100), at(150), at(50));
    println!("2 s: {slow} / {stock} / {quick} ft/s at 50/100/150% acceleration");
    assert!(quick > stock * 1.1 && slow < stock * 0.9, "acceleration {slow} {stock} {quick}");

    // Grip: a hard turn on the gas, the sideways pull the tires hold (the speed times the rate the
    // path turns, over a second of it). Slick, the driven pair spins off the line too, so it turns
    // in at half the speed: the path's angle alone would read a tighter turn.
    let turn = |t: usize| Controls { gas: F::ONE, stick: (if t < 90 { F::ZERO } else { F(-1.0) }, F::ZERO), ..Controls::default() };
    let pull = |pct: u16| {
        let b = run(Knob::AtvGrip, pct, 132, &turn);
        let way = |b: &game_sim::atv_body::Body| b.velocity.x().atan2(b.velocity.z());
        let turned = |t: usize| {
            let d = way(&b[t + 1]) - way(&b[t]);
            (d + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
        };
        (101..131).map(|t| speed(&b[t]) * turned(t).abs() / DT).sum::<f32>() / 30.0
    };
    let (stock, slick) = (pull(100), pull(40));
    println!("sideways pull in the turn: {stock} -> {slick} ft/s^2 at 40% grip");
    assert!(slick < stock * 0.8, "grip {stock} -> {slick}");

    // Suspension: stiffer springs sag less, the quad standing higher once settled.
    let stand = |pct: u16| run(Knob::AtvSuspension, pct, 120, &|_| Controls::default())[119].position.y();
    let (stock, stiff, soft) = (stand(100), stand(200), stand(50));
    println!("rest height: {soft} / {stock} / {stiff} at 50/100/200%");
    assert!(stiff > stock && stock > soft, "suspension {soft} {stock} {stiff}");
}
