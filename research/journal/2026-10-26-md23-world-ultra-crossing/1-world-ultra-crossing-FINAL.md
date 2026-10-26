# 1 — world ultra crossing (FINAL)

Task MD23. 2026-10-26.

## What I did

**The stitch was already fixed.** openmx-mods 44abe2c ("stitch: every at on a whole foot", hand-landed
2026-10-23 after MD17) puts every tile's and join's `at` on a whole foot. The user's
`~/repos/openmx/mods/tracks/world_ultra/mod.json` (written 2026-10-23 10:55) has that fix: a scan of
every number in it finds fractions only under `stitch` (spacings, slopes, collar periods, gate edges).
None are in `tiles[].at` or `joins[].at`, and `untamed_nat_or` is at `[216, 216]`. So there was nothing
left to round, and openmx-mods is untouched.

**world_ultra loads.** Release build, headless, offscreen RADV, 320x224, against the user's mods:

- 77 tiles and 2135 joins, 116132 x 33908 feet;
- 81 of 2212 pieces built before the ride in 5.3 s;
- 11653 chunks uploaded;
- first frame at about 14 s wall, max RSS 2154 MB.

world_untamed loads too: 114 of 697 pieces in 2.95 s, 6 s wall, 1202 MB.

**Riding across it.** A bike cannot cross 116,000 feet:

- a full-throttle gas tape from the spawn went 550 ft in 297 ticks and stalled about 2000 ft out;
- `bike_speed`/`crash_tolerance` at 500% do no better;
- gravity 0 leaves no traction;
- the parked biplane stands at free06's local spot, (2304, 2281), which is untamed_nat_or's corner of
  this world (MD24/MD32). Flown from there, its sim cost climbs to 30-300 ms a tick as it sinks, and
  it only got 26 mph in 3600 ticks. That is filed as **MD33**.

So the shell gained **`--world-sweep X0,Z0,X1,Z1`** (main.rs). With it, the ground stream's near point
goes linearly from `x0,z0` to `x1,z1` over the ride's `--frames`, instead of following the riders:

- it needs `--frames`, and refuses without it;
- the rider still rides its tape, and the simulation's tiles and the dress stream follow the rider as
  before;
- it is the real `GroundStream` and the real GPU `add_ground_piece_of`/`drop_ground_piece` path, so
  what it measures is MD17's ground stream end to end.

The shell also now prints `world ground: held at most N chunks at once in the ride, room for M` at the
end of a world ride. That is the most `Renderer::ground_chunks()` reported after each frame's changes.

**Results** (release, headless, 3600-tick gas tape, desktop reach 6000/9000; RSS sampled once a second
from /proc by a Python wrapper; the machine was under load 45 on 32 cores from the other slots' gates,
so wall times and fps are not meaningful, but RSS and counts are):

| run | built mid-ride | dropped | chunks held at most | room at most | RSS over the ride | max RSS |
|---|---|---|---|---|---|---|
| sweep x 500 → 115600 at z 18000 | 739 | 751 | 18381 | 20540 | 2002-2089 MB | 2125 MB |
| sweep (115600,33400) → (500,500) | 965 | 982 | 19523 | 20540 | 2004-2088 MB | 2164 MB |
| `--world-ground all`, 2 frames | — | — | — | 197178 | climbs to 6878 | 6874 MB |

**RSS is bounded on the crossing.**

- About 95,000 chunks went through the GPU in each sweep. The ground's room never passed 20540 chunks
  (a tenth of the whole world's 197178), so a dropped piece's room is reused.
- RSS held within ±50 MB of 2.05 GB from 12 s to the end of each crossing. The whole world at once is
  6.9 GB.
- Each sweep waited once at tick 0: 1249 ms for untamed_bt_b and 21 ms for u04_02_34_00. In both cases
  the sweep's first point sat over a piece the start had not built, because the start builds around
  the rider, not the sweep. There were no waits after that, at about 32 ft a tick (a sweep crosses
  the map at roughly 1000 mph).

## What the original does

Nothing to match: the original has no worlds. This is openmx's own streaming (MD17, MD28).
Nothing was taken from opentennis: it has no streamed terrain.

## How it was proved

- **Gate:** `crates/openmx/tests/world_streaming.rs`,
  `a_sweep_streams_the_ground_along_its_line_and_leaves_the_ride_alone`. It runs MD17's generated
  9-tile world with reach 200/300, sweeping from the start to (1800, 1800) over 540 ticks:
  - a piece is built (before or during the ride) if and only if the sweep's line, sampled at every
    tick, comes within load reach of it;
  - t00, v0 and h0 are dropped;
  - the GPU held fewer chunks at once than the whole world's (48 of 80, seen by a temporary failing
    probe, since removed);
  - the ride hash equals a ride without the sweep.
- **By hand, on the real world_ultra:** the runs in the table above, with the commands below. The
  scratch tape is 3600 lines of `frame 0x3d08850a 0x0079ffbf7f7f7f7f000000000000ff<17 zero bytes> -`:

```
DISPLAY=:0 WAYLAND_DISPLAY=wayland-1 ./target/release/openmx --headless --size 320x224 --overlay none \
  --controls original --mods ~/repos/openmx/mods --track world_ultra --play gas.tape --frames 3600 \
  --world-sweep 500,18000,115600,18000
```

## Not verified / not 1:1

- **No vehicle crossed the world.** The crossing is a sweep of the ground stream's near point, not a
  rider:
  - the bike stalls about 2000 ft out on a gas tape;
  - the parked plane is in the wrong place (MD24/MD32), and is too slow to fly there (MD33).
  So the simulation's own tile stream (`Tiled::stream`) and the dress stream (MD15) were not crossed
  with the sweep. Their memory over a crossing is **not shown**.
- **RSS is the process's resident set.** GPU memory on the offscreen RADV device is not measured
  directly. Its share shows only through the ground's room (`ground_chunks().1`), which stayed at
  20540 chunks.
- **A sweep there and back was not run.** Each line crosses once, so a piece built, dropped and built
  again on a return trip was not exercised on world_ultra. The generated-world test does not revisit
  either. Room staying at 20540 while about 95,000 chunks passed through shows dropped room is reused.
- **Tick-0 waits.** The 1249 ms wait for untamed_bt_b comes from the sweep's start, not from riding.
  It is not counted as a streaming fault.
- **The "held at most" figure is read after each frame's changes.** It is not read inside the frame,
  between an add and a drop.
- **The first sweep wrote 1468 chunks in one frame**, over CHUNKS_A_FRAME's 256. That is MD28's rule
  that a piece the near point is over goes whole: a sweep is over a piece it had not reached the frame
  before, which a rider at bike speed would not be.
- **world_mxu and world_mxvatv were not reloaded here.** Only world_ultra and world_untamed were.

## Next

None for MD23. MD33 is filed for the slow plane sim on world_ultra.
