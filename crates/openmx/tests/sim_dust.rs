//! S420: a race's bike emitters in its Sim — the track's `mxparticles.dat` as `game_sim::dust` runs it
//! (particles.rs's `sim_rider`), and the shell's emitters placing particles on the frames the Sim's
//! drew (`PlaneFrame::sim_draws`).
//!
//! The disc is not in the repo: without it the list test says so and passes.

#[allow(dead_code)]
#[path = "../src/particles.rs"]
mod particles;

use game_render::Disc;
use game_render::biplane;
use game_render::ini;
use game_render::mat::{M4, v3};
use game_sim::dust;
use particles::{Atlas, PlaneFrame, PlaneFx};
use ps2_float::{F, Vf};

/// A track's bike particles off the disc: its list (`mxparticles.dat`, or the AI's
/// `mxaiparticles.dat`), the `.emt`s and zones beside it, and its sheet.
fn disc_fx(track: &str, list: &str) -> Option<PlaneFx> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            eprintln!("skipped: {why}");
            return None;
        }
    };
    let names = disc.entries().iter().map(|e| e.name.clone()).collect::<Vec<_>>();
    let member = |end: &str| names.iter().filter(|n| n.to_ascii_lowercase().ends_with(end)).min_by_key(|n| n.len()).cloned().unwrap();
    let archives = [disc.res(&member(&format!("{track}.res"))).unwrap(), disc.res(&member(&format!("{track}_2.res"))).unwrap()];
    let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
    let texture = game_render::pst::Texture::parse(&read(&particles::sheet_texture(track)).unwrap()).unwrap();
    let atlas = Atlas::parse(&ini::text(&read(&particles::sheet(track)).unwrap()), texture.width, texture.height);
    let list = biplane::emitters(&ini::text(&read(list).unwrap()));
    Some(PlaneFx::new(&list, atlas, read))
}

/// The bike at `at`, level, facing +z: the Sim's transform (rows right, up, forward, place).
fn bike_at(at: [f32; 3]) -> game_sim::math::Mat4 {
    game_sim::math::Mat4([Vf([1.0, 0.0, 0.0, 0.0]), Vf([0.0, 1.0, 0.0, 0.0]), Vf([0.0, 0.0, 1.0, 0.0]), Vf([at[0], at[1], at[2], 1.0])])
}

/// free09's list, both the player's and the AI's, as the Sim runs it: C33's riders' manager entries
/// (research/oracle.txt § S328c), the two tyre tracks being the Sim's tracks — two collision models,
/// the rider's dust and chunks and the foot dust (MinimumEmissionSpeed 30), one exhaust (MES 0: the
/// YZ250 has no `250_Exhaust_Particle02`, so the list's left exhaust is not built), the rear tyre's
/// dust (MES 5), and the sprays, front, rear, front, rear, rear (MES 3, 3, 3, 3, 1), the two chunk
/// sprays on the other emit path.
#[test]
fn free09s_list_is_c33s_entries() {
    for list in [particles::BIKE_PARTICLES, "mxaiparticles.dat"] {
        let Some(fx) = disc_fx("free09", list) else { return };
        let bike = bike_at([10.0, 2.0, -40.0]);
        let world = |name: &str| (!name.eq_ignore_ascii_case("250_Exhaust_Particle02")).then_some(Vf([11.0, 3.0, -42.0, 1.0]));
        let rider = particles::sim_rider(&fx, world, &bike);
        assert_eq!(rider.entries.len(), fx.entries.len(), "{list}: one Sim entry an entry");
        let built: Vec<(usize, &dust::Entry)> = rider.entries.iter().enumerate().filter(|(_, e)| e.kind != dust::Kind::Quiet).collect();
        let kinds: Vec<String> = built
            .iter()
            .map(|(_, e)| match e.kind {
                dust::Kind::Collision { .. } => "collision".to_string(),
                dust::Kind::Spheres { mask, .. } => format!("spheres {mask:#x}"),
                dust::Kind::Exhaust { max_speed } => format!("exhaust {}", max_speed.0),
                dust::Kind::Generic => "generic".to_string(),
                dust::Kind::Spray { threshold, .. } => format!("spray {} tyre {:?}", threshold.0, e.tyre),
                dust::Kind::Quiet => unreachable!(),
            })
            .collect();
        assert_eq!(
            kinds,
            [
                "collision",
                "collision",
                "spheres 0x1fc",
                "spheres 0x1fc",
                "spheres 0x3",
                "exhaust 15",
                "generic",
                "spray 8 tyre Some(0)",
                "spray 5 tyre Some(1)",
                "spray 4 tyre Some(0)",
                "spray 5 tyre Some(1)",
                "spray 3 tyre Some(1)",
            ],
            "{list}"
        );
        let mes: Vec<f32> = built.iter().map(|(_, e)| e.emitter.min_speed.0).collect();
        assert_eq!(mes, [3.0, 15.0, 3.0, 15.0, 30.0, 0.0, 5.0, 3.0, 3.0, 3.0, 3.0, 1.0], "{list}: MinimumEmissionSpeed");
        let vu: Vec<usize> = built.iter().enumerate().filter(|(_, (i, _))| fx.entries[*i].file.vu).map(|(k, _)| k).collect();
        assert_eq!(vu, [9, 10], "{list}: the other emit path");
        let disable: Vec<u8> = built.iter().map(|(_, e)| e.disable).collect();
        assert_eq!(disable, [0x15, 0x15, 0x15, 0x15, 0xd, 8, 0xd, 0xd, 0xd, 0xd, 0xd, 0xd], "{list}: Disable");
        for (i, e) in &built {
            assert!(!e.live && !e.emitter.on, "{list} {i}: built off");
            assert_eq!(e.emitter.cull, Some(dust::Cull { far: F(800.0), directional: F(200.0) }), "{list} {i}: the constructor's cull");
            if fx.entries[*i].node.is_some() {
                assert_eq!(e.emitter.place, dust::Place::Chassis(Vf([1.0, 1.0, -2.0, 1.0])), "{list} {i}: its node in the chassis' frame");
                assert_eq!(e.emitter.pos.0, [11.0, 3.0, -42.0, 1.0], "{list} {i}: starts on its node");
            }
        }
    }
}

/// A one-entry list: a generic emitter on `Pipe` with a 20 ft/s gate.
fn gated() -> PlaneFx {
    let list = biplane::emitters("[Smoke]\nType = Generic\nDisable = Wrecking\nAttachedToSoultree = Pipe\nEmitter = smoke.emt\n");
    let emt = "[General Info]\nParticleGroupName=\"Smoke\"\nMinimumEmitRate=60\nMaximumEmitRate=60\nMinimumEmissionSpeed=20\nMinParticlesPerFrame=1\n";
    let read = |name: &str| (name == "smoke.emt").then(|| emt.as_bytes().to_vec());
    let sheet = "[General Info]\nNumFrames=1\nNumGroups=1\n[Group0]\nName=\"Smoke\"\nFirstFrame=0\nLastFrame=0\n\
                 [Frame0]\nUVLeft=0\nUVRight=8\nUVTop=0\nUVBottom=8\n";
    PlaneFx::new(&list, Atlas::parse(sheet, 8, 8), read)
}

/// `frames` frames of the pipe going `speed` ft/s along x, the Sim's draws for the entry `draws`
/// (`None`: no Sim); what is in the air after.
fn run(speed: f32, draws: Option<u8>, frames: usize) -> usize {
    const DT: f32 = 1.0 / 30.0;
    let mut fx = gated();
    for f in 0..frames {
        let mut m = M4::IDENTITY;
        m.0[3] = [speed * DT * f as f32, 5.0, 0.0, 1.0];
        let frame = PlaneFrame { nodes: vec![("pipe".to_string(), m)], sim_draws: draws.map(|d| vec![d]), vel: v3(speed, 0.0, 0.0), ..PlaneFrame::default() };
        fx.step(&frame, DT);
    }
    fx.particles.len()
}

/// The shell's emitter places particles on the frames the Sim's twin drew, whatever its own speed
/// gate says, and none on the frames it did not; with no Sim its own gate holds.
#[test]
fn the_sim_draws_open_the_gate() {
    assert_eq!(run(5.0, None, 10), 0, "own gate: 5 ft/s is under 20");
    assert!(run(40.0, None, 10) > 0, "own gate: 40 ft/s is over 20");
    assert!(run(5.0, Some(2), 10) > 0, "the Sim drew: it emits under its own gate");
    assert_eq!(run(40.0, Some(0), 10), 0, "the Sim drew nothing: it emits nothing over its own gate");
}

/// A built rider runs in the Sim's frame: the generic on a still bike switches on, takes its point
/// and draws nothing; moving it past the gate draws twice a frame.
#[test]
fn a_built_rider_draws_when_its_emitter_moves() {
    let fx = gated();
    let mut rider = particles::sim_rider(&fx, |_| Some(Vf([0.0, 5.0, 0.0, 1.0])), &bike_at([0.0, 0.0, 0.0]));
    let curve = game_sim::camera::CurveTable::build();
    let dt = F(1.0 / 30.0);
    let mut draws = Vec::new();
    for f in 0..4 {
        let x = if f < 2 { 0.0 } else { f as f32 * 2.0 };
        draws.push(rider.frame(dust::Feed::default(), &bike_at([x, 0.0, 0.0]), &curve, dt));
    }
    assert_eq!(draws, [0, 0, 2, 2], "switched on at 0, still at 1, 60 ft/s at 2 and 3");
}

/// A psg node: `name` under `parent`, `at` from it.
fn node(name: &str, parent: Option<usize>, at: [f32; 3]) -> game_render::psg::Node {
    let mut local = M4::IDENTITY;
    local.0[3] = [at[0], at[1], at[2], 1.0];
    game_render::psg::Node { name: name.to_string(), local, parent, center: v3(0.0, 0.0, 0.0), half_extents: v3(0.0, 0.0, 0.0), radius: 0.0 }
}

fn model(nodes: Vec<game_render::psg::Node>) -> game_render::psg::Model {
    game_render::psg::Model { nodes, materials: Vec::new(), meshes: Vec::new(), groups: 0, group_distance: Vec::new() }
}

/// A list on the rig's three nodes and one on a node of the frame: a rear and a front generic, an
/// exhaust, and a generic fixed to `Seat`.
fn rigged() -> PlaneFx {
    let list = biplane::emitters(
        "[Rear]\nType = Generic\nAttachedToSoultree = RearTire_Particle01\nEmitter = smoke.emt\n\
         [Front]\nType = Generic\nAttachedToSoultree = FrontTire_Particle01\nEmitter = smoke.emt\n\
         [Pipe]\nType = Exhaust\nMaxSpeed = 15\nAttachedToSoultree = 250_Exhaust_Particle01\nEmitter = smoke.emt\n\
         [Seat]\nType = Generic\nAttachedToSoultree = Seat\nEmitter = smoke.emt\n",
    );
    let emt = "[General Info]\nParticleGroupName=\"Smoke\"\nMinimumEmitRate=60\nMaximumEmitRate=60\nMinimumEmissionSpeed=1\nMinParticlesPerFrame=1\n";
    let read = |name: &str| (name == "smoke.emt").then(|| emt.as_bytes().to_vec());
    let sheet = "[General Info]\nNumFrames=1\nNumGroups=1\n[Group0]\nName=\"Smoke\"\nFirstFrame=0\nLastFrame=0\n\
                 [Frame0]\nUVLeft=0\nUVRight=8\nUVTop=0\nUVBottom=8\n";
    PlaneFx::new(&list, Atlas::parse(sheet, 8, 8), read)
}

/// S420a: the tyres' and the exhaust's emitters hang on the Sim's node rigs off the bike's model:
/// each starts on its node's point, the rear one moves with the rear compression and the front one
/// with the fork's, and an emitter on a frame node stays fixed in the chassis.
#[test]
fn the_tyres_and_the_pipe_ride_the_rigs() {
    let fx = rigged();
    assert_eq!(fx.entries.len(), 4, "every entry built");
    let chassis = model(vec![
        node("Bike", None, [0.0, 0.0, 0.0]),
        node("SwingArm", Some(0), [0.0, 1.5, -0.5]),
        node("RearTire_Particle01", Some(1), [0.0, -0.4, -2.0]),
        node("Handlebars", Some(0), [0.0, 3.0, 1.5]),
        node("Shocks", Some(3), [0.0, -1.0, 0.6]),
        node("FrontTire_Particle01", Some(4), [0.0, -1.2, 0.4]),
        node("Seat", Some(0), [0.0, 2.5, -0.3]),
    ]);
    let pipe = model(vec![node("Pipe", None, [9.0, 9.0, 9.0]), node("250_Exhaust_Particle01", Some(0), [0.3, 1.8, -1.9])]);
    let bike = bike_at([10.0, 2.0, -40.0]);
    let world = |name: &str| {
        let i = chassis.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name))?;
        let p = chassis.world(i).point(v3(0.0, 0.0, 0.0));
        Some(Vf([p.x + 10.0, p.y + 2.0, p.z - 40.0, 1.0]))
    };
    let mut rider = particles::sim_rider(&fx, |n| world(n).or(Some(Vf([10.3, 3.8, -41.9, 1.0]))), &bike);
    let axis = game_sim::tire::steering_axis(Vf::new(0.0, 3.0, 1.5, 1.0), Vf::new(0.0, 2.0, 2.1, 1.0));
    particles::sim_rig(&mut rider, &fx, &chassis, &[&pipe], Vf::new(0.0, 0.8, -2.4, 1.0), axis, &bike);
    let places: Vec<dust::Place> = rider.entries.iter().map(|e| e.emitter.place).collect();
    assert_eq!(&places[..3], [dust::Place::Rear, dust::Place::Front, dust::Place::Exhaust]);
    assert!(matches!(places[3], dust::Place::Chassis(_)), "the seat's stays fixed");
    let rear = rider.rear.expect("a rear rig");
    let front = rider.front.expect("a front rig");
    let exhaust = rider.exhaust.expect("an exhaust rig");
    assert_eq!(rear.pivot.0, [0.0, 1.5, -0.5, 1.0], "SwingArm's place");
    assert_eq!(rear.mount.0, [0.0, 0.8, -2.4, 1.0], "the Sim's mount");
    assert_eq!(front.rest.0, [0.0, 2.0, 2.1, 1.0], "Shocks in the Bike frame");
    let near = |a: Vf, b: [f32; 3]| (0..3).all(|k| (a.0[k] - b[k]).abs() < 1e-4);
    assert!(near(front.point(&bike), [10.0, 2.0 + 0.8, -40.0 + 2.5]), "the front node at the bind: {:?}", front.point(&bike));
    assert!(near(exhaust.point(), [10.3, 3.8, -41.9]), "the pipe's node under Bike's world, its root's own local dropped: {:?}", exhaust.point());
    assert_eq!(rider.entries[1].emitter.pos.0, front.point(&bike).0, "starts on its node");
    let (low, high) = (rear.point(&bike, F(0.0)), rear.point(&bike, F(0.5)));
    assert!((high.0[1] - low.0[1]).abs() > 0.05, "the rear node swings with the compression: {low:?} {high:?}");
    // The fork slides up its axis with the front compression, on the next drive.
    let mut slid = front;
    let curve = game_sim::camera::CurveTable::build();
    slid.drive(&bike, &dust::Feed { front_compression: F(0.5), ..dust::Feed::default() }, &curve);
    assert!(slid.point(&bike).0[1] > front.point(&bike).0[1] + 0.3, "compressed, the front node rides up");
}

/// A one-entry list: a collision model with a 20 ft/s gate.
fn struck() -> PlaneFx {
    let list = biplane::emitters("[Dust]\nType = CollisionModel\nEmitter = smoke.emt\n");
    let emt = "[General Info]\nParticleGroupName=\"Smoke\"\nMinimumEmitRate=60\nMaximumEmitRate=60\nMinimumEmissionSpeed=20\nMinParticlesPerFrame=1\n";
    let read = |name: &str| (name == "smoke.emt").then(|| emt.as_bytes().to_vec());
    let sheet = "[General Info]\nNumFrames=1\nNumGroups=1\n[Group0]\nName=\"Smoke\"\nFirstFrame=0\nLastFrame=0\n\
                 [Frame0]\nUVLeft=0\nUVRight=8\nUVTop=0\nUVBottom=8\n";
    PlaneFx::new(&list, Atlas::parse(sheet, 8, 8), read)
}

/// S420a: a collision model's particles go to the contacts its Sim twin drew at, whatever their own
/// speed gate says, and to no other.
#[test]
fn the_sim_hits_pick_the_contacts() {
    const DT: f32 = 1.0 / 30.0;
    let run = |hits: Option<u32>| {
        let mut fx = struck();
        for _ in 0..6 {
            let frame = PlaneFrame {
                ground_contacts: vec![v3(0.0, 0.0, 0.0), v3(100.0, 0.0, 0.0)],
                ground_zones: vec![0, 0],
                contact_vel: Some(v3(1.0, 0.0, 0.0)),
                sim_hits: hits.map(|h| vec![h]),
                ..PlaneFrame::default()
            };
            fx.step(&frame, DT);
        }
        let near = fx.particles.iter().filter(|p| p.at.x < 50.0).count();
        (near, fx.particles.len() - near)
    };
    assert_eq!(run(None), (0, 0), "own gate: 1 ft/s is under 20");
    let (near, far) = run(Some(0b10));
    assert!(near == 0 && far > 0, "the Sim drew at contact 1 only: {near} {far}");
    assert_eq!(run(Some(0)), (0, 0), "the Sim drew at none");
}
