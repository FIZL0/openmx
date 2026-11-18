"""C42: `poke.py laps N` sets the race's lap count (score manager +0x94, C31) and prints the points;
`poke.py read` prints player 0's profile points (+0xc0), the frame and every card's bank.
Run from the repo root, inside `./kit pcsx2 with`."""
import struct, sys
sys.path.insert(0, '.')
from ps2kit.pine import Pine

APP, MGR, FRAME, IN_GAME, CARDS = 0x00479990, 0x004e1df8, 0x00479d04, 0x004e2650, 0x8fc760
p = Pine(check=False, lock=False, timeout=5.0)
f = lambda a: struct.unpack('<f', struct.pack('<I', p.read32(a)))[0]
prof = p.read32(APP + 0x3f4)
if sys.argv[1] == 'laps':
    m = p.read32(MGR)
    mgr = p.read32(p.read32(m + 0x58)) if m else 0
    if not mgr:
        raise SystemExit('no score manager')
    print('laps', p.read32(mgr + 0xe4), '->', sys.argv[2])
    p.write32(mgr + 0xe4, int(sys.argv[2]))
print('frame', p.read32(FRAME), 'in_game', p.read32(IN_GAME), 'profile', hex(prof),
      'points', p.read32(prof + 0xc0) if prof else None,
      'banks', [f(CARDS + i * 0x1e0 + 0xdc) for i in range(4)], flush=True)
