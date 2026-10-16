"""A PNG reader and writer in the standard library, for comparing two frames.

The port writes PNGs and PCSX2 writes PNGs, and the only thing this project ever needs to do with a
pair of them is put them next to each other and look. That is small enough to keep here rather than
take a dependency for, and it means `side_by_side.py` runs on a bare Python.

Reads 8-bit greyscale, RGB and RGBA, interlace 0 — which is every PNG either side of this comparison
writes. Writes 8-bit RGB.
"""

import struct
import zlib


def read(path):
    """-> (width, height, rows) where each row is a bytearray of RGB triples."""
    data = open(path, "rb").read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"{path}: not a PNG")
    width = height = depth = color = None
    idat = bytearray()
    palette = None
    at = 8
    while at < len(data):
        (length,) = struct.unpack_from(">I", data, at)
        kind = data[at + 4 : at + 8]
        body = data[at + 8 : at + 8 + length]
        at += 12 + length
        if kind == b"IHDR":
            width, height, depth, color, _, _, interlace = struct.unpack(">IIBBBBB", body)
            if depth != 8 or interlace != 0:
                raise ValueError(f"{path}: {depth}-bit, interlace {interlace}: not supported")
            if color not in (0, 2, 3, 6):
                raise ValueError(f"{path}: colour type {color}: not supported")
        elif kind == b"PLTE":
            palette = body
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break
    raw = zlib.decompress(bytes(idat))
    channels = {0: 1, 2: 3, 3: 1, 6: 4}[color]
    stride = width * channels
    rows = []
    previous = bytearray(stride)
    at = 0
    for _ in range(height):
        filt = raw[at]
        line = bytearray(raw[at + 1 : at + 1 + stride])
        at += 1 + stride
        _unfilter(filt, line, previous, channels)
        rows.append(_to_rgb(line, channels, color, palette))
        previous = line
    return width, height, rows


def _unfilter(filt, line, previous, channels):
    if filt == 0:
        return
    for i in range(len(line)):
        a = line[i - channels] if i >= channels else 0
        b = previous[i]
        c = previous[i - channels] if i >= channels else 0
        if filt == 1:
            line[i] = (line[i] + a) & 0xFF
        elif filt == 2:
            line[i] = (line[i] + b) & 0xFF
        elif filt == 3:
            line[i] = (line[i] + (a + b) // 2) & 0xFF
        elif filt == 4:
            p = a + b - c
            pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
            nearest = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
            line[i] = (line[i] + nearest) & 0xFF
        else:
            raise ValueError(f"filter {filt}")


def _to_rgb(line, channels, color, palette):
    if color == 2:
        return bytearray(line)
    out = bytearray()
    if color == 0:
        for v in line:
            out += bytes((v, v, v))
    elif color == 3:
        for v in line:
            out += palette[3 * v : 3 * v + 3]
    else:  # RGBA, over nothing: the alpha of a screenshot is not a difference anybody means
        for i in range(0, len(line), 4):
            out += line[i : i + 3]
    return out


def write(path, width, height, rows):
    """`rows` as `read` returns them: one bytearray of RGB triples each."""
    raw = bytearray()
    for row in rows:
        raw.append(0)
        raw += row
    chunks = [
        _chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)),
        _chunk(b"IDAT", zlib.compress(bytes(raw), 9)),
        _chunk(b"IEND", b""),
    ]
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n")
        for c in chunks:
            f.write(c)


def _chunk(kind, body):
    return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))


def scale(width, height, rows, factor):
    """Box-average down by a whole factor, which is all a frame ever needs to be looked at."""
    if factor == 1:
        return width, height, rows
    w, h = width // factor, height // factor
    out = []
    for y in range(h):
        row = bytearray(w * 3)
        for x in range(w):
            for c in range(3):
                total = 0
                for dy in range(factor):
                    line = rows[y * factor + dy]
                    for dx in range(factor):
                        total += line[(x * factor + dx) * 3 + c]
                row[x * 3 + c] = total // (factor * factor)
        out.append(row)
    return w, h, out
