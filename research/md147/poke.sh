#!/bin/bash
# MD147: load a race quad state from a slot, write every entry of the quad's surface hazard table (16 x 12 B
# at quad +0xde4: the hazard word, then the time; research/md147/surfaces.py) to HAZARD and TIME, and save
# the result into another slot. Rocksbury's own table is all zero, so this is how a ride gets a hazard
# surface under the quad.
#   KIT_CONFIG=<SLUS-21701 kit.toml> ./kit pcsx2 with -- bash research/md147/poke.sh PID IN QUAD HAZARD TIME OUT
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
C6=$ROOT/research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 IN=$2 QUAD=$3 HAZARD=$4 TIME=$5 OUT=$6
cd "$ROOT"
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
./kit pine load "$IN" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Pause' > /dev/null 2>&1 || true
sleep 1
for i in $(seq 0 15); do
    a=$(printf '%x' $((QUAD + 0xde4 + 12 * i)))
    ./kit pine write "$a" "$HAZARD" --as u32 > /dev/null
    ./kit pine write "$(printf '%x' $((0x$a + 4)))" "$TIME" --as f32 > /dev/null
done
./kit pine read "$(printf '%x' $((QUAD + 0xde4)))" 6
./kit pine save "$OUT" > /dev/null
echo "vsync $(./kit pine read 0x410318 1 | awk '{print $2}'), saved slot $OUT"
