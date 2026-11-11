//! MD123 — **the quad on MX Unleashed's worlds**: [`quad::Ride`] stepped headless on free09 (the
//! split track the shell reads: its terrain and the props of both archives), its ground query
//! [`quad::world_ground`] and its sweep [`quad::OnWorld`].
//!
//! The original has no quad on an MX Unleashed track, so nothing here is compared with a capture: the
//! ride is checked for riding (it stands, gets going on the gas, turns on the stick, stays on the
//! ground it is over). The quad is MD37a's spawned one, read out of `atv-r-spawn.ee.bin` and moved to
//! free09: a quad made from its files alone waits on its model's load inputs (MD129).
//!
//! MD123a — the same quad with its collision record, frame tail and wreck on ([`quad::Recorded`],
//! [`quad::Tailed`], [`quad::Wrecked`]): off a lip in free09's terrain and down again, then into a
//! prop hard enough that the record's strike wrecks it (reason 15), put back when the wreck clock runs
//! out, and riding on. The record's rays are the image's riding and wreck parts'.

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

/// MD123a's seeding beyond MD123's: the image's collision record and its resolution's fields, the
/// riding and the wreck parts' rays (their count at `+0x178`, the segments at `*(+0x180)`), the
/// update's words a ride without a player or a race reads (the vehicle state's mode, the stunt speed,
/// the surface table) and the wreck's `MaxWreckTime` and struck class, every prop of the class.
fn md123a_rigs(pre: &Ee, at: &image::Image, ride: &mut quad::Ride) {
    let rays = |part: u32| -> Vec<[Vf; 2]> {
        let (n, segs) = (pre.u32_at(part + 0x178), pre.u32_at(part + 0x180));
        let q = |a: u32| Vf::new(pre.f32_at(a), pre.f32_at(a + 4), pre.f32_at(a + 8), pre.f32_at(a + 12));
        (0..n).map(|k| [q(segs + 0x20 * k), q(segs + 0x20 * k + 0x10)]).collect()
    };
    let (riding, wreck) = (at.parts[0], at.parts[4]);
    let record = game_sim::handling::record::read(&|a| pre.u32_at(a), at.record);
    let held = game_sim::handling::record::held(&|a| pre.u32_at(a), at.record).expect("the record's resolution fields");
    let handles = quad::Rays { active: riding, active_too: riding, record: riding, wreck, riding };
    ride.recorded = Some(quad::Recorded { record, held, riding: rays(riding), wreck: rays(wreck), rays: handles });
    // The wreck body built on WreckATVm.col when the file is there: its rays are the image's wreck part's.
    let col = Kit::load().ok().map(|kit| kit.ctx("mxvatv-ps2/md37a").join(quad::WRECK_FILE));
    if let Some(bytes) = col.and_then(|p| std::fs::read(p).ok()) {
        let file = game_sim::track::Rays::parse(&bytes).expect("WreckATVm.col");
        let from_file = game_sim::flight_search::Rays::of(&file).pairs();
        let bits = |r: &[[Vf; 2]]| r.iter().flat_map(|s| s.iter().flat_map(|v| [v.x(), v.y(), v.z(), v.w()].map(f32::to_bits))).collect::<Vec<_>>();
        assert_eq!(bits(&from_file), bits(&rays(wreck)), "WreckATVm.col's rays are the image's wreck part's");
        let mut w = quad::WreckBody { riding: quad::Col::made(riding), ..quad::WreckBody::default() };
        ride.build_wreck(&mut w, &file, at.parts[3], wreck, image::IMAGE_QUAD, |_| 0);
        let r = ride.recorded.as_ref().unwrap();
        assert_eq!((r.rays.wreck, r.rays.riding, r.wreck.len()), (wreck, riding, from_file.len()));
        println!("wreck body built on {}: {} rays, bit for bit the image's", quad::WRECK_FILE, from_file.len());
    } else {
        println!("{} is not there: the wreck rays are the image's part", quad::WRECK_FILE);
    }
    let q = image::IMAGE_QUAD;
    let f = |o: u32| F(pre.f32_at(q + o));
    let state = pre.u32_at(q + 0xbc4);
    let tail = quad::Tail {
        vehicle_state: (state != 0).then(|| pre.u32_at(state + 0x28)),
        stunt_speed: f(0x174c),
        surfaces: std::array::from_fn(|i| {
            let a = q + 0xdf4 + 0xc * i as u32;
            quad::Surface { hazard: pre.u32_at(a) != 0, time: F(pre.f32_at(a + 4)) }
        }),
        ..quad::Tail::default()
    };
    ride.tailed = Some(quad::Tailed { tail, ..quad::Tailed::default() });
    let class = pre.u32_at(game_sim::handling::wreck::STRUCK_CLASS);
    let max_time = f(game_sim::handling::wreck::MAX_WRECK_TIME);
    ride.wrecked = Some(quad::Wrecked { max_time, class, category: class, ..quad::Wrecked::default() });
}

/// The image-seeded quad put down at `start` (two feet over the ground) facing `yaw` (radians from
/// `+z` toward `+x`), with MD123a's record, tail and wreck.
fn seeded(pre: &Ee, at: &image::Image, r: &mut Ride, world: &mut Place<'_>, start: (f32, f32), yaw: f32) -> quad::Ride {
    let (floor, _) = world.ground(Vf::new(start.0, 1.0e4, start.1, 1.0));
    oasis::place(r, [start.0, floor.y() + 2.0, start.1], [0.0, (yaw / 2.0).sin(), 0.0, (yaw / 2.0).cos()]);
    let mut body = seed::body(&|a| pre.u32_at(a), at.body);
    body.set_view(&r.s.body);
    (body.last_position, body.last_quat) = (body.position, body.quat);
    let mut ride = quad::Ride::new(r.s.p.clone(), r.s.q.clone(), body, r.bike.clone(), Vec::<Call>::new());
    md123a_rigs(pre, at, &mut ride);
    ride
}

/// A lip in free09's terrain near the spot, and the run at it: the nearest point (on an 8 ft grid,
/// eight headings) where the ground rises [`LIP_RISE`] over the 15 ft before it and falls as much
/// over the 15 ft after, with the 90 ft before the ramp within 12 ft of level. Answers the run's start
/// and heading.
fn lip(world: &Place<'_>) -> Option<((f32, f32), f32)> {
    let h = |x: f32, z: f32| world.ground.at(F(x), F(z)).map(|g| g.y.0);
    let mut best: Option<(f32, (f32, f32), f32)> = None;
    for i in -100..=100 {
        for j in -100..=100 {
            let (x, z) = (SPOT.0 + 8.0 * i as f32, SPOT.1 + 8.0 * j as f32);
            let d = (8.0 * i as f32).hypot(8.0 * j as f32);
            if best.is_some_and(|b| b.0 <= d) {
                continue;
            }
            let Some(top) = h(x, z) else { continue };
            for a in 0..8 {
                let yaw = a as f32 * std::f32::consts::FRAC_PI_4;
                let (ux, uz) = (yaw.sin(), yaw.cos());
                let at = |t: f32| h(x + ux * t, z + uz * t);
                let (Some(face), Some(after)) = (at(-15.0), at(15.0)) else { continue };
                if top - face < LIP_RISE || top - after < LIP_RISE {
                    continue;
                }
                let run: Option<Vec<f32>> = (2..=10).map(|k| at(-15.0 - 10.0 * k as f32)).collect();
                let Some(run) = run else { continue };
                let (lo, hi) = run.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
                if hi - lo < 12.0 {
                    best = Some((d, (x - ux * 105.0, z - uz * 105.0), yaw));
                }
            }
        }
    }
    best.map(|(_, start, yaw)| (start, yaw))
}

/// How much a lip rises and falls ([`lip`]), ft; the frames ridden at it; the frames in the air
/// that are a jump.
const LIP_RISE: f32 = 4.0;
const LIP_FRAMES: usize = 240;
const AIRBORNE: usize = 8;

/// The name of a call, its variant.
fn call_name(c: &Call) -> String {
    format!("{c:?}").split([' ', '(', '{']).next().unwrap().to_string()
}

#[test]
fn the_quad_jumps_a_free09_lip_strikes_a_prop_wrecks_and_is_put_back() {
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
        // The lip: a run at it on the gas, off it into the air, and down again on all four.
        let (start, yaw) = lip(world).expect("a lip in free09's terrain near the spot");
        let mut ride = seeded(&pre, &at, &mut r, world, start, yaw);
        let (mut flight, mut longest, mut landed) = (0usize, 0usize, 0usize);
        for k in 0..LIP_FRAMES {
            ride.rest.clear();
            ride.step(&Controls { gas: F::ONE, ..Controls::default() }, dt, world, segments);
            let p = ride.body.position;
            assert!(p.0.iter().all(|v| v.is_finite()), "lip frame {k}: the body is at {p:?}");
            assert_eq!(ride.wreck(), None, "lip frame {k}: the quad wrecked off the lip at {p:?}");
            flight = if ride.q.airborne { flight + 1 } else { 0 };
            longest = longest.max(flight);
            if longest >= AIRBORNE && ride.q.all_down {
                landed += 1;
            }
        }
        eprintln!("MD123a free09 lip from {start:?} yaw {yaw}: {longest} frames in the air, {landed} on all four after");
        assert!(longest >= AIRBORNE, "the quad was in the air {longest} frames off the lip");
        assert!(landed > 0, "the quad never came down on all four after the lip");

        // The static prop nearest the spot with a face a rider's height tall toward a run up to it on
        // the ground: a level ray from the run's start at one and at four feet strikes this prop first,
        // near the far end.
        let level = |world: &mut Place<'_>, a: Vf, b: Vf| {
            use game_sim::collide::{Bundle, Probe, Record};
            let mut bundle = Bundle { count: 1, centre: a, radius: F(RUN_UP + 40.0), ..Bundle::default() };
            bundle.probes[0] = Probe::between(a, b);
            let mut record = Record::new();
            world.solid(&bundle, &mut record);
            record.hit[0].then_some((record.fraction[0].0, record.owner[0]))
        };
        let mut candidates: Vec<(usize, Vf, f32)> = world
            .props
            .props
            .iter()
            .enumerate()
            .filter(|(_, p)| p.moving.is_none() && (1.0..20.0).contains(&p.radius.0))
            .map(|(i, p)| {
                let (dx, dz) = (SPOT.0 - p.centre.x(), SPOT.1 - p.centre.z());
                (i, p.centre, (dx * dx + dz * dz).sqrt())
            })
            .collect();
        candidates.sort_by(|a, b| a.2.total_cmp(&b.2));
        let (prop, start) = candidates
            .iter()
            .take(400)
            .find_map(|&(i, c, d)| {
                let r = world.props.props[i].radius.0;
                let (ux, uz) = ((SPOT.0 - c.x()) / d.max(1.0), (SPOT.1 - c.z()) / d.max(1.0));
                let run = r + RUN_UP;
                let start = (c.x() + ux * run, c.z() + uz * run);
                let g = world.ground.at(F(start.0), F(start.1))?.y.0;
                let ok = [1.0f32, 4.0].iter().all(|&h| {
                    let a = Vf::new(start.0, g + h, start.1, 1.0);
                    let b = Vf::new(c.x(), g + h, c.z(), 1.0);
                    level(world, a, b).is_some_and(|(f, owner)| owner == i as i32 && f * run > RUN_UP - 10.0)
                });
                ok.then_some(((c, r, i), start))
            })
            .expect("a tall prop on free09's ground with a clear run up");
        let toward = (prop.0.x() - start.0, prop.0.z() - start.1);
        let yaw = toward.0.atan2(toward.1);
        let mut ride = seeded(&pre, &at, &mut r, world, start, yaw);
        let rec = &ride.recorded.as_ref().unwrap().record;
        eprintln!(
            "MD123a prop {} {:?} r {} from {start:?} yaw {yaw}; record part {} rays {} world rays {} hf {} terrain {} body {} mask {:#x}, \
             riding rays {}, wreck rays {}, wrecks {:?}",
            prop.2, prop.0, prop.1, rec.part, rec.rays, rec.world_rays, rec.heightfield, rec.terrain, rec.body, rec.mask,
            ride.recorded.as_ref().unwrap().riding.len(), ride.recorded.as_ref().unwrap().wreck.len(), ride.wrecked
        );

        let (mut wrecked_at, mut put_at, mut aloft, mut touched) = (None, None, 0usize, 0usize);
        let mut handed = std::collections::BTreeSet::new();
        let mut back_at = Vf::ZERO;
        for k in 0..FRAMES {
            ride.rest.clear();
            // On after the put-back, the stick hard over: the spot kept faces the prop.
            let stick = if put_at.is_some() { (F::ONE, F::ZERO) } else { (F::ZERO, F::ZERO) };
            ride.step(&Controls { gas: F::ONE, stick, ..Controls::default() }, dt, world, segments);
            let p = ride.body.position;
            assert!(p.0.iter().all(|v| v.is_finite()), "frame {k}: the body is at {p:?}");
            handed.extend(ride.rest.iter().map(call_name));
            aloft += usize::from(ride.q.airborne);
            touched += usize::from(ride.recorded.as_ref().unwrap().record.on_object);
            if let Some(why) = ride.wreck() {
                eprintln!("MD123a frame {k}: wreck {why} at {p:?}, speed {}", ride.q.speed_len.0);
                wrecked_at.get_or_insert((k, why));
            }
            if ride.rest.iter().any(|c| matches!(c, Call::VehiclePutBack { .. })) && put_at.is_none() {
                eprintln!("MD123a frame {k}: put back to {p:?}");
                put_at = Some(k);
                back_at = p;
            }
            if put_at.is_some_and(|f| k > f + RIDE_ON) {
                break;
            }
        }
        let gone = ride.body.position.sub(back_at);
        let gone = (gone.x() * gone.x() + gone.z() * gone.z()).sqrt();
        eprintln!(
            "MD123a free09: wreck {wrecked_at:?}, put back {put_at:?}, then {gone} ft on; aloft {aloft} frames, the record on a prop {touched}; \
             handed on {handed:?}"
        );
        assert!(touched > 0, "the record never touched the prop");
        let (k, why) = wrecked_at.expect("the strike on the prop did not wreck the quad");
        assert_eq!(why, 15, "the strike's wreck is reason 15");
        let put = put_at.expect("the wrecked quad was never put back");
        assert!(put > k, "put back before the wreck");
        assert!(gone > 10.0, "the quad did not ride on after the put-back: {gone} ft");
    });
}

/// The run up to the prop, ft; the frames the strike's ride may take; the frames ridden on after the
/// put-back.
const RUN_UP: f32 = 60.0;
const FRAMES: usize = 900;
const RIDE_ON: usize = 120;
