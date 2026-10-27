#!/bin/bash
# One playback of the 2p tape per emulator setting (C12): stop the copy, set the setting, start it, play
# the whole tape with research/c12/play_variant.sh, compare against C10's capture of record.
# From the repo root, with the copy's a11y env:
#
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   bash research/c12/sweep.sh NAME PATCH RATE SKIP [N]
#
# PATCH: 1 enables PCSX2's "60 FPS" patch for the disc (per-game ini), 0 leaves it off. RATE/SKIP: the
# copy's EECycleRate / EECycleSkip. Writes context/capture/2p/c12/NAME.csv and its compare beside it.
set -e
NAME=$1 PATCH=$2 RATE=$3 SKIP=$4 N=${5:-12917}
TAPE=${TAPE:-context/recordings/split/2p.p2m2}   # copied before it plays
CFG=$(./kit pcsx2 status | awk '/^config/{print $2}')
INI=$CFG/inis/PCSX2.ini
GAME=$CFG/gamesettings/SLUS-20767_9866FE54.ini
./kit pcsx2 stop > /dev/null 2>&1 || true
sed -i -e "s/^EECycleRate = .*/EECycleRate = $RATE/" -e "s/^EECycleSkip = .*/EECycleSkip = $SKIP/" "$INI"
if [ "$PATCH" = 1 ]; then printf '[Patches]\nEnable = 60 FPS\n' > "$GAME"; else rm -f "$GAME"; fi
PID=$(./kit pcsx2 start 2>&1 | grep -o 'pid [0-9]*' | awk '{print $2}')
for i in $(seq 1 60); do ./kit pine > /dev/null 2>&1 && break; timeout 1 tail -f /dev/null || true; done
timeout 2 tail -f /dev/null || true
OUT=context/capture/2p/c12/$NAME.csv
timeout 1500 ./kit pcsx2 with -- bash research/c12/play_variant.sh "$PID" "$TAPE" "$N" "$OUT" 2>&1 | grep -E 'ended|FAIL' || true
python3 -B research/c10/compare.py 2p "c12/$NAME" > "context/capture/2p/c12/$NAME.compare.txt" 2>&1 || true
echo "== $NAME (patch $PATCH, EECycleRate $RATE, EECycleSkip $SKIP)"
grep -v '^2p:' "context/capture/2p/c12/$NAME.compare.txt"
