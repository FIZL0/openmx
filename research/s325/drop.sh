#!/bin/sh
# S325: a high nose-first drop in the original. recording2's state in slot 3 of the kit copy, teleported
# to free09's farmyard (4642.5, 3650) into slot 4 once (`research/s325/drop.sh tele`), then per drop:
# thrown UP ft/s up and 15 forward with a pitch rate RATE once airborne (research/s138_nose.py, state
# into slot 8), and the drop recorded from slot 8 into context/capture/s138/NAME.csv with states into
# slots 5..7 from the first tick WHEN holds (research/s138_hits.py). The air control damps the rate
# within a few ticks, so RATE sets the nose's angle for the whole fall: 5 gave 22 degrees, 40 gave 76,
# 60 passed 85. Run under `./kit pcsx2 with --`, PCSX2 started with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
# QT_ACCESSIBILITY=1, the game not paused. The test reads the third state's eeMemory.bin as
# NAME-<frame>.ee.bin (unzip it out of the .p2s).
# usage: research/s325/drop.sh tele | research/s325/drop.sh NAME UP RATE WHEN FRAMES
#   s325-nose22 60 5.0 'vy<-40' 140 · s325-nose76 60 40.0 'vy<-40' 140 · s325-dive 60 60.0 True 60
#   s325-flat 85 0.0 'vy<-60' 200
set -e
cd "$(dirname "$0")/../.."
pid=$(./kit pcsx2 status | sed -n 's/.*pid \([0-9]*\).*/\1/p' | head -1)
if [ "$1" = tele ]; then
    ./kit pine load 3
    python3 -B -c "import time; time.sleep(4)"
    python3 -B research/s76_tele.py "$pid" 4642.5 135.6 3650 8 4
    exit 0
fi
python3 -B research/s138_nose.py "$pid" 4 "$2" "$3" 8 15 0.3 > /dev/null
python3 -B research/s138_hits.py "$1" --pad research/s138/idle.pad --state 8 --when "$4" --frames "$5"
