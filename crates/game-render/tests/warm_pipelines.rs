//! P377: a put-back draws its rider see-through with pipelines the ride's first picture never used,
//! and a phone's driver pays for a pipeline's first draw in that frame's submit (15 and 33 ms frames
//! on an iPhone). `Renderer::warm_pipelines` draws with every one at the load: after it, a ghost
//! frame names no pipeline as first drawn, and its picture is the one an unwarmed renderer draws.

use game_render::gpu::{Actor, Renderer, Scene3, Target, View};
use game_render::mat::{M4, v3};
use game_render::psg::{Mesh, Model, Node, Strip, Vertex};
use game_render::pst::Texture;
use game_render::scene::Scene;
use game_render::world::{Assets, Camera, Lighting, Loaded, Surface};

/// A wall 10 feet square, upright across +z at the origin.
fn wall() -> Loaded {
    let strip = Strip {
        verts: [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
            .iter()
            .map(|&(u, v)| Vertex {
                pos: v3((u - 0.5) * 10.0, (v - 0.5) * 10.0, 0.0),
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
            half_extents: v3(5.0, 5.0, 1.0),
            radius: 10.0,
        }],
        materials: vec!["wall".into()],
        meshes: vec![Mesh { material: 0, group: 0, strips: vec![strip] }],
        groups: 1,
        group_distance: vec![f32::MAX],
    };
    let tris = model.triangles(0);
    Loaded { model, tris, textures: vec![Some(0)], surfaces: vec![Surface::default()], radius: 10.0 }
}

/// The first frames of a ride on a fresh renderer, warmed or not: one with the actor solid, one
/// with it a ghost. What the ghost frame drew for the first time, and its picture.
fn ride(warm: bool) -> Option<(Vec<String>, Vec<u8>)> {
    let mut gpu = match Renderer::headless() {
        Ok(g) => g,
        Err(why) => {
            println!("SKIPPED: {why}");
            return None;
        }
    };
    let mut assets = Assets::default();
    assets.textures.push(Texture { width: 2, height: 2, rgba: [200, 90, 40, 255].repeat(4) });
    assets.models.insert("rider".into(), wall());
    let scene = Scene::parse("[Fog]\nColorRGB=61,68,76\nMinimumFogDistance=5000.00\nMaximumFogDistance=9000.00\n");
    let light = Lighting::of(&scene);
    gpu.set_scene(&scene);
    gpu.set_sky(None);

    let (w, h) = (160u32, 120u32);
    let texture = gpu.offscreen(w, h);
    let view = texture.create_view(&Default::default());
    let target = Target { view: &view, size: (w, h), frame: [0, 0, w, h], scale: 1 };
    let camera = Camera { eye: v3(0.0, 0.0, 0.0), at: v3(0.0, 0.0, 10.0), fov: 0.9, near: 0.5, far: 2000.0, up: None };
    let mut frame = |gpu: &mut Renderer, alpha: f32| {
        let at = M4::placement(v3(0.0, 0.0, 30.0), v3(0.0, 0.0, 1.0), v3(0.0, 1.0, 0.0));
        let actors = [Actor { model: "rider", at, pose: None, lod_from: None, alpha, casts: true }];
        let views = [View { rect: [0, 0, w, h], camera: &camera, actors: &actors }];
        gpu.render(&target, &assets, &Scene3 { views: &views, poses: &[], overlay: &[], sprites: &[], light: &light })
    };

    frame(&mut gpu, 1.0);
    if warm {
        gpu.warm_pipelines(&target);
    }
    let ghost = frame(&mut gpu, 0.5);
    Some((ghost.first_drawn, gpu.read(&texture)))
}

#[test]
fn a_warmed_renderer_draws_a_ghost_with_no_first_draw() {
    let Some((cold, cold_picture)) = ride(false) else { return };
    assert!(cold.iter().any(|p| p.contains("ghost")), "the cold ghost frame named {cold:?}, no ghost pipeline");
    let Some((warmed, warmed_picture)) = ride(true) else { return };
    assert!(warmed.is_empty(), "after the warm, the ghost frame still drew {warmed:?} first");
    let differ = cold_picture.iter().zip(&warmed_picture).filter(|(a, b)| a != b).count();
    assert_eq!(differ, 0, "{differ} bytes of the ghost picture differ after the warm");
}
