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
use game_render::raster::{Batch, Frame, Mode, Shade};
use game_render::splat::Splat;
#[cfg(not(target_arch = "wasm32"))]
use game_render::gpu::{GroundChunk, GroundVertex};
use game_render::world::{Camera, Lighting};
use game_sim::track::{Terrain, TerrainLayout};

/// The colour the ground is painted where the track has no paint of its own ([`Splat`]).
///
/// The `.trn` carries a 1 MiB image and a 256-colour palette for it, but which of its tiles a cell
/// takes and how the cell's corners index into one is not decoded (V1 says so too), so the ground is
/// one dirt colour with the file's own normals doing all the shaping. It reads as dirt and it is
/// honest about being flat.
pub const DIRT: [f32; 3] = [150.0, 118.0, 86.0];

/// A ground cell's paint in a [`Batch`]: the track's, under its baked light map or outside it, or
/// none.
const PAINTED: u32 = 0;
const BAKED: u32 = 1;
const UNPAINTED: u32 = 2;

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
    /// A field is `width x 256` samples, as the simulation's ground has it, whatever the tree's depth
    /// ([`game_sim::world::SAMPLES_PER_WIDTH`]).
    pub fn new(terrain: Terrain<'a>, layout: &TerrainLayout) -> Ground<'a> {
        let per_width = game_sim::world::SAMPLES_PER_WIDTH;
        Ground {
            spacing: terrain.scale.0,
            samples_x: layout.width.max(1) * per_width,
            samples_z: layout.breadth.max(1) * per_width,
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

    /// The whole field as the GPU draws it (P44): square chunks of `cells` cells a side, each the
    /// grid of its cells' corners with their stored normals. The GPU picks each chunk's level of
    /// detail and so its triangles (V32), split as [`Ground::draw`] splits them at full detail.
    ///
    /// `cells` is rounded up to a power of two. A chunk past the field's far edge repeats the edge's
    /// corners, and a cell the terrain has no quad for is listed in the chunk's `missing`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn chunks(&self, cells: u32) -> Vec<GroundChunk> {
        let cells = cells.max(1).next_power_of_two();
        let (across, along) = (self.samples_x - 1, self.samples_z - 1);
        let row = cells + 1;
        let mut out = Vec::new();
        for (cz, z0) in (0..along).step_by(cells as usize).enumerate() {
            for (cx, x0) in (0..across).step_by(cells as usize).enumerate() {
                let mut vertices = vec![GroundVertex::default(); (row * row) as usize];
                let mut found = vec![false; vertices.len()];
                let mut missing = vec![false; (cells * cells) as usize];
                // Each corner from a quad that has it: the cell it starts, or failing that a
                // neighbour's. Past the field's edge, the edge's own.
                for j in 0..cells {
                    for i in 0..cells {
                        let (sx, sz) = (x0 + i, z0 + j);
                        if sx >= across || sz >= along {
                            continue;
                        }
                        let Some(quad) = self.terrain.cell(sx, sz, 1) else {
                            missing[(j * cells + i) as usize] = true;
                            continue;
                        };
                        for (k, dx, dz) in [(0, 0, 0), (1, 1, 0), (3, 1, 1), (2, 0, 1)] {
                            let at = ((j + dz) * row + i + dx) as usize;
                            if found[at] {
                                continue;
                            }
                            let n = quad.normal[k];
                            let normal = v3(n[0].0, n[1].0, n[2].0).norm();
                            vertices[at] = GroundVertex {
                                pos: [(sx + dx) as f32 * self.spacing, self.spacing * quad.height[k].0, (sz + dz) as f32 * self.spacing],
                                normal: [normal.x, normal.y, normal.z],
                            };
                            found[at] = true;
                        }
                    }
                }
                if !found.iter().any(|&f| f) {
                    continue;
                }
                // Past the far edge: the last corner of the field along that row or column.
                let (w, h) = (cells.min(across - x0), cells.min(along - z0));
                for j in 0..row {
                    for i in 0..row {
                        let at = (j * row + i) as usize;
                        if i > w || j > h {
                            vertices[at] = vertices[(j.min(h) * row + i.min(w)) as usize];
                            found[at] = found[(j.min(h) * row + i.min(w)) as usize];
                        }
                    }
                }
                let (mut lo, mut hi) = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
                for (v, _) in vertices.iter().zip(&found).filter(|(_, f)| **f) {
                    let p = v3(v.pos[0], v.pos[1], v.pos[2]);
                    lo = v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
                    hi = v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
                }
                // A corner no quad reached is only ever used by a missing cell; put it on the
                // chunk's floor so the chunk's sphere and its errors stay honest.
                for (v, _) in vertices.iter_mut().zip(&found).filter(|(_, f)| !**f) {
                    v.pos[1] = lo.y;
                }
                let any_missing = missing.iter().any(|&m| m);
                let centre = lo.add(hi).scale(0.5);
                out.push(GroundChunk {
                    vertices,
                    cells,
                    at: (cx as u32, cz as u32),
                    missing: if any_missing { missing } else { Vec::new() },
                    centre,
                    radius: hi.sub(centre).len(),
                });
            }
        }
        out
    }

    /// Draw the ground around a point: every cell within `radius` samples of it, as two triangles.
    ///
    /// Returns how many triangles went through the rasteriser. Cells behind the camera and cells
    /// beyond the scene's fog are dropped before they are transformed, the way the renderer's own
    /// terrain pass does it — at 60 Hz that cull is most of the frame's budget.
    ///
    /// The cells are gathered into one [`Batch`] and filled on every core (P47): the paint is a
    /// whole program of texture passes a pixel, and on one core it was the frame's whole budget
    /// many times over at a desktop's size.
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
        let mut batch = Batch::new(frame.width, frame.height);
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
                // Which paint the batch gives the cell's pixels: the splat lit by its light map
                // or by the corners, or no paint at all.
                let tag = match (&self.paint, baked) {
                    (Some(_), true) => BAKED,
                    (Some(_), false) => PAINTED,
                    (None, _) => UNPAINTED,
                };
                for tri in [[0, 1, 2], [0, 2, 3]] {
                    let v = [shade[tri[0]], shade[tri[1]], shade[tri[2]]];
                    batch.triangle(v, tag, Mode::CUT_OUT);
                    drawn += 1;
                }
            }
        }
        let paint = |tag: u32, x: f32, z: f32| match &self.paint {
            Some(splat) if tag != UNPAINTED => {
                let c = splat.paint(x, z);
                let l = if tag == BAKED { splat.light(x, z).unwrap_or(1.0) } else { 1.0 };
                let lit = c.map(|c| (c as f32 * l).min(255.0) as u8);
                [lit[0], lit[1], lit[2], 255]
            }
            _ => [255, 255, 255, 255],
        };
        frame.draw(&batch, &paint, light.fog_color);
        drawn
    }
}
