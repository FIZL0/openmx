# 1 — shootoff put back (FINAL)

Task S77. 2026-10-22.

## What I did

- Found why the port's put-back after a shootoff (the map edge's cannonball, reason 13) goes back to
  the edge. In a cannonball, S66's record update skipped every tick, because the original's
  cannonball branch reads a flag the port did not have. So the record kept the last riding spot,
  which is the spot just inside the edge before the throw.
- The original keeps the record on the wreck bike on every tick its collision record says it
  touched something. The put-back lands on the last such spot.
- Ported that branch as `ride::Ride::keep_back_thrown` in `crates/game-sim/src/ride.rs`.
  - S66's overhead test is factored out as `open_sky`, and its clearance test as `clearance`.
  - `clearance` returns the spot and normal even when it refuses. `clear` wraps it, so `keep_back`
    behaves exactly as before.
- Wired it into `crates/game-sim/src/rider.rs`:
  - The down branch notes the wreck bike's part point, translation and forward row when the reason
    is the cannonball and `down.bike.record.touching` is set.
  - The record is kept after the verdict and the put-backs, which is where the original's race
    manager runs it. A tick that put the bike back keeps nothing.
- Wrote the capture script `research/s77_edge_back.py`. It loads a state and steers the bike over PINE
  past an edge. It lets the pad go when thrown, holds gas after the put-back, then presses L1+R1.
  Every tick it logs, with bits:
  - the reason, `+0xc84` and the touched words;
  - the record;
  - the wreck node's row 2 and translation;
  - the part.
- Captured three shootoffs on free09 with emulator copy 2:

  | Edge  | State  | Thrown | Last touch | Clock put-back | Chord |
  |-------|--------|--------|------------|----------------|-------|
  | east  | slot 7 | 3720   | 3953       | 4085           | 4205  |
  | north | slot 6 | 4541   | 4758       | 4903           | 5023  |
  | west  | slot 7 | 4415   | 4700       | 4773           | 4893  |

  The captures are in `context/s77/`, with the two states' RAM as `slot6/slot7.eeMemory.bin` from
  `./kit state ram N --out`.
- Tests:
  - `crates/game-sim/tests/ride.rs`: `the_record_follows_the_wreck_bike_through_a_shootoff` and
    `each_captured_post_shootoff_put_back_lands_bit_exact_at_the_landing`.
  - `crates/game-sim/tests/off_the_map.rs`: `the_put_back_after_a_cannonball_lands_where_the_bike_came_down`,
    which checks the whole rider on a flat world: the throw, the touch-gated record, and the clock's
    put-back far from the edge.
- Added an S77 block to `research/oracle.txt`.

## What the original does

`./kit fn 0x0027e9d0` (put_back_keep) runs after the bike's update. Its first part is common to both
branches: the overhead test. While `+0x748 != 0` that test stands on the wreck part
`*(*(+0x7a8)+0x124)+0x40`, and a roof returns with no update.

The cannonball branch then runs when **both** of these hold:

- `+0xc84 == 13`. This is the request's copy of the reason, filled before the tick's verdict, so it
  is one tick late. On the throw's own tick it is 0, and the riding branch runs on the part point.
- `*(*(+0x7a8)+0x120)+0x40 != 0`: the wreck bike's collision record has touched something.

The branch does this:

- **Spot.** The chassis' `+0x254` node (the same node as bike `+0x18`) gives its translation
  `+0xb0`. The spot's `w` is a stack word the function never writes, which was 0 on every captured
  tick.
- **Heading.** The node's row 2 `+0xa0`, taken as `(x, 0, z, stale)` and made unit with `vrsqrt` (the
  `w` is not scaled).
- **Clearance.** It calls `put_back_clear(bike, &spot, &heading, bike+0x3d0, 1)`.
  - The normal is written straight into the record. The ground query writes it, and put_back_solid's
    lift can overwrite it.
  - So on a refused tick only `+0x3d0` changes: the west edge has 4 such ticks.
  - The mode argument is never read.
- **On pass.** `+0x3b0` becomes the spot as the clearance left it: on the ground or lifted, with no
  `+0x13a0` raise. `+0x3c0` becomes the heading flattened once (the clearance flattens it again for
  its ring). There is no `normal.y > 0.8` test.

From the captures:

- During the flight the record does not change.
- From the first touch, its x/z equal the node's x/z bit for bit on every tick, its y is the ground
  height under the node, and its `w` is 0.
- In all three captures the touched word drops to 0 for good some ticks before the clock fires. The
  node then rises away from the ground.
- So the record freezes on the last touch. The clock's put-back (clock 5.0995) places the node exactly
  there, 2137–2743 ft inside the map.

## How it was proved

Each test compares the port with three PINE captures of the original, one per edge of free09, run
over the original's own ground query and `put_back_solid` in the interpreter on the states' RAM (S66
and S75's `TheOriginalsTerrain`).

- **`the_record_follows_the_wreck_bike_through_a_shootoff`.** On every tick where the original's
  record changed while the wreck bike touched, `keep_back_thrown` (from the record the tick before)
  gives the same point (4 words), up (4 words) and heading (x, y, z) bit for bit.
  - 96 ticks moved the record: 30 east, 8 north, 58 west.
  - 4 ticks were held with only the up changed, all on the west edge.
  - 973 untouched in-flight ticks were checked unchanged.
  - 6 rows were read before their tick's update and skipped. That is allowed up to a quarter of the
    moved ticks.
  - Each row is tried with the wreck bike and part from that row and from the row before, as S66
    did, because a row can be read mid-tick.
- **`each_captured_post_shootoff_put_back_lands_bit_exact_at_the_landing`.** For all 3 edges,
  `place_back` from the port's own last kept record puts the bike's origin on the original's
  put-back node to the bit, 2266 (east), 2137 (north) and 2743 (west) ft from the edge.
- **`the_put_back_after_a_cannonball_lands_where_the_bike_came_down`.** A whole-rider check on a
  flat test world.

## Not verified / not 1:1

- **One track, not two.** The brief asked for two tracks. Every motocross save state on every
  emulator copy is on free09: slots 6 and 7 both start at the free09 spawn, and the RAM is full of
  `Free09_` strings. S66's "free06" label for slot 6 was wrong. Free06 only appears in the biplane
  state. Getting to another track means driving the game's menus to a new course and saving a state,
  which this run did not do. Instead there are three edges (east, north, west) from two states.
  The branch reads no track data except the ground and meshes, which the test answers with the
  original's own code.
- **The throw's own tick.** In the original this tick takes the riding branch on the wreck part's
  point (`+0xc84` is still 0), which puts the record at the part's x/z by the edge. The port keeps
  that tick's riding update from the sweep probe in `Ride::frame` instead. It is not ported because
  the port has no wreck part before the verdict. Any touch overwrites it. It would only matter for a
  wreck bike that never touches before the clock fires, which no capture had.
- **The put-back tick.** In the original, put_back_keep also runs after the clock's put-back with
  `+0xc84` still 13. If the touched word were still set, it would re-keep the record at the placed
  node. The port keeps nothing on that tick. Not measured: no captured put-back tick had the wreck
  bike touching.
- **The port's touched flag and wreck bike against the original's.** The tests feed the original's
  captured node, part and record. Whether the port's own `WreckBike` flies the same arc and touches
  on the same ticks is S59/S26's whole-ride question, not proved here. S59 already lists the
  cannonball flight as not a bit-exact whole-ride replay.
- **No whole-pad replay of the captures in the port.** The captures were steered live over PINE, not
  played from a tape, so the port cannot ride the same input. Only the record's update and the
  put-back are compared, tick by tick.
- **Stale lanes.**
  - The spot's `w` is the port's 0.0. In the original it is a stack word the function never writes;
    it was 0 on every captured tick, and the tests compare it.
  - The heading's `w` is the forward row's in the port, but stale stack in the original (it varied
    on every captured tick). It is not compared. It reaches only the hash: `math::basis` does not
    read it.
- **The request gate's other half.** `+0xc84` is filled only when `(bike+8 & bike+0xb84) != 0`. The
  port takes "the wreck as of the tick's start" for it, which matched the gate on every captured
  tick. The mask word itself was not traced.
- **What refused the 4 held west ticks** was not identified. The mesh query met nothing on them, so
  it was the crowding or checkpoint half of `put_back_solid`. The port refused them the same way
  through `World::crowded`, so the bits agree, but the cause was not named.
- **The overhead test's roof refusal in a cannonball** never fired in the captures, so it is
  unexercised there. It is the S66 code, which is proved riding.

## Next

Nothing for S77. A follow-up was filed with `./kit plan add` for a second track's capture once a
non-free09 motocross state exists.
