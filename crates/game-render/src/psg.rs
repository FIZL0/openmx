//! `.psg` — the disc's model format: a node tree, a material per mesh, and the VU1 vertex streams.
//!
//! Everything below was read off the disc rather than out of the game's loader, so the names are
//! descriptions of what the bytes do. The layout, little-endian throughout:
//!
//! ```text
//! 0   "psg\0", u32 version (5 on this disc), u32 node count
//! 12  node[count], 224 bytes each:
//!       char name[128]
//!       f32 matrix[16]      the node's transform relative to its parent, translation in the last row
//!       f32 center[3], f32 half_extents[3], f32 radius    bounds, this node and its children
//!       i32 parent          -1 at the root
//!     u32 material count, then that many 64-byte names (a `.psm` each, one per mesh)
//!     f32 scales[4]         position, normal, u, v: what a packed integer is multiplied by
//!     16 zero bytes
//!     u8  lod group count, u8 (see below), u16 0
//!     group[count]: f32 distance, u32 mesh count, then that many meshes:
//!       u32 mesh index (counts across all groups), u32 stream mask, u32 lod count, char name[] (always
//!       empty on this disc)
//!       lod[count]: f32 distance, u16 vertices, u16 (unexplained), u32 stream bytes, then the stream
//! ```
//!
//! A **LOD group** is a whole copy of the model at one level of detail — group 0 is the finest, and the
//! last group is usually a single four-vertex billboard. The **stream mask** says which of the four
//! vertex streams a batch carries: 1 position, 2 normal, 4 texture coordinates, 8 colour.
//!
//! The stream itself is a VIF1 DMA packet, the same bytes the game sends to the hardware: `UNPACK`
//! commands that write one array each into VU1 memory, then `MSCAL`/`MSCNT` to run the microprogram over
//! them. One such batch is up to sixteen vertices, and it starts with a GIF tag whose primitive field is
//! "triangle strip" and whose loop count is the batch's vertex count.
//!
//! Two things in a vertex are not where a modern format would put them, and both matter:
//!
//! * The position's `w` is not a coordinate. Bit 15 is the GS's **ADC** flag: set means "do not draw the
//!   triangle that ends here", which is how a strip restarts without a new packet — so the first two
//!   vertices of every batch have it set. Bits 4..15 are the vertex's **node**, as the VU1 address of
//!   that node's matrix: node index = `(w >> 4) - 1`. That is what holds a bike together, and it was
//!   confirmed by matching each node's stored bounds against the bounds of the vertices that name it.
//! * Positions are 16-bit integers scaled by `scales[0]`, which is chosen per file so the model fills the
//!   range — a bale's scale is 8.24e-5, a whole bike's 1.23e-4.

use crate::mat::{M4, V3, v3};

/// One vertex of a strip, unpacked into floats.
#[derive(Clone, Copy, Debug, Default)]
pub struct Vertex {
    pub pos: V3,
    pub normal: V3,
    pub uv: [f32; 2],
    pub color: [u8; 4],
    /// Which node's transform places it.
    pub node: usize,
    /// The GS's ADC flag: the triangle ending at this vertex is skipped.
    pub adc: bool,
}

/// One VIF batch: a triangle strip of at most sixteen vertices.
#[derive(Clone, Debug, Default)]
pub struct Strip {
    pub verts: Vec<Vertex>,
}

/// All the geometry of one material, at one level of detail.
#[derive(Clone, Debug)]
pub struct Mesh {
    /// Index into [`Model::materials`].
    pub material: usize,
    /// Which LOD group it belongs to; 0 is the finest.
    pub group: usize,
    pub strips: Vec<Strip>,
}

/// A node of the model's tree.
#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    /// Relative to the parent.
    pub local: M4,
    pub parent: Option<usize>,
    pub center: V3,
    pub half_extents: V3,
    pub radius: f32,
}

/// A parsed `.psg`.
#[derive(Clone, Debug)]
pub struct Model {
    pub nodes: Vec<Node>,
    /// One `.psm` name per mesh, indexed by the mesh's own index.
    pub materials: Vec<String>,
    pub meshes: Vec<Mesh>,
    /// How many levels of detail the file holds.
    pub groups: usize,
}

/// A triangle ready to draw: three vertices already in model space.
#[derive(Clone, Copy, Debug)]
pub struct Tri {
    pub v: [Vertex; 3],
    pub material: usize,
}

impl Model {
    /// Parse a whole `.psg`.
    pub fn parse(d: &[u8]) -> Result<Model, String> {
        if d.len() < 12 || &d[..4] != b"psg\0" {
            return Err("not a .psg: no psg\\0 at the front".into());
        }
        let version = le32(d, 4)?;
        if version != 5 {
            return Err(format!(".psg version {version}, expected 5"));
        }
        let node_count = le32(d, 8)? as usize;
        let mut nodes = Vec::with_capacity(node_count);
        for i in 0..node_count {
            let at = 12 + 224 * i;
            need(d, at, 224)?;
            let mut m = [0.0f32; 16];
            for (k, slot) in m.iter_mut().enumerate() {
                *slot = f32_at(d, at + 128 + 4 * k)?;
            }
            let parent = i32::from_le_bytes([
                d[at + 220],
                d[at + 221],
                d[at + 222],
                d[at + 223],
            ]);
            nodes.push(Node {
                name: cstr(&d[at..at + 128]),
                local: M4::from_row_major(&m),
                parent: if parent >= 0 {
                    Some(parent as usize)
                } else {
                    None
                },
                center: vec_at(d, at + 192)?,
                half_extents: vec_at(d, at + 204)?,
                radius: f32_at(d, at + 216)?,
            });
        }
        let mut at = 12 + 224 * node_count;
        let material_count = le32(d, at)? as usize;
        at += 4;
        let mut materials = Vec::with_capacity(material_count);
        for _ in 0..material_count {
            need(d, at, 64)?;
            materials.push(cstr(&d[at..at + 64]));
            at += 64;
        }
        let scales = [
            f32_at(d, at)?,
            f32_at(d, at + 4)?,
            f32_at(d, at + 8)?,
            f32_at(d, at + 12)?,
        ];
        need(d, at, 36)?;
        let groups = d[at + 32] as usize;
        at += 36;

        let mut meshes = Vec::new();
        let mut next_index = 0usize;
        for group in 0..groups {
            need(d, at, 8)?;
            let _group_distance = f32_at(d, at)?;
            let mesh_count = le32(d, at + 4)? as usize;
            at += 8;
            for _ in 0..mesh_count {
                need(d, at, 13)?;
                let index = le32(d, at)? as usize;
                let mask = le32(d, at + 4)?;
                let lod_count = le32(d, at + 8)? as usize;
                if index != next_index {
                    return Err(format!(
                        "mesh {next_index} calls itself {index}: the file is not laid out as expected"
                    ));
                }
                // The mesh's own name, NUL-terminated and empty in every file on this disc.
                let mut p = at + 12;
                while p < d.len() && d[p] != 0 {
                    p += 1;
                }
                at = p + 1;
                let mut strips = Vec::new();
                for lod in 0..lod_count {
                    need(d, at, 12)?;
                    let verts = u16_at(d, at + 4)? as usize;
                    let bytes = le32(d, at + 8)? as usize;
                    let start = at + 12;
                    need(d, start, bytes)?;
                    // Only the finest level is kept: a still frame has no use for the others, and
                    // keeping them would quadruple the triangles in memory.
                    if lod == 0 {
                        strips = unpack_strips(d, start, verts, &scales, mask)?;
                    }
                    at = start + bytes;
                }
                meshes.push(Mesh {
                    material: index,
                    group,
                    strips,
                });
                next_index += 1;
            }
        }
        Ok(Model {
            nodes,
            materials,
            meshes,
            groups,
        })
    }

    /// A node's transform all the way to the root.
    pub fn world(&self, node: usize) -> M4 {
        let mut m = M4::IDENTITY;
        let mut at = Some(node);
        let mut guard = 0;
        while let Some(i) = at {
            let Some(n) = self.nodes.get(i) else { break };
            m = m.mul(n.local);
            at = n.parent;
            guard += 1;
            if guard > self.nodes.len() {
                break; // a cycle in the tree; draw what we have rather than hang
            }
        }
        m
    }

    /// Every triangle of one LOD group, with each vertex already placed by its node.
    ///
    /// Strips are expanded here rather than at draw time because the ADC flag makes the expansion
    /// awkward: a vertex with it set ends no triangle, which is how one batch holds several strips.
    pub fn triangles(&self, group: usize) -> Vec<Tri> {
        let world: Vec<M4> = (0..self.nodes.len()).map(|i| self.world(i)).collect();
        let mut out = Vec::new();
        for mesh in self.meshes.iter().filter(|m| m.group == group) {
            for strip in &mesh.strips {
                for i in 2..strip.verts.len() {
                    if strip.verts[i].adc {
                        continue;
                    }
                    let mut t = Tri {
                        v: [strip.verts[i - 2], strip.verts[i - 1], strip.verts[i]],
                        material: mesh.material,
                    };
                    for v in t.v.iter_mut() {
                        if let Some(m) = world.get(v.node) {
                            v.pos = m.point(v.pos);
                            v.normal = m.dir(v.normal);
                        }
                    }
                    out.push(t);
                }
            }
        }
        out
    }
}

/// Walk one mesh's VIF packet, turning each batch into a strip.
///
/// The packet is a list of 32-bit VIF codes; the ones that appear on this disc are `NOP`, `STCYCL`,
/// `UNPACK` and `MSCAL`/`MSCNT`. An `UNPACK`'s data follows the code inline and is padded to the
/// packet's own four-byte grid — the packet does not start on a four-byte boundary of the *file*, which
/// is why every offset below is measured from `start`.
fn unpack_strips(
    d: &[u8],
    start: usize,
    want_verts: usize,
    scales: &[f32; 4],
    mask: u32,
) -> Result<Vec<Strip>, String> {
    let mut out = Vec::new();
    let mut p = start;
    let mut got = 0usize;
    // The four arrays a batch can carry, as (offset, count) once seen.
    let (mut pos, mut normal, mut uv, mut color) = (None, None, None, None);
    while p + 4 <= d.len() && got < want_verts {
        let w = le32(d, p)?;
        let cmd = ((w >> 24) & 0x7f) as u8;
        let num = ((w >> 16) & 0xff) as usize;
        p += 4;
        match cmd {
            // NOP, STCYCL, OFFSET, BASE, ITOP, STMOD, MSKPATH3, MARK, FLUSH*: no data follows.
            0x00 | 0x01 | 0x02 | 0x03 | 0x04 | 0x05 | 0x06 | 0x07 | 0x10 | 0x11 | 0x13 => {}
            // MSCAL, MSCALF, MSCNT: run the microprogram — the end of a batch.
            0x14 | 0x15 | 0x17 => {
                if let Some((at, n)) = pos {
                    out.push(strip_from(d, at, n, normal, uv, color, scales, mask)?);
                    got += n;
                }
                pos = None;
                normal = None;
                uv = None;
                color = None;
            }
            0x20 => p += 4,  // STMASK
            0x30 | 0x31 => p += 16, // STROW, STCOL
            c if c & 0x60 == 0x60 => {
                let components = ((c >> 2) & 3) as usize + 1;
                let width = match c & 3 {
                    0 => 32,
                    1 => 16,
                    2 => 8,
                    _ => return Err(format!("UNPACK {c:#04x} has no width")),
                };
                let n = if num == 0 { 256 } else { num };
                let at = p;
                p += n * components * width / 8;
                // Back onto the packet's four-byte grid.
                p = start + (p - start).div_ceil(4) * 4;
                match (components, width) {
                    (4, 16) => pos = Some((at, n)),
                    (3, 8) => normal = Some((at, n)),
                    (2, 16) => uv = Some((at, n)),
                    (4, 8) => color = Some((at, n)),
                    // The batch's GIF tag: a template the microprogram patches and sends on.
                    (4, 32) => {}
                    _ => {
                        return Err(format!(
                            "unexpected UNPACK of {n} x {components} x {width} bits at {at}"
                        ));
                    }
                }
            }
            c => return Err(format!("unknown VIF code {c:#04x} at {}", p - 4)),
        }
    }
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn strip_from(
    d: &[u8],
    at: usize,
    n: usize,
    normal: Option<(usize, usize)>,
    uv: Option<(usize, usize)>,
    color: Option<(usize, usize)>,
    scales: &[f32; 4],
    mask: u32,
) -> Result<Strip, String> {
    let mut verts = Vec::with_capacity(n);
    need(d, at, n * 8)?;
    for i in 0..n {
        let x = i16_at(d, at + 8 * i)?;
        let y = i16_at(d, at + 8 * i + 2)?;
        let z = i16_at(d, at + 8 * i + 4)?;
        let w = u16_at(d, at + 8 * i + 6)?;
        let mut v = Vertex {
            pos: v3(
                x as f32 * scales[0],
                y as f32 * scales[0],
                z as f32 * scales[0],
            ),
            adc: w & 0x8000 != 0,
            // The VU1 address of this vertex's matrix; the first node lives one slot in.
            node: (((w & 0x7fff) >> 4) as usize).saturating_sub(1),
            color: [0x80, 0x80, 0x80, 0x80],
            ..Default::default()
        };
        if mask & 2 != 0 {
            if let Some((o, _)) = normal {
                need(d, o, n * 3)?;
                v.normal = v3(
                    d[o + 3 * i] as i8 as f32 * scales[1],
                    d[o + 3 * i + 1] as i8 as f32 * scales[1],
                    d[o + 3 * i + 2] as i8 as f32 * scales[1],
                );
            }
        }
        if mask & 4 != 0 {
            if let Some((o, _)) = uv {
                need(d, o, n * 4)?;
                v.uv = [
                    i16_at(d, o + 4 * i)? as f32 * scales[2],
                    i16_at(d, o + 4 * i + 2)? as f32 * scales[3],
                ];
            }
        }
        if mask & 8 != 0 {
            if let Some((o, _)) = color {
                need(d, o, n * 4)?;
                v.color = [d[o + 4 * i], d[o + 4 * i + 1], d[o + 4 * i + 2], d[o + 4 * i + 3]];
            }
        }
        verts.push(v);
    }
    Ok(Strip { verts })
}

fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

fn need(d: &[u8], at: usize, len: usize) -> Result<(), String> {
    if at + len > d.len() {
        return Err(format!(
            "the file ends at {} but {len} bytes were wanted at {at}",
            d.len()
        ));
    }
    Ok(())
}

fn le32(d: &[u8], at: usize) -> Result<u32, String> {
    need(d, at, 4)?;
    Ok(u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]]))
}

fn u16_at(d: &[u8], at: usize) -> Result<u16, String> {
    need(d, at, 2)?;
    Ok(u16::from_le_bytes([d[at], d[at + 1]]))
}

fn i16_at(d: &[u8], at: usize) -> Result<i16, String> {
    Ok(u16_at(d, at)? as i16)
}

fn f32_at(d: &[u8], at: usize) -> Result<f32, String> {
    Ok(f32::from_bits(le32(d, at)?))
}

fn vec_at(d: &[u8], at: usize) -> Result<V3, String> {
    Ok(v3(f32_at(d, at)?, f32_at(d, at + 4)?, f32_at(d, at + 8)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_strip_skips_the_triangles_its_adc_flags_mark() {
        // Six vertices, the first two flagged as every batch's are, plus a restart at the fourth.
        let mut strip = Strip::default();
        for i in 0..6 {
            strip.verts.push(Vertex {
                adc: i < 2 || i == 3,
                ..Default::default()
            });
        }
        let model = Model {
            nodes: vec![],
            materials: vec!["m".into()],
            meshes: vec![Mesh {
                material: 0,
                group: 0,
                strips: vec![strip],
            }],
            groups: 1,
        };
        // Six vertices make four triangles; the one ending at the flagged fourth vertex is dropped.
        assert_eq!(model.triangles(0).len(), 3);
    }

    #[test]
    fn a_model_without_its_magic_is_refused() {
        assert!(Model::parse(b"not a model").is_err());
    }
}
