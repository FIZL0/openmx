//! Sun flares (V72): the glare a track's `.toy` puts where its sun is, laid over the picture when the
//! camera looks that way.
//!
//! **The list.** `[SunFlares] NumberOfSunFlares = n`, then `[SunFlare1]` to `[SunFlaren]`, the same
//! rule as the toy's other lists: the count is all the reader goes by. Every track has one section and
//! most a count of 1; free10, nat08, nat11, nat19 and sx04, 06, 10, 15, 16, 19, 22 and 24 say 0 and
//! have none. The original's reader takes, per section:
//! - `TextureMap`, the picture (each track archive carries its `.pst` and a `.psm` beside it);
//! - `LightDirection`, and where that is missing `TargetPosition − Position`: the way the sunlight
//!   travels, normalised. No section on the disc has a `LightDirection`, so the sun is towards
//!   `Position` from `TargetPosition`;
//! - `useToyFlareSettings` (1 where missing), and then `occlusionSampleSize` (16), `flareColor`
//!   (128, 128, 128, 128), `flareAlpha` (40) and `flareScreenSize` (350), which it writes over the
//!   picture's own `OcclusionFlare` filter. The disc's sections spell the colour `FlareRGBA`, which is
//!   not the key the reader asks for, so every flare keeps the default colour, 128 — the GS's 1.0:
//!   the picture's own. nat22's `FlareRGBA = 238, 228, 191` changes nothing. sx14, sx21 and sx23 have
//!   none of these keys and so draw at the defaults: a 350 px flare at 40.
//! - `Saturation`, its zones' `SaturationMinColor`/`SaturationMaxColor`, `MinScale`, `MaxScale`,
//!   `Width`, `Height`, `VertexColor`, `RetrieveMaterial`: read, and not drawn here (the journal says
//!   why).
//!
//! **Where it is drawn.** Every frame the flare takes the point twenty feet from the camera towards
//! the sun ([`SunFlare::point`]). Where that point is outside the view's frustum, nothing is drawn;
//! inside, the picture is laid on as a square `2 × flareScreenSize` pixels of the 640 × 448 frame
//! across, centred on the point's place on the screen, added to the frame at `flareAlpha / 128`. The
//! flare's `.psm` says how: the picture is copied into a render target, the `OcclusionFlare` filter
//! samples the depth buffer in an `occlusionSampleSize`-pixel square round the point, and the sprite
//! is drawn from the target with no depth test of its own. The pictures are a white sun and its rays
//! on black, so adding is what shows them.
//!
//! **What is the port's own.** Where it stops: the port draws the sprite while any of it is on
//! screen ([`SunFlare::on_screen`]), not only while its point is (V110, the user's ruling). The occlusion. The original reads its own depth buffer; here the
//! caller casts a few rays across that square towards the sun ([`SunFlare::sample_rays`]) and asks
//! what fraction of them reach the sky, and the flare is drawn at that share of its brightness. The
//! sprite is a world-space quad just past the near plane, square to the view, so that both
//! renderers draw it as a [`Glow`].

use crate::glow::{Glow, Vertex};
use crate::ini::{self, Section};
use crate::mat::{V3, v3};
use crate::world::Camera;

/// The original's frame, whose pixels `flareScreenSize` and `occlusionSampleSize` count.
pub const FRAME_HEIGHT: f32 = 448.0;
/// How far towards the sun the point the flare is placed by lies, in feet.
pub const POINT_DISTANCE: f32 = 20.0;
/// How far in front of the eye the port lays its quad, in feet, at the least: half as far again as
/// the near plane where that is further, and short of anything the camera looks over.
pub const QUAD_DEPTH: f32 = 1.0;

/// One `[SunFlareN]` as the original reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct SunFlare {
    /// `TextureMap`, without its extension: the `.pst` in the track's archive.
    pub texture: String,
    /// The way the sunlight travels, normalised: the sun is the other way.
    pub light: V3,
    /// `occlusionSampleSize`, in pixels of the 640 × 448 frame.
    pub sample_size: f32,
    /// `flareColor` as the GS's colour, 128 a channel for the picture's own.
    pub color: [f32; 4],
    /// `flareAlpha`, out of 128.
    pub alpha: f32,
    /// `flareScreenSize`: half the sprite's side, in pixels of the 640 × 448 frame.
    pub screen_size: f32,
}

/// The flares a `.toy` lists, `SunFlare1` to the count its `[SunFlares]` says.
pub fn flares(text: &str) -> Vec<SunFlare> {
    let sections = ini::parse(text);
    let find = |name: &str| sections.iter().find(|s| s.name.eq_ignore_ascii_case(name));
    let count = find("SunFlares").and_then(|s| s.number("NumberOfSunFlares")).unwrap_or(0.0).max(0.0) as usize;
    (1..=count).filter_map(|i| find(&format!("SunFlare{i}")).map(flare)).collect()
}

fn flare(s: &Section) -> SunFlare {
    let point = |key: &str| s.numbers(key).filter(|n| n.len() >= 3).map(|n| v3(n[0], n[1], n[2]));
    let zero = v3(0.0, 0.0, 0.0);
    let light = point("LightDirection")
        .unwrap_or_else(|| point("TargetPosition").unwrap_or(zero).sub(point("Position").unwrap_or(zero)));
    let texture = s.get("TextureMap").unwrap_or("").trim();
    let texture = texture.rsplit_once('.').map_or(texture, |(stem, _)| stem).to_string();
    let mut f = SunFlare {
        texture,
        light: if light.len() > 0.0 { light.norm() } else { light },
        sample_size: 16.0,
        color: [128.0; 4],
        alpha: 40.0,
        screen_size: 350.0,
    };
    let int = |key: &str, default: f32| s.get(key).and_then(|v| v.trim().parse::<i32>().ok()).map_or(default, |v| v as f32);
    // Off, the picture's own filter keeps what its `.psm` read. No section on the disc turns it off,
    // and that reader is not ported: the toy's defaults stand in for it.
    if int("useToyFlareSettings", 1.0) != 0.0 {
        f.sample_size = int("occlusionSampleSize", f.sample_size);
        if let Some(c) = s.numbers("flareColor").filter(|c| c.len() >= 3) {
            f.color = [c[0], c[1], c[2], c.get(3).copied().unwrap_or(f.color[3])];
        }
        f.alpha = int("flareAlpha", f.alpha);
        f.screen_size = int("flareScreenSize", f.screen_size);
    }
    f
}

impl SunFlare {
    /// The point the flare is drawn at: [`POINT_DISTANCE`] from the eye towards the sun.
    pub fn point(&self, camera: &Camera) -> V3 {
        camera.eye.sub(self.light.scale(POINT_DISTANCE))
    }

    /// Whether that point is inside the view's frustum, the only test the original draws it by.
    pub fn in_view(&self, camera: &Camera, width: usize, height: usize) -> bool {
        if self.light.len() == 0.0 {
            return false;
        }
        let [x, y, z, w] = camera.view_projection(width, height).clip(self.point(camera));
        w > 0.0 && x.abs() <= w && y.abs() <= w && (0.0..=w).contains(&z)
    }

    /// Whether any of the sprite is on screen (V110): the square `screen_size` pixels of the
    /// 448-line view either side of the sun's place, tested against the view's edges rather than its
    /// centre, so a flare whose sun has just left the frame is still drawn until the last of it has.
    /// The original draws by [`Self::in_view`] alone and pops the flare off at the edge; this is the
    /// port's, by the user's ruling (2026-10-28).
    pub fn on_screen(&self, camera: &Camera, width: usize, height: usize) -> bool {
        if self.light.len() == 0.0 || width == 0 || height == 0 {
            return false;
        }
        let [x, y, _, w] = camera.view_projection(width, height).clip(self.point(camera));
        if w <= 0.0 {
            return false;
        }
        // The sprite is square to the view, so on screen it is the centre ± its half side: 2 ×
        // screen_size / 448 of the view's height in NDC, and that over the aspect across.
        let half_y = 2.0 * self.screen_size / FRAME_HEIGHT;
        let half_x = half_y * height as f32 / width as f32;
        (x / w).abs() < 1.0 + half_x && (y / w).abs() < 1.0 + half_y
    }

    /// Rays from the eye across the sample square round the sun, at its centre and its four corners:
    /// the caller says what share of them reach the sky. They are world rays walked over the
    /// ground, not reads of the frame's depth, so a sun just past the view's edge is sampled as
    /// truly as one in the middle: the share does not snap as the sun crosses the edge.
    pub fn sample_rays(&self, camera: &Camera) -> Vec<V3> {
        let (right, up, _) = basis(camera);
        let toward = self.light.scale(-1.0);
        // A pixel of the 448-line frame, as an angle's tangent at unit depth.
        let pixel = 2.0 * (camera.fov / 2.0).tan() / FRAME_HEIGHT;
        let half = self.sample_size / 2.0 * pixel;
        [(0.0, 0.0), (-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
            .iter()
            .map(|(i, j)| toward.add(right.scale(i * half)).add(up.scale(j * half)).norm())
            .collect()
    }

    /// The flare as a glow cut from `texture`, at `visible` (0 to 1) of its brightness, or none where
    /// it is not drawn: wholly off screen ([`Self::on_screen`]), or wholly hidden.
    pub fn glow(&self, camera: &Camera, width: usize, height: usize, visible: f32, texture: usize) -> Option<Glow> {
        if visible <= 0.0 || !self.on_screen(camera, width, height) {
            return None;
        }
        let (right, up, forward) = basis(camera);
        let toward = self.light.scale(-1.0);
        let along = toward.dot(forward);
        if along <= 0.0 {
            return None;
        }
        // The sun's place on the plane `depth` in front of the eye, and the sprite's half side
        // there: flareScreenSize pixels of a 448-line frame, which is the view's height.
        let depth = (camera.near * 1.5).max(QUAD_DEPTH);
        let centre = camera.eye.add(toward.scale(depth / along));
        let half = depth * 2.0 * (camera.fov / 2.0).tan() * self.screen_size / FRAME_HEIGHT;
        let tint = [
            self.color[0] / 128.0,
            self.color[1] / 128.0,
            self.color[2] / 128.0,
            (self.alpha / 128.0) * visible.min(1.0),
        ];
        let corner = |i: f32, j: f32, uv: [f32; 2]| {
            let at = centre.add(right.scale(i * half)).add(up.scale(j * half));
            Vertex { at: [at.x, at.y, at.z], uv, tint }
        };
        let (a, b, c, d) = (corner(-1.0, 1.0, [0.0, 0.0]), corner(1.0, 1.0, [1.0, 0.0]), corner(1.0, -1.0, [1.0, 1.0]), corner(-1.0, -1.0, [0.0, 1.0]));
        Some(Glow { texture, additive: true, tris: vec![[a, b, c], [a, c, d]] })
    }
}

/// The view's right, up and forward, as [`crate::mat::M4::look_at`] builds them.
fn basis(camera: &Camera) -> (V3, V3, V3) {
    let forward = camera.at.sub(camera.eye).norm();
    let right = v3(0.0, 1.0, 0.0).cross(forward).norm();
    let up = forward.cross(right);
    (right, up, forward)
}

/// How far above the eye a sample ray has to climb before nothing on a track can stand across it, in
/// feet: the disc's hills and cliffs stand a few hundred over the ground the camera is near.
pub const CEILING: f32 = 1000.0;

/// The share of `rays` from `eye` that nothing in `ground` stands across: each walked out to
/// `reach` feet (or [`CEILING`] above the eye) in steps that grow with distance, and stopped where
/// `ground` (the height of the surface under a point, if any) rises above it.
pub fn sky_share(eye: V3, rays: &[V3], reach: f32, mut ground: impl FnMut(f32, f32, f32) -> Option<f32>) -> f32 {
    if rays.is_empty() {
        return 0.0;
    }
    let clear = rays
        .iter()
        .filter(|ray| {
            let mut t = 1.0;
            while t < reach {
                let p = eye.add(ray.scale(t));
                if p.y > eye.y + CEILING {
                    break;
                }
                if ground(p.x, p.y, p.z).is_some_and(|h| h > p.y) {
                    return false;
                }
                t += (t * 0.15).max(2.0);
            }
            true
        })
        .count();
    clear as f32 / rays.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::Frame;

    const FREE06: &str = "[SunFlares]\nNumberOfSunFlares=1\n\n[SunFlare1]\nTextureMap=SunFlare03.tga\nSaturation=T\n\
        MinScale=0.5\nMaxScale=3\nPosition=4321.700195,1220.199951,2291.199951\n\
        TargetPosition=2880.000000,135.000000,2880.000000\n\nuseToyFlareSettings=1\nOcclusionSampleSize=26\n\
        FlareRGBA=128.0,128.0,128.0,0\nFlareAlpha=50\nFlareScreenSize=200\n";

    fn camera(at: V3) -> Camera {
        Camera { eye: v3(0.0, 0.0, 0.0), at, fov: 1.0, near: 0.05, far: 4000.0 }
    }

    #[test]
    fn reads_free06s() {
        let f = flares(FREE06);
        assert_eq!(f.len(), 1);
        let f = &f[0];
        assert_eq!(f.texture, "SunFlare03");
        assert_eq!((f.sample_size, f.alpha, f.screen_size), (26.0, 50.0, 200.0));
        // FlareRGBA is not the key the original asks for.
        assert_eq!(f.color, [128.0; 4]);
        let sun = v3(4321.700195 - 2880.0, 1220.199951 - 135.0, 2291.199951 - 2880.0).norm();
        assert!(f.light.scale(-1.0).sub(sun).len() < 1e-5);
    }

    #[test]
    fn the_count_is_all_it_goes_by() {
        assert!(flares(&FREE06.replace("NumberOfSunFlares=1", "NumberOfSunFlares=0")).is_empty());
        assert!(flares(&FREE06.replace("NumberOfSunFlares=1", "")).is_empty());
    }

    #[test]
    fn the_defaults_where_keys_are_missing() {
        let bare = "[SunFlares]\nNumberOfSunFlares=1\n[SunFlare1]\nTextureMap=SunFlare02.tga\nPosition=0,10,0\nTargetPosition=0,0,0\n";
        let f = &flares(bare)[0];
        assert_eq!((f.sample_size, f.alpha, f.screen_size, f.color), (16.0, 40.0, 350.0, [128.0; 4]));
        // A LightDirection wins over the two points.
        let lit = format!("{bare}LightDirection=0,0,-2\n");
        assert_eq!(flares(&lit)[0].light, v3(0.0, 0.0, -1.0));
    }

    #[test]
    fn drawn_only_looking_its_way() {
        let f = &flares(FREE06)[0];
        let sun = f.light.scale(-1.0);
        let toward = camera(sun.scale(10.0));
        let away = camera(sun.scale(-10.0));
        assert!(f.in_view(&toward, 640, 448));
        assert!(!f.in_view(&away, 640, 448));
        assert!(f.glow(&away, 640, 448, 1.0, 0).is_none());
        assert!(f.glow(&toward, 640, 448, 0.0, 0).is_none());
        let g = f.glow(&toward, 640, 448, 0.5, 3).unwrap();
        assert!(g.additive && g.texture == 3 && g.tris.len() == 2);
        assert!((g.tris[0][0].tint[3] - 50.0 / 128.0 * 0.5).abs() < 1e-6);
    }

    /// Straight at the sun, the sprite is 2 × flareScreenSize lines of a 448-line view tall, centred.
    #[test]
    fn its_size_on_screen() {
        let f = &flares(FREE06)[0];
        let cam = camera(f.light.scale(-1.0));
        let g = f.glow(&cam, 640, 448, 1.0, 0).unwrap();
        let m = cam.view_projection(640, 448);
        let row = |v: &Vertex| {
            let [_, y, _, w] = m.clip(v3(v.at[0], v.at[1], v.at[2]));
            (1.0 - y / w) * 224.0
        };
        let (top, bottom) = (row(&g.tris[0][0]), row(&g.tris[0][2]));
        assert!(((bottom - top).abs() - 400.0).abs() < 0.1, "{top} {bottom}");
        assert!(((top + bottom) / 2.0 - 224.0).abs() < 0.1);
    }

    #[test]
    fn a_hill_hides_it() {
        let f = &flares(FREE06)[0];
        let cam = camera(f.light.scale(-1.0));
        let rays = f.sample_rays(&cam);
        assert_eq!(rays.len(), 5);
        assert_eq!(sky_share(cam.eye, &rays, 3000.0, |_, _, _| Some(-5.0)), 1.0);
        assert_eq!(sky_share(cam.eye, &rays, 3000.0, |_, _, _| None), 1.0);
        // A wall of ground 1000 ft high everywhere past 100 ft out.
        let wall = |x: f32, _: f32, z: f32| Some(if x * x + z * z > 100.0 * 100.0 { 1000.0 } else { -5.0 });
        assert_eq!(sky_share(cam.eye, &rays, 3000.0, wall), 0.0);
        // A ridge that stands across the lower row only.
        let toward = f.light.scale(-1.0);
        let ridge_top = toward.scale(200.0 / toward.x.hypot(toward.z)).y;
        let ridge = |x: f32, _: f32, z: f32| Some(if x.hypot(z) > 199.0 && x.hypot(z) < 260.0 { ridge_top } else { -5.0 });
        let share = sky_share(cam.eye, &rays, 3000.0, ridge);
        assert!(share > 0.0 && share < 1.0, "{share}");
    }

    /// A flare whose sun is level along +z, and a camera at the origin turned `yaw` radians off it.
    fn level(yaw: f32) -> (SunFlare, Camera) {
        let mut f = flares(FREE06).remove(0);
        f.light = v3(0.0, 0.0, -1.0);
        (f, camera(v3(-yaw.sin(), 0.0, yaw.cos())))
    }

    /// The sun's place on screen in NDC, and the sprite's half width there.
    fn ndc_x(f: &SunFlare, cam: &Camera, w: usize, h: usize) -> (f32, f32) {
        let [x, _, _, cw] = cam.view_projection(w, h).clip(f.point(cam));
        (x / cw, 2.0 * f.screen_size / FRAME_HEIGHT * h as f32 / w as f32)
    }

    /// The first yaw, in steps of a thousandth of a radian, at which `pass` holds.
    fn first_yaw(mut pass: impl FnMut(f32) -> bool) -> f32 {
        (0..1600).map(|i| i as f32 * 0.001).find(|&a| pass(a)).expect("never")
    }

    /// V110: a sun just past the right edge still draws its glow; turned until the whole sprite is
    /// past it, none — in a full view, both split shapes, and through the software renderer.
    #[test]
    fn drawn_until_the_whole_sprite_is_off_screen() {
        for (w, h) in [(640, 448), (320, 448), (640, 224)] {
            // The sun's centre just past the right edge.
            let past = first_yaw(|a| { let (f, c) = level(a); !f.in_view(&c, w, h) });
            let (f, cam) = level(past + 0.002);
            let (x, half) = ndc_x(&f, &cam, w, h);
            assert!(x > 1.0 && x < 1.0 + half, "{w}x{h}: {x} {half}");
            let g = f.glow(&cam, w, h, 1.0, 0).unwrap_or_else(|| panic!("{w}x{h}: no glow at {x}"));
            // The software renderer lays the part still on screen along the right edge.
            let mut frame = Frame::new(w, h, [0; 3]);
            assert_eq!(crate::glow::draw(&mut frame, &cam, &[], &[g]), 2);
            let lit = |col: usize| (0..h).any(|row| frame.color[(row * w + col) * 3] > 0);
            assert!(lit(w - 1) && !lit(0), "{w}x{h}");
            // Turned on until the sprite's near edge has gone too.
            let gone = first_yaw(|a| { let (f, c) = level(a); let (x, half) = ndc_x(&f, &c, w, h); x - half >= 1.0 });
            let (f, cam) = level(gone - 0.002);
            assert!(f.glow(&cam, w, h, 1.0, 0).is_some(), "{w}x{h}: gone early");
            let (f, cam) = level(gone + 0.002);
            assert!(!f.on_screen(&cam, w, h) && f.glow(&cam, w, h, 1.0, 0).is_none(), "{w}x{h}");
        }
    }

    /// Off the edge the sample rays still read the sky as they would in the middle of the view:
    /// they point at the sun, not into the frame.
    #[test]
    fn rays_past_the_edge_read_the_sky() {
        let (f, straight) = level(0.0);
        let past = first_yaw(|a| { let (f, c) = level(a); !f.in_view(&c, 640, 448) });
        let (_, edge) = level(past + 0.01);
        let (a, b) = (f.sample_rays(&straight), f.sample_rays(&edge));
        assert!(a[0].sub(b[0]).len() < 1e-6 && b[0].sub(v3(0.0, 0.0, 1.0)).len() < 1e-6);
        // A ridge across the lower half of the sample square, 200 ft out.
        let ridge = |_: f32, _: f32, z: f32| Some(if z > 199.0 && z < 260.0 { 0.0 } else { -50.0 });
        let (sa, sb) = (sky_share(straight.eye, &a, 3000.0, ridge), sky_share(edge.eye, &b, 3000.0, ridge));
        assert!(sa > 0.0 && sa < 1.0 && sa == sb, "{sa} {sb}");
    }
}
