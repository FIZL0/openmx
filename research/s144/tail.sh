#!/bin/sh
# S144: recording2's bike (slot 3) teleported to the farmyard, thrown up UP ft/s with FWD forward, a
# pitch rate RATE (negative: tail down) and a yaw rate YAW (research/s144_tail.py, state into slot 8),
# and the landing recorded from slot 8 into context/capture/s144/NAME.csv.
# Run under `./kit pcsx2 with --`.  usage: research/s144/tail.sh NAME UP RATE FWD YAW [FRAMES] [PAD]
set -e
cd "$(dirname "$0")/../.."
pid=$(./kit pcsx2 status | sed -n 's/.*pid \([0-9]*\).*/\1/p' | head -1)
./kit pine load 3
python3 -B -c "import time; time.sleep(4)"
python3 -B research/s76_tele.py "$pid" 4642.5 135.6 3650 8 4
python3 -B research/s144_tail.py "$pid" 4 "$2" "$3" 8 "$4" 0.5 "$5"
python3 -B research/s138_hits.py "../s144/$1" --pad "${7:-research/s138/idle.pad}" --state 8 --when 1 --frames "${6:-150}"
