#!/usr/bin/env python3
"""MX Unleashed Xbox .res: `xbox_res.py FILE ls` | `xbox_res.py FILE x OUTDIR` (the PS2 disc's inner .res is the same container).
Layout: u32 3, u32 n+12, u32, u32, u32 n; (n/2-1) u16 block lengths, u16 0; u32 count, u32;
count x {u32 len, name[len], u32 offset, u32 size}; blocks: zlib[len] + 0xFF, 24576 bytes each inflated."""
import os, struct, sys, zlib

B = 24576
data = open(sys.argv[1], "rb").read()
ver, _, _, _, n = struct.unpack_from("<5I", data, 0)
assert ver == 3, ver
nb = n // 2 - 1
lens = struct.unpack_from(f"<{nb}H", data, 20)
o = 20 + nb * 2 + 2
count, _ = struct.unpack_from("<II", data, o); o += 8
ents = []
for _ in range(count):
    (ln,) = struct.unpack_from("<I", data, o); o += 4
    name = data[o:o + ln].rstrip(b"\0").decode("latin-1"); o += ln
    off, size = struct.unpack_from("<II", data, o); o += 8
    ents.append((name, off, size))
starts = []
for ln in lens:
    starts.append(o); o += ln + 1
cache = {}
def block(i):
    if i not in cache: cache[i] = zlib.decompress(data[starts[i]:starts[i] + lens[i]])
    return cache[i]
def read(off, size):
    out = bytearray()
    while len(out) < size:
        p = off + len(out); b = block(p // B)
        out += b[p % B:p % B + size - len(out)]
    return bytes(out)
if sys.argv[2] == "ls":
    for name, off, size in ents: print(f"{size:>10} {name}")
else:
    for name, off, size in ents:
        d = os.path.join(sys.argv[3], name.replace("\\", "/"))
        os.makedirs(os.path.dirname(d) or ".", exist_ok=True)
        open(d, "wb").write(read(off, size))
