"""MD80: python3 -I research/md80_hist.py ATV_START ATV_END BOOT_START BOOT_END (hex, end exclusive), from the
repo root: the mnemonic counts that differ between a SLUS-21104 function and its SLUS-20767 twin, and the
float constants (lui high halves) only one side loads."""
import sys, subprocess, re, collections, struct

def dis(prog, lo, hi):
    args = ['./kit', 'dis', hex(lo), hex(hi), '--limit', '100000'] + (['-p', prog] if prog else [])
    out = []
    for line in subprocess.run(args, capture_output=True, text=True).stdout.splitlines():
        m = re.match(r'^([0-9a-f]{8})\s+([0-9a-f]{8})\s+(\S+)\s*(.*)$', line)
        if m:
            out.append((m.group(3), int(m.group(2), 16)))
    return out

def consts(ins):
    c = collections.Counter()
    for mn, w in ins:
        if mn == 'lui':
            v = struct.unpack('<f', struct.pack('<I', (w & 0xffff) << 16))[0]
            if 1e-6 < abs(v) < 1e7:
                c[f'{v:g}'] += 1
    return c

a = dis('atv', int(sys.argv[1], 16), int(sys.argv[2], 16))
b = dis(None, int(sys.argv[3], 16), int(sys.argv[4], 16))
ha = collections.Counter(m for m, _ in a if m != 'nop')
hb = collections.Counter(m for m, _ in b if m != 'nop')
print('atv', sum(ha.values()), 'boot', sum(hb.values()))
print('mnemonics (atv-boot):', ' '.join(f'{k}{ha[k]-hb[k]:+d}' for k in sorted(set(ha) | set(hb)) if ha[k] != hb[k]))
ca, cb = consts(a), consts(b)
print('consts only atv:', dict(ca - cb), ' only boot:', dict(cb - ca))
