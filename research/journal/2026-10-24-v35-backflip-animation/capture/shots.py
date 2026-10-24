#!/usr/bin/env python3
"""V35: pause recording2's playback at a few ticks round its first backflip and ./kit shot each one.

    ./kit pcsx2 with -- bash research/journal/2026-10-24-v35-backflip-animation/capture/shots.sh PID REC OUTDIR [TICK ...]

Run after shots.sh has started the playback (C6's recipe). Polls the frame counter over PINE with a
plain sleep (a load trips next_frame's watchdog), pauses at each target, reads the tick it actually
stopped on, takes the shot and goes on.
"""
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT))
from ps2kit import pine as pine_mod  # noqa: E402

FRAME = 0x00479d04
out = Path(sys.argv[1])
targets = [int(t) for t in sys.argv[2:]] or [2508, 2514, 2522, 2532, 2542, 2560, 2600]
out.mkdir(parents=True, exist_ok=True)
p = pine_mod.Pine()
deadline = time.time() + 600
for t in targets:
    while p.read32(FRAME) < t - 1:
        if time.time() > deadline:
            sys.exit(f"never reached {t}: at {p.read32(FRAME)}")
        time.sleep(0.01)
    p.pause()
    time.sleep(0.5)
    at = p.read32(FRAME)
    name = out / f"orig-{at}.png"
    subprocess.run([str(ROOT / "kit"), "shot", str(name)], check=False)
    print(f"target {t}: paused at {at} -> {name}", flush=True)
    p.resume()
p.pause()
