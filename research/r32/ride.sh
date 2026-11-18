#!/bin/bash
# C39: record one of S89's rides (watch/s89-rails.toml) with no uinput pad (/dev/uinput is root's).
#   ./kit pcsx2 with -- bash research/r32/ride.sh PID STATE.p2s PAD ROWS OUT.csv
# PCSX2 copy started with the AT-SPI env and NominalScalar 0.25 (the spec's quarter speed). PAD is a
# watch/pads/s89-*.pad, wall-clock at a quarter speed: its sleeps are divided by 4 into game time and
# the script becomes a from-savestate tape (research/c15/tape.py) on STATE.p2s, played over AT-SPI
# (C6's recipe). The spec's `state` line goes; rows start on the first frame after the state's own
# frame with the rider in (C15's play.sh trigger, on @frame_counter as S89 recorded), and every row
# must hold the human's rider (rider_ctl_src 1).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
C6="$ROOT/research/journal/2026-10-20-c6-c6-recording2-capture/playback"
PID=$1 STATE=$2 PAD=$3 N=$4 OUT=$5
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
python3 -I - "$PAD" "$WORK/game.pad" <<'PY'
import sys
out = []
for line in open(sys.argv[1]):
    w = line.split('#')[0].split()
    if w and w[0] == 'sleep' and not w[1].endswith('t'):
        line = 'sleep %d\n' % (int(w[1]) // 4)
    out.append(line)
open(sys.argv[2], 'w').writelines(out)
PY
python3 -I "$ROOT/research/c15/tape.py" "$ROOT/context/recordings/animandpump.p2m2" "$WORK/game.pad" "$WORK/play.p2m2"
FIRST=$(python3 -I -c "import sys, zipfile, struct; print(struct.unpack_from('<I', zipfile.ZipFile(sys.argv[1]).read('eeMemory.bin'), 0x479d04)[0])" "$STATE")
python3 -I -c 'import shutil,sys; shutil.copy(sys.argv[1], sys.argv[2])' "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
python3 -I - "$ROOT/watch/s89-rails.toml" "$WORK/spec.toml" "$FIRST" <<'PY'
import sys
src, dst, first = sys.argv[1:]
s = open(src).read()
s = s.replace('every       = 1\n', f'every       = 1\nstart       = "frame_counter > {first} and frame_counter <= {int(first) + 26} and rider_type == 2"\n')
s = s.replace('[setup]\nstate   = 9\n', '[setup]\n')
# the chain reads null on the load's first frame, and a required field ends the capture there even
# before `start` arms; the rows are checked for rider_ctl_src 1 afterwards instead
s = s.replace('required = true\n', '')
assert 'start ' in s and 'state   = 9' not in s, 's89-rails.toml changed: the spec rewrite missed'
open(dst, 'w').write(s)
PY
mkdir -p "$(dirname "$OUT")"
echo "state frame $FIRST, work $WORK"
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
# the movie pauses the VM when it ends; keep it running until the recorder has its rows
deadline=$((SECONDS + 1200))
while kill -0 $rec 2> /dev/null && [ $SECONDS -lt $deadline ]; do
    python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null || true
    sleep 2
done
kill $rec 2> /dev/null || true
wait $rec || true
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null || true
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null || true
grep -v 'null pointer' "$WORK/rec.log" | tail -8
python3 -I -c "import csv,sys; csv.field_size_limit(1<<24); rows=list(csv.DictReader(open(sys.argv[1]))); bad=[r['frame_counter'] for r in rows if r['rider_ctl_src']!='1']; print(len(rows), 'rows, frames', rows[0]['frame_counter'], '..', rows[-1]['frame_counter']); sys.exit('rows without the human rider: %s' % bad[:5] if bad or not rows else 0)" "$OUT"
