"""C33: wait (up to 300 s) until the copy's game is booted and counting frames, so a state load is
accepted (PINE refuses one on a copy still in the BIOS)."""
import sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

t0 = time.time()
while time.time() - t0 < 300:
    try:
        p = Pine(check=False, lock=False, timeout=5.0)
        if p.read32(0x00479d04) > 0:
            print('booted', round(time.time() - t0, 1), flush=True)
            time.sleep(2)
            raise SystemExit(0)
    except (OSError, RuntimeError):
        pass
    except SystemExit as e:
        if e.code == 0:
            raise
    time.sleep(0.5)
raise SystemExit('the game never booted')
