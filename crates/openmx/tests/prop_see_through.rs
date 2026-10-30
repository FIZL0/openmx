//! V107: what is behind the biplane's spinning prop shows through it. The blur cone's disc is a
//! blended material whose `.psm` leaves depth writes on, and it was drawn among the other
//! translucent runs in no order (GPU) or in the actors' order (software): the train's carriages,
//! whose materials blend too and come after the plane, failed the depth test inside the disc and
//! were missing there, the sky showing through instead.
//!
//! The disc's own plane and cone, from behind and above the plane looking ahead through the prop,
//! with a wall a little way ahead drawn after the plane — once blended (with depth writes, as the
//! carriages' are) and once opaque — on both renderers: wherever the cone's disc is drawn, the wall
//! must show through it as it shows beside it.
//!
//! `src/vehicle.rs` and `src/plane_draw.rs` are compiled in by path, the way `tests/biplane_surfaces.rs`
//! takes them.

#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;
#[path = "../src/plane_draw.rs"]
#[allow(dead_code)]
mod plane_draw;

use game_render::biplane::{self, Biplane};
use game_render::mat::{M4, V3, v3};
use game_render::psg::{Mesh, Model, Node, Strip, Vertex};
use game_render::pst::Texture;
use game_render::raster::Frame;
use game_render::scene::{Scene, Shading};
use game_render::world::{Assets, Camera, Lighting, Loaded, Surface};
use game_render::{Disc, gpu};


/// A red wall `size` feet square, upright across +z at the origin, of one material.
fn wall(surface: Surface, texture: usize) -> Loaded {
    let size = 400.0;
    let strip = Strip {
        verts: [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
            .iter()
            .map(|&(u, v)| Vertex {
                pos: v3((u - 0.5) * size, (v - 0.5) * size, 0.0),
                normal: v3(0.0, 0.0, -1.0),
                uv: [u, v],
                color: [128, 128, 128, 128],
                node: 0,
                adc: false,
            })
            .collect(),
    };
    let model = Model {
        nodes: vec![Node {
            name: "root".into(),
            local: M4::IDENTITY,
            parent: None,
            center: v3(0.0, 0.0, 0.0),
            half_extents: v3(size / 2.0, size / 2.0, 1.0),
            radius: size,
        }],
        materials: vec!["wall".into()],
        meshes: vec![Mesh { material: 0, group: 0, strips: vec![strip] }],
        groups: 1,
        group_distance: vec![f32::MAX],
    };
    let tris = model.triangles(0);
    Loaded { model, tris, textures: vec![Some(texture)], surfaces: vec![surface], radius: size }
}

fn translate(m: M4, by: V3) -> M4 {
    let mut m = m;
    m.0[3][0] += by.x;
    m.0[3][1] += by.y;
    m.0[3][2] += by.z;
    m
}

#[test]
fn what_is_behind_the_spinning_prop_shows_through_it() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            eprintln!("skipped: {why}");
            return;
        }
    };
    let res = disc.res("machine.res").expect("the machines' archive");
    let mut assets = Assets::default();
    let drawn = Biplane::load(&mut assets, &[res], 0).expect("the biplane's model");
    let cone = &assets.models[biplane::PROP];
    assert!(cone.surfaces.iter().any(|s| s.blend), "the blur cone has no see-through disc");
    assert!(cone.surfaces.iter().filter(|s| s.blend).all(|s| !s.depth_write), "the blur cone's disc writes depth");
    let hub = drawn.prop.expect("the plane's PropDummy");
    let hub = v3(hub.0[3][0], hub.0[3][1], hub.0[3][2]);

    assets.textures.push(Texture { width: 2, height: 2, rgba: [255, 0, 0, 255].repeat(4) });
    let red = assets.textures.len() - 1;
    let blended = Surface { shading: Shading::Prelit, cutoff: 0, depth_cutoff: 0, blend: true, additive: false, depth_write: true, clamp: [false; 2], keep_coverage: false };
    let opaque = Surface { blend: false, ..blended };

    let scene = Scene::parse("[Fog]\nColorRGB=61,68,76\nMinimumFogDistance=5000.00\nMaximumFogDistance=9000.00\n");
    let light = Lighting::of(&scene);
    let mut gpu = match gpu::Renderer::headless() {
        Ok(mut g) => {
            g.set_scene(&scene);
            g.set_sky(None);
            Some(g)
        }
        Err(e) => {
            eprintln!("GPU skipped: {e}");
            None
        }
    };
    // Behind the plane and above its nose, looking ahead through the prop: the disc stands over the
    // cowling, against the wall ahead.
    let camera = Camera { eye: hub.add(v3(0.0, 2.5, -9.0)), at: hub.add(v3(0.0, 0.0, 20.0)), fov: 0.9, near: 0.5, far: 2000.0 };
    let wall_at = translate(M4::IDENTITY, hub.add(v3(0.0, 0.0, 30.0)));
    let (w, h) = (320usize, 240usize);
    let parts = drawn.parts(M4::IDENTITY, false);

    // The software path draws `draw_scene`'s extras in their order, the plane and its cone first;
    // the GPU draws the plane's actors, then the wall's.
    let mut pictures = |assets: &Assets, with_cone: bool, with_wall: Option<&str>| -> (Vec<u8>, Option<Vec<u8>>) {
        let mut extra: Vec<(&str, M4)> = parts.iter().take(if with_cone { 2 } else { 1 }).copied().collect();
        if let Some(wall) = with_wall {
            extra.push((wall, wall_at));
        }
        let mut frame = Frame::new(w, h, scene.fog_color);
        game_render::world::draw_scene(&mut frame, &scene, assets, &extra, &camera);
        let hard = gpu.as_mut().map(|g| {
            let actors: Vec<gpu::Actor> = extra.iter().map(|&(model, at)| gpu::Actor { model, at, pose: None, lod_from: None, alpha: 1.0, casts: false }).collect();
            let texture = g.offscreen(w as u32, h as u32);
            let view = texture.create_view(&Default::default());
            let views = [gpu::View { rect: [0, 0, w as u32, h as u32], camera: &camera, actors: &actors }];
            let target = gpu::Target { view: &view, size: (w as u32, h as u32), frame: [0, 0, w as u32, h as u32], scale: 1 };
            g.render(&target, assets, &gpu::Scene3 { views: &views, poses: &[], overlay: &[], sprites: &[], light: &light });
            g.read(&texture)
        });
        (frame.color, hard)
    };
    let differs = |p: &[u8], q: &[u8]| p.iter().zip(q).any(|(&x, &y)| x.abs_diff(y) > 12);
    let redder = |p: &[u8], q: &[u8]| p[0] as i32 - q[0] as i32 > 24;

    // Each wall a model of its own: the GPU keeps a model by its name.
    for (name, model, surface) in [("blended", "wall@blended", blended), ("opaque", "wall@opaque", opaque)] {
        assets.models.insert(model.into(), wall(surface, red));
        let (plane_soft, plane_hard) = pictures(&assets, false, None);
        let (cone_soft, cone_hard) = pictures(&assets, true, None);
        let (both_soft, both_hard) = pictures(&assets, true, Some(model));
        let (wall_soft, wall_hard) = pictures(&assets, false, Some(model));
        let mut check = |renderer: &str, plane: &[u8], cone: &[u8], both: &[u8], bare: &[u8]| {
            // The disc: where the cone changes the picture of the plane alone, over the fog.
            let disc: Vec<usize> = (0..w * h).filter(|&i| differs(&plane[3 * i..3 * i + 3], &cone[3 * i..3 * i + 3])).collect();
            // Of those, where the wall is seen without the cone (not hidden by the plane).
            let open: Vec<usize> = disc.iter().copied().filter(|&i| redder(&bare[3 * i..3 * i + 3], &plane[3 * i..3 * i + 3])).collect();
            // And the wall through the cone: redder than the cone over the fog.
            let through = open.iter().filter(|&&i| redder(&both[3 * i..3 * i + 3], &cone[3 * i..3 * i + 3])).count();
            // And the disc still over it: the wall through the cone is not the bare wall, as it would
            // be with the wall drawn after a disc that writes no depth.
            let over = open.iter().filter(|&&i| differs(&both[3 * i..3 * i + 3], &bare[3 * i..3 * i + 3])).count();
            if let Ok(dir) = std::env::var("GPU_TEST_PNG") {
                let dir = std::path::Path::new(&dir);
                let _ = game_render::png::write_rgb(&dir.join(format!("v107_{name}_{renderer}.png")), w, h, both);
                let _ = game_render::png::write_rgb(&dir.join(format!("v107_{name}_{renderer}_cone.png")), w, h, cone);
            }
            eprintln!("{name} wall, {renderer}: {} disc pixels, {} over the wall, {through} show the wall through the disc, {over} the disc over the wall", disc.len(), open.len());
            assert!(open.len() > 300, "{name} wall, {renderer}: only {} pixels of the disc stand over the wall", open.len());
            assert!(through * 100 >= open.len() * 95, "{name} wall, {renderer}: the wall shows through {through} of {} disc pixels", open.len());
            assert!(over * 100 >= open.len() * 80, "{name} wall, {renderer}: the disc is over the wall in {over} of its {} pixels", open.len());
        };
        check("software", &plane_soft, &cone_soft, &both_soft, &wall_soft);
        if let (Some(plane), Some(cone), Some(both), Some(bare)) = (&plane_hard, &cone_hard, &both_hard, &wall_hard) {
            check("gpu", plane, cone, both, bare);
        }
    }
}
