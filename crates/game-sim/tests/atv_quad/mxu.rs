//! MD123 — **the quad on MX Unleashed's worlds**: [`quad::Ride`] stepped headless on free09 (the
//! split track the shell reads: its terrain and the props of both archives), its ground query
//! [`quad::world_ground`] and its sweep [`quad::OnWorld`].
//!
//! The original has no quad on an MX Unleashed track, so nothing here is compared with a capture: the
//! ride is checked for riding (it stands, gets going on the gas, turns on the stick, stays on the
//! ground it is over). The quad is MD37a's spawned one, read out of `atv-r-spawn.ee.bin` and moved to
//! free09: a quad made from its files alone waits on its model's load inputs (MD129).

use super::*;
use game_render::disc::Disc;
use game_sim::handling::seed;
use game_sim::ride::Controls;
use game_sim::track::{Scene, Terrain as TerrainFile};
use game_sim::world::{Ground, Place, Props, World as _};

/// The split free09's archives, read by suffix (tests/c16_fly.rs's).
fn track_read(disc: &mut Disc) -> impl Fn(&str) -> Result<Vec<u8>, String> + use<> {
    let parts = ["Tracks\\free09_ss.res", "Tracks\\free09_ss_2.res"].map(|n| disc.res(n).unwrap_or_else(|e| panic!("{n}: {e}")));
    move |want: &str| parts.iter().find_map(|r| r.read_suffix(want).ok()).ok_or_else(|| format!("no archive of this track has {want}"))
}

/// The split free09 as the shell reads it, handed to `then`.
fn in_free09<R>(disc: &mut Disc, then: impl FnOnce(&mut Place<'_>) -> R) -> R {
    let read = track_read(disc);
    let scene = Scene::parse(&read(".scn").unwrap()).expect("parse the scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let bytes = read(leaf.rsplit('\\').next().unwrap()).unwrap();
    let terrain = TerrainFile::parse(&bytes).expect("parse the terrain");
    let ground = Ground::new(&terrain, &scene.terrain);
    let (props, _) = Props::load(&scene, |w: &str| read(w));
    then(&mut Place { ground, props })
}

/// Where the quad is put down: on a rutted track of free09's (memory: free09 road shot spots), two
/// feet over the ground, facing `+z`.
const SPOT: (f32, f32) = (1872.0, 1900.0);

/// The phases of the ride, by frame: standing, then the gas, then the gas with the stick hard right.
const STAND: usize = 60;
const GAS: usize = 240;
const STEER: usize = 360;

/// The flattened heading of a velocity, radians.
fn heading(v: Vf) -> f32 {
    v.x().atan2(v.z())
}

#[test]
fn the_spawned_quad_rides_free09_headless() {
    let Some(pre) = image() else { return };
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    let dt = F::from_bits(0x3d08_850a);
    let (mut r, at) = image::read_ride(&pre, image::IMAGE_QUAD, dt).unwrap_or_else(|e| panic!("{e}"));
    let pairs = oasis::image_rays(&pre).pairs();
    let segments: [[Vf; 2]; 4] = std::array::from_fn(|k| pairs[k]);
    in_free09(&mut disc, |world| {
        let (floor, _) = world.ground(Vf::new(SPOT.0, 1.0e4, SPOT.1, 1.0));
        assert!(floor.y() > -1.0e5, "no ground at the spot");
        oasis::place(&mut r, [SPOT.0, floor.y() + 2.0, SPOT.1], [0.0, 0.0, 0.0, 1.0]);
        let mut body = seed::body(&|a| pre.u32_at(a), at.body);
        body.set_view(&r.s.body);
        (body.last_position, body.last_quat) = (body.position, body.quat);
        let mut ride = quad::Ride::new(r.s.p.clone(), r.s.q.clone(), body, r.bike.clone(), Vec::<Call>::new());

        let start = ride.body.position;
        let (mut top, mut down, mut lowest) = (0.0f32, 0usize, f32::MAX);
        let mut turned = (0.0f32, 0.0f32);
        let mut handed = std::collections::BTreeSet::new();
        for k in 0..STEER {
            let c = match k {
                k if k < STAND => Controls::default(),
                k if k < GAS => Controls { gas: F::ONE, ..Controls::default() },
                _ => Controls { gas: F::ONE, stick: (F::ONE, F::ZERO), ..Controls::default() },
            };
            ride.rest.clear();
            ride.step(&c, dt, world, segments);
            for c in &ride.rest {
                handed.insert(format!("{c:?}").split([' ', '(']).next().unwrap().to_string());
            }
            let p = ride.body.position;
            assert!(p.0.iter().all(|v| v.is_finite()), "frame {k}: the body is at {p:?}");
            let (g, _) = world.ground(p);
            lowest = lowest.min(p.y() - g.y());
            top = top.max(ride.q.speed_len.0);
            down += usize::from(ride.q.all_down);
            if k == GAS - 1 {
                turned.0 = heading(ride.body.velocity);
            }
            if k == STEER - 1 {
                turned.1 = heading(ride.body.velocity);
            }
            assert_eq!(ride.wreck(), None, "frame {k}: the quad wrecked on free09");
        }
        let d = ride.body.position.sub(start);
        let gone = (d.x() * d.x() + d.z() * d.z()).sqrt();
        let turn = (turned.1 - turned.0 + std::f32::consts::PI).rem_euclid(2.0 * std::f32::consts::PI) - std::f32::consts::PI;
        eprintln!(
            "MD123 free09: {gone} ft gone, top {top} ft/s, all four down {down} of {STEER} frames, lowest {lowest} ft over the ground, \
             turned {turn} rad on the stick; handed on {handed:?}"
        );
        assert!(top > 20.0, "the quad never got going on the gas: top {top} ft/s");
        assert!(gone > 50.0, "the quad went only {gone} ft");
        assert!(down > STEER / 2, "the quad was off the ground {} of {STEER} frames", STEER - down);
        assert!(lowest > -1.0, "the body sank {lowest} ft into the ground");
        assert!(turn.abs() > 0.3, "the stick did not turn the quad: {turn} rad");
        let known = ["GravityRecord", "PartMove", "SpatialMove", "PreloadCue", "StartStunt", "EndStunt"];
        assert!(handed.iter().all(|n| known.contains(&n.as_str())), "calls nobody answers on free09: {handed:?}");
    });
}
