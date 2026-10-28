#!/bin/sh
# S139: recording2's bike teleported to X Z in free09's farmyard, standing, then given the world
# velocity (VX, VY, VZ) by hand (research/s139_shove.py, state into slot 8), and the run recorded from
# slot 8 with the chassis sweep back (research/s139/sweep.toml), states saved from WHEN. Run under
# `./kit pcsx2 with --`.
# usage: [STATE=slot] research/s139/shove.sh NAME X Z VX VY VZ WHEN [PAD]   (STATE: recording2's state, slot 3 by default)
set -e
cd "$(dirname "$0")/../.."
pid=$(./kit pcsx2 status | sed -n 's/.*pid \([0-9]*\).*/\1/p' | head -1)
./kit pine load "${STATE:-3}"
python3 -B -c "import time; time.sleep(4)"
python3 -B research/s76_tele.py "$pid" "$2" 135.6 "$3" 8 4
python3 -B research/s139_shove.py "$pid" 4 "$4" "$5" "$6" 8
python3 -B research/journal/2026-10-20-c6-c6-recording2-capture/playback/resume_if_paused.py "$pid"
python3 -B research/s138_hits.py "$1" --watch research/s139/sweep.toml --pad "${8:-research/s138/idle.pad}" --state 8 \
    --when "$7" --frames ${FRAMES:-60}
