#!/usr/bin/env python3
"""XDVDFS (Xbox disc) reader: `xbox_xiso.py ISO ls` lists files, `xbox_xiso.py ISO x OUTDIR [prefix]` extracts. Pass paths, not globs."""
import os, struct, sys

S = 2048
iso, cmd = sys.argv[1], sys.argv[2]
f = open(iso, "rb")

def rd(off, n):
    f.seek(off); return f.read(n)

base = next(b for b in (0, 0x0FD90000, 0x02080000, 0x18300000)
            if rd(b + 0x10000, 20) == b"MICROSOFT*XBOX*MEDIA")
root, rsize = struct.unpack("<II", rd(base + 0x10000 + 20, 8))

def walk(sector, size, path):
    table = rd(base + sector * S, size)
    seen, stack = set(), [0]
    while stack:
        o = stack.pop()
        if o in seen or o + 14 > len(table): continue
        seen.add(o)
        l, r, st, sz, attr, nl = struct.unpack("<HHIIBB", table[o:o + 14])
        if l == 0xFFFF and r == 0xFFFF: continue  # padding
        name = table[o + 14:o + 14 + nl].decode("latin-1")
        p = f"{path}/{name}" if path else name
        if attr & 0x10:
            if sz: yield from walk(st, sz, p)
        else:
            yield p, st, sz
        if l: stack.append(l * 4)
        if r: stack.append(r * 4)

files = sorted(walk(root, rsize, ""))
if cmd == "ls":
    for p, st, sz in files: print(f"{sz:>11} {p}")
else:
    out, pre = sys.argv[3], (sys.argv[4] if len(sys.argv) > 4 else "")
    for p, st, sz in files:
        if not p.lower().startswith(pre.lower()): continue
        d = os.path.join(out, p); os.makedirs(os.path.dirname(d), exist_ok=True)
        with open(d, "wb") as w:
            f.seek(base + st * S); left = sz
            while left:
                b = f.read(min(left, 1 << 20)); w.write(b); left -= len(b)
