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
