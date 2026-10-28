#!/bin/bash
# V70: watch/v70-zones.toml over a human recording played back on the original — C6's play_capture.sh
# (research/journal/2026-10-20-c6-c6-recording2-capture/playback/) with this spec, its rider chain cut
# to animandpump's one hop and its start armed on FIRST. Run from the repo root, as one job:
#
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 start                         # note the pid; wait for ./kit pine to answer
#   ./kit pcsx2 with -- bash research/v70/play.sh PID context/recordings/animandpump.p2m2 1939 1100 \
#       context/capture/v70/animandpump.csv
set -e
C6=$(cd "$(dirname "$0")/../journal/2026-10-20-c6-c6-recording2-capture/playback" && pwd)
PID=$1 REC=$2 FIRST=$3 N=$4 OUT=$5
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e '/^state  *= /d' -e 's/0x8 -> 0x8 -> 0x0/0x8 -> 0x0/' \
    -e "s/^start .*/start       = \"frame_counter >= $FIRST and frame_counter <= $((FIRST + 26)) and rider_type == 2\"/" \
    watch/v70-zones.toml > "$WORK/spec.toml"

python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 -B "$C6/resume_if_paused.py" "$PID"
python3 -B "$C6/press.py" "$PID" 'System/Reset'
sleep 2
KIT_FRAME_STALL=300 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
sleep 2
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2"
sleep 4
python3 -B "$C6/resume_if_paused.py" "$PID"
wait $rec || true
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Pause'
grep -v 'null pointer' "$WORK/rec.log" | tail -5
python3 -c "import csv,sys; r=next(csv.DictReader(open(sys.argv[1]))); sys.exit(0 if r['rider_type']=='2' else 'first row has no rider: the movie did not load')" "$OUT"
echo "work files: $WORK"
