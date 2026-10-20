#!/bin/bash
# I7: play a human bike recording back on the original and capture watch/acceptance.toml plus the
# player record's two left-stick floats (+0x324/+0x328, the values the ride's stick consumers were
# handed), so the bike's dead zone can be read off the report bytes beside them. C6's recipe and
# C6's AT-SPI helpers, unchanged; only the spec grows. Run from the repo root, as one job:
#
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 start                         # note the pid it prints; wait for ./kit pine to answer
#   ./kit pcsx2 with -- bash research/journal/2026-10-20-i7-ride-stick-dead-zone/playback/play_capture.sh \
#       PID context/recordings/recording2.p2m2 2374 1894 context/capture/i7/recording2-sticks.csv
#
# FIRST is the first frame_counter to write (the recording's state holds FIRST - 1); N the rows.
set -e
C6=research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 REC=$2 FIRST=$3 N=$4 OUT=$5
mkdir -p "$(dirname "$OUT")"
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e '/^state  = /d' -e 's/^warmup = .*/warmup = 0/' -e '/^required = true/d' \
    -e "s/^every       = 1 .*/&\nstart       = \"frame_counter >= $FIRST and frame_counter <= $((FIRST + 26)) and rider_type == 2\"/" \
    watch/acceptance.toml > "$WORK/spec.toml"
cat >> "$WORK/spec.toml" <<'EOF'

[[field]]
name = "stick_x"                  # bike +0x20 -> +0x324: the player record's left stick x
addr = "@game_manager -> 0x34 -> 0x34 -> 0x8 -> 0x8 -> 0x8 -> 0x8 -> 0x8 -> 0x8 -> 0x8 -> 0x8 -> 0x0 -> 0x20 -> 0x324"
type = "f32"

[[field]]
name = "stick_y"                  # bike +0x20 -> +0x328: the player record's left stick y
addr = "@game_manager -> 0x34 -> 0x34 -> 0x8 -> 0x8 -> 0x8 -> 0x8 -> 0x8 -> 0x8 -> 0x8 -> 0x8 -> 0x0 -> 0x20 -> 0x328"
type = "f32"
EOF

python3 "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$C6/resume_if_paused.py" "$PID"
python3 "$C6/press.py" "$PID" 'System/Reset'
sleep 2
KIT_FRAME_STALL=120 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
sleep 2
python3 "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 "$C6/open_only.py" "$PID" "$WORK/play.p2m2"
sleep 4
python3 "$C6/resume_if_paused.py" "$PID"
wait $rec || true
python3 "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 "$C6/press.py" "$PID" 'System/Pause'
grep -v 'null pointer' "$WORK/rec.log" | tail -5
python3 -c "import csv,sys; r=next(csv.DictReader(open(sys.argv[1]))); sys.exit(0 if r['rider_type']=='2' else 'first row has no rider: the movie did not load')" "$OUT"
echo "work files (the played copy, the spec, the logs): $WORK"
