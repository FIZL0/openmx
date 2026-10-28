"""S129: load a state slot, let the game run until the train's clock passes each of the given values,
and take `./kit shot` there. usage: shot_at.py SLOT OUTDIR CLOCK [CLOCK ...] (under `./kit pcsx2 with --`)"""
import struct
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT))
from ps2kit.pine import Pine             # noqa: E402

ENGINE = 0x00d525f0
slot, out = int(sys.argv[1]), Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
p = Pine()
p.load_state(slot)
time.sleep(1.0)
p.resume()
time.sleep(0.5)



def clock():
    return struct.unpack('<f', p.read_block(p.read32(ENGINE + 0x430) + 0xc, 4))[0]


for want in (float(c) for c in sys.argv[3:]):
    while clock() < want:
        time.sleep(0.005)
    subprocess.run([str(ROOT / 'kit'), 'shot', str(out / ('clk%.2f.png' % want))], check=False)
    print('shot at', clock(), flush=True)
