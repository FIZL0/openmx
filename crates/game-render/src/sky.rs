//! `.cub` — a track's sky: a cube around the camera, painted in vertex colours.
//!
//! The scene names it (`[Environment] CubeFile=sky89.cub`, which is `skies\sky89.ps2.cub` in the
//! track's archive), and `tex\skyasgeometry.psm` says how it is drawn: `Vu1StripSkyUntextured`, no
//! texture at all. The clouds are in the vertex colours of a dense mesh — 3090 strips on `free09`.
//!
//! The file is a ready-made VIF stream. It is cut into 96 packets, each starting
//! `00 00 00 00 09 02 00 00`, and each packet is one patch of the cube: a run of strips, every strip
//! an `UNPACK V2-8` of positions and an `UNPACK V3-8` of the same number of RGB colours, then an
//! `MSCNT` to draw it.
//!
//! ```text
//! code  imm lo, imm hi (0xc0), count (<= 40), 0x66   then count x (u, v), bytes, 0..=128 across the patch
//! code  imm lo, imm hi (0xc0), count, 0x6a           then count x (r, g, b)
//! ```
//!
//! Sixteen packets make a face, four by four from the top left, and the six faces come in the order
//! four sides, then the top, then the bottom. Which way each face runs was found by matching edges —
//! side 0's right edge is side 3's left one, the four sides' top edges are the top face's four edges
//! — and gives one consistent cube: sides 0 to 3 face +z, -x, -z, +x in the cube's own frame, each with
//! its rows running down from the top. The lower half of every side and the whole bottom face are
//! the scene's fog colour, so the sky meets the fogged ground at the horizon by construction.
//!
//! The engine builds the same cube when it loads a sky: six faces of five by five corners, 25600 feet
//! from the middle, in the world's own axes, in this order and running these ways (V25 read it in the
//! decompiled loader; the journal has where). So the cube needs no turning, and the clouds over the
//! human's save state land where the original's frame has them. The cube is drawn around the camera
//! here; at 25600 feet the difference from wherever the original centres it is not a pixel. The
//! packets' own headers and the file's 92-byte preamble are not read.

use crate::disc::Res;
use crate::ini;
use crate::mat::{V3, v3};
use crate::pst::Texture;
use crate::raster::Frame;
use crate::world::Camera;

/// Pixels a patch is painted into along each side; a face is four patches across.
const PATCH: usize = 64;
const FACE: usize = 4 * PATCH;

/// A decoded sky.
pub struct Sky {
    /// Sides 0 to 3, the top and the bottom, each `FACE` square.
    pub faces: Vec<Texture>,
}

impl Sky {
    /// Decode a `.cub`.
    pub fn parse(d: &[u8]) -> Result<Sky, String> {
        let packets: Vec<usize> = (0..d.len().saturating_sub(8))
            .filter(|&i| d[i..i + 8] == [0, 0, 0, 0, 9, 2, 0, 0])
            .collect();
        if packets.len() != 96 {
            return Err(format!("a sky is 96 packets, this has {}", packets.len()));
        }
        let mut faces: Vec<Texture> = (0..6)
            .map(|_| Texture {
                width: FACE,
                height: FACE,
                rgba: vec![0; FACE * FACE * 4],
            })
            .collect();
        let mut strips = 0;
        let mut i = packets[0];
        while i + 4 <= d.len() {
            let n = d[i + 2] as usize;
            let positions = d[i + 1] == 0xc0 && d[i + 3] == 0x66 && (1..=40).contains(&n);
            if !positions || i + 4 + 2 * n > d.len() {
                i += 1;
                continue;
            }
            let uv = &d[i + 4..i + 4 + 2 * n];
            let after = i + 4 + 2 * n;
            // The colours' unpack follows within a word or two of padding.
            let Some(c) = (after..(after + 12).min(d.len().saturating_sub(3)))
                .find(|&j| d[j + 1] == 0xc0 && d[j + 2] as usize == n && d[j + 3] == 0x6a)
            else {
                i += 1;
                continue;
            };
            let rgb = &d[c + 4..(c + 4 + 3 * n).min(d.len())];
            if rgb.len() < 3 * n {
                break;
            }
            let packet = packets.partition_point(|&p| p < i) - 1;
            let (face, patch) = (packet / 16, packet % 16);
            let origin = ((patch % 4) * PATCH, (patch / 4) * PATCH);
            let scale = PATCH as f32 / 128.0;
            let at = |k: usize| {
                (
                    origin.0 as f32 + uv[2 * k] as f32 * scale,
                    origin.1 as f32 + uv[2 * k + 1] as f32 * scale,
                    [rgb[3 * k] as f32, rgb[3 * k + 1] as f32, rgb[3 * k + 2] as f32],
                )
            };
            for k in 2..n {
                paint(&mut faces[face], [at(k - 2), at(k - 1), at(k)]);
            }
            strips += 1;
            i = c + 4 + 3 * n;
        }
        if strips == 0 {
            return Err("a sky with no strips".to_string());
        }
        Ok(Sky { faces })
    }

    /// The sky a scene names in its `[Environment]`, from the archives that hold it; `None` if it
    /// names none or the file does not decode.
    pub fn load(archives: &[Res], scene_text: &str) -> Option<Sky> {
        let cube = ini::parse(scene_text)
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case("Environment"))?
            .get("CubeFile")?
            .trim()
            .to_ascii_lowercase();
        let stem = cube.strip_suffix(".cub").unwrap_or(&cube);
        let bytes = archives
            .iter()
            .find_map(|a| a.read_suffix(&format!("{stem}.ps2.cub")).or_else(|_| a.read_suffix(&cube)).ok())?;
        Sky::parse(&bytes).ok()
    }

    /// The sky's colour in a direction of the world.
    pub fn color(&self, dir: V3) -> [u8; 3] {
        let d = dir;
        let (ax, ay, az) = (d.x.abs(), d.y.abs(), d.z.abs());
        // Which face, and where on it from its top left, 0..1 each way.
        let (face, u, v) = if ay >= ax && ay >= az {
            if d.y > 0.0 {
                (4, d.x / ay, d.z / ay)
            } else {
                (5, d.x / ay, -d.z / ay)
            }
        } else if az >= ax {
            if d.z > 0.0 {
                (0, d.x / az, -d.y / az)
            } else {
                (2, -d.x / az, -d.y / az)
            }
        } else if d.x < 0.0 {
            (1, d.z / ax, -d.y / ax)
        } else {
            (3, -d.z / ax, -d.y / ax)
        };
        let f = &self.faces[face];
        let px = (((u + 1.0) * 0.5 * f.width as f32) as usize).min(f.width - 1);
        let py = (((v + 1.0) * 0.5 * f.height as f32) as usize).min(f.height - 1);
        let at = 4 * (py * f.width + px);
        [f.rgba[at], f.rgba[at + 1], f.rgba[at + 2]]
    }

    /// Paint the sky behind a frame: every pixel nothing has been drawn into yet.
    pub fn draw(&self, frame: &mut Frame, camera: &Camera) {
        let (w, h) = (frame.width, frame.height);
        let vp = camera.view_projection(w, h);
        let forward = camera.at.sub(camera.eye).norm();
        let mut right = forward.cross(v3(0.0, 1.0, 0.0)).norm();
        // Whichever way the cross product came out, make it the screen's right.
        let probe = vp.clip(camera.eye.add(forward).add(right.scale(0.1)));
        if probe[0] / probe[3] < 0.0 {
            right = right.scale(-1.0);
        }
        let mut up = right.cross(forward).norm();
        let probe = vp.clip(camera.eye.add(forward).add(up.scale(0.1)));
        if probe[1] / probe[3] < 0.0 {
            up = up.scale(-1.0);
        }
        let half_h = (camera.fov / 2.0).tan();
        let half_w = half_h * w as f32 / h as f32;
        for y in 0..h {
            let ny = 1.0 - 2.0 * (y as f32 + 0.5) / h as f32;
            for x in 0..w {
                let at = y * w + x;
                if frame.depth[at] > 0.0 {
                    continue;
                }
                let nx = 2.0 * (x as f32 + 0.5) / w as f32 - 1.0;
                let dir = forward
                    .add(right.scale(nx * half_w))
                    .add(up.scale(ny * half_h));
                frame.color[at * 3..at * 3 + 3].copy_from_slice(&self.color(dir));
            }
        }
    }
}

/// One Gouraud triangle into a face, in the face's pixels.
fn paint(face: &mut Texture, v: [(f32, f32, [f32; 3]); 3]) {
    let area = (v[1].0 - v[0].0) * (v[2].1 - v[0].1) - (v[2].0 - v[0].0) * (v[1].1 - v[0].1);
    if area.abs() < 1e-6 {
        return;
    }
    let lo_x = v.iter().map(|p| p.0).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
    let hi_x = v.iter().map(|p| p.0).fold(f32::MIN, f32::max).ceil().min(face.width as f32 - 1.0) as usize;
    let lo_y = v.iter().map(|p| p.1).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
    let hi_y = v.iter().map(|p| p.1).fold(f32::MIN, f32::max).ceil().min(face.height as f32 - 1.0) as usize;
    for y in lo_y..=hi_y {
        for x in lo_x..=hi_x {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = ((v[1].0 - px) * (v[2].1 - py) - (v[2].0 - px) * (v[1].1 - py)) / area;
            let w1 = ((v[2].0 - px) * (v[0].1 - py) - (v[0].0 - px) * (v[2].1 - py)) / area;
            let w2 = 1.0 - w0 - w1;
            // A hair of slack so the strips' shared edges leave no gaps.
            if w0 < -0.01 || w1 < -0.01 || w2 < -0.01 {
                continue;
            }
            let at = 4 * (y * face.width + x);
            for k in 0..3 {
                let c = w0 * v[0].2[k] + w1 * v[1].2[k] + w2 * v[2].2[k];
                face.rgba[at + k] = c.clamp(0.0, 255.0) as u8;
            }
            face.rgba[at + 3] = 255;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sky of 96 packets, each one strip of a single triangle over the patch's top left, coloured
    /// by which face it is on.
    fn tiny() -> Vec<u8> {
        let mut d = vec![0u8; 92];
        for packet in 0..96u8 {
            d.extend_from_slice(&[0, 0, 0, 0, 9, 2, 0, 0]);
            d.extend_from_slice(&[0; 24]);
            d.extend_from_slice(&[1, 0xc0, 3, 0x66, 0, 0, 128, 0, 0, 128]);
            d.extend_from_slice(&[0, 0]);
            let c = 40 * (packet / 16);
            d.extend_from_slice(&[0x13, 0xc0, 3, 0x6a, c, c, c, c, c, c, c, c, c]);
            d.extend_from_slice(&[0, 0, 0, 0x17]);
        }
        d
    }

    #[test]
    fn a_sky_is_six_faces_of_sixteen_patches() {
        let sky = Sky::parse(&tiny()).unwrap();
        assert_eq!(sky.faces.len(), 6);
        // Face 2's first patch, near its top left corner.
        assert_eq!(sky.faces[2].rgba[4 * (2 * FACE + 2)], 80);
        // The top face straight up, which is where four of its patches meet: patch 10's top left
        // corner is painted, colour 4 x 40.
        assert_eq!(sky.color(v3(0.0001, 1.0, 0.0001)), [160, 160, 160]);
    }

    #[test]
    fn looking_along_each_side_finds_that_side() {
        let mut sky = Sky::parse(&tiny()).unwrap();
        for (i, f) in sky.faces.iter_mut().enumerate() {
            f.rgba.chunks_mut(4).for_each(|p| p.copy_from_slice(&[i as u8, 0, 0, 255]));
        }
        assert_eq!(sky.color(v3(0.0, 0.0, 1.0))[0], 0);
        assert_eq!(sky.color(v3(-1.0, 0.0, 0.0))[0], 1);
        assert_eq!(sky.color(v3(0.0, 0.0, -1.0))[0], 2);
        assert_eq!(sky.color(v3(1.0, 0.0, 0.0))[0], 3);
        assert_eq!(sky.color(v3(0.0, -1.0, 0.0))[0], 5);
    }

    #[test]
    fn something_that_is_not_a_sky_is_refused() {
        assert!(Sky::parse(&[0u8; 400]).is_err());
    }
}
