#!/usr/bin/env python3
"""Decode PS2 .pst and Xbox .dxt textures to PNG and lay each pair side by side.
xbox_textures.py PS2_DIR XBOX_DIR OUT_DIR  (dirs hold the unpacked .res trees; pairs matched by name)."""
import os, struct, sys
import numpy as np
from PIL import Image, ImageDraw

def u32(b, o): return struct.unpack_from("<I", b, o)[0]

# ---- PS2 .pst (after crates/game-render/src/pst.rs) ----
def pst(b):
    if u32(b, 4) == 0x2E:                         # 16 colours, two pixels a byte, low nibble first
        pal = np.frombuffer(b, np.uint8, 64, 8).reshape(16, 4).copy(); mips_at, bpp = 72, 4
    else:                                         # 256 colours, GS palette order
        raw = np.frombuffer(b, np.uint8, 1024, 8).reshape(256, 4)
        idx = [(i & 0xE7) | ((i & 8) << 1) | ((i & 16) >> 1) for i in range(256)]
        pal = raw[idx].copy(); mips_at, bpp = 1032, 8
    pal[:, 3] = np.minimum(pal[:, 3].astype(int) * 2, 255)
    n, o = u32(b, mips_at), mips_at + 4
    for _ in range(n):                            # smallest first: keep the last
        h, w, c, _z = struct.unpack_from("<4I", b, o); px = b[o + 16:o + 16 + c]; o += 16 + c
    a = np.frombuffer(px, np.uint8)
    if bpp == 4: a = np.stack([a & 15, a >> 4], 1).ravel()
    return pal[a[:w * h]].reshape(h, w, 4)

# ---- Xbox .dxt ----
def unswizzle(a, w, h):                           # Xbox Morton order -> linear
    # index of each linear (x, y) into the swizzled buffer: x and y bits interleaved, x first
    sx = np.zeros(w, np.int64); sy = np.zeros(h, np.int64)
    bit = 1; xb = yb = 1
    while xb < w or yb < h:
        if xb < w: sx += np.where(np.arange(w) & xb, bit, 0); bit <<= 1; xb <<= 1
        if yb < h: sy += np.where(np.arange(h) & yb, bit, 0); bit <<= 1; yb <<= 1
    return a[(sy[:, None] + sx[None, :]).ravel()].reshape(h, w, *a.shape[1:])

def rgb565(c):
    return np.stack([((c >> 11) & 31) * 255 // 31, ((c >> 5) & 63) * 255 // 63, (c & 31) * 255 // 31], -1)

def dxt_blocks(px, w, h, alpha5):
    bs = 16 if alpha5 else 8
    blk = np.frombuffer(px, np.uint8)[: (w // 4 or 1) * (h // 4 or 1) * bs].reshape(-1, bs)
    out = np.zeros((max(h, 4), max(w, 4), 4), np.uint8); bw = max(w // 4, 1)
    for i, k in enumerate(blk):
        c = k[bs - 8:]
        c0, c1 = struct.unpack_from("<HH", c, 0); bits = u32(c.tobytes(), 4)
        p0, p1 = rgb565(np.array(c0)), rgb565(np.array(c1))
        if c0 > c1 or alpha5:
            cols = [p0, p1, (2 * p0 + p1) // 3, (p0 + 2 * p1) // 3]; al = [255] * 4
        else:
            cols = [p0, p1, (p0 + p1) // 2, np.zeros(3, int)]; al = [255, 255, 255, 0]
        tile = np.zeros((4, 4, 4), np.uint8)
        for t in range(16):
            s = (bits >> (2 * t)) & 3; tile[t // 4, t % 4, :3] = cols[s]; tile[t // 4, t % 4, 3] = al[s]
        if alpha5:
            a0, a1 = int(k[0]), int(k[1]); ab = int.from_bytes(k[2:8].tobytes(), "little")
            av = [a0, a1] + ([((6 - j) * a0 + (j + 1) * a1) // 7 for j in range(6)] if a0 > a1
                             else [((4 - j) * a0 + (j + 1) * a1) // 5 for j in range(4)] + [0, 255])
            for t in range(16): tile[t // 4, t % 4, 3] = av[(ab >> (3 * t)) & 7]
        y, x = divmod(i, bw); out[y * 4:y * 4 + 4, x * 4:x * 4 + 4] = tile
    return out[:h, :w]

def dxt(b):
    fmt = u32(b, 4)
    if fmt in (0x37, 0x38):                       # 8-bit palette, BGRA entries
        npal = u32(b, 8); pal = np.frombuffer(b, np.uint8, npal * 4, 12).reshape(-1, 4)[:, [2, 1, 0, 3]].copy()
        if fmt == 0x37: pal[:, 3] = 255
        o = 12 + npal * 4
    elif fmt in (0x26, 0x36):
        o = 8 + 4
    else:
        raise ValueError(f"fmt {fmt:#x}")
    n = u32(b, o); o += 12                        # mip count, W, H
    for _ in range(n):
        w, h, c = struct.unpack_from("<3I", b, o); px = b[o + 12:o + 12 + c]; o += 12 + c
    if fmt in (0x37, 0x38):
        a = np.frombuffer(px, np.uint8)[:w * h]
        return pal[unswizzle(a, w, h)]
    return dxt_blocks(px, w, h, alpha5=(fmt == 0x36))

def main(pd, xd, out):
    os.makedirs(out, exist_ok=True); rows = []
    for root, _, fs in os.walk(pd):
        for f in sorted(fs):
            if not f.lower().endswith(".pst"): continue
            rel = os.path.relpath(os.path.join(root, f), pd)
            xp = os.path.join(xd, rel[:-4] + ".dxt")
            if not os.path.exists(xp): continue
            try:
                a = pst(open(os.path.join(root, f), "rb").read()); b = dxt(open(xp, "rb").read())
            except Exception as e:
                print("skip", rel, e); continue
            name = rel[:-4].replace(os.sep, "__")
            A, B = Image.fromarray(a, "RGBA"), Image.fromarray(b, "RGBA")
            A.save(f"{out}/{name}.ps2.png"); B.save(f"{out}/{name}.xbox.png")
            H = max(A.height, B.height) + 20
            s = Image.new("RGBA", (A.width + B.width + 10, H), (40, 40, 40, 255))
            s.paste(A, (0, 20)); s.paste(B, (A.width + 10, 20))
            d = ImageDraw.Draw(s); d.text((2, 3), f"PS2 {A.width}x{A.height}", fill="white")
            d.text((A.width + 12, 3), f"Xbox {B.width}x{B.height}", fill="white")
            os.makedirs(f"{out}/side_by_side", exist_ok=True); s.save(f"{out}/side_by_side/{name}.png")
            rows.append(f"{name}\tPS2 {A.width}x{A.height}\tXbox {B.width}x{B.height}")
    open(f"{out}/sizes.tsv", "w").write("\n".join(rows) + "\n"); print(len(rows), "pairs")

if __name__ == "__main__":
    main(*sys.argv[1:4])
