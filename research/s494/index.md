# S494: a bike ridden into a parked flier, on the original

Question: does the original's bike strike free06's parked helicopter and free10's parked plane, and
does a strike wake them? Answer: it strikes both, and pushes their bodies. Nothing wakes: both
fliers are awake all along (their sleep thresholds are never met, R52).

## Recipe (kit copy 2, accessibility bus on)

1. Boot to the track in free ride: `research/r52/boot.py` (free10, or free06). At copy speed the
   frame counter rests at 0 through the load and boot.py gives up; then
   `python3 -B research/s494/settle.py PID SLOT FRAMES` waits FRAMES frames and saves slot SLOT.
2. `KIT_PCSX2=2 ./kit pcsx2 with -- bash research/s494/flier.sh PID STATE.p2s MACHINE DIST FRAMES OUT.csv YAW LIFT`:
   a held-gas tape from that state; `ride.py` moves the bike DIST ft short of the flier's body
   along heading YAW (degrees, 0 = +z) with S76's triple-delta teleport, zeroes its velocities, and
   logs the flier's body a row a frame. `ride.py PID MACHINE probe` prints the machine's body and
   stored spot (machine `+0xe0`; R52's "+0x50 + 0x90") without moving anything.

Captures (context/, not committed):

- `context/capture/s494/plane1.csv`: free10, DIST 40, YAW −27.78 (the bike's own heading), LIFT
  3.6154737. The bike stops dead on the plane's hull ~13 ft short of its body on frames 414–417.
  The body's velocity goes from (−0.013, 0, 0.021) to (−0.70, 0.26, 1.35) ft/s, slows over ~100
  frames, and has moved 1.6 ft along z. The rider wrecks; the bike is put back ~frame 498.
- `context/capture/s494/heli1.csv`: free06, DIST 40, YAW −6.91, LIFT 7.0. The strike is at frames
  358–360: the helicopter's velocity jumps to (−0.94, −0.27, 2.22) ft/s. The bike stays pressed
  against it and pushes it at 1.0–1.3 ft/s for 80+ frames; it moves ~4.2 ft.

## The port

The parked plane is a `Flight` nobody flies in `Sim::parked_planes` (game-sim `parked_plane.rs`):
stepped every frame after the parked cars, its hull in the bikes' list after theirs, and its body
on each bike's resolve, written back. The shell hands it on a one-seat ride (`InPlay::hand_to`).
Test: `a_bike_ridden_into_a_parked_plane_stops_and_pushes_it` (tests/replay.rs) parks one on
free09 at S485's truck spot, nose to the bike: the bike stops 6.9 ft short of the body, the plane
peaks at (0, 0.49, 2.05) ft/s and moves 0.96 ft in 120 ticks. Same shape as plane1.csv, different
map and contact point, so not held to its numbers.

The helicopter (S494a) is the same: a `ParkedHeli` in `Sim::parked_helis` (game-sim
`parked_heli.rs`), its frame the shell's settle frame (collision search, then `Kiowa::tick`
unflown over `Grounded`, crash asks left undone), stepped after the parked planes; its hull
(`kiowaf.col`) after the planes' in the bikes' list, its body after theirs on each resolve. The
shell now settles it with `ParkedHeli::settled`, proved bit-identical to its own `parked_heli` on
R52's free06 image (openmx tests/parked_fliers.rs), and hands it on a one-seat ride. Test:
`a_bike_ridden_into_a_parked_helicopter_stops_and_pushes_it` parks it at the same free09 spot,
facing the bike: the bike stops 14.4 ft short of the body, the helicopter peaks at (0, −0.08, 2.64)
ft/s, slows to 1.72 ft/s 10 ticks on and 0.47 ft/s 40 on, and moves 1.87 ft in 120 ticks.
heli1.csv: 2.22 ft/s, ~4.2 ft at 1.0–1.3 ft/s over 80+ frames. Same shape, not held to numbers.
