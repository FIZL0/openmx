#!/bin/bash
# S90: the shell's shots of the s90 ride tape at the PCSX2 run's ticks; $1 = out dir, rest = extra flags.
export DISPLAY=:0 WAYLAND_DISPLAY=wayland-1
out=$1
shift
mkdir -p "$out"
for t in ${TICKS:-60 90 96 102 120 150 200 206 212 218 225 240 260 300 335 350 410}; do
    timeout 120 target/release/openmx --track free09 --play context/tapes/s90/ride.tape --headless \
        --frames $((t + 2)) --shot "$out/shell-$t.png" --shot-frame "$t" --size 640x480 --no-sound "$@" \
        > "$out/log-$t.txt" 2>&1
done
