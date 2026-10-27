"""C12: which BIOS a .p2s was made on — its EE kernel (low RAM, copied from ROM at boot) and the IOP's
ROM strings against other states'. A .p2s holds RAM, not the ROM, so a load runs the local BIOS.

    python3 -B research/c12/bios.py          # the 2p state against the kit's and animandpump's

Research, not product.
"""
import os
import re
import zipfile

R = os.path.join('context', 'recordings')
STATES = {'2p': os.path.join(R, 'split', '2p.p2m2_SaveState.p2s'),
          'kit first_recording': os.path.join(R, 'first_recording.p2m2_SaveState.p2s'),
          'animandpump (v2.9.52)': os.path.join(R, 'animandpump.p2m2_SaveState.p2s')}
ee = {k: zipfile.ZipFile(p).read('eeMemory.bin') for k, p in STATES.items()}
iop = {k: zipfile.ZipFile(p).read('iopMemory.bin') for k, p in STATES.items()}
for k in STATES:
    if k == '2p':
        continue
    for lo, hi in ((0, 0x10000), (0x10000, 0x40000), (0x40000, 0x80000)):
        d = sum(1 for i in range(lo, hi, 4) if ee['2p'][i:i + 4] != ee[k][i:i + 4])
        print('EE %06x..%06x  2p vs %-22s %6d of %d words differ' % (lo, hi, k, d, (hi - lo) // 4))
for k, m in iop.items():
    s = sorted(set(x.group().decode() for x in re.finditer(rb'(?:ROMVER|PS2 ?BIOS|rom0:[A-Z0-9]+|\d{4}[A-Z]\d{8})[ -~]{0,16}', m)))
    print(k, s[:10])
