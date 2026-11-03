#!/bin/bash
# F27a1: the S195 knock from save slot 9 (aim.py's state) with PCSX2's Video Capture on, menus over AT-SPI
# (PCSX2 started with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1). Writes context/f27a1/NAME.mp4
# and knock.py's csv and states. usage: bash research/f27a1/capture.sh PID NAME
set -e
pid=$1; name=$2
python3 -B research/journal/2026-10-20-c6-c6-recording2-capture/playback/resume_if_paused.py $pid || true
python3 -B research/journal/2026-10-21-p49-engine-sound/capture/press.py $pid 'System/Video Capture' >/dev/null 2>&1 &
press=$!
python3 -c 'import time; time.sleep(3)'
python3 -B research/journal/2026-10-21-p49-engine-sound/capture/save_as.py $pid 'Video Capture' $(pwd)/context/f27a1/$name.mp4
kill $press 2>/dev/null || true
python3 -c 'import time; time.sleep(2)'
python3 -B research/s195/knock.py $name 9 2 --frames 300 || echo "knock.py rc $?"
timeout 8 python3 -B research/journal/2026-10-21-p49-engine-sound/capture/press.py $pid 'System/Video Capture' || true
