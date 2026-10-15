//! The game's two archive formats, and the inflate they need.
//!
//! Everything on the disc past the boot ELF lives in one 1.4 GB container, and nearly every member of
//! *that* is itself a second container holding the files one part of the game needs — a track, a bike, a
//! menu. Both layouts are in `research/journal/d5-archives.md` with the hexdump they were read off.
//!
//! ```text
//! MX.PSR  ──┬── Tracks\sx01.res ──┬── tracks\sx01\sx01.scn     the track
//!           │                     ├── tracks\sx01\sx01.trn     its terrain
//!           │                     └── col\*.col                its collision
//!           ├── Bike\250sx.res ───┬── bike\race250a.phy         the bike's physics parameters
//!           │                     └── bike\anim\*.psg           its geometry
//!           └── machine.res
//! ```
//!
//! # Why this is slices and not files
//!
//! This crate does no I/O (see the rules in [`crate`]), so both readers parse a byte slice and hand back
//! *where* each member is rather than its bytes. The outer archive is bigger than anything worth holding
//! in memory, so [`Psr::parse`] wants only the front of the file — the directory — and gives byte
//! offsets for the app layer to read. The inner ones are a couple of megabytes and arrive whole.
//!
//! # Why a hand-written inflate
//!
//! The port has no third-party dependencies, which is worth more than the two hundred lines below: it is
//! what lets the simulation build for wasm and aarch64 without a C toolchain. Inflate is lossless, so
//! unlike the arithmetic in [`crate::physics`] there is nothing here that has to match the PS2 bit for
//! bit — it either reproduces the bytes or it fails.

#![deny(clippy::float_arithmetic)]

/// One member of an archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The name exactly as the archive spells it, `\`-separated: `tracks\sx01\sx01.scn`.
    pub name: String,
    /// Where the member starts. A byte offset into the `.PSR` file for [`Psr`]; a byte offset into the
    /// *unpacked* stream for [`Res`].
    pub at: u64,
    /// The member's length in bytes.
    pub size: u32,
}

impl Entry {
    /// The name with `\` turned into `/`, for writing the member out as a file.
    pub fn path(&self) -> String {
        self.name.replace('\\', "/")
    }
}

/// The disc's one big container: `MX.PSR`.
///
/// A directory at the front, then every member laid out in 2048-byte sectors — disc sectors, so that
/// loading a member is one seek and one read of whole sectors. Members are *not* compressed here; the
/// compression is a level down, in [`Res`].
///
/// Entries named `<PAD>` are reserved gaps rather than files, and are dropped during parsing.
#[derive(Clone, Debug)]
pub struct Psr {
    /// Every real member, in the order the directory lists them.
    pub entries: Vec<Entry>,
    /// How many bytes of directory the header claims; the parser checks it against what it read.
    pub dir_bytes: u32,
}

impl Psr {
    /// The sector size members are aligned to.
    pub const SECTOR: u64 = 2048;

    /// Parse the directory. `front` must hold the header and the whole directory — the header's
    /// directory length is at offset 0x10, so `64 << 10` is plenty and the error says if it is not.
    ///
    /// ```text
    /// 0x00  u32 2          version
    /// 0x04  u32 n          entries, pads included
    /// 0x08  u32            unknown, stable across the one disc we have
    /// 0x0c  u32            unknown
    /// 0x10  u32            directory length in bytes, from 0x18
    /// 0x14  u32            unknown
    /// 0x18  n x { u32 namelen, char name[namelen], u32 size, u32 first_sector }
    /// ```
    pub fn parse(front: &[u8]) -> Result<Psr, String> {
        let version = u32_at(front, 0)?;
        if version != 2 {
            return Err(format!("MX.PSR: version {version}, expected 2"));
        }
        let n = u32_at(front, 4)? as usize;
        let dir_bytes = u32_at(front, 0x10)?;
        let mut entries = Vec::with_capacity(n);
        let mut at = 0x18;
        for k in 0..n {
            let len = u32_at(front, at)? as usize;
            if len == 0 || len > 256 {
                return Err(format!(
                    "MX.PSR: entry {k} has a {len}-byte name at {at:#x}"
                ));
            }
            let name = ascii(front, at + 4, len)
                .ok_or_else(|| format!("MX.PSR: entry {k}'s name is not ASCII"))?;
            let size = u32_at(front, at + 4 + len)?;
            let sector = u32_at(front, at + 8 + len)? as u64;
            at += 12 + len;
            // A reserved gap, not a file: same shape, a name no loader asks for.
            if name != "<PAD>" {
                entries.push(Entry {
                    name,
                    at: sector * Psr::SECTOR,
                    size,
                });
            }
        }
        let read = (at - 0x18) as u32;
        if read != dir_bytes {
            return Err(format!(
                "MX.PSR: the header claims a {dir_bytes}-byte directory, {read} bytes were read"
            ));
        }
        Ok(Psr { entries, dir_bytes })
    }

    /// The member of that exact name, if the archive has one. Names are case-sensitive here; the game's
    /// own references are not always, so a caller matching user-facing names should fold case itself.
    pub fn find(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name == name)
    }
}

/// A `.res` container: one deflated stream, cut into fixed-size blocks, with a directory of members.
///
/// The cutting is the point. Every block unpacks to exactly [`Res::BLOCK`] bytes and every member starts
/// on a block boundary, so reading one 433-byte file out of a 10 MB stream inflates one block rather
/// than all four hundred of them — which is how the game loads a single bike off a disc with 32 MB of
/// RAM. [`Res::read`] does exactly that; [`Res::unpack`] is the whole stream, for tooling.
#[derive(Clone, Debug)]
pub struct Res {
    /// Every member, in directory order.
    pub entries: Vec<Entry>,
    /// The packed length of each block, in file order.
    packed: Vec<u16>,
    /// Where the first block starts.
    data_at: usize,
}

impl Res {
    /// What every block unpacks to. The last block of a file is this size too — the stream is padded.
    pub const BLOCK: usize = 24576;

    /// Parse the header, the block table and the directory.
    ///
    /// ```text
    /// 0x00  u32 3          version
    /// 0x04  u32            table_bytes + 12
    /// 0x08  u32 832        constant on this disc, meaning unknown
    /// 0x0c  u32 0
    /// 0x10  u32            table_bytes: the block table including its terminator, so blocks = n/2 - 1
    /// 0x14  u16 packed[n]  each block's deflated length
    ///       u16 0          terminator
    ///       u32            entries
    ///       u32            directory length in bytes
    ///       entries x { u32 namelen, char name[namelen], u8 0, u24 at/256, u32 size }
    ///       blocks x  { u8 deflated[packed[i]], u8 0xff }
    /// ```
    ///
    /// The block count is taken from the header rather than by scanning for the terminator, so a zero
    /// in the table cannot truncate the archive silently.
    pub fn parse(file: &[u8]) -> Result<Res, String> {
        let version = u32_at(file, 0)?;
        if version != 3 {
            return Err(format!(".res: version {version}, expected 3"));
        }
        let table_bytes = u32_at(file, 0x10)? as usize;
        if table_bytes < 2 || table_bytes % 2 != 0 {
            return Err(format!(".res: a {table_bytes}-byte block table"));
        }
        let blocks = table_bytes / 2 - 1; // the terminating zero is in that count
        let mut packed = Vec::with_capacity(blocks);
        for k in 0..blocks {
            packed.push(u16_at(file, 0x14 + k * 2)?);
        }
        if u16_at(file, 0x14 + blocks * 2)? != 0 {
            return Err(format!(".res: no terminator after {blocks} block lengths"));
        }

        let mut at = 0x14 + table_bytes;
        let n = u32_at(file, at)? as usize;
        let dir_bytes = u32_at(file, at + 4)? as usize;
        at += 8;
        let dir_start = at;
        let mut entries = Vec::with_capacity(n);
        for k in 0..n {
            let len = u32_at(file, at)? as usize;
            if len == 0 || len > 256 {
                return Err(format!(".res: entry {k} has a {len}-byte name at {at:#x}"));
            }
            let name =
                ascii(file, at + 4, len).ok_or_else(|| format!(".res: entry {k} is not ASCII"))?;
            let end = at + 4 + len;
            if *file.get(end).ok_or(".res: truncated directory")? != 0 {
                return Err(format!(".res: no NUL after {name}"));
            }
            // The offset is in 256-byte units, which is only ever a multiple of the block size — the
            // u24 exists so that a 10 MB stream's offsets fit in three bytes.
            let at_256 =
                u32::from_le_bytes([file[end + 1], file[end + 2], file[end + 3], 0]) as u64;
            let size = u32_at(file, end + 4)?;
            at = end + 8;
            entries.push(Entry {
                name,
                at: at_256 * 256,
                size,
            });
        }
        if at - dir_start != dir_bytes {
            return Err(format!(
                ".res: the header claims a {dir_bytes}-byte directory, {} bytes were read",
                at - dir_start
            ));
        }

        let res = Res {
            entries,
            packed,
            data_at: at,
        };
        // Every byte of the file is now accounted for, which is the check that the whole layout is
        // right rather than merely plausible.
        let want = res.data_at + res.packed.iter().map(|&p| p as usize + 1).sum::<usize>();
        if want != file.len() {
            return Err(format!(
                ".res: the layout accounts for {want} bytes of a {}-byte file",
                file.len()
            ));
        }
        Ok(res)
    }

    /// How many blocks the stream is cut into.
    pub fn blocks(&self) -> usize {
        self.packed.len()
    }

    /// What the stream unpacks to in total, padding included.
    pub fn unpacked_len(&self) -> usize {
        self.packed.len() * Res::BLOCK
    }

    /// The member of that exact name, if the archive has one.
    pub fn find(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// One member's bytes, inflating only the blocks it spans.
    pub fn read(&self, file: &[u8], e: &Entry) -> Result<Vec<u8>, String> {
        let first = (e.at as usize) / Res::BLOCK;
        let skip = (e.at as usize) % Res::BLOCK;
        let span = skip + e.size as usize;
        let count = span.div_ceil(Res::BLOCK);
        if first + count > self.packed.len() {
            return Err(format!(
                "{}: runs to {} of a {}-byte stream",
                e.name,
                e.at as usize + e.size as usize,
                self.unpacked_len()
            ));
        }
        let mut out = Vec::with_capacity(count * Res::BLOCK);
        let mut at = self.data_at
            + self.packed[..first]
                .iter()
                .map(|&p| p as usize + 1)
                .sum::<usize>();
        for k in first..first + count {
            let len = self.packed[k] as usize;
            let end = at + len;
            let block = file
                .get(at..end)
                .ok_or_else(|| format!("{}: block {k} is cut off", e.name))?;
            let (bytes, used) = inflate::zlib(block)?;
            if used != len {
                return Err(format!(
                    "{}: block {k} is {len} bytes, {used} were read",
                    e.name
                ));
            }
            if bytes.len() != Res::BLOCK {
                return Err(format!("{}: block {k} unpacked to {}", e.name, bytes.len()));
            }
            out.extend_from_slice(&bytes);
            // One 0xff between blocks. Not a length, not a checksum: a separator the game's loader
            // steps over, and the only thing in either format that is not self-describing.
            if *file
                .get(end)
                .ok_or_else(|| format!("{}: no byte after block {k}", e.name))?
                != 0xff
            {
                return Err(format!("{}: block {k} is not followed by 0xff", e.name));
            }
            at = end + 1;
        }
        out.drain(..skip);
        out.truncate(e.size as usize);
        Ok(out)
    }

    /// The whole stream, every block inflated. For tools that want all of it; the game never does this.
    pub fn unpack(&self, file: &[u8]) -> Result<Vec<u8>, String> {
        let whole = Entry {
            name: "<the whole stream>".into(),
            at: 0,
            size: self.unpacked_len() as u32,
        };
        self.read(file, &whole)
    }
}

fn u16_at(d: &[u8], at: usize) -> Result<u16, String> {
    let b = d
        .get(at..at + 2)
        .ok_or_else(|| format!("short read at {at:#x}"))?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

fn u32_at(d: &[u8], at: usize) -> Result<u32, String> {
    let b = d
        .get(at..at + 4)
        .ok_or_else(|| format!("short read at {at:#x}"))?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn ascii(d: &[u8], at: usize, len: usize) -> Option<String> {
    let b = d.get(at..at + len)?;
    b.iter()
        .all(|&c| (0x20..0x7f).contains(&c))
        .then(|| String::from_utf8_lossy(b).into_owned())
}

/// DEFLATE (RFC 1951) and its zlib wrapper (RFC 1950), decompression only.
///
/// Written from the RFCs, in the shape of zlib's own `puff.c`: a canonical Huffman table is a count per
/// code length plus the symbols in length order, which decodes bit by bit with no table building worth
/// the name. Slower than a real inflate and perfectly fast enough — a 2 MB archive is milliseconds, and
/// the simulation never inflates anything in a frame.
pub mod inflate {
    /// Base length for each of the 29 length codes (257..286).
    const LEN: [u16; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    /// Extra bits to read for each length code.
    const LEN_BITS: [u8; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    /// Base distance for each of the 30 distance codes.
    const DIST: [u16; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    /// Extra bits to read for each distance code.
    const DIST_BITS: [u8; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];
    /// The order the dynamic-block header lists code lengths in — short ones first, because the common
    /// case is that the tail of this list is all zeroes and need not be sent.
    const ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];

    /// Inflate a zlib stream: the two-byte header, the deflate data, the Adler-32 trailer. Returns the
    /// bytes and how many of `src` were consumed, so a caller with a declared length can check it.
    pub fn zlib(src: &[u8]) -> Result<(Vec<u8>, usize), String> {
        if src.len() < 6 {
            return Err(format!("zlib: {} bytes is not a stream", src.len()));
        }
        let (cmf, flg) = (src[0], src[1]);
        if cmf & 0x0f != 8 {
            return Err(format!("zlib: compression method {}", cmf & 0x0f));
        }
        if (cmf as u16 * 256 + flg as u16) % 31 != 0 {
            return Err("zlib: header check failed".into());
        }
        if flg & 0x20 != 0 {
            return Err("zlib: needs a preset dictionary".into());
        }
        let (out, used) = deflate(&src[2..])?;
        let trailer = src
            .get(2 + used..6 + used)
            .ok_or("zlib: the Adler-32 trailer is missing")?;
        let want = u32::from_be_bytes([trailer[0], trailer[1], trailer[2], trailer[3]]);
        let got = adler32(&out);
        if got != want {
            return Err(format!("zlib: Adler-32 {got:#010x}, expected {want:#010x}"));
        }
        Ok((out, used + 6))
    }

    /// Inflate a bare deflate stream. Returns the bytes and how many of `src` were consumed.
    pub fn deflate(src: &[u8]) -> Result<(Vec<u8>, usize), String> {
        let mut b = Bits {
            d: src,
            at: 0,
            bits: 0,
            held: 0,
        };
        let mut out = Vec::new();
        loop {
            let last = b.take(1)? == 1;
            match b.take(2)? {
                0 => stored(&mut b, &mut out)?,
                1 => {
                    let (lit, dist) = fixed();
                    block(&mut b, &mut out, &lit, &dist)?;
                }
                2 => {
                    let (lit, dist) = dynamic(&mut b)?;
                    block(&mut b, &mut out, &lit, &dist)?;
                }
                _ => return Err("deflate: reserved block type".into()),
            }
            if last {
                return Ok((out, b.at));
            }
        }
    }

    /// Adler-32 of the output, which is how a zlib stream proves it decompressed correctly.
    pub fn adler32(d: &[u8]) -> u32 {
        let (mut a, mut b) = (1u32, 0u32);
        for &byte in d {
            a = (a + byte as u32) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }

    /// A bit reader, least-significant bit first, as deflate numbers its bits.
    struct Bits<'a> {
        d: &'a [u8],
        /// How many bytes have been drawn into `held`; also "bytes consumed" once `bits` is flushed.
        at: usize,
        bits: u32,
        held: u32,
    }

    impl Bits<'_> {
        fn take(&mut self, n: u32) -> Result<u32, String> {
            while self.bits < n {
                let byte = *self.d.get(self.at).ok_or("deflate: ran off the end")?;
                self.held |= (byte as u32) << self.bits;
                self.bits += 8;
                self.at += 1;
            }
            let v = self.held & ((1u32 << n) - 1);
            self.held >>= n;
            self.bits -= n;
            Ok(v)
        }

        /// Drop the partial byte, which is how a stored block starts.
        fn align(&mut self) {
            let whole = self.bits / 8;
            self.held >>= whole * 8;
            self.bits -= whole * 8;
            self.held = 0;
            self.bits = 0;
        }
    }

    /// A canonical Huffman code: how many symbols have each length, and the symbols in length order.
    struct Huff {
        count: [u16; 16],
        symbol: Vec<u16>,
    }

    impl Huff {
        fn new(lengths: &[u8]) -> Result<Huff, String> {
            let mut count = [0u16; 16];
            for &l in lengths {
                count[l as usize] += 1;
            }
            count[0] = 0;
            // An over-subscribed set of lengths is a corrupt stream, and catching it here is what stops
            // `decode` from walking off the end of `symbol`.
            let mut left = 1i32;
            for len in 1..16 {
                left = (left << 1) - count[len] as i32;
                if left < 0 {
                    return Err("deflate: over-subscribed Huffman code".into());
                }
            }
            let mut offs = [0u16; 16];
            for len in 1..15 {
                offs[len + 1] = offs[len] + count[len];
            }
            let mut symbol = vec![0u16; lengths.len()];
            for (sym, &l) in lengths.iter().enumerate() {
                if l != 0 {
                    symbol[offs[l as usize] as usize] = sym as u16;
                    offs[l as usize] += 1;
                }
            }
            Ok(Huff { count, symbol })
        }

        /// Walk the code lengths, one bit at a time: at each length, the codes of that length are
        /// consecutive, so "is this code in this length's range" is one comparison.
        fn decode(&self, b: &mut Bits) -> Result<u16, String> {
            let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
            for len in 1..16 {
                code |= b.take(1)? as i32;
                let count = self.count[len] as i32;
                if code - first < count {
                    return Ok(self.symbol[(index + code - first) as usize]);
                }
                index += count;
                first = (first + count) << 1;
                code <<= 1;
            }
            Err("deflate: a code longer than 15 bits".into())
        }
    }

    fn stored(b: &mut Bits, out: &mut Vec<u8>) -> Result<(), String> {
        b.align();
        let len = b.take(16)? as usize;
        let nlen = b.take(16)? as usize;
        if len != !nlen & 0xffff {
            return Err("deflate: a stored block's length does not match its complement".into());
        }
        let bytes =
            b.d.get(b.at..b.at + len)
                .ok_or("deflate: a stored block runs off the end")?;
        out.extend_from_slice(bytes);
        b.at += len;
        Ok(())
    }

    /// The fixed code tables, which every stream may use without sending them (RFC 1951 §3.2.6).
    fn fixed() -> (Huff, Huff) {
        let mut lengths = [0u8; 288];
        for (sym, l) in lengths.iter_mut().enumerate() {
            *l = match sym {
                0..=143 => 8,
                144..=255 => 9,
                256..=279 => 7,
                _ => 8,
            };
        }
        // Both are built from valid length sets, so neither can fail.
        (Huff::new(&lengths).unwrap(), Huff::new(&[5u8; 30]).unwrap())
    }

    /// The code tables a dynamic block sends ahead of itself, themselves Huffman-coded.
    fn dynamic(b: &mut Bits) -> Result<(Huff, Huff), String> {
        let nlit = b.take(5)? as usize + 257;
        let ndist = b.take(5)? as usize + 1;
        let ncode = b.take(4)? as usize + 4;
        if nlit > 286 || ndist > 30 {
            return Err(format!(
                "deflate: {nlit} literal and {ndist} distance codes"
            ));
        }
        let mut meta = [0u8; 19];
        for k in 0..ncode {
            meta[ORDER[k]] = b.take(3)? as u8;
        }
        let meta = Huff::new(&meta)?;

        let mut lengths = vec![0u8; nlit + ndist];
        let mut k = 0;
        while k < lengths.len() {
            let sym = meta.decode(b)?;
            let (value, times) = match sym {
                0..=15 => (sym as u8, 1),
                // 16 repeats the previous length, 17 and 18 run of zeroes — short and long.
                16 => {
                    if k == 0 {
                        return Err("deflate: a repeat with nothing to repeat".into());
                    }
                    (lengths[k - 1], 3 + b.take(2)? as usize)
                }
                17 => (0, 3 + b.take(3)? as usize),
                18 => (0, 11 + b.take(7)? as usize),
                _ => return Err(format!("deflate: code-length symbol {sym}")),
            };
            if k + times > lengths.len() {
                return Err("deflate: code lengths overrun their count".into());
            }
            lengths[k..k + times].fill(value);
            k += times;
        }
        if lengths[256] == 0 {
            return Err("deflate: no end-of-block code".into());
        }
        Ok((Huff::new(&lengths[..nlit])?, Huff::new(&lengths[nlit..])?))
    }

    fn block(b: &mut Bits, out: &mut Vec<u8>, lit: &Huff, dist: &Huff) -> Result<(), String> {
        loop {
            let sym = lit.decode(b)? as usize;
            match sym {
                0..=255 => out.push(sym as u8),
                256 => return Ok(()),
                _ => {
                    let k = sym - 257;
                    if k >= LEN.len() {
                        return Err(format!("deflate: length symbol {sym}"));
                    }
                    let len = LEN[k] as usize + b.take(LEN_BITS[k] as u32)? as usize;
                    let k = dist.decode(b)? as usize;
                    if k >= DIST.len() {
                        return Err(format!("deflate: distance symbol {k}"));
                    }
                    let back = DIST[k] as usize + b.take(DIST_BITS[k] as u32)? as usize;
                    if back > out.len() {
                        return Err(format!("deflate: a match {back} bytes before the start"));
                    }
                    // Byte at a time on purpose: a match may overlap its own output (that is how
                    // deflate spells a run), so this cannot be a slice copy.
                    let from = out.len() - back;
                    for i in 0..len {
                        out.push(out[from + i]);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stored block is the one deflate stream that can be written by hand, so it is the one that can
    /// test the reader without any game data present.
    #[test]
    fn a_stored_block_round_trips() {
        let body = b"the quick brown fox";
        let mut src = vec![0x78, 0x01, 0x01, 19, 0, !19u8, 0xff];
        src.extend_from_slice(body);
        src.extend_from_slice(&inflate::adler32(body).to_be_bytes());
        let (out, used) = inflate::zlib(&src).expect("a stored block must inflate");
        assert_eq!(out, body);
        assert_eq!(used, src.len());
    }

    #[test]
    fn a_broken_checksum_is_an_error() {
        let body = b"the quick brown fox";
        let mut src = vec![0x78, 0x01, 0x01, 19, 0, !19u8, 0xff];
        src.extend_from_slice(body);
        src.extend_from_slice(&(inflate::adler32(body) ^ 1).to_be_bytes());
        let why = inflate::zlib(&src).expect_err("a bad Adler-32 must be caught");
        assert!(why.contains("Adler-32"), "{why}");
    }

    #[test]
    fn an_empty_header_is_an_error_not_a_panic() {
        assert!(Psr::parse(&[]).is_err());
        assert!(Res::parse(&[]).is_err());
        assert!(Res::parse(&[3, 0, 0, 0]).is_err());
    }
}
