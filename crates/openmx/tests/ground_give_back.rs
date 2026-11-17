//! P307: the iPhone app in the background gives back a streamed world's ground. After P298a gave
//! back the ride's models, the phone's log still had a world ride at 1.3 GB in the background (GPU
//! 621 MB): its ground pieces' vertex pages and its paint's page arrays were all still held.
//!
//! Now `Renderer::give_back_ground` lets go every piece but the stand-in, the vertex pages past the
//! stand-in's, and the page arrays (each map on its far pages), and `take_back_ground_paint` makes
//! the arrays again on return, before the paint stream writes a map's pages into them; the ground
//! stream builds its pieces again (MD97's `build_again`).
//!
//! What is proved, on the GPU with the phone's lean vertex pages: a two-map world with a stand-in, a
//! piece in view and a 400-chunk piece off it gives back both pieces, 401 chunks, the second vertex
//! page and both arrays; the frame drawn then has the stand-in where the piece was and does not
//! fail; and with the arrays made again, the piece added and the maps' pages written again, the
//! frame is the one from before to the byte. Skips without an adapter.

use game_render::gpu::{GroundChunk, GroundPaint, GroundPiece, GroundVertex, Renderer, Scene3, StandIn, Target, View};
use game_render::mat::v3;
use game_render::pst::Texture;
use game_render::terrain::{PaintTile, Pages, Regions, WorldPaint};
use game_render::world::{Assets, Camera, Lighting};
use game_sim::tiles::generate;
use game_sim::track::Terrain;

const SPACING: f32 = 9.0;
/// A map's side: four leaves of sixteen samples, one page.
const SIDE: f32 = 64.0 * SPACING;
/// Where the second map sits, past the first.
const AT: [f32; 2] = [2048.0, 0.0];
const DIRT: [f32; 3] = [150.0, 118.0, 86.0];
/// The off-screen piece's chunks: past one lean vertex page of 64-cell chunks (330 of them).
const MANY: usize = 400;

/// A checker of two-pixel squares.
fn checker(x: u32, y: u32) -> [u8; 3] {
    if (x / 2 + y / 2) % 2 == 0 { [220, 200, 60] } else { [40, 60, 160] }
}

/// A generated mod map's pages.
fn mod_pages() -> Pages {
    let page: &dyn Fn(u32, u32) -> [u8; 3] = &checker;
    let bytes = generate::terrain_painted("p307", (4, 4), SPACING, |_, _| 40.0, |_, _| 1, Some(page));
    let terrain = Terrain::parse(&bytes).expect("the generated terrain reads");
    Pages::of(&terrain, SPACING, (4, 4)).expect("it has a page")
}

/// A disc map's ground as a world bakes it: 128-pixel pages.
fn baked_pages() -> Pages {
    let page = Texture { width: 128, height: 128, rgba: [90, 140, 70, 255].repeat(128 * 128) };
    Pages { pages: vec![page], across: 4, along: 4, page_of: vec![Some(0); 16], spacing: SPACING, detail: None, detail_repeat: SPACING * 8.0 }
}

/// Two maps, the baked one at the origin and the mod map at [`AT`]: an array each.
fn world() -> WorldPaint {
    WorldPaint {
        regions: Regions { owned: vec![[0.0, 0.0, SIDE, SIDE], [AT[0], AT[1], AT[0] + SIDE, AT[1] + SIDE]], period: [0.0; 2] },
        tiles: vec![
            PaintTile { at: [0.0, 0.0], pages: Some(baked_pages()), sharp: None },
            PaintTile { at: AT, pages: Some(mod_pages()), sharp: None },
        ],
        dirt: DIRT,
        far: Default::default(),
    }
}

/// A flat chunk of 64 cells at `at`, `y` feet up.
fn chunk(at: [f32; 2], y: f32) -> GroundChunk {
    let cells = 64;
    let vertices = (0..=cells)
        .flat_map(|j| (0..=cells).map(move |i| (i, j)))
        .map(|(i, j)| GroundVertex { pos: [at[0] + i as f32 * SPACING, y, at[1] + j as f32 * SPACING], normal: [0.0, 1.0, 0.0] })
        .collect();
    GroundChunk { vertices, cells, at: (0, 0), missing: Vec::new(), centre: v3(at[0] + SIDE / 2.0, y, at[1] + SIDE / 2.0), radius: SIDE }
}

/// One frame of the mod map from a rider's height, looking along it.
fn frame(gpu: &mut Renderer) -> Vec<u8> {
    let scene = game_render::scene::Scene::parse("[Fog]\nColorRGB=61,68,76\nMinimumFogDistance=5000.00\nMaximumFogDistance=9000.00\n");
    let light = Lighting::of(&scene);
    gpu.set_scene(&scene);
    gpu.set_sky(None);
    let camera = Camera { eye: v3(AT[0] + 288.0, 46.0, AT[1] + 120.0), at: v3(AT[0] + 300.0, 40.0, AT[1] + 160.0), fov: 1.0, near: 0.5, far: 2000.0 };
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

/// The two pieces as the ground stream hands them over: the mod map's, shown at once, and many
/// chunks far off.
fn add_pieces(gpu: &mut Renderer) {
    gpu.add_ground_piece_of(1, GroundPiece::of(vec![chunk(AT, 40.0)])).expect("the mod map's piece");
    gpu.show_ground_piece(1, 0.0);
    gpu.add_ground_piece_of(2, GroundPiece::of(vec![chunk([20000.0, 20000.0], 40.0); MANY])).expect("the far piece");
    gpu.show_ground_piece(2, 0.0);
}

/// Each map's pages written into its run of its array again, as the paint stream does.
fn write_maps(gpu: &mut Renderer, w: &WorldPaint) {
    for (i, t) in w.tiles.iter().enumerate() {
        let size = gpu.world_paint_layer_size(i).expect("a paged map");
        assert!(gpu.world_paint_take(i), "room for map {i}");
        for (k, page) in t.pages.as_ref().unwrap().pages.iter().enumerate() {
            gpu.world_paint_write(i, k as u32, &game_render::gpu::page_layer_mips(page, size));
        }
        gpu.world_paint_show(i);
    }
}

#[test]
fn the_background_gives_back_a_worlds_ground_and_it_comes_back_the_same() {
    let mut gpu = match Renderer::headless() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    game_render::gpu::WORLD_LEAN.store(true, std::sync::atomic::Ordering::Relaxed);
    let w = world();
    gpu.set_ground_streamed(64, GroundPaint::World(&w), DIRT, 40000.0).expect("the world");
    // The stand-in under the mod map, a little lower, so where it is drawn shows.
    let s = StandIn {
        piece: GroundPiece::of(vec![chunk(AT, 38.0)]),
        step: SPACING,
        origin: AT,
        cells: [64, 64],
        rects: vec![(1, [AT[0], AT[1], AT[0] + SIDE, AT[1] + SIDE])],
    };
    gpu.set_ground_stand_in(s).expect("the stand-in");
    add_pieces(&mut gpu);
    let (chunks, room) = gpu.ground_chunks();
    assert_eq!(chunks, 2 + MANY);
    assert!(room > 330, "one vertex page holds {room}: the far piece did not take a second");
    assert_eq!(gpu.world_paint_pool().expect("a world").1, 2, "both maps on their own pages from the load");
    let before = frame(&mut gpu);

    let (pieces, let_go, bytes) = gpu.give_back_ground();
    eprintln!("gave back {pieces} pieces, {let_go} chunks, {} KB", bytes >> 10);
    assert_eq!((pieces, let_go), (2, 1 + MANY));
    let page = 65 * 65 * std::mem::size_of::<GroundVertex>() as u64;
    assert!(bytes >= (room as u64 - 330) * page, "{bytes} bytes: the second vertex page was not let go");
    assert_eq!(gpu.ground_chunks(), (1, 330), "the stand-in's chunk on its one page is all that is held");
    let (used, shown) = gpu.world_paint_pool().expect("a world");
    assert_eq!(shown, 0, "every map on its far pages");
    assert!(used.iter().all(|u| u.0 == 0), "layers still taken: {used:?}");
    // Drawn while given back (the shell draws nothing in the background, but nothing must fail).
    let away = frame(&mut gpu);
    let off = before.chunks_exact(3).zip(away.chunks_exact(3)).filter(|(a, b)| a != b).count();
    assert!(off > 5000, "only {off} pixels changed: the piece or its paint is still drawn");
    assert_eq!(gpu.give_back_ground(), (0, 0, 0), "a second give-back has nothing left");

    assert!(gpu.take_back_ground_paint(), "the arrays are made again");
    assert!(!gpu.take_back_ground_paint(), "and only once");
    add_pieces(&mut gpu);
    write_maps(&mut gpu, &w);
    assert_eq!(gpu.ground_chunks().0, 2 + MANY);
    let back = frame(&mut gpu);
    let off = before.chunks_exact(3).zip(back.chunks_exact(3)).filter(|(a, b)| a != b).count();
    assert_eq!(off, 0, "{off} pixels differ from before the give-back");
}
