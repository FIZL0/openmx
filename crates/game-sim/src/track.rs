//! One track as the disc stores it: the scene that lists what is on it, the collision shapes the
//! simulation tests against, and the terrain the wheels run on.
//!
//! A freeride track is three files inside one `.res` member of the big container (see [`crate::archive`]),
//! plus one `.col` per placed shape:
//!
//! ```text
//! Tracks\free06.res ──┬── tracks\free06\free06.scn    the scene: spawn, terrain parameters, placements
//!                     ├── tracks\free06\free06.trn    the terrain: a quantised heightfield in a tree
//!                     └── col\*.col                   one collision mesh per placed shape
//! ```
//!
//! The scene is plain CRLF INI text. The other two are binary, little-endian, and both are read here as
//! borrowed byte slices: nothing in this module does I/O, allocates a copy of a heightfield, or looks at
//! the disc — the app layer hands it the bytes [`crate::archive`] produced.
//!
//! What the simulation actually tests against on this track is the terrain heightfield plus the shapes
//! the scene places: `free06` has no `[SoultreeTerrain]` section and no separate terrain collision mesh,
//! so the ground *is* the heightfield. `research/journal/2026-10-15-d6-d6-freeride-track/` has the
//! hexdumps, the byte accounting and the `./kit fn` recipes for the loaders these were read off.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use ps2_float::{F, fpu};

/// A vector as the files store it: three singles, read as the FPU would read them.
pub type V3 = [F; 3];

// ---------------------------------------------------------------------------------------------------
// reading
// ---------------------------------------------------------------------------------------------------

/// A cursor over a file that cannot run off the end, and says what it wanted when it does.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    what: &'static str,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], what: &'static str) -> Reader<'a> {
        Reader { bytes, at: 0, what }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).ok_or("offset overflow")?;
        let got = self.bytes.get(self.at..end).ok_or_else(|| {
            format!(
                "{}: wanted {n} bytes at {} of a {}-byte file",
                self.what,
                self.at,
                self.bytes.len()
            )
        })?;
        self.at = end;
        Ok(got)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().expect("2 bytes")))
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("4 bytes")))
    }

    fn i32(&mut self) -> Result<i32, String> {
        Ok(self.u32()? as i32)
    }

    /// A single as the FPU would hold it: a denormal on the disc is a zero in a register.
    fn f(&mut self) -> Result<F, String> {
        Ok(F(fpu::operand(self.u32()?)))
    }

    fn v3(&mut self) -> Result<V3, String> {
        Ok([self.f()?, self.f()?, self.f()?])
    }

    /// `n` bytes skipped on purpose, so the byte accounting still covers them.
    fn skip(&mut self, n: usize) -> Result<(), String> {
        self.take(n).map(|_| ())
    }

    /// Everything read, and nothing left over — the cheap proof that a layout is right.
    fn finished(&self) -> Result<(), String> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(format!(
                "{}: parsed to {} of a {}-byte file",
                self.what,
                self.at,
                self.bytes.len()
            ))
        }
    }
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().expect("2 bytes")))
}

// ---------------------------------------------------------------------------------------------------
// the scene
// ---------------------------------------------------------------------------------------------------

/// The scene file: CRLF INI, one section per thing on the track.
///
/// `[SceneInfo]` has the spawn, `[Environment]` names the terrain and how it is laid out, and then one
/// `[Model<n>]` per static model and one `[Object<n>]` per placed collision shape. Only the keys the
/// simulation needs are kept; the rest of the file is lighting, fog and ecosystem parameters that belong
/// to rendering.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub name: String,
    /// Where a rider starts when the track is entered with no other reason to be somewhere.
    pub start: V3,
    pub start_direction: V3,
    /// The terrain file's name as the scene spells it, to be looked up in the same `.res`.
    pub terrain_file: String,
    pub terrain: TerrainLayout,
    /// The visual models. They carry no collision; the shapes below do.
    pub models: Vec<Placement>,
    /// The collision shapes, one entry per placement — the same `.col` appears many times over.
    pub collision: Vec<Placement>,
}

/// How big the terrain is and how its samples map to world units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TerrainLayout {
    /// Root cells across and along. Each is one child of the terrain tree's root.
    pub width: u32,
    pub breadth: u32,
    /// World units per terrain sample, across and along.
    pub width_scale: F,
    pub breadth_scale: F,
    pub border_width: F,
    pub min_division_size: F,
}

/// One thing placed on the track: a model or a collision shape, with where it sits and how it faces.
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    /// The `.col` or model file, as the scene spells it.
    pub file: String,
    pub position: V3,
    /// Where it faces. Absent in the file for anything axis-aligned, which means +Z.
    pub look: V3,
    /// Which way is up for it. Absent in the file for anything standing upright, which means +Y.
    pub up: V3,
    /// The `Type=` key, where the scene gives one. A name, not a number.
    pub kind: Option<String>,
}

impl Default for Placement {
    fn default() -> Placement {
        Placement {
            file: String::new(),
            position: [F::ZERO; 3],
            look: [F::ZERO, F::ZERO, F(1.0)],
            up: [F::ZERO, F(1.0), F::ZERO],
            kind: None,
        }
    }
}

impl Scene {
    /// Read a `.scn`. Unknown sections and keys are ignored, the way the game's own INI reader ignores
    /// what the class asking does not claim.
    pub fn parse(bytes: &[u8]) -> Result<Scene, String> {
        let text = core::str::from_utf8(bytes).map_err(|e| format!("the scene is not text: {e}"))?;
        let mut scene = Scene::default();
        let mut section = String::new();
        let mut current = Placement::default();
        let mut collecting: Option<bool> = None; // Some(true) = a collision shape, Some(false) = a model

        for raw in text.lines() {
            let line = raw.trim();
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                Scene::flush(&mut scene, &mut current, &mut collecting);
                section = name.to_ascii_lowercase();
                collecting = match () {
                    _ if numbered(&section, "object") => Some(true),
                    _ if numbered(&section, "model") => Some(false),
                    _ => None,
                };
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { continue };
            let (key, value) = (key.trim().to_ascii_lowercase(), value.trim());
            match (section.as_str(), key.as_str()) {
                ("sceneinfo", "scenename") => scene.name = value.to_owned(),
                ("sceneinfo", "defaultstartposition") => scene.start = vector(value)?,
                ("sceneinfo", "defaultstartdirection") => scene.start_direction = vector(value)?,
                ("environment", "terrainfile") => scene.terrain_file = value.to_owned(),
                ("environment", "terrainwidth") => scene.terrain.width = integer(value)? as u32,
                ("environment", "terrainbreadth") => scene.terrain.breadth = integer(value)? as u32,
                ("environment", "terrainwidthscale") => scene.terrain.width_scale = single(value)?,
                ("environment", "terrainbreadthscale") => scene.terrain.breadth_scale = single(value)?,
                ("environment", "terrainborderwidth") => scene.terrain.border_width = single(value)?,
                ("environment", "mindivisionsize") => scene.terrain.min_division_size = single(value)?,
                _ if collecting.is_some() => match key.as_str() {
                    "col" | "slt" => current.file = value.to_owned(),
                    "position" => current.position = vector(value)?,
                    "lookvector" => current.look = vector(value)?,
                    "upvector" => current.up = vector(value)?,
                    "type" => current.kind = Some(value.to_owned()),
                    _ => {}
                },
                _ => {}
            }
        }
        Scene::flush(&mut scene, &mut current, &mut collecting);
        if scene.terrain_file.is_empty() {
            return Err("the scene names no terrain file".to_owned());
        }
        Ok(scene)
    }

    fn flush(scene: &mut Scene, current: &mut Placement, collecting: &mut Option<bool>) {
        if let Some(is_collision) = collecting.take()
            && !current.file.is_empty()
        {
            let done = core::mem::take(current);
            if is_collision {
                scene.collision.push(done);
            } else {
                scene.models.push(done);
            }
        }
        *current = Placement::default();
    }
}

/// `model17` is `Model<n>`; `models` is not.
fn numbered(section: &str, prefix: &str) -> bool {
    let Some(rest) = section.strip_prefix(prefix) else { return false };
    !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())
}

fn single(value: &str) -> Result<F, String> {
    // The scene writes its singles as decimal text with six places. The game reads them as it reads
    // any ini float, `strtod` and then the truncating double-to-float ([`crate::phy::number`]); a host
    // parse rounds to nearest and is a bit high on most of free06's props (S96, the props' centres
    // against biplane.p2m2's state). The host parse still says what is not a number.
    value
        .trim()
        .parse::<f32>()
        .map(|_| F(fpu::operand(crate::phy::number(value.trim()).to_bits())))
        .map_err(|e| format!("{value:?} is not a number: {e}"))
}

fn integer(value: &str) -> Result<i32, String> {
    value
        .trim()
        .parse::<i32>()
        .map_err(|e| format!("{value:?} is not an integer: {e}"))
}

fn vector(value: &str) -> Result<V3, String> {
    let mut parts = value.split(',');
    let mut out = [F::ZERO; 3];
    for slot in &mut out {
        *slot = single(parts.next().ok_or_else(|| format!("{value:?} is not a vector"))?)?;
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------
// collision
// ---------------------------------------------------------------------------------------------------

/// Which collision class a `.col` holds. One header, read by one check, in front of two different
/// bodies — the word says which, so a reader does not have to know what the file is for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Class {
    /// A triangle mesh: everything the wheels can hit.
    Mesh = 0,
    /// Line segments: the parts of a vehicle that sweep against the world.
    Rays = 2,
}

/// The header both collision classes share.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ColHeader {
    version: u32,
    class: Class,
    surface: Option<u32>,
}

const COL_MAGIC: u32 = 1234;
const COL_TAG: &[u8] = b"col\0";

fn col_header(r: &mut Reader, want: Option<Class>) -> Result<ColHeader, String> {
    let magic = r.u32()?;
    let tag = r.take(4)?;
    if magic != COL_MAGIC || tag != COL_TAG {
        return Err(format!("not a collision file: magic {magic:#x}, tag {tag:?}"));
    }
    let version = r.u32()?;
    if version != 1 && version != 2 {
        return Err(format!("collision version {version} is not 1 or 2"));
    }
    let class = match r.u32()? {
        0 => Class::Mesh,
        2 => Class::Rays,
        // A third class, which gains three words in front of its transform and then does not account for
        // its own bytes under either body below. One file on this disc is one, and nothing places it, so
        // this says so rather than guessing at the bytes.
        other => return Err(format!("collision class {other} is not understood")),
    };
    if let Some(want) = want
        && class != want
    {
        return Err(format!("this is a {class:?} collision file, read as {want:?}"));
    }
    // Version 1 files predate the surface word; the loader reads it only for later ones.
    let surface = if version > 1 { Some(r.u32()?) } else { None };
    Ok(ColHeader { version, class, surface })
}

/// Either collision class, read without knowing in advance which a file is.
#[derive(Clone, Debug, PartialEq)]
pub enum Collision {
    Mesh(Mesh),
    Rays(Rays),
}

impl Collision {
    /// Read a `.col`, whichever class it holds.
    pub fn parse(bytes: &[u8]) -> Result<Collision, String> {
        let class = col_header(&mut Reader::new(bytes, "a collision file"), None)?.class;
        match class {
            Class::Mesh => Mesh::parse(bytes).map(Collision::Mesh),
            Class::Rays => Rays::parse(bytes).map(Collision::Rays),
        }
    }
}

/// A node of either collision class's bounding-volume tree: a sphere-ish volume, a box, and two children.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    pub volume: F,
    pub centre: V3,
    pub half_extent: V3,
    /// The two child words, as stored. [`Node::child`] says what each one means.
    pub children: [u16; 2],
}

/// What a tree node's child word points at.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Child {
    /// Another node, by index.
    Node(u16),
    /// A triangle (or, in the ray class, a segment), by index.
    Leaf(u16),
}

impl Node {
    /// The top bit of a child word marks a leaf; the rest is the index.
    pub fn child(&self, which: usize) -> Child {
        let word = self.children[which];
        if word & 0x8000 == 0 {
            Child::Node(word)
        } else {
            Child::Leaf(word & 0x7fff)
        }
    }
}

fn nodes(r: &mut Reader, count: u32) -> Result<Vec<Node>, String> {
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        out.push(Node {
            volume: r.f()?,
            centre: r.v3()?,
            half_extent: r.v3()?,
            children: [r.u16()?, r.u16()?],
        });
    }
    Ok(out)
}

/// One triangle of a collision mesh: its plane, and its three vertices by index.
///
/// The plane is stored whole — `normal · v + distance == 0` for each of the three vertices — so a test
/// against it costs a dot product and no cross products.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Triangle {
    pub distance: F,
    pub normal: V3,
    pub vertices: [u16; 3],
}

/// A mesh's vertices, in whichever of the two widths the file uses.
///
/// The loader reads either array straight off the disc without converting it, so this keeps the packed
/// form as it lies: the fixed-point scale that turns one into world units is not proved yet.
#[derive(Clone, Debug, PartialEq)]
pub enum Points {
    /// Three singles per vertex.
    Wide(Vec<V3>),
    /// Three shorts per vertex, fixed-point.
    Packed(Vec<[i16; 3]>),
}

impl Points {
    pub fn len(&self) -> usize {
        match self {
            Points::Wide(v) => v.len(),
            Points::Packed(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A collision mesh: the shape a `.col` holds for everything the wheels can hit.
///
/// One of these is loaded once per distinct `.col` and placed many times over by the scene — `free06`
/// places 353 shapes drawn from 92 files.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    pub version: u32,
    /// Which friction surface the whole mesh is, where the file is new enough to say.
    pub surface: Option<u32>,
    /// Four rows of three: the three axes, then the translation.
    pub transform: [V3; 4],
    pub vertices: Points,
    /// A second array of the same shape as the vertices, present in most files.
    pub normals: Option<Points>,
    pub nodes: Vec<Node>,
    pub triangles: Vec<Triangle>,
}

impl Mesh {
    /// Read a `.col` as the ordinary collision class reads it.
    pub fn parse(bytes: &[u8]) -> Result<Mesh, String> {
        let mut r = Reader::new(bytes, "a collision mesh");
        let header = col_header(&mut r, Some(Class::Mesh))?;
        let transform = [r.v3()?, r.v3()?, r.v3()?, r.v3()?];
        r.skip(4)?; // the transform's fourth column, which the loader fills in with 0,0,0,1 instead
        let vertex_count = r.u32()?;
        let node_count = r.u32()?;
        let triangle_count = r.u32()?;
        let packed = r.u32()?;
        let vertices = points(&mut r, vertex_count, packed)?;
        let normals = if r.u32()? == 0 { None } else { Some(points(&mut r, vertex_count, packed)?) };
        let nodes = nodes(&mut r, node_count)?;
        let mut triangles = Vec::with_capacity(triangle_count as usize);
        for _ in 0..triangle_count {
            triangles.push(Triangle {
                distance: r.f()?,
                normal: r.v3()?,
                vertices: [r.u16()?, r.u16()?, r.u16()?],
            });
        }
        r.finished()?;
        Ok(Mesh {
            version: header.version,
            surface: header.surface,
            transform,
            vertices,
            normals,
            nodes,
            triangles,
        })
    }
}

fn points(r: &mut Reader, count: u32, packed: u32) -> Result<Points, String> {
    if packed == 0 {
        let mut out = Vec::with_capacity(count as usize);
        for _ in 0..count {
            out.push(r.v3()?);
        }
        Ok(Points::Wide(out))
    } else {
        let mut out = Vec::with_capacity(count as usize);
        for _ in 0..count {
            out.push([r.u16()? as i16, r.u16()? as i16, r.u16()? as i16]);
        }
        Ok(Points::Packed(out))
    }
}

/// One line segment of a ray collision object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub from: V3,
    pub to: V3,
}

/// The other collision class: line segments in a tree instead of triangles, used for the parts of a
/// vehicle that sweep against the world. On the disc these are the `*m.col` files.
#[derive(Clone, Debug, PartialEq)]
pub struct Rays {
    pub version: u32,
    pub surface: Option<u32>,
    /// Four rows of four, stored whole here where the mesh class stores three columns.
    pub transform: [[F; 4]; 4],
    pub nodes: Vec<Node>,
    pub segments: Vec<Segment>,
}

impl Rays {
    /// Read a `.col` as the ray collision class reads it. Which class a file belongs to is the caller's
    /// to know — both share one header, and the game picks the loader by what it is loading the file for.
    pub fn parse(bytes: &[u8]) -> Result<Rays, String> {
        let mut r = Reader::new(bytes, "a ray collision object");
        let header = col_header(&mut r, Some(Class::Rays))?;
        let mut transform = [[F::ZERO; 4]; 4];
        for row in &mut transform {
            for cell in row.iter_mut() {
                *cell = r.f()?;
            }
        }
        let segment_count = r.u32()?;
        let node_count = r.u32()?;
        let repeated = r.u32()?;
        if repeated != segment_count {
            return Err(format!("segment count {segment_count} then {repeated}"));
        }
        let nodes = nodes(&mut r, node_count)?;
        let mut segments = Vec::with_capacity(segment_count as usize);
        for _ in 0..segment_count {
            segments.push(Segment { from: r.v3()?, to: r.v3()? });
        }
        r.finished()?;
        Ok(Rays { version: header.version, surface: header.surface, transform, nodes, segments })
    }
}

// ---------------------------------------------------------------------------------------------------
// the terrain
// ---------------------------------------------------------------------------------------------------

/// A terrain leaf block: 17×17 samples of height, normal and surface, in 2129 bytes.
///
/// ```text
///  +0x000  i32         the block's base height, in raw units
///  +0x004  u16 × 340   the heightfield: 17 columns of a 20-wide row, 17 rows
///  +0x2ac  i8  × 867   one normal per sample, three signed bytes, 17-wide rows
///  +0x60f  u8  × 289   one byte per sample, not yet read
///  +0x730  u8  × 289   one surface id per sample, in the low seven bits
/// ```
pub const BLOCK_BYTES: usize = 2129;

/// How wide a leaf block's sample grid is. The 17th row and column repeat the next block's first.
pub const BLOCK_SAMPLES: u32 = 17;

/// How many children a terrain tree node has, across and along.
pub const TREE_ARITY: u32 = 16;

const BLOCK_HEIGHTS: usize = 4;
const BLOCK_HEIGHT_STRIDE: usize = 20;
const BLOCK_NORMALS: usize = BLOCK_HEIGHTS + 680;
const BLOCK_UNKNOWN: usize = BLOCK_NORMALS + 867;
const BLOCK_SURFACES: usize = BLOCK_UNKNOWN + 289;

/// One node of the terrain tree, as the file stores it. 97 bytes, children table immediately after.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainNode {
    /// How much of the block's relief this node uses: 1.0 for the block as authored, less for a node
    /// that reuses a block flattened towards its base height.
    pub vertical_scale: F,
    /// Raw height units to local height units. The same value for every node of a terrain.
    pub height_scale: F,
    /// The node's own bounds in local height units, which is what the two scales above produce.
    pub min_y: F,
    pub max_y: F,
    /// Which leaf block this node samples. Blocks are shared: 2304 leaves over 842 blocks on `free06`.
    pub block: u16,
    /// File offset of this node's 16×16 table of child offsets, or 0 at a leaf.
    pub children_at: u32,
    /// Fields read but not yet understood: a word in front of the block index, the word after it, a
    /// 16-word table, the word after that, and a byte.
    pub unread: Unread,
}

/// The parts of a terrain node whose meaning is not read yet, kept so the layout stays honest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unread {
    pub before_block: u16,
    pub after_block: u32,
    pub table: [u32; 16],
    pub after_table: u32,
    pub flags: u8,
}

const NODE_BYTES: usize = 97;

/// The terrain: a quantised heightfield cut into blocks, with a 16×16-ary tree over them.
///
/// Heights are stored as one `u16` per sample and come back as `height_scale × base + vertical_scale ×
/// (raw − base)`, which is how the game's own query computes them — see the journal for the `./kit fn`
/// recipe. That value is in local units; world y is it times the terrain's vertical scale, which is this
/// file's own [`Terrain::scale`].
#[derive(Clone, Debug, PartialEq)]
pub struct Terrain<'a> {
    bytes: &'a [u8],
    /// The version character out of the tag, `'7'` on this disc.
    pub version: u8,
    /// The platform tag, `b"ps2\0"`.
    pub platform: [u8; 4],
    /// The palette's name, which is also the only name the file carries.
    pub name: String,
    /// World units per sample, and the multiplier from a local height to a world one.
    pub scale: F,
    /// How many times the tree subdivides before reaching a leaf.
    pub levels: u32,
    /// How many leaf blocks the file holds.
    pub blocks: u32,
    palette_at: Option<usize>,
    blocks_at: usize,
    root_at: usize,
}

impl<'a> Terrain<'a> {
    /// Read a `.trn` header and check that its blocks account exactly for the space in front of its tree.
    pub fn parse(bytes: &'a [u8]) -> Result<Terrain<'a>, String> {
        let mut r = Reader::new(bytes, "a terrain");
        let tag = r.take(4)?;
        if tag != b"PLN\0" {
            return Err(format!("not a terrain: tag {tag:?}"));
        }
        let version = r.u8()?;
        let platform: [u8; 4] = r.take(4)?.try_into().expect("4 bytes");
        let name_bytes = r.take(32)?;
        let name = String::from_utf8_lossy(name_bytes.split(|&b| b == 0).next().unwrap_or(b""))
            .into_owned();
        let palette_at = if r.u32()? == 0 {
            None
        } else {
            let at = r.at;
            r.skip(1024)?;
            Some(at)
        };
        let scale = r.f()?;
        let levels = r.i32()?;
        let tree_at = r.i32()?;
        let declared = r.i32()?;
        let blocks = r.i32()?;
        if levels < 0 || tree_at < 0 || declared < 0 || blocks < 0 {
            return Err(format!("terrain counts {levels}, {tree_at}, {declared}, {blocks}"));
        }
        // One word per block, in front of the blocks themselves and not read by anything here.
        r.skip(declared as usize * 4)?;
        let textures = r.i32()?;
        let _textures_again = r.i32()?;
        if textures < 0 {
            return Err(format!("terrain texture count {textures}"));
        }
        r.skip(textures as usize * 4)?;

        let blocks_at = r.at;
        let blocks_end = blocks_at
            .checked_add(blocks as usize * BLOCK_BYTES)
            .ok_or("the terrain's blocks overflow")?;
        if blocks_end != tree_at as usize {
            return Err(format!(
                "{blocks} blocks of {BLOCK_BYTES} from {blocks_at} end at {blocks_end}, \
                 but the tree starts at {tree_at}"
            ));
        }
        if tree_at as usize + NODE_BYTES > bytes.len() {
            return Err(format!("the tree at {tree_at} does not fit a {}-byte file", bytes.len()));
        }
        Ok(Terrain {
            bytes,
            version,
            platform,
            name,
            scale,
            levels: levels as u32,
            blocks: blocks as u32,
            palette_at,
            blocks_at,
            root_at: tree_at as usize,
        })
    }

    /// The 1024-byte palette, where the file carries one.
    pub fn palette(&self) -> Option<&'a [u8]> {
        self.palette_at.map(|at| &self.bytes[at..at + 1024])
    }

    /// Where the tree's root node sits in the file.
    pub fn root_at(&self) -> u32 {
        self.root_at as u32
    }

    /// The root node: the whole terrain's bounds, and the table of its root cells.
    pub fn root(&self) -> Option<TerrainNode> {
        self.node(self.root_at as u32)
    }

    /// One node, by file offset.
    pub fn node(&self, at: u32) -> Option<TerrainNode> {
        let mut r = Reader::new(self.bytes.get(at as usize..)?, "a terrain node");
        let vertical_scale = r.f().ok()?;
        let height_scale = r.f().ok()?;
        let min_y = r.f().ok()?;
        let max_y = r.f().ok()?;
        let before_block = r.u16().ok()?;
        let block = r.u16().ok()?;
        let after_block = r.u32().ok()?;
        let mut table = [0u32; 16];
        for slot in &mut table {
            *slot = r.u32().ok()?;
        }
        let after_table = r.u32().ok()?;
        let flags = r.u8().ok()?;
        let children_at = r.u32().ok()?;
        Some(TerrainNode {
            vertical_scale,
            height_scale,
            min_y,
            max_y,
            block,
            children_at,
            unread: Unread { before_block, after_block, table, after_table, flags },
        })
    }

    /// One of a node's 16×16 children, by file offset, or `None` where the terrain has no such cell.
    pub fn child(&self, node: &TerrainNode, cx: u32, cz: u32) -> Option<u32> {
        if node.children_at == 0 || cx >= TREE_ARITY || cz >= TREE_ARITY {
            return None;
        }
        let at = node.children_at as usize + (cx + cz * TREE_ARITY) as usize * 4;
        let word = u32::from_le_bytes(self.bytes.get(at..at + 4)?.try_into().expect("4 bytes"));
        if word == 0 { None } else { Some(word) }
    }

    /// Walk down to the leaf holding one terrain sample, and say where in that leaf it lands.
    ///
    /// This is the game's own descent: shift the sample coordinates down by four bits per level, take the
    /// child at that cell, mask the coordinates back and go again, exactly `levels` times.
    pub fn leaf(&self, sx: u32, sz: u32) -> Option<(TerrainNode, u32, u32)> {
        let mut node = self.root()?;
        let (mut sx, mut sz) = (sx, sz);
        let mut shift = self.levels * 4;
        for _ in 0..self.levels {
            let (cx, cz) = (sx >> shift, sz >> shift);
            if cx >= TREE_ARITY || cz >= TREE_ARITY {
                return None;
            }
            let at = self.child(&node, cx, cz)?;
            node = self.node(at)?;
            let mask = (1u32 << shift) - 1;
            sx &= mask;
            sz &= mask;
            shift -= 4;
        }
        Some((node, sx, sz))
    }

    /// One leaf block's bytes, by index.
    pub fn block(&self, index: u16) -> Option<Block<'a>> {
        if u32::from(index) >= self.blocks {
            return None;
        }
        let at = self.blocks_at + index as usize * BLOCK_BYTES;
        Block::new(self.bytes.get(at..at + BLOCK_BYTES)?)
    }

    /// The four corners of one terrain cell: the height, normal and surface id the game's query returns.
    ///
    /// `step` is the cell's width in samples — the game asks for 1 or 2 depending on how far away the
    /// terrain is being looked at. The corners come back in the order the original writes them: the
    /// sample itself, then +x, then +z, then both.
    pub fn cell(&self, sx: u32, sz: u32, step: u32) -> Option<Quad> {
        let (node, lx, lz) = self.leaf(sx, sz)?;
        let block = self.block(node.block)?;
        // The original does not check this; its callers only ever ask for a cell inside the leaf.
        if lx + step >= BLOCK_SAMPLES || lz + step >= BLOCK_SAMPLES {
            return None;
        }
        let corners = [(0, 0), (step, 0), (0, step), (step, step)];
        let mut height = [F::ZERO; 4];
        let mut normal = [[F::ZERO; 4]; 4];
        let mut surface = [0u8; 4];
        for (i, (dx, dz)) in corners.into_iter().enumerate() {
            let (x, z) = (lx + dx, lz + dz);
            height[i] = node.height(block.raw_height(x, z)?, block.base);
            let n = block.raw_normal(x, z)?;
            // The relief is flattened by the node's vertical scale, so the gradient — and with it the
            // normal's horizontal part — is scaled the same way, and y is left alone.
            normal[i] = [
                F::from(i32::from(n[0])) * node.vertical_scale,
                F::from(i32::from(n[1])),
                F::from(i32::from(n[2])) * node.vertical_scale,
                F(1.0),
            ];
            surface[i] = block.surface(x, z)?;
        }
        Some(Quad { height, normal, surface })
    }
}

impl TerrainNode {
    /// One raw sample as a local height, the way the game's query computes it.
    ///
    /// A node at full relief is the plain product; a flattened one pulls every sample towards its
    /// block's base height, with the subtraction done in integers before the conversion, as the original
    /// does it.
    pub fn height(&self, raw: u16, base: i32) -> F {
        if self.vertical_scale == F(1.0) {
            self.height_scale * F::from(i32::from(raw))
        } else {
            let relief = F::from(i32::from(raw).wrapping_sub(base));
            self.height_scale * (F::from(base) + self.vertical_scale * relief)
        }
    }
}

/// The four corners of one terrain cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    /// Local heights — world y is these times the terrain's scale.
    pub height: [F; 4],
    /// Per-corner normals, each with 1.0 in its fourth lane, unnormalised as the original leaves them.
    pub normal: [[F; 4]; 4],
    /// Per-corner surface id, which indexes the friction surfaces in a `.phy`.
    pub surface: [u8; 4],
}

/// One leaf block of the terrain, borrowed out of the file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Block<'a> {
    bytes: &'a [u8],
    /// The height every sample in this block is measured from, in raw units.
    pub base: i32,
}

impl<'a> Block<'a> {
    fn new(bytes: &'a [u8]) -> Option<Block<'a>> {
        if bytes.len() < BLOCK_BYTES {
            return None;
        }
        let base = i32::from_le_bytes(bytes[0..4].try_into().expect("4 bytes"));
        Some(Block { bytes, base })
    }

    /// One sample's height before any scaling. Rows are 20 samples apart, of which 17 are used.
    pub fn raw_height(&self, x: u32, z: u32) -> Option<u16> {
        if x >= BLOCK_SAMPLES || z >= BLOCK_SAMPLES {
            return None;
        }
        let i = x as usize + z as usize * BLOCK_HEIGHT_STRIDE;
        u16_at(self.bytes, BLOCK_HEIGHTS + i * 2)
    }

    /// One sample's normal, three signed bytes as stored.
    pub fn raw_normal(&self, x: u32, z: u32) -> Option<[i8; 3]> {
        if x >= BLOCK_SAMPLES || z >= BLOCK_SAMPLES {
            return None;
        }
        let at = BLOCK_NORMALS + (x as usize + z as usize * BLOCK_SAMPLES as usize) * 3;
        let b = self.bytes.get(at..at + 3)?;
        Some([b[0] as i8, b[1] as i8, b[2] as i8])
    }

    /// One sample's surface id. The top bit is not part of it.
    pub fn surface(&self, x: u32, z: u32) -> Option<u8> {
        if x >= BLOCK_SAMPLES || z >= BLOCK_SAMPLES {
            return None;
        }
        let at = BLOCK_SURFACES + x as usize + z as usize * BLOCK_SAMPLES as usize;
        Some(self.bytes.get(at).copied()? & 0x7f)
    }

    /// The one byte per sample between the normals and the surface ids, whose meaning is not read yet.
    pub fn unread(&self, x: u32, z: u32) -> Option<u8> {
        if x >= BLOCK_SAMPLES || z >= BLOCK_SAMPLES {
            return None;
        }
        let at = BLOCK_UNKNOWN + x as usize + z as usize * BLOCK_SAMPLES as usize;
        self.bytes.get(at).copied()
    }
}

// ---------------------------------------------------------------------------------------------------
// the bike a freeride track starts with
// ---------------------------------------------------------------------------------------------------

/// Which engine class a bike's physics parameters are for. The index the game keeps is this order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Engine {
    Cc50 = 0,
    Cc125 = 1,
    Cc250 = 2,
    Cc500 = 3,
}

/// Which of the two tunings of a class a bike uses. Both files are built from one engine class, and both
/// are loaded together.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tuning {
    /// The `…A.phy` of the pair.
    A,
    /// The `…P.phy` of the pair.
    P,
}

/// The name of a bike's physics parameter file, the way the game builds it: one of sixteen names from an
/// engine class, a tier and which of the pair is wanted.
///
/// The loader formats `<prefix><tier><cc><A|P>.phy` and the prefix is a global the engine fills in once
/// at startup with `Bike\`, so the name it asks the resource system for is spelled exactly like this.
/// The archives store those members lowercased (`bike\race125a.phy`), so a lookup against them has to
/// fold case.
///
/// All sixteen names exist on the disc, so the archive is no evidence about which pair a mode loads:
/// the `.phy` pairs ship inside the bike *model* archives — a 125 model carries `race125*`/`pro125*`,
/// a 250 model its own — while a track's own second archive carries the 250 pair and the wreck bike.
/// Which pair is asked for comes from the event's own two fields and nothing else; `pro` is the career's
/// upgraded tier.
pub fn physics_file(engine: Engine, pro: bool, tuning: Tuning) -> String {
    let tier = if pro { "Pro" } else { "Race" };
    let cc = match engine {
        Engine::Cc50 => 50,
        Engine::Cc125 => 125,
        Engine::Cc250 => 250,
        Engine::Cc500 => 500,
    };
    let which = match tuning {
        Tuning::A => 'A',
        Tuning::P => 'P',
    };
    format!("Bike\\{tier}{cc}{which}.phy")
}

/// What a freeride track's bike is, absent a career profile saying otherwise: the 125 of the lower tier,
/// i.e. `Bike\Race125P.phy` and `Bike\Race125A.phy`.
///
/// Both come from the event the loader reads, whose constructor defaults the class to 125 and the tier to
/// Race and which nothing on the freeride path writes — measured in a world the original had loaded, and
/// in a scripted boot that set the mode but deliberately left the two fields alone (D7). The 250 the port
/// used to assume is what the `.cfg` keys default to, and those keys are gated on a section the retail
/// disc does not ship.
pub const FREERIDE_ENGINE: Engine = Engine::Cc125;

/// Which tier a freeride track's bike is, same source as [`FREERIDE_ENGINE`]: the Race files, not the Pro
/// ones.
pub const FREERIDE_PRO: bool = false;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scene_is_read_section_by_section() {
        let scn = b"[SceneInfo]\r\n\
                    SceneName=Example\r\n\
                    DefaultStartPosition=2884.00,338.00,2750.00\r\n\
                    DefaultStartDirection=0.00,0.00,1.00\r\n\
                    \r\n\
                    [Environment]\r\n\
                    TerrainFile=Free06.trn\r\n\
                    TerrainWidth=3\r\n\
                    TerrainWidthScale=9.000000\r\n\
                    MinDivisionSize=96.000000\r\n\
                    \r\n\
                    [StaticModels]\r\n\
                    NumberOfStaticModels=1\r\n\
                    \r\n\
                    [Model1]\r\n\
                    SLT=MM_HTTower\r\n\
                    Position=1.0,2.0,3.0\r\n\
                    \r\n\
                    [CollisionObjects]\r\n\
                    NumberOfCollisionObjects=1\r\n\
                    \r\n\
                    [Object1]\r\n\
                    COL=MM_HTTower.col\r\n\
                    Position=4.0,5.0,6.0\r\n\
                    LookVector=1.0,0.0,0.0\r\n";
        let scene = Scene::parse(scn).expect("the scene");
        assert_eq!(scene.name, "Example");
        assert_eq!(scene.start, [F(2884.0), F(338.0), F(2750.0)]);
        assert_eq!(scene.terrain_file, "Free06.trn");
        assert_eq!(scene.terrain.width, 3);
        assert_eq!(scene.terrain.width_scale, F(9.0));
        assert_eq!(scene.models.len(), 1);
        assert_eq!(scene.models[0].file, "MM_HTTower");
        // No LookVector in the file means facing +Z.
        assert_eq!(scene.models[0].look, [F::ZERO, F::ZERO, F(1.0)]);
        assert_eq!(scene.collision.len(), 1);
        assert_eq!(scene.collision[0].file, "MM_HTTower.col");
        assert_eq!(scene.collision[0].look, [F(1.0), F::ZERO, F::ZERO]);
    }

    #[test]
    fn a_child_word_says_node_or_leaf() {
        let node = Node {
            volume: F::ZERO,
            centre: [F::ZERO; 3],
            half_extent: [F::ZERO; 3],
            children: [2, 0x8003],
        };
        assert_eq!(node.child(0), Child::Node(2));
        assert_eq!(node.child(1), Child::Leaf(3));
    }

    #[test]
    fn a_flattened_node_pulls_its_samples_towards_the_blocks_base() {
        let full = TerrainNode {
            vertical_scale: F(1.0),
            height_scale: F(0.5),
            min_y: F::ZERO,
            max_y: F::ZERO,
            block: 0,
            children_at: 0,
            unread: Unread { before_block: 0, after_block: 0, table: [0; 16], after_table: 0, flags: 0 },
        };
        let half = TerrainNode { vertical_scale: F(0.5), ..full };
        assert_eq!(full.height(100, 60), F(50.0));
        // 60 + 0.5 x (100 - 60) = 80, halved again by the height scale.
        assert_eq!(half.height(100, 60), F(40.0));
    }

    #[test]
    fn the_sixteen_physics_names_are_a_class_and_a_tier() {
        assert_eq!(physics_file(Engine::Cc250, false, Tuning::P), "Bike\\Race250P.phy");
        assert_eq!(physics_file(Engine::Cc250, true, Tuning::A), "Bike\\Pro250A.phy");
        assert_eq!(physics_file(Engine::Cc50, false, Tuning::A), "Bike\\Race50A.phy");
        // The freeride pair, the one the physics proofs are compared against.
        assert_eq!(physics_file(FREERIDE_ENGINE, FREERIDE_PRO, Tuning::P), "Bike\\Race125P.phy");
        assert_eq!(physics_file(FREERIDE_ENGINE, FREERIDE_PRO, Tuning::A), "Bike\\Race125A.phy");
    }
}
