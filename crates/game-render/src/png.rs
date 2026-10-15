//! A PNG writer, so that a frame can be looked at.
//!
//! The workspace has no third-party dependencies, and a PNG does not need one: the format is four
//! chunks, and the compression inside it is allowed to compress nothing. Every row is stored with
//! filter 0 and the whole thing goes out in zlib *stored* blocks, which makes the file about as big as
//! the pixels — fine for a screenshot nobody commits.

/// Encode 8-bit RGB pixels, `width * height * 3` of them.
pub fn encode_rgb(width: usize, height: usize, rgb: &[u8]) -> Vec<u8> {
    assert_eq!(rgb.len(), width * height * 3, "pixels do not match the size");
    let mut raw = Vec::with_capacity(height * (1 + width * 3));
    for y in 0..height {
        raw.push(0); // filter: none
        raw.extend_from_slice(&rgb[y * width * 3..(y + 1) * width * 3]);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8 bits, truecolour, no interlace
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Write a frame to a file.
pub fn write_rgb(path: &std::path::Path, width: usize, height: usize, rgb: &[u8]) -> Result<(), String> {
    std::fs::write(path, encode_rgb(width, height, rgb)).map_err(|e| format!("{}: {e}", path.display()))
}

fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(tag);
    out.extend_from_slice(data);
    let mut crc = crc32(0xffff_ffff, tag);
    crc = crc32(crc, data);
    out.extend_from_slice(&(crc ^ 0xffff_ffff).to_be_bytes());
}

/// A zlib stream of stored (uncompressed) deflate blocks.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut at = 0;
    loop {
        let n = (data.len() - at).min(0xffff);
        let last = at + n == data.len();
        out.push(if last { 1 } else { 0 });
        out.extend_from_slice(&(n as u16).to_le_bytes());
        out.extend_from_slice(&(!(n as u16)).to_le_bytes());
        out.extend_from_slice(&data[at..at + n]);
        at += n;
        if last {
            break;
        }
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn adler32(d: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in d {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn crc32(start: u32, d: &[u8]) -> u32 {
    let mut c = start;
    for &x in d {
        c ^= x as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_image_comes_out_as_a_png() {
        let png = encode_rgb(2, 2, &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
        // The IHDR's own CRC, which is what catches a mistake in the chunk writer.
        let crc = crc32(crc32(0xffff_ffff, b"IHDR"), &png[16..16 + 13]) ^ 0xffff_ffff;
        assert_eq!(png[29..33], crc.to_be_bytes());
    }

    #[test]
    fn the_checksums_match_the_ones_zlib_computes() {
        assert_eq!(adler32(b"abc"), 0x024d0127);
        assert_eq!(crc32(0xffff_ffff, b"123456789") ^ 0xffff_ffff, 0xcbf43926);
    }
}
