//! **BPB: MX vs. ATV Untamed's binary INI (MD69).** Untamed has no text INIs: every one the earlier
//! games read as text (the stunt table, `.scn`, `.toy`, `.psm`, `.prt`, `.aud`, ...) is on its disc as
//! `<name>.bpb`, the same sections and keys compiled to a table. SLUS_217.01's loader (`0x001425c0`)
//! opens `%s.bpb` first and reads the text INI only when there is none; [`ini`] reads either, so a
//! reader of a text INI takes the BPB as it is (openmx-mods `notes/mxvatv-later.md`, *Scenes: binary
//! (BPB)*, and its `tools/bpb`).
//!
//! ```text
//! header   0x00 "BPB\0"
//!          0x04 u32 offset of the section records (0x20)
//!          0x08 u32 offset of the key records
//!          0x0c u16 sections   0x0e u16 keys
//!          0x10 u32 offset of the name table      0x14 u32 its length
//!          0x18 u32 offset of the value strings   0x1c u32 their length
//! section  16 bytes: u32 hash, u32 name (offset in the name table), u16 index (0xffff: none),
//!          u16 keys, u32 first key
//! key      16 bytes: u32 hash, u32 name, u16 type, u16 0, then by type:
//!          1 a string, u32 offset in the value strings;  2 a string of up to five bytes, held in the
//!          record's last six (NUL-padded);  4 an i32;  8 an f32
//! ```
//!
//! A section's title is its name and then its index when it has one (`%s%d`, `0x00142c30`): `Stunt`
//! index 1 is `[Stunt1]` — no zero padding, where MX Unleashed's text tables write `[Stunt01]`. The
//! game looks names up by [`hash`] and falls back to a case-folded compare; here they are matched by
//! name, case-folded, as the text reader's are, so the hashes are not checked.
//!
//! The values become the text the [`Ini`] getters read: a string as it is, an int in decimal, and a
//! float as the shortest decimal of it widened to a double, which is the float exactly — the text
//! reader's `strtod` and truncating double-to-single give back the same bits.

use crate::phy::Ini;

pub const MAGIC: &[u8; 4] = b"BPB\0";
const HEADER: usize = 0x20;
const RECORD: usize = 16;

/// The name hash: djb2 (`h * 33 + c` from 5381) over the name with ASCII letters lowered.
pub fn hash(name: &[u8]) -> u32 {
    name.iter().fold(5381u32, |h, &c| h.wrapping_mul(33).wrapping_add(u32::from(c.to_ascii_lowercase())))
}

/// Whether the bytes are a BPB (or the `ZLIBME` wrapping of one the loader also takes).
pub fn is_bpb(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC) || bytes.starts_with(b"ZLIBME")
}

/// Read an INI file's bytes as the game's loader takes them: a BPB when they are one, else the text.
pub fn ini(bytes: &[u8]) -> Result<Ini, String> {
    if is_bpb(bytes) { parse(bytes) } else { Ok(Ini::parse_bytes(bytes)) }
}

/// Read a BPB into the sections and pairs the text reader would give for its INI.
pub fn parse(b: &[u8]) -> Result<Ini, String> {
    if b.starts_with(b"ZLIBME") {
        return Err("a ZLIBME (zlib-wrapped) BPB: not read here, and none is on Untamed's disc".into());
    }
    if b.get(..4) != Some(MAGIC) || b.len() < HEADER {
        return Err(format!("not a BPB (starts {:02x?})", &b[..b.len().min(4)]));
    }
    let sec_off = u32_at(b, 4)?;
    let key_off = u32_at(b, 8)?;
    let (n_sec, n_key) = (usize::from(u16_at(b, 0xc)?), usize::from(u16_at(b, 0xe)?));
    let names = region(b, u32_at(b, 0x10)?, u32_at(b, 0x14)? as usize, "the name table")?;
    let values = region(b, u32_at(b, 0x18)?, u32_at(b, 0x1c)? as usize, "the value strings")?;
    let secs = region(b, sec_off, n_sec * RECORD, "the section records")?;
    let keys = region(b, key_off, n_key * RECORD, "the key records")?;

    let mut titles = Vec::with_capacity(n_sec);
    let mut pairs = Vec::with_capacity(n_key);
    for (s, r) in secs.chunks_exact(RECORD).enumerate() {
        let mut title = latin1(cstr(names, u32_at(r, 4)?, "a section name")?);
        let index = u16_at(r, 8)?;
        let (count, first) = (usize::from(u16_at(r, 10)?), u32_at(r, 12)? as usize);
        if first + count > n_key {
            return Err(format!("section {s} ({title}): keys {first}+{count} of {n_key}"));
        }
        if index != 0xffff {
            // `%d` of the index the game holds as a signed 16-bit number.
            title.push_str(&(index as i16).to_string());
        }
        for r in keys[first * RECORD..(first + count) * RECORD].chunks_exact(RECORD) {
            let name = latin1(cstr(names, u32_at(r, 4)?, "a key name")?);
            let value = match u16_at(r, 8)? {
                1 => latin1(cstr(values, u32_at(r, 12)?, "a string value")?),
                2 => {
                    let raw = &r[10..16];
                    latin1(&raw[..raw.iter().position(|&c| c == 0).unwrap_or(raw.len())])
                }
                4 => (u32_at(r, 12)? as i32).to_string(),
                8 => float_text(f32::from_bits(u32_at(r, 12)?)),
                t => return Err(format!("{title}.{name}: type {t} is not one of 1, 2, 4, 8")),
            };
            // The text reader drops a pair with nothing after its `=`; so does this.
            if !value.is_empty() {
                pairs.push((titles.len(), name, value));
            }
        }
        titles.push(title);
    }
    Ok(Ini::from_parts(titles, pairs))
}

/// A float as text that reads back to its bits: the shortest decimal of the float as a double.
fn float_text(f: f32) -> String {
    let mut s = format!("{}", f64::from(f));
    if f.is_finite() && !s.contains(['.', 'e']) {
        s.push_str(".0");
    }
    s
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

fn u16_at(b: &[u8], at: usize) -> Result<u16, String> {
    b.get(at..at + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| format!("{at:#x}: past the end ({} bytes)", b.len()))
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| format!("{at:#x}: past the end ({} bytes)", b.len()))
}

/// The NUL-terminated string at `off` in a table.
fn cstr<'a>(table: &'a [u8], off: u32, what: &str) -> Result<&'a [u8], String> {
    let rest = table
        .get(off as usize..)
        .ok_or_else(|| format!("{what} at {off:#x} is past its table ({:#x} bytes)", table.len()))?;
    let end = rest.iter().position(|&c| c == 0).ok_or_else(|| format!("{what} at {off:#x} has no NUL"))?;
    Ok(&rest[..end])
}

fn region<'a>(b: &'a [u8], off: u32, len: usize, what: &str) -> Result<&'a [u8], String> {
    b.get(off as usize..off as usize + len)
        .ok_or_else(|| format!("{what}: {len:#x} bytes at {off:#x} pass the end ({:#x})", b.len()))
}
