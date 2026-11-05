//! P298a: a ride's models and textures given back to the GPU in the background
//! (`Renderer::give_back_models`) come back on the next frame from the assets still held: the
//! scene's props, a grove-free owner's props and an actor are all drawn again, the same picture to
//! the byte, and what is on the GPU then is what was before.

use game_render::gpu::{Actor, Renderer, Scene3, Target, View};
use game_render::mat::{M4, v3};
use game_render::psg::{Mesh, Model, Node, Strip, Vertex};
use game_render::pst::Texture;
use game_render::scene::{Placement, Scene};
use game_render::world::{Assets, Camera, Lighting, Loaded, Surface};

/// A wall `size` feet square, upright across +z at the origin, painted with `texture`.
fn wall(size: f32, texture: usize) -> Loaded {
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
    Loaded { model, tris, textures: vec![Some(texture)], surfaces: vec![Surface::default()], radius: size }
}

fn at(x: f32, y: f32, z: f32) -> Placement {
    Placement { model: String::new(), pos: v3(x, y, z), look: v3(0.0, 0.0, 1.0), up: v3(0.0, 1.0, 0.0) }
}

#[test]
fn models_given_back_draw_again_on_the_next_frame() {
    let mut gpu = match Renderer::headless() {
        Ok(g) => g,
        Err(why) => {
            println!("SKIPPED: {why}");
            return;
        }
    };
    let mut assets = Assets::default();
    for rgba in [[220, 40, 40, 255], [40, 200, 60, 255], [50, 60, 230, 255]] {
        assets.textures.push(Texture { width: 2, height: 2, rgba: rgba.repeat(4) });
    }
    assets.models.insert("scene_wall".into(), wall(40.0, 0));
    assets.models.insert("owned_wall".into(), wall(20.0, 1));
    assets.models.insert("actor_wall".into(), wall(10.0, 2));

    let mut scene = Scene::parse("[Fog]\nColorRGB=61,68,76\nMinimumFogDistance=5000.00\nMaximumFogDistance=9000.00\n");
    scene.models.push(Placement { model: "scene_wall".into(), ..at(0.0, 0.0, 60.0) });
    let light = Lighting::of(&scene);
    gpu.set_scene(&scene);
    gpu.add_props(7, &[Placement { model: "owned_wall".into(), ..at(-12.0, 0.0, 50.0) }], None);
    gpu.set_sky(None);

    let (w, h) = (160u32, 120u32);
    let texture = gpu.offscreen(w, h);
    let view = texture.create_view(&Default::default());
    let camera = Camera { eye: v3(0.0, 0.0, 0.0), at: v3(0.0, 0.0, 10.0), fov: 0.9, near: 0.5, far: 2000.0 };
    let mut picture = |gpu: &mut Renderer| {
        let actors = [Actor { model: "actor_wall", at: M4::placement(v3(10.0, 0.0, 40.0), v3(0.0, 0.0, 1.0), v3(0.0, 1.0, 0.0)), pose: None, lod_from: None, alpha: 1.0, casts: false }];
        let views = [View { rect: [0, 0, w, h], camera: &camera, actors: &actors }];
        let target = Target { view: &view, size: (w, h), frame: [0, 0, w, h], scale: 1 };
        gpu.render(&target, &assets, &Scene3 { views: &views, poses: &[], overlay: &[], sprites: &[], light: &light });
        gpu.read(&texture)
    };

    let before = picture(&mut gpu);
    let held = gpu.uploaded();
    assert_eq!(held, (3, 3, 3), "the three walls, their materials and textures");
    // Each wall's own colour is in the picture, shaded: all three are drawn.
    for c in 0..3 {
        let its = |p: &[u8]| (0..3).filter(|&o| o != c).all(|o| p[c] > p[o].saturating_mul(3));
        assert!(before.chunks_exact(3).any(its), "the wall painted in channel {c} is not drawn");
    }

    assert_eq!(gpu.give_back_models(), held);
    assert_eq!(gpu.uploaded(), (0, 0, 0), "nothing left on the GPU");

    let after = picture(&mut gpu);
    assert_eq!(gpu.uploaded(), held, "uploaded again as drawn");
    let differ = before.iter().zip(&after).filter(|(a, b)| a != b).count();
    assert_eq!(differ, 0, "{differ} bytes differ after the give-back");

    // And again: a second time away is the same as the first.
    gpu.give_back_models();
    assert!(picture(&mut gpu) == before, "the second give-back drew a different picture");
}
