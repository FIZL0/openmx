//! MXA's custom tracks (MD259): a Track Studio `.mxatrack` read and written out as a track mod.
//!
//! The file, as MXA's TrackCore writes it (`context/mxa/NOTES.md`; reference only, this reader is ours):
//! a zip holding
//!
//! ```text
//!  manifest.json   {"version": 1, "collisionSha256": "<hex of collision.msc>", "units": "feet"}
//!  track.json      the track document: id, title, author, environment, venueId, groundTexture,
//!                  width, depth (feet), resolution (+ resolutionZ, 0 = square), surface 0..15,
//!                  spawnX/Z/Yaw, the route, features, props ...
//!  collision.msc   the terrain the editor baked, which is what a rider rides:
//!                  "MSC1" u32 version (1..3), f32 bounds min[3] max[3], spawn[3], spawn direction[3],
//!                  u32 resolution, rows, surface, f32 heights[resolution × rows] (x fastest),
//!                  v2: a surface byte a sample; v3: u32 count (0 or one a sample), the bytes;
//!                  bytes padded to 4 with zeros; u32 chunks, each u32 vertices, u32 triangles,
//!                  vertices × f32[4], triangles × u32[3] (v1/v2: the ground's 64-cell chunks
//!                  first, then the solid props; v3: the props only)
//!  preview.png, heightmap.f32, sculpt.f32, mask0..3.f32, colormap.rgba   the editor's own, unread here
//! ```
//!
//! The document's x and z run from −width/2 to width/2 and −depth/2 to depth/2, sample (i, j) at
//! (−width/2 + i·width/(resolution−1), −depth/2 + j·depth/(rows−1)). MXA puts a `.scn`'s start
//! position into the same space unchanged (its corner at 0, no axis turned), so a track mod's terrain
//! is the same grid moved by half the size, its lowest point lifted to 0 when it is below.
//!
//! [`open`] refuses what MXA's own loader refuses (the manifest's version and units, the hash, the
//! collision's header, sizes and ranges). [`write_mod`] lays out the folder: `<id>.ps2.trn`,
//! `<id>.scn` and `mod.json`.

use crate::json::Json;
use crate::jsonw::{J, o, s};
use std::path::{Path, PathBuf};

/// The largest package MXA opens: 512 MB.
pub const MAX_FILE: u64 = 512 << 20;
/// "MSC1", little-endian.
const MSC_MAGIC: u32 = 0x3143_534d;
/// The finest MX Unleashed terrain the writer lays out: 16 root cells of 256 samples.
const MAX_SIDE: u32 = 4097;

/// A Track Studio track: what its document says, and the ground it baked.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    /// The document's own id (a GUID's 32 hex digits when the editor made it).
    pub id: String,
    pub title: String,
    pub author: String,
    /// `Stadium` or `Outdoor`.
    pub environment: String,
    /// The stadium the document names (`SX01` .. `SX18`), or empty.
    pub venue: String,
    /// The ground texture the document names, a path in MX vs. ATV's files.
    pub ground_texture: String,
    pub description: String,
    pub location: String,
    /// Feet across x and along z.
    pub width: f32,
    pub depth: f32,
    /// Samples across x and along z.
    pub resolution: u32,
    pub rows: u32,
    /// The riding surface, 0..15, where no per-sample one is given.
    pub surface: u8,
    /// Baked heights in feet, x fastest.
    pub heights: Vec<f32>,
    /// A surface a sample, when the package has them.
    pub surfaces: Option<Vec<u8>>,
    /// Where the rider starts and the way it faces, in the document's space.
    pub spawn: [f32; 3],
    pub direction: [f32; 3],
}

/// Read a `.mxatrack` from its bytes.
pub fn open(bytes: &[u8]) -> Result<Track, String> {
    if bytes.len() as u64 > MAX_FILE {
        return Err("the track package is too big".into());
    }
    let zip = Zip::read(bytes)?;
    for name in ["manifest.json", "track.json", "collision.msc"] {
        if !zip.has(name) {
            return Err(format!("not a Track Studio track: no {name}"));
        }
    }
    let manifest = text(&zip.file("manifest.json", 4096)?, "manifest.json")?;
    let manifest = Json::parse(&manifest).map_err(|e| format!("manifest.json: {e}"))?;
    if manifest.get("version").and_then(Json::f64) != Some(1.0) || manifest.get("units").and_then(Json::str) != Some("feet") {
        return Err("an unsupported track package (manifest version 1 in feet is read)".into());
    }
    let want = manifest.get("collisionSha256").and_then(Json::str).unwrap_or_default().to_ascii_lowercase();
    let doc = text(&zip.file("track.json", 256 << 20)?, "track.json")?;
    let doc = Json::parse(&doc).map_err(|e| format!("track.json: {e}"))?;
    let collision = zip.file("collision.msc", 320 << 20)?;
    if sha256_hex(&collision) != want {
        return Err("the track's collision data is damaged (its hash is not the manifest's)".into());
    }
    let mut t = document(&doc)?;
    ground(&mut t, &collision)?;
    Ok(t)
}

/// The document's fields this reader takes, checked as MXA checks them.
fn document(d: &Json) -> Result<Track, String> {
    let string = |k: &str, default: &str| -> Result<String, String> {
        match d.get(k) {
            None | Some(Json::Null) => Ok(default.to_owned()),
            Some(v) => v.str().map(str::to_owned).ok_or_else(|| format!("track.json: {k} is not text")),
        }
    };
    let number = |k: &str, default: f64| -> Result<f64, String> {
        match d.get(k) {
            None => Ok(default),
            Some(v) => v.f64().filter(|n| n.is_finite()).ok_or_else(|| format!("track.json: {k} is not a number")),
        }
    };
    let version = number("schemaVersion", 8.0)?;
    if !(1.0..=8.0).contains(&version) || version.fract() != 0.0 {
        return Err(format!("track.json: project version {version}, and versions 1 to 8 are read"));
    }
    let id = string("id", "")?;
    let title = string("title", "Untitled track")?;
    let author = string("author", "")?;
    if id.trim().is_empty() || id.len() > 64 || title.chars().count() > 120 || author.chars().count() > 120 {
        return Err("track.json: the track's name or identity is invalid".into());
    }
    let environment = string("environment", "Stadium")?;
    if environment != "Stadium" && environment != "Outdoor" {
        return Err(format!("track.json: environment {environment:?} is neither Stadium nor Outdoor"));
    }
    let (width, depth) = (number("width", 720.0)?, number("depth", 520.0)?);
    if !(32.0..=32768.0).contains(&width) || !(32.0..=32768.0).contains(&depth) {
        return Err(format!("track.json: {width} × {depth} feet, and 32 to 32768 a side is read"));
    }
    let whole = |k: &str, default: f64| -> Result<i64, String> {
        let n = number(k, default)?;
        if n.fract() != 0.0 { Err(format!("track.json: {k} is not a whole number")) } else { Ok(n as i64) }
    };
    let resolution = whole("resolution", 257.0)?;
    let rows = match whole("resolutionZ", 0.0)? {
        0 => resolution,
        r => r,
    };
    let side = |n: i64| (65..=8193).contains(&n);
    if !side(resolution) || !side(rows) || resolution * rows > 17_000_000 {
        return Err(format!("track.json: a {resolution} × {rows} terrain, and 65 to 8193 a side is read"));
    }
    let surface = whole("surface", 0.0)?;
    if !(0..=15).contains(&surface) {
        return Err(format!("track.json: riding surface {surface}, and 0 to 15 is read"));
    }
    Ok(Track {
        id,
        title: title.trim().to_owned(),
        author,
        environment,
        venue: string("venueId", "")?,
        ground_texture: string("groundTexture", "")?,
        description: string("description", "")?,
        location: string("location", "")?,
        width: width as f32,
        depth: depth as f32,
        resolution: resolution as u32,
        rows: rows as u32,
        surface: surface as u8,
        heights: Vec::new(),
        surfaces: None,
        spawn: [0.0; 3],
        direction: [0.0, 0.0, 1.0],
    })
}

/// The baked ground out of `collision.msc`, against the document's size.
fn ground(t: &mut Track, b: &[u8]) -> Result<(), String> {
    let mut r = Reader { b, at: 0 };
    if r.u32()? != MSC_MAGIC {
        return Err("collision.msc: not a collision file".into());
    }
    let version = r.u32()?;
    if !(1..=3).contains(&version) {
        return Err(format!("collision.msc: version {version}, and 1 to 3 is read"));
    }
    let mut v = [0f32; 12];
    for f in &mut v {
        *f = r.f32()?;
        if !f.is_finite() {
            return Err("collision.msc: a bound that is not a number".into());
        }
    }
    let (lo, hi) = ([v[0], v[1], v[2]], [v[3], v[4], v[5]]);
    if lo[0] != -t.width / 2.0 || lo[2] != -t.depth / 2.0 || hi[0] != t.width / 2.0 || hi[2] != t.depth / 2.0 || hi[1] <= lo[1] || hi[1] - lo[1] > 65536.0 {
        return Err("collision.msc: its bounds are not the track's".into());
    }
    if r.u32()? != t.resolution || r.u32()? != t.rows || r.u32()? != u32::from(t.surface) {
        return Err("collision.msc: its grid is not the track's".into());
    }
    let n = (t.resolution * t.rows) as usize;
    let mut heights = Vec::with_capacity(n);
    for _ in 0..n {
        let h = r.f32()?;
        if !h.is_finite() || h < lo[1] || h > hi[1] {
            return Err("collision.msc: a height outside its bounds".into());
        }
        heights.push(h);
    }
    let masked = match version {
        2 => n,
        3 => r.u32()? as usize,
        _ => 0,
    };
    if masked != 0 && masked != n {
        return Err("collision.msc: a surface mask of the wrong size".into());
    }
    if masked != 0 {
        let mask = r.bytes(n)?.to_vec();
        if mask.iter().any(|&m| m > 15) {
            return Err("collision.msc: a surface past 15".into());
        }
        while r.at % 4 != 0 {
            if r.bytes(1)?[0] != 0 {
                return Err("collision.msc: padding that is not zero".into());
            }
        }
        t.surfaces = Some(mask);
    }
    // The solid pieces (and, before v3, the ground again as triangles): only checked, since the
    // heights above are the ground; a prop's shape is not laid out (journal).
    let chunks = r.u32()?;
    for _ in 0..chunks {
        let (verts, tris) = (r.u32()? as usize, r.u32()? as usize);
        if verts < 3 || tris == 0 {
            return Err("collision.msc: an empty collision chunk".into());
        }
        r.bytes(verts.checked_mul(16).ok_or("collision.msc: a chunk too big")?)?;
        for _ in 0..tris * 3 {
            if r.u32()? as usize >= verts {
                return Err("collision.msc: an index past its chunk".into());
            }
        }
    }
    if r.at != b.len() {
        return Err("collision.msc: data after its last chunk".into());
    }
    t.heights = heights;
    t.spawn = [v[6], v[7], v[8]];
    t.direction = [v[9], v[10], v[11]];
    Ok(())
}

/// The track mod's id for a track: `mxa_` and the document id's letters and digits, lowercased.
pub fn mod_id(t: &Track) -> Result<String, String> {
    let tail: String = t.id.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).take(36).collect();
    if tail.is_empty() {
        return Err(format!("track id {:?} has no letters or digits", t.id));
    }
    Ok(format!("mxa_{tail}"))
}

/// The ground laid out for the terrain writer: samples `spacing` feet apart from the document's
/// −width/2, −depth/2 corner, in feet above that corner's lowest point (`lift` added).
#[derive(Clone, Debug, PartialEq)]
pub struct Laid {
    pub width: u32,
    pub depth: u32,
    pub spacing: f32,
    pub feet: Vec<f32>,
    pub surfaces: Vec<u8>,
    pub lift: f32,
}

/// The document's grid as the terrain writer takes it: the samples themselves when they are as far
/// apart across as along and at most [`MAX_SIDE`] a side; otherwise sampled again at the finer of
/// the two spacings (or coarser, to fit), between samples as MXA's own height query interpolates.
/// The far edge is dropped where the size is not a whole number of samples (less than one).
pub fn lay(t: &Track) -> Laid {
    let (n, rows) = (t.resolution, t.rows);
    let dx = t.width / (n - 1) as f32;
    let dz = t.depth / (rows - 1) as f32;
    let low = t.heights.iter().copied().fold(f32::INFINITY, f32::min);
    let lift = if low < 0.0 { -low } else { 0.0 };
    let surface_at = |i: usize| t.surfaces.as_ref().map_or(t.surface, |s| s[i]);
    if dx == dz && n <= MAX_SIDE && rows <= MAX_SIDE {
        return Laid {
            width: n,
            depth: rows,
            spacing: dx,
            feet: t.heights.iter().map(|h| h + lift).collect(),
            surfaces: (0..t.heights.len()).map(surface_at).collect(),
            lift,
        };
    }
    let spacing = dx.min(dz).max(t.width / (MAX_SIDE - 1) as f32).max(t.depth / (MAX_SIDE - 1) as f32);
    let w = ((t.width / spacing) as u32 + 1).min(MAX_SIDE);
    let d = ((t.depth / spacing) as u32 + 1).min(MAX_SIDE);
    let (mut feet, mut surfaces) = (Vec::with_capacity((w * d) as usize), Vec::with_capacity((w * d) as usize));
    for j in 0..d {
        for i in 0..w {
            let fx = (i as f32 * spacing / dx).clamp(0.0, (n - 1) as f32);
            let fz = (j as f32 * spacing / dz).clamp(0.0, (rows - 1) as f32);
            let (x0, z0) = ((fx as u32).min(n - 2), (fz as u32).min(rows - 2));
            let (tx, tz) = (fx - x0 as f32, fz - z0 as f32);
            let at = |x: u32, z: u32| (z * n + x) as usize;
            let h = |x: u32, z: u32| t.heights[at(x, z)];
            let near = h(x0, z0) + (h(x0 + 1, z0) - h(x0, z0)) * tx;
            let far = h(x0, z0 + 1) + (h(x0 + 1, z0 + 1) - h(x0, z0 + 1)) * tx;
            feet.push(near + (far - near) * tz + lift);
            surfaces.push(surface_at(at(fx.round() as u32, fz.round() as u32)));
        }
    }
    Laid { width: w, depth: d, spacing, feet, surfaces, lift }
}

/// Write `t` as the track mod `tracks/<mod_id>/`, replacing what was there; `stamp` is kept in
/// its `mod.json` under `mxatrack` (the importer's note of which file it came from). The folder.
pub fn write_mod(t: &Track, tracks: &Path, stamp: J) -> Result<PathBuf, String> {
    let id = mod_id(t)?;
    let laid = lay(t);
    let (mut grid, hs) = trnwrite::Grid::from_feet(laid.width, laid.depth, &laid.feet, laid.spacing)?;
    grid.surfaces = laid.surfaces.clone();
    let trn = trnwrite::build(&grid, &trnwrite::Options::new(laid.spacing, hs))?;
    let (bytes, _) = trnwrite::write(&trn)?;
    // Into our space: the corner at 0, the ground lifted with it.
    let start = [t.spawn[0] + t.width / 2.0, t.spawn[1] + laid.lift, t.spawn[2] + t.depth / 2.0];
    let flat = (t.direction[0] * t.direction[0] + t.direction[2] * t.direction[2]).sqrt();
    let direction = if flat > 1e-6 { [t.direction[0] / flat, 0.0, t.direction[2] / flat] } else { [0.0, 0.0, 1.0] };
    let scene = trnwrite::scene::SceneText::for_terrain(&trn, &id, start, direction).to_text();

    let dir = tracks.join(&id);
    let staged = tracks.join(format!(".{id}.part"));
    let _ = std::fs::remove_dir_all(&staged);
    std::fs::create_dir_all(&staged).map_err(|e| format!("{}: {e}", staged.display()))?;
    let put = |name: &str, b: &[u8]| std::fs::write(staged.join(name), b).map_err(|e| format!("{}: {e}", staged.join(name).display()));
    put(&format!("{id}.ps2.trn"), &bytes)?;
    put(&format!("{id}.scn"), scene.as_bytes())?;
    let name = if t.title.is_empty() { id.clone() } else { t.title.clone() };
    let mut meta = vec![
        ("standard", J::N(1.0)),
        ("kind", s("track")),
        ("id", s(&id)),
        ("name", s(name)),
        ("category", s(&t.environment)),
        ("source", o(vec![("game", s("MXA Track Studio")), ("data", s(format!("an .mxatrack, track {}", t.id)))])),
        ("version", s("1.0.0")),
        ("author", s(&t.author)),
        ("permission", s("the track's author's; imported on this machine from the file they made")),
        (
            "todo",
            J::A(vec![
                s(format!("ground texture {} not drawn: the ground has a grey palette", t.ground_texture)),
                s("the document's scenery, props and their collision are not laid out"),
            ]),
        ),
        ("mxatrack", stamp),
    ];
    if !t.description.is_empty() {
        meta.insert(4, ("description", s(&t.description)));
    }
    put("mod.json", o(meta).pretty().as_bytes())?;
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::rename(&staged, &dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

/// A `.mxatrack` holding `t`, laid out as TrackCore exports one: the manifest, a track document
/// with `t`'s fields (plus `extra`, more of the document's own fields written as JSON members,
/// `"route": [], ...`, or empty), and the collision in its v3 layout (the surfaces when `t` has them,
/// no solid pieces); the document deflated, the rest stored. For tests, which have no real one.
pub fn package(t: &Track, extra: &str) -> Vec<u8> {
    let mut c = Vec::new();
    let mut put = |b: &[u8]| c.extend_from_slice(b);
    put(&MSC_MAGIC.to_le_bytes());
    put(&3u32.to_le_bytes());
    let low = t.heights.iter().copied().fold(0f32, f32::min) - 2.0;
    let high = t.heights.iter().copied().fold(0f32, f32::max) + 5.0;
    let v = [-t.width / 2.0, low, -t.depth / 2.0, t.width / 2.0, high, t.depth / 2.0];
    for f in v.iter().chain(&t.spawn).chain(&t.direction) {
        put(&f.to_le_bytes());
    }
    for n in [t.resolution, t.rows, u32::from(t.surface)] {
        put(&n.to_le_bytes());
    }
    for h in &t.heights {
        put(&h.to_le_bytes());
    }
    match &t.surfaces {
        Some(m) => {
            put(&(m.len() as u32).to_le_bytes());
            put(m);
            put(&vec![0u8; (4 - m.len() % 4) % 4]);
        }
        None => put(&0u32.to_le_bytes()),
    }
    put(&0u32.to_le_bytes());
    let manifest = format!(r#"{{"version":1,"collisionSha256":"{}","units":"feet"}}"#, sha256_hex(&c));
    let q = |v: &str| J::S(v.to_owned()).compact();
    let doc = format!(
        r#"{{"schemaVersion":8,"id":{},"title":{},"author":{},"environment":{},"width":{:?},"depth":{:?},"venueId":{},"groundTexture":{},"description":{},"location":{},"resolution":{},"resolutionZ":{},"surface":{}{}{}}}"#,
        q(&t.id),
        q(&t.title),
        q(&t.author),
        q(&t.environment),
        t.width,
        t.depth,
        q(&t.venue),
        q(&t.ground_texture),
        q(&t.description),
        q(&t.location),
        t.resolution,
        if t.rows == t.resolution { 0 } else { t.rows },
        t.surface,
        if extra.is_empty() { "" } else { "," },
        extra
    );
    zip(&[("manifest.json", manifest.as_bytes(), false), ("track.json", doc.as_bytes(), true), ("collision.msc", &c, false)])
}

/// A zip of `(name, bytes, deflate)` entries.
fn zip(files: &[(&str, &[u8], bool)]) -> Vec<u8> {
    let (mut out, mut dir) = (Vec::new(), Vec::new());
    for &(name, bytes, deflate) in files {
        let packed = if deflate { miniz_oxide::deflate::compress_to_vec(bytes, 6) } else { bytes.to_vec() };
        let (method, crc) = (if deflate { 8u16 } else { 0 }, crc32(bytes));
        let local = out.len() as u32;
        let head = |sig: u32, out: &mut Vec<u8>, central: bool| {
            out.extend_from_slice(&sig.to_le_bytes());
            if central {
                out.extend_from_slice(&20u16.to_le_bytes());
            }
            for h in [20u16, 0, method, 0, 0x21] {
                out.extend_from_slice(&h.to_le_bytes());
            }
            for w in [crc, packed.len() as u32, bytes.len() as u32] {
                out.extend_from_slice(&w.to_le_bytes());
            }
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            if central {
                for h in [0u16, 0, 0] {
                    out.extend_from_slice(&h.to_le_bytes());
                }
                out.extend_from_slice(&0u32.to_le_bytes());
                out.extend_from_slice(&local.to_le_bytes());
            }
            out.extend_from_slice(name.as_bytes());
        };
        head(0x0403_4b50, &mut out, false);
        out.extend_from_slice(&packed);
        head(0x0201_4b50, &mut dir, true);
    }
    let at = out.len() as u32;
    out.extend_from_slice(&dir);
    out.extend_from_slice(b"PK\x05\x06");
    for h in [0u16, 0, files.len() as u16, files.len() as u16] {
        out.extend_from_slice(&h.to_le_bytes());
    }
    out.extend_from_slice(&(dir.len() as u32).to_le_bytes());
    out.extend_from_slice(&at.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn text(b: &[u8], name: &str) -> Result<String, String> {
    // JsonUtility writes UTF-8; a byte-order mark is skipped.
    let b = b.strip_prefix(b"\xef\xbb\xbf").unwrap_or(b);
    String::from_utf8(b.to_vec()).map_err(|_| format!("{name} is not UTF-8"))
}

/// SHA-256 as lowercase hex, as the manifest writes it.
pub fn sha256_hex(b: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn bytes(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.b.len()).ok_or("collision.msc is cut short")?;
        let out = &self.b[self.at..end];
        self.at = end;
        Ok(out)
    }

    fn u32(&mut self) -> Result<u32, String> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn f32(&mut self) -> Result<f32, String> {
        self.u32().map(f32::from_bits)
    }
}

// ---- the zip ------------------------------------------------------------------------------------

/// A zip's directory over its bytes: stored and deflated entries, no zip64 (MXA's packages stop at
/// 512 MB).
struct Zip<'a> {
    b: &'a [u8],
    entries: Vec<(String, u16, u32, usize, usize, usize)>,
}

fn u16_at(b: &[u8], at: usize) -> Result<usize, String> {
    b.get(at..at + 2).map(|x| u16::from_le_bytes([x[0], x[1]]) as usize).ok_or_else(|| "the zip is cut short".into())
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]])).ok_or_else(|| "the zip is cut short".into())
}

impl<'a> Zip<'a> {
    fn read(b: &'a [u8]) -> Result<Zip<'a>, String> {
        let from = b.len().saturating_sub(22 + 0xffff);
        let end = (from..=b.len().saturating_sub(22)).rev().find(|&i| b[i..].starts_with(b"PK\x05\x06")).ok_or("not a zip file")?;
        let (n, at) = (u16_at(b, end + 10)?, u32_at(b, end + 16)? as usize);
        let mut i = at;
        let mut entries = Vec::with_capacity(n);
        for _ in 0..n {
            if u32_at(b, i)? != 0x0201_4b50 {
                return Err("a damaged zip directory".into());
            }
            let (flags, method) = (u16_at(b, i + 8)?, u16_at(b, i + 10)? as u16);
            let crc = u32_at(b, i + 16)?;
            let (packed, size) = (u32_at(b, i + 20)? as usize, u32_at(b, i + 24)? as usize);
            let (nlen, xlen, clen) = (u16_at(b, i + 28)?, u16_at(b, i + 30)?, u16_at(b, i + 32)?);
            let local = u32_at(b, i + 42)? as usize;
            let name = b.get(i + 46..i + 46 + nlen).ok_or("the zip is cut short")?;
            let name = String::from_utf8_lossy(name).into_owned();
            if flags & 1 != 0 {
                return Err(format!("{name} is encrypted"));
            }
            if entries.iter().any(|e: &(String, u16, u32, usize, usize, usize)| e.0 == name) {
                return Err(format!("{name} is in the zip twice"));
            }
            entries.push((name, method, crc, packed, size, local));
            i += 46 + nlen + xlen + clen;
        }
        Ok(Zip { b, entries })
    }

    fn has(&self, name: &str) -> bool {
        self.entries.iter().any(|e| e.0 == name)
    }

    /// An entry inflated and checked against its CRC, at most `cap` bytes.
    fn file(&self, name: &str, cap: usize) -> Result<Vec<u8>, String> {
        let &(_, method, crc, packed, size, local) = self.entries.iter().find(|e| e.0 == name).ok_or_else(|| format!("no {name}"))?;
        if size > cap {
            return Err(format!("{name} is too big"));
        }
        if u32_at(self.b, local)? != 0x0403_4b50 {
            return Err(format!("{name}: damaged in the zip"));
        }
        let from = local + 30 + u16_at(self.b, local + 26)? + u16_at(self.b, local + 28)?;
        let data = self.b.get(from..from + packed).ok_or_else(|| format!("{name}: cut short in the zip"))?;
        let out = match method {
            0 => data.to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec_with_limit(data, size).map_err(|_| format!("{name}: damaged in the zip"))?,
            m => return Err(format!("{name}: packed with method {m}, and stored or deflate is read")),
        };
        if out.len() != size || crc32(&out) != crc {
            return Err(format!("{name}: damaged in the zip"));
        }
        Ok(out)
    }
}

/// The zip's CRC-32.
pub fn crc32(d: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, e) in t.iter_mut().enumerate() {
            *e = (0..8).fold(i as u32, |c, _| (c >> 1) ^ (0xEDB8_8320 & (c & 1).wrapping_neg()));
        }
        t
    });
    !d.iter().fold(!0u32, |c, &b| t[((c ^ u32::from(b)) & 0xff) as usize] ^ (c >> 8))
}
