#!/usr/bin/env python3
"""List or extract the PC (Steam) MX vs. ATV Unleashed .pak files.

Layout: u32 count, then count entries of {char name[100]; u32 offset; u32 size}
(offset from the end of the table), then the data. Names are "Data\\..." paths.

    tools/mxvatv_pak.py ls context/mxvatv-pc/data/common.pak
    tools/mxvatv_pak.py x  context/mxvatv-pc/data/common.pak 'Veh/*.phy' context/mxvatv-pc-x/

Extract into context/ only: it is game data and never committed.
"""
import fnmatch, os, struct, sys

ENTRY = 108


def toc(path):
    with open(path, 'rb') as f:
        (n,) = struct.unpack('<I', f.read(4))
        table = f.read(n * ENTRY)
    base = 4 + n * ENTRY
    out = []
    for k in range(n):
        e = table[k * ENTRY:(k + 1) * ENTRY]
        name = e[:ENTRY - 8].split(b'\0')[0].decode('latin1')
        off, size = struct.unpack_from('<II', e, ENTRY - 8)
        out.append((name, base + off, size))
    return out


def extract(path, pattern, dest):
    n = 0
    with open(path, 'rb') as f:
        for name, off, size in toc(path):
            rel = name.replace('\\', '/').removeprefix('Data/')
            if fnmatch.fnmatch(rel.lower(), pattern.lower()):
                p = os.path.join(dest, rel)
                os.makedirs(os.path.dirname(p), exist_ok=True)
                f.seek(off)
                with open(p, 'wb') as o:
                    o.write(f.read(size))
                n += 1
    return n


def check(path):
    # the table's last member ends exactly at the end of the file
    t = toc(path)
    last = max(t, key=lambda e: e[1])
    assert last[1] + last[2] == os.path.getsize(path), path


if __name__ == '__main__':
    cmd, path, *rest = sys.argv[1:]
    if cmd == 'ls':
        check(path)
        for name, off, size in toc(path):
            print(f'{size}\t{name}')
    elif cmd == 'x':
        print(extract(path, rest[0], rest[1]), 'files')
    elif cmd == 'check':
        for p in [path, *rest]:
            check(p)
        print('ok')
