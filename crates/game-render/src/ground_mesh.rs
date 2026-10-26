//! The flat meshes a freeride scene lays into its ground's paint (V67): the roads, the ruts, the
//! runway, the dirt loops and the erosion.
//!
//! A freeride `.scn` places them as models with `Type=TerrainScreenSpace` (`Free06_Road01`,
//! `Free09_Highway`, `Free10_Runway`, `Free09_Dirt_Loop`, some two hundred `Free06_Rut_*` a map), every
//! one flat at `y = 0` with its own one-pass `.psm` (`free06_road.psm`: the road texture laid over
//! the frame by `(Cs - Cd) * As + Cd`, its edges faded out by the vertex alpha). The original does not
//! draw them in the world. It draws them from above into the ground's off-screen paint target, the
//! one [`crate::splat`]'s passes paint, and the ground then samples that target, hills and all.
//!
//! They go in after the whole of `terrainscreenspacemesh.psm`'s program, its detail pass included:
//! the program's first pass replaces the frame (no blend) and the target is not cleared before use
//! (`terrainscreenspaceprologue.psm`'s `clearTargetBeforeUse = 0`), so a mesh drawn before it would
//! be painted out — and the light-map tiles, which are screen-space models of the same kind
//! (`Free06_Shadow_01..16`, blended `Cd * As`), would light nothing. The epilogue has no passes.
//! Among themselves they go in the scene's order.
//!
//! The light-map tiles are left out here: [`crate::splat::Shadow`] already reads the same sixteen
//! textures by name, over the same squares, and lights the paint after this.
//!
//! [`GroundMeshes::paint`] runs the meshes over a colour the splat has painted, per point, as the GS
//! would blend them: the triangles under the point in the scene's order, each one's texture sampled
//! at the point's place on it, times the vertex colour, through its own blend. Every blend on the
//! disc's meshes is the modulate one, `Cd * (1 - As) + Cs * As`, so what a stack of them does to any
//! colour beneath is `Cd * keep + over` ([`GroundMeshes::affine`]): the GPU draws the meshes from above
//! into targets that hold `over` in their colour and `keep` in their alpha, as the original draws
//! them into its own, and the ground's shader applies them to its paint.

use crate::disc::Res;
use crate::psg::Model;
use crate::pst::Texture;
use crate::scene::Placement;
use crate::splat::{Pass, Splat, blend};

/// One triangle of a screen-space mesh, laid flat in the world.
#[derive(Clone, Debug)]
pub struct Flat {
    /// The corners' world `x` and `z`.
    pub xz: [[f32; 2]; 3],
    pub uv: [[f32; 2]; 3],
    /// The vertex colours, 128 meaning "unchanged".
    pub color: [[f32; 4]; 3],
    /// The `.psm` pass it is drawn with, its texture an index into [`GroundMeshes::textures`].
    pub pass: Pass,
    /// For the barycentric weights: the inverse of twice the signed area, and its bounding box.
    inv_area: f32,
    lo: [f32; 2],
    hi: [f32; 2],
}

/// Every screen-space mesh of a scene, triangle by triangle in the order they are drawn, with a grid
/// over the world to find the ones under a point.
pub struct GroundMeshes {
    pub textures: Vec<Texture>,
    pub tris: Vec<Flat>,
    /// The grid: `CELL`-foot squares from `origin`, `across` by `along`, each listing the triangles
    /// that reach into it in drawing order.
    origin: [f32; 2],
    across: usize,
    along: usize,
    cells: Vec<Vec<u32>>,
}

/// The lookup grid's square, in feet.
const CELL: f32 = 32.0;

/// Whether a scene model is a light-map tile, `<track>_Shadow_<nn>`, which [`crate::splat::Shadow`]
/// draws by name.
pub fn is_light_map(model: &str) -> bool {
    let lower = model.to_ascii_lowercase();
    lower
        .rsplit_once("_shadow_")
        .is_some_and(|(_, n)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether a scene model's mesh lays one of the paint program's own layers again: a material of it
/// textured with one of `layers`, the textures `terrainscreenspacemesh.psm` names (V77).
///
/// Only `Free06_Erode` does, on every freeride track of the disc: dirt-cliff bands over free06's
/// slopes and some fifteen feet onto the flat at each foot, textured with `free06_erode`, which the
/// program's fifth pass already lays through its own mask. The original shows no trace of the mesh:
/// next to it the flat at a band's foot is as light as the flat beyond, where the port drew a slab
/// at the erosion texture's own dark brown, and up a cliff the streaks run down the slope the way
/// the program's world-turned layer runs, not along the band the way the mesh's texture would. So
/// it is left out.
pub fn repeats_a_layer(archives: &[Res], model: &str, layers: &[String]) -> bool {
    let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(name).ok());
    let Some(m) = read(&format!("{}.psg", model.to_ascii_lowercase())).and_then(|d| Model::parse(&d).ok()) else {
        return false;
    };
    m.materials.iter().any(|name| {
        read(&format!("{}.psm", name.to_ascii_lowercase())).is_some_and(|psm| {
            crate::ini::parse(&crate::ini::text(&psm))
                .iter()
                .filter_map(|s| s.get("TextureMap_1"))
                .any(|t| layers.iter().any(|l| l.eq_ignore_ascii_case(t.trim())))
        })
    })
}

impl GroundMeshes {
    /// The screen-space meshes `placements` name, read from `archives`: each model's `.psg`, its
    /// materials' `.psm` and their textures. A model, material or texture that is not there is left
    /// out, and the light-map tiles always are. `None` when nothing is left.
    pub fn load(archives: &[Res], placements: &[Placement]) -> Option<GroundMeshes> {
        let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(name).ok());
        let mut models: Vec<(String, Option<Model>)> = Vec::new();
        let mut materials: Vec<(String, Option<Pass>)> = Vec::new();
        let mut textures: Vec<Texture> = Vec::new();
        let mut names: Vec<String> = Vec::new();
        let mut tris = Vec::new();
        for p in placements.iter().filter(|p| !is_light_map(&p.model)) {
            let key = p.model.to_ascii_lowercase();
            let at = match models.iter().position(|(n, _)| *n == key) {
                Some(i) => i,
                None => {
                    let m = read(&format!("{key}.psg")).and_then(|d| Model::parse(&d).ok());
                    models.push((key, m));
                    models.len() - 1
                }
            };
            let Some(model) = &models[at].1 else { continue };
            let mut passes = Vec::new();
            for name in &model.materials {
                let key = name.to_ascii_lowercase();
                let found = materials.iter().find(|(n, _)| *n == key).map(|(_, p)| p.clone());
                let pass = found.unwrap_or_else(|| {
                    let pass = read(&format!("{key}.psm")).and_then(|psm| {
                        let mut loaded = None;
                        let one = Splat::parse(&crate::ini::text(&psm), |t| {
                            let t = t.to_ascii_lowercase();
                            if let Some(i) = names.iter().position(|n| *n == t) {
                                loaded = Some(i);
                                // A stand-in: the pass keeps its texture's index, set below.
                                return Some(Texture { width: 1, height: 1, rgba: vec![0; 4] });
                            }
                            let tex = crate::replace::parse(&read(&format!("{t}.pst"))?)?;
                            textures.push(tex);
                            names.push(t);
                            loaded = Some(textures.len() - 1);
                            Some(Texture { width: 1, height: 1, rgba: vec![0; 4] })
                        });
                        let mut pass = one.passes.into_iter().next()?;
                        pass.texture = loaded?;
                        Some(pass)
                    });
                    materials.push((key, pass.clone()));
                    pass
                });
                passes.push(pass);
            }
            tris.extend(lay(model, &passes, p));
        }
        GroundMeshes::of(textures, tris)
    }

    /// The meshes from triangles already laid out, with the grid built over them.
    pub fn of(textures: Vec<Texture>, tris: Vec<Flat>) -> Option<GroundMeshes> {
        if tris.is_empty() {
            return None;
        }
        let mut lo = [f32::MAX; 2];
        let mut hi = [f32::MIN; 2];
        for t in &tris {
            for k in 0..2 {
                lo[k] = lo[k].min(t.lo[k]);
                hi[k] = hi[k].max(t.hi[k]);
            }
        }
        let origin = [crate::pst::floor(lo[0] / CELL) * CELL, crate::pst::floor(lo[1] / CELL) * CELL];
        let across = ((hi[0] - origin[0]) / CELL) as usize + 1;
        let along = ((hi[1] - origin[1]) / CELL) as usize + 1;
        let mut cells = vec![Vec::new(); across * along];
        for (i, t) in tris.iter().enumerate() {
            let c0 = ((t.lo[0] - origin[0]) / CELL) as usize;
            let c1 = (((t.hi[0] - origin[0]) / CELL) as usize).min(across - 1);
            let r0 = ((t.lo[1] - origin[1]) / CELL) as usize;
            let r1 = (((t.hi[1] - origin[1]) / CELL) as usize).min(along - 1);
            for r in r0..=r1 {
                for c in c0..=c1 {
                    cells[r * across + c].push(i as u32);
                }
            }
        }
        Some(GroundMeshes { textures, tris, origin, across, along, cells })
    }

    /// The world rectangle the meshes lie in, `x` and `z` from the first corner to the second.
    pub fn bounds(&self) -> ([f32; 2], [f32; 2]) {
        let o = self.origin;
        (o, [o[0] + self.across as f32 * CELL, o[1] + self.along as f32 * CELL])
    }

    /// The triangles that reach into the grid square holding a point, in drawing order.
    fn under(&self, x: f32, z: f32) -> &[u32] {
        let (c, r) = ((x - self.origin[0]) / CELL, (z - self.origin[1]) / CELL);
        if c < 0.0 || r < 0.0 || c as usize >= self.across || r as usize >= self.along {
            return &[];
        }
        &self.cells[r as usize * self.across + c as usize]
    }

    /// Whether any mesh covers a point of the world.
    pub fn covers(&self, x: f32, z: f32) -> bool {
        self.under(x, z).iter().any(|&i| self.tris[i as usize].weights(x, z).is_some())
    }

    /// `frame`, the splat's colour at a point with its alpha in the GS's units, with every mesh over
    /// the point blended onto it in turn.
    pub fn paint(&self, x: f32, z: f32, mut frame: [f32; 4]) -> [f32; 4] {
        for &i in self.under(x, z) {
            let t = &self.tris[i as usize];
            let Some(w) = t.weights(x, z) else { continue };
            let at = |k: usize| t.uv[0][k] * w[0] + t.uv[1][k] * w[1] + t.uv[2][k] * w[2];
            let c = |k: usize| t.color[0][k] * w[0] + t.color[1][k] * w[1] + t.color[2][k] * w[2];
            let texel = self.textures[t.pass.texture].sample_smooth(at(0), at(1));
            let p = &t.pass;
            // The GS's modulate: the texel times the vertex colour over 128, and the pass's constant
            // colour as the splat's passes have it; the `.pst` decoder doubled the alpha.
            let src = [
                texel[0] as f32 * c(0) / 128.0 * p.rgba[0] / 128.0,
                texel[1] as f32 * c(1) / 128.0 * p.rgba[1] / 128.0,
                texel[2] as f32 * c(2) / 128.0 * p.rgba[2] / 128.0,
                (texel[3] as f32 * 0.5 * c(3) / 128.0 * p.rgba[3] / 128.0).min(255.0),
            ];
            frame = blend(src, frame, p.blend, p.fixed);
        }
        frame
    }

    /// [`GroundMeshes::paint`] as `Cd * keep + over` at a point: what the meshes do to whatever colour
    /// is under them, worked out from the two colours black and white. Exact for the blends the disc's
    /// meshes have, short of the clamp at 255.
    pub fn affine(&self, x: f32, z: f32) -> ([f32; 3], f32) {
        let black = self.paint(x, z, [0.0; 4]);
        let white = self.paint(x, z, [255.0; 4]);
        let keep = (0..3).map(|k| (white[k] - black[k]) / 255.0).sum::<f32>() / 3.0;
        ([black[0], black[1], black[2]], keep.clamp(0.0, 1.0))
    }
}

impl Flat {
    /// A triangle from its corners' world `x` and `z`, texture places and vertex colours.
    pub fn new(xz: [[f32; 2]; 3], uv: [[f32; 2]; 3], color: [[f32; 4]; 3], pass: Pass) -> Flat {
        let [a, b, c] = xz;
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
        let inv_area = if area.abs() > 1e-6 { 1.0 / area } else { 0.0 };
        let lo = [a[0].min(b[0]).min(c[0]), a[1].min(b[1]).min(c[1])];
        let hi = [a[0].max(b[0]).max(c[0]), a[1].max(b[1]).max(c[1])];
        Flat { xz, uv, color, pass, inv_area, lo, hi }
    }

    /// The point's barycentric weights, or `None` outside the triangle (or for one with no area).
    #[inline]
    fn weights(&self, x: f32, z: f32) -> Option<[f32; 3]> {
        if self.inv_area == 0.0 || x < self.lo[0] || x > self.hi[0] || z < self.lo[1] || z > self.hi[1] {
            return None;
        }
        let [a, b, c] = self.xz;
        let w1 = ((x - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (z - a[1])) * self.inv_area;
        let w2 = ((b[0] - a[0]) * (z - a[1]) - (x - a[0]) * (b[1] - a[1])) * self.inv_area;
        let w0 = 1.0 - w1 - w2;
        (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0).then_some([w0, w1, w2])
    }
}

/// One placed model's triangles, flat in the world, each with its material's pass; a mesh whose
/// material did not load is left out.
fn lay(model: &Model, passes: &[Option<Pass>], at: &Placement) -> Vec<Flat> {
    let place = at.transform();
    model
        .triangles(0)
        .into_iter()
        .filter_map(|t| {
            let pass = passes.get(t.material)?.clone()?;
            let xz = t.v.map(|v| {
                let p = place.point(v.pos);
                [p.x, p.z]
            });
            Some(Flat::new(xz, t.v.map(|v| v.uv), t.v.map(|v| v.color.map(f32::from)), pass))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modulate(texture: usize) -> Pass {
        Pass {
            texture,
            scale: [1.0, 1.0],
            angle: 0.0,
            turn: (0.0, 1.0),
            rgba: [128.0; 4],
            blend: Some([0, 1, 0, 1]),
            fixed: 128.0,
            reads: (true, true),
        }
    }

    fn square(x0: f32, z0: f32, side: f32, alpha: f32, texture: usize) -> Vec<Flat> {
        let c = [128.0, 128.0, 128.0, alpha];
        let (a, b, cc, d) = ([x0, z0], [x0 + side, z0], [x0 + side, z0 + side], [x0, z0 + side]);
        vec![
            Flat::new([a, b, cc], [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]], [c; 3], modulate(texture)),
            Flat::new([a, cc, d], [[0.0, 0.0], [1.0, 1.0], [0.0, 1.0]], [c; 3], modulate(texture)),
        ]
    }

    fn flat(rgba: [u8; 4]) -> Texture {
        Texture { width: 2, height: 2, rgba: rgba.repeat(4) }
    }

    #[test]
    fn a_road_modulates_the_paint_under_it_and_nothing_else() {
        // The road's texel is (40, 50, 60) with a decoded alpha of 255, 127.5 to the GS; the vertex
        // alpha of 64 halves it to 63.75: just under half road, half paint.
        let m = GroundMeshes::of(vec![flat([40, 50, 60, 255])], square(100.0, 100.0, 50.0, 64.0, 0)).unwrap();
        let under = [200.0, 100.0, 0.0, 128.0];
        let on = m.paint(120.0, 130.0, under);
        assert!((on[0] - 120.3).abs() < 0.1 && (on[1] - 75.1).abs() < 0.1 && (on[2] - 29.9).abs() < 0.1, "{on:?}");
        assert_eq!(m.paint(90.0, 130.0, under), under);
        assert!(m.covers(120.0, 130.0) && !m.covers(151.0, 130.0));
        let (over, keep) = m.affine(120.0, 130.0);
        assert!((keep - (1.0 - 63.75 / 128.0)).abs() < 0.001, "{keep}");
        assert!((over[0] + under[0] * keep - on[0]).abs() < 0.5);
    }

    #[test]
    fn meshes_go_down_in_the_scenes_order() {
        let mut tris = square(0.0, 0.0, 10.0, 128.0, 0);
        tris.extend(square(0.0, 0.0, 10.0, 128.0, 1));
        let m = GroundMeshes::of(vec![flat([255, 0, 0, 255]), flat([0, 0, 255, 255])], tris).unwrap();
        let c = m.paint(3.0, 6.0, [0.0, 255.0, 0.0, 0.0]);
        assert!(c[2] > 250.0 && c[0] < 5.0, "the second mesh is on top: {c:?}");
    }

    #[test]
    fn the_light_map_tiles_are_known_by_name() {
        assert!(is_light_map("Free06_Shadow_01"));
        assert!(is_light_map("free09_shadow_16"));
        assert!(!is_light_map("Free06_Road01"));
        assert!(!is_light_map("Free06_Shadow"));
    }
}
