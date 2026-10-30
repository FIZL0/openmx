//! Generated grids, written and read back with openmx's `Terrain`: every sample's cell is the grid's.

use game_sim::track::{Scene, Terrain};
use ps2_float::F;
use trnwrite::scene::SceneText;
use trnwrite::{Grid, Options, Page, Trn, build, header, normal, openmx_view, read, write};

/// A deterministic bumpy grid: a few ramps and a ridge, so neighbouring blocks differ.
fn grid(width: u32, depth: u32, seed: u32) -> Grid {
    let mut s = seed;
    let mut heights = Vec::new();
    let mut surfaces = Vec::new();
    for z in 0..depth {
        for x in 0..width {
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let ridge = 4000u32.saturating_sub((x as i32 - 2 * z as i32).unsigned_abs() * 50);
            heights.push((20_000 + x * 13 + z * 7 + ridge + (s >> 24)) as u16);
            surfaces.push(((x / 5 + z / 7) % 9) as u8 | if s & 0x100 != 0 { 0x80 } else { 0 });
        }
    }
    Grid {
        width,
        depth,
        heights,
        surfaces,
        normals: Vec::new(),
    }
}

/// Every cell openmx reads off the terrain is the grid's: heights as `height_scale × raw`, the computed
/// normals, the surface ids. Cells at steps 1 and 2, as the game asks for them.
fn assert_cells(t: &Terrain, g: &Grid, hs: f32) {
    let (w, d) = (g.width, g.depth);
    let leaves = ((w - 1).div_ceil(16), (d - 1).div_ceil(16));
    let at = |x: u32, z: u32| (x.min(w - 1) + z.min(d - 1) * w) as usize;
    let mut cells = 0;
    for step in [1, 2] {
        for sz in 0..leaves.1 * 16 {
            for sx in 0..leaves.0 * 16 {
                let fits = sx % 16 + step < 17 && sz % 16 + step < 17;
                let q = t.cell(sx, sz, step);
                assert_eq!(q.is_some(), fits, "cell {sx},{sz} step {step}");
                let Some(q) = q else { continue };
                cells += 1;
                for (i, (dx, dz)) in [(0, 0), (step, 0), (0, step), (step, step)]
                    .into_iter()
                    .enumerate()
                {
                    let (x, z) = (sx + dx, sz + dz);
                    let want = F(hs) * F::from(i32::from(g.heights[at(x, z)]));
                    assert_eq!(q.height[i].to_bits(), want.to_bits(), "height at {x},{z}");
                    assert_eq!(
                        q.surface[i],
                        g.surfaces[at(x, z)] & 0x7f,
                        "surface at {x},{z}"
                    );
                    let n = normal(g, x.min(w - 1), z.min(d - 1), hs);
                    let got = [q.normal[i][0], q.normal[i][1], q.normal[i][2]];
                    let want = [n[0], n[1], n[2]].map(|v| F::from(i32::from(v)));
                    assert_eq!(got, want, "normal at {x},{z}");
                }
            }
        }
    }
    assert!(cells > 0);
    // Past the tree: no leaf.
    assert!(t.cell(leaves.0 * 16, 0, 1).is_none() || leaves.0 % 16 != 0);
}

fn round_trip(g: &Grid, o: &Options) -> (Trn, Vec<u8>) {
    let t = build(g, o).expect("builds");
    let (bytes, w) = write(&t).expect("writes");
    assert_eq!(w.tree_at, header(&bytes).unwrap().tree_at);
    let view = openmx_view(&bytes).unwrap();
    let back = Terrain::parse(&view).expect("openmx reads it");
    assert_eq!(back.scale, F(o.scale));
    assert_eq!(back.levels, t.levels);
    assert_eq!(back.blocks as usize, w.blocks);
    assert_eq!(back.name, o.name);
    assert_cells(&back, g, o.height_scale);
    // Decoded again it is the same terrain, and written again the same bytes.
    let again = read(&bytes).expect("reads back");
    assert_eq!(again, t);
    assert_eq!(write(&again).unwrap().0, bytes);
    (t, bytes)
}

#[test]
fn a_one_level_grid_round_trips() {
    // 70×45 samples: 5×3 leaves, the last of each row and column only partly on the grid.
    let g = grid(70, 45, 7);
    let (t, bytes) = round_trip(&g, &Options::new(4.5, 0.0007));
    assert_eq!(t.levels, 1);
    assert_eq!(t.count(), (1 + 15, 15));
    assert_eq!(t.root_cells(), (1, 1));
    // No pages: openmx takes the file as written, without the view.
    assert!(Terrain::parse(&bytes).is_ok());
}

#[test]
fn a_two_level_grid_round_trips() {
    // 300×290: 19×19 leaves, over 2×2 root cells.
    let g = grid(300, 290, 11);
    let (t, _) = round_trip(&g, &Options::new(9.0, 0.00065));
    assert_eq!(t.levels, 2);
    assert_eq!(t.root_cells(), (2, 2));
    assert_eq!(t.count().1, 19 * 19);
    // Every node's bounds hold its children's.
    for c in t.root.children.iter().flatten() {
        assert!(t.root.min_y <= c.min_y && c.max_y <= t.root.max_y);
    }
}

#[test]
fn given_normals_are_kept() {
    let mut g = grid(33, 33, 3);
    g.normals = (0..33 * 33)
        .map(|i| [(i % 7) as i8 - 3, 120, (i % 5) as i8 - 2])
        .collect();
    let t = build(&g, &Options::new(3.0, 0.0006)).unwrap();
    let (bytes, _) = write(&t).unwrap();
    let back = Terrain::parse(&bytes).unwrap();
    let blk = back.block(back.leaf(17, 20).unwrap().0.block).unwrap();
    assert_eq!(blk.raw_normal(1, 4), Some(g.normals[17 + 20 * 33]));
}

#[test]
fn identical_blocks_are_written_once() {
    // A flat 5×5-leaf grid: every leaf block is the same. The root's block samples the same heights but
    // has base 0, as above-leaf blocks do on the disc, so it is the second block.
    let g = Grid {
        width: 81,
        depth: 81,
        heights: vec![30_000; 81 * 81],
        surfaces: vec![2; 81 * 81],
        normals: vec![],
    };
    let t = build(&g, &Options::new(9.0, 0.00065)).unwrap();
    let (bytes, w) = write(&t).unwrap();
    assert_eq!((w.leaves, w.blocks), (25, 2));
    let back = Terrain::parse(&bytes).unwrap();
    assert_eq!(back.blocks, 2);
    for (x, z) in [(0, 0), (40, 70), (79, 79)] {
        assert_eq!(back.leaf(x, z).unwrap().0.block, 1);
    }
    // A bumpy grid shares nothing.
    let g = grid(81, 81, 5);
    let (_, w) = write(&build(&g, &Options::new(9.0, 0.00065)).unwrap()).unwrap();
    assert_eq!(w.blocks, 26);
}

#[test]
fn pages_sit_between_the_blocks_and_the_tree() {
    let g = grid(129, 129, 9);
    let mut o = Options::new(4.5, 0.0008);
    o.pages = (0..3u8)
        .map(|i| Page::psmt8(vec![i; 65536]).unwrap())
        .collect();
    // 129 samples = 3 tiles a side (a tile is 64 samples); a 4×4 map leaves the rest of the cell empty.
    o.tiles_x = 4;
    o.page_map = (0..16)
        .map(|i| {
            if i % 4 < 3 && i / 4 < 3 {
                Some((i % 3) as u16)
            } else {
                None
            }
        })
        .collect();
    let (t, bytes) = round_trip(&g, &o);

    // The game reads the root where the last page ends, and the header says the same place.
    let h = header(&bytes).unwrap();
    assert_eq!(h.pages.len(), 3);
    assert_eq!(h.pages_end, h.tree_at);
    assert_eq!(h.tree_at - h.pages_at, 3 * 65_560);
    // The page offset table names each record.
    let offsets: Vec<u32> = (0..3)
        .map(|i| {
            let at = h.blocks_at - 12 + 4 * i;
            u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
        })
        .collect();
    assert_eq!(
        offsets,
        [
            h.pages_at as u32,
            h.pages_at as u32 + 65_560,
            h.pages_at as u32 + 131_120
        ]
    );
    // openmx refused the file as written until its S98 (MD1, 2026-10-22), and read the view; since
    // then it reads the file itself, its pages where the header says.
    if let Ok(direct) = Terrain::parse(&bytes) {
        assert_eq!(direct.root_at() as usize, h.tree_at);
    }
    // The root of a one-level terrain is the node over the root cell, and carries its pages.
    assert_eq!(t.root.pages[0], Some(0));
    assert_eq!(t.root.pages[5], Some(2));
    assert_eq!(t.root.pages[3], None);
    assert!(
        t.root
            .children
            .iter()
            .flatten()
            .all(|c| c.pages == [None; 16])
    );
}

#[test]
fn what_the_game_cannot_take_is_refused() {
    let g = grid(33, 33, 1);
    let mut o = Options::new(4.5, 0.0008);
    o.pages = vec![Page::psmt8(vec![0; 65536]).unwrap(); 129];
    assert!(build(&g, &o).unwrap_err().contains("128"));

    let mut o = Options::new(4.5, 0.0008);
    o.tiles_x = 1;
    o.page_map = vec![Some(0)];
    assert!(build(&g, &o).unwrap_err().contains("page 0 of 0"));

    let big = Grid {
        width: 4098,
        depth: 2,
        heights: vec![0; 8196],
        surfaces: vec![],
        normals: vec![],
    };
    assert!(
        build(&big, &Options::new(4.5, 0.0008))
            .unwrap_err()
            .contains("16 root cells")
    );
    // 4097 samples is the most: 16 root cells.
    let most = Grid {
        width: 4097,
        depth: 17,
        heights: vec![1; 4097 * 17],
        surfaces: vec![],
        normals: vec![],
    };
    let t = build(&most, &Options::new(4.5, 0.0008)).unwrap();
    assert_eq!(t.root_cells(), (16, 1));

    let mut o = Options::new(4.5, 0.0008);
    o.name = "x".repeat(32);
    assert!(build(&g, &o).is_err());
}

#[test]
fn heights_in_feet_quantise_to_the_top() {
    let feet: Vec<f32> = (0..17 * 17).map(|i| (i % 17) as f32 * 2.5).collect();
    let (g, hs) = Grid::from_feet(17, 17, &feet, 4.5).unwrap();
    assert_eq!(*g.heights.iter().max().unwrap(), 65535);
    for (h, raw) in feet.iter().zip(&g.heights) {
        let back = 4.5 * hs * f32::from(*raw);
        assert!((back - h).abs() <= 4.5 * hs, "{h} came back {back}");
    }
    assert!(Grid::from_feet(1, 1, &[-1.0], 4.5).is_err());
}

#[test]
fn the_scene_reads_back_with_the_terrain_layout() {
    let g = grid(300, 290, 2);
    let t = build(&g, &Options::new(4.5, 0.0008)).unwrap();
    let s = SceneText::for_terrain(&t, "elev_test", [100.0, 50.5, 200.25], [0.0, 0.0, 1.0]);
    let text = s.to_text();
    assert!(text.contains("\r\n") && !text.replace("\r\n", "").contains('\n'));
    let back = Scene::parse(text.as_bytes()).expect("game-sim reads the scene");
    assert_eq!(back.name, "elev_test");
    assert_eq!(back.terrain_file, "elev_test.trn");
    assert_eq!((back.terrain.width, back.terrain.breadth), (2, 2));
    assert_eq!(back.terrain.width_scale, t.scale);
    assert_eq!(back.terrain.breadth_scale, t.scale);
    assert_eq!(back.terrain.min_division_size, F(96.0));
    assert_eq!(back.start, [F(100.0), F(50.5), F(200.25)]);
    // The map edge (width × 256 × scale) covers the grid.
    assert!(back.terrain.width * 256 >= g.width - 1 && back.terrain.breadth * 256 >= g.depth - 1);
}
