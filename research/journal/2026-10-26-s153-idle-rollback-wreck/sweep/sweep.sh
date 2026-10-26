#!/usr/bin/env bash
# One idle trace at x z deg on a track: prints the spot, the first wreck's tick and the signed speed
# (velocity along the heading) on the tick before it, if it wrecks.
# usage: sweep.sh TRACK X Z DEG FRAMES
cd "$(dirname "$0")/../../../.."
./target/release/openmx --track "$1" --phy race125p.phy --trace-at "$2,$3,$4" --trace "$5" --headless --trace-feel 2>&1 \
  | awk -v spot="$1 $2 $3 $4" '/^[0-9]+ / && NF>=12 {
      if ($12 == 1) { printf "%s: wreck tick %d signed %.2f min %.2f at %s,%s,%s\n", spot, $1, prev, low, $6, $7, $8; exit }
      prev = $3 * sin($9) + $5 * cos($9); if (prev < low) low = prev }'
true
