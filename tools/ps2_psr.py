#!/usr/bin/env python3
"""ps2_psr.py ISO NAME OUT: copy one MX.PSR member (e.g. Tracks\\free06.res) out of the PS2 ISO; NAME "ls" lists them."""
import struct, sys
iso, want = sys.argv[1:3]; out = sys.argv[3] if len(sys.argv) > 3 else None
f = open(iso, "rb")
def rd(o, n): f.seek(o); return f.read(n)
pvd = rd(16 * 2048, 2048); root = pvd[156:190]
lba, size = struct.unpack_from("<I", root, 2)[0], struct.unpack_from("<I", root, 10)[0]
d = rd(lba * 2048, size); o = 0; psr = None
while o < len(d):
    ln = d[o]
    if ln == 0: o = (o // 2048 + 1) * 2048; continue
    nm = d[o + 33:o + 33 + d[o + 32]].decode(); 
    if nm.startswith("MX.PSR"): psr = struct.unpack_from("<I", d, o + 2)[0] * 2048
    o += ln
front = rd(psr, 1 << 20); n = struct.unpack_from("<I", front, 4)[0]; at = 0x18
for _ in range(n):
    ln = struct.unpack_from("<I", front, at)[0]; name = front[at+4:at+4+ln].rstrip(b"\0").decode("latin-1")
    sz, sec = struct.unpack_from("<II", front, at + 4 + ln); at += 12 + ln
    if want == "ls": print(sz, name)
    elif name.lower() == want.lower(): open(out, "wb").write(rd(psr + sec * 2048, sz)); print("ok", sz)
