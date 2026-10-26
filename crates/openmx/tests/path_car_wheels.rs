//! V82: the path cars' wheels turn. Every track's `.toy` car whose model has the original's wheel
//! nodes (`toy::Wheels`: free06's dump trucks and scrapers, the trailers and trucks elsewhere) is
//! posed through one second of the track's clock in 30 steps, the way the shell draws it: the palette
//! the GPU renderer is handed as the car's pose, and the triangles the software renderer skins out of
//! that palette (whole once, then at the wheels only). Over the second each wheel turns about its axle
//! by the flat distance the car moved — summed out of where `Traffic::placed` stood it, step by step —
//! over the wheel's radius, backwards for a car going forwards; and a steered front wheel turns into
//! the bend no further than 30°.
//!
//! The disc is not in the repo: without it the test says so and passes.

use game_render::Disc;
use game_render::disc::Res;
use game_render::mat::{M4, V3, v3};
use game_render::toy::{self, Traffic, Wheels};
use game_render::world::Assets;

/// One second of the track's clock, in the scene's steps.
const STEPS: usize = 30;
/// Where the second starts: well past the cars' warm-up from the load.
const FROM: f64 = 40.0;

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            eprintln!("skipped: {why}");
            None
        }
    }
}

fn row(m: &M4, r: usize) -> V3 {
    v3(m.0[r][0], m.0[r][1], m.0[r][2])
}

/// `v`, a direction in the model's space, in the frame of the node placed at `parent`.
fn into(v: V3, parent: Option<&M4>) -> V3 {
    match parent {
        Some(p) => v3(v.dot(row(p, 0).norm()), v.dot(row(p, 1).norm()), v.dot(row(p, 2).norm())),
        None => v,
    }
}

/// A wheel's spin and steer as its palette has it, in its parent's frame: the spin out of its forward
/// and up, the steer out of its axle, which the spin does not move.
fn spin_steer(palette: &[M4], node: usize, parent: Option<usize>) -> (f32, f32) {
    let p = parent.map(|p| &palette[p]);
    let axle = into(row(&palette[node], 0), p).norm();
    let up = into(row(&palette[node], 1), p).norm();
    let forward = into(row(&palette[node], 2), p).norm();
    (forward.y.atan2(up.y), (-axle.z).atan2(axle.x))
}

/// The smallest turn from `a` to `b`.
fn turn(a: f32, b: f32) -> f32 {
    let d = (b - a).rem_euclid(std::f32::consts::TAU);
    if d > std::f32::consts::PI { d - std::f32::consts::TAU } else { d }
}

/// The track's archives, its second's too.
fn archives(disc: &mut Disc, member: &str) -> Vec<Res> {
    let mut out = vec![disc.res(member).expect("the track's archive")];
    let second = member.to_ascii_lowercase().replace(".res", "_2.res");
    if let Some(e) = disc.entries().iter().find(|e| e.name.to_ascii_lowercase() == second) {
        let name = e.name.clone();
        out.push(disc.res(&name).expect("the track's second archive"));
    }
    out
}

#[test]
fn every_path_cars_wheels_roll_its_distance() {
    let Some(mut disc) = disc() else { return };
    let tracks: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("tracks\\") && n.ends_with(".res") && !n.ends_with("_2.res")
        })
        .collect();
    let mut rolled = Vec::new();
    for member in tracks {
        let stem = member.rsplit('\\').next().unwrap().trim_end_matches(".res").trim_end_matches(".RES").to_ascii_lowercase();
        let archives = archives(&mut disc, &member);
        let mut flat_ground = |p: V3| (p, v3(0.0, 1.0, 0.0));
        let traffic = Traffic::load(&archives, &stem, &mut flat_ground);
        let mut assets = Assets::default();
        for (k, (car, path)) in traffic.cars.iter().enumerate() {
            if !assets.load_placed(&archives, car.stem()) {
                continue;
            }
            let loaded = &assets.models[car.stem()];
            let model = &loaded.model;
            let Some(wheels) = Wheels::new(model, car, path) else { continue };
            let what = format!("{stem} car {k} ({})", car.stem());
            // Where the shell stands the car, and every wheel's angle in both renderers, every step.
            let mut moved = 0.0f64;
            let mut was: Option<V3> = None;
            let mut spun = vec![0.0f32; wheels.nodes.len()];
            let mut skinned = vec![0.0f32; wheels.nodes.len()];
            let mut before: Option<(Vec<M4>, Vec<f32>)> = None;
            let mut rims: Vec<usize> = Vec::new();
            let mut tris = Vec::new();
            for step in 0..=STEPS {
                let elapsed = FROM + step as f64 * f64::from(toy::SCENE_STEP);
                let at = row(&traffic.placed(elapsed, &mut flat_ground)[k].1, 3);
                if let Some(was) = was {
                    moved += f64::from(v3(at.x - was.x, 0.0, at.z - was.z).len());
                }
                was = Some(at);
                let palette = wheels.palette_at(model, car, path, elapsed);
                // The software renderer's wheel: its triangles moved at the wheels only, as the shell
                // does from the second picture on, which are the whole model skinned afresh; and one
                // vertex of it, the one furthest off the axle at the first step, about the axle in
                // the parent's frame.
                wheels.reskin(model, &palette, &mut tris);
                let fresh = game_render::skin::skin(model, &palette, 0);
                assert_eq!(tris.len(), fresh.len(), "{what}: triangles");
                for (a, b) in tris.iter().zip(&fresh) {
                    for (a, b) in a.v.iter().zip(&b.v) {
                        assert!(a.pos.sub(b.pos).len() < 1e-4 && a.normal.sub(b.normal).len() < 1e-4, "{what}: a moved wheel is not the skinned one");
                    }
                }
                let verts: Vec<&game_render::psg::Vertex> = tris.iter().flat_map(|t| t.v.iter()).collect();
                let off = |w: usize, i: usize| {
                    let n = wheels.nodes[w];
                    into(verts[i].pos.sub(row(&palette[n], 3)), model.nodes[n].parent.map(|p| &palette[p]))
                };
                if rims.is_empty() {
                    rims = (0..wheels.nodes.len())
                        .map(|w| {
                            (0..verts.len())
                                .filter(|&i| verts[i].node == wheels.nodes[w])
                                .max_by(|&a, &b| {
                                    let (a, b) = (off(w, a), off(w, b));
                                    a.y.hypot(a.z).total_cmp(&b.y.hypot(b.z))
                                })
                                .unwrap_or_else(|| panic!("{what}: wheel {w}'s node draws nothing"))
                        })
                        .collect();
                }
                let angles: Vec<f32> = rims.iter().enumerate().map(|(w, &i)| off(w, i)).map(|o| o.y.atan2(o.z)).collect();
                if let Some((then, angles_then)) = &before {
                    for (w, &n) in wheels.nodes.iter().enumerate() {
                        let parent = model.nodes[n].parent;
                        let (a0, s0) = spin_steer(then, n, parent);
                        let (a1, s1) = spin_steer(&palette, n, parent);
                        spun[w] += turn(a0, a1);
                        let steered = wheels.steer && w < 2;
                        if !steered {
                            assert!(s0.abs() < 1e-4 && s1.abs() < 1e-4, "{what} wheel {w} steers ({s1})");
                            skinned[w] += turn(angles_then[w], angles[w]);
                        } else {
                            assert!(s1.abs() <= toy::STEER_STOP + 1e-4, "{what} wheel {w} steers {s1}, past the stop");
                        }
                    }
                }
                before = Some((palette, angles));
            }
            let want = -(moved as f32) * wheels.inv_radius;
            println!("{what}: {} wheels, radius {:.3}, moved {moved:.2} ft, turned {:?}, want {want:.4}", wheels.nodes.len(), 1.0 / wheels.inv_radius, spun);
            for (w, (&gpu, &soft)) in spun.iter().zip(&skinned).enumerate() {
                assert!((gpu - want).abs() <= 0.01 + want.abs() * 0.01, "{what} wheel {w}: the palette turned {gpu} for {want}");
                if !(wheels.steer && w < 2) {
                    assert!((soft - want).abs() <= 0.01 + want.abs() * 0.01, "{what} wheel {w}: the skinned wheel turned {soft} for {want}");
                }
            }
            rolled.push((what, want));
        }
    }
    let free06 = rolled.iter().filter(|(w, t)| w.starts_with("free06 ") && t.abs() > 0.5).count();
    assert!(free06 >= 6, "free06's wheeled cars that rolled: {free06} of {rolled:?}");
}
