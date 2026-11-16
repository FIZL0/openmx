//! MD144 — **the quad's steps routed to Untamed's** ([`quad::Ride::untamed`]).
//!
//! Untamed's preload, backflip and 360 are proved against SLUS-21701 on the bike (MD93, MD93a, MD93b,
//! `untamed_reworked.rs`); the quad's ground step calls those same functions. What is new here is the
//! quad's side: [`quad::Steps`] handing them the quad's words and taking their answers back. That is
//! checked against the SLUS-21104 path the C28 replays prove, on generated states where the two
//! originals do the same thing (one trick button at most, and that one R1; a stick off centre; a stunt id
//! outside 41..=48, the ids whose bounds Untamed moved): every word of the quad, its body, its bike
//! steps' words and the calls handed on must be equal, routed or not.
//!
//! The stunt ids Untamed's quad starts are read off MD147's spawn image (`context/untamed/md147/
//! spawn.ee.bin`, the quad at `0x01be4760`). Without the files the tests skip, and say so.

use super::Gen;
use game_sim::phy::Ini;
use game_sim::quad::{self, Call, Moto, Steps};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};

/// MD147's spawn image's player quad.
const QUAD: u32 = 0x01be_4760;

/// Unleashed's race quad's `.phy` and chassis (MD123's files), which the generated states start from.
fn quad_files() -> Option<(Ini, quad::Model)> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let dir = kit.ctx("mxvatv-ps2/md37a");
    let (phy, psg) = (dir.join("phy/atv_race250p.phy"), dir.join("250r_atv_chasis.psg"));
    let (Ok(phy), Ok(psg)) = (std::fs::read(&phy), std::fs::read(&psg)) else {
        skip(&format!("{} or {} is not there (MXVSATV.PSR's vehicletype_1.res; MD129)", phy.display(), psg.display()));
        return None;
    };
    Some((Ini::parse_bytes(&phy), quad::Model::parse(&psg).unwrap_or_else(|e| panic!("250r_atv_chasis.psg: {e}"))))
}

fn spawned(files: &(Ini, quad::Model)) -> quad::Ride {
    let (at, heading) = (Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 1.0));
    let curve = game_sim::camera::CurveTable::build();
    let load = quad::Load::read_model(&files.0, &files.1, at, heading, &curve).unwrap_or_else(|e| panic!("{e}"));
    quad::Ride::spawn(&load, &files.1, at, heading, None, Vec::<Call>::new())
}

/// A stick off centre, often past the 0.7 and 0.8 the lips read.
fn stick(g: &mut Gen) -> F {
    let x = g.pick(&[-1.0, -0.95, -0.81, -0.71, -0.5, -0.1, 0.1, 0.5, 0.71, 0.81, 0.95, 1.0]);
    if g.b() { x } else { F(x.0 * (0.5 + g.unit())) }
}

/// The ride's quad, body and steps' words, with the state the 360's and the backflip's branches read
/// generated over them.
fn state(g: &mut Gen, r: &quad::Ride) -> (quad::Quad, quad::Body, quad::Bike) {
    let (mut q, mut body, mut b) = (r.q.clone(), r.body.view(), r.bike.clone());
    (b.stick.x, b.stick.y) = (stick(g), stick(g));
    b.stick_hold = g.f(0.0, 1.0);
    b.buttons = [0, 0, 4, 4, 1, 2, 3][g.bits() as usize % 7];
    b.stunt_id = [0, 20, 40, 49, 50, 60, 255][g.bits() as usize % 7];
    (b.touching, b.spin_active, b.spin_applied) = (g.bits() % 5 == 0, g.b(), g.b());
    let s = &mut b.parts.spin;
    (s.armed, s.dir, s.clip) = (g.b(), if g.b() { 1 } else { -1 }, (g.bits() % 3) as i32);
    (s.grace, s.heading, s.flick) = (g.f(-0.05, 0.9), g.f(0.0, 6.2831855), g.f(-1.0, 1.0));
    let yaw = if g.b() { g.f(-1.5, 1.5) * s.max_yaw } else { s.min_yaw * g.f(0.9, 1.1) };
    s.moment = Vf::new(g.f(-1.0, 1.0).0 * s.max_pitch.0, yaw.0, 0.0, 0.0);
    let a = &mut b.air;
    (a.flip_active, a.flip_armed, a.flip_torque_applied) = (g.b(), g.b(), g.bits() % 4 == 0);
    a.flip_grace = g.f(-0.05, 0.6);
    a.flip_torque = Vf::new(-g.f(0.0, 2.0).0 * b.air_keys.min_backflip_torque.0, 0.0, 0.0, 0.0);
    b.flip_clip = game_sim::handling::FlipClip((g.bits() % 3) as i32);
    (b.right_y, b.up_y) = (g.f(-1.0, 1.0), g.f(-1.0, 1.0));
    (q.on_ground, q.spinning, q.flipping) = (g.b(), g.b(), g.bits() % 3 == 0);
    q.brake = if g.bits() % 4 == 0 { F(0.5) } else { F::ZERO };
    q.contact_vel = Vf::new(0.0, 0.0, g.f(0.0, 60.0).0, 0.0);
    q.authority = g.f(-0.5, 1.0);
    q.attitude = Vf::new(g.f(-20.0, 20.0).0, 0.0, g.f(-20.0, 20.0).0, 0.0);
    body.velocity = Vf::new(g.f(-30.0, 30.0).0, g.f(-10.0, 20.0).0, g.f(-30.0, 60.0).0, 0.0);
    body.moment = Vf::new(g.f(-50.0, 50.0).0, g.f(-50.0, 50.0).0, g.f(-50.0, 50.0).0, 0.0);
    (q, body, b)
}

/// One call through [`Steps`], on SLUS-21104's step or Untamed's: everything it left, as text (a
/// float's debug print tells `-0` from `+0`), the switch itself set aside.
fn run(c: Call, untamed: bool, q: &quad::Quad, body: &quad::Body, b: &quad::Bike) -> String {
    let (mut q, mut body, mut bike) = (q.clone(), *body, b.clone());
    bike.parts.scene.untamed = untamed;
    let mut m = Steps { bike, rest: Vec::<Call>::new(), ground: |_: Vf, _: &mut u8| (Vf::ZERO, Vf::ZERO), sweep: () };
    m.call(c, &mut q, &mut body);
    m.bike.parts.scene.untamed = false;
    format!("{q:?}\n{body:?}\n{:?}\n{:?}", m.bike, m.rest)
}

/// Untamed's 360 and backflip, routed, are SLUS-21104's on every state where the two originals agree;
/// 20,000 each, with how often each branch's tell-tale moved counted so a dead generator shows.
#[test]
fn the_quads_untamed_spin_and_backflip_are_slus21104s_where_both_agree() {
    let Some(files) = quad_files() else { return };
    let r = spawned(&files);
    let dt = F::from_bits(0x3c0e_a0ea);
    let mut g = Gen(0x0144_5eed);
    let (mut calls, mut fired, mut moved) = (0, 0, 0);
    for k in 0..40_000 {
        let (q, body, b) = state(&mut g, &r);
        let c = if k % 2 == 0 { Call::SpinAir(dt) } else { Call::Backflip(dt) };
        let (slus, untamed) = (run(c, false, &q, &body, &b), run(c, true, &q, &body, &b));
        if slus != untamed {
            let (a, b): (Vec<&str>, Vec<&str>) = (slus.split(", ").collect(), untamed.split(", ").collect());
            let at = a.iter().zip(&b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
            let near = |v: &[&str]| v[at.saturating_sub(4)..(at + 4).min(v.len())].join(", ");
            panic!("case {k} ({c:?}) differs:\nSLUS-21104 … {} …\nUntamed    … {} …", near(&a), near(&b));
        }
        let mut m = Steps { bike: b.clone(), rest: Vec::<Call>::new(), ground: |_: Vf, _: &mut u8| (Vf::ZERO, Vf::ZERO), sweep: () };
        m.bike.parts.scene.untamed = true;
        let (mut q2, mut body2) = (q.clone(), body);
        m.call(c, &mut q2, &mut body2);
        calls += usize::from(!m.rest.is_empty());
        fired += usize::from(m.bike.spin_active != b.spin_active || m.bike.air.flip_active != b.air.flip_active);
        moved += usize::from(body2.moment != body.moment);
    }
    eprintln!("MD144: 40000 states, {calls} with a call handed on, {fired} turned a 360 or flip on or off, {moved} moved the body");
    assert!(calls > 500 && fired > 2000 && moved > 2000, "the generator reaches too few branches: {calls} {fired} {moved}");
}

/// Untamed's air control compares the stunt id with 49 and 54 where SLUS-21104 has 41 and 46; the
/// routed quad hands it the id 8 lower, so an Untamed id meets SLUS-21104's bounds as it meets
/// Untamed's. Over ids round both pairs of bounds, the routed call on id `n` equals the SLUS-21104
/// call on `n − 8`.
#[test]
fn the_quads_untamed_air_control_reads_the_stunt_id_8_lower() {
    let Some(files) = quad_files() else { return };
    let r = spawned(&files);
    let dt = F::from_bits(0x3c0e_a0ea);
    let mut g = Gen(0x0144_a1c0);
    for k in 0..4000 {
        let (mut q, body, mut b) = state(&mut g, &r);
        q.on_ground = false;
        b.rider_stunt = 36 + (k % 30) as i32;
        let untamed = run(Call::AirborneControl(dt), true, &q, &body, &b);
        b.rider_stunt -= 8;
        let slus = run(Call::AirborneControl(dt), false, &q, &body, &b);
        b.rider_stunt += 8;
        let (left, right) = (untamed.rsplit_once("rider_stunt").unwrap().0, slus.rsplit_once("rider_stunt").unwrap().0);
        assert!(left == right, "case {k} (id {}) parts:\nSLUS-21104 {slus}\nUntamed    {untamed}", b.rider_stunt);
    }
}

/// The stunt ids [`quad::Ride::untamed`] gives the quad are the spawned Untamed quad's: the backflip
/// 49 (`+0x13c8`) and the 360s 52 and 53 (`+0x13cc`, `+0x13d0`; SLUS-21104's 41, 44, 45 at `+0x13b8`,
/// `+0x13bc`, `+0x13c0`), and the image's quad is the race quad (`atv_Race250P.phy` at `+0x134`).
#[test]
fn the_untamed_quads_stunt_ids_are_its_spawn_images() {
    let Some(files) = quad_files() else { return };
    let Ok(kit) = Kit::load().map_err(|w| skip(&w)) else { return };
    let path = kit.ctx("untamed/md147").join(format!("spawn{}", ".ee.bin"));
    if !path.exists() {
        skip(&format!("{} is not there: eeMemory.bin of MD147's spawn.p2s (research/md147)", path.display()));
        return;
    }
    let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    let name: Vec<u8> = (0..20).map(|i| ee.u8_at(QUAD + 0x134 + i)).take_while(|&c| c != 0).collect();
    assert_eq!(String::from_utf8_lossy(&name), "atv_Race250P.phy");
    let mut r = spawned(&files);
    let slus = (r.bike.flip_stunt, r.bike.parts.spin.stunt_right, r.bike.parts.spin.stunt_left);
    assert_eq!(slus, (41, 44, 45), "SLUS-21104's ids");
    r.untamed();
    let ours = (r.bike.flip_stunt, r.bike.parts.spin.stunt_right, r.bike.parts.spin.stunt_left);
    assert_eq!(ours, (ee.u32_at(QUAD + 0x13c8), ee.u32_at(QUAD + 0x13cc), ee.u32_at(QUAD + 0x13d0)));
    assert!(r.bike.untamed() && r.bike.parts.scene.untamed && r.tailed.as_ref().is_some_and(|t| t.tail.untamed) == r.tailed.is_some());
}

/// And the routing is live: with R1 held beside another trick button Untamed's backflip and 360 arm
/// where SLUS-21104's (which wants R1 alone) do not, so some of those states must come out different.
#[test]
fn the_quads_untamed_step_arms_on_r1_with_other_buttons() {
    let Some(files) = quad_files() else { return };
    let r = spawned(&files);
    let dt = F::from_bits(0x3c0e_a0ea);
    let mut g = Gen(0x0144_0b17);
    let mut differ = [0; 2];
    for k in 0..20_000 {
        let (q, body, mut b) = state(&mut g, &r);
        b.buttons = [5, 6, 0xc][k % 3];
        let c = if k % 2 == 0 { Call::SpinAir(dt) } else { Call::Backflip(dt) };
        differ[k % 2] += usize::from(run(c, false, &q, &body, &b) != run(c, true, &q, &body, &b));
    }
    assert!(differ[0] > 5 && differ[1] > 5, "R1 with another button changed too few states: 360 {}, flip {}", differ[0], differ[1]);
}
