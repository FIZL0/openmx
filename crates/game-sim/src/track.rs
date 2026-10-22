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

/// Which collision class a `.col` holds. One header, read by one check, in front of three different
/// bodies — the word says which, so a reader does not have to know what the file is for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Class {
    /// A triangle mesh: everything the wheels can hit.
    Mesh = 0,
    /// Several triangle meshes, each tied to a bone of the model that loads it: the moving parts of an
    /// animated toy.
    Hierarchy = 1,
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
        1 => Class::Hierarchy,
        2 => Class::Rays,
        // 8 is a finite plane, which the loader can build but no file on the disc asks for (S49).
        other => return Err(format!("collision class {other} is not understood")),
    };
    if let Some(want) = want
        && class != want
    {
        return Err(format!("this is a {class:?} collision file, read as {want:?}"));
    }
    // Version 1 files predate the surface word; the loader reads it only for later ones. A hierarchy has
    // none of its own: each of its parts carries one.
    let surface = if version > 1 && class != Class::Hierarchy { Some(r.u32()?) } else { None };
    Ok(ColHeader { version, class, surface })
}

/// Either collision class, read without knowing in advance which a file is.
#[derive(Clone, Debug, PartialEq)]
pub enum Collision {
    Mesh(Mesh),
    Hierarchy(Hierarchy),
    Rays(Rays),
}

impl Collision {
    /// Read a `.col`, whichever class it holds.
    pub fn parse(bytes: &[u8]) -> Result<Collision, String> {
        let class = col_header(&mut Reader::new(bytes, "a collision file"), None)?.class;
        match class {
            Class::Mesh => Mesh::parse(bytes).map(Collision::Mesh),
            Class::Hierarchy => Hierarchy::parse(bytes).map(Collision::Hierarchy),
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
        let mesh = mesh_body(&mut r, header.version, header.surface)?;
        r.finished()?;
        Ok(mesh)
    }
}

/// A mesh's body, from its transform to its last triangle: the part both the mesh class and each part of
/// a hierarchy read the same way.
fn mesh_body(r: &mut Reader, version: u32, surface: Option<u32>) -> Result<Mesh, String> {
    let transform = [r.v3()?, r.v3()?, r.v3()?, r.v3()?];
    r.skip(4)?; // the transform's fourth column, which the loader fills in with 0,0,0,1 instead
    let vertex_count = r.u32()?;
    let node_count = r.u32()?;
    let triangle_count = r.u32()?;
    let packed = r.u32()?;
    let vertices = points(r, vertex_count, packed)?;
    let normals = if r.u32()? == 0 { None } else { Some(points(r, vertex_count, packed)?) };
    let nodes = nodes(r, node_count)?;
    let mut triangles = Vec::with_capacity(triangle_count as usize);
    for _ in 0..triangle_count {
        triangles.push(Triangle {
            distance: r.f()?,
            normal: r.v3()?,
            vertices: [r.u16()?, r.u16()?, r.u16()?],
        });
    }
    Ok(Mesh {
        version,
        surface,
        transform,
        vertices,
        normals,
        nodes,
        triangles,
    })
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

/// One part of a hierarchy: a mesh, and the bone of the loading model it rides on.
#[derive(Clone, Debug, PartialEq)]
pub struct BonePart {
    /// Which bone, counted through the model's skeleton from its root in the order the loader flattens
    /// it. As stored: the loader clamps it to the last bone when it binds the part, and binds nothing at
    /// all when the owner has no skeleton.
    pub bone: u32,
    /// The part's mesh. Its `surface` is the part's own word, where the file is new enough to have one;
    /// the loader keeps that word's low byte.
    pub mesh: Mesh,
}

/// The third collision class: several meshes, each tied to one bone of the model that loads the file.
///
/// It is what an animated toy with `HasCollision` loads — a windmill's blades, a gondola, a shovel — and
/// the toy then splits it into one ordinary mesh collidable per part, each following its bone, so the
/// clip that turns the model turns its collision with it. MX Unleashed's disc has 21 of these files.
#[derive(Clone, Debug, PartialEq)]
pub struct Hierarchy {
    pub version: u32,
    /// The word after the part count, which the loader reads and drops. 0 in every file on the disc.
    pub unread: u32,
    /// The parts the loader keeps, in file order. A part with no vertices is read and then dropped, and
    /// the next one takes its place.
    pub parts: Vec<BonePart>,
}

impl Hierarchy {
    /// Read a `.col` as the hierarchy class reads it: a part count and an unused word, then for each
    /// part its bone, its surface word (version 2 and later) and an ordinary mesh body.
    pub fn parse(bytes: &[u8]) -> Result<Hierarchy, String> {
        let mut r = Reader::new(bytes, "a collision hierarchy");
        let header = col_header(&mut r, Some(Class::Hierarchy))?;
        let count = r.u32()?;
        let unread = r.u32()?;
        let mut parts = Vec::new();
        for _ in 0..count {
            let bone = r.u32()?;
            let surface = if header.version > 1 { Some(r.u32()?) } else { None };
            let mesh = mesh_body(&mut r, header.version, surface)?;
            if !mesh.vertices.is_empty() {
                parts.push(BonePart { bone, mesh });
            }
        }
        r.finished()?;
        Ok(Hierarchy { version: header.version, unread, parts })
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
    /// The images stored between the blocks and the tree. The free tracks carry none; the nationals
    /// carry 32 or 48. MX vs. ATV's and Untamed's terrains are this same layout with 16 to 64 of them.
    pub textures: Vec<TerrainTexture<'a>>,
    palette_at: Option<usize>,
    blocks_at: usize,
    root_at: usize,
}

impl<'a> Terrain<'a> {
    /// Read a `.trn` header and check that its blocks and textures account exactly for the space in
    /// front of its tree.
    ///
    /// The loader reads the blocks, then a count of textures and that many of them, one after another,
    /// and then the tree. The word table ahead of the blocks says where each texture starts; the
    /// loader skips it, and the reader here checks it against where each one does start.
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
        let table = r.i32()?;
        let stored = r.i32()?;
        if table < 0 || stored < 0 {
            return Err(format!("terrain texture counts {table}, {stored}"));
        }
        let mut starts = Vec::with_capacity(table as usize);
        for _ in 0..table {
            starts.push(r.u32()? as usize);
        }

        let blocks_at = r.at;
        r.skip(blocks as usize * BLOCK_BYTES)
            .map_err(|_| format!("{blocks} blocks of {BLOCK_BYTES} from {blocks_at} overrun the file"))?;
        let blocks_end = r.at;
        let mut textures = Vec::with_capacity(stored as usize);
        for i in 0..stored as usize {
            if let Some(&want) = starts.get(i) {
                if want != r.at {
                    return Err(format!("terrain texture {i} starts at {}, but its table says {want}", r.at));
                }
            }
            textures.push(TerrainTexture::read(&mut r).map_err(|e| format!("terrain texture {i}: {e}"))?);
        }
        if r.at != tree_at as usize {
            return Err(format!(
                "{blocks} blocks of {BLOCK_BYTES} from {blocks_at} end at {blocks_end} and {stored} \
                 textures after them at {}, but the tree starts at {tree_at}",
                r.at
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
            textures,
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

/// One image a terrain file carries, as the game's texture reader takes it: a format word, a level
/// count, and per level a 16-byte header — width, height, byte count, and a word the reader does not
/// use — followed by that many bytes.
///
/// Every one on this disc is format 39, one level of 256×256 bytes, a byte to a pixel.
/// Formats 39 and 40 are the two whose levels the reader takes whole like this; the others are refused.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainTexture<'a> {
    /// The pixel format word the terrain loader hands the texture reader.
    pub format: u32,
    /// The levels in the order the file stores them.
    pub levels: Vec<TextureLevel<'a>>,
}

/// One level of a [`TerrainTexture`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureLevel<'a> {
    pub width: u32,
    pub height: u32,
    /// The header's fourth word, which the reader does not use: a pointer left over from the tool that
    /// wrote the file.
    pub unread: u32,
    pub pixels: &'a [u8],
}

impl<'a> TerrainTexture<'a> {
    fn read(r: &mut Reader<'a>) -> Result<TerrainTexture<'a>, String> {
        let format = r.u32()?;
        // The reader's two palette formats take their levels some other way, and no terrain on this
        // disc uses them, so only the two whose layout is read here are accepted.
        if format != 39 && format != 40 {
            return Err(format!("format {format}"));
        }
        let count = r.u32()?;
        let mut levels = Vec::new();
        for _ in 0..count {
            let width = r.u32()?;
            let height = r.u32()?;
            let size = r.u32()?;
            let unread = r.u32()?;
            let pixels = r.take(size as usize)?;
            levels.push(TextureLevel { width, height, unread, pixels });
        }
        Ok(TerrainTexture { format, levels })
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

    fn word(out: &mut Vec<u8>, w: u32) {
        out.extend_from_slice(&w.to_le_bytes());
    }

    fn terrain_node(out: &mut Vec<u8>, vertical_scale: F, block: u16, children_at: u32) {
        for f in [vertical_scale, F(0.5), F::ZERO, F::ZERO] {
            word(out, f.0.to_bits());
        }
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&block.to_le_bytes());
        out.extend_from_slice(&[0; 4 + 64 + 4 + 1]);
        word(out, children_at);
    }

    /// A one-level terrain the way MX vs. ATV writes them: a palette, two blocks, a texture table of
    /// two, the two pages between the last block and the tree, then the tree. Returns the bytes and
    /// where the table, the pages and the tree sit.
    fn terrain_with_pages() -> (Vec<u8>, [usize; 2], usize) {
        let mut out = Vec::new();
        out.extend_from_slice(b"PLN\0");
        out.push(b'7');
        out.extend_from_slice(b"ps2\0");
        let mut name = [0u8; 32];
        name[..6].copy_from_slice(b"mdtest");
        out.extend_from_slice(&name);
        word(&mut out, 1);
        out.extend((0..1024).map(|i| i as u8));
        word(&mut out, F(4.5).0.to_bits());
        word(&mut out, 1);
        let tree_at_slot = out.len();
        word(&mut out, 0);
        word(&mut out, 2); // the words in front of the blocks
        word(&mut out, 2); // blocks
        word(&mut out, 0);
        word(&mut out, 0);
        word(&mut out, 2); // the texture table
        word(&mut out, 2); // the textures stored
        let table_at = out.len();
        word(&mut out, 0);
        word(&mut out, 0);
        for b in 0..2u32 {
            let start = out.len();
            word(&mut out, 100 * b);
            for z in 0..BLOCK_HEIGHT_STRIDE as u32 {
                for x in 0..BLOCK_HEIGHT_STRIDE as u32 {
                    if z < BLOCK_SAMPLES {
                        out.extend_from_slice(&(1000 * b + x + 20 * z).min(0xffff).to_le_bytes()[..2]);
                    }
                }
            }
            assert_eq!(out.len(), start + BLOCK_NORMALS);
            for i in 0..289u32 {
                out.extend_from_slice(&[(i % 7) as u8, 100, (b as u8).wrapping_sub(3)]);
            }
            out.extend(std::iter::repeat_n(0xee, 289));
            out.extend((0..289u32).map(|i| 0x80 | (i % 5) as u8 | (b as u8) << 4));
            assert_eq!(out.len(), start + BLOCK_BYTES);
        }
        // Two pages: the 256×256 byte one every terrain on the discs carries (format, level count, then
        // width, height, size and the unused word: 24 bytes), and a smaller format-40 one of two levels,
        // each level's header in front of its own bytes.
        let mut pages = [0; 2];
        pages[0] = out.len();
        for w in [39, 1, 256, 256, 65536, 77] {
            word(&mut out, w);
        }
        out.extend((0..65536u32).map(|i| (i * 7) as u8));
        pages[1] = out.len();
        for w in [40, 2, 8, 4, 32, 0] {
            word(&mut out, w);
        }
        out.extend(std::iter::repeat_n(0x11, 32));
        for w in [4, 2, 8, 0] {
            word(&mut out, w);
        }
        out.extend(std::iter::repeat_n(0x22, 8));
        for (i, at) in pages.into_iter().enumerate() {
            out[table_at + 4 * i..table_at + 4 * i + 4].copy_from_slice(&(at as u32).to_le_bytes());
        }

        // The tree: a root whose table names two leaves, block 0 at full relief and block 1 flattened.
        let tree_at = out.len();
        out[tree_at_slot..tree_at_slot + 4].copy_from_slice(&(tree_at as u32).to_le_bytes());
        let children_at = (tree_at + NODE_BYTES) as u32;
        let leaves_at = children_at + 256 * 4;
        terrain_node(&mut out, F(1.0), 0, children_at);
        let mut children = [0u32; 256];
        children[0] = leaves_at;
        children[1] = leaves_at + NODE_BYTES as u32;
        for c in children {
            word(&mut out, c);
        }
        terrain_node(&mut out, F(1.0), 0, 0);
        terrain_node(&mut out, F(0.5), 1, 0);
        (out, pages, tree_at)
    }

    /// MD1: a terrain with a texture table and pages between its last block and its tree, as every MX
    /// vs. ATV one has, reads — the pages come out where the table says, and the cells read through the
    /// tree the same as they would without the pages.
    #[test]
    fn a_terrain_with_pages_before_its_tree_reads_and_hands_them_out() {
        let (bytes, pages, tree_at) = terrain_with_pages();
        let t = Terrain::parse(&bytes).expect("parses");
        assert_eq!((t.version, &t.platform, t.name.as_str()), (b'7', b"ps2\0", "mdtest"));
        assert_eq!((t.scale, t.levels, t.blocks, t.root_at()), (F(4.5), 1, 2, tree_at as u32));
        assert_eq!(t.palette().map(|p| (p.len(), p[1], p[1023])), Some((1024, 1, 255)));

        assert_eq!(t.textures.len(), 2);
        let page = &t.textures[0];
        assert_eq!((page.format, page.levels.len()), (39, 1));
        let l = page.levels[0];
        assert_eq!((l.width, l.height, l.unread, l.pixels.len()), (256, 256, 77, 65536));
        assert_eq!(l.pixels.as_ptr(), bytes[pages[0] + 24..].as_ptr());
        assert_eq!((l.pixels[0], l.pixels[1], l.pixels[65535]), (0, 7, (65535u32 * 7) as u8));
        let small = &t.textures[1];
        assert_eq!(small.format, 40);
        let sizes: Vec<_> = small.levels.iter().map(|l| (l.width, l.height, l.pixels.len(), l.pixels[0])).collect();
        assert_eq!(sizes, [(8, 4, 32, 0x11), (4, 2, 8, 0x22)]);

        // Block 0 at full relief: the plain product of the raw sample and the height scale.
        let q = t.cell(2, 3, 1).expect("a cell of the first leaf");
        let raw = [2 + 60, 3 + 60, 2 + 80, 3 + 80];
        for i in 0..4 {
            assert_eq!(q.height[i], F(0.5) * F::from(raw[i]), "corner {i}");
        }
        assert_eq!(q.surface, [(2 + 51) % 5, (3 + 51) % 5, (2 + 68) % 5, (3 + 68) % 5].map(|s| s as u8));
        assert_eq!(q.normal[0], [F::from(((2 + 51) % 7) as i32), F(100.0), F(-3.0), F(1.0)]);
        // Block 1 through the flattened leaf: pulled halfway to its base of 100.
        let q = t.cell(16, 0, 2).expect("a cell of the second leaf");
        let raw = [1000, 1002, 1040, 1042];
        for i in 0..4 {
            let flat = F::from(100) + F(0.5) * F::from(raw[i] - 100);
            assert_eq!(q.height[i], F(0.5) * flat, "corner {i}");
        }
        assert_eq!(q.surface[0], 0x10);
        assert_eq!(q.normal[0], [F::ZERO, F(100.0), F(-2.0) * F(0.5), F(1.0)]);
        // A cell the root's table leaves empty, and one past the leaf's edge.
        assert_eq!(t.cell(40, 0, 1), None);
        assert_eq!(t.cell(15, 0, 2), None);
    }

    /// The pages have to account for the gap to the byte: a table that disagrees with where a page
    /// starts, or a stray byte before the tree, is refused rather than read past.
    #[test]
    fn a_terrain_whose_pages_do_not_reach_its_tree_is_refused() {
        let (bytes, pages, tree_at) = terrain_with_pages();
        let table_at = pages[0] - 2 * BLOCK_BYTES - 8;
        let mut moved = bytes.clone();
        moved[table_at + 4..table_at + 8].copy_from_slice(&(pages[1] as u32 + 4).to_le_bytes());
        let why = Terrain::parse(&moved).unwrap_err();
        assert!(why.contains("terrain texture 1 starts at"), "{why}");

        // One byte more in front of the tree, with the header's tree offset moved to match.
        let mut stray = bytes[..tree_at].to_vec();
        stray.push(0);
        stray.extend_from_slice(&bytes[tree_at..]);
        let tree_at_slot = 4 + 1 + 4 + 32 + 4 + 1024 + 4 + 4;
        stray[tree_at_slot..tree_at_slot + 4].copy_from_slice(&(tree_at as u32 + 1).to_le_bytes());
        let why = Terrain::parse(&stray).unwrap_err();
        assert!(why.contains("but the tree starts at"), "{why}");
    }
}
