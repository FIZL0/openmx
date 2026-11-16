// ---------------------------------------------------------------------------------------------------
// S326 — the moving world in the AI lane's scene: the riders' bikes walked, and passed over
// ---------------------------------------------------------------------------------------------------
//
// The original's kind-3 scene walk hands out, after the static tree's things, everything its moving
// structure holds, wherever it is: each rider's bike frame (class 0, a mesh: `bikef.col`), its wheel
// rays (class 2), the rider's sphere and the ragdoll's parts (classes 3 and 5), and the map's other
// moving things. The avoidance skips any object of the AI_IGNORE class, and every bike collidable
// carries that bit (the bike builder ORs it in), so no feeler ever stops on a bike. `ai::WorldScene`
// walks the Sim's riders' bike frames (`ai::Ridden`) flagged so.
//
// recording2's image (S12e) holds four bikes, three of them AI riders' and all within a few feet. The
// first test reads each bike frame's matrix out of RAM and proves the port's sphere of it is the
// object's own. The second puts another rider's bike frame across the first AI rider's way, at a grid
// of distances and offsets along its heading, and runs one rider tick on each side from the same
// image: the original's `ai_update` in the interpreter, the port's `Pilot::think_among` with the four
// bikes. The walk must hand out the bikes as AI_IGNORE on both sides, neither may sweep one, and the
// tick that follows must be alike. Then the tick's own bundle is swept at each bike through the
// original's per-object sweep and the port's scene: alike, met or not, feeler by feeler.

use super::lane::s323_free09;
use super::sim::{S12E_BUDGET, S12E_DT, S12E_RIDERS, S12E_STATE, s12e_bike, s12e_pass_through, s12e_racer, s12e_seek_world, s12e_u64, s12e_vf};
use super::*;
use game_render::disc::Disc;
use game_sim::ai::{Mount, Pilot, Ridden, Seat, Thing};
use game_sim::bike_col::{self, BikeCol};
use game_sim::eco::Wooded;
use game_sim::math::Mat4;
use game_sim::ragdoll::Tables;
use game_sim::track::Terrain;
use game_sim::world::{Ground, Place};
use ps2_oracle::{Ee, Kit};
use std::cell::Cell;
use std::sync::Arc;

/// The riding bike frame's category (`flight_search::BIKE`): what tells a bike in the walk.
const BIKE: u32 = game_sim::flight_search::BIKE;

/// The bike's two collision files, off the first of the disc's archives that carries them.
fn bike_col() -> Option<Arc<BikeCol>> {
    let mut disc = Disc::open_configured().map_err(|w| skip(&w)).ok()?;
    let res = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let read = |name: String| res.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
    let col = BikeCol::load(&read(bike_col::rays_file(bike_col::MODEL)), &read(bike_col::shape_file(bike_col::MODEL)))
        .expect("the bike's collision files");
    Some(Arc::new(col))
}

/// The walk of rider `r`'s interface, through the original's begin/next/end: every object it hands
/// out.
fn walk(ee: &mut Ee, at: &[u32; 3], r: u32) -> Vec<u32> {
    let iface = r + 0x10d0;
    ee.call(at[0], &[u64::from(iface)], &[], S12E_BUDGET);
    assert_ne!(ee.v0(), 0, "the walk opens");
    let mut out = Vec::new();
    loop {
        ee.call(at[1], &[u64::from(iface)], &[], S12E_BUDGET);
        match ee.v0() as u32 {
            0 => break,
            o => out.push(o),
        }
    }
    ee.call(at[2], &[u64::from(iface)], &[], S12E_BUDGET);
    out
}

/// A walked object that is a riding bike's frame: a mesh (class 0) of the bike's category.
fn is_bike(ee: &Ee, o: u32) -> bool {
    ee.u8_at(o + 2) == 2 && ee.u32_at(o + 0x68) == 0 && ee.u32_at(o + 0x84) == BIKE
}

/// A collision object's matrix (`+0x120`), as the port stands a bike's frame by its chassis'.
fn matrix(ee: &Ee, o: u32) -> Mat4 {
    Mat4([0, 1, 2, 3].map(|k| Vf(got(ee, o + 0x120 + 0x10 * k).map(f32::from_bits))))
}

/// **The port's sphere of a riding bike's frame is the object's own.** At recording2's image, every
/// bike frame the first AI rider's walk hands out: collidable, its category none of `AI_IGNORE`'s,
/// and `ai::Ridden::sphere` of its matrix (the frame mesh's root box's centre through it, and the
/// mesh's reach) is its `+0x40` and `+0x60`, bit for bit.
#[test]
fn s326_a_bikes_sphere_is_the_objects() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let path = kit.ctx("states").join(S12E_STATE);
    if !path.exists() {
        return skip(&format!("{} is not there: `./kit state ram` of recording2.p2m2_SaveState.p2s", path.display()));
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let Some(at) = named(&["ai_iface_walk_begin", "ai_iface_walk_next", "ai_iface_walk_end"]) else { return };
    let Some(col) = bike_col() else { return };
    let at = [at[0], at[1], at[2]];
    let bikes: Vec<u32> = walk(&mut ee, &at, S12E_RIDERS[0].1).into_iter().filter(|&o| is_bike(&ee, o)).collect();
    assert_eq!(bikes.len(), 4, "recording2's four riders' bikes");
    for &o in &bikes {
        assert_eq!(ee.u8_at(o + 0x31) & 1, 1, "bike {o:#x} is collidable");
        let ridden = Ridden { col: col.clone(), at: matrix(&ee, o) };
        let (centre, reach) = ridden.sphere();
        assert_eq!(got(&ee, o + 0x40), centre.to_bits(), "bike {o:#x}'s centre");
        assert_eq!(ee.u32_at(o + 0x60), reach.to_bits(), "bike {o:#x}'s reach");
    }
}

/// What the original's tick swept: each object, whether it met, and its feelers' hits.
#[derive(Default)]
struct Swept {
    walked: Vec<u32>,
    sweeps: Vec<(u32, bool, Vec<bool>)>,
    ignore_bit: u32,
    feelers: u32,
}

/// **An AI rider's feelers meet another rider's bike as the original's do.** From recording2's image,
/// the second AI rider's bike frame is stood across the first's way — its matrix moved along the
/// first rider's heading by 6 to 110 feet and across it by up to 20 — and one tick is run on each
/// side. The original's `ai_update` sweeps whatever its walk hands out; the port's pilot thinks among
/// the four bikes (their matrices read from RAM). Per placement: the bikes walked are the original's
/// (by their spheres), each bike sweep is the original's (met or not, and which feelers hit), no
/// sweep at anything else met, and the tick after — the answer, the generator, both flag words, the
/// steer, speed, throttle and brake, the target speed, the levers and the assist's yaw rate and
/// velocity — is the original's. Some placements must meet, and some must not.
#[test]
fn s326_the_feelers_meet_another_riders_bike() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let path = kit.ctx("states").join(S12E_STATE);
    if !path.exists() {
        return skip(&format!("{} is not there: `./kit state ram` of recording2.p2m2_SaveState.p2s", path.display()));
    }
    let ram = std::fs::read(&path).expect("the image");
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let Some(at) = named(&[
        "ai_update",
        "rand_state",
        "ai_iface_walk_begin",
        "ai_iface_walk_next",
        "ai_iface_walk_end",
        "bundle_sweep_object",
        "class_bit",
    ]) else {
        return;
    };
    let [update, rand_state, begin, next, end, sweep, class_bit] = at[..] else { unreachable!() };
    let Some(col) = bike_col() else { return };
    let Some((terrain_bytes, scene, props, trees)) = s323_free09(&kit, &ee) else { return };
    let terrain = Terrain::parse(&terrain_bytes).expect("free09's terrain");
    let tables: Tables = controls::tables(&ee);
    let dt = F(f32::from_bits(S12E_DT));
    let ground = Ground::new(&terrain, &scene.terrain);
    let mut world = Wooded { world: Place { ground, props }, trees };

    let (b, r) = S12E_RIDERS[0];
    let bikes: Vec<u32> = walk(&mut ee, &[begin, next, end], r).into_iter().filter(|&o| is_bike(&ee, o)).collect();
    assert_eq!(bikes.len(), 4, "recording2's four riders' bikes");
    // The bike to move: the second AI rider's, the frame nearest its body.
    let body1 = s12e_vf(&ee, ee.u32_at(S12E_RIDERS[1].1 + 0x7a4) + 0x60);
    let near = |o: u32| {
        let c = Vf(got(&ee, o + 0x40).map(f32::from_bits));
        let d = [c.x() - body1.x(), c.z() - body1.z()];
        d[0] * d[0] + d[1] * d[1]
    };
    let moved = *bikes.iter().min_by(|&&a, &&c| near(a).total_cmp(&near(c))).unwrap();
    let position = s12e_vf(&ee, b + 0xc0);
    let heading = s12e_vf(&ee, b + 0x1d0);
    let flat = (heading.x() * heading.x() + heading.z() * heading.z()).sqrt();
    let (fx, fz) = (heading.x() / flat, heading.z() / flat);
    println!("rider 0 at {position:?}, heading ({fx}, {fz}); moving bike {moved:#x}; feeler length {}", ee.f32_at(ee.u32_at(b + 0x2a0) + 0x160));

    let theirs: Rc<RefCell<Swept>> = Rc::default();
    {
        let t = theirs.clone();
        s12e_pass_through(&mut ee, next, |_| {}, move |e| {
            if e.v0() != 0 {
                t.borrow_mut().walked.push(e.v0() as u32);
            }
        });
        let t = theirs.clone();
        s12e_pass_through(&mut ee, class_bit, |_| {}, move |e| t.borrow_mut().ignore_bit = e.v0() as u32);
        let (t, swept) = (theirs.clone(), Rc::new(Cell::new((0u32, 0u32))));
        let s2 = swept.clone();
        s12e_pass_through(&mut ee, sweep, move |e| s2.set((e.r(4) as u32, e.r(5) as u32)), move |e| {
            let (bundle, o) = swept.get();
            let mut t = t.borrow_mut();
            let hits = e.u32_at(e.u32_at(bundle + 0x6c) + 0xa4);
            let hits = (0..t.feelers).map(|k| e.u32_at(hits + 4 * k) != 0).collect();
            t.sweeps.push((o, e.v0() != 0, hits));
        });
    }

    let filler = ai::spawn(&mut Rng::from_seed(0), 0, None, 2);
    let (mut placements, mut meeting, mut bike_sweeps, mut met_sweeps, mut walked_bikes, mut swept_others) = (0, 0, 0, 0, 0, 0);
    for along in [6.0f32, 10.0, 15.0, 20.0, 30.0, 45.0, 60.0, 80.0, 95.0, 110.0] {
        for across in [-20.0f32, -10.0, -5.0, -2.0, 0.0, 2.0, 5.0, 10.0, 20.0] {
            let what = format!("bike {along} ft ahead, {across} ft across");
            ee.load(0, &ram);
            ee.set(28, u64::from(addr("boot_gp").unwrap()));
            // Stand the bike there: its matrix's translation moved, and its sphere's centre as the
            // engine refreshes it from the matrix (the port's, proved on the image's bikes above).
            let m = matrix(&ee, moved);
            let t = m.0[3];
            let to = Vf::new(position.x() + fx * along - fz * across, t.y(), position.z() + fz * along + fx * across, t.w());
            let m = Mat4([m.0[0], m.0[1], m.0[2], to]);
            for (k, v) in to.to_bits().into_iter().enumerate() {
                ee.set_u32(moved + 0x150 + 4 * k as u32, v);
            }
            let (centre, _) = Ridden { col: col.clone(), at: m }.sphere();
            for (k, v) in centre.to_bits().into_iter().enumerate() {
                ee.set_u32(moved + 0x40 + 4 * k as u32, v);
            }

            // The port's rider, read off the same image, and the four bikes where they now stand.
            let Some((mut seek_world, lists)) = s12e_seek_world(&kit, &ee) else { return };
            let racer = s12e_racer(&ee, &lists, b, r);
            let bike = s12e_bike(&ee, b, r);
            let mut pilot = Pilot {
                racer,
                character: filler.character,
                wander: filler.wander,
                levers: bike.levers,
                recovery: bike.recovery,
                card: bike.card,
                seat: Seat {
                    speed_scale: bike.speed_scale,
                    slow_radius: bike.slow_radius,
                    slow_speed: bike.slow_speed,
                    player: bike.player,
                    air_cone: bike.air_cone,
                    look_ahead: bike.rider.look_ahead,
                    bump_chance: bike.rider.bump_chance,
                    race_phase: bike.race_phase,
                    path_net: bike.path_net,
                    live: bike.live,
                },
                ignore: Vec::new(),
            };
            let ridden: Vec<Ridden> = bikes.iter().map(|&o| Ridden { col: col.clone(), at: matrix(&ee, o) }).collect();
            let (mut yaw_rate, mut velocity) = (bike.yaw_rate, bike.velocity);
            let cofg = s12e_vf(&ee, ee.u32_at(r + 0x7a4) + 0x60);
            let mut rng = Rng::from_state(s12e_u64(&ee, rand_state));

            *theirs.borrow_mut() = Swept { feelers: ee.u32_at(b + 0x2a4), ..Swept::default() };
            ee.call(update, &[u64::from(b)], &[dt.to_f32()], S12E_BUDGET);
            let answer = ee.v0() as u32;
            let t = std::mem::take(&mut *theirs.borrow_mut());

            let mount = Mount { rider: ai::Rider { velocity, ..bike.rider }, cofg, yaw_rate: &mut yaw_rate, velocity: &mut velocity };
            let thought = pilot.think_among(mount, &mut world, &ridden, &mut seek_world, &tables, &bike.others, None, None, &mut rng, dt);

            // The walk: the four bikes, collidable and of the AI_IGNORE class, as the port's moving
            // world; and so neither side sweeps one.
            let theirs_bikes: Vec<u32> = t.walked.iter().copied().filter(|&o| is_bike(&ee, o)).collect();
            assert_eq!(theirs_bikes, bikes, "{what}: the bikes walked");
            assert_eq!(t.ignore_bit, bike_col::RIDE_START[0], "{what}: AI_IGNORE's bit is the ride start's word");
            for &o in &theirs_bikes {
                assert!(ee.u8_at(o + 0x31) & 1 == 1 && ee.u32_at(o + 0x84) & t.ignore_bit != 0, "{what}: bike {o:#x} is AI_IGNORE");
            }
            let ours_moving: Vec<Thing> = thought.walked.iter().copied().filter(|th| matches!(th, Thing::Solid { id, .. } if *id >= ai::MOVING)).collect();
            assert_eq!(
                ours_moving,
                (0..4).map(|k| Thing::Solid { id: ai::MOVING + k, collidable: true, ai_ignore: true }).collect::<Vec<_>>(),
                "{what}: the port's moving world"
            );
            assert!(!t.sweeps.iter().any(|(o, _, _)| bikes.contains(o)), "{what}: the original swept a bike");
            assert!(!thought.swept.iter().any(|(id, _)| *id >= ai::MOVING), "{what}: the port swept a bike");
            let other_met = t.sweeps.iter().filter(|(_, m, _)| *m).count();
            assert_eq!(other_met, 0, "{what}: the original met something");
            assert!(thought.swept.iter().all(|(_, h)| h.is_none()), "{what}: the port met a prop");
            walked_bikes += theirs_bikes.len();
            swept_others += t.sweeps.len();

            // The sweep the avoidance would make, had the bikes not been AI_IGNORE: the tick's own
            // bundle (its feelers as the original left them) at each bike, through the original's
            // per-object sweep and the port's scene. The brain's bundle keeps its probes' lengths,
            // origins and directions in the arrays at +0x134, +0x138 and +0x13c.
            let bundle = ee.u32_at(b + 0x2a0);
            let n = ee.u32_at(bundle + 0x120);
            assert_eq!(n, t.feelers, "{what}: the bundle's feelers");
            let (origins, dirs, lengths) = (ee.u32_at(bundle + 0x138), ee.u32_at(bundle + 0x13c), ee.u32_at(bundle + 0x134));
            let fan: Vec<ai::Feeler> = (0..n)
                .map(|k| ai::Feeler {
                    origin: Vf(got(&ee, origins + 0x10 * k).map(f32::from_bits)),
                    direction: Vf(got(&ee, dirs + 0x10 * k).map(f32::from_bits)),
                    length: F(ee.f32_at(lengths + 4 * k)),
                })
                .collect();
            let lent = ai::Lent::new(&mut world);
            let mut scene = ai::WorldScene {
                world: &lent,
                ridden: &ridden,
                cofg,
                centre: s12e_vf(&ee, bundle + 0x150),
                radius: F(ee.f32_at(bundle + 0x160)),
                walked: Vec::new(),
                swept: Vec::new(),
            };
            for (k, &o) in bikes.iter().enumerate() {
                *theirs.borrow_mut() = Swept { feelers: n, ..Swept::default() };
                ee.call(sweep, &[u64::from(bundle), u64::from(o), u64::from(ee.u32_at(bundle + 0x6c))], &[], S12E_BUDGET);
                let (_, met, hits) = theirs.borrow_mut().sweeps.pop().expect("the sweep ran");
                let ours = ai::Scene::sweep(&mut scene, ai::MOVING + k as u32, &fan);
                assert_eq!(ours.is_some(), met, "{what}: bike {k}'s sweep met: theirs {hits:?}; fan {fan:?}; centre {:?} radius {}; arrays {origins:#x} {dirs:#x} {lengths:#x}; bike {:?}", s12e_vf(&ee, bundle + 0x150), ee.f32_at(bundle + 0x160), ridden[k].sphere());
                if met {
                    assert_eq!(ours.as_deref(), Some(&hits[..]), "{what}: bike {k}'s feelers");
                    if o == moved {
                        meeting += 1;
                    }
                    met_sweeps += 1;
                }
                bike_sweeps += 1;
            }

            // The tick that follows.
            let told = &thought.told;
            assert_eq!(answer, u32::from(told.answer), "{what}: the answer");
            assert_eq!(s12e_u64(&ee, rand_state), rng.state(), "{what}: the generator");
            let racer = &pilot.racer;
            assert_eq!(s12e_u64(&ee, b + 0x30), racer.brain.flags, "{what}: flags");
            assert_eq!(s12e_u64(&ee, b + 0x38), racer.moves, "{what}: second flags");
            let body = ee.u32_at(r + 0x7a4);
            assert_eq!(got(&ee, body + 0x260), velocity.to_bits(), "{what}: body velocity");
            let br = &racer.brain;
            let words = [
                ("speed", b + 0x80, br.speed),
                ("steer", b + 0xf0, br.steer),
                ("pitch", b + 0xf4, br.pitch),
                ("throttle", b + 0xe0, br.throttle),
                ("brake", b + 0xe4, br.brake),
                ("target speed", b + 0x1e0, racer.seeker.speed),
                ("lever steer", r + 0x628, pilot.levers.steer),
                ("lever lean", r + 0x62c, pilot.levers.lean),
                ("lever gas", r + 0x63c, pilot.levers.gas),
                ("lever brake", r + 0x640, pilot.levers.brake),
                ("back-up", r + 0x14c0, pilot.recovery.back_up),
                ("swerve bar", r + 0x14c4, pilot.recovery.swerve_bar),
                ("swerve", r + 0x14c8, pilot.recovery.swerve),
                ("yaw rate", body + 0x284, yaw_rate),
            ];
            let off: Vec<String> = words
                .into_iter()
                .filter(|&(_, at, ours)| ee.u32_at(at) != ours.to_bits())
                .map(|(name, at, ours)| format!("{name}: theirs {} ours {}", ee.f32_at(at), ours.to_f32()))
                .collect();
            assert!(off.is_empty(), "{what}:\n{}", off.join("\n"));

            placements += 1;
        }
    }
    println!(
        "{placements} placements: {walked_bikes} bikes walked and none swept ({swept_others} other sweeps, none met), the ticks alike; \
         the bundle at each bike: {bike_sweeps} sweeps, {met_sweeps} met ({meeting} on the moved bike), alike"
    );
    assert!(meeting > 0, "no placement put the moved bike on a feeler");
    assert!(meeting < placements, "every placement met the moved bike");
}

