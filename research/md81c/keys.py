#!/usr/bin/env python3
"""MD81c: the key strings a .phy loader hands its reader, in order, for two builds' twin loaders.

Reads `./kit dis` listings and resolves each `lui $5`/`addiu $5, $5` pair to a string in the ELF.

    python3 -I research/md81c/keys.py ATV_ELF ATV.s UNT_ELF UNT.s
"""
import os
import re
import sys
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'md81a'))
import twins  # noqa: E402

LINE = re.compile(r'^([0-9a-f]{8})\s+[0-9a-f]{8}\s+(\S+)\s*(.*?)\s*(;.*)?$')


def cstr(segs, a):
    for va, b in segs:
        if va <= a < va + len(b):
            e = b.index(b'\0', a - va)
            return b[a - va:e].decode('latin-1')
    return f'?{a:#x}'


def keys(elf, path):
    segs, hi, out = twins.load(elf), None, []
    for raw in open(path):
        m = LINE.match(raw.rstrip('\n'))
        if not m:
            continue
        op, args = m.group(2), [x.strip() for x in m.group(3).split(',')]
        if op == 'lui' and args[0] == '$5':
            hi = int(args[1], 0)
        elif op == 'addiu' and args[:2] == ['$5', '$5'] and hi is not None:
            out.append(cstr(segs, ((hi << 16) + int(args[2], 0)) & 0xffffffff))
            hi = None
    return out


a, u = keys(sys.argv[1], sys.argv[2]), keys(sys.argv[3], sys.argv[4])
import difflib
print(f'{len(a)} keys -> {len(u)} keys')
for line in difflib.unified_diff(a, u, 'atv', 'untamed', lineterm='', n=1):
    print(line)
