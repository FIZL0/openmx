#!/bin/sh
# S139: recording2's bike (slot 3) teleported to X Z in free09's farmyard and driven north at full gas into a post,
# the chassis sweep back recorded with the whole bike (research/s139/sweep.toml), states saved from WHEN.
# usage: [STATE=slot] research/s139/post.sh NAME X Z WHEN [PAD]   (STATE: recording2's state, slot 3 by default)
set -e
cd "$(dirname "$0")/../.."
pid=$(./kit pcsx2 status | sed -n 's/.*pid \([0-9]*\).*/\1/p' | head -1)
[ -n "$pid" ] || pid=$(pgrep -f "pcsx2-qt.*$(pwd | xargs basename)" | head -1)
./kit pine load "${STATE:-3}"
python3 -B -c "import time; time.sleep(4)"
python3 -B research/s76_tele.py "$pid" "$2" 135.6 "$3" 8 4
python3 -B research/journal/2026-10-20-c6-c6-recording2-capture/playback/resume_if_paused.py "$pid"
python3 -B research/s138_hits.py "$1" --watch research/s139/sweep.toml --pad "${5:-research/s76/gas.pad}" --state 4 --when "$4" --frames 260
