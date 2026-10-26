//! V77: free06's (Dallas's) slopes and the flats at their feet are as light, against the rest of the
//! ground, as in the original.
//!
//! The spots are pixels of four PCSX2 shots taken on free06 (journal 2026-10-26 V77): each one the
//! ground point a ray through the middle of one cell of an 8x6 grid meets, and the grey of a 40x40
//! patch there in a 936x702 shot, HUD, rider and sky left out. The port's grey at a spot is the
//! paint the shell lays there, lit by the baked light map, averaged over a few feet as the patch is.
//!
//! The original's ground comes out about 0.7 of the port's everywhere (the sky between them is the
//! same), which is not this task's, so one scale is fitted over every spot and each spot is held
//! to it. Before V77 the erosion mesh painted the cliff feet a dark slab at its texture's brown:
//! the same spots then miss the original by twice as much.
//!
//! Needs the user's own disc, so it skips with the reason when it is not there.

use game_render::disc::{Disc, Res};
use game_render::ground_mesh::GroundMeshes;
use game_render::scene::Scene;
use game_render::splat::Splat;

/// A spot: which shot and cell, the world `x` and `z` the cell's middle ray meets the ground at, and
/// the original's grey there.
struct Spot {
    name: &'static str,
    x: f32,
    z: f32,
    original: f32,
}

const fn spot(name: &'static str, x: f32, z: f32, original: f32) -> Spot {
    Spot { name, x, z, original }
}

const SPOTS: &[Spot] = &[
    // p3: the cliff north of the start line, eroded, and the flat at its foot under the band.
    spot("p3 cliff 1,0", 2831.0, 3215.0, 86.0),
    spot("p3 cliff 0,4", 2842.0, 3218.0, 83.0),
    spot("p3 cliff 1,5", 2852.0, 3209.0, 73.0),
    spot("p3 cliff 1,7", 2870.0, 3207.0, 93.0),
    spot("p3 foot 2,1", 2823.0, 3188.0, 77.0),
    spot("p3 foot 2,3", 2835.0, 3190.0, 102.0),
    spot("p3 foot 2,5", 2849.0, 3194.0, 99.0),
    spot("p3 flat 3,5", 2845.0, 3178.0, 100.0),
    // r1: the shaded slope west of the start, which the light map darkens most, and the flat below.
    spot("r1 slope 0,2", 3360.0, 3129.0, 36.0),
    spot("r1 slope 0,4", 3334.0, 3149.0, 33.0),
    spot("r1 slope 1,3", 3362.0, 3141.0, 39.0),
    spot("r1 slope 1,5", 3339.0, 3164.0, 36.0),
    spot("r1 flat 2,5", 3376.0, 3160.0, 76.0),
    spot("r1 flat 3,2", 3397.0, 3147.0, 76.0),
    // r3: the flat east of the start at the foot of the bushy cliff, all under the erosion band.
    spot("r3 cliff 1,1", 3446.0, 3432.0, 82.0),
    spot("r3 foot 2,1", 3411.0, 3395.0, 112.0),
    spot("r3 foot 2,2", 3414.0, 3389.0, 112.0),
    spot("r3 foot 3,1", 3400.0, 3382.0, 120.0),
    spot("r3 foot 3,2", 3401.0, 3379.0, 110.0),
    spot("r3 flat 3,0", 3399.0, 3386.0, 120.0),
    // r4: the bank north-west of the start, half under the band, and the flat at its foot.
    spot("r4 bank 1,0", 2948.0, 3756.0, 79.0),
    spot("r4 bank 1,2", 2913.0, 3707.0, 76.0),
    spot("r4 bank 1,5", 2791.0, 3757.0, 55.0),
    spot("r4 bank 1,6", 2779.0, 3763.0, 50.0),
    spot("r4 foot 2,4", 2788.0, 3789.0, 85.0),
    spot("r4 foot 2,5", 2782.0, 3785.0, 102.0),
    spot("r4 flat 2,1", 2806.0, 3801.0, 109.0),
    spot("r4 shade 0,5", 2837.0, 3605.0, 35.0),
];

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
            let member = disc.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(want.as_str()))?.name.clone();
            Some(disc.res(&member).expect("the track's archive"))
        })
        .collect()
}

/// The lit ground's grey round a point: paint times light, averaged over 9x9 points 1 ft apart.
fn grey(splat: &Splat, x: f32, z: f32) -> f32 {
    let mut sum = 0.0;
    for j in -4..=4 {
        for i in -4..=4 {
            let (px, pz) = (x + i as f32, z + j as f32);
            let c = splat.paint(px, pz);
            let luma = 0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32;
            sum += luma * splat.light(px, pz).unwrap_or(1.0);
        }
    }
    sum / 81.0
}

/// The scale that best takes the port's greys to the original's, and each spot's miss after it, as a
/// fraction of the original's grey.
fn fit(port: &[f32]) -> (f32, Vec<f32>) {
    let k = SPOTS.iter().zip(port).map(|(s, p)| s.original * p).sum::<f32>() / port.iter().map(|p| p * p).sum::<f32>();
    (k, SPOTS.iter().zip(port).map(|(s, p)| (k * p - s.original).abs() / s.original).collect())
}

#[test]
fn dallas_slopes_are_as_light_as_the_originals() {
    let Some(mut disc) = disc() else { return };
    let archives = archives(&mut disc, "free06");
    let mut splat = Splat::load(&archives, "free06", (6903.0, 6903.0)).expect("free06's paint");
    let port: Vec<f32> = SPOTS.iter().map(|s| grey(&splat, s.x, s.z)).collect();
    let (k, miss) = fit(&port);
    for ((s, p), m) in SPOTS.iter().zip(&port).zip(&miss) {
        println!("{:14} ({:4},{:4}) original {:3} port {:5.1} x {k:.2} = {:5.1}  {:3.0}%", s.name, s.x, s.z, s.original, p, k * p, m * 100.0);
    }
    let mean = miss.iter().sum::<f32>() / miss.len() as f32;
    println!("scale {k:.2}, mean miss {:.1}%", mean * 100.0);

    // The erosion mesh laid back over the paint, as before V77.
    let text = game_render::ini::text(&archives[0].read_suffix("free06.scn").expect("the scene"));
    let scene = Scene::parse(&text);
    let mut placements = scene.ground_meshes.clone();
    placements.retain(|p| !game_render::ground_mesh::is_light_map(&p.model));
    assert!(placements.iter().any(|p| p.model.eq_ignore_ascii_case("free06_erode")), "the scene places the erosion mesh");
    splat.meshes = GroundMeshes::load(&archives, &placements);
    let before: Vec<f32> = SPOTS.iter().map(|s| grey(&splat, s.x, s.z)).collect();
    let (k_before, miss_before) = fit(&before);
    let mean_before = miss_before.iter().sum::<f32>() / miss_before.len() as f32;
    println!("with the erosion mesh: scale {k_before:.2}, mean miss {:.1}%", mean_before * 100.0);

    assert!((0.55..0.9).contains(&k), "the original's ground is {k:.2} of the port's");
    assert!(mean < 0.15, "the spots miss the original by {:.1}% on average", mean * 100.0);
    for (s, m) in SPOTS.iter().zip(&miss) {
        assert!(*m < 0.4, "{} at ({}, {}) misses the original's {} by {:.0}%", s.name, s.x, s.z, s.original, m * 100.0);
    }
    assert!(mean * 1.4 < mean_before, "the erosion mesh missed by {:.1}%, now {:.1}%", mean_before * 100.0, mean * 100.0);
}
