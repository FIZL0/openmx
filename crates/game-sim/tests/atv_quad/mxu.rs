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
    let limits = quad::Limits {
        front: f(0x13e8),
        back: f(0x13ec),
        side: f(0x13f0),
        racing_down: f(0x13f4),
        freestyle_down: f(0x13f8),
        ground_pitch_down: f(0x13fc),
        ground_pitch_up: f(0x1400),
        airborne_pitch_down: f(0x1404),
        airborne_pitch_up: f(0x1408),
        airborne_roll: f(0x140c),
        no_hander: f(0x1410),
        no_hander_max_roll: f(0x1414),
        joust_height: f(0x1418),
        crossup: f(0x1420),
        crossup_time: f(0x1424),
    };
    let word = |o: u32| pre.u32_at(q + o);
    ride.wrecked = Some(quad::Wrecked {
        max_time,
        class,
        category: class,
        limits,
        edge: word(0x308) != 0,
        wrecks: pre.u32_at(game_sim::handling::wreck::WRECKS_ON) != 0 && word(0xc1c) != 0,
        small: word(0xbc0) == 0,
        balance: word(0x152c) as i32,
        wanted: word(0x1530) as i32,
        blend: f(0x14e4),
        trick_wrecked: word(0x14cc) != 0,
        racing: word(0x146c) != 0,
        crossup: f(0x141c),
        ..quad::Wrecked::default()
    });
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

/// MD123 — the quad made from its files alone ([`quad::Ride::spawn`]): the R quad's `atv_race250p.phy`
/// (MD122's) and its chassis `250r_atv_chasis.psg` (MD129's), or why they are not there.
fn quad_files() -> Option<(Ini, quad::Model)> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let dir = kit.ctx("mxvatv-ps2/md37a");
    let (phy, psg) = (dir.join("phy/atv_race250p.phy"), dir.join("250r_atv_chasis.psg"));
    let (Ok(phy), Ok(psg)) = (std::fs::read(&phy), std::fs::read(&psg)) else {
        skip(&format!(
            "{} or {} is not there: both are MXVSATV.PSR's vehicletype_1.res (research/journal/2026-11-11-md129-atv-model-load)",
            phy.display(),
            psg.display()
        ));
        return None;
    };
    Some((Ini::parse_bytes(&phy), quad::Model::parse(&psg).unwrap_or_else(|e| panic!("250r_atv_chasis.psg: {e}"))))
}

/// The quad made from its files at `at` facing `heading`, as [`quad::Ride::spawn`] makes it.
fn from_files(files: &(Ini, quad::Model), at: Vf, heading: Vf, bounds: Option<(Vf, Vf)>) -> (quad::Ride, quad::Load) {
    let curve = game_sim::camera::CurveTable::build();
    let load = quad::Load::read_model(&files.0, &files.1, at, heading, &curve).unwrap_or_else(|e| panic!("{e}"));
    (quad::Ride::spawn(&load, &files.1, at, heading, bounds, Vec::<Call>::new()), load)
}

/// Bits of a debug print's floats, for comparing a struct whole.
fn same<T: std::fmt::Debug>(what: &str, ours: &T, image: &T, diffs: &mut Vec<String>) {
    let (a, b) = (format!("{ours:?}"), format!("{image:?}"));
    if a != b {
        diffs.push(format!("{what}:\n  ours  {a}\n  image {b}"));
    }
}

/// The quad made from its files at C28's spawn point and heading is the spawn image's in every word
/// the ride does not move: its keys, its body's limits and mass, the engine's, air and clutch keys, the
/// wheels' tuning and tires, the 360's keys and stunt ids, the centres of gravity, and the chain of
/// chassis nodes under the scene node with their bind locals. The image is some frames into the ride,
/// so its state (pose, speeds, contacts) is not compared; the record's inertia is MD134's one float.
#[test]
fn the_quad_from_its_files_is_c28s_spawned_quad() {
    let Some(files) = quad_files() else { return };
    let kit = Kit::load().unwrap();
    let path = kit.ctx("mxvatv-ps2/md37a").join("c28-atv-r-spawn.ee.bin");
    let Ok(ee) = Ee::from_ram(&path) else {
        skip(&format!("{} is not there (C28's; memory: md37a ATV states live in s2)", path.display()));
        return;
    };
    const QUAD: u32 = 0x0150_a990;
    let dt = F::from_bits(0x3d08_850a);
    let (img, at) = image::read_ride(&ee, QUAD, dt).unwrap_or_else(|e| panic!("{e}"));
    let v = |a: u32| Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12));
    let (ours, _) = from_files(&files, v(QUAD + 0x50), v(QUAD + 0x60), img.bike.parts.bounds.area);
    let ib = seed::body(&|a| ee.u32_at(a), at.body);
    let (b, ob) = (&img.bike, &ours.bike);
    let mut diffs = Vec::new();
    same("params", &ours.p, &img.s.p, &mut diffs);
    let limits = |x: &game_sim::atv_body::Body| (x.enabled, x.linear_axes, x.angular_axes, x.damping, x.ang_damping, x.max_speed, x.max_accel, x.max_rates, x.max_ang_accel);
    same("body limits", &limits(&ours.body), &limits(&ib), &mut diffs);
    same("body mass", &ours.body.inv_mass, &ib.inv_mass, &mut diffs);
    same("joystick", &ob.joystick, &b.joystick, &mut diffs);
    same("air keys", &ob.air_keys, &b.air_keys, &mut diffs);
    same("gyro and gas gyro", &(ob.gyro, ob.gas_gyro_torque), &(b.gyro, b.gas_gyro_torque), &mut diffs);
    same("stunt ids", &(ob.stunt_id, ob.rider_stunt, ob.flip_stunt, ob.flip_clip), &(b.stunt_id, b.rider_stunt, b.flip_stunt, b.flip_clip), &mut diffs);
    same("cofgs", &(ob.cofg, ob.parts.cofg_off), &(b.cofg, b.parts.cofg_off), &mut diffs);
    let ck = |p: &game_sim::r#move::Params| (p.clutch_torque_scale_time, p.max_clutch_torque_scale_speed, p.max_clutch_torque_scale, p.up_shift_rpm, p.down_shift_rpm, p.cofg, p.balance_cofg, p.cofg_travel_forward, p.cofg_travel_back, p.clutch_cofg_travel_back, p.brake_cofg_travel_forward, p.drag);
    same("clutch keys", &ck(&ob.clutch_keys), &ck(&b.clutch_keys), &mut diffs);
    let ek = |e: &game_sim::r#move::Engine| ((e.gear, e.clutch_torque_scale, e.torque_scale), e.gears, e.final_drive, e.rev_limit, e.idle_throttle, e.engine_braking, e.inv_inertia, e.clutch_static, e.clutch_kinetic, e.min_rpm, e.curve.points);
    same("engine keys", &ek(&ob.engine), &ek(&b.engine), &mut diffs);
    // The tables past their counts are whatever followed them in RAM.
    let tables = |e: &game_sim::r#move::Engine| (e.gear_ratios[..e.gears as usize].to_vec(), e.curve_rpm[..10].to_vec(), e.curve_torque[..10].to_vec());
    same("engine tables", &tables(&ob.engine), &tables(&b.engine), &mut diffs);
    let s = |x: &game_sim::atv_moto::Spin| (x.min_time, x.max_time, x.min_pitch, x.max_pitch, x.min_yaw, x.max_yaw, x.rate, x.stunt_right, x.stunt_left);
    same("360 keys", &s(&ob.parts.spin), &s(&b.parts.spin), &mut diffs);
    same("parts flags", &(ob.parts.driven, ob.parts.body_on, ob.parts.flags, ob.parts.record), &(b.parts.driven, b.parts.body_on, b.parts.flags, b.parts.record), &mut diffs);
    for k in 0..4 {
        let w = |w: &game_sim::wheel::Wheel| ((w.has_ground, w.steered, w.mount, w.probe_length), (w.max_compression, w.max_damper_speed, w.extension_rate, w.damping_out, w.damping_in), (w.max_steer_angle, w.min_steer_angle, w.steer_damping, w.steer_inv_inertia));
        same(&format!("wheel {k}"), &w(&ob.wheels[k]), &w(&b.wheels[k]), &mut diffs);
        let sp = |s: &game_sim::atv_wheel::Spin| (s.brake.is_some(), s.parametric, s.inv_inertia, s.tire_inertia, s.arm, s.other_load_scale, s.inv_radius, s.surfaces);
        same(&format!("wheel {k}'s spin"), &sp(&ob.spins[k]), &sp(&b.spins[k]), &mut diffs);
        let qw = |w: &quad::Wheel| (w.steered, w.balancing, w.max_steer_angle, w.inv_radius);
        same(&format!("quad wheel {k}"), &qw(&ours.q.wheels[k]), &qw(&img.s.q.wheels[k]), &mut diffs);
        same(&format!("more {k}'s steer axis"), &ours.bike.more[k].steer_axis, &b.more[k].steer_axis, &mut diffs);
    }
    let qk = |q: &Quad| (q.active, q.push, q.push_target, q.push_gain, q.gravity, q.authority_time);
    same("quad keys", &qk(&ours.q), &qk(&img.s.q), &mut diffs);
    // The chain's links; its locals are the animation's pose by now (the root bobbed, the next node
    // turned), the bind pose at the reload (MD129).
    for n in 1..game_sim::atv_moto::NODES {
        let node = |x: &game_sim::atv_moto::Node| (x.parent, x.child, x.sibling);
        same(&format!("scene node {n}"), &node(&ob.parts.scene.nodes[n]), &node(&b.parts.scene.nodes[n]), &mut diffs);
    }
    // The scene node's box was last gathered in the image's own frame, over the animated pose: within
    // a few 1e-4 of the bind pose's at the spawn frame, not bit for bit.
    let (o0, i0) = (&ob.parts.scene.nodes[0], &b.parts.scene.nodes[0]);
    let off = (0..3).map(|k| (o0.centre.0[k] - i0.centre.0[k]).abs().max((o0.half.0[k] - i0.half.0[k]).abs())).fold(0.0f32, f32::max);
    eprintln!("MD123 spawn: the scene node's box is {off} ft off the image's");
    assert!(off < 1.0e-3, "the scene node's box is {off} ft off the image's: {:?} {:?} against {:?} {:?}", o0.centre, o0.half, i0.centre, i0.half);
    for d in &diffs {
        eprintln!("MD123 spawn differs, {d}");
    }
    assert!(diffs.is_empty(), "{} groups differ from C28's spawned quad", diffs.len());
}

/// The quad made from its files put down on free09's ground at `start`, facing `yaw` (radians from
/// `+z` toward `+x`).
fn files_on(files: &(Ini, quad::Model), world: &mut Place<'_>, start: (f32, f32), yaw: f32) -> (quad::Ride, quad::Load) {
    let (floor, _) = world.ground(Vf::new(start.0, 1.0e4, start.1, 1.0));
    assert!(floor.y() > -1.0e5, "no ground at {start:?}");
    let at = Vf::new(start.0, floor.y(), start.1, 1.0);
    let heading = Vf::new(yaw.sin(), 0.0, yaw.cos(), 1.0);
    let area = world.area().map(|a| (Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(a.width.0, 0.0, a.depth.0, 1.0)));
    from_files(files, at, heading, area)
}

/// The quad made from its files rides free09 headless: it stands, gets going on the gas, turns on the
/// stick and stays on the ground, as MD37a's spawned quad does (`the_spawned_quad_rides_free09_headless`);
/// then, from a run at free09's lip, it is in the air and comes down on all four.
#[test]
fn the_quad_from_its_files_rides_free09_headless() {
    let Some(files) = quad_files() else { return };
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    let dt = F::from_bits(0x3d08_850a);
    in_free09(&mut disc, |world| {
        let (mut ride, load) = files_on(&files, world, SPOT, 0.0);
        let segments = quad::trigger_segments(&load.tires, true).expect("the trigger probe's segments");
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
            handed.extend(ride.rest.iter().map(call_name));
            let p = ride.body.position;
            assert!(p.0.iter().all(|v| v.is_finite()), "frame {k}: the body is at {p:?}");
            let (g, _) = world.ground(p);
            lowest = lowest.min(p.y() - g.y());
            top = top.max(ride.q.speed_len.0);
            down += usize::from(ride.q.all_down);
            if k == STAND - 1 {
                let d = ride.body.position.sub(start);
                eprintln!("MD123 files: stood {STAND} frames, moved {:?}, all four down {}", d, ride.q.all_down);
            }
            if k == GAS - 1 {
                turned.0 = heading(ride.body.velocity);
            }
            // The most it turned while it still had way on: past the turn the quad leaves the track
            // and comes down tangled (the image's quad as well), so the last frame's heading says
            // nothing about the stick.
            if k >= GAS && ride.q.speed_len.0 > 20.0 {
                let t = (heading(ride.body.velocity) - turned.0 + std::f32::consts::PI).rem_euclid(2.0 * std::f32::consts::PI) - std::f32::consts::PI;
                if t.abs() > turned.1.abs() {
                    turned.1 = t;
                }
            }
            assert_eq!(ride.wreck(), None, "frame {k}: the quad wrecked on free09");
        }
        let d = ride.body.position.sub(start);
        let gone = (d.x() * d.x() + d.z() * d.z()).sqrt();
        let turn = turned.1;
        eprintln!(
            "MD123 files free09: {gone} ft gone, top {top} ft/s, all four down {down} of {STEER} frames, lowest {lowest} ft over the ground, \
             turned {turn} rad at most on the stick; handed on {handed:?}"
        );
        assert!(top > 20.0, "the quad never got going on the gas: top {top} ft/s");
        assert!(gone > 50.0, "the quad went only {gone} ft");
        assert!(down > STEER / 2, "the quad was off the ground {} of {STEER} frames", STEER - down);
        assert!(lowest > -1.0, "the body sank {lowest} ft into the ground");
        assert!(turn.abs() > 0.3, "the stick did not turn the quad: {turn} rad");
        let known = ["GravityRecord", "PartMove", "SpatialMove", "PreloadCue", "StartStunt", "EndStunt"];
        assert!(handed.iter().all(|n| known.contains(&n.as_str())), "calls nobody answers on free09: {handed:?}");

        // The lip.
        let (start, yaw) = lip(world).expect("a lip in free09's terrain near the spot");
        let (mut ride, _) = files_on(&files, world, start, yaw);
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
        eprintln!("MD123 files free09 lip from {start:?} yaw {yaw}: {longest} frames in the air, {landed} on all four after");
        assert!(longest >= AIRBORNE, "the quad was in the air {longest} frames off the lip");
        assert!(landed > 0, "the quad never came down on all four after the lip");
    });
}

/// MD123's rigs' files ([`quad::Ride::rig`]): the bike's `m.col` (the riding rays) and
/// [`quad::WRECK_FILE`], both MXVSATV.PSR's `Veh\\Col`, or why they are not there.
fn quad_cols() -> Option<(game_sim::track::Rays, game_sim::track::Rays)> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let dir = kit.ctx("mxvatv-ps2/md37a");
    let (riding, wreck) = (dir.join("250r_atv_chasism.col"), dir.join(quad::WRECK_FILE));
    let (Ok(r), Ok(w)) = (std::fs::read(&riding), std::fs::read(&wreck)) else {
        skip(&format!(
            "{} or {} is not there: MXVSATV.PSR's Veh\\Col\\250R_ATV_ChasisM.col and WreckATVm.col (research/journal/2026-11-10-md123-atv-on-mxu)",
            riding.display(),
            wreck.display()
        ));
        return None;
    };
    let parse = |b: &[u8], what: &str| game_sim::track::Rays::parse(b).unwrap_or_else(|e| panic!("{what}: {e}"));
    Some((parse(&r, "250r_atv_chasism.col"), parse(&w, quad::WRECK_FILE)))
}

/// The quad made from its files with its rigs from its files is MD123a's image-seeded rigs in every
/// word the ride does not move: the collision record's flags and tuning, its resolution's fields, the
/// riding and the wreck rays bit for bit, the tail's mode, stunt speed and surfaces, and the wreck's
/// time and class. The record's last sweep and contacts, and its resolution's speeds, are state; its
/// inverse inertia is MD134's one float off.
#[test]
fn the_quad_rigged_from_its_files_is_md123as() {
    let (Some(files), Some(cols)) = (quad_files(), quad_cols()) else { return };
    let Some(pre) = image() else { return };
    let (r, at) = image::read_ride(&pre, image::IMAGE_QUAD, F::from_bits(0x3d08_850a)).unwrap_or_else(|e| panic!("{e}"));
    let mut img = quad::Ride::new(r.s.p.clone(), r.s.q.clone(), seed::body(&|a| pre.u32_at(a), at.body), r.bike.clone(), Vec::<Call>::new());
    md123a_rigs(&pre, &at, &mut img);
    let v = |a: u32| Vf::new(pre.f32_at(a), pre.f32_at(a + 4), pre.f32_at(a + 8), pre.f32_at(a + 12));
    let (mut ours, load) = from_files(&files, v(image::IMAGE_QUAD + 0x50), v(image::IMAGE_QUAD + 0x60), None);
    ours.rig(&load, &cols.0, &cols.1);
    let (a, b) = (ours.recorded.as_ref().unwrap(), img.recorded.as_ref().unwrap());
    let mut diffs = Vec::new();
    let still = |r: &game_sim::handling::record::Record| {
        let mut r = r.clone();
        (r.swept_from, r.swept_to, r.contacts) = (Vf::ZERO, Vf::ZERO, Vec::new());
        r
    };
    same("record", &still(&a.record), &still(&b.record), &mut diffs);
    let collider = |h: &game_sim::atv_contact::Held| {
        let mut c = h.record.clone();
        (c.collider.cofg_world, c.collider.normal_speed, c.collider.relative) = (Vf::ZERO, F::ZERO, Vf::ZERO);
        c.collider.inv_inertia = Vf::ZERO;
        (c, h.mass, h.body.is_some(), h.entries.len(), h.movers.len())
    };
    same("held", &collider(&a.held), &collider(&b.held), &mut diffs);
    // The record's inverse inertia is the part reader's from the model's box: MD134's one float.
    let (ii, ij) = (a.held.record.collider.inv_inertia, b.held.record.collider.inv_inertia);
    let off = ii.0.iter().zip(ij.0).map(|(x, y)| (x - y).abs() / y.abs().max(1e-6)).fold(0.0f32, f32::max);
    println!("MD123 rigs: the record's inverse inertia is {off} (relative) off the image's (MD134)");
    assert!(off < 1e-5, "the record's inverse inertia: ours {ii:?}, the image's {ij:?}");
    let bits = |r: &[[Vf; 2]]| r.iter().flat_map(|s| s.iter().flat_map(|v| v.0.map(f32::to_bits))).collect::<Vec<_>>();
    same("riding rays", &bits(&a.riding), &bits(&b.riding), &mut diffs);
    same("wreck rays", &bits(&a.wreck), &bits(&b.wreck), &mut diffs);
    same("rays active", &(a.rays.active == a.rays.riding, a.rays.wreck != a.rays.riding), &(b.rays.active == b.rays.riding, b.rays.wreck != b.rays.riding), &mut diffs);
    let tail = |t: &quad::Tailed| (t.tail.vehicle_state, t.tail.stunt_speed, t.tail.surfaces);
    same("tail", &tail(ours.tailed.as_ref().unwrap()), &tail(img.tailed.as_ref().unwrap()), &mut diffs);
    same("wreck", &ours.wrecked, &img.wrecked, &mut diffs);
    assert!(diffs.is_empty(), "the files' rigs are not MD123a's:\n{}", diffs.join("\n"));
    println!("MD123 rigs: {} riding rays and {} wreck rays from the files, the record and the wreck MD123a's", a.riding.len(), a.wreck.len());
}

/// The static prop nearest [`SPOT`] with a face a rider's height tall toward a run up to it on the
/// ground (as the MD123a test finds it): the prop's centre and index, and the run's start.
fn prop_run(world: &mut Place<'_>) -> (Vf, usize, (f32, f32)) {
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
    candidates
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
            ok.then_some((c, i, start))
        })
        .expect("a tall prop on free09's ground with a clear run up")
}

/// The quad made from its files, rigged from its files, strikes free09's prop on the gas, wrecks
/// (reason 15), is put back when `MaxWreckTime` runs out, and rides on — as MD123a's image-seeded one
/// does (`the_quad_jumps_a_free09_lip_strikes_a_prop_wrecks_and_is_put_back`).
#[test]
fn the_quad_from_its_files_wrecks_on_a_free09_prop_and_is_put_back() {
    let (Some(files), Some(cols)) = (quad_files(), quad_cols()) else { return };
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    let dt = F::from_bits(0x3d08_850a);
    in_free09(&mut disc, |world| {
        let (centre, prop, start) = prop_run(world);
        let yaw = (centre.x() - start.0).atan2(centre.z() - start.1);
        let (mut ride, load) = files_on(&files, world, start, yaw);
        ride.rig(&load, &cols.0, &cols.1);
        let segments = quad::trigger_segments(&load.tires, true).expect("the trigger probe's segments");
        eprintln!("MD123 files prop {prop} {centre:?} from {start:?} yaw {yaw}");
        let (mut wrecked_at, mut put_at, mut touched) = (None, None, 0usize);
        let mut handed = std::collections::BTreeSet::new();
        let (mut back_at, mut far) = (Vf::ZERO, 0.0f32);
        for k in 0..FRAMES {
            ride.rest.clear();
            let stick = if put_at.is_some() { (F::ONE, F::ZERO) } else { (F::ZERO, F::ZERO) };
            ride.step(&Controls { gas: F::ONE, stick, ..Controls::default() }, dt, world, segments);
            let p = ride.body.position;
            assert!(p.0.iter().all(|v| v.is_finite()), "frame {k}: the body is at {p:?}");
            handed.extend(ride.rest.iter().map(call_name));
            touched += usize::from(ride.recorded.as_ref().unwrap().record.on_object);
            if let Some(why) = ride.wreck() {
                if wrecked_at.is_none() {
                    eprintln!("MD123 files frame {k}: wreck {why} at {p:?}, speed {}", ride.q.speed_len.0);
                }
                wrecked_at.get_or_insert((k, why));
            }
            if ride.rest.iter().any(|c| matches!(c, Call::VehiclePutBack { .. })) && put_at.is_none() {
                eprintln!("MD123 files frame {k}: put back to {p:?}");
                put_at = Some(k);
                back_at = p;
            }
            if put_at.is_some() {
                // The farthest it gets: on the full stick it rides a circle back toward the spot.
                let d = p.sub(back_at);
                far = far.max((d.x() * d.x() + d.z() * d.z()).sqrt());
            }
            if put_at.is_some_and(|f| k > f + RIDE_ON) {
                break;
            }
        }
        let gone = far;
        eprintln!(
            "MD123 files free09: wreck {wrecked_at:?}, put back {put_at:?}, then {gone} ft on; the record on a prop {touched}; handed on {handed:?}"
        );
        assert!(touched > 0, "the record never touched the prop");
        let (k, why) = wrecked_at.expect("the strike on the prop did not wreck the quad");
        assert_eq!(why, 15, "the strike's wreck is reason 15");
        let put = put_at.expect("the wrecked quad was never put back");
        assert!(put > k, "put back before the wreck");
        assert!(gone > 10.0, "the quad did not ride on after the put-back: {gone} ft");
    });
}

/// MD133's drops: how high over free09's ground at [`SPOT`] the quad made from its files is let go,
/// and how many frames it gets to wreck in.
const DROP: f32 = 300.0;
const DROP_FRAMES: usize = 300;

/// The quad made from its files and rigged from them, [`DROP`] over free09's ground at [`SPOT`] facing
/// `+z`, turned `pitch` degrees about its own side axis (positive: the nose down), let go with the
/// controls off: the first wreck it is entered for, the frame, and the ride.
fn dropped(world: &mut Place<'_>, files: &(Ini, quad::Model), cols: &(game_sim::track::Rays, game_sim::track::Rays), pitch: f32) -> Option<(usize, u8, quad::Ride)> {
    let (floor, _) = world.ground(Vf::new(SPOT.0, 1.0e4, SPOT.1, 1.0));
    let at = Vf::new(SPOT.0, floor.y() + DROP, SPOT.1, 1.0);
    let area = world.area().map(|a| (Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(a.width.0, 0.0, a.depth.0, 1.0)));
    let (mut ride, load) = from_files(files, at, Vf::new(0.0, 0.0, 1.0, 1.0), area);
    ride.rig(&load, &cols.0, &cols.1);
    let (s, c) = (pitch.to_radians() / 2.0).sin_cos();
    let q = ride.body.quat;
    // q · (the turn about x), Hamilton's product.
    let turned = Vf::new(q.w() * s + q.x() * c, q.y() * c + q.z() * s, q.z() * c - q.y() * s, q.w() * c - q.x() * s);
    (ride.body.quat, ride.body.last_quat) = (turned, turned);
    let segments = quad::trigger_segments(&load.tires, true).expect("the trigger probe's segments");
    let dt = F::from_bits(0x3d08_850a);
    for k in 0..DROP_FRAMES {
        ride.rest.clear();
        ride.step(&Controls::default(), dt, world, segments);
        assert!(ride.body.position.0.iter().all(|v| v.is_finite()), "frame {k}: the body is at {:?}", ride.body.position);
        if let Some(why) = ride.wreck() {
            eprintln!("MD133 drop pitch {pitch}: frame {k} wreck {why} at {:?}, velocity {:?}", ride.body.position, ride.body.velocity);
            return Some((k, why, ride));
        }
    }
    None
}

/// MD133: dropped level from [`DROP`] ft onto free09, the quad made from its files lands too hard —
/// the vertical load past `RacingDownWreckSpeedLimit` — and wrecks flat (5) on the frame it lands.
#[test]
fn the_quad_dropped_level_onto_free09_wrecks_flat() {
    let (Some(files), Some(cols)) = (quad_files(), quad_cols()) else { return };
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    in_free09(&mut disc, |world| {
        let (k, why, ride) = dropped(world, &files, &cols, 0.0).expect("the dropped quad did not wreck");
        assert_eq!(why, 5, "the hard landing's wreck is flat (5), frame {k}");
        assert!(ride.q.on_ground, "the flat wreck came before the landing, frame {k}");
        assert_eq!(ride.wrecked.as_ref().unwrap().reason, 5);
    });
}

/// MD133: dropped nose down (88°, past `AirbornePitchDownWreckAngle`), the quad made from its files
/// nose-dives (8) in the air, before it lands.
#[test]
fn the_quad_dropped_nose_down_over_free09_nose_dives() {
    let (Some(files), Some(cols)) = (quad_files(), quad_cols()) else { return };
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    in_free09(&mut disc, |world| {
        let (k, why, ride) = dropped(world, &files, &cols, 88.0).expect("the dropped quad did not wreck");
        assert_eq!(why, 8, "the nose-down drop's wreck is the nose-dive (8), frame {k}");
        assert!(!ride.q.on_ground, "the nose-dive came after the landing, frame {k}");
    });
}

/// How far in from free09's west edge the edge run starts, and its frames.
const EDGE_RUN: f32 = 200.0;
const EDGE_FRAMES: usize = 600;

/// MD133: ridden on the gas west at free09's edge, the quad made from its files is out of bounds (13)
/// once within 150 ft of it, thrown back toward the map's middle (its velocity turned east and up),
/// and stays down for the cannonball's window and `MaxWreckTime` before it is put back on the map.
#[test]
fn the_quad_ridden_off_free09s_edge_is_out_of_bounds() {
    let (Some(files), Some(cols)) = (quad_files(), quad_cols()) else { return };
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    let dt = F::from_bits(0x3d08_850a);
    in_free09(&mut disc, |world| {
        let start = (EDGE_RUN, SPOT.1);
        let (mut ride, load) = files_on(&files, world, start, -std::f32::consts::FRAC_PI_2);
        ride.rig(&load, &cols.0, &cols.1);
        let segments = quad::trigger_segments(&load.tires, true).expect("the trigger probe's segments");
        let (mut wrecked_at, mut put_at) = (None, None);
        for k in 0..EDGE_FRAMES {
            ride.rest.clear();
            let x0 = ride.body.position.x();
            ride.step(&Controls { gas: F::ONE, ..Controls::default() }, dt, world, segments);
            let (p, v) = (ride.body.position, ride.body.velocity);
            assert!(p.0.iter().all(|f| f.is_finite()), "frame {k}: the body is at {p:?}");
            if let Some(why) = ride.wreck() {
                if wrecked_at.is_none() {
                    eprintln!("MD133 edge frame {k}: wreck {why} at {p:?} (from x {x0}), thrown at {v:?}");
                    assert_eq!(why, 13, "the edge's wreck is out of bounds (13)");
                    assert!(p.x() < 150.0 && 150.0 <= x0, "out of bounds at x {}, not the frame it crossed the edge's 150 ft (from {x0})", p.x());
                    assert!(v.x() > 0.0 && v.y() > 0.0, "the throw is not back in and up: {v:?}");
                    wrecked_at = Some(k);
                }
            }
            if ride.rest.iter().any(|c| matches!(c, Call::VehiclePutBack { .. })) {
                eprintln!("MD133 edge frame {k}: put back to {p:?}");
                put_at = Some(k);
                assert!(p.x() > 150.0, "put back out of bounds at {p:?}");
                break;
            }
        }
        let k = wrecked_at.expect("the run off the edge did not wreck the quad");
        let put = put_at.expect("the thrown quad was never put back");
        let (window, max) = (2.5 / dt.0, load.max_wreck_time.0 / dt.0);
        assert!((put - k) as f32 > window + max, "put back {} frames after the throw, inside the window and MaxWreckTime", put - k);
        assert!(!ride.down(), "put back but still down");
    });
}
