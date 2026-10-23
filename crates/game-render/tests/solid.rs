//! Solid textures stay solid (V54): free06's train and the trucks along its line drew with holes the
//! sky showed through, on both renderers, where the original's are solid.
//!
//! The cause was the alpha test a material gets when its `.psm` says nothing. The original fills in
//! what a material leaves out from its default material (the one named `Vu1StripDiffuse.vuo`, read
//! out of a free06 save state's RAM): the test on, method "greater", reference 0 — so only a texel of
//! alpha 0 is a hole. The port cut every texel under 16 of the GS's 128, and the boxcars (1,528
//! texels of alpha 1), the scraper, the dump truck, the company truck, the pickup and the
//! port-a-john all carry texels under that. The scraper and the dump truck say `alphaTestOnOff = 0`
//! and were cut all the same. And the caboose, whose test fails into the colour buffer only, wrote
//! no depth at all, so whatever was drawn after it showed through its solid sides.
//!
//! Each check draws a material over a backdrop twice, the backdrop a different colour each time:
//! where the material is solid nothing of the backdrop is left, so the two pictures agree there.
//!
//! Needs the user's own disc, so every test skips with the reason when it is not there.

use game_render::disc::{Disc, Res};
use game_render::mat::{M4, v3};
use game_render::psg::{Mesh, Model, Node, Strip, Vertex};
use game_render::raster::Frame;
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

fn free06(disc: &mut Disc) -> Res {
    let member = disc
        .entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with("free06.res"))
        .min_by_key(|e| e.name.len())
        .map(|e| e.name.clone())
        .expect("free06's archive is on the disc");
    disc.res(&member).unwrap()
}

const W: usize = 96;
const H: usize = 64;

/// A quad ten feet across, upright, its texture's whole picture over it, prelit at 0x80.
fn quad() -> Model {
    let verts = [(-5.0, -5.0, 0.0, 1.0), (5.0, -5.0, 1.0, 1.0), (-5.0, 5.0, 0.0, 0.0), (5.0, 5.0, 1.0, 0.0)]
        .iter()
        .map(|&(x, y, u, v)| Vertex {
            pos: v3(x, y, 0.0),
            normal: v3(0.0, 0.0, -1.0),
            uv: [u, v],
            color: [128; 4],
            node: 0,
            adc: false,
        })
        .collect();
    Model {
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
    }
}

fn flat(rgb: [u8; 3]) -> Texture {
    Texture { width: 2, height: 2, rgba: [rgb[0], rgb[1], rgb[2], 255].repeat(4) }
}

/// The material under test on a quad ten feet off, and a backdrop of `behind` ten feet further on.
/// The backdrop is drawn after the material, with `backdrop`'s surface, so a material that keeps
/// depth hides it and one that does not is painted over.
fn scene(texture: Texture, surface: Surface, behind: [u8; 3], backdrop: Surface) -> (Scene, Assets, Camera) {
    let mut assets = Assets::default();
    assets.textures.push(texture);
    assets.textures.push(flat(behind));
    for (name, i, surface) in [("front", 0, surface), ("back", 1, backdrop)] {
        let model = quad();
        let tris = model.triangles(0);
        assets.models.insert(
            name.into(),
            Loaded { model, tris, textures: vec![Some(i)], surfaces: vec![surface], radius: 8.0 },
        );
    }
    let at = |model: &str, z: f32, scale: f32| Placement {
        model: model.into(),
        pos: v3(0.0, 0.0, z * scale),
        look: v3(0.0, 0.0, 1.0),
        up: v3(0.0, 1.0, 0.0),
    };
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
        models: vec![at("front", 10.0, 1.0), at("back", 12.0, 1.0)],
        ..Scene::parse("")
    };
    let camera = Camera { eye: v3(0.0, 0.0, 0.0), at: v3(0.0, 0.0, 10.0), fov: 1.2, near: 0.1, far: 1000.0 };
    (scene, assets, camera)
}

fn soft(scene: &Scene, assets: &Assets, camera: &Camera) -> Vec<u8> {
    let mut frame = Frame::new(W, H, scene.fog_color);
    assert_eq!(draw_scene(&mut frame, scene, assets, &[], camera), 4);
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

/// The pixels well inside the front quad (it spans about 66 of the 96 columns and all the rows).
fn inside() -> impl Iterator<Item = usize> {
    (8..H - 8).flat_map(|y| (24..W - 24).map(move |x| y * W + x))
}

/// Of the pixels inside the front quad, how many differ between the two backdrops by more than
/// `by`, on the software renderer and (where there is one) the GPU.
fn shows_through(texture: &Texture, surface: Surface, backdrop: Surface, by: u8) -> (usize, Option<usize>) {
    let count = |a: &[u8], b: &[u8]| {
        inside()
            .filter(|&i| (0..3).any(|k| a[3 * i + k].abs_diff(b[3 * i + k]) > by))
            .count()
    };
    let [one, two] = [[255, 0, 255], [0, 255, 0]].map(|c| scene(texture.clone(), surface, c, backdrop));
    let soft = count(&soft(&one.0, &one.1, &one.2), &soft(&two.0, &two.1, &two.2));
    let hard = hard(&one.0, &one.1, &one.2).zip(hard(&two.0, &two.1, &two.2)).map(|(a, b)| count(&a, &b));
    (soft, hard)
}

/// A material as the port reads it off the disc, and its texture.
fn material(res: &Res, name: &str) -> (Texture, Surface) {
    let mut assets = Assets::default();
    let (i, surface) = assets.resolve(&|f: &str| res.read_suffix(f).ok(), name);
    let texture = assets.textures[i.unwrap_or_else(|| panic!("{name} has a texture"))].clone();
    (texture, surface)
}

/// The train's boxcars and engine-side trucks have no texel of alpha 0, and the original's test lets
/// every texel over 0 through: drawn whole, they hide the backdrop at every pixel on both renderers.
/// Before V54 the boxcars' 1,528 texels of alpha 1 were holes, and that is checked too.
#[test]
fn the_free06_materials_that_had_holes_hide_what_is_behind_them() {
    let Some(mut disc) = disc() else { return };
    let res = free06(&mut disc);
    let mut gpu_checked = 0;
    for name in ["mm_boxcars", "mm_scraper", "mm_dumptruck", "mm_companytruck", "free06_pickup", "free06_port_a_jon"] {
        let (texture, surface) = material(&res, name);
        assert!(!surface.blend, "{name} is drawn opaque");
        let least = texture.rgba.chunks(4).map(|p| p[3]).min().unwrap();
        assert!(least > 0 && least < 32, "{name}'s faintest texel is {least} of 255: under the old cut, over 0");
        // Lit by its texel alone, so a pixel is a texel's colour.
        let surface = Surface { shading: Shading::Prelit, ..surface };
        let opaque = Surface { shading: Shading::Prelit, ..Surface::default() };
        let (soft, hard) = shows_through(&texture, surface, opaque, 0);
        assert_eq!(soft, 0, "{name}: the backdrop shows through {soft} pixels drawn in software");
        if let Some(hard) = hard {
            assert_eq!(hard, 0, "{name}: the backdrop shows through {hard} pixels on the GPU");
            gpu_checked += 1;
        }
        if name == "mm_boxcars" {
            let old = Surface { cutoff: 32, ..surface };
            assert!(shows_through(&texture, old, opaque, 0).0 > 0, "the old cut leaves holes the check sees");
        }
    }
    println!("checked on the GPU: {gpu_checked} of 6");
}

/// The caboose's test (greater or equal to 80, failing into colour only) leaves its solid texels in
/// the depth buffer: a blended thing drawn after it and further off is hidden where the caboose is
/// solid. Before V54 the caboose wrote no depth and the backdrop painted over all of it. Its texels
/// of alpha 1 (7% of them) are the see-through ones, as in the original, and stay so.
#[test]
fn the_caboose_hides_what_is_drawn_after_it_where_it_is_solid() {
    let Some(mut disc) = disc() else { return };
    let res = free06(&mut disc);
    let (texture, surface) = material(&res, "mm_caboose");
    assert_eq!((surface.blend, surface.cutoff, surface.depth_cutoff, surface.depth_write), (true, 0, 160, true));
    let surface = Surface { shading: Shading::Prelit, ..surface };
    let see_through = texture.rgba.chunks(4).filter(|p| p[3] < 160).count() as f32 / (texture.rgba.len() / 4) as f32;
    let all = inside().count() as f32;
    let (soft, hard) = shows_through(&texture, surface, surface, 2);
    // The share that shows through is about the share of see-through texels, nowhere near all.
    for (path, n) in [("software", Some(soft)), ("GPU", hard)] {
        let Some(n) = n else { continue };
        let share = n as f32 / all;
        assert!(share < see_through + 0.05, "{path}: {:.1}% shows through, {:.1}% of texels are see-through", share * 100.0, see_through * 100.0);
    }
    let old = Surface { depth_write: false, depth_cutoff: 0, ..surface };
    // The backdrop is smaller on screen than the caboose, so even uncovered it reaches about two
    // thirds of the pixels checked.
    let old = shows_through(&texture, old, old, 2).0;
    assert!(old as f32 > 0.5 * all && old > 10 * soft.max(1), "without depth the backdrop covers {old}, with it {soft}");
}

/// The materials that are meant to be see-through still are: the bushes' billboard (tested against
/// 16) shows the backdrop through a good share of it, and a blended shadow through all of it.
#[test]
fn the_see_through_free06_materials_still_show_what_is_behind_them() {
    let Some(mut disc) = disc() else { return };
    let res = free06(&mut disc);
    let opaque = Surface { shading: Shading::Prelit, ..Surface::default() };
    // As much as the backdrop shows with nothing at all in front: it is smaller on screen.
    let empty = Texture { width: 2, height: 2, rgba: vec![0; 16] };
    let reach = shows_through(&empty, opaque, opaque, 0);
    for name in ["bushesbillboard", "free06_shadow_01"] {
        let (texture, surface) = material(&res, name);
        let least = match surface.blend && surface.cutoff == 0 {
            true => 0.99,
            false => {
                let fail = texture.rgba.chunks(4).filter(|p| p[3] < surface.cutoff).count();
                let share = fail as f32 / (texture.rgba.len() / 4) as f32;
                assert!(share > 0.25, "{name}: {:.0}% of texels fail its test", share * 100.0);
                // Its empty texels gather at the picture's edges, and only the middle is checked.
                0.25
            }
        };
        let surface = Surface { shading: Shading::Prelit, ..surface };
        let (soft, hard) = shows_through(&texture, surface, opaque, 0);
        for (path, n, all) in [("software", Some(soft), Some(reach.0)), ("GPU", hard, reach.1)] {
            let (Some(n), Some(all)) = (n, all) else { continue };
            assert!(n as f32 > least * all as f32, "{name} on {path}: only {n} of {all} pixels show the backdrop");
        }
    }
}
