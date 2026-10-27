//! MD31: a mod map in a world is drawn at its own texture resolution. A world's first map was a
//! disc map, whose ground is baked into 128-pixel pages (MD15), and the world's one page array took
//! the first map's size: every mod map after it had its 256-pixel pages resampled down to 128,
//! nearest, and its ground came out blurred and aliased beside the same map ridden alone.
//!
//! What is proved: a generated world of a baked-size map and a generated mod map uploads the mod
//! map's pages at their own 256 pixels with every mip level, in an array of their own, and its 512-
//! pixel detail map at 512 with every level, repeating every `DetailMapTilingMultiplier` samples in
//! the world as alone; and on the GPU, the mod map's ground drawn in the world matches the same map
//! drawn alone, pixel for pixel but for rounding. The GPU half skips where there is no adapter.
//!
//! MD35: the same for a disc map. Its ground is a splat, layers that repeat every few feet, and a
//! world baked it into 128-pixel pages, four and a half feet a pixel: a disc map in a world was a
//! smear. Now the GPU runs the splat's passes per pixel in a world as alone, and takes from the bake
//! only what it does to them ([`SharpSplat::modulate`]). Proved: a generated disc map's ground drawn
//! in a world matches it drawn alone at three distances, where the bake alone does not.

use game_render::gpu::{self, ArrayShape, GroundChunk, GroundPaint, GroundVertex, Renderer, Scene3, Target, View};
use game_render::mat::v3;
use game_render::pst::Texture;
use game_render::splat::{Pass, Splat};
use game_render::terrain::{PaintTile, Pages, Regions, SharpSplat, WorldPaint};
use game_render::world::{Assets, Camera, Lighting};
use game_sim::tiles::generate;
use game_sim::track::Terrain;

const SPACING: f32 = 9.0;
/// The mod map's side: four leaves of sixteen samples, one page.
const SIDE: f32 = 64.0 * SPACING;
/// Where the mod map sits in the world, past the baked map.
const AT: [f32; 2] = [2048.0, 0.0];
const DIRT: [f32; 3] = [150.0, 118.0, 86.0];

/// The mod map's page: a checker of two-pixel squares, which a page at half its size cannot hold.
fn checker(x: u32, y: u32) -> [u8; 3] {
    if (x / 2 + y / 2) % 2 == 0 { [220, 200, 60] } else { [40, 60, 160] }
}

/// A generated mod map's pages, as the world's look reads them: its terrain's own page, and a
/// 512-pixel detail map over it every `multiplier` samples.
fn mod_pages(detail: Option<Texture>, multiplier: f32) -> Pages {
    let page: &dyn Fn(u32, u32) -> [u8; 3] = &checker;
    let bytes = generate::terrain_painted("md31", (4, 4), SPACING, |_, _| 40.0, |_, _| 1, Some(page));
    let terrain = Terrain::parse(&bytes).expect("the generated terrain reads");
    let pages = Pages::of(&terrain, SPACING, (4, 4)).expect("it has a page");
    assert_eq!((pages.pages[0].width, pages.pages[0].height), (256, 256), "the generator's page");
    pages.with_detail(detail, multiplier)
}

/// A 512-pixel detail map: a grey whose alpha changes every pixel, so a coarser copy would show.
fn detail() -> Texture {
    let n = 512;
    Texture { width: n, height: n, rgba: (0..n * n).flat_map(|i| [100, 100, 100, (((i % n) ^ (i / n)) % 64) as u8]).collect() }
}

/// A disc map's ground as a world bakes it (`world_dress::bake_splat`): 128-pixel pages.
fn baked_pages() -> Pages {
    let page = Texture { width: 128, height: 128, rgba: [90, 140, 70, 255].repeat(128 * 128) };
    Pages { pages: vec![page], across: 4, along: 4, page_of: vec![Some(0); 16], spacing: SPACING, detail: None, detail_repeat: SPACING * 8.0 }
}

/// The world: the baked map first, at the origin, and the mod map at [`AT`].
fn world(pages: Pages) -> WorldPaint {
    WorldPaint {
        regions: Regions { owned: vec![[0.0, 0.0, SIDE, SIDE], [AT[0], AT[1], AT[0] + SIDE, AT[1] + SIDE]], period: [0.0; 2] },
        tiles: vec![PaintTile { at: [0.0, 0.0], pages: Some(baked_pages()), sharp: None }, PaintTile { at: AT, pages: Some(pages), sharp: None }],
        dirt: DIRT,
    }
}

#[test]
fn a_world_uploads_a_mod_maps_pages_at_their_own_size_with_every_mip() {
    let w = world(mod_pages(Some(detail()), 8.0));
    let packed = w.pack(2048, 8192);
    let at = packed[1].expect("the mod map fits");
    assert_eq!(at.array, 1, "the mod map's pages are not in the baked map's array: {packed:?}");
    let (pages, details) = gpu::world_paint_arrays(&w, &packed);
    assert_eq!(
        pages,
        vec![ArrayShape { width: 128, height: 128, layers: 2, mips: 8 }, ArrayShape { width: 256, height: 256, layers: 2, mips: 9 }],
        "one array a page size, each at its own size down to 1 x 1"
    );
    assert_eq!(details, ArrayShape { width: 512, height: 512, layers: 2, mips: 10 }, "the detail map at its own size");
    // The detail repeats every multiplier's samples, in the world as alone, so the CPU's paint of a
    // point of the mod map is the map's own paint there.
    let alone = mod_pages(Some(detail()), 8.0);
    assert_eq!(alone.detail_repeat, 8.0 * SPACING);
    for (x, z) in [(10.0, 10.0), (100.25, 300.5), (283.0, 17.75), (500.0, 560.0)] {
        let own = alone.paint(x, z).expect("on the page").map(f32::from);
        assert_eq!(w.paint(AT[0] + x, AT[1] + z), own, "at ({x}, {z})");
    }
}

/// A smaller detail map than another map's goes into the array scaled up, never a larger one down.
#[test]
fn a_world_never_shrinks_a_detail_map() {
    let small = Texture { width: 64, height: 64, rgba: vec![128; 64 * 64 * 4] };
    let mut w = world(mod_pages(Some(detail()), 8.0));
    w.tiles[0].pages.as_mut().unwrap().detail = Some(small);
    let (_, details) = gpu::world_paint_arrays(&w, &w.pack(2048, 8192));
    assert_eq!((details.width, details.height, details.mips), (512, 512, 10));
}

/// The mod map's ground, flat, as one chunk of 64 cells, moved by `at`.
fn chunk(at: [f32; 2]) -> GroundChunk {
    let cells = 64;
    let vertices = (0..=cells)
        .flat_map(|j| (0..=cells).map(move |i| (i, j)))
        .map(|(i, j)| GroundVertex { pos: [at[0] + i as f32 * SPACING, 40.0, at[1] + j as f32 * SPACING], normal: [0.0, 1.0, 0.0] })
        .collect();
    GroundChunk { vertices, cells, at: (0, 0), missing: Vec::new(), centre: v3(at[0] + SIDE / 2.0, 40.0, at[1] + SIDE / 2.0), radius: SIDE }
}

/// One frame of a ground from a rider's height, looking along it, `at` added to the camera.
fn frame(gpu: &mut Renderer, at: [f32; 2]) -> Vec<u8> {
    frame_from(gpu, v3(at[0] + 288.0, 46.0, at[1] + 120.0), v3(at[0] + 300.0, 40.0, at[1] + 160.0))
}

/// One frame of the ground from `eye`, looking at `look`.
fn frame_from(gpu: &mut Renderer, eye: game_render::mat::V3, look: game_render::mat::V3) -> Vec<u8> {
    let scene = game_render::scene::Scene::parse("[Fog]\nColorRGB=61,68,76\nMinimumFogDistance=5000.00\nMaximumFogDistance=9000.00\n");
    let light = Lighting::of(&scene);
    gpu.set_scene(&scene);
    gpu.set_sky(None);
    let camera = Camera { eye, at: look, fov: 1.0, near: 0.5, far: 2000.0 };
    let (w, h) = (320u32, 180u32);
    let texture = gpu.offscreen(w, h);
    let view = texture.create_view(&Default::default());
    let views = [View { rect: [0, 0, w, h], camera: &camera, actors: &[] }];
    gpu.render(
        &Target { view: &view, size: (w, h), frame: [0, 0, w, h], scale: 1 },
        &Assets::default(),
        &Scene3 { views: &views, poses: &[], overlay: &[], sprites: &[], light: &light },
    );
    gpu.read(&texture)
}

#[test]
fn a_mod_map_in_a_world_is_drawn_as_sharp_as_alone() {
    let mut gpu = match Renderer::headless() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    // No detail map, so the page's own checker is all there is to see.
    let pages = mod_pages(None, 8.0);
    gpu.set_ground_painted(&[chunk([0.0, 0.0])], GroundPaint::Pages(&pages), DIRT).expect("the map alone");
    let alone = frame(&mut gpu, [0.0, 0.0]);
    let w = world(pages);
    gpu.set_ground_streamed(64, GroundPaint::World(&w), DIRT, 4.0 * SIDE).expect("the world");
    gpu.add_ground_piece(1, &[chunk(AT)]).expect("the mod map's piece");
    let in_world = frame(&mut gpu, AT);
    // The checker is on screen, both of its colours, so a blurred one would differ.
    let near = |p: &[u8], c: [u8; 3]| (0..3).all(|k| (p[k] as i32 - c[k] as i32).abs() < 40);
    let lit = alone.chunks_exact(3).filter(|p| near(p, [220, 200, 60]) || near(p, [40, 60, 160])).count();
    assert!(lit > 2000, "only {lit} pixels of the checker alone");
    let (mut off, mut worst) = (0, 0);
    for (a, b) in alone.chunks_exact(3).zip(in_world.chunks_exact(3)) {
        let d = a.iter().zip(b).map(|(&a, &b)| (a as i32 - b as i32).abs()).max().unwrap_or(0);
        off += usize::from(d > 8);
        worst = worst.max(d);
    }
    if let Ok(dir) = std::env::var("GPU_TEST_PNG") {
        let dir = std::path::Path::new(&dir);
        let _ = game_render::png::write_rgb(&dir.join("md31_alone.png"), 320, 180, &alone);
        let _ = game_render::png::write_rgb(&dir.join("md31_world.png"), 320, 180, &in_world);
    }
    let share = off as f32 / (alone.len() / 3) as f32;
    eprintln!("{off} pixels differ by more than 8, worst {worst}");
    assert!(share < 0.01, "{off} pixels ({:.1}%) of the world's frame differ from alone's by more than 8, worst {worst}", share * 100.0);
}

/// A disc map's splat: one layer, a checker of two-pixel squares on 64 pixels, laid every 8 feet and
/// turned 30 degrees — 1/8 foot a pixel, which a baked page at 4.5 feet a pixel cannot begin to hold.
fn splat() -> Splat {
    let n = 64;
    let layer = Texture {
        width: n,
        height: n,
        rgba: (0..n * n)
            .flat_map(|i| {
                let c = checker((i % n) as u32, (i / n) as u32);
                [c[0], c[1], c[2], 255]
            })
            .collect(),
    };
    let angle = 30f32.to_radians();
    let pass = Pass {
        texture: 0,
        scale: [1.0 / 8.0, 1.0 / 8.0],
        angle,
        rgba: [128.0; 4],
        blend: None,
        fixed: 0.0,
        turn: angle.sin_cos(),
        reads: (true, false),
    };
    Splat { textures: vec![layer], passes: vec![pass], shadow: None, meshes: None }
}

/// The disc map in a world, as `worlds::look` makes it: a page baked from the splat (here its
/// average, which is all 4.5 feet a pixel keeps of it), and with `sharp` the splat itself and a
/// modulate page of ones — no roads, no light map — to draw it per pixel.
fn disc_world(sharp: bool) -> WorldPaint {
    let page = Texture { width: 128, height: 128, rgba: [130, 130, 110, 255].repeat(128 * 128) };
    let one = game_render::terrain::MODULATE_ONE;
    let modulate = Texture { width: 128, height: 128, rgba: [one, one, one, 255].repeat(128 * 128) };
    let pages = Pages { pages: vec![page], across: 4, along: 4, page_of: vec![Some(0); 16], spacing: SPACING, detail: None, detail_repeat: SPACING * 8.0 };
    let sharp = sharp.then(|| SharpSplat { splat: std::sync::Arc::new(splat()), modulate: vec![modulate] });
    WorldPaint {
        regions: Regions { owned: vec![[0.0, 0.0, SIDE, SIDE], [AT[0], AT[1], AT[0] + SIDE, AT[1] + SIDE]], period: [0.0; 2] },
        tiles: vec![PaintTile { at: [0.0, 0.0], pages: Some(baked_pages()), sharp: None }, PaintTile { at: AT, pages: Some(pages), sharp }],
        dirt: DIRT,
    }
}

/// How many of `a`'s pixels `b` has off by more than [`SPLAT_TOLERANCE`], and the worst.
fn differ(a: &[u8], b: &[u8]) -> (usize, i32) {
    let (mut off, mut worst) = (0, 0);
    for (a, b) in a.chunks_exact(3).zip(b.chunks_exact(3)) {
        let d = a.iter().zip(b).map(|(&a, &b)| (a as i32 - b as i32).abs()).max().unwrap_or(0);
        off += usize::from(d > SPLAT_TOLERANCE);
        worst = worst.max(d);
    }
    (off, worst)
}

/// How far a pixel of a disc map in a world may be from alone's. A world samples the splat through
/// the ground's anisotropic sampler and with gradients it works out itself (a map's block runs in
/// non-uniform control flow), alone through the plain one and the hardware's: 22 feet over a two-
/// texel checker the two parted by up to 12 on 1.7% of the pixels, the frame's oblique edges; the
/// bake they replace was off by more than 16 on eight pixels in ten.
const SPLAT_TOLERANCE: i32 = 16;

#[test]
fn a_disc_map_in_a_world_is_drawn_as_sharp_as_alone_near_and_far() {
    let mut gpu = match Renderer::headless() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    // Near (the ground under the bike), a rider's look along it, and far, top-down so the
    // anisotropic filter a world's ground has and alone's splat does not cannot part them.
    let views = [("near", v3(300.0, 52.0, 300.0), v3(300.5, 40.0, 300.0)), ("mid", v3(300.0, 62.0, 300.0), v3(300.5, 40.0, 300.0)), ("far", v3(300.0, 300.0, 300.0), v3(300.5, 40.0, 300.0))];
    let moved = |p: game_render::mat::V3| v3(p.x + AT[0], p.y, p.z + AT[1]);
    let mut shots = Vec::new();
    for sharp in [true, false] {
        let w = disc_world(sharp);
        gpu.set_ground_streamed(64, GroundPaint::World(&w), DIRT, 4.0 * SIDE).expect("the world");
        gpu.add_ground_piece(1, &[chunk(AT)]).expect("the disc map's piece");
        shots.push(views.map(|(_, eye, look)| frame_from(&mut gpu, moved(eye), moved(look))));
    }
    let s = splat();
    gpu.set_ground(&[chunk([0.0, 0.0])], Some(&s), DIRT).expect("the map alone");
    let alone = views.map(|(_, eye, look)| frame_from(&mut gpu, eye, look));
    let pixels = (alone[0].len() / 3) as f32;
    for (k, (name, _, _)) in views.iter().enumerate() {
        let near = |p: &[u8], c: [u8; 3]| (0..3).all(|j| (p[j] as i32 - c[j] as i32).abs() < 40);
        let checker = alone[k].chunks_exact(3).filter(|p| near(p, [220, 200, 60]) || near(p, [40, 60, 160])).count();
        let (off, worst) = differ(&alone[k], &shots[0][k]);
        let (baked, _) = differ(&alone[k], &shots[1][k]);
        eprintln!("{name}: {checker} checker pixels alone; the world {off} off by more than {SPLAT_TOLERANCE} (worst {worst}), the bake alone {baked}");
        if let Ok(dir) = std::env::var("GPU_TEST_PNG") {
            let dir = std::path::Path::new(&dir);
            let _ = game_render::png::write_rgb(&dir.join(format!("md35_{name}_alone.png")), 320, 180, &alone[k]);
            let _ = game_render::png::write_rgb(&dir.join(format!("md35_{name}_world.png")), 320, 180, &shots[0][k]);
        }
        assert!(off as f32 / pixels < 0.01, "{name}: {off} pixels of the world's frame differ from alone's by more than {SPLAT_TOLERANCE}, worst {worst}");
        // Near and mid the checker is there to lose; far it has averaged out, alone too.
        if *name != "far" {
            assert!(checker as f32 / pixels > 0.3, "{name}: only {checker} pixels of the checker alone");
            assert!(baked as f32 / pixels > 0.3, "{name}: the baked page alone ({baked} pixels off) is as sharp as the splat; the test proves nothing");
        }
    }
}
