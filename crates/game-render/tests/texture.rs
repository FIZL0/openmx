//! Texture mapping (V27): a known texture's texels and a known model's texture coordinates against
//! values read by hand out of the disc's bytes, and a triangle's sampled colour at a known texture
//! coordinate, through the software rasteriser and the GPU renderer alike.
//!
//! The disc tests need the user's disc (`kit.toml`) and say SKIPPED without one; the drawn ones
//! that need a GPU say so too.

use game_render::disc::{Disc, Res};
use game_render::mat::{M4, v3};
use game_render::psg::{Mesh, Model, Node, Strip, Vertex};
use game_render::raster::{Frame, Look, Shade};
use game_render::scene::{Placement, Scene, Shading};
use game_render::world::{Assets, Camera, Lighting, Loaded, Surface, draw_scene};
use game_render::Texture;

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            println!("SKIPPED: {why}");
            None
        }
    }
}

fn free09(disc: &mut Disc) -> Res {
    let member = disc
        .entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with("free09.res"))
        .min_by_key(|e| e.name.len())
        .map(|e| e.name.clone())
        .expect("free09's archive is on the disc");
    disc.res(&member).unwrap()
}

fn le32(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(d[at..at + 4].try_into().unwrap())
}

fn texel(t: &Texture, x: usize, y: usize) -> [u8; 4] {
    t.rgba[4 * (y * t.width + x)..4 * (y * t.width + x) + 4].try_into().unwrap()
}

/// `free09_rock.pst`, the rock layer of the freeride ground: six mips, 8x8 up to 256x256, the
/// smallest first. Read by hand from the file: each mip's head is (height, width, bytes, 0), at
/// 1036, 1116, 1388, 2428, 6540 and 22940, so the 256x256 mip's pixels start at 22956. Its first
/// pixel is index 130, whose colour is palette slot 130 (the GS's block swap leaves 130 where it is)
/// at byte 8 + 4 * 130 = 528: 116, 118, 108, alpha 0x80. Pixel (1, 0) is index 117, slot 109 after
/// the swap, at byte 444: 112, 115, 107. Pixel (17, 200) is index 99, slot 99, at 404: 118, 113,
/// 96. Pixel (255, 255) is index 177, slot 169, at 684: 126, 129, 117. Before V27 the decoder read
/// the picture from byte 1052, the 8x8 mip's first pixel, and the top 85 rows were the small mips.
#[test]
fn a_disc_texture_is_the_last_and_largest_mip_of_its_chain() {
    let Some(mut disc) = disc() else { return };
    let res = free09(&mut disc);
    let bytes = res.read_suffix("free09_rock.pst").unwrap();
    assert_eq!(bytes.len(), 88_492);
    assert_eq!(le32(&bytes, 1032), 6, "six mips");
    let heads: Vec<(u32, u32, u32)> = [1036, 1116, 1388, 2428, 6540, 22940]
        .iter()
        .map(|&at| (le32(&bytes, at), le32(&bytes, at + 4), le32(&bytes, at + 8)))
        .collect();
    assert_eq!(
        heads,
        [(8, 8, 64), (16, 16, 256), (32, 32, 1024), (64, 64, 4096), (128, 128, 16384), (256, 256, 65536)]
    );
    let t = Texture::parse(&bytes).unwrap();
    assert_eq!((t.width, t.height), (256, 256));
    assert_eq!(bytes[22956], 130);
    assert_eq!(texel(&t, 0, 0), [116, 118, 108, 255]);
    assert_eq!(texel(&t, 1, 0), [112, 115, 107, 255]);
    assert_eq!(texel(&t, 17, 200), [118, 113, 96, 255]);
    assert_eq!(texel(&t, 255, 255), [126, 129, 117, 255]);
}

/// `sign_speedlmt.psg`, the speed-limit sign on `free09`: one node, one material, scales at byte
/// 304 of (position 3.15e-4, normal 1/127, u 1/2047, v 1/2047). Its first texture-coordinate
/// unpack is the VIF code 0x650b8017 at byte 525 (UNPACK V2-16, eleven vertices, signed), and its
/// first three pairs are the bytes c4 07 1e 00, c4 07 e5 05 and 24 02 1e 00: (1988, 30), (1988,
/// 1509) and (548, 30), so (0.97118, 0.01466), (0.97118, 0.73718) and (0.26771, 0.01466).
#[test]
fn a_disc_model_s_texture_coordinates_are_its_integers_times_the_file_s_scale() {
    let Some(mut disc) = disc() else { return };
    let res = free09(&mut disc);
    let bytes = res.read_suffix("sign_speedlmt.psg").unwrap();
    assert_eq!(le32(&bytes, 304 + 8), 0x3a00_1002, "u's scale is 1/2047");
    assert_eq!(le32(&bytes, 525), 0x650b_8017);
    assert_eq!(&bytes[529..541], &[0xc4, 0x07, 0x1e, 0x00, 0xc4, 0x07, 0xe5, 0x05, 0x24, 0x02, 0x1e, 0x00]);
    let model = Model::parse(&bytes).unwrap();
    let first = &model.meshes[0].strips[0].verts;
    let scale = 1.0f32 / 2047.0;
    for (v, (u, w)) in first.iter().zip([(1988, 30), (1988, 1509), (548, 30)]) {
        assert!((v.uv[0] - u as f32 * scale).abs() < 1e-6, "{:?}", v.uv);
        assert!((v.uv[1] - w as f32 * scale).abs() < 1e-6, "{:?}", v.uv);
    }
}

/// A triangle covering the frame, every corner at the centre of the rock texture's texel (17, 200):
/// the software rasteriser paints it that texel's colour, 118, 113, 96, at full light.
#[test]
fn a_triangle_paints_the_texel_at_its_texture_coordinate() {
    let Some(mut disc) = disc() else { return };
    let res = free09(&mut disc);
    let t = Texture::parse(&res.read_suffix("free09_rock.pst").unwrap()).unwrap();
    let uv = [17.5 / 256.0, 200.5 / 256.0];
    let paint = |u: f32, v: f32| t.sample(u, v);
    let mut frame = Frame::new(4, 4, [0, 0, 0]);
    let at = |x: f32, y: f32| Shade { clip: [x, y, 0.5, 1.0], uv, light: [1.0; 3], fog: 0.0 };
    frame.triangle_with([at(-3.0, -3.0), at(9.0, -3.0), at(-3.0, 9.0)], &Look::cut_out(&paint), [0, 0, 0]);
    assert_eq!(&frame.color[(2 * 4 + 2) * 3..(2 * 4 + 2) * 3 + 3], &[118, 113, 96]);
}

/// One upright quad in front of the camera, ten feet across, with texture coordinates running
/// `u0..u1` from its left edge to its right and a constant `v`, prelit at 0x80 so the texel is the
/// colour.
fn one_quad(texture: Texture, u: (f32, f32), v: f32, clamp: [bool; 2]) -> (Scene, Assets, Camera) {
    let verts = [(-5.0, -5.0, u.0), (5.0, -5.0, u.1), (-5.0, 5.0, u.0), (5.0, 5.0, u.1)]
        .iter()
        .map(|&(x, y, u)| Vertex {
            pos: v3(x, y, 0.0),
            normal: v3(0.0, 0.0, -1.0),
            uv: [u, v],
            color: [128; 4],
            node: 0,
            adc: false,
        })
        .collect();
    let model = Model {
        nodes: vec![Node {
            name: "root".into(),
            local: M4::IDENTITY,
            parent: None,
            center: v3(0.0, 0.0, 0.0),
            half_extents: v3(5.0, 5.0, 0.1),
            radius: 8.0,
        }],
        materials: vec!["quad".into()],
        meshes: vec![Mesh { material: 0, group: 0, strips: vec![Strip { verts }] }],
        groups: 1,
        group_distance: vec![f32::MAX],
    };
    let mut assets = Assets::default();
    assets.textures.push(texture);
    let tris = model.triangles(0);
    let surface = Surface { shading: Shading::Prelit, cutoff: 0, clamp, ..Surface::default() };
    assets.models.insert(
        "quad".into(),
        Loaded { model, tris, textures: vec![Some(0)], surfaces: vec![surface], radius: 8.0 },
    );
    let scene = Scene {
        name: "test".into(),
        start: v3(0.0, 0.0, 0.0),
        start_dir: v3(0.0, 0.0, 1.0),
        terrain: String::new(),
        fog_color: [0, 0, 0],
        fog_near: 1.0e5,
        fog_far: 2.0e5,
        fog_visibility: 1.0,
        fog_haziness: 0.0,
        lights: Vec::new(),
        models: vec![Placement {
            model: "quad".into(),
            pos: v3(0.0, 0.0, 10.0),
            look: v3(0.0, 0.0, 1.0),
            up: v3(0.0, 1.0, 0.0),
        }],
        ..Scene::parse("")
    };
    let camera = Camera { eye: v3(0.0, 0.0, 0.0), at: v3(0.0, 0.0, 10.0), fov: 1.2, near: 0.1, far: 1000.0 };
    (scene, assets, camera)
}

const W: usize = 96;
const H: usize = 64;

/// The pixel column the quad's own point `x` feet along its width lands in.
fn column(scene: &Scene, camera: &Camera, x: f32) -> usize {
    let at = scene.models[0].transform().point(v3(x, 0.0, 0.0));
    let c = camera.view_projection(W, H).clip(at);
    ((c[0] / c[3] * 0.5 + 0.5) * W as f32) as usize
}

fn soft(scene: &Scene, assets: &Assets, camera: &Camera) -> Vec<u8> {
    let mut frame = Frame::new(W, H, scene.fog_color);
    assert_eq!(draw_scene(&mut frame, scene, assets, &[], camera), 2);
    frame.color
}

#[cfg(feature = "gpu")]
fn hard(scene: &Scene, assets: &Assets, camera: &Camera) -> Option<Vec<u8>> {
    use game_render::gpu::{Renderer, Scene3, Target, View};
    let mut gpu = match Renderer::headless() {
        Ok(r) => r,
        Err(e) => {
            println!("SKIPPED the GPU half: {e}");
            return None;
        }
    };
    gpu.set_scene(scene);
    gpu.set_sky(None);
    let (w, h) = (W as u32, H as u32);
    let texture = gpu.offscreen(w, h);
    let view = texture.create_view(&Default::default());
    let light = Lighting::of(scene);
    let views = [View { rect: [0, 0, w, h], camera, actors: &[] }];
    gpu.render(
        &Target { view: &view, size: (w, h), frame: [0, 0, w, h], scale: 1 },
        assets,
        &Scene3 { views: &views, poses: &[], overlay: &[], sprites: &[], light: &light },
    );
    Some(gpu.read(&texture))
}

#[cfg(not(feature = "gpu"))]
fn hard(_: &Scene, _: &Assets, _: &Camera) -> Option<Vec<u8>> {
    println!("SKIPPED the GPU half: built without it");
    None
}

fn pixel(rgb: &[u8], x: usize, y: usize) -> [u8; 3] {
    rgb[3 * (y * W + x)..3 * (y * W + x) + 3].try_into().unwrap()
}

fn near(a: [u8; 3], b: [u8; 3], by: i32) -> bool {
    a.iter().zip(b).all(|(&a, b)| (a as i32 - b as i32).abs() <= by)
}

/// The rock texture's texel (17, 200) under every pixel of a drawn quad: both renderers paint it
/// 118, 113, 96 — the GPU's bilinear filter at a texel's centre is that texel, and a coordinate that
/// does not change across the quad reads its finest mip.
#[test]
fn both_renderers_draw_a_quad_the_colour_of_its_texel() {
    let Some(mut disc) = disc() else { return };
    let res = free09(&mut disc);
    let t = Texture::parse(&res.read_suffix("free09_rock.pst").unwrap()).unwrap();
    let u = 17.5 / 256.0;
    let (scene, assets, camera) = one_quad(t, (u, u), 200.5 / 256.0, [false; 2]);
    let want = [118, 113, 96];
    assert_eq!(pixel(&soft(&scene, &assets, &camera), W / 2, H / 2), want);
    if let Some(gpu) = hard(&scene, &assets, &camera) {
        let got = pixel(&gpu, W / 2, H / 2);
        assert!(near(got, want, 2), "the GPU drew {got:?}");
    }
}

/// A material that clamps (`wrapModeHorizontal = 1`) holds its texture's edge past u = 1, where a
/// repeating one starts the texture again. A 4x1 texture, red red blue blue, over u 0..2: at u =
/// 1.25 a repeating quad is red (0.25 is the first half) and a clamping one blue (its last texel).
#[test]
fn a_clamping_material_holds_the_edge_where_a_repeating_one_wraps() {
    let stripes = || Texture {
        width: 4,
        height: 1,
        rgba: [[200, 0, 0, 255], [200, 0, 0, 255], [0, 0, 200, 255], [0, 0, 200, 255]].concat(),
    };
    // u = 1.25 is 1.25 / 2 of the way across the ten feet: 1.25 feet along from the centre. A quarter
    // of a texel's width either side stays inside the stripe for both filters.
    for (clamp, want) in [(false, [200, 0, 0]), (true, [0, 0, 200])] {
        let (scene, assets, camera) = one_quad(stripes(), (0.0, 2.0), 0.5, [clamp, false]);
        let x = column(&scene, &camera, 1.25);
        let soft = soft(&scene, &assets, &camera);
        assert_eq!(pixel(&soft, x, H / 2), want, "software, clamp {clamp}");
        if let Some(gpu) = hard(&scene, &assets, &camera) {
            let got = pixel(&gpu, x, H / 2);
            assert!(near(got, want, 8), "GPU, clamp {clamp}: {got:?}");
        }
    }
}
