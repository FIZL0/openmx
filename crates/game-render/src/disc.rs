//! Getting bytes off the user's own disc, at runtime, in three hops.
//!
//! 1. **ISO 9660.** A PS2 disc is plain ISO 9660 with 2048-byte sectors and no extensions worth reading.
//!    The primary volume descriptor is at sector 16; its root directory record is at +156; every file in
//!    the root is a (extent, size) pair. The only file this crate wants is `MX.PSR`.
//! 2. **`MX.PSR`.** One container holding the whole game, parsed by [`game_sim::archive::Psr`]: a
//!    directory at the front and every member aligned to a disc sector. Members are not compressed here.
//! 3. **`.res`.** Each track, bike and menu is one of those members, and is itself an archive of
//!    deflate-compressed blocks — [`game_sim::archive::Res`]. Reading one 400-byte material out of a
//!    10 MB track inflates the one block it lives in.
//!
//! Nothing is cached to disk and nothing is copied into the repository: the disc stays the only source.

use game_sim::archive::{Entry, Psr};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const SECTOR: u64 = 2048;

/// Anything a disc image can be read out of by offset: a file on the desktop, or the browser's own
/// storage behind the web shell's host calls (N4), where there is no file system to open a path in.
pub trait Image: Read + Seek {}
impl<T: Read + Seek> Image for T {}

/// An opened disc image, with `MX.PSR`'s directory in memory.
pub struct Disc {
    file: Box<dyn Image>,
    /// Where `MX.PSR` starts in the image.
    psr_at: u64,
    psr: Psr,
}

impl Disc {
    /// Open an ISO 9660 image and find `MX.PSR` in its root directory.
    pub fn open(path: &Path) -> Result<Disc, String> {
        let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Disc::read_from(Box::new(file)).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The same, out of an image that is not a file: the browser hands the shell its copy of the disc
    /// a read at a time.
    pub fn read_from(mut file: Box<dyn Image>) -> Result<Disc, String> {
        let pvd = read_at(&mut file, 16 * SECTOR, SECTOR as usize)?;
        if pvd.len() < 6 || &pvd[1..6] != b"CD001" {
            return Err("no CD001 at sector 16".to_string());
        }
        let (root_extent, root_size) = (le32(&pvd[156 + 2..]), le32(&pvd[156 + 10..]) as usize);
        let root = read_at(&mut file, root_extent as u64 * SECTOR, root_size)?;
        let psr_at = find_in_directory(&root, "MX.PSR")
            .ok_or("no MX.PSR in the root directory")?;
        // 64 KiB covers the header and the whole directory; `Psr::parse` says so if it does not.
        let front = read_at(&mut file, psr_at, 64 << 10)?;
        let psr = Psr::parse(&front)?;
        Ok(Disc {
            file,
            psr_at,
            psr,
        })
    }

    /// The disc this crate's tools use when nothing else is said: `[game] iso` from `kit.toml`, looked
    /// for in this directory and its parents. The error is the message a test should skip with.
    pub fn open_configured() -> Result<Disc, String> {
        let (root, iso) = configured_iso()?;
        let path = root.join(iso);
        if !path.exists() {
            return Err(format!(
                "no disc: {} does not exist. Dump your own and set [game] iso in kit.toml.",
                path.display()
            ));
        }
        Disc::open(&path)
    }

    /// Where `open_configured` looks for the disc, whether or not it is there: the shell keeps its
    /// settings beside it (P36).
    pub fn configured_path() -> Result<PathBuf, String> {
        let (root, iso) = configured_iso()?;
        Ok(root.join(iso))
    }

    /// Every member of `MX.PSR`, in directory order.
    pub fn entries(&self) -> &[Entry] {
        &self.psr.entries
    }

    /// One member's bytes, by name (`tracks\sx01.res`), matched without regard to case or slash style.
    pub fn member(&mut self, name: &str) -> Result<Vec<u8>, String> {
        let want = normalise(name);
        let e = self
            .psr
            .entries
            .iter()
            .find(|e| normalise(&e.name) == want)
            .ok_or_else(|| format!("MX.PSR has no member {name}"))?;
        let (at, size) = (self.psr_at + e.at, e.size as usize);
        read_at(&mut self.file, at, size)
    }

    /// Part of one member: `len` bytes from `at` into it, for a member too big to read whole (the
    /// 160 MB sound archive, P42). Short at the member's end.
    pub fn member_part(&mut self, name: &str, at: u64, len: usize) -> Result<Vec<u8>, String> {
        let want = normalise(name);
        let e = self
            .psr
            .entries
            .iter()
            .find(|e| normalise(&e.name) == want)
            .ok_or_else(|| format!("MX.PSR has no member {name}"))?;
        if at >= e.size as u64 {
            return Err(format!("{name} has no byte {at}"));
        }
        let len = len.min((e.size as u64 - at) as usize);
        read_at(&mut self.file, self.psr_at + e.at + at, len)
    }

    /// One `.res` archive, unpacked far enough to read members out of it.
    pub fn res(&mut self, name: &str) -> Result<Res, String> {
        let bytes = self.member(name)?;
        let dir = game_sim::archive::Res::parse(&bytes)?;
        Ok(Res { bytes, dir })
    }
}

/// A `.res` archive held in memory, with its directory parsed.
pub struct Res {
    bytes: Vec<u8>,
    dir: game_sim::archive::Res,
}

impl Res {
    /// Every member, in directory order.
    pub fn entries(&self) -> &[Entry] {
        &self.dir.entries
    }

    /// One member's bytes, by name, matched without regard to case or slash style.
    pub fn read(&self, name: &str) -> Result<Vec<u8>, String> {
        let want = normalise(name);
        let e = self
            .dir
            .entries
            .iter()
            .find(|e| normalise(&e.name) == want)
            .ok_or_else(|| format!("the archive has no member {name}"))?;
        self.dir.read(&self.bytes, e)
    }

    /// The first member whose name ends in `suffix` — how the scene's `SLT=bale_single_02` finds
    /// `geom\bale_single_02.psg` without knowing which directory the track keeps its models in.
    pub fn find_suffix(&self, suffix: &str) -> Option<&Entry> {
        let want = normalise(suffix);
        self.dir
            .entries
            .iter()
            .find(|e| normalise(&e.name).ends_with(&want))
    }

    /// `find_suffix` and `read` in one step.
    pub fn read_suffix(&self, suffix: &str) -> Result<Vec<u8>, String> {
        let e = self
            .find_suffix(suffix)
            .ok_or_else(|| format!("the archive has no member ending in {suffix}"))?;
        self.dir.read(&self.bytes, e)
    }
}

/// Where [`Disc::open_configured`] looks: `[game] iso` from the nearest `kit.toml`, joined to the
/// directory it was found in. The web shell's test hands this path to its stand-in for the browser.
pub fn configured_image() -> Result<PathBuf, String> {
    let (root, iso) = configured_iso()?;
    Ok(root.join(iso))
}

/// Names on this disc are spelled with either slash and in any case; comparisons fold both.
fn normalise(name: &str) -> String {
    name.replace('\\', "/").to_ascii_lowercase()
}

/// `(repo root, iso path as written)` from the nearest `kit.toml`.
fn configured_iso() -> Result<(PathBuf, String), String> {
    let mut dir = std::env::current_dir().map_err(|e| e.to_string())?;
    loop {
        let toml = dir.join("kit.toml");
        if toml.is_file() {
            let text = std::fs::read_to_string(&toml).map_err(|e| format!("{}: {e}", toml.display()))?;
            for line in text.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("iso") {
                    if let Some(rest) = rest.trim_start().strip_prefix('=') {
                        let v = rest.split('#').next().unwrap_or("").trim();
                        let v = v.trim_matches('"').trim_matches('\'');
                        if !v.is_empty() {
                            return Ok((dir, v.to_string()));
                        }
                    }
                }
            }
            return Err(format!("{}: no [game] iso = ... line", toml.display()));
        }
        if !dir.pop() {
            return Err("no kit.toml in this directory or any above it".to_string());
        }
    }
}

/// Scan one ISO 9660 directory extent for a file, returning its byte offset in the image.
fn find_in_directory(dir: &[u8], name: &str) -> Option<u64> {
    let mut at = 0usize;
    while at + 33 < dir.len() {
        let len = dir[at] as usize;
        if len == 0 {
            // The rest of this sector is padding; records never straddle one.
            at = (at / SECTOR as usize + 1) * SECTOR as usize;
            continue;
        }
        let name_len = dir[at + 32] as usize;
        if at + 33 + name_len <= dir.len() {
            let raw = &dir[at + 33..at + 33 + name_len];
            // "MX.PSR;1": the version suffix is part of the stored name.
            let got = String::from_utf8_lossy(raw);
            let got = got.split(';').next().unwrap_or("");
            if got.eq_ignore_ascii_case(name) {
                return Some(le32(&dir[at + 2..]) as u64 * SECTOR);
            }
        }
        at += len;
    }
    None
}

fn le32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn read_at(file: &mut Box<dyn Image>, at: u64, len: usize) -> Result<Vec<u8>, String> {
    file.seek(SeekFrom::Start(at)).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; len];
    let mut got = 0;
    while got < len {
        match file.read(&mut buf[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) => return Err(e.to_string()),
        }
    }
    buf.truncate(got);
    if buf.is_empty() {
        return Err(format!("read {len} bytes at {at}: nothing there"));
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_record_is_found_by_name_without_its_version() {
        // One synthetic record: length 48, extent 100, 8-byte name "MX.PSR;1".
        let mut rec = vec![0u8; 48];
        rec[0] = 48;
        rec[2..6].copy_from_slice(&100u32.to_le_bytes());
        rec[32] = 8;
        rec[33..41].copy_from_slice(b"MX.PSR;1");
        assert_eq!(find_in_directory(&rec, "MX.PSR"), Some(100 * SECTOR));
        assert_eq!(find_in_directory(&rec, "OTHER"), None);
    }

    #[test]
    fn names_compare_across_slashes_and_case() {
        assert_eq!(normalise("Tracks\\SX01.res"), normalise("tracks/sx01.res"));
    }
}
