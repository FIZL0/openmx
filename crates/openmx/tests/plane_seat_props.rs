//! S259: a seat flying the biplane among seats that share one prop set (S197c,
//! [`game_sim::prop_seats`]) strikes the shared set. Before, [`vehicle::fly_lane_met`] stepped the
//! seat's own copy of the props, which the shared set had emptied, so a plane seat never knocked a
//! prop online or in split screen.
//!
//! S241's capture: hit0-11042's plane rolling at free06's ball 0 (s2's and s3's context, as
//! game-sim's `tests/prop_plane.rs` reads it). Seat 1 flies it through [`vehicle::fly_lane_shared`],
//! seat 0's bike stands far off and steps through [`Sim::step_shared`], and the set steps once after
//! them ([`game_sim::prop_seats::Shared::finish`], the plane seat 1's striker). Against one seat
//! flying alone with its own props ([`vehicle::fly_lane_met`], S241's lane): the ball wakes on the
//! same tick and goes as it does there, bit for bit, the plane too, and both seats hold that ball.
//! Skipped with a line where the disc, MX.PSR or the state is not there.
//!
//! `src/vehicle.rs` is compiled in by path, the way `tests/machines.rs` takes it.

#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use game_render::Disc;
use game_sim::Sim;
use game_sim::archive::{Psr, Res};
use game_sim::biplane::Body;
use game_sim::ride::Controls;
use game_sim::flight::{self, Flight};
use game_sim::flight_search::{self, Rays};
use game_sim::phy::{self, Ini, Plane};
use game_sim::prop_set::PropSet;
use game_sim::track::{self, Scene, Terrain};
use game_sim::world::{Ground, World};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

/// The biplane in biplane.p2m2's states: `+0x160` its body.
const PLANE: u32 = 0x00c4_4a40;
/// free06's EyeBall 0's physics object (`+0x110` body, `+0x13c` node).
const BALL: u32 = 0x00d3_f510;

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|k| ee.u32_at(a + k)))
}

fn body_of(ee: &Ee, b: u32) -> Body {
    let q = |a: u32| quad(ee, b + a);
    Body {
        enabled: ee.u32_at(b) != 0,
        inv_mass: F(ee.f32_at(b + 0x10)),
        damping: q(0x20),
        last_position: q(0x50),
        position: q(0x60),
        velocity: q(0x70),
        last_force: q(0x80),
        force: q(0x90),
        accel: q(0xa0),
        shock_peak: q(0xb0),
        shock: q(0xc0),
        step: q(0xf0),
        inv_inertia: q(0x120),
        ang_damping: q(0x130),
        last_quat: q(0x150),
        quat: q(0x170),
        cofg: q(0x180),
        moment: q(0x190),
        last_moment: q(0x1a0),
        rates: q(0x1b0),
        ang_accel: q(0x1c0),
        twist_peak: q(0x1d0),
        twist: q(0x1e0),
    }
}

fn image(kit: &Kit) -> Option<Ee> {
    let rel = "states/s241-hit0-11042.eeMemory.bin";
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context").join(rel);
    let path = if here.exists() { here } else { kit.ctx(rel) };
    let mut ee = Ee::from_ram(&path).map_err(|why| skip(&format!("{why} — research/s241/slots.py ram hit0-11042 writes it"))).ok()?;
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// The plane off `machine.res`, with its rays and hull.
fn the_plane(kit: &Kit) -> Option<Flight> {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/iso/MX.PSR");
    let path = if here.exists() { here } else { kit.ctx("iso").join("MX.PSR") };
    let Ok(mut f) = File::open(&path) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", path.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("machine.res").expect("machine.res is in MX.PSR");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("machine.res");
    let member = |name: &str| {
        let e = res.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).unwrap_or_else(|| panic!("machine.res has {name}"));
        res.read(&bytes, e).expect("its bytes")
    };
    let plane = Plane::read_bytes(&member(phy::BIPLANE_PHY));
    let rays = track::Rays::parse(&member(flight_search::BIPLANE_RAYS)).expect("biplanem.col");
    let hull = track::Mesh::parse(&member(flight::HULL)).expect("biplanef.col");
    Some(Flight::new(&plane).with_rays(Rays::of(&rays)).with_hull(hull))
}

/// free06's knockable props and its terrain, off the disc.
fn free06() -> Option<(PropSet, Ground<'static>)> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let member = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("Tracks\\free06.res")).expect("free06.res").name.clone();
    let res = disc.res(&member).expect("free06's archive");
    let res = &res;
    let toy = Ini::parse_bytes(&res.read_suffix("free06.toy").expect("free06.toy"));
    let scene = Scene::parse(&res.read_suffix("free06.scn").expect("free06.scn")).expect("free06.scn parses");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let bytes: &'static [u8] = Box::leak(res.read_suffix(&leaf).expect("free06's terrain").into_boxed_slice());
    let terrain: &'static Terrain = Box::leak(Box::new(Terrain::parse(bytes).expect("free06's terrain parses")));
    let ground = Ground::new(terrain, &scene.terrain);
    let area = ground.area();
    let (set, said) =
        PropSet::load(&toy, "Tracks\\Free06\\", &scene.models, &mut |p| res.read(p).ok(), &mut |p| res.read_suffix(p).ok(), area);
    assert!(said.is_empty(), "free06 loads whole: {said:?}");
    Some((set, ground))
}

/// Ball 0 in the port's set: the prop nearest its node in RAM, its body and node the state's.
fn seed(ee: &Ee, set: &mut PropSet) -> (usize, usize) {
    let node = ee.u32_at(BALL + 0x13c);
    let origin = quad(ee, node + 0xb0);
    let mut best = None;
    for (c, coll) in set.collections.iter().enumerate() {
        for (k, ob) in coll.objects.iter().enumerate() {
            let d = (ob.node.origin - origin).0[..3].iter().map(|v| v * v).sum::<f32>();
            if ob.collision.is_some() && best.is_none_or(|(_, _, b)| d < b) {
                best = Some((c, k, d));
            }
        }
    }
    let (c, k, d) = best.expect("a prop with a collision object");
    assert!(d < 2500.0, "ball 0 is {d:?} from the nearest prop");
    let coll = &mut set.collections[c];
    coll.bones[k].body = body_of(ee, ee.u32_at(BALL + 0x110));
    let ob = &mut coll.objects[k];
    ob.node.rows = [0, 1, 2].map(|r| quad(ee, node + 0x80 + 16 * r));
    ob.node.origin = origin;
    (c, k)
}

fn ball(sim: &Sim, (c, o): (usize, usize)) -> Body {
    sim.traffic.props.collections[c].bones[o].body
}

#[test]
fn a_plane_seat_knocks_the_shared_ball() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let Some(mut plane) = the_plane(&kit) else { return };
    let Some((mut set, mut ground)) = free06() else { return };
    let Some(ee) = image(&kit) else { return };
    let at = seed(&ee, &mut set);
    plane.body = body_of(&ee, ee.u32_at(PLANE + 0x160));
    let dt = F(ee.f32_at(addr("frame_dt").unwrap()));
    let flying = || {
        let mut s = Sim::new(0x5259);
        s.plane = Some(plane.clone());
        s.traffic.props = set.clone();
        s
    };
    let wanted = vehicle::Wanted::default();
    let ground: &mut dyn World = &mut ground;

    // One seat alone, its own props (S241's lane).
    let mut alone = flying();
    let mut want = Vec::new();
    for _ in 0..12 {
        vehicle::fly_lane_met(&mut alone, dt, wanted, ground, &[], None);
        want.push((ball(&alone, at), alone.plane.as_ref().unwrap().body));
    }
    let woke = want.iter().position(|(b, _)| b.enabled).map(|k| k + 1);
    assert_eq!(woke, Some(3), "alone, ball 0 wakes on the original's tick");

    // Seat 1 flies it among the seats, seat 0's bike far off.
    let mut bike = Sim::new(0x5259);
    bike.traffic.props = set.clone();
    let spot = set.collections[at.0].objects[at.1].node.origin;
    bike.rider.place(Vf::new(spot.x(), spot.y(), spot.z() + 2000.0, 1.0), Vf::new(1.0, 0.0, 0.0, 0.0));
    let mut sims = [bike, flying()];
    for (tick, (want_ball, want_plane)) in want.iter().enumerate() {
        let mut props = game_sim::prop_seats::Shared::take(&mut sims.iter_mut().collect::<Vec<_>>()).expect("two seats, props");
        let [bike, seat] = &mut sims;
        vehicle::fly_lane_shared(seat, dt, wanted, ground, &[], None, Some((&mut props, 1)));
        bike.step_shared(dt, &Controls::default(), ground, None, &[], &mut props, 0);
        props.finish(&mut sims.iter_mut().collect::<Vec<_>>(), dt, ground);
        for (k, s) in sims.iter().enumerate() {
            let got = ball(s, at);
            assert_eq!(got.enabled, want_ball.enabled, "tick {}: seat {}'s ball awake as alone", tick + 1, k + 1);
            assert_eq!(
                (got.position.to_bits(), got.velocity.to_bits(), got.rates.to_bits()),
                (want_ball.position.to_bits(), want_ball.velocity.to_bits(), want_ball.rates.to_bits()),
                "tick {}: seat {}'s ball as alone\ngot {got:?}\nwant {want_ball:?}",
                tick + 1,
                k + 1
            );
        }
        let got = sims[1].plane.as_ref().unwrap().body;
        assert_eq!(
            (got.position.to_bits(), got.velocity.to_bits(), got.rates.to_bits()),
            (want_plane.position.to_bits(), want_plane.velocity.to_bits(), want_plane.rates.to_bits()),
            "tick {}: the plane as alone",
            tick + 1
        );
    }
    assert!(ball(&sims[0], at).enabled, "the ball the plane knocked is awake in seat 0's world, which the shell draws");
}

/// S259a: two seats flying together ([`flight::frames_together_props`], as online's and split
/// screen's held planes fly through [`vehicle::fly_together`]) strike the seats' one set. Seat 1 flies
/// S241's plane at ball 0, seat 0 the same plane 2000 ft off; the walk readies seat 1 then seat 0
/// ([`vehicle::ready_lane`]), flies both in one frame, settles them, and the set steps with both
/// planes as its strikers. Against seat 1's plane flying alone: the ball and the plane as there, bit
/// for bit, in both seats.
#[test]
fn planes_flown_together_knock_the_shared_ball() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let Some(mut plane) = the_plane(&kit) else { return };
    let Some((mut set, mut ground)) = free06() else { return };
    let Some(ee) = image(&kit) else { return };
    let at = seed(&ee, &mut set);
    plane.body = body_of(&ee, ee.u32_at(PLANE + 0x160));
    let dt = F(ee.f32_at(addr("frame_dt").unwrap()));
    let flying = |plane: &Flight| {
        let mut s = Sim::new(0x5259);
        s.plane = Some(plane.clone());
        s.traffic.props = set.clone();
        s
    };
    let wanted = vehicle::Wanted::default();
    let ground: &mut dyn World = &mut ground;

    let mut alone = flying(&plane);
    let mut want = Vec::new();
    for _ in 0..12 {
        vehicle::fly_lane_met(&mut alone, dt, wanted, ground, &[], None);
        want.push((ball(&alone, at), alone.plane.as_ref().unwrap().body));
    }
    assert_eq!(want.iter().position(|(b, _)| b.enabled).map(|k| k + 1), Some(3), "alone, ball 0 wakes on the original's tick");

    let mut far = plane.clone();
    let by = Vf::new(0.0, 0.0, 2000.0, 0.0);
    (far.body.position, far.body.last_position) = (far.body.position.add(by), far.body.last_position.add(by));
    far.collider.cofg_world = far.collider.cofg_world.add(by);
    let mut sims = [flying(&far), flying(&plane)];
    for (tick, (want_ball, want_plane)) in want.iter().enumerate() {
        let mut props = game_sim::prop_seats::Shared::take(&mut sims.iter_mut().collect::<Vec<_>>()).expect("two seats, props");
        let mut held = Vec::new();
        for seat in [1, 0] {
            let (plane, ready) = vehicle::ready_lane(&mut sims[seat], dt, wanted, ground, Some(&mut props)).expect("a flying seat");
            held.push((seat, plane, ready));
        }
        {
            let tables = sims[1].tables.clone();
            let seats: Vec<usize> = held.iter().map(|(s, ..)| *s).collect();
            let mut fleet: Vec<flight::Fly<'_>> = held
                .iter_mut()
                .map(|(_, p, r)| flight::Fly { flight: p.flight.as_mut().unwrap(), pad: r.pad, met: None, bikes: &[] })
                .collect();
            vehicle::fly_together(dt, &mut fleet, ground, &tables.ride, Some((props.set_mut(), &seats)));
        }
        for (seat, plane, ready) in held {
            vehicle::settle_lane(&mut sims[seat], plane, ready);
        }
        props.finish(&mut sims.iter_mut().collect::<Vec<_>>(), dt, ground);
        for (k, s) in sims.iter().enumerate() {
            let got = ball(s, at);
            assert_eq!(got.enabled, want_ball.enabled, "tick {}: seat {}'s ball awake as alone", tick + 1, k + 1);
            assert_eq!(
                (got.position.to_bits(), got.velocity.to_bits(), got.rates.to_bits()),
                (want_ball.position.to_bits(), want_ball.velocity.to_bits(), want_ball.rates.to_bits()),
                "tick {}: seat {}'s ball as alone\ngot {got:?}\nwant {want_ball:?}",
                tick + 1,
                k + 1
            );
        }
        let got = sims[1].plane.as_ref().unwrap().body;
        assert_eq!(
            (got.position.to_bits(), got.velocity.to_bits(), got.rates.to_bits()),
            (want_plane.position.to_bits(), want_plane.velocity.to_bits(), want_plane.rates.to_bits()),
            "tick {}: the plane as alone",
            tick + 1
        );
    }
    assert!(ball(&sims[0], at).enabled, "the ball the planes knocked is awake in seat 0's world");
}
