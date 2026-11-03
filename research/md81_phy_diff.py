"""MD81: diff two INI files (a .phy, .dat, .aud) key by key, values compared as f32 lists, strings case- and
space-folded. MX vs. ATV Unleashed text on the left, Untamed's .bpb decoded by openmx-mods' `bpb` on the right.

    python3 -I research/md81_phy_diff.py UNLEASHED.phy UNTAMED.phy
"""
import sys, struct, re, os
def f32(x): return struct.unpack('<f', struct.pack('<f', float(x)))[0]
def parse(p):
    d = {}; order=[]; sec=None
    for line in open(p, encoding='latin-1'):
        line = re.split(r";|//", line, maxsplit=1)[0].strip()
        if not line: continue
        if line.startswith('['):
            sec = line.strip('[]').strip().lower(); d.setdefault(sec, {}); order.append(sec); continue
        if '=' not in line: continue
        k, v = line.split('=', 1)
        d.setdefault(sec, {})[k.strip().lower()] = v.strip()
    return d
def norm(v):
    out=[]
    for t in v.split(','):
        t=t.strip().strip('"')
        try: out.append(f32(t.rstrip('fF') if re.match(r'^-?[\d.]+[fF]$', t) else t))
        except ValueError: out.append(re.sub(r'\s+', '', t.lower()))
    return out
a, b = parse(sys.argv[1]), parse(sys.argv[2])
diffs=[]
for s in sorted(set(a)|set(b), key=lambda s:(s not in a, s)):
    if s not in a: diffs.append(f'+section [{s}] ({len(b[s])} keys)'); continue
    if s not in b: diffs.append(f'-section [{s}] ({len(a[s])} keys)'); continue
    for k in list(a[s]) + [k for k in b[s] if k not in a[s]]:
        if k not in b[s]: diffs.append(f'-[{s}] {k} = {a[s][k]}')
        elif k not in a[s]: diffs.append(f'+[{s}] {k} = {b[s][k]}')
        elif norm(a[s][k]) != norm(b[s][k]): diffs.append(f'~[{s}] {k}: {a[s][k]} -> {b[s][k]}')
nk = sum(len(v) for v in a.values())
print(f'{os.path.basename(sys.argv[1])}: {len(a)} sections {nk} keys; {len(diffs)} differences')
for x in diffs: print('   ', x)
