"""S241: unpause a PCSX2 copy through its System > Pause menu item (AT-SPI; no pad here).

usage: python3 -B research/s241/run.py PID"""
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'journal/2026-10-20-c6-c6-recording2-capture/playback'))
import atspi_lib as A                    # noqa: E402
from gi.repository import Atspi          # noqa: E402

item = A.menu_path(A.app_of(int(sys.argv[1])), ['System', 'Pause'])
if item.get_state_set().contains(Atspi.StateType.CHECKED):
    A.press(item)
time.sleep(0.5)
print('paused' if item.get_state_set().contains(Atspi.StateType.CHECKED) else 'running')
