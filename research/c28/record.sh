#!/bin/bash
# C28: one MD37a ATV capture with no uinput pad. The .pad becomes a from-savestate tape (research/c15/
# tape.py) played over AT-SPI from STATE.p2s, and research/md37a/atv.toml is recorded every tick from
# the state's own vsync on (C15's play.sh trigger, with the ATV's vptr 0x003ce250 at +4).
#   KIT_CONFIG=<SLUS-21104 kit.toml> ./kit pcsx2 with -- bash research/c28/record.sh PID ATV STATE.p2s NAME PAD [FRAMES]
# Writes context/capture/md37a/NAME.csv (+ .json) and context/recordings/c28/NAME.p2m2 (+ .ticks.csv).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
C6=$(cd "$HERE/../journal/2026-10-20-c6-c6-recording2-capture/playback" && pwd)
PID=$1 ATV=$2 STATE=$3 NAME=$4 PAD=$5 N=${6:-300}
OUT=$ROOT/context/capture/md37a/$NAME.csv
TAPE=$ROOT/context/recordings/c28/$NAME.p2m2
mkdir -p "$(dirname "$OUT")" "$(dirname "$TAPE")"
python3 -I "$ROOT/research/c15/tape.py" "$ROOT/context/recordings/c28/template.p2m2" "$ROOT/research/md37a/$PAD" "$TAPE"
FIRST=$(python3 -I -c "import sys, zipfile, struct; print(struct.unpack_from('<I', zipfile.ZipFile(sys.argv[1]).read('eeMemory.bin'), 0x3fa6d8)[0])" "$STATE")
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
cp "$TAPE" "$WORK/play.p2m2"
cp "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
python3 -I - "$HERE/../md37a/atv.toml" "$WORK/spec.toml" "$ATV" "$NAME" "$FIRST" <<'PY'
import sys
src, dst, atv, name, first = sys.argv[1:]
s = open(src).read().replace('ATV + ', f'{atv} + ')
s = s.replace('name        = "atv"', f'name        = "{name}"')
s = s.replace('frames      = 300\n', f'frames      = 300\nstart       = "vsync >= {first} and vsync <= {int(first) + 26} and vptr == {0x3ce250}"\n')
s = s.replace('[setup]\nwarmup    = 2\nsettle_ms = 300\n', f'''[setup]
warmup = 0

[[field]]
name = "vsync"          # 0x3fa6d8, +2 a tick
addr = "0x3fa6d8"
type = "u32"

[[field]]
name = "vptr"           # the ATV's 0x003ce250 once the state is in
addr = "{atv} + 0x4"
type = "u32"
''')
assert 'start ' in s and 'name = "vptr"' in s, 'atv.toml changed: the spec rewrite missed'
open(dst, 'w').write(s)
PY
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
while kill -0 $rec 2> /dev/null; do
    python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null || true
    sleep 2
done
wait $rec || true
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
grep -v 'null pointer' "$WORK/rec.log" | tail -6
python3 -c "import csv,sys; csv.field_size_limit(1<<24); r=next(csv.DictReader(open(sys.argv[1]))); sys.exit(0 if r['vptr']=='$((0x3ce250))' else 'first row has no ATV: the movie did not load')" "$OUT"
