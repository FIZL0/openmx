#!/bin/bash
# C38: load MD166's ATV race state in this kit copy and log the quads' records every frame.
#   KIT_CONFIG=<SLUS-21104 kit.toml> KIT_PCSX2=N ./kit pcsx2 with -- bash research/c38/watch.sh FRAMES [SAVE_FROM SAVE_TO]
# The copy must be running (./kit pcsx2 start with WAYLAND_DISPLAY set) with the state staged as slot 7.
set -e
./kit pine load 7
sleep 3
python3 -B research/c38/poll.py "$@"
