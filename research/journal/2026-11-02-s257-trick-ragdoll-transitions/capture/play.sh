#!/bin/bash
# S257: play recording2 back on the original and log S74's bones rows through its tricks
# (V61's play.sh, with bones_playback.py in place of the spec recorder).
#   KIT_PCSX2=1 ./kit pcsx2 with -- bash research/journal/2026-11-02-s257-trick-ragdoll-transitions/capture/play.sh \
#       PID context/recordings/recording2.p2m2 2374 1700 context/s257/recording2.txt
# The steps and why each is there are C6's (research/journal/2026-10-20-c6-c6-recording2-capture).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
C6=$(cd "$HERE/../../2026-10-20-c6-c6-recording2-capture/playback" && pwd)
PID=$1 REC=$2 FIRST=$3 N=$4 OUT=$5
WORK=$(mktemp -d -p "${SCRATCH:-/tmp}")
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays

python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' || true
python3 -B "$C6/resume_if_paused.py" "$PID"
python3 -B "$C6/press.py" "$PID" 'System/Reset'
read -t 2 < /dev/zero || true
python3 -B "$HERE/bones_playback.py" "$FIRST" "$N" "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
read -t 2 < /dev/zero || true
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2"
read -t 4 < /dev/zero || true
python3 -B "$C6/resume_if_paused.py" "$PID"
wait $rec || true
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' || true
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Pause'
cat "$WORK/rec.log"
echo "work files: $WORK"
