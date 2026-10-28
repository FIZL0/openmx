"""V70: copy a .p2s over copy 2's slot 8, load it, and say which actor-list hop holds the human's
rider (type 2) and where its bike stands.

usage: python3 -B research/v70/load.py P2S
"""
import shutil, subprocess, sys, time
sys.path.insert(0, '.')
from ps2kit.config import cfg
from ps2kit.pine import Pine

SLOT8 = '/home/rycs/repos/openmx-slots/s2-slots/pcsx2/s2/PCSX2/savestates/SLUS-20767 (9866FE54).08.p2s'
shutil.copy(sys.argv[1], SLOT8)
subprocess.run(['./kit', 'pine', 'load', '8'], check=True)
time.sleep(2.0)
p = Pine()
GM = cfg().symbols['game_manager']
for hops in ((0x8, 0x0), (0x8, 0x8, 0x0), (0x8, 0x8, 0x8, 0x0)):
    a = p.read32(p.read32(p.read32(GM) + 0x34) + 0x34)
    for o in hops:
        a = p.read32(a + o)
    t = p.read32(a)
    print(hops, hex(a), 'type', t, 'pos', [round(p.read_f32(a + 0x8d0 + 4 * k), 1) for k in range(3)],
          'zones', p.read8(a + 0x910), p.read8(a + 0x911))
