// ---------------------------------------------------------------------------------------------------
// S323 — the Sim's AI lane: `ai::Pilot::think` against the Sim's own world
// ---------------------------------------------------------------------------------------------------
//
// recording2's free09 image holds three AI riders mid-ride (S12e). Each one's brain, interface and card
// are read out of RAM into a `Pilot`, and free09 is loaded off the disc into the world a Sim rides in:
// its ground, its props and its plants. Then, rider by rider in the image's actor order and tick after
// tick, the original's `ai_update` runs in the interpreter on its own world, and the port's pilot
// thinks against the port's: its ground probes ask the port's ground, its scene walk is the port's
// broad phase round the bike's centre of gravity, and each sweep is the port's bundle against the
// port's prop. Nothing of the original is replaced: its ground query, scene walk and sweeps are only
// watched as they run, so that what each side asked and met can be compared. The bike is frozen
// between ticks, as in S12e: its physics is not run, so only the brains move on.

use super::sim::{
    S12E_BUDGET, S12E_DT, S12E_RIDERS, S12E_STATE, s12e_bike, s12e_f, s12e_pass_through, s12e_racer, s12e_seek_world,
    s12e_u64, s12e_vf,
};
use super::*;
use game_sim::ai::{Mount, Pilot, Seat, Thing};
use game_sim::archive::{Psr, Res};
use game_sim::contact_search::Heightfield;
use game_sim::eco::{Trees, Wooded};
use game_sim::flight_search::{self, FIRST};
use game_sim::ragdoll::Tables;
use game_sim::track::{self, Terrain};
use game_sim::world::{Ground, Place, Props};
use ps2_oracle::{Ee, Kit};
use std::cell::Cell;
use std::collections::HashMap;

const S323_TICKS: usize = 60;

/// free09 off the disc, as the Sim's world is made of it: the terrain and its layout (for the ground),
/// the props across both archives, and the plants on the image's curve table.
pub(crate) fn s323_free09(kit: &Kit, ee: &Ee) -> Option<(Vec<u8>, track::Scene, Props, Trees)> {
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut file) = std::fs::File::open(&at) else {
        skip(&format!("no {} — `./kit extract MX.PSR`", at.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    std::io::Read::read_exact(&mut file, &mut front).expect("the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let mut member = |name: &str| -> Option<Vec<u8>> {
        let e = psr.find(name)?;
        std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(e.at)).expect("seek");
        let mut bytes = vec![0u8; e.size as usize];
        std::io::Read::read_exact(&mut file, &mut bytes).expect("read");
        Some(bytes)
    };
    let bytes = member("Tracks\\free09.res")?;
    let res = Res::parse(&bytes).ok()?;
    let read = |suffix: &str| {
        let e = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(suffix)).unwrap_or_else(|| panic!("free09 has no {suffix}"));
        res.read(&bytes, e).expect("a free09 member")
    };
    let scene = track::Scene::parse(&read(".scn")).expect("free09's scene");
    let terrain_bytes = read(&scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn"));
    let terrain = Terrain::parse(&terrain_bytes).expect("free09's terrain");
    let ground = Ground::new(&terrain, &scene.terrain);
    let field = ground.field();

    let mut eco = game_sim::eco::Ecosystem::parse(&read(".esb")).expect("free09.esb parses");
    let layout = &scene.terrain;
    let extent = F::from(layout.width.max(layout.breadth) as i32 * game_sim::world::SAMPLES_PER_WIDTH as i32) * terrain.scale;
    eco.stand(extent, |p| F(game_sim::contact_search::ground(&field, p).point.y()));
    let table = ee.u32_at(addr("curve_table_ptr").unwrap());
    let entries: Vec<[f32; 2]> = (0..game_sim::camera::CURVE_ENTRIES as u32)
        .map(|i| [f32::from_bits(ee.u32_at(table + 8 * i)), f32::from_bits(ee.u32_at(table + 8 * i + 4))])
        .collect();
    let curve = CurveTable::from_entries(&entries);
    let (trees, missed) = Trees::new(&eco, game_sim::eco::cell_size(extent), &curve);
    assert!(missed.is_empty(), "free09's solids the port does not build: {missed:?}");

    let mut parts = vec![(bytes.clone(), res.clone())];
    if let Some(b) = member("Tracks\\free09_2.res")
        && let Ok(r) = Res::parse(&b)
    {
        parts.push((b, r));
    }
    let by_suffix = |res: &Res, want: &str| {
        let want = want.to_ascii_lowercase();
        res.entries.iter().find(|e| {
            let name = e.name.to_ascii_lowercase();
            name == want || name.ends_with(&format!("\\{want}"))
        }).cloned()
    };
    let (props, missing) = Props::load(&scene, |want| {
        parts
            .iter()
            .find_map(|(bytes, res)| by_suffix(res, want).and_then(|e| res.read(bytes, &e).ok()))
            .ok_or_else(|| "no archive of this track has it".to_string())
    });
    // free09's utility shack is a hierarchy, which the port's world has not got (S192's back search).
    let missing: Vec<_> = missing.into_iter().filter(|m| !m.contains("Hierarchy")).collect();
    assert!(missing.is_empty(), "the track's props the archives lack: {missing:?}");
    Some((terrain_bytes, scene, props, trees))
}

/// The port's world with every ground question it was asked kept: the point (bits), and the answer's
/// height and normal.
struct Watched<'a> {
    world: &'a mut dyn World,
    asked: Vec<([u32; 4], u32, [u32; 4])>,
}

impl World for Watched<'_> {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let (p, n) = self.world.ground(at);
        self.asked.push((at.to_bits(), p.y().to_bits(), n.to_bits()));
        (p, n)
    }
    fn solid(&mut self, bundle: &Bundle, record: &mut Record) -> bool {
        self.world.solid(bundle, record)
    }
    fn wooded(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>, Option<&Trees>) {
        self.world.wooded()
    }
}

/// A thing of either side's walk by what it is, not where it lives: a mesh by its place and reach, a
/// plant by its cell, and anything else of the original's by its class byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Seen {
    Mesh([u32; 4], u32),
    Plant(u16, u16),
    Other(u8),
}

/// What the original's walk and sweeps met in one rider's tick, and its ground queries.
#[derive(Default)]
struct Theirs {
    walked: Vec<(u32, Seen, Thing)>,
    sweeps: Vec<(u32, bool, Vec<bool>)>,
    ignore_bit: u32,
    feelers: u32,
    asked: Vec<([u32; 4], u32, [u32; 4])>,
}

/// **The Sim's AI lane thinks as the original's riders do, on free09's own world.** Three riders,
/// sixty ticks each, interleaved in the image's actor order. Per rider and tick, against the original's
/// `ai_update` run unhooked: every ground point asked and its answer (height and normal, from the port's
/// ground), every thing the scene walk handed out (the port's broad phase round the centre of gravity,
/// matched by place and reach or by cell), every solid swept with its answer and per-feeler hits (the
/// port's sweep of the bundle against the port's prop), then the tick's answer, the generator, the
/// flag words, the brain's copy of the bike, the four controls, the levers it hands off, the recovery
/// words, and the body's yaw rate and velocity the assist pulls.
#[test]
fn s323_the_lane_thinks_on_the_sims_own_world() {
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
    let Some(at) = named(&[
        "ai_update",
        "ground_query",
        "rand_state",
        "ai_iface_walk_next",
        "bundle_sweep_object",
        "class_bit",
        "world_manager",
    ]) else {
        return;
    };
    let [update, query, rand_state, walk_next, sweep, class_bit, world_manager] = at[..] else { unreachable!() };
    let Some((mut seek_world, lists)) = s12e_seek_world(&kit, &ee) else { return };
    let Some((terrain_bytes, scene, props, trees)) = s323_free09(&kit, &ee) else { return };
    let terrain = Terrain::parse(&terrain_bytes).expect("free09's terrain");
    let tables: Tables = controls::tables(&ee);
    assert_eq!(trees.cell().to_bits(), ee.u32_at(ee.u32_at(world_manager) + 0xd4), "the plants' cell is the world's");

    // The port's props by their place and reach, as the original's meshes are matched (S192).
    let field = {
        let g = Ground::new(&terrain, &scene.terrain);
        let f = g.field();
        ([F(f.min.x()), F(f.min.z())], [F(f.max.x()), F(f.max.z())])
    };
    let (broad, objects) = flight_search::world_flown(&props.props, Some(&trees), None, Some(field), Vf::new(0.0, 0.0, 0.0, 1.0), F(0.0));
    let mesh_of = |id: u32| {
        let i = id as usize;
        Seen::Mesh(broad.places[i].centre.map(F::to_bits), objects[i].radius.to_bits())
    };
    let ours_by_seen: HashMap<Seen, u32> = (FIRST..FIRST + props.props.len()).map(|i| (mesh_of(i as u32), i as u32)).collect();
    assert_eq!(ours_by_seen.len(), props.props.len(), "two props stand at one place");

    let theirs: Rc<RefCell<Theirs>> = Rc::default();
    {
        let asking: Rc<Cell<(u32, u32, [u32; 4])>> = Rc::default();
        let (a, a2, t) = (asking.clone(), asking.clone(), theirs.clone());
        s12e_pass_through(
            &mut ee,
            query,
            move |e| a.set((e.r(5) as u32, e.r(6) as u32, got(e, e.r(5) as u32))),
            move |e| {
                let (point, normal, asked) = a2.get();
                let n = if normal == 0 { [0; 4] } else { got(e, normal) };
                t.borrow_mut().asked.push((asked, e.u32_at(point + 4), n));
            },
        );
        let t = theirs.clone();
        s12e_pass_through(&mut ee, walk_next, |_| {}, move |e| {
            let o = e.v0() as u32;
            if o == 0 {
                return;
            }
            let mut t = t.borrow_mut();
            let class = e.read(o + 2, 1) as u8;
            let (seen, thing) = match class {
                2 => (
                    Seen::Mesh(got(e, o + 0x40), e.u32_at(o + 0x60)),
                    Thing::Solid { id: o, collidable: e.read(o + 0x31, 1) & 1 != 0, ai_ignore: e.u32_at(o + 0x84) & t.ignore_bit != 0 },
                ),
                3 => {
                    let (x, z) = (e.read(o + 8, 2) as u16, e.read(o + 0xc, 2) as u16);
                    (Seen::Plant(x, z), Thing::Group { x, z })
                }
                c => (Seen::Other(c), Thing::Other),
            };
            t.walked.push((o, seen, thing));
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

    // The port's side: each pilot read once, then carried.
    let filler = ai::spawn(&mut Rng::from_seed(0), 0, None, 2);
    let mut pilots: Vec<Pilot> = Vec::new();
    let mut bodies: Vec<(ai::Rider, Vf, F, Vf)> = Vec::new();
    let mut others: Vec<Vec<Other>> = Vec::new();
    for &(b, r) in &S12E_RIDERS {
        let racer = s12e_racer(&ee, &lists, b, r);
        let bike = s12e_bike(&ee, b, r);
        let ignore: Vec<u32> = (0..ee.u32_at(b + 0x150)).map(|k| ee.u32_at(ee.u32_at(b + 0x154) + 4 * k)).collect();
        println!("brain {b:#x}: bundle radius {}, ignore list {:x?}", s12e_f(&ee, ee.u32_at(b + 0x2a0) + 0x160), ignore);
        let ignore = ignore
            .iter()
            .filter_map(|&o| (ee.read(o + 2, 1) == 2).then(|| ours_by_seen.get(&Seen::Mesh(got(&ee, o + 0x40), ee.u32_at(o + 0x60)))).flatten().copied())
            .collect();
        pilots.push(Pilot {
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
            ignore,
        });
        bodies.push((bike.rider, s12e_vf(&ee, ee.u32_at(r + 0x7a4) + 0x60), bike.yaw_rate, bike.velocity));
        others.push(bike.others);
    }
    let ground = Ground::new(&terrain, &scene.terrain);
    let mut world = Wooded { world: Place { ground, props }, trees };
    let mut rng = Rng::from_state(s12e_u64(&ee, rand_state));
    let dt = F(f32::from_bits(S12E_DT));

    let (mut walked, mut swept, mut met, mut asked, mut ours_swept_n) = (0, 0, 0, 0, 0);
    let mut unmatched: HashMap<Seen, usize> = HashMap::new();
    for tick in 0..S323_TICKS {
        for (k, &(b, r)) in S12E_RIDERS.iter().enumerate() {
            let what = format!("tick {tick} rider {k} (brain {b:#x})");
            *theirs.borrow_mut() = Theirs { feelers: ee.u32_at(b + 0x2a4), ..Theirs::default() };
            ee.call(update, &[u64::from(b)], &[dt.to_f32()], S12E_BUDGET);
            let answer = ee.v0() as u32;
            let t = std::mem::take(&mut *theirs.borrow_mut());

            let (seen, cofg, yaw_rate, velocity) = &mut bodies[k];
            let mut watched = Watched { world: &mut world, asked: Vec::new() };
            let mount = Mount { rider: ai::Rider { velocity: *velocity, ..*seen }, cofg: *cofg, yaw_rate, velocity };
            let thought = pilots[k].think(mount, &mut watched, &mut seek_world, &tables, &others[k], None, &mut rng, dt);
            let pilot = &pilots[k];

            // The ground: every point asked, and the port's answer to it.
            assert_eq!(t.asked.len(), watched.asked.len(), "{what}: the ground queries");
            for (q, (theirs, ours)) in t.asked.iter().zip(&watched.asked).enumerate() {
                // The look-ahead point's w is the frame's, which the original's copy does not set.
                let n = if q == 0 { 3 } else { 4 };
                assert_eq!(theirs.0[..n], ours.0[..n], "{what}: ground query {q}'s point");
                // A query the original asks without a normal (a null pointer) is only its height.
                let normal = if theirs.2 == [0; 4] { [0; 4] } else { ours.2 };
                assert_eq!((theirs.1, theirs.2), (ours.1, normal), "{what}: ground query {q}'s answer at {:?}", Vf::from_bits(ours.0));
            }
            asked += t.asked.len();

            // The walk: the same things, in the same order.
            let theirs_seen: Vec<Seen> = t.walked.iter().map(|w| w.1).collect();
            let ours_seen: Vec<Seen> = thought
                .walked
                .iter()
                .map(|th| match *th {
                    Thing::Solid { id, .. } => mesh_of(id),
                    Thing::Group { x, z } => Seen::Plant(x, z),
                    Thing::Other => Seen::Other(0),
                })
                .collect();
            if theirs_seen != ours_seen {
                for s in theirs_seen.iter().filter(|s| !ours_seen.contains(s)) {
                    *unmatched.entry(*s).or_insert(0) += 1;
                }
                let extra: Vec<_> = ours_seen.iter().filter(|s| !theirs_seen.contains(s)).collect();
                let first = theirs_seen.iter().zip(&ours_seen).position(|(a, b)| a != b);
                assert!(
                    extra.is_empty() && theirs_seen.iter().filter(|s| ours_seen.contains(s)).eq(ours_seen.iter()),
                    "{what}: the walk: theirs {} things, ours {}; first apart at {first:?}; ours extra {extra:?}",
                    theirs_seen.len(),
                    ours_seen.len()
                );
            }
            walked += t.walked.len();

            // The sweeps: the same solids, each met or not alike, with the same feelers hit.
            let theirs_swept: Vec<(Seen, bool, Vec<bool>)> = t
                .sweeps
                .iter()
                .map(|(o, m, h)| {
                    let at = t.walked.iter().find(|w| w.0 == *o).map(|w| w.1).expect("a swept object the walk handed out");
                    (at, *m, h.clone())
                })
                .filter(|(s, _, _)| ours_by_seen.contains_key(s))
                .collect();
            let ours_swept: Vec<(Seen, bool, Vec<bool>)> =
                thought.swept.iter().map(|(id, hits)| (mesh_of(*id), hits.is_some(), hits.clone().unwrap_or_default())).collect();
            assert_eq!(theirs_swept.len(), ours_swept.len(), "{what}: the solids swept");
            for (i, (a, b)) in theirs_swept.iter().zip(&ours_swept).enumerate() {
                assert_eq!((a.0, a.1), (b.0, b.1), "{what}: sweep {i}");
                if a.1 {
                    assert_eq!(a.2, b.2, "{what}: sweep {i}'s feelers");
                }
            }
            // What the port's world has not got (riders, bikes, the shack's hierarchy) the original
            // swept too; none of it may have met, or the brains would part.
            let unknown_met = t.sweeps.iter().filter(|(o, m, _)| *m && !theirs_swept.iter().any(|s| t.walked.iter().any(|w| w.0 == *o && w.1 == s.0))).count();
            assert_eq!(unknown_met, 0, "{what}: the original met a solid the port's world has not got");
            swept += t.sweeps.len();
            ours_swept_n += ours_swept.len();
            met += ours_swept.iter().filter(|s| s.1).count();

            // The tick: what S12e's join compares, now on the port's own world.
            let told = &thought.told;
            assert_eq!(answer, u32::from(told.answer), "{what}: the answer");
            assert!(told.missing.is_empty() && told.rescue.is_none(), "{what}: {told:?}");
            assert_eq!(s12e_u64(&ee, rand_state), rng.state(), "{what}: the generator");
            let racer = &pilot.racer;
            assert_eq!(s12e_u64(&ee, b + 0x30), racer.brain.flags, "{what}: flags");
            assert_eq!(s12e_u64(&ee, b + 0x38), racer.moves, "{what}: second flags");
            let s = &racer.senses;
            let body = ee.u32_at(r + 0x7a4);
            for (name, at, ours) in [
                ("position", b + 0xc0, s.frame[3]),
                ("ground normal", b + 0x2c0, s.ground_normal),
                ("heading", b + 0x1d0, racer.seeker.heading),
                ("body velocity", body + 0x260, *velocity),
            ] {
                assert_eq!(got(&ee, at), ours.to_bits(), "{what}: {name}");
            }
            let br = &racer.brain;
            let words = [
                ("ground", b + 0x204, s.ground),
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
                ("lever reverse", r + 0x648, pilot.levers.reverse),
                ("clutch", r + 0x644, racer.habits.clutch),
                ("back-up", r + 0x14c0, pilot.recovery.back_up),
                ("swerve bar", r + 0x14c4, pilot.recovery.swerve_bar),
                ("swerve", r + 0x14c8, pilot.recovery.swerve),
                ("yaw rate", body + 0x284, *yaw_rate),
            ];
            let off: Vec<String> = words
                .into_iter()
                .filter(|&(_, at, ours)| ee.u32_at(at) != ours.to_bits())
                .map(|(name, at, ours)| format!("{name}: theirs {} ours {}", ee.f32_at(at), ours.to_f32()))
                .collect();
            assert!(off.is_empty(), "{what}:\n{}", off.join("\n"));
            // The bike's controls are the levers handed off, and the habits' clutch.
            let c = thought.controls;
            assert_eq!(
                [c.gas, c.clutch, c.brake, c.reverse, c.stick.0, c.stick.1].map(F::to_bits),
                [pilot.levers.gas, racer.habits.clutch, pilot.levers.brake, pilot.levers.reverse, pilot.levers.steer, pilot.levers.lean].map(F::to_bits),
                "{what}: the controls"
            );
        }
    }
    // Near recording2's riders the static tree is empty: every thing walked and swept is a moving
    // one, which the port's world has not got. The fixed walk is the next test's.
    assert!(asked > 0, "the riders asked no ground");
    println!(
        "{} rider ticks: {asked} ground queries, {walked} things walked, {swept} solids swept ({ours_swept_n} of the port's, {met} met); \
         the original's walked things the port's world has not got: {unmatched:?}",
        S323_TICKS * 3
    );
}

/// **The lane's scene walk is the original's over the fixed world.** The original's walk is a kind-3
/// query: the static tree inside the box round the bike's centre of gravity, then the moving
/// structure, which hands out every moving thing (riders, bikes, their parts) wherever it is. Near
/// recording2's riders the static tree holds nothing, so the stepped test above cannot tell a right
/// static walk from an empty one. Here the first rider's centre of gravity is put on each of free09's
/// props in turn, and the original's walk is called through the interface (begin, next until none,
/// end). Its fixed things (the props the port's world holds, matched by place and reach, and the plant
/// groups by cell) must be the port's `WorldScene` walk at that centre, in order.
#[test]
fn s323_the_lanes_walk_is_the_originals_on_the_fixed_world() {
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
    let [begin, next, end] = at[..] else { unreachable!() };
    let Some((terrain_bytes, scene, props, trees)) = s323_free09(&kit, &ee) else { return };
    let terrain = Terrain::parse(&terrain_bytes).expect("free09's terrain");
    let field = {
        let g = Ground::new(&terrain, &scene.terrain);
        let f = g.field();
        ([F(f.min.x()), F(f.min.z())], [F(f.max.x()), F(f.max.z())])
    };
    let (broad, objects) = flight_search::world_flown(&props.props, Some(&trees), None, Some(field), Vf::new(0.0, 0.0, 0.0, 1.0), F(0.0));
    let mesh_of = |id: u32| Seen::Mesh(broad.places[id as usize].centre.map(F::to_bits), objects[id as usize].radius.to_bits());
    let ours_by_seen: HashMap<Seen, u32> = (FIRST..FIRST + props.props.len()).map(|i| (mesh_of(i as u32), i as u32)).collect();
    let centres: Vec<Vf> = (FIRST..FIRST + props.props.len())
        .map(|i| {
            let [x, y, z, _] = broad.places[i].centre.map(F::to_f32);
            Vf::new(x, y, z, 1.0)
        })
        .collect();

    let r = S12E_RIDERS[0].1;
    let iface = r + 0x10d0;
    let body = ee.u32_at(r + 0x7a4);
    let ground = Ground::new(&terrain, &scene.terrain);
    let mut world = Wooded { world: Place { ground, props }, trees };
    let (mut solids, mut plants) = (0, 0);
    for (i, centre) in centres.iter().enumerate() {
        let cofg = *centre;
        for (k, b) in cofg.to_bits().into_iter().enumerate() {
            ee.set_u32(body + 0x60 + 4 * k as u32, b);
        }
        ee.call(begin, &[u64::from(iface)], &[], S12E_BUDGET);
        assert_ne!(ee.v0(), 0, "prop {i}: the walk opens");
        let mut theirs = Vec::new();
        loop {
            ee.call(next, &[u64::from(iface)], &[], S12E_BUDGET);
            let o = ee.v0() as u32;
            if o == 0 {
                break;
            }
            let seen = match ee.u8_at(o + 2) {
                2 => Seen::Mesh(got(&ee, o + 0x40), ee.u32_at(o + 0x60)),
                3 => Seen::Plant(ee.read(o + 8, 2) as u16, ee.read(o + 0xc, 2) as u16),
                c => Seen::Other(c),
            };
            if matches!(seen, Seen::Plant(..)) || ours_by_seen.contains_key(&seen) {
                theirs.push(seen);
            }
        }
        ee.call(end, &[u64::from(iface)], &[], S12E_BUDGET);

        let lent = ai::Lent::new(&mut world);
        let mut ws = ai::WorldScene { world: &lent, cofg, centre: cofg, radius: F(100.0), walked: Vec::new(), swept: Vec::new() };
        let ours: Vec<Seen> = ai::Scene::walk(&mut ws)
            .expect("the port's walk opens")
            .iter()
            .map(|th| match *th {
                Thing::Solid { id, .. } => mesh_of(id),
                Thing::Group { x, z } => Seen::Plant(x, z),
                Thing::Other => Seen::Other(0),
            })
            .collect();
        assert_eq!(theirs, ours, "prop {i} at {cofg:?}: the fixed things walked");
        solids += ours.iter().filter(|s| matches!(s, Seen::Mesh(..))).count();
        plants += ours.iter().filter(|s| matches!(s, Seen::Plant(..))).count();
    }
    assert!(solids >= centres.len(), "each prop's own walk hands it out");
    println!("{} walks, one on each prop: {solids} props and {plants} plant groups, alike", centres.len());
}
