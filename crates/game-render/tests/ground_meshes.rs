//! V67: the roads, ruts and runways a freeride scene places as `Type=TerrainScreenSpace` models are
//! painted into the ground, not stood on it as props.
//!
//! On free06 and free09 a point on the road's mesh takes the road texture's colour, in the software
//! renderer's paint and in a headless GPU frame, and a point off every mesh keeps the splat's own.
//!
//! Needs the user's own disc, so every test skips with the reason when it is not there.

use game_render::disc::{Disc, Res};
use game_render::ground_mesh::GroundMeshes;
use game_render::gpu::{GroundChunk, GroundPaint, GroundVertex, Renderer, Scene3, Target, View};
use game_render::scene::Scene;
use game_render::splat::Splat;
use game_render::v3;
use game_render::world::{Assets, Camera, Lighting};

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            println!("SKIPPED: {why}");
            None
        }
    }
}

/// A track's two archives, as the shell opens them: `name.res` and `name_2.res`.
fn archives(disc: &mut Disc, name: &str) -> Vec<Res> {
    [format!("\\{name}.res"), format!("\\{name}_2.res")]
        .iter()
        .filter_map(|want| {
            let member = disc
                .entries()
                .iter()
                .find(|e| e.name.to_ascii_lowercase().ends_with(want.as_str()))?
                .name
                .clone();
            Some(disc.res(&member).expect("the track's archive"))
        })
        .collect()
}

/// The scene's text, and how many of its models are screen-space ones, counted from the text itself.
fn scene(archives: &[Res], name: &str) -> (Scene, usize) {
    let text = game_render::ini::text(&archives[0].read_suffix(&format!("{name}.scn")).expect("the scene"));
    let typed = text
        .lines()
        .filter(|l| {
            let l: String = l.chars().filter(|c| !c.is_whitespace()).collect();
            l.eq_ignore_ascii_case("Type=TerrainScreenSpace")
        })
        .count();
    (Scene::parse(&text), typed)
}

/// A flat chunk of `cells` cells `size` feet each at chunk (`x`, `z`).
fn flat_chunk(cells: u32, size: f32, x: u32, z: u32) -> GroundChunk {
    let side = cells as f32 * size;
    let (x0, z0) = (x as f32 * side, z as f32 * side);
    let vertices = (0..=cells)
        .flat_map(|j| (0..=cells).map(move |i| (i, j)))
        .map(|(i, j)| GroundVertex { pos: [x0 + i as f32 * size, 0.0, z0 + j as f32 * size], normal: [0.0, 1.0, 0.0] })
        .collect();
    GroundChunk {
        vertices,
        cells,
        at: (x, z),
        missing: Vec::new(),
        centre: v3(x0 + side / 2.0, 0.0, z0 + side / 2.0),
        radius: side * std::f32::consts::FRAC_1_SQRT_2,
    }
}

fn distance(a: [u8; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|k| (a[k] as f32 - b[k]).powi(2)).sum::<f32>().sqrt()
}

/// Where on the road the mesh lays it on most thickly, and where near that nothing is laid at all.
fn on_and_off(road: &GroundMeshes, all: &GroundMeshes) -> ([f32; 2], [f32; 2]) {
    let on = road
        .tris
        .iter()
        .map(|t| [(t.xz[0][0] + t.xz[1][0] + t.xz[2][0]) / 3.0, (t.xz[0][1] + t.xz[1][1] + t.xz[2][1]) / 3.0])
        .filter(|p| p[0] > 0.0 && p[1] > 0.0)
        .min_by(|a, b| road.affine(a[0], a[1]).1.total_cmp(&road.affine(b[0], b[1]).1))
        .expect("the road has triangles");
    let off = (1..=40)
        .flat_map(|r| (0..32).map(move |k| (r as f32 * 3.0, k as f32 * std::f32::consts::TAU / 32.0)))
        .map(|(r, a)| [on[0] + r * a.cos(), on[1] + r * a.sin()])
        .find(|p| {
            let (over, keep) = all.affine(p[0], p[1]);
            keep > 0.999 && over.iter().all(|&o| o < 0.5) && p[0] > 0.0 && p[1] > 0.0
        })
        .expect("somewhere near the road that no mesh reaches");
    (on, off)
}

#[test]
fn the_roads_are_painted_into_the_ground_and_are_not_props() {
    let Some(mut disc) = disc() else { return };
    let mut gpu = Renderer::headless().map_err(|e| println!("SKIPPED the GPU half: {e}")).ok();
    let mut checked = 0;
    for track in ["free06", "free09"] {
        let archives = archives(&mut disc, track);
        let (scene, typed) = scene(&archives, track);
        // Every screen-space model is a ground mesh, and none of them is a prop.
        assert!(typed > 200, "{track}: {typed} screen-space models");
        assert_eq!(scene.ground_meshes.len(), typed, "{track}");
        for m in &scene.ground_meshes {
            assert!(!scene.models.iter().any(|p| p.model == m.model && p.pos.sub(m.pos).len() < 0.01), "{track}: {} is a prop", m.model);
        }

        let mut splat = Splat::load(&archives, track, (6912.0, 6912.0)).expect("the track's paint");
        let all = splat.meshes.take().expect("the scene's meshes load");
        let roads: Vec<_> = scene.ground_meshes.iter().filter(|p| p.model.to_ascii_lowercase().contains("road")).cloned().collect();
        let road = GroundMeshes::load(&archives, &roads).unwrap_or_else(|| panic!("{track}: no road mesh"));
        let (on, off) = on_and_off(&road, &all);
        let bare = [splat.paint(on[0], on[1]), splat.paint(off[0], off[1])];
        splat.meshes = Some(all);
        let painted = [splat.paint(on[0], on[1]), splat.paint(off[0], off[1])];

        // The road's own colour there: what it lays over the paint, given how much it covers.
        let (over, keep) = road.affine(on[0], on[1]);
        assert!(keep < 0.6, "{track}: the road hardly covers {on:?}: {keep}");
        let colour = over.map(|o| o / (1.0 - keep));
        println!("{track}: road at {on:?} is {colour:?} over {keep}; painted {:?} from {:?}; off at {off:?} {:?}", painted[0], bare[0], painted[1]);
        assert!(
            distance(painted[0], colour) + 20.0 < distance(bare[0], colour),
            "{track}: the road's {colour:?} at {on:?}, painted {:?} from {:?}",
            painted[0],
            bare[0]
        );
        assert_eq!(painted[1], bare[1], "{track}: off the meshes at {off:?}");

        let Some(gpu) = gpu.as_mut() else { continue };
        // A flat chunk under both points, looked at from above with no fog, once with the meshes and
        // once without: the light is the same in both, so it divides out.
        let side = 512.0;
        let (cx, cz) = (((on[0] + off[0]) / 2.0 / side) as u32, ((on[1] + off[1]) / 2.0 / side) as u32);
        let chunks: Vec<GroundChunk> = (cz - 1..=cz + 1).flat_map(|z| (cx - 1..=cx + 1).map(move |x| flat_chunk(32, 16.0, x, z))).collect();
        let look = Scene::parse("[Fog]\nColorRGB=0,0,0\nMinimumFogDistance=50000.00\nMaximumFogDistance=90000.00\n");
        let light = Lighting::of(&look);
        gpu.set_scene(&look);
        gpu.set_sky(None);
        let mid = [(on[0] + off[0]) / 2.0, (on[1] + off[1]) / 2.0];
        let camera = Camera { eye: v3(mid[0], 300.0, mid[1] - 1.0), at: v3(mid[0], 0.0, mid[1]), fov: 0.9, near: 1.0, far: 2000.0 };
        let (w, h) = (512u32, 512u32);
        let frame = |gpu: &mut Renderer, splat: &Splat| {
            gpu.set_ground_painted(&chunks, GroundPaint::Splat(splat), [150.0, 118.0, 86.0]).unwrap();
            let texture = gpu.offscreen(w, h);
            let view = texture.create_view(&Default::default());
            let views = [View { rect: [0, 0, w, h], camera: &camera, actors: &[] }];
            gpu.render(
                &Target { view: &view, size: (w, h), frame: [0, 0, w, h], scale: 1 },
                &Assets::default(),
                &Scene3 { views: &views, poses: &[], overlay: &[], sprites: &[], light: &light },
            );
            let rgb = gpu.read(&texture);
            let vp = camera.view_projection(w as usize, h as usize);
            [on, off].map(|p| {
                let c = vp.clip(v3(p[0], 0.0, p[1]));
                let (sx, sy) = ((c[0] / c[3] + 1.0) / 2.0 * w as f32, (1.0 - c[1] / c[3]) / 2.0 * h as f32);
                assert!(sx >= 0.0 && sy >= 0.0 && sx < w as f32 && sy < h as f32, "{p:?} is on screen");
                let at = (sy as usize * w as usize + sx as usize) * 3;
                [rgb[at], rgb[at + 1], rgb[at + 2]]
            })
        };
        let with = frame(gpu, &splat);
        let all = splat.meshes.take();
        let without = frame(gpu, &splat);
        splat.meshes = all;
        // The GPU's light at the road's point, per channel, from the frame without the meshes.
        let lit: [f32; 3] = std::array::from_fn(|k| without[0][k] as f32 / (bare[0][k] as f32).max(1.0));
        let expected: [f32; 3] = std::array::from_fn(|k| painted[0][k] as f32 * lit[k]);
        println!("{track} GPU: on {:?} (without {:?}, expected {expected:?}); off {:?} (without {:?})", with[0], without[0], with[1], without[1]);
        assert!(distance(with[0], expected) < 16.0, "{track}: the GPU's road at {on:?} is {:?}, not {expected:?}", with[0]);
        assert!(distance(with[0], without[0].map(f32::from)) > 20.0, "{track}: the GPU drew no road at {on:?}");
        assert!(distance(with[1], without[1].map(f32::from)) < 3.0, "{track}: the GPU changed {off:?}: {:?} from {:?}", with[1], without[1]);
        checked += 1;
    }
    if gpu.is_some() {
        assert_eq!(checked, 2);
    }
}
