//! The biplane's moving parts move in the shell (V38): the wing flaps with the ailerons, the rear
//! flaps with the elevator, the rudder and the prop, in the shell's own draw path and both renderers.
//!
//! The chain from the stick to the picture, one link a proof:
//! - the stick and L1/R1 into the plane's control state — S34's, the flight's own tests;
//! - the control state into the surface step's angles — S35's `tests/biplane_anim.rs`, against the
//!   human's biplane recording;
//! - the angles into the model's node matrices — [`the_pose_is_the_originals_bones`] here, against
//!   the bone matrices the same recording left the original's model with;
//! - the stick, held each way on the disc's plane flown through the shell's own garage, into the
//!   nodes the original moves for it, and those nodes into a drawn picture that changes, GPU and
//!   software alike — [`holding_the_stick_moves_the_drawn_planes_surfaces`].
//!
//! `src/vehicle.rs` and `src/plane_draw.rs` are compiled in by path, the way `tests/machines.rs`
//! takes the first.

#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;
#[path = "../src/plane_draw.rs"]
mod plane_draw;

use game_render::biplane::{self, Biplane};
use game_render::mat::{M4, v3};
use game_render::raster::Frame;
use game_render::world::{Assets, Camera, Lighting};
use game_render::{Disc, gpu};
use game_sim::Sim;
use game_sim::biplane_anim::{self, FLAPS, PROP, REAR, RUDDER};
use game_sim::collide::{Bundle, Record};
use game_sim::input::{AXIS_CENTRE, Control, PADS, Report};
use game_sim::world::World;
use ps2_float::{F, Vf};
use vehicle::{Garage, Kind, Plane, Spawn};

const DT: F = F(1.0 / 60.0);

/// Flat ground at height 0, nothing solid on it.
struct Flat;

impl World for Flat {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// The plane's model, off the disc's machines' archive as the shell loads it, and its `.phy`.
fn the_discs_plane() -> Option<(Assets, Biplane, game_sim::phy::Plane)> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            eprintln!("skipped: {why}");
            return None;
        }
    };
    let res = disc.res("machine.res").expect("the machines' archive");
    let phy = game_sim::phy::Plane::read_bytes(&res.read_suffix(game_sim::phy::BIPLANE_PHY).expect("biplane.phy"));
    let mut assets = Assets::default();
    let drawn = Biplane::load(&mut assets, &[res], 0).expect("the biplane's model");
    Some((assets, drawn, phy))
}

fn node(assets: &Assets, drawn: &Biplane, name: &str) -> usize {
    let model = &assets.models[&drawn.model].model;
    model.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name)).unwrap_or_else(|| panic!("no {name} on the model"))
}

fn apart(a: M4, b: M4) -> f32 {
    (0..4).flat_map(|i| (0..4).map(move |j| (i, j))).map(|(i, j)| (a.0[i][j] - b.0[i][j]).abs()).fold(0.0, f32::max)
}

/// One pass of the shell's pad into the garage, as `tests/ride.rs`'s `pass`.
fn pass(sim: &mut Sim, garage: &mut Garage, report: Report) {
    let mut reports = [Report::missing(); PADS];
    reports[0] = report;
    let controls = sim.poll(DT, &reports);
    garage.step(sim, DT, &controls, &mut Flat);
}

/// Every row of the human's biplane recording (S35's `pilot.csv`): the surface step's angles off the
/// row's controls, posed into the disc's model by `game_render::biplane::pose`, against the bone
/// matrices the original's model was left with on that row. Same tolerance as S35's own test: the
/// wing flaps turn about the hinge the plane keeps, which this stands in for with the rest matrix's
/// first row.
#[test]
fn the_pose_is_the_originals_bones() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/capture/biplane/pilot.csv");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipped: no {} (s2 has it; journal 2026-10-20-s35-s35-the-biplane-moves)", path.display());
        return;
    };
    let Some((assets, drawn, _)) = the_discs_plane() else { return };
    let model = &assets.models[&drawn.model].model;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap().split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name).unwrap_or_else(|| panic!("no {name} column"));
    let rows: Vec<Vec<f32>> = lines.map(|l| l.split(',').map(|v| v.parse().unwrap_or(f32::NAN)).collect()).collect();
    let mat = |r: &[f32], name: &str| {
        let mut m = M4::IDENTITY;
        for (i, row) in m.0.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = r[col(&format!("{name}.m{i}{j}"))];
            }
        }
        m
    };
    let bones = [("TopLeftFlap", "bone_top_left", 2e-3), ("TopRightFlap", "bone_top_right", 2e-3), ("RearLeftFlap", "bone_rear_left", 1e-5), ("Rudder", "bone_rudder", 1e-5)];
    let (mut checked, mut moved) = (0, [0usize; 4]);
    for i in 1..rows.len() {
        let (r, was) = (&rows[i], &rows[i - 1]);
        // As S35's test: the step sees the crash the tick before left it, unless this tick reset it.
        let crashed = was[col("crash")] != 0.0 && r[col("crash")] != 0.0;
        let s = biplane_anim::surfaces(F(r[col("pitch_cmd")]), F(r[col("max_elevator")]), F(r[col("aileron")]), F(r[col("rudder")]), F(r[col("prop_angle")]), crashed);
        let pose = biplane::pose(model, &s);
        for (k, (name, bone, tolerance)) in bones.iter().enumerate() {
            let at = node(&assets, &drawn, name);
            let want = mat(r, bone);
            assert!(apart(pose[at], want) < *tolerance, "row {i}: {name} is {:?}, the original's {:?}", pose[at].0, want.0);
            moved[k] += usize::from(apart(model.world(at), want) > 1e-2);
        }
        if s.prop.is_some() {
            let at = node(&assets, &drawn, PROP);
            assert!(apart(pose[at], mat(r, "bone_prop")) < 1e-5, "row {i}: the prop");
        }
        checked += 1;
    }
    eprintln!("{checked} rows, off their rest on {moved:?} of them");
    assert!(checked > 1000, "only {checked} rows");
    // The recording does move all four, so the check is not of a plane standing still.
    assert!(moved.iter().all(|&m| m > 50), "the recording hardly moved them: {moved:?}");
}

/// The disc's plane, flown level through the shell's garage on gas, then held each way for a third of
/// a second: stick left and right, up and down, L1 and R1. The plane is posed the way the shell draws
/// it ([`plane_draw::actors`] for the GPU, [`plane_draw::skinned`] for the software path), and:
/// - each hold moves the nodes the original's surface step moves for it — the four wing flaps for the
///   stick's x, left pair against right; the rear flaps for its y; the rudder for L1/R1 — each way
///   opposite to the other, and nothing else on the model but the prop;
/// - the GPU's actor is posed by exactly that pose, and the software path's copy is skinned with it;
/// - each renderer's picture of the plane held that way differs from its picture of the same plane
///   with its surfaces centred, and the prop alone left as it was.
#[test]
fn holding_the_stick_moves_the_drawn_planes_surfaces() {
    let Some((mut assets, drawn, phy)) = the_discs_plane() else { return };
    let flight = game_sim::flight::Flight::new(&phy);
    let lowest = flight.tires.iter().map(|t| t.center.y() - t.radius.0).fold(f32::MAX, f32::min);
    let spawn = Spawn { at: Vf::new(400.0, -lowest, -300.0, 1.0), facing: Vf::new(0.0, 0.0, -1.0, 0.0) };
    let mut sim = Sim::new(0);
    let mut garage = Garage::new(Kind::Biplane, Some(Plane::flying(spawn, flight))).unwrap();
    let gas = Report::from_buttons(Control::Cross.mask());
    for _ in 0..30 * 60 {
        pass(&mut sim, &mut garage, gas);
    }
    assert!(!garage.plane.as_ref().unwrap().on_ground(), "thirty seconds of gas and the plane is not up");

    let stick = |x: u8, y: u8| {
        let mut r = gas;
        r.set_axis(2, x);
        r.set_axis(3, y);
        r
    };
    let buttons = |c: Control| Report::from_buttons(Control::Cross.mask() | c.mask());
    let holds = [
        ("stick left", stick(0x00, AXIS_CENTRE)),
        ("stick right", stick(0xff, AXIS_CENTRE)),
        ("stick up", stick(AXIS_CENTRE, 0x00)),
        ("stick down", stick(AXIS_CENTRE, 0xff)),
        ("L1", buttons(Control::L1)),
        ("R1", buttons(Control::R1)),
    ];
    let model = assets.models[&drawn.model].model.clone();
    let rest: Vec<M4> = (0..model.nodes.len()).map(|i| model.world(i)).collect();
    let flaps = FLAPS.map(|n| node(&assets, &drawn, n));
    let rear = REAR.map(|n| node(&assets, &drawn, n));
    let rudder = node(&assets, &drawn, RUDDER);
    let prop = node(&assets, &drawn, PROP);
    // How far a node has turned off its rest, signed by the turn's sense about `axis` in the node's
    // parent's frame: one of the rest's rows across the axis (`y`, or `z` for a turn about y) taken
    // through the pose, projected on `axis × row`.
    let turn = |pose: &[M4], i: usize, axis: [f32; 3]| -> f32 {
        let row = if axis[1].abs() > 0.9 { 2 } else { 1 };
        let (a, b) = (rest[i].0[row], pose[i].0[row]);
        let cross = [axis[1] * a[2] - axis[2] * a[1], axis[2] * a[0] - axis[0] * a[2], axis[0] * a[1] - axis[1] * a[0]];
        (0..3).map(|k| (b[k] - a[k]) * cross[k]).sum()
    };
    let hinge = |i: usize| {
        let r = rest[i].0[0];
        let n = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
        [r[0] / n, r[1] / n, r[2] / n]
    };

    let mut gpu = match gpu::Renderer::headless() {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("GPU skipped: {e}");
            None
        }
    };
    let scene = game_render::scene::Scene::parse("[Fog]\nColorRGB=61,68,76\nMinimumFogDistance=5000.00\nMaximumFogDistance=9000.00\n");
    let light = Lighting::of(&scene);
    if let Some(g) = gpu.as_mut() {
        g.set_scene(&scene);
        g.set_sky(None);
    }
    // Behind and above the plane, which is drawn at its own space's origin, nose along +z: both wings,
    // the tail and the rudder in the picture.
    let camera = Camera { eye: v3(0.0, 16.0, -38.0), at: v3(0.0, 4.0, -4.0), fov: 0.9, near: 0.5, far: 500.0 };
    let (w, h) = (320usize, 240usize);
    let mut pictures = |plane: &Plane, assets: &mut Assets| -> (Vec<u8>, Option<Vec<u8>>) {
        let at = M4::IDENTITY;
        // The software path: the plane's own copy, skinned, drawn as `draw_scene` draws an actor.
        let parts = plane_draw::skinned(&drawn, "biplane@test", assets, plane, at);
        let mut frame = Frame::new(w, h, [0, 0, 0]);
        let vp = camera.view_projection(w, h);
        game_render::world::draw_model(&mut frame, &assets.models[parts[0].0], assets, at, vp, camera.eye, &light);
        let soft = frame.color.clone();
        let hard = gpu.as_mut().map(|g| {
            let mut poses = Vec::new();
            let actors = plane_draw::actors(&drawn, assets, plane, at, 1.0, &mut poses);
            let actors = &actors[..1];
            let texture = g.offscreen(w as u32, h as u32);
            let view = texture.create_view(&Default::default());
            let views = [gpu::View { rect: [0, 0, w as u32, h as u32], camera: &camera, actors }];
            let target = gpu::Target { view: &view, size: (w as u32, h as u32), frame: [0, 0, w as u32, h as u32], scale: 1 };
            g.render(&target, assets, &gpu::Scene3 { views: &views, poses: &poses, overlay: &[], sprites: &[], light: &light });
            g.read(&texture)
        });
        (soft, hard)
    };
    let differ = |a: &[u8], b: &[u8]| a.chunks(3).zip(b.chunks(3)).filter(|(p, q)| p.iter().zip(q.iter()).any(|(&x, &y)| x.abs_diff(y) > 24)).count();

    let mut turns = Vec::new();
    for (name, report) in holds {
        let (mut sim, mut garage) = (sim.clone(), garage.clone());
        for _ in 0..20 {
            pass(&mut sim, &mut garage, report);
        }
        let plane = garage.plane.clone().unwrap();
        let surfaces = plane.surfaces();
        let pose = drawn.pose(&assets, &surfaces);
        // The GPU's actor names this pose and the software path's copy is skinned with it.
        let mut poses = Vec::new();
        let actors = plane_draw::actors(&drawn, &assets, &plane, M4::IDENTITY, 1.0, &mut poses);
        assert_eq!((actors[0].model, actors[0].pose), (drawn.model.as_str(), Some(0)), "{name}: the GPU's plane is not posed");
        assert_eq!(poses[0], pose, "{name}: the GPU's pose");
        plane_draw::skinned(&drawn, "biplane@test", &mut assets, &plane, M4::IDENTITY);
        let skinned = &assets.models["biplane@test"].tris;
        let want = game_render::skin::skin(&model, &pose, 0);
        assert_eq!(skinned.len(), want.len(), "{name}: the software path's triangles");
        for (a, b) in skinned.iter().zip(&want) {
            assert!(a.v.iter().zip(&b.v).all(|(p, q)| p.pos == q.pos), "{name}: the software path's triangles");
        }
        // Nothing but the surfaces and the prop moves.
        for (i, n) in model.nodes.iter().enumerate() {
            if !flaps.contains(&i) && !rear.contains(&i) && i != rudder && i != prop {
                assert!(apart(pose[i], rest[i]) < 1e-6, "{name}: {} moved", n.name);
            }
        }
        let t = (
            flaps.map(|i| turn(&pose, i, hinge(i))),
            rear.map(|i| turn(&pose, i, [1.0, 0.0, 0.0])),
            turn(&pose, rudder, [0.0, 1.0, 0.0]),
        );
        eprintln!("{name}: aileron {:.3} elevator {:.3} rudder {:.3}; flaps {:?} rear {:?} rudder {:.3}", surfaces.flaps[0].0, -surfaces.rear.0, -surfaces.rudder.0, t.0, t.1, t.2);

        // The picture: the plane as it is held, against the same plane with its surfaces centred and
        // its prop where it was.
        let mut centred = plane.clone();
        let f = centred.flight.as_mut().unwrap();
        (f.surfaces.aileron, f.surfaces.elevator, f.surfaces.rudder) = (F::ZERO, F::ZERO, F::ZERO);
        assert_eq!(centred.surfaces().prop, surfaces.prop);
        let (soft, hard) = pictures(&plane, &mut assets);
        let (soft_centred, hard_centred) = pictures(&centred, &mut assets);
        let lit = soft.chunks(3).filter(|p| p.iter().any(|&c| c > 0)).count();
        assert!(lit > 2000, "{name}: only {lit} pixels of plane in the software picture");
        let soft_moved = differ(&soft, &soft_centred);
        let hard_moved = hard.as_ref().zip(hard_centred.as_ref()).map(|(a, b)| differ(a, b));
        if let Ok(dir) = std::env::var("GPU_TEST_PNG") {
            let dir = std::path::Path::new(&dir);
            let file = name.replace(' ', "_");
            let _ = game_render::png::write_rgb(&dir.join(format!("v38_{file}_soft.png")), w, h, &soft);
            if let Some(hard) = &hard {
                let _ = game_render::png::write_rgb(&dir.join(format!("v38_{file}_gpu.png")), w, h, hard);
            }
        }
        eprintln!("{name}: {soft_moved} software pixels and {hard_moved:?} GPU pixels off the centred plane's");
        assert!(soft_moved > 40, "{name}: the software picture hardly moved ({soft_moved} pixels)");
        if let Some(moved) = hard_moved {
            assert!(moved > 40, "{name}: the GPU picture hardly moved ({moved} pixels)");
        }
        turns.push((name, t));
    }

    // Each hold moves the original's nodes for it, and the opposite hold the other way.
    let [left, right, up, down, l1, r1] = [0, 1, 2, 3, 4, 5].map(|i| turns[i].1);
    const MOVED: f32 = 0.02;
    // The stick's x: the left pair one way and the right pair the other (the surface step's +aileron
    // and -aileron), and stick right all four the other way.
    for k in 0..4 {
        assert!(left.0[k].abs() > MOVED && right.0[k].abs() > MOVED, "the stick's x leaves {} still", FLAPS[k]);
        assert!(left.0[k].signum() != right.0[k].signum(), "{} turns the same way for stick left and right", FLAPS[k]);
    }
    assert!(left.0[0].signum() == left.0[1].signum() && left.0[2].signum() == left.0[3].signum(), "a wing's two flaps part");
    // The stick's y: both rear flaps, together, one way up and the other down.
    for k in 0..2 {
        assert!(up.1[k].abs() > MOVED && down.1[k].abs() > MOVED, "the stick's y leaves {} still", REAR[k]);
        assert!(up.1[k].signum() != down.1[k].signum(), "{} turns the same way for stick up and down", REAR[k]);
    }
    // L1 and R1: the rudder, one way and the other.
    assert!(l1.2.abs() > MOVED && r1.2.abs() > MOVED, "L1/R1 leave the rudder still: {} {}", l1.2, r1.2);
    assert!(l1.2.signum() != r1.2.signum(), "the rudder turns the same way for L1 and R1");
}

/// A stick step on the disc's plane in flight, drawn four frames a tick the way the shell draws
/// between ticks (V79, [`plane_draw::shown`]): the ailerons, the elevator and the rudder thrown full
/// at once, and every drawn frame moves each surface at most a quarter of what the control law lets
/// it move in a tick — no frame steps a whole tick's turn — while the frames on a tick are that tick's
/// own pose and the surfaces get all the way over.
#[test]
fn a_stick_step_glides_between_the_drawn_frames() {
    let Some((_, _, phy)) = the_discs_plane() else { return };
    let flight = game_sim::flight::Flight::new(&phy);
    let j = flight.joystick;
    let lowest = flight.tires.iter().map(|t| t.center.y() - t.radius.0).fold(f32::MAX, f32::min);
    let spawn = Spawn { at: Vf::new(400.0, -lowest, -300.0, 1.0), facing: Vf::new(0.0, 0.0, -1.0, 0.0) };
    let mut sim = Sim::new(0);
    let mut garage = Garage::new(Kind::Biplane, Some(Plane::flying(spawn, flight))).unwrap();
    let gas = Report::from_buttons(Control::Cross.mask());
    for _ in 0..30 * 60 {
        pass(&mut sim, &mut garage, gas);
    }
    let mut thrown = Report::from_buttons(Control::Cross.mask() | Control::R1.mask());
    thrown.set_axis(2, 0x00);
    thrown.set_axis(3, 0x00);
    let tick = [(j.aileron_rate * DT).0, (j.elevator_rate * DT * j.max_elevator).0, (j.rudder_rate * DT).0];
    let angles = |s: &biplane_anim::Surfaces| [s.flaps[0].0, s.rear.0, s.rudder.0];
    let mut was = garage.plane.as_ref().unwrap().surfaces();
    let start = angles(&was);
    let mut drawn_before = angles(&was);
    let mut frames = 0;
    for _ in 0..20 {
        pass(&mut sim, &mut garage, thrown);
        let plane = garage.plane.as_ref().unwrap();
        for k in 1..=4 {
            let shown = plane_draw::shown(Some(&was), plane, k as f32 / 4.0);
            let now = angles(&shown);
            for (s, ((a, b), step)) in ["aileron", "elevator", "rudder"].iter().zip(drawn_before.iter().zip(now).zip(tick)) {
                assert!((b - a).abs() <= step / 4.0 * 1.001 + 1e-6, "frame {frames}: the {s} jumps {a} -> {b}, a tick is {step}");
            }
            if k == 4 {
                assert_eq!(shown, plane.surfaces(), "the frame on the tick is the tick's pose");
            }
            drawn_before = now;
            frames += 1;
        }
        was = plane.surfaces();
    }
    let end = angles(&was);
    assert!((end[0] - start[0]).abs() > 0.5, "the ailerons go over: {start:?} -> {end:?}");
    assert!((end[1] - start[1]).abs() > 0.2, "the elevator goes over: {start:?} -> {end:?}");
    assert!((end[2] - start[2]).abs() > 0.3, "the rudder goes over: {start:?} -> {end:?}");
    // And with no tick before it (the frame cut, the plane just out) the frame is the tick's.
    assert_eq!(plane_draw::shown(None, garage.plane.as_ref().unwrap(), 0.25), was);
}
