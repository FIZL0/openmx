#!/bin/sh
# S138: recording2's bike (slot 3) teleported to X Z in free09's farmyard and driven north at full gas,
# the run recorded with states saved from the tick WHEN holds. Run under `./kit pcsx2 with --`.
# usage: research/s138/wall.sh NAME X Z WHEN [PAD]
set -e
cd "$(dirname "$0")/../.."
pid=$(./kit pcsx2 status | sed -n 's/.*pid \([0-9]*\).*/\1/p' | head -1)
[ -n "$pid" ] || pid=$(pgrep -f "pcsx2-qt.*$(pwd | xargs basename)" | head -1)
./kit pine load 3
python3 -B -c "import time; time.sleep(4)"
python3 -B research/s76_tele.py "$pid" "$2" 135.6 "$3" 8 4
python3 -B research/journal/2026-10-20-c6-c6-recording2-capture/playback/resume_if_paused.py "$pid"
python3 -B research/s138_hits.py "$1" --pad "${5:-research/s76/gas.pad}" --state 4 --when "$4" --frames 260
