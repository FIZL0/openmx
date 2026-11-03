#!/bin/bash
# F27a1: the port's knock of free09's ball 2, heard. The shell rides research/f27a1/tape.py's run-up
# from the original's aimed line (tune.py's pick) in a window on a private headless weston. An empty
# ALSA_CONFIG_PATH leaves cpal no device, so the shell falls back to piping its mix, and a stand-in
# pw-cat first on PATH keeps the 48 kHz s16 stereo stream it is handed.
#
# usage (from the worktree, after cargo build --release -p openmx):
#   bash research/f27a1/ours.sh OUT_DIR [DIST GAS TICKS]
# writes OUT_DIR/ours.s16 (the stream), OUT_DIR/ours.tape and OUT_DIR/ours.log.
set -e
out=$(realpath "$1"); dist=${2:-36}; gas=${3:-12}; ticks=${4:-150}
mkdir -p "$out/bin"
cat > "$out/bin/pw-cat" <<EOF
#!/bin/sh
exec cat > "$out/ours.s16"
EOF
chmod +x "$out/bin/pw-cat"
: > "$out/no-alsa.conf"
python3 -I research/f27a1/tape.py "$gas" "$ticks" "$out/ours.tape"
at=$(python3 -I -c "
import math
b=(5042.84,3554.94); f=(0.586,0.810); n=math.hypot(*f)
print('%.2f,%.2f,%.2f' % (b[0]-$dist*f[0]/n, b[1]-$dist*f[1]/n, math.degrees(math.atan2(*f))))")
sock=f27a1-$$
weston --backend=headless --shell=kiosk --idle-time=0 --socket=$sock --width=1280 --height=720 \
    > "$out/weston.log" 2>&1 &
wpid=$!
trap 'kill $wpid 2>/dev/null || true' EXIT
for _ in $(seq 50); do [ -S "$XDG_RUNTIME_DIR/$sock" ] && break; python3 -c 'import time; time.sleep(0.1)'; done
ALSA_CONFIG_PATH="$out/no-alsa.conf" PATH="$out/bin:$PATH" WAYLAND_DISPLAY=$sock DISPLAY= timeout 300 ./target/release/openmx \
    --track free09 --trace-at "$at" --play "$out/ours.tape" ${OPENMX_ARGS:-} > "$out/ours.log" 2>&1 || echo "openmx rc $?"
echo "start $at, gas $gas of $ticks ticks; $(stat -c %s "$out/ours.s16") bytes of sound"
