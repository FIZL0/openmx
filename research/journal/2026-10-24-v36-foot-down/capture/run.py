"""V36: put first_recording's state in copy 1's slot 8, wait for PINE, and record feet.toml under stop.pad.

    WAYLAND_DISPLAY=wayland-1 ./kit pcsx2 with -- python3 -B research/journal/2026-10-24-v36-foot-down/capture/run.py [out.csv]
"""

import shutil
import subprocess
import sys
import time
from pathlib import Path

here = Path(__file__).resolve().parent
root = here.parents[3]
slots = root.parent / f"{root.name}-slots" / "pcsx2" / "s1" / "PCSX2" / "savestates"
shutil.copyfile(root / "context/recordings/first_recording.p2m2_SaveState.p2s", slots / "SLUS-20767 (9866FE54).08.p2s")

subprocess.run(["./kit", "pcsx2", "start"], cwd=root, check=True)
for _ in range(120):
    if subprocess.run(["./kit", "pine"], cwd=root, capture_output=True).returncode == 0:
        break
    time.sleep(1)
else:
    sys.exit("PINE never answered")

out = sys.argv[1] if len(sys.argv) > 1 else "context/capture/v36/feet.csv"
(root / out).parent.mkdir(parents=True, exist_ok=True)
subprocess.run(["./kit", "pine", "load", "8"], cwd=root, check=True)
time.sleep(3)
subprocess.run(
    ["./kit", "record", str(here / "feet.toml"), "--pad", str(here / "stop.pad"), "--seconds", "19", "--out", out],
    cwd=root,
    check=True,
)
