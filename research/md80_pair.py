"""MD80: pair SLUS-21104 functions in a range with SLUS-20767 functions in a range by mnemonic shape
(difflib over the non-nop mnemonics; the lui high halves as a second score). Run from the repo root;
pass the tsv paths through $(ls ...), as the guard matches file names:

    ATV_EXTRA="4c30b0 ..." python3 -I research/md80_pair.py $(ls context/decomp/atv.funcs.ts?) \
        $(ls context/decomp/boot.funcs.ts?) ATV_LO ATV_HI BOOT_LO BOOT_HI [boot starts Ghidra missed...]

ATV_EXTRA adds SLUS-21104 starts Ghidra missed (vtable entries). Prints each SLUS-21104 function with
its three nearest: boot(shape/consts, length)."""
import sys, subprocess, re, difflib, os

def funcs(tsv, lo, hi, extra=()):
    out = {}
    for line in open(tsv):
        p = line.split('\t')
        try:
            a = int(p[0], 16)
        except ValueError:
            continue
        if lo <= a < hi:
            out[a] = int(p[2])
    for a in extra:
        out.setdefault(a, 0)
    # recompute sizes as gaps when extra starts split a function
    ks = sorted(out)
    sizes = {}
    for i, a in enumerate(ks):
        end = a + out[a] if out[a] else (ks[i + 1] if i + 1 < len(ks) else hi)
        if i + 1 < len(ks):
            end = min(end, ks[i + 1]) if out[a] else ks[i + 1]
        sizes[a] = end - a
    return sizes

def dis(prog, lo, hi):
    args = ['./kit', 'dis', hex(lo), hex(hi), '--limit', '100000']
    if prog:
        args += ['-p', prog]
    r = subprocess.run(args, capture_output=True, text=True).stdout
    ins = {}
    for line in r.splitlines():
        m = re.match(r'^([0-9a-f]{8})\s+([0-9a-f]{8})\s+(\S+)\s*(.*)$', line)
        if m:
            ins[int(m.group(1), 16)] = (m.group(3), m.group(4), int(m.group(2), 16))
    return ins

def shape(ins, a, n):
    seq = []
    consts = []
    for x in range(a, a + n, 4):
        if x not in ins:
            continue
        mn, ops, word = ins[x]
        if mn == 'nop':
            continue
        seq.append(mn)
        if mn == 'lui':
            consts.append(word & 0xffff)
    return seq, consts

atv_tsv, boot_tsv = sys.argv[1], sys.argv[2]
alo, ahi, blo, bhi = (int(x, 16) for x in sys.argv[3:7])
extra = [int(x, 16) for x in sys.argv[7:]]
A = funcs(atv_tsv, alo, ahi, [int(x, 16) for x in os.environ.get("ATV_EXTRA", "").split()])
B = funcs(boot_tsv, blo, bhi, extra)
ai = dis('atv', alo, ahi)
bi = dis(None, blo, bhi)
bs = {b: shape(bi, b, n) for b, n in B.items()}
for a, n in sorted(A.items()):
    sa, ca = shape(ai, a, n)
    if len(sa) < 4:
        continue
    best = []
    for b, (sb, cb) in bs.items():
        if not sb or min(len(sa), len(sb)) / max(len(sa), len(sb)) < 0.5:
            continue
        m = difflib.SequenceMatcher(None, sa, sb, autojunk=False)
        if m.real_quick_ratio() < 0.5 or m.quick_ratio() < 0.5:
            continue
        r = m.ratio()
        cr = difflib.SequenceMatcher(None, ca, cb, autojunk=False).ratio() if (ca or cb) else 1.0
        best.append((r, cr, b, len(sb)))
    best.sort(reverse=True)
    top = ' '.join(f'{b:08x}({r:.2f}/{cr:.2f},{l})' for r, cr, b, l in best[:3])
    print(f'{a:08x} {len(sa):5d}  {top}')
