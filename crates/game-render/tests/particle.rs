//! Particles in software (P62): the rasteriser draws the GPU's camera-facing squares by the same
//! rules — blended, depth-tested with no depth written, alpha doubled, cut from the sheet's cell,
//! sheet top up, fogged by distance.

use game_render::mat::v3;
use game_render::particle::{Particle, draw};
use game_render::world::{Camera, Lighting};
use game_render::{Frame, Texture};

const W: usize = 64;
const H: usize = 64;
const GREY: [u8; 3] = [100, 100, 100];

/// Looking down -z from the origin, level.
fn camera() -> Camera {
    Camera { eye: v3(0.0, 0.0, 0.0), at: v3(0.0, 0.0, -1.0), fov: 1.0, near: 0.05, far: 4000.0 }
}

/// No fog anywhere near.
fn clear() -> Lighting {
    Lighting {
        sun: v3(0.0, 1.0, 0.0),
        sun_color: [1.0; 3],
        ambient: [1.0; 3],
        fog_color: [0, 0, 255],
        fog_near: 1.0e6,
        fog_far: 2.0e6,
    }
}

/// An 8x8 sheet of four 4x4 cells: red, green on top; blue, white below; each texel's alpha `a`.
fn sheet(a: u8) -> Texture {
    let mut rgba = Vec::new();
    for y in 0..8 {
        for x in 0..8 {
            let c = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]][y / 4 * 2 + x / 4];
            rgba.extend_from_slice(&[c[0], c[1], c[2], a]);
        }
    }
    Texture { width: 8, height: 8, rgba }
}

fn pixel(f: &Frame, x: usize, y: usize) -> [u8; 3] {
    let at = (y * f.width + x) * 3;
    [f.color[at], f.color[at + 1], f.color[at + 2]]
}

/// One square of `size` feet, ten feet ahead, from the whole sheet's top-left cell.
fn ahead(size: f32, alpha: f32) -> Particle {
    Particle { at: [0.0, 0.0, -10.0], size, uv: [0.0, 0.0, 0.5, 0.5], alpha }
}

#[test]
fn an_opaque_texel_at_half_alpha_doubles_to_the_texel() {
    let mut f = Frame::new(W, H, GREY);
    let n = draw(&mut f, &camera(), &sheet(255), &[ahead(4.0, 0.5)], &clear());
    assert_eq!(n, 1);
    assert_eq!(pixel(&f, 32, 32), [255, 0, 0]);
    // A square four feet across at ten feet, through a one-radian view, is about a quarter of the
    // frame: the corners are untouched.
    assert_eq!(pixel(&f, 2, 2), GREY);
    // And it writes no depth.
    assert!(f.depth.iter().all(|&d| d == 0.0));
}

#[test]
fn a_faint_particle_mixes_into_the_frame() {
    let mut f = Frame::new(W, H, GREY);
    // Texel alpha 128 of 255, times a quarter, times two: a quarter of the way to red.
    draw(&mut f, &camera(), &sheet(128), &[ahead(4.0, 0.25)], &clear());
    let [r, g, b] = pixel(&f, 32, 32);
    let a = (128.0f32 * 0.25 * 2.0) as u8 as f32 / 255.0;
    let want = |under: f32, over: f32| (under + (over - under) * a) as u8;
    assert_eq!([r, g, b], [want(100.0, 255.0), want(100.0, 0.0), want(100.0, 0.0)]);
    // Once over every pixel it covers: no darker diagonal where the GPU's two triangles meet.
    let drawn: Vec<[u8; 3]> = f.color.chunks(3).filter(|&c| c != GREY).map(|c| [c[0], c[1], c[2]]).collect();
    assert!(drawn.len() > 100);
    assert!(drawn.iter().all(|&c| c == [r, g, b]));
}

#[test]
fn the_sheet_top_is_the_view_up_and_its_left_the_view_left() {
    let mut f = Frame::new(W, H, GREY);
    // The whole sheet on one square: red top left, green top right, blue bottom left, white bottom right.
    let p = Particle { uv: [0.0, 0.0, 1.0, 1.0], ..ahead(4.0, 1.0) };
    draw(&mut f, &camera(), &sheet(255), &[p], &clear());
    assert_eq!(pixel(&f, 28, 28), [255, 0, 0]);
    assert_eq!(pixel(&f, 36, 28), [0, 255, 0]);
    assert_eq!(pixel(&f, 28, 36), [0, 0, 255]);
    assert_eq!(pixel(&f, 36, 36), [255, 255, 255]);
}

#[test]
fn a_cell_never_reads_its_neighbour() {
    let mut f = Frame::new(W, H, GREY);
    // The bottom-right cell alone: white to every edge of the square.
    let p = Particle { uv: [0.5, 0.5, 1.0, 1.0], ..ahead(4.0, 1.0) };
    draw(&mut f, &camera(), &sheet(255), &[p], &clear());
    let drawn: Vec<[u8; 3]> = (0..H).flat_map(|y| (0..W).map(move |x| (x, y))).map(|(x, y)| pixel(&f, x, y)).filter(|&c| c != GREY).collect();
    assert!(!drawn.is_empty());
    assert!(drawn.iter().all(|&c| c == [255, 255, 255]), "{:?}", drawn.iter().find(|&&c| c != [255, 255, 255]));
}

#[test]
fn a_nearer_surface_hides_it_and_nothing_behind_the_eye_is_drawn() {
    let mut f = Frame::new(W, H, GREY);
    // Something at five feet already in the depth buffer: 1/w larger than the particle's 1/10.
    f.depth.iter_mut().for_each(|d| *d = 1.0 / 5.0);
    draw(&mut f, &camera(), &sheet(255), &[ahead(4.0, 1.0)], &clear());
    assert_eq!(pixel(&f, 32, 32), GREY);

    let mut f = Frame::new(W, H, GREY);
    let behind = Particle { at: [0.0, 0.0, 10.0], ..ahead(4.0, 1.0) };
    assert_eq!(draw(&mut f, &camera(), &sheet(255), &[behind], &clear()), 0);
    assert!(f.color.chunks(3).all(|c| c == GREY));
}

#[test]
fn it_faces_the_camera_from_any_side() {
    // Looking along +x from the far side of the particle, and straight down on it: a square either way.
    for camera in [
        Camera { eye: v3(-10.0, 0.0, -10.0), at: v3(0.0, 0.0, -10.0), ..camera() },
        Camera { eye: v3(0.0, 10.0, -10.0), at: v3(0.0, 0.0, -10.0001), ..camera() },
    ] {
        let mut f = Frame::new(W, H, GREY);
        draw(&mut f, &camera, &sheet(255), &[ahead(4.0, 1.0)], &clear());
        assert_eq!(pixel(&f, 32, 32), [255, 0, 0]);
        let row = (0..W).filter(|&x| pixel(&f, x, 32) != GREY).count();
        let column = (0..H).filter(|&y| pixel(&f, 32, y) != GREY).count();
        assert!(row > 8 && row.abs_diff(column) <= 1, "{row} across, {column} down");
    }
}

#[test]
fn far_particles_take_the_fog() {
    let fog = Lighting { fog_near: 0.0, fog_far: 20.0, ..clear() };
    let mut f = Frame::new(W, H, GREY);
    // Ten feet into a twenty-foot fog: halfway from red to the fog's blue.
    draw(&mut f, &camera(), &sheet(255), &[ahead(4.0, 1.0)], &fog);
    assert_eq!(pixel(&f, 32, 32), [127, 0, 127]);
}

#[test]
fn no_alpha_no_size_or_no_particles_draw_nothing() {
    let mut f = Frame::new(W, H, GREY);
    let n = draw(&mut f, &camera(), &sheet(255), &[ahead(4.0, 0.0), ahead(0.0, 1.0)], &clear());
    assert_eq!(n, 0);
    assert_eq!(draw(&mut f, &camera(), &sheet(255), &[], &clear()), 0);
    // A transparent texel is not drawn however opaque the particle.
    draw(&mut f, &camera(), &sheet(0), &[ahead(4.0, 1.0)], &clear());
    assert!(f.color.chunks(3).all(|c| c == GREY));
}
