//! The freeride area's ground: the port's own terrain query, asked about the world instead of about a
//! sample.
//!
//! D6 ported the game's height-and-surface query — the tree descent, the block lookup and the
//! dequantisation, with the `vertical_scale == 1` short path the original takes — and proved it against
//! `free06`. What that query answers is "the four corners of the cell at sample (x, z)". A rider needs
//! two things on top of it, and both are this module's rather than the port's:
//!
//! * **Where between the samples they are.** That blend is the ride's question, not the picture's, and
//!   it lives beside the ride in [`game_sim::world::Ground`] (P2) — still not the original's
//!   arithmetic, and still saying so there. This module only draws.
//! * **A mesh to draw.** The renderer's own `.trn` reader (V1) reads the supercross layout, and the
//!   off-road tracks — `free06` among them — are laid out the other way and refused by it. So the
//!   ground drawn here is built from D6's samples: the ones around the rider, as two triangles a cell,
//!   lit by the stored normals and faded into the scene's fog.
//!
//! Heights come out of the port as [`ps2_float::F`] and are used here as host floats: the shell draws
//! and places a camera with them, and nothing bit-exact depends on what happens after `.0`.

use game_render::mat::{V3, v3};
use game_render::raster::{Frame, Look, Shade};
use game_render::splat::Splat;
use game_render::world::{Camera, Lighting};
use game_sim::track::{Terrain, TerrainLayout};

/// How many samples a terrain tree's leaf holds along one side, which is also how many cells: the
/// shared seventeenth sample belongs to the next leaf along.
const LEAF_SAMPLES: u32 = 16;

/// The colour the ground is painted where the track has no paint of its own ([`Splat`]).
///
/// The `.trn` carries a 1 MiB image and a 256-colour palette for it, but which of its tiles a cell
/// takes and how the cell's corners index into one is not decoded (V1 says so too), so the ground is
/// one dirt colour with the file's own normals doing all the shaping. It reads as dirt and it is
/// honest about being flat.
const DIRT: [f32; 3] = [150.0, 118.0, 86.0];

/// The terrain, and what the scene says about how big it is.
pub struct Ground<'a> {
    terrain: Terrain<'a>,
    /// World units between neighbouring samples, which is also local height to world height.
    pub spacing: f32,
    /// How many samples the field holds across and along.
    pub samples_x: u32,
    pub samples_z: u32,
    /// The track's own ground paint and baked light (V25), where it has them.
    pub paint: Option<Splat>,
}

impl<'a> Ground<'a> {
    /// The ground of one track: its terrain file, and the scene's own statement of how many root
    /// cells it has.
    ///
    /// The sample count is the tree's shape rather than a number in the file: each level of the tree
    /// is sixteen cells across, so a field is `width x 16^levels` samples.
    pub fn new(terrain: Terrain<'a>, layout: &TerrainLayout) -> Ground<'a> {
        let per_root = LEAF_SAMPLES.pow(terrain.levels);
        Ground {
            spacing: terrain.scale.0,
            samples_x: layout.width.max(1) * per_root,
            samples_z: layout.breadth.max(1) * per_root,
            terrain,
            paint: None,
        }
    }

    /// Paint the ground with the track's own layers instead of one dirt colour.
    pub fn painted(mut self, paint: Option<Splat>) -> Ground<'a> {
        self.paint = paint;
        self
    }

    /// How big the field is in world units.
    pub fn extent(&self) -> (f32, f32) {
        (
            (self.samples_x - 1) as f32 * self.spacing,
            (self.samples_z - 1) as f32 * self.spacing,
        )
    }

    /// Draw the ground around a point: every cell within `radius` samples of it, as two triangles.
    ///
    /// Returns how many triangles went through the rasteriser. Cells behind the camera and cells
    /// beyond the scene's fog are dropped before they are transformed, the way the renderer's own
    /// terrain pass does it — at 60 Hz that cull is most of the frame's budget.
    pub fn draw(
        &self,
        frame: &mut Frame,
        camera: &Camera,
        light: &Lighting,
        around: V3,
        radius: u32,
    ) -> usize {
        let view_projection = camera.view_projection(frame.width, frame.height);
        let forward = camera.at.sub(camera.eye).norm();
        let (cx, cz) = (
            (around.x / self.spacing) as i64,
            (around.z / self.spacing) as i64,
        );
        let radius = radius as i64;
        let mut drawn = 0;
        for sz in (cz - radius).max(0)..(cz + radius).min(self.samples_z as i64 - 1) {
            for sx in (cx - radius).max(0)..(cx + radius).min(self.samples_x as i64 - 1) {
                let (sx, sz) = (sx as u32, sz as u32);
                let Some(quad) = self.terrain.cell(sx, sz, 1) else {
                    continue;
                };
                let (x0, z0) = (sx as f32 * self.spacing, sz as f32 * self.spacing);
                let middle = v3(
                    x0 + self.spacing * 0.5,
                    self.spacing * (quad.height[0].0 + quad.height[3].0) * 0.5,
                    z0 + self.spacing * 0.5,
                );
                let to_middle = middle.sub(camera.eye);
                if to_middle.len() > light.fog_far || to_middle.dot(forward) < -2.0 * self.spacing {
                    continue;
                }
                // The corners in the order the query hands them over, as a quad wound the way the
                // rasteriser's two triangles want: sample, +x, both, +z.
                let corner = [(0, 0, 0), (1, 1, 0), (3, 1, 1), (2, 0, 1)];
                // Under the baked light map the light is in the paint, per pixel; elsewhere it is the
                // sun on the stored normals, per corner.
                let baked = self
                    .paint
                    .as_ref()
                    .is_some_and(|p| p.light(middle.x, middle.z).is_some());
                let shade = corner.map(|(i, dx, dz): (usize, u32, u32)| {
                    let point = v3(
                        x0 + dx as f32 * self.spacing,
                        self.spacing * quad.height[i].0,
                        z0 + dz as f32 * self.spacing,
                    );
                    let n = quad.normal[i];
                    let normal = v3(n[0].0, n[1].0, n[2].0).norm();
                    let lambert = (-normal.dot(light.sun)).max(0.0);
                    let mut lit = [0.0f32; 3];
                    for k in 0..3 {
                        // Without paint the dirt's colour goes in where a texel would have been: the
                        // rasteriser multiplies white by this.
                        let sunlit = light.ambient[k] + light.sun_color[k] * lambert;
                        lit[k] = match (&self.paint, baked) {
                            (Some(_), true) => 1.0,
                            (Some(_), false) => sunlit,
                            (None, _) => sunlit * DIRT[k] / 255.0,
                        };
                    }
                    let fog = ((point.sub(camera.eye).len() - light.fog_near)
                        / (light.fog_far - light.fog_near).max(1.0))
                    .clamp(0.0, 1.0);
                    Shade {
                        clip: view_projection.clip(point),
                        // The paint is a function of where on the ground a pixel is, so that is
                        // what is interpolated.
                        uv: [point.x, point.z],
                        light: lit,
                        fog,
                    }
                });
                for tri in [[0, 1, 2], [0, 2, 3]] {
                    let v = [shade[tri[0]], shade[tri[1]], shade[tri[2]]];
                    match &self.paint {
                        Some(splat) => {
                            let paint = |x: f32, z: f32| {
                                let c = splat.paint(x, z);
                                let l = if baked { splat.light(x, z).unwrap_or(1.0) } else { 1.0 };
                                let lit = c.map(|c| (c as f32 * l).min(255.0) as u8);
                                [lit[0], lit[1], lit[2], 255]
                            };
                            frame.triangle_with(v, &Look::cut_out(&paint), light.fog_color);
                        }
                        None => frame.triangle(v, None, light.fog_color),
                    }
                    drawn += 1;
                }
            }
        }
        drawn
    }
}
