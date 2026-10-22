//! MD4: a mod track's ground from its terrain's own pages, the way MX vs. ATV draws its maps.
//!
//! A generated one-level terrain, 8 x 4 leaves, carries two pages: its root's table names page 0 for
//! the left 4x4 block of leaves and page 1 for the right one. Page 0 is red on its left half and green
//! on its right, so the left block's first two columns of leaves draw red and the next two green;
//! page 1 is blue all over. One leaf is missing and draws nothing. Both the colour the software
//! renderer asks for ([`Pages::paint`]) and the GPU's frame are checked.

use game_render::gpu::{GroundChunk, GroundPaint, GroundVertex, Renderer, Scene3, Target, View};
use game_render::scene::Scene;
use game_render::terrain::{Pages, detail_multiplier, detail_over};
use game_render::world::{Assets, Camera, Lighting};
use game_render::{Texture, v3};
use game_sim::track::Terrain;

const RED: u8 = 1;
const BLUE: u8 = 2;
const GREEN: u8 = 3;
const ACROSS: u32 = 8;
const ALONG: u32 = 4;
/// The leaf with no node under the root.
const MISSING: (u32, u32) = (7, 3);
const NODE_BYTES: usize = 97;
const BLOCK_BYTES: usize = 2129;

fn word(out: &mut Vec<u8>, w: u32) {
    out.extend_from_slice(&w.to_le_bytes());
}

/// A node: the four floats, the block, then the sixteen-word page table, and where its children are.
fn node(out: &mut Vec<u8>, block: u16, table: [u32; 16], children_at: u32) {
    for f in [1.0f32, 0.5, 0.0, 0.0] {
        word(out, f.to_bits());
    }
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&block.to_le_bytes());
    word(out, 0);
    for w in table {
        word(out, w);
    }
    word(out, 0);
    out.push(0);
    word(out, children_at);
}

/// The terrain, as MX vs. ATV writes them: palette, one flat block, the two pages, then the tree.
fn terrain() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"PLN\0");
    out.push(b'7');
    out.extend_from_slice(b"ps2\0");
    out.extend_from_slice(&[0; 32]);
    word(&mut out, 1);
    // The palette: indices below 8 sit where they are under the GS's swizzle.
    let mut palette = [0u8; 1024];
    for (i, c) in [(RED, [200, 30, 30]), (BLUE, [30, 30, 200]), (GREEN, [30, 200, 30])] {
        palette[4 * i as usize..4 * i as usize + 3].copy_from_slice(&c);
        palette[4 * i as usize + 3] = 0x80;
    }
    out.extend_from_slice(&palette);
    word(&mut out, 1.0f32.to_bits());
    word(&mut out, 1); // levels
    let tree_at_slot = out.len();
    word(&mut out, 0);
    word(&mut out, 2);
    word(&mut out, 1); // blocks
    word(&mut out, 0);
    word(&mut out, 0);
    word(&mut out, 2); // the texture table
    word(&mut out, 2); // the textures stored
    let table_at = out.len();
    word(&mut out, 0);
    word(&mut out, 0);
    out.extend(std::iter::repeat_n(0, BLOCK_BYTES));
    for (i, fill) in [[RED, GREEN], [BLUE, BLUE]].into_iter().enumerate() {
        let at = out.len() as u32;
        out[table_at + 4 * i..table_at + 4 * i + 4].copy_from_slice(&at.to_le_bytes());
        for w in [39, 1, 256, 256, 65536, 0] {
            word(&mut out, w);
        }
        out.extend((0..65536usize).map(|p| fill[usize::from(p % 256 >= 128)]));
    }
    let tree_at = out.len() as u32;
    out[tree_at_slot..tree_at_slot + 4].copy_from_slice(&tree_at.to_le_bytes());
    let children_at = tree_at + NODE_BYTES as u32;
    let leaf_at = children_at + 256 * 4;
    // The root's table: the left block of leaves is page 0, the right one page 1 — twice its number.
    let mut table = [0; 16];
    table[1] = 2;
    node(&mut out, 0, table, children_at);
    for cz in 0..16 {
        for cx in 0..16 {
            let there = cx < ACROSS && cz < ALONG && (cx, cz) != MISSING;
            word(&mut out, if there { leaf_at } else { 0 });
        }
    }
    node(&mut out, 0, [0; 16], 0);
    out
}

fn pages() -> Pages {
    let bytes = terrain();
    let t = Terrain::parse(&bytes).expect("the generated terrain reads");
    assert_eq!(t.textures.len(), 2);
    Pages::of(&t, 1.0, (ACROSS, ALONG)).expect("it has pages")
}

/// What each leaf draws: page 0's red, page 0's green, page 1's blue, or nothing.
fn expected(lx: u32, lz: u32) -> Option<[u8; 3]> {
    if (lx, lz) == MISSING {
        None
    } else if lx < 2 {
        Some([200, 30, 30])
    } else if lx < 4 {
        Some([30, 200, 30])
    } else {
        Some([30, 30, 200])
    }
}

#[test]
fn a_terrain_with_two_pages_paints_each_where_its_leaves_name_it() {
    let p = pages();
    let leaf_page: Vec<_> = (0..ALONG).flat_map(|z| (0..ACROSS).map(move |x| (x, z))).map(|(x, z)| p.page_of[(z * ACROSS + x) as usize]).collect();
    assert_eq!(&leaf_page[..8], &[Some(0), Some(0), Some(0), Some(0), Some(1), Some(1), Some(1), Some(1)]);
    assert_eq!(leaf_page[31], None, "the missing leaf");
    for lz in 0..ALONG {
        for lx in 0..ACROSS {
            // The middle of the leaf, away from the pages' halves' seam.
            let (x, z) = (lx as f32 * 16.0 + 8.0, lz as f32 * 16.0 + 8.0);
            assert_eq!(p.paint(x, z), expected(lx, lz), "leaf {lx},{lz}");
        }
    }
    assert_eq!(p.paint(-1.0, 8.0), None, "off the terrain");
    assert_eq!(p.paint(8.0, 64.5), None, "past its last row of leaves");

    // A page's 64 samples lie inside its 8-pixel border: 1/32 at its first, 31/32 at its last.
    let (page, uv) = p.page_at(64.0, 0.0).unwrap();
    assert_eq!((page, uv), (1, [1.0 / 32.0, 1.0 / 32.0]));
    let (page, uv) = p.page_at(63.999, 47.0).unwrap();
    assert_eq!(page, 0);
    assert!((uv[0] - 31.0 / 32.0).abs() < 1e-4 && (uv[1] - (1.0 / 32.0 + 15.0 / 16.0 * 47.0 / 64.0)).abs() < 1e-4, "{uv:?}");
    // The fourth leaf along takes the page's last quarter, not its first.
    assert!((p.page_at(8.0, 56.0).unwrap().1[1] - (1.0 / 32.0 + 15.0 / 16.0 * 56.0 / 64.0)).abs() < 1e-5);
}

#[test]
fn the_detail_map_presses_in_by_its_alpha_and_repeats_by_the_scenes_number() {
    assert_eq!(detail_multiplier("[Environment]\nDetailMapTilingMultiplier=12.7\n"), 12.0, "read whole");
    assert_eq!(detail_multiplier("[Fog]\nColorRGB=1,2,3\n"), 8.0);
    // No alpha leaves the page as it is; the .pst's doubled 128 (256 clamps to 255) is about half.
    assert_eq!(detail_over([200.0, 100.0, 0.0], [24, 24, 24, 0]), [200.0, 100.0, 0.0]);
    let half = detail_over([200.0, 100.0, 0.0], [24, 24, 24, 128]);
    assert_eq!(half, [112.0, 62.0, 12.0]);

    let detail = Texture { width: 2, height: 1, rgba: vec![24, 24, 24, 0, 24, 24, 24, 128] };
    let p = pages().with_detail(Some(detail), 4.0);
    assert_eq!(p.detail_repeat, 4.0);
    // The middle of the detail's first texel, where it has no alpha, leaves leaf 0's red alone.
    assert_eq!(p.paint(1.0, 8.0), Some([200, 30, 30]));
    let darker = p.paint(3.0, 8.0).unwrap();
    assert!(darker[0] < 200 && darker[0] > 24, "{darker:?}");
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

#[test]
fn the_gpu_draws_each_page_where_its_leaves_name_it() {
    let mut gpu = match Renderer::headless() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let p = pages();
    // The leaves are 128 x 64 feet; the chunk is 128 x 128, so its far half has no page and is dirt.
    let chunks = [flat_chunk(8, 16.0, 0, 0)];
    let dirt = [150.0, 118.0, 86.0];
    let scene = Scene::parse("[Fog]\nColorRGB=0,0,0\nMinimumFogDistance=5000.00\nMaximumFogDistance=9000.00\n");
    let light = Lighting::of(&scene);
    gpu.set_scene(&scene);
    gpu.set_sky(None);
    gpu.set_ground_painted(&chunks, GroundPaint::Pages(&p), dirt).unwrap();

    // Looking almost straight down on the middle of the chunk.
    let camera = Camera { eye: v3(64.0, 260.0, 63.0), at: v3(64.0, 0.0, 64.0), fov: 0.9, near: 1.0, far: 1000.0 };
    let (w, h) = (256u32, 256u32);
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
    let pixel = |x: f32, z: f32| {
        let c = vp.clip(v3(x, 0.0, z));
        let (sx, sy) = ((c[0] / c[3] + 1.0) / 2.0 * w as f32, (1.0 - c[1] / c[3]) / 2.0 * h as f32);
        assert!(sx >= 0.0 && sy >= 0.0 && sx < w as f32 && sy < h as f32, "{x},{z} is on screen");
        let at = (sy as usize * w as usize + sx as usize) * 3;
        [rgb[at], rgb[at + 1], rgb[at + 2]]
    };
    // Which of red, green, blue a pixel is mostly; the dirt is red over green over blue.
    let strongest = |c: [u8; 3]| (0..3).max_by_key(|&k| c[k]).unwrap();
    for lz in 0..ALONG {
        for lx in 0..ACROSS {
            let c = pixel(lx as f32 * 16.0 + 8.0, lz as f32 * 16.0 + 8.0);
            match expected(lx, lz) {
                Some(e) => {
                    let k = strongest(e);
                    assert_eq!(strongest(c), k, "leaf {lx},{lz}: {c:?}");
                    assert!(c.iter().enumerate().all(|(i, &v)| i == k || u32::from(v) * 2 < u32::from(c[k])), "leaf {lx},{lz}: {c:?}");
                }
                None => assert!(c[0] > c[1] && c[1] > c[2], "the missing leaf is dirt: {c:?}"),
            }
        }
    }
    let c = pixel(64.0, 100.0);
    assert!(c[0] > c[1] && c[1] > c[2] && c[2] > 20, "no leaves there, so dirt: {c:?}");
}
