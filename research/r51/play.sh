#!/bin/bash
# R51: play a pad script (research/c15/tape.py's language) from an MD147 Untamed (SLUS-21701) quad state and
# capture research/r51/camera.toml every tick. research/md147/play.sh with the camera recorder:
#   KIT_CONFIG=<SLUS-21701 kit.toml> KIT_PCSX2=N ./kit pcsx2 with -- bash research/r51/play.sh PID SCRIPT.pad \
#       STATE ROWS NAME
# PID is the PCSX2 copy's (the AT-SPI presses need it). STATE names context/untamed/STATE.p2s (md147/spawn),
# without the extension, as the context guard reads command lines. Rows start on the state's own vsync once the quad's
# vtable is in. ROWS is cut to the tape's ticks less 5: the movie's end stops the VM. Writes
# context/capture/r51/NAME.csv (+ .json) and context/recordings/r51/NAME.p2m2 (+ .ticks.csv).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
C6=$ROOT/research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 PAD=$2 STATE=$ROOT/context/untamed/$3.p2s N=$4 NAME=$5
VPTR=$((0x3f0470))
OUT=$ROOT/context/capture/r51/$NAME.csv
TAPE=$ROOT/context/recordings/r51/$NAME.p2m2
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
sed -e "s/^every       = 1.*/&\nstart       = \"vsync >= $FIRST and vsync <= $((FIRST + 26)) and vptr == $VPTR\"/" \
    "$HERE/camera.toml" > "$WORK/spec.toml"

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
python3 -c "import csv,sys; csv.field_size_limit(1<<24); r=next(csv.DictReader(open(sys.argv[1]))); sys.exit(0 if r['vptr']==sys.argv[2] else 'first row has no quad: the movie did not load')" "$OUT" "$VPTR"
echo "rows $N of $TICKS ticks, first vsync $FIRST; work files: $WORK"
