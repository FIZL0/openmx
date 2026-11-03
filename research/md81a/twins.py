#!/usr/bin/env python3
"""MD81a: find each SLUS-21104 function's twin in SLUS-21701 and say whether its code is the same.

Two functions are the same when their instructions are equal word for word once what moves with the
link is masked: jal/j targets, lui immediates and the low halves paired with them, and gp-relative
offsets. Branch offsets are relative and stay compared, as do every other immediate (field offsets,
float constants built with lui are masked, so a float constant read through lui is reported as a lui
difference only when the instruction stream differs elsewhere -- see --consts).

    python3 -I research/md81a/twins.py ATV_ELF ATV_FUNCS UNTAMED_ELF UNTAMED_FUNCS ADDR...

ADDR are SLUS-21104 function addresses (hex). Prints, per address, the twin (or the nearest), and with
--consts the lui/ori/addiu float constants of both so a retuned literal shows.
"""
import struct
import sys
import difflib


def load(path):
    d = open(path, 'rb').read()
    phoff, = struct.unpack_from('<I', d, 0x1c)
    n, = struct.unpack_from('<H', d, 0x2c)
    segs = []
    for i in range(n):
        t, off, va, pa, fs, ms, fl, al = struct.unpack_from('<8I', d, phoff + 32 * i)
        if t == 1 and fs:
            segs.append((va, d[off:off + fs]))
    return segs


def words(segs, addr, size):
    for va, b in segs:
        if va <= addr < va + len(b):
            o = addr - va
            return list(struct.unpack_from(f'<{size // 4}I', b, o))
    return []


def funcs(path):
    out = []
    for line in open(path):
        p = line.rstrip('\n').split('\t')
        if len(p) >= 3 and p[2].isdigit():
            out.append((int(p[0], 16), p[1], int(p[2])))
    return out


MEM = set(range(0x20, 0x40)) | {0x1a, 0x1b, 0x1e, 0x1f}


def norm(ws, mask_mem=False):
    """The instruction words with what the link moves masked, and the masked literals beside. With
    mask_mem, every load/store offset and every addiu off a non-sp register too (the object's fields
    moved between the builds: the pairs are listed by offsets())."""
    out, lits = [], []
    hi = {}
    for i, w in enumerate(ws):
        op = w >> 26
        rs = (w >> 21) & 31
        rt = (w >> 16) & 31
        if op in (2, 3):  # j, jal
            out.append(w & 0xfc000000)
            continue
        if op == 0x0f:  # lui
            hi[rt] = (i, w & 0xffff)
            out.append(w & 0xffff0000)
            lits.append(('lui', w & 0xffff))
            continue
        if op not in (0, 1, 0x10, 0x11, 0x12, 0x13, 0x1c) and rs == 28:  # gp-relative
            out.append(w & 0xffff0000)
            continue
        if op not in (0, 1, 4, 5, 6, 7, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x1c) and rs in hi and i - hi[rs][0] < 12:
            # the low half of a lui pair (an address, or a float's low bits)
            if op == 0x0d:
                lits.append(('ori', (hi[rs][1] << 16) | (w & 0xffff)))
            out.append(w & 0xffff0000)
            continue
        if op == 0 and (w & 0x3f) in (0x21, 0x2d, 0x25):
            pass
        if mask_mem and (op in MEM or (op == 0x09 and rs not in (29, 0))):
            out.append(w & 0xffff0000)
            continue
        out.append(w)
        # a write to a register ends its lui pairing
        if op == 0:
            hi.pop((w >> 11) & 31, None)
        elif op not in (4, 5, 6, 7, 0x14, 0x15, 0x16, 0x17, 0x2b, 0x29, 0x28, 0x39, 0x3f, 0x1f):
            hi.pop(rt, None)
    return out, lits


def offsets(aw, uw):
    """(atv offset, untamed offset, base reg) for every load/store the two equal-masked runs share."""
    out = []
    for a, u in zip(aw, uw):
        op = a >> 26
        if (op in MEM or op == 0x09) and (a & 0xffff) != (u & 0xffff):
            out.append(((a >> 21) & 31, a & 0xffff, u & 0xffff))
    return out


def pair(a_segs, u_segs, a, asz, u, usz):
    aw, uw = words(a_segs, a, asz), words(u_segs, u, usz)
    an, _ = norm(aw, True)
    un, _ = norm(uw, True)
    sm = difflib.SequenceMatcher(None, an, un, autojunk=False)
    moved = {}
    rest = []
    for tag, i1, i2, j1, j2 in sm.get_opcodes():
        if tag == 'equal':
            for reg, x, y in offsets(aw[i1:i2], uw[j1:j2]):
                moved.setdefault((x, y), 0)
                moved[(x, y)] += 1
        else:
            rest.append((tag, a + 4 * i1, a + 4 * i2, u + 4 * j1, u + 4 * j2))
    return sm.ratio(), moved, rest


def main():
    if '--pair' in sys.argv:
        args = [a for a in sys.argv[1:] if not a.startswith('--')]
        a_segs, u_segs = load(args[0]), load(args[2])
        a_f = {x: s for x, n, s in funcs(args[1])}
        u_f = {x: s for x, n, s in funcs(args[3])}
        for spec in args[4:]:
            x, y = (int(v, 16) for v in spec.split(':'))
            r, moved, rest = pair(a_segs, u_segs, x, a_f[x], y, u_f[y])
            mv = ' '.join(f'{p:#x}->{q:#x}' for (p, q) in sorted(moved))
            print(f'{x:08x}:{y:08x} {a_f[x]}/{u_f[y]} B  masked ratio {r:.3f}  moved {mv or "none"}')
            for tag, i1, i2, j1, j2 in rest:
                print(f'   {tag:8s} atv {i1:08x}..{i2:08x}  unt {j1:08x}..{j2:08x}')
        return

    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    consts = '--consts' in sys.argv
    loose = '--loose' in sys.argv
    atv_elf, atv_funcs, unt_elf, unt_funcs = args[:4]
    want = [int(a, 16) for a in args[4:]]
    a_segs, u_segs = load(atv_elf), load(unt_elf)
    a_f = {a: (n, s) for a, n, s in funcs(atv_funcs)}
    u_f = funcs(unt_funcs)
    u_norm = {}
    by_hash = {}
    for a, n, s in u_f:
        if s < 4:
            continue
        nw, lits = norm(words(u_segs, a, s), loose)
        u_norm[a] = (nw, lits, s)
        by_hash.setdefault(tuple(nw), []).append(a)
    for a in want:
        name, size = a_f.get(a, ('?', 0))
        nw, lits = norm(words(a_segs, a, size), loose)
        hits = by_hash.get(tuple(nw), [])
        if hits:
            ul = [u_norm[h][1] for h in hits]
            same_lits = [h for h, l in zip(hits, ul) if l == lits]
            print(f'{a:08x} {size:5d}  SAME code as untamed {" ".join(f"{h:08x}" for h in hits)}'
                  f'  literals {"equal" if same_lits else "DIFFER"}')
            if consts and not same_lits:
                print('   atv ', lits)
                print('   unt ', ul[0])
            continue
        best = None
        for ua, (unw, ulits, us) in u_norm.items():
            if not (0.7 * size <= us <= 1.3 * size):
                continue
            r = difflib.SequenceMatcher(None, nw, unw, autojunk=False).quick_ratio()
            if best and r <= best[0]:
                continue
            r = difflib.SequenceMatcher(None, nw, unw, autojunk=False).ratio()
            if not best or r > best[0]:
                best = (r, ua, us)
        if best:
            print(f'{a:08x} {size:5d}  DIFFERENT; nearest untamed {best[1]:08x} ({best[2]} B), ratio {best[0]:.3f}')
        else:
            print(f'{a:08x} {size:5d}  no candidate')


if __name__ == '__main__':
    main()
