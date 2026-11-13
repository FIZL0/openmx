//! P337: the disc's supercross stadiums (and nationals) draw their dirt, not one flat colour.
//!
//! A supercross track has no splat: its ground is its `.trn`'s own pages under `<track>_d.pst`, as a
//! mod track's is (MD4), and so is a national's. [`track_paint`] gives every `sxNN` on the disc, and
//! nat01, its pages on every leaf and a detail map, leaves a freeride track its splat, and a headless
//! GPU frame of sx01's pages is textured where the bare colour is flat.
//!
//! Needs the user's own disc, so every test skips with the reason when it is not there.

use game_render::disc::{Disc, Res};
use game_render::gpu::{GroundChunk, GroundPaint, GroundVertex, Renderer, Scene3, Target, TrackPaint, View, track_paint};
use game_render::scene::Scene;
use game_render::v3;
use game_render::world::{Assets, Camera, Lighting};
use game_sim::track;

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

/// The track's paint as the shell reads it (openmx's `Ground::dressed`): its field `width x 256`
/// samples a side capped at the tree's, sixteen samples a leaf.
fn paint_of(archives: &[Res], name: &str) -> TrackPaint {
    let scn = archives[0].read_suffix(&format!("{name}.scn")).expect("the scene");
    let place = track::Scene::parse(&scn).expect("the scene parses");
    let tagged = place.terrain_file.rsplit_once('.').map(|(s, e)| format!("{s}.ps2.{e}")).unwrap_or_default();
    let bytes = archives[0].read_suffix(&place.terrain_file).or_else(|_| archives[0].read_suffix(&tagged)).expect("the terrain");
    let terrain = track::Terrain::parse(&bytes).expect("the terrain parses");
    let tree = terrain.samples().unwrap_or([0, 0]);
    let cap = |count: u32, axis: usize| {
        let scene = count.max(1) * game_sim::world::SAMPLES_PER_WIDTH;
        if tree[axis] > 0 { scene.min(tree[axis]) } else { scene }
    };
    let (sx, sz) = (cap(place.terrain.width, 0), cap(place.terrain.breadth, 1));
    let spacing = terrain.scale.0;
    let extent = ((sx - 1) as f32 * spacing, (sz - 1) as f32 * spacing);
    track_paint(archives, name, &game_render::ini::text(&scn), &terrain, spacing, (sx / 16, sz / 16), extent)
}

#[test]
fn every_supercross_track_paints_its_dirt_from_its_pages() {
    let Some(mut disc) = disc() else { return };
    let mut names: Vec<String> = disc
        .entries()
        .iter()
        .filter_map(|e| {
            let n = e.name.to_ascii_lowercase();
            let stem = n.rsplit(['\\', '/']).next()?.strip_suffix(".res")?.to_string();
            (stem.len() == 4 && stem.starts_with("sx") && stem[2..].bytes().all(|b| b.is_ascii_digit())).then_some(stem)
        })
        .collect();
    names.sort();
    names.dedup();
    assert!(names.len() >= 10, "the disc's supercross tracks: {names:?}");
    for name in &names {
        let a = archives(&mut disc, name);
        let p = paint_of(&a, name);
        assert!(p.splat.is_none(), "{name} has no splat");
        assert!(matches!(p.paint(), GroundPaint::Pages(_)), "{name} is drawn from its pages");
        let pages = p.pages.unwrap_or_else(|| panic!("{name}: its terrain's pages"));
        let paged = pages.page_of.iter().filter(|p| p.is_some()).count();
        assert_eq!(paged, pages.page_of.len(), "{name}: every leaf has a page");
        assert!(pages.detail.is_some(), "{name}: its detail map");
    }
    println!("{} supercross tracks paged: {}", names.len(), names.join(" "));
    // A freeride track keeps its splat; a national has none and is paged as well.
    let a = archives(&mut disc, "free09");
    let p = paint_of(&a, "free09");
    assert!(p.splat.is_some() && p.pages.is_none(), "free09 keeps its splat");
    let a = archives(&mut disc, "nat01");
    let p = paint_of(&a, "nat01");
    assert!(p.splat.is_none(), "nat01 has no splat");
    let pages = p.pages.expect("nat01's terrain's pages");
    assert!(pages.page_of.iter().all(|p| p.is_some()) && pages.detail.is_some(), "nat01: a page on every leaf, and its detail map");
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

/// How far the frame's pixels stray from their mean, on average over the three channels.
fn spread(rgb: &[u8]) -> f32 {
    let n = (rgb.len() / 3) as f32;
    (0..3)
        .map(|k| {
            let mean = rgb.iter().skip(k).step_by(3).map(|&v| v as f32).sum::<f32>() / n;
            (rgb.iter().skip(k).step_by(3).map(|&v| (v as f32 - mean).powi(2)).sum::<f32>() / n).sqrt()
        })
        .sum::<f32>()
        / 3.0
}

#[test]
fn dirt_textures_load() {
    let Some(mut disc) = disc() else { return };
    let mut gpu = match Renderer::headless() {
        Ok(r) => r,
        Err(e) => {
            println!("SKIPPED: {e}");
            return;
        }
    };
    let a = archives(&mut disc, "sx01");
    let p = paint_of(&a, "sx01");
    assert!(p.pages.is_some(), "sx01's pages");
    let dirt = [150.0, 118.0, 86.0];
    let scene = Scene::parse("[Fog]\nColorRGB=0,0,0\nMinimumFogDistance=5000.00\nMaximumFogDistance=9000.00\n");
    let light = Lighting::of(&scene);
    // A flat stretch of the stadium floor, 256 feet a side, looked straight down on.
    let chunks = [flat_chunk(16, 16.0, 1, 1)];
    let camera = Camera { eye: v3(384.0, 200.0, 383.0), at: v3(384.0, 0.0, 384.0), fov: 0.9, near: 1.0, far: 1000.0 };
    let (w, h) = (256u32, 256u32);
    let mut shoot = |paint: GroundPaint| {
        gpu.set_scene(&scene);
        gpu.set_sky(None);
        gpu.set_ground_painted(&chunks, paint, dirt).unwrap();
        let texture = gpu.offscreen(w, h);
        let view = texture.create_view(&Default::default());
        let views = [View { rect: [0, 0, w, h], camera: &camera, actors: &[] }];
        gpu.render(
            &Target { view: &view, size: (w, h), frame: [0, 0, w, h], scale: 1 },
            &Assets::default(),
            &Scene3 { views: &views, poses: &[], overlay: &[], sprites: &[], light: &light },
        );
        gpu.read(&texture)
    };
    let bare = spread(&shoot(GroundPaint::Bare));
    let paged = spread(&shoot(p.paint()));
    println!("pixel spread: bare {bare:.2}, sx01's pages {paged:.2}");
    assert!(bare < 1.0, "the bare ground is one colour: {bare}");
    assert!(paged > 4.0, "sx01's dirt is textured: {paged}");
}
