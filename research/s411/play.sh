#!/bin/bash
# S411: C15's play.sh with card.toml (the bike, its score card and the node pool) and its own start
# state (C6's recipe: research/journal/2026-10-20-c6-c6-recording2-capture):
#   KIT_CONFIG=$PWD/context/c32-atv-kit.toml KIT_PCSX2=N ./kit pcsx2 with -- bash research/s411/play.sh \
#       PID context/recordings/s411/NAME.p2m2 context/mxvatv-ps2/s411/start.p2s ROWS context/capture/s411/NAME.csv
# Tapes: python3 -I research/c15/tape.py context/recordings/animandpump.p2m2 research/s411/NAME.pad OUT.
# STATE.p2s becomes the tape's _SaveState.p2s. Rows start on the state's own vsync (13441, FIRST=)
# once the bike's vptr is in, so a long-booted VM cannot arm on the menus.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
C15=$(cd "$HERE/../c15" && pwd)
C6=$(cd "$HERE/../journal/2026-10-20-c6-c6-recording2-capture/playback" && pwd)
PID=$1 REC=$2 STATE=$3 N=$4 OUT=$5
FIRST=${FIRST:-13441}
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
cp "$REC" "$WORK/play.p2m2"
cp "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e "s/^every       = 1.*/&\nstart       = \"vsync >= $FIRST and vsync <= $((FIRST + 26)) and vptr == 3998800\"/" \
    "$HERE/card.toml" > "$WORK/spec.toml"
mkdir -p "$(dirname "$OUT")"

python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 -B "$C6/resume_if_paused.py" "$PID"
python3 -B "$C6/press.py" "$PID" 'System/Reset'
sleep 2
KIT_FRAME_STALL=${STALL:-60} ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
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
grep -v 'null pointer' "$WORK/rec.log" | tail -14
python3 -c "import csv,sys; csv.field_size_limit(1<<24); r=next(csv.DictReader(open(sys.argv[1]))); sys.exit(0 if r['vptr']=='3998800' else 'first row has no bike: the movie did not load')" "$OUT"
echo "work files (the played copy, the spec, the logs): $WORK"
