//! PLN v7 terrains (`.ps2.trn`) and the scenes that name them (`.scn`), written for MX Unleashed's loader
//! and read back with openmx's `game_sim::track::Terrain`.
//!
//! A terrain is held here as [`Trn`]: the file decoded, every node with its own block's contents rather
//! than an index, so two terrains compare with `==`. Three ways in and one way out:
//!
//! - [`build`] cuts a height grid into 17×17-sample leaves under a 16×16-ary tree (the generated maps).
//! - [`read`] decodes a file through openmx's reader (the disc round trip, and copying a disc map).
//! - [`write`] lays a [`Trn`] out as the game reads it, sharing identical blocks.
//!
//! The file, in the order the game's loader (`0x001d9930`) reads it (`notes/mxu-terrain-limits.md`):
//!
//! ```text
//!  "PLN\0" '7' "ps2\0" name[32]  palette flag (u32) [palette 1024]
//!  scale f32, levels i32, tree offset i32, blocks A i32, blocks B i32, A × block offset u32
//!  pages C i32, pages D i32, C × page offset u32
//!  B × block (2129 bytes)
//!  D × page: format u32, mips u32, per mip a 4-word header and its pixels
//!  the tree, preorder: a node (97 bytes), its 256-word child table if it has one, then its children
//! ```

use std::borrow::Cow;
use std::collections::HashMap;

use game_sim::track::{BLOCK_BYTES, BLOCK_SAMPLES, TREE_ARITY, Terrain, TerrainNode, Unread};
use ps2_float::F;

pub mod scene;

/// Samples in one block: 17 × 17.
pub const SAMPLES: usize = (BLOCK_SAMPLES * BLOCK_SAMPLES) as usize;
/// A node as the file stores it, without its child table.
pub const NODE_BYTES: usize = 97;
/// The name every terrain on all three discs carries (it names the palette's kind).
pub const DISC_NAME: &str = "UnCompressedPSXPal8";
/// The node flag byte (node +0x27 in the game) on every node of every disc terrain.
pub const DISC_FLAGS: u8 = 0x27;
/// The page format word on every disc page: 8-bit indexed.
pub const PAGE_PSMT8: u32 = 0x27;
/// The loader keeps pages in a fixed array of 128 (terrain +0x560..+0x75f).
pub const MAX_PAGES: usize = 128;
/// Mip slots run into the palette pointer after 6 (`0x001880a0`).
pub const MAX_MIPS: usize = 6;
/// The descent refuses a cell index over 15 at any level, and a corner is a u16 in samples.
pub const MAX_LEVELS: u32 = 2;
/// A table word that names no page: the loader turns `word & 0xfffe == 0xfffe` into -1.
const NO_PAGE: u32 = 0xfffe;

const HEIGHT_STRIDE: usize = 20;
const OFF_HEIGHTS: usize = 4;
const OFF_NORMALS: usize = OFF_HEIGHTS + 2 * HEIGHT_STRIDE * BLOCK_SAMPLES as usize;
const OFF_UNREAD: usize = OFF_NORMALS + 3 * SAMPLES;
const OFF_SURFACES: usize = OFF_UNREAD + SAMPLES;

// ---------------------------------------------------------------------------------------------------
// the model
// ---------------------------------------------------------------------------------------------------

/// One 17×17 block: what a node samples. Rows are `x + 17 z`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Block {
    /// The height a flattened node pulls the samples towards, in raw units.
    pub base: i32,
    pub heights: [u16; SAMPLES],
    pub normals: [[i8; 3]; SAMPLES],
    /// The byte per sample the PLN loader skips (`0x001d0fb0`). Kept as the source has it.
    pub unread: [u8; SAMPLES],
    /// The surface id in the low seven bits; the top bit is kept as the source has it.
    pub surfaces: [u8; SAMPLES],
}

/// One tree node, leaf or not.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// 1.0, or less for a node that reuses a block flattened towards its base.
    pub vertical_scale: F,
    /// Raw height units to local ones.
    pub height_scale: F,
    /// The node's bounds in local height units (the game culls with them).
    pub min_y: F,
    pub max_y: F,
    /// The half-word in front of the block index. Only the LOD format reads it (`0x001d52c4`); a PLN
    /// terrain's value is kept, and a built one writes 0.
    pub lod: u16,
    pub block: Block,
    /// The texture page under each 4×4 quarter-of-a-quarter of the node, `i = qx + 4 qz` (assumed: see the
    /// journal), or `None`.
    pub pages: [Option<u16>; 16],
    /// The byte at node +0x27. 0x27 on every disc node.
    pub flags: u8,
    /// Empty at a leaf, else 256 slots, `cx + 16 cz`.
    pub children: Vec<Option<Node>>,
}

/// One texture page record, from its format word on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub format: u32,
    pub mips: Vec<Mip>,
}

/// One mip: its 16-byte header (width, height, byte count, and a word the disc fills with leftovers or 0)
/// and its pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mip {
    pub header: [u32; 4],
    pub pixels: Vec<u8>,
}

/// A whole terrain.
#[derive(Clone, Debug, PartialEq)]
pub struct Trn {
    pub name: String,
    pub palette: Option<Box<[u8; 1024]>>,
    /// Feet a sample, and the multiplier from a local height to a world one.
    pub scale: F,
    /// How many levels below the root the leaves are: 1 for a 256-sample map, 2 above that.
    pub levels: u32,
    pub pages: Vec<Page>,
    pub root: Node,
}

impl Page {
    /// A one-mip 256×256 8-bit page, as every disc page is.
    pub fn psmt8(pixels: Vec<u8>) -> Result<Page, String> {
        if pixels.len() != 256 * 256 {
            return Err(format!("a page is 65536 pixels, not {}", pixels.len()));
        }
        Ok(Page {
            format: PAGE_PSMT8,
            mips: vec![Mip {
                header: [256, 256, 65536, 0],
                pixels,
            }],
        })
    }

    fn len(&self) -> usize {
        8 + self.mips.iter().map(|m| 16 + m.pixels.len()).sum::<usize>()
    }
}

impl Block {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; BLOCK_BYTES];
        out[0..4].copy_from_slice(&self.base.to_le_bytes());
        for z in 0..BLOCK_SAMPLES as usize {
            for x in 0..BLOCK_SAMPLES as usize {
                let i = x + z * BLOCK_SAMPLES as usize;
                let at = OFF_HEIGHTS + 2 * (x + z * HEIGHT_STRIDE);
                out[at..at + 2].copy_from_slice(&self.heights[i].to_le_bytes());
                for k in 0..3 {
                    out[OFF_NORMALS + 3 * i + k] = self.normals[i][k] as u8;
                }
                out[OFF_UNREAD + i] = self.unread[i];
                out[OFF_SURFACES + i] = self.surfaces[i];
            }
        }
        out
    }

    /// The block's bytes, as [`Block::to_bytes`] writes them. The three unused height columns of each
    /// row are dropped (zero on every disc block).
    pub fn from_bytes(b: &[u8]) -> Result<Block, String> {
        if b.len() < BLOCK_BYTES {
            return Err(format!("a block is {BLOCK_BYTES} bytes, not {}", b.len()));
        }
        let mut blk = Block {
            base: i32::from_le_bytes(b[0..4].try_into().expect("4 bytes")),
            heights: [0; SAMPLES],
            normals: [[0; 3]; SAMPLES],
            unread: [0; SAMPLES],
            surfaces: [0; SAMPLES],
        };
        for z in 0..BLOCK_SAMPLES as usize {
            for x in 0..BLOCK_SAMPLES as usize {
                let i = x + z * BLOCK_SAMPLES as usize;
                let at = OFF_HEIGHTS + 2 * (x + z * HEIGHT_STRIDE);
                blk.heights[i] = u16::from_le_bytes([b[at], b[at + 1]]);
                let n = &b[OFF_NORMALS + 3 * i..OFF_NORMALS + 3 * i + 3];
                blk.normals[i] = [n[0] as i8, n[1] as i8, n[2] as i8];
                blk.unread[i] = b[OFF_UNREAD + i];
                blk.surfaces[i] = b[OFF_SURFACES + i];
            }
        }
        Ok(blk)
    }
}

impl Node {
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// The node as openmx's reader sees it, for its height arithmetic.
    fn as_game(&self) -> TerrainNode {
        TerrainNode {
            vertical_scale: self.vertical_scale,
            height_scale: self.height_scale,
            min_y: self.min_y,
            max_y: self.max_y,
            block: 0,
            children_at: 0,
            unread: Unread {
                before_block: 0,
                after_block: 0,
                table: [0; 16],
                after_table: 0,
                flags: 0,
            },
        }
    }

    /// The lowest and highest local height of this node's own block, computed as openmx's
    /// `TerrainNode::height` computes a sample (the game's query).
    pub fn block_bounds(&self) -> (F, F) {
        let g = self.as_game();
        let mut lo = g.height(self.block.heights[0], self.block.base);
        let mut hi = lo;
        for &raw in &self.block.heights[1..] {
            let h = g.height(raw, self.block.base);
            lo = lo.min(h);
            hi = hi.max(h);
        }
        (lo, hi)
    }

    fn count(&self) -> (usize, usize) {
        let (mut nodes, mut leaves) = (1, usize::from(self.is_leaf()));
        for c in self.children.iter().flatten() {
            let (n, l) = c.count();
            nodes += n;
            leaves += l;
        }
        (nodes, leaves)
    }

    /// Bytes this node and everything under it take in the file.
    fn span(&self) -> usize {
        if self.is_leaf() {
            NODE_BYTES
        } else {
            NODE_BYTES
                + 4 * 256
                + self
                    .children
                    .iter()
                    .flatten()
                    .map(Node::span)
                    .sum::<usize>()
        }
    }
}

impl Trn {
    /// Nodes and leaves in the tree.
    pub fn count(&self) -> (usize, usize) {
        self.root.count()
    }

    /// Root cells a side that hold anything: what the scene's TerrainWidth and TerrainBreadth say.
    /// A root cell is 256 samples at either depth.
    pub fn root_cells(&self) -> (u32, u32) {
        let (mut w, mut d) = (0, 0);
        for (i, c) in self.root.children.iter().enumerate() {
            if c.is_some() {
                w = w.max(i as u32 % TREE_ARITY + 1);
                d = d.max(i as u32 / TREE_ARITY + 1);
            }
        }
        if self.levels == 1 { (1, 1) } else { (w, d) }
    }
}

// ---------------------------------------------------------------------------------------------------
// writing
// ---------------------------------------------------------------------------------------------------

/// What [`write`] did, for the log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Written {
    pub nodes: usize,
    pub leaves: usize,
    pub blocks: usize,
    pub pages: usize,
    pub tree_at: usize,
}

/// Lay a terrain out as MX Unleashed's loader reads it. Identical blocks are written once and shared,
/// in the order the tree's preorder walk first meets them. The header's tree offset is the root's real
/// position: the game reads the file in sequence and ignores the offset, openmx seeks to it, and both
/// have to land on the same node.
pub fn write(t: &Trn) -> Result<(Vec<u8>, Written), String> {
    check(t)?;

    // The blocks, shared.
    let mut index: HashMap<&Block, u16> = HashMap::new();
    let mut blocks: Vec<&Block> = Vec::new();
    let mut stack = vec![&t.root];
    while let Some(n) = stack.pop() {
        if !index.contains_key(&n.block) {
            if blocks.len() > usize::from(u16::MAX) {
                return Err(
                    "more than 65536 distinct blocks (the node's block index is a u16)".into(),
                );
            }
            index.insert(&n.block, blocks.len() as u16);
            blocks.push(&n.block);
        }
        // Reversed, so the pops come out in the file's preorder.
        stack.extend(n.children.iter().rev().flatten());
    }

    let mut head = Vec::new();
    head.extend_from_slice(b"PLN\0");
    head.push(b'7');
    head.extend_from_slice(b"ps2\0");
    let mut name = [0u8; 32];
    name[..t.name.len()].copy_from_slice(t.name.as_bytes());
    head.extend_from_slice(&name);
    match &t.palette {
        Some(p) => {
            head.extend_from_slice(&1u32.to_le_bytes());
            head.extend_from_slice(&p[..]);
        }
        None => head.extend_from_slice(&0u32.to_le_bytes()),
    }
    let blocks_at = head.len() + 4 * 5 + 4 * blocks.len() + 8 + 4 * t.pages.len();
    let pages_at = blocks_at + blocks.len() * BLOCK_BYTES;
    let tree_at = pages_at + t.pages.iter().map(Page::len).sum::<usize>();
    let end = tree_at + t.root.span();
    if end > i32::MAX as usize {
        return Err(format!(
            "{end} bytes is past what the header's offsets can say"
        ));
    }

    let mut out = head;
    out.reserve(end - out.len());
    out.extend_from_slice(&t.scale.to_bits().to_le_bytes());
    for word in [t.levels as usize, tree_at, blocks.len(), blocks.len()] {
        out.extend_from_slice(&(word as u32).to_le_bytes());
    }
    for i in 0..blocks.len() {
        out.extend_from_slice(&((blocks_at + i * BLOCK_BYTES) as u32).to_le_bytes());
    }
    out.extend_from_slice(&(t.pages.len() as u32).to_le_bytes());
    out.extend_from_slice(&(t.pages.len() as u32).to_le_bytes());
    let mut at = pages_at;
    for p in &t.pages {
        out.extend_from_slice(&(at as u32).to_le_bytes());
        at += p.len();
    }
    debug_assert_eq!(out.len(), blocks_at);
    for b in &blocks {
        out.extend_from_slice(&b.to_bytes());
    }
    for p in &t.pages {
        out.extend_from_slice(&p.format.to_le_bytes());
        out.extend_from_slice(&(p.mips.len() as u32).to_le_bytes());
        for m in &p.mips {
            for w in m.header {
                out.extend_from_slice(&w.to_le_bytes());
            }
            out.extend_from_slice(&m.pixels);
        }
    }
    debug_assert_eq!(out.len(), tree_at);
    write_node(&mut out, &t.root, &index);
    debug_assert_eq!(out.len(), end);

    let (nodes, leaves) = t.count();
    Ok((
        out,
        Written {
            nodes,
            leaves,
            blocks: blocks.len(),
            pages: t.pages.len(),
            tree_at,
        },
    ))
}

fn write_node(out: &mut Vec<u8>, n: &Node, index: &HashMap<&Block, u16>) {
    let at = out.len();
    for f in [n.vertical_scale, n.height_scale, n.min_y, n.max_y] {
        out.extend_from_slice(&f.to_bits().to_le_bytes());
    }
    out.extend_from_slice(&n.lod.to_le_bytes());
    out.extend_from_slice(&index[&n.block].to_le_bytes());
    // The word after the block index: the loader masks it the way it masks the page table and then
    // drops it. 0xfffe ("none") on every disc node.
    out.extend_from_slice(&NO_PAGE.to_le_bytes());
    for p in n.pages {
        let word = p.map_or(NO_PAGE, |i| u32::from(i) * 2);
        out.extend_from_slice(&word.to_le_bytes());
    }
    out.extend_from_slice(&0u32.to_le_bytes()); // read and dropped; 0 on every disc node
    out.push(n.flags);
    if n.is_leaf() {
        out.extend_from_slice(&0u32.to_le_bytes());
        return;
    }
    let table_at = at + NODE_BYTES;
    out.extend_from_slice(&(table_at as u32).to_le_bytes());
    let mut child_at = table_at + 4 * 256;
    for c in &n.children {
        let word = match c {
            Some(c) => {
                let w = child_at;
                child_at += c.span();
                w
            }
            None => 0,
        };
        out.extend_from_slice(&(word as u32).to_le_bytes());
    }
    for c in n.children.iter().flatten() {
        write_node(out, c, index);
    }
}

/// What the game's loader and tree code can take (`notes/mxu-terrain-limits.md`, *What a writer has to
/// respect*), and what the layout needs to be readable at all.
pub fn check(t: &Trn) -> Result<(), String> {
    if t.name.len() >= 32 || !t.name.is_ascii() {
        return Err(format!(
            "the name {:?} must be ASCII and under 32 bytes",
            t.name
        ));
    }
    if !(1..=MAX_LEVELS).contains(&t.levels) {
        return Err(format!("{} levels: the game's maps have 1 or 2", t.levels));
    }
    if t.pages.len() > MAX_PAGES {
        return Err(format!(
            "{} pages: the loader keeps at most {MAX_PAGES}",
            t.pages.len()
        ));
    }
    for (i, p) in t.pages.iter().enumerate() {
        if p.mips.is_empty() || p.mips.len() > MAX_MIPS {
            return Err(format!(
                "page {i} has {} mips: 1 to {MAX_MIPS}",
                p.mips.len()
            ));
        }
        for m in &p.mips {
            if m.header[2] as usize != m.pixels.len() {
                return Err(format!(
                    "page {i}: a mip says {} bytes and has {}",
                    m.header[2],
                    m.pixels.len()
                ));
            }
        }
    }
    check_node(t, &t.root, 0)
}

fn check_node(t: &Trn, n: &Node, level: u32) -> Result<(), String> {
    for p in n.pages.iter().flatten() {
        if usize::from(*p) >= t.pages.len() {
            return Err(format!("a node names page {p} of {}", t.pages.len()));
        }
    }
    if level == t.levels {
        if !n.is_leaf() {
            return Err(format!(
                "a node at level {level} of {} has children",
                t.levels
            ));
        }
        return Ok(());
    }
    if n.children.len() != 256 {
        return Err(format!(
            "a node at level {level} of {} has {} child slots, not 256",
            t.levels,
            n.children.len()
        ));
    }
    for c in n.children.iter().flatten() {
        check_node(t, c, level + 1)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------
// reading back
// ---------------------------------------------------------------------------------------------------

/// The parts of the header openmx's `Terrain` does not hand out: where the blocks and pages are.
#[derive(Clone, Debug)]
pub struct Header {
    pub palette: Option<Box<[u8; 1024]>>,
    pub declared_at: usize,
    pub declared: usize,
    pub blocks_at: usize,
    pub blocks: usize,
    pub page_words: usize,
    pub pages: Vec<Page>,
    pub pages_at: usize,
    pub pages_end: usize,
    pub tree_at: usize,
}

fn word(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4)
        .map(|w| u32::from_le_bytes(w.try_into().expect("4 bytes")))
        .ok_or_else(|| format!("the file ends before byte {}", at + 4))
}

/// Read the header and walk the page records between the blocks and the tree, the way the game's
/// loader does (format word, mip count, per mip a 16-byte header and its pixels).
pub fn header(b: &[u8]) -> Result<Header, String> {
    if b.get(0..4) != Some(b"PLN\0") {
        return Err("not a PLN terrain".into());
    }
    let mut at = 41;
    let palette = if word(b, at)? != 0 {
        let p: [u8; 1024] = b
            .get(at + 4..at + 4 + 1024)
            .ok_or("the palette runs off the file")?
            .try_into()
            .expect("1024");
        at += 4 + 1024;
        Some(Box::new(p))
    } else {
        at += 4;
        None
    };
    let tree_at = word(b, at + 8)? as usize;
    let declared_at = at + 12;
    let declared = word(b, at + 12)? as usize;
    let blocks = word(b, at + 16)? as usize;
    at += 20 + 4 * declared;
    let page_words = word(b, at)? as usize;
    let page_count = word(b, at + 4)? as usize;
    at += 8 + 4 * page_words;
    let blocks_at = at;
    let pages_at = blocks_at + blocks * BLOCK_BYTES;
    let mut pages = Vec::with_capacity(page_count.min(MAX_PAGES));
    let mut at = pages_at;
    for i in 0..page_count {
        let format = word(b, at)?;
        let mips = word(b, at + 4)? as usize;
        if mips > 16 {
            return Err(format!("page {i} claims {mips} mips"));
        }
        at += 8;
        let mut page = Page {
            format,
            mips: Vec::with_capacity(mips),
        };
        for _ in 0..mips {
            let h = [
                word(b, at)?,
                word(b, at + 4)?,
                word(b, at + 8)?,
                word(b, at + 12)?,
            ];
            at += 16;
            let px = b
                .get(at..at + h[2] as usize)
                .ok_or_else(|| format!("page {i} runs off the file"))?;
            at += px.len();
            page.mips.push(Mip {
                header: h,
                pixels: px.to_vec(),
            });
        }
        pages.push(page);
    }
    if at != tree_at {
        return Err(format!(
            "{page_count} pages from {pages_at} end at {at}, but the header puts the tree at {tree_at} \
             (the game reads the root from where the pages end)"
        ));
    }
    Ok(Header {
        palette,
        declared_at,
        declared,
        blocks_at,
        blocks,
        page_words,
        pages,
        pages_at,
        pages_end: at,
        tree_at,
    })
}

/// The file as openmx's `Terrain::parse` will take it today.
///
/// `Terrain::parse` wants the blocks to end exactly at the tree, so it refuses every terrain with texture
/// pages (MD1 in `notes/openmx-asks.md`). Until it reads them, this hands it a copy in which the page
/// offsets and the pages have become extra (skipped) block-offset words in front of the blocks: the
/// blocks move down by the pages' length and so end at the tree again, and the tree and every offset in
/// it stay where they were. A file without pages comes back unchanged. Remove this once MD1 lands.
pub fn openmx_view(b: &[u8]) -> Result<Cow<'_, [u8]>, String> {
    let h = header(b)?;
    if h.page_words == 0 && h.pages.is_empty() {
        return Ok(Cow::Borrowed(b));
    }
    let gap = 4 * h.page_words + (h.pages_end - h.pages_at);
    let words_end = h.declared_at + 8 + 4 * h.declared;
    let mut v = Vec::with_capacity(b.len());
    v.extend_from_slice(&b[..words_end]);
    v.extend(std::iter::repeat_n(0u8, gap));
    v.extend_from_slice(&[0u8; 8]); // no page offsets, no pages
    v.extend_from_slice(&b[h.blocks_at..h.pages_at]);
    v.extend_from_slice(&b[h.tree_at..]);
    let declared = (h.declared + gap / 4) as u32;
    v[h.declared_at..h.declared_at + 4].copy_from_slice(&declared.to_le_bytes());
    debug_assert_eq!(v.len(), b.len() - 8 + 8);
    Ok(Cow::Owned(v))
}

/// Decode a terrain: the tree is walked with openmx's `Terrain` (through [`openmx_view`]); the palette,
/// the pages and the block bytes openmx does not hand out are taken from the file at the places
/// [`header`] finds.
pub fn read(b: &[u8]) -> Result<Trn, String> {
    let h = header(b)?;
    let view = openmx_view(b)?;
    let t = Terrain::parse(&view)?;
    let block = |i: u16| -> Result<Block, String> {
        if usize::from(i) >= h.blocks {
            return Err(format!("a node names block {i} of {}", h.blocks));
        }
        let at = h.blocks_at + usize::from(i) * BLOCK_BYTES;
        Block::from_bytes(&b[at..at + BLOCK_BYTES])
    };
    let root = t.root().ok_or("the root node does not fit the file")?;
    let root = read_node(&t, &root, 0, &block)?;
    Ok(Trn {
        name: t.name.clone(),
        palette: h.palette,
        scale: t.scale,
        levels: t.levels,
        pages: h.pages,
        root,
    })
}

fn read_node(
    t: &Terrain,
    n: &TerrainNode,
    level: u32,
    block: &dyn Fn(u16) -> Result<Block, String>,
) -> Result<Node, String> {
    let mut pages = [None; 16];
    for (p, w) in pages.iter_mut().zip(n.unread.table) {
        if w & 0xfffe != 0xfffe {
            *p = Some(((w & 0xfffe) >> 1) as u16);
        }
    }
    let mut children = Vec::new();
    if level < t.levels {
        children.reserve(256);
        for cz in 0..TREE_ARITY {
            for cx in 0..TREE_ARITY {
                children.push(match t.child(n, cx, cz) {
                    Some(at) => {
                        let c = t
                            .node(at)
                            .ok_or_else(|| format!("a child node at {at} is off the file"))?;
                        Some(read_node(t, &c, level + 1, block)?)
                    }
                    None => None,
                });
            }
        }
    } else if n.children_at != 0 {
        return Err(format!("a leaf at level {level} has a child table"));
    }
    Ok(Node {
        vertical_scale: n.vertical_scale,
        height_scale: n.height_scale,
        min_y: n.min_y,
        max_y: n.max_y,
        lod: n.unread.before_block,
        block: block(n.block)?,
        pages,
        flags: n.unread.flags,
        children,
    })
}

// ---------------------------------------------------------------------------------------------------
// building from a grid
// ---------------------------------------------------------------------------------------------------

/// A height grid: `width × depth` samples, rows along x, then z. Sample (0, 0) is the tree's corner.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub width: u32,
    pub depth: u32,
    /// Raw heights: world y = scale × height_scale × raw.
    pub heights: Vec<u16>,
    /// Surface byte per sample (the id in the low seven bits). Empty means all 0.
    pub surfaces: Vec<u8>,
    /// Normals per sample as the file stores them. Empty means computed ([`normal`]).
    pub normals: Vec<[i8; 3]>,
}

/// What goes with a grid into a terrain.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub name: String,
    /// Feet a sample.
    pub scale: f32,
    /// Raw units to local ones: world y = scale × height_scale × raw.
    pub height_scale: f32,
    /// The 256-colour palette (RGBA, alpha 0x80 = opaque on the GS). None writes a grey ramp: every disc
    /// terrain has one, so the file always carries one.
    pub palette: Option<Box<[u8; 1024]>>,
    pub pages: Vec<Page>,
    /// Which page lies under each 64×64-sample tile, rows along x then z, `tiles_x` wide. A tile is a
    /// quarter of a quarter of a root cell, as on the disc (16 pages a root cell, on the node over it).
    pub page_map: Vec<Option<u16>>,
    pub tiles_x: u32,
}

impl Options {
    pub fn new(scale: f32, height_scale: f32) -> Options {
        Options {
            name: DISC_NAME.to_owned(),
            scale,
            height_scale,
            palette: None,
            pages: Vec::new(),
            page_map: Vec::new(),
            tiles_x: 0,
        }
    }
}

impl Grid {
    /// A grid of world heights in feet, quantised to u16 with the height scale that puts the highest
    /// sample at 65535. Heights must not be negative (the raw units are unsigned and nothing offsets
    /// them). Returns the grid and the height scale.
    pub fn from_feet(
        width: u32,
        depth: u32,
        feet: &[f32],
        scale: f32,
    ) -> Result<(Grid, f32), String> {
        if feet.len() != (width * depth) as usize {
            return Err(format!("{} heights for a {width}×{depth} grid", feet.len()));
        }
        let top = feet.iter().copied().fold(0.0f32, f32::max);
        if let Some(low) = feet.iter().find(|h| !(**h >= 0.0)) {
            return Err(format!(
                "height {low}: the terrain's raw heights cannot go below 0"
            ));
        }
        let height_scale = if top > 0.0 {
            top / (scale * 65535.0)
        } else {
            1.0 / 65535.0
        };
        let unit = f64::from(scale) * f64::from(height_scale);
        let heights = feet
            .iter()
            .map(|h| (f64::from(*h) / unit).round().clamp(0.0, 65535.0) as u16)
            .collect();
        Ok((
            Grid {
                width,
                depth,
                heights,
                surfaces: Vec::new(),
                normals: Vec::new(),
            },
            height_scale,
        ))
    }

    fn at(&self, x: i64, z: i64) -> usize {
        let x = x.clamp(0, i64::from(self.width) - 1) as usize;
        let z = z.clamp(0, i64::from(self.depth) - 1) as usize;
        x + z * self.width as usize
    }

    fn height(&self, x: i64, z: i64) -> u16 {
        self.heights[self.at(x, z)]
    }
}

/// A sample's normal from its neighbours' heights: central differences, scaled to 127 and truncated.
///
/// This is not the original tool's formula, which is not known. On free06's full-relief leaves it gives
/// the disc's three bytes exactly at about two samples in three, and is never more than a few units off
/// elsewhere in open ground (journal). Grid edges use the edge sample again (clamped).
pub fn normal(g: &Grid, x: u32, z: u32, height_scale: f32) -> [i8; 3] {
    let (x, z) = (i64::from(x), i64::from(z));
    let hs = f64::from(height_scale);
    // world y = scale × hs × raw over 2 × scale feet: the scale cancels.
    let gx = (f64::from(g.height(x + 1, z)) - f64::from(g.height(x - 1, z))) * hs / 2.0;
    let gz = (f64::from(g.height(x, z + 1)) - f64::from(g.height(x, z - 1))) * hs / 2.0;
    let len = (gx * gx + 1.0 + gz * gz).sqrt();
    [
        (-gx / len * 127.0) as i8,
        (1.0 / len * 127.0) as i8,
        (-gz / len * 127.0) as i8,
    ]
}

/// Cut a grid into a terrain: 17×17-sample leaves 16 samples apart (each shares its last row and column
/// with the next), one level under the root when the grid is at most 256 samples a side (+1), two
/// otherwise (at most 16 root cells, 4096 samples). Samples past the grid's last row or column repeat
/// it. Every node above the leaves samples the grid every 16 (or 256) samples, as the disc's do.
pub fn build(g: &Grid, o: &Options) -> Result<Trn, String> {
    let n = (g.width * g.depth) as usize;
    if g.width < 2 || g.depth < 2 || g.heights.len() != n {
        return Err(format!(
            "{} heights for a {}×{} grid",
            g.heights.len(),
            g.width,
            g.depth
        ));
    }
    if !g.surfaces.is_empty() && g.surfaces.len() != n {
        return Err(format!(
            "{} surface bytes for {n} samples",
            g.surfaces.len()
        ));
    }
    if !g.normals.is_empty() && g.normals.len() != n {
        return Err(format!("{} normals for {n} samples", g.normals.len()));
    }
    if !(o.scale > 0.0) || !(o.height_scale > 0.0) {
        return Err("the scale and the height scale must be positive".into());
    }
    let leaves_x = (g.width - 1).div_ceil(16);
    let leaves_z = (g.depth - 1).div_ceil(16);
    let levels = if leaves_x <= 16 && leaves_z <= 16 {
        1
    } else {
        2
    };
    if leaves_x > 256 || leaves_z > 256 {
        return Err(format!(
            "{}×{} samples: at most 16 root cells (4097 samples) a side",
            g.width, g.depth
        ));
    }
    if !o.page_map.is_empty() && (o.tiles_x == 0 || o.page_map.len() % o.tiles_x as usize != 0) {
        return Err(format!(
            "a page map of {} tiles is not {} wide",
            o.page_map.len(),
            o.tiles_x
        ));
    }
    let palette = o.palette.clone().unwrap_or_else(|| {
        let mut p = Box::new([0u8; 1024]);
        for i in 0..256 {
            p[4 * i..4 * i + 4].copy_from_slice(&[i as u8, i as u8, i as u8, 0x80]);
        }
        p
    });
    let b = Builder {
        g,
        o,
        hs: F(o.height_scale),
        leaves: (leaves_x, leaves_z),
        levels,
    };
    let root = b
        .node(0, 0, 0)
        .expect("the root cell holds the grid's corner");
    let t = Trn {
        name: o.name.clone(),
        palette: Some(palette),
        scale: F(o.scale),
        levels,
        pages: o.pages.clone(),
        root,
    };
    check(&t)?;
    Ok(t)
}

struct Builder<'a> {
    g: &'a Grid,
    o: &'a Options,
    hs: F,
    leaves: (u32, u32),
    levels: u32,
}

impl Builder<'_> {
    /// The node at `level` whose corner is leaf (`lx`, `lz`), or None past the grid.
    fn node(&self, level: u32, lx: u32, lz: u32) -> Option<Node> {
        if lx >= self.leaves.0 || lz >= self.leaves.1 {
            return None;
        }
        // Samples between the block's samples, which is also the leaves a side under the node.
        let step = 16u32.pow(self.levels - level);
        let mut block = self.block(lx * 16, lz * 16, step);
        block.base = if level == self.levels {
            // Not used by the query at full relief; the lowest sample, so a later flattening (which
            // pulls towards it) stays inside the block's own range.
            i32::from(*block.heights.iter().min().expect("289 samples"))
        } else {
            0 // as on every disc node above the leaves
        };
        let mut node = Node {
            vertical_scale: F(1.0),
            height_scale: self.hs,
            min_y: F::ZERO,
            max_y: F::ZERO,
            lod: 0,
            block,
            pages: [None; 16],
            flags: DISC_FLAGS,
            children: Vec::new(),
        };
        if level < self.levels {
            let child = step / 16;
            node.children = (0..256)
                .map(|i| self.node(level + 1, lx + (i % 16) * child, lz + (i / 16) * child))
                .collect();
            let mut kids = node.children.iter().flatten();
            let first = kids
                .next()
                .expect("a node with a corner on the grid has a child");
            let (mut lo, mut hi) = (first.min_y, first.max_y);
            for c in kids {
                lo = lo.min(c.min_y);
                hi = hi.max(c.max_y);
            }
            node.min_y = lo;
            node.max_y = hi;
            // The node over one root cell (256 samples) carries that cell's 16 pages.
            if level == self.levels - 1 {
                node.pages = self.pages(lx * 16, lz * 16);
            }
        } else {
            let (lo, hi) = node.block_bounds();
            node.min_y = lo;
            node.max_y = hi;
        }
        Some(node)
    }

    fn block(&self, x0: u32, z0: u32, step: u32) -> Block {
        let mut b = Block {
            base: 0,
            heights: [0; SAMPLES],
            normals: [[0; 3]; SAMPLES],
            unread: [0; SAMPLES],
            surfaces: [0; SAMPLES],
        };
        for z in 0..BLOCK_SAMPLES {
            for x in 0..BLOCK_SAMPLES {
                let i = (x + z * BLOCK_SAMPLES) as usize;
                let (sx, sz) = (x0 + x * step, z0 + z * step);
                let at = self.g.at(i64::from(sx), i64::from(sz));
                b.heights[i] = self.g.heights[at];
                b.surfaces[i] = self.g.surfaces.get(at).copied().unwrap_or(0);
                b.normals[i] = if self.g.normals.is_empty() {
                    let (cx, cz) = (sx.min(self.g.width - 1), sz.min(self.g.depth - 1));
                    normal(self.g, cx, cz, self.o.height_scale)
                } else {
                    self.g.normals[at]
                };
            }
        }
        b
    }

    fn pages(&self, x0: u32, z0: u32) -> [Option<u16>; 16] {
        let mut out = [None; 16];
        if self.o.page_map.is_empty() {
            return out;
        }
        let tiles_z = self.o.page_map.len() as u32 / self.o.tiles_x;
        for (i, p) in out.iter_mut().enumerate() {
            let (tx, tz) = (x0 / 64 + i as u32 % 4, z0 / 64 + i as u32 / 4);
            if tx < self.o.tiles_x && tz < tiles_z {
                *p = self.o.page_map[(tx + tz * self.o.tiles_x) as usize];
            }
        }
        out
    }
}
