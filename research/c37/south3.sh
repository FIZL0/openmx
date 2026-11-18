#!/bin/sh
# C37: B15's south3-bones capture remade. S144's midback-2734.p2s (B15's state) was lost, so the state
# is made again as S144's tail.sh made it: recording2's bike (slot 3) teleported to free09's farmyard
# (slot 4), thrown up UP ft/s with a tail-down pitch rate RATE (slot 8), then teleported mid-air to
# S167's south spot by the windmill and captured with its bones (research/r14_windmill_bones.py ride=0).
# Run under `./kit pcsx2 with --` on a copy started with QT_ACCESSIBILITY=1.
# usage: research/c37/south3.sh UP RATE [OUT]
set -e
cd "$(dirname "$0")/../.."
pid=$(./kit pcsx2 status | sed -n 's/.*pid \([0-9]*\).*/\1/p' | head -1)
python3 -B research/c37/put_state.py context/recordings/recording2.p2m2_SaveState.p2s 3
./kit pine load 3
python3 -B -c "import time; time.sleep(4)"
python3 -B research/s76_tele.py "$pid" 4642.5 135.6 3650 8 4
python3 -B research/s144_tail.py "$pid" 4 "$1" "$2" 8 0 0.5 0
python3 -B research/r14_windmill_bones.py "$pid" 8 2439 140 4067.5 30 "${3:-context/capture/b15/south3-bones.csv}" bones ride=0
