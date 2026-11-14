#!/bin/bash
# MD160: play a pad script (research/c15/tape.py's language) from an Untamed (SLUS-21701) truck state and
# capture research/md160/truck.toml every tick. research/md39a/play.sh with Untamed's vsync and tape name:
#   KIT_CONFIG=<SLUS-21701 kit.toml> ./kit pcsx2 with -- bash research/md160/play.sh PID SCRIPT.pad \
#       STATE.p2s CAR ROWS NAME
# CAR is the truck object (hex, research/md160/ram.py's moving one). Rows start on the state's own vsync
# once the truck's vtable (0x003f1840) is in, so a long-booted VM cannot arm on the menus. ROWS is cut to
# the tape's ticks less 5: the movie's end stops the VM. Writes context/capture/md160/NAME.csv (+ .json)
# and context/recordings/md160/NAME.p2m2 (+ .ticks.csv).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
C6=$ROOT/research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 PAD=$2 STATE=$3 CAR=$4 N=$5 NAME=$6
VPTR=$((0x3f1840))
OUT=$ROOT/context/capture/md160/$NAME.csv
TAPE=$ROOT/context/recordings/md160/$NAME.p2m2
mkdir -p "$(dirname "$OUT")" "$(dirname "$TAPE")"
python3 -I "$ROOT/research/c15/tape.py" "$ROOT/context/recordings/animandpump.p2m2" "$PAD" "$TAPE" \
    --game "MX vs. ATV Untamed" > /dev/null
TICKS=$(($(wc -l < "$TAPE.ticks.csv") - 1))
[ "$N" -gt $((TICKS - 5)) ] && N=$((TICKS - 5))
FIRST=$(python3 -I -c "import sys, zipfile, struct; z = zipfile.ZipFile(sys.argv[1]); print(struct.unpack_from('<I', z.read(next(n for n in z.namelist() if n.endswith('eeMemory.bin'))), 0x410318)[0])" "$STATE")
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
cp "$TAPE" "$WORK/play.p2m2"
cp "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e "s/CAR/$CAR/" \
    -e "s/^every       = 1.*/&\nstart       = \"vsync >= $FIRST and vsync <= $((FIRST + 26)) and vptr == $VPTR\"/" \
    "$HERE/truck.toml" > "$WORK/spec.toml"

python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Reset' > /dev/null
sleep 2
cd "$ROOT"
KIT_FRAME_STALL=300 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
sleep 2
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2" > /dev/null
sleep 4
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
wait $rec || true
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Pause' > /dev/null
grep -v 'null pointer' "$WORK/rec.log" | tail -6
python3 -c "import csv,sys; csv.field_size_limit(1<<24); r=next(csv.DictReader(open(sys.argv[1]))); sys.exit(0 if r['vptr']==sys.argv[2] else 'first row has no truck: the movie did not load')" "$OUT" "$VPTR"
echo "rows $N of $TICKS ticks, first vsync $FIRST; work files: $WORK"
