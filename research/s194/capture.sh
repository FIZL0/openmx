#!/bin/bash
# S194: record one of S129's/S146's rides with watch/s129-train.toml (which has the bike's clock since
# S194) by playing a held-buttons movie from a kit copy's save slot: P21's flat_capture.sh with S129's
# spec. The copy must run at a quarter speed (NominalScalar 0.25 in its ini) with the accessibility env.
#
#   KIT_PCSX2=1 ./kit pcsx2 with -- research/s194/capture.sh PID SLOT_P2S BUTTONS FIRST N OUT
#
# FIRST is the save state's frame_counter + 1; BUTTONS as research/s194/make_tape.py takes them.
set -e
H=research/journal/2026-10-20-c6-c6-recording2-capture/playback
S=$(dirname "$0")
PID=$1 STATE=$2 BUTTONS=$3 FIRST=$4 N=$5 OUT=$6
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
TEMPLATE=${S194_TEMPLATE:-/home/rycs/repos/openmx-slots/kept-context/s4/recordings/biplane.p2m2}
python3 -B "$S/make_tape.py" "$TEMPLATE" "$STATE" "$WORK/play.p2m2" $((N * 4 + 200)) "$BUTTONS"
chmod u+w "$WORK"/*
python3 -B - "$FIRST" > "$WORK/spec.toml" <<'EOF'
import re, sys
first = int(sys.argv[1])
s = open('watch/s129-train.toml').read()
s = re.sub(r'(?m)^state   = .*\n', '', s)
s = re.sub(r'(?m)^warmup  = .*$', 'warmup  = 0', s)
s = re.sub(r'(?m)^required = true\n', '', s)
s = re.sub(r'(?m)^every       = 1$',
           f'every       = 1\nstart       = "frame_counter >= {first} and frame_counter <= {first + 26} and rider_type == 2"', s)
print(s, end='')
EOF

python3 "$H/press.py" "$PID" 'Tools/Input Recording/Stop' || true
python3 "$H/resume_if_paused.py" "$PID"
python3 "$H/press.py" "$PID" 'System/Reset'
python3 -c 'import time; time.sleep(2)'
mkdir -p "$(dirname "$OUT")"
KIT_FRAME_STALL=300 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
python3 -c 'import time; time.sleep(2)'
python3 "$H/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 "$H/open_only.py" "$PID" "$WORK/play.p2m2"
python3 -c 'import time; time.sleep(4)'
python3 "$H/resume_if_paused.py" "$PID"
timeout 1800 tail --pid=$rec -f /dev/null || true
wait $rec || true
python3 "$H/press.py" "$PID" 'Tools/Input Recording/Stop' || true
python3 "$H/resume_if_paused.py" "$PID" > /dev/null
python3 "$H/press.py" "$PID" 'System/Pause'
grep -v 'null pointer' "$WORK/rec.log" | tail -5
echo "work files: $WORK"
