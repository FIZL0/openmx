"""V36: the original stopped, shot. Copy 1 must be running with first_recording's state in slot 8 (run.py).

For each ride: load slot 8, play the ride's pad script (ride off, brake to a stand), shoot the window,
and record two seconds of feet.toml so the shot carries the bike's tilt and the foot pins.

    WAYLAND_DISPLAY=wayland-1 ./kit pcsx2 with -- python3 -B research/journal/2026-10-24-v36-foot-down/capture/shots.py
"""

import subprocess
import time
from pathlib import Path

here = Path(__file__).resolve().parent
root = here.parents[3]
out = root / "context/shots/v36"
out.mkdir(parents=True, exist_ok=True)

RIDES = {
    # straight off the start line, onto the flat
    "flat": ["down cross", "sleep 3000", "release", "down square", "sleep 4000", "release", "sleep 2500"],
    # hard left off the line, towards the rise beside the start, stopping on it
    "left": ["stick l -1 0", "down cross", "sleep 4500", "release", "down square", "sleep 4000", "release", "sleep 2500"],
    # hard right
    "right": ["stick l 1 0", "down cross", "sleep 4500", "release", "down square", "sleep 4000", "release", "sleep 2500"],
}


def kit(*args):
    subprocess.run(["./kit", *args], cwd=root, check=True)


for name, moves in RIDES.items():
    kit("pine", "load", "8")
    time.sleep(3)
    kit("pad", "send", *moves)
    kit("shot", str(out / f"{name}.png"))
    kit("record", str(here / "feet.toml"), "--seconds", "2", "--out", f"context/capture/v36/{name}.csv")
