"""Press a PCSX2 menu item over AT-SPI. usage: press.py PID 'System/Video Capture'

C6's atspi_lib (research/journal/2026-10-20-c6-c6-recording2-capture/playback/) does the finding.
"""
import os, sys
here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, '../../2026-10-20-c6-c6-recording2-capture/playback'))
import atspi_lib as A

app = A.app_of(int(sys.argv[1]))
item = A.menu_path(app, sys.argv[2].split('/'))
print(A.press(item))
