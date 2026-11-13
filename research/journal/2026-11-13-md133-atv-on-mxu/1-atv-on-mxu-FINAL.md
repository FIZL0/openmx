# 1 — atv on mxu (FINAL)

Task MD133. 2026-11-13.

## What I did

- `quad::Ride::wreck_on(dt, world)` replaces MD123a's countdown with the quad's whole wreck test as
  `handling::wreck::wreck_test` runs it, on the quad's fields:
  - not driven (the tail's vehicle state mode not 1) and riding: no control (14), the clock at
    `MaxWreckTime`, counting;
  - driven, wrecks on: the map edge armed and not already 13, `Ride::throw` (the quad's out-of-bounds
    test and its throw back in, disassembled: see below) enters 13; otherwise, riding,
    `Ride::wreck_entries` runs `handling::step::wreck_entries` (every clause, then the hits off the
    record's contacts) and enters both answers in turn;
  - `Ride::enter` (13's three extra draws into the rates, `Call::Wreck`, the wreck rays swapped in, the
    reason, the clock 0, not counting); `Ride::clock` (the settle window 2.5 s for 13, else 0.5 s, then
    `MaxWreckTime`); `Ride::unwreck` (loads and the body's correction peak to the origin, the rider's
    stunt words 255, the backflip's torque out, the riding rays back, `put_back` at the record's spot,
    the engine's revs to idle and first gear, 13's drag divided back, the landing crossup 0.5).
  - `Ride::keep_back`: the put-back spot is S66's record (`ride::keep_back_on` /
    `ride::keep_back_thrown_on`, the bodies of `Ride::keep_back`/`keep_back_thrown` split out so the
    quad can keep its own `Back` and search cache), not the last all-four spot. The spot is the
    record's point turned to its facing and up (`math::basis`), the body a foot up the up row as the
    spawn stands it.
- `quad::Wrecked` grew the words the test reads (`Limits` from the `.phy`'s `[Wrecks]`, the edge and
  wrecks-on flags, the size word, the rider's balance/wanted/blend/trick words, racing, the landing
  crossup, the rng, the record and its cache). `Ride::rig` fills them as C28's spawn image holds them;
  the test's `md123a_rigs` reads them off the image, and `the_quad_rigged_from_its_files_is_md123as`
  compares the two whole.
- Tests (tests/atv_quad/mxu.rs): dropped level from 300 ft onto free09 → flat (5) on landing; dropped
  88° nose down → nose-dive (8) in the air; ridden on the gas west from x 200 → out of bounds (13),
  thrown back east and up, put back after the cannonball window and `MaxWreckTime`. The prop strike
  (15) still wrecks and is put back.

## What the original does

The wreck test, clock and unwreck: `handling::wreck` (oracle.txt "MD133" and the S-notes it cites).
The quad's out-of-bounds test (vt +0x16c): `./kit dis -p atv 0x004a4f20` on a scratch kit.toml; in words
in research/oracle.txt under MD133 — margin 150 on the box (and the terrain test), the throw toward the
middle at the body's height, its speed `min(2100.1, w/2) / (2·√14.9)`, lift `240/root + 16.1·root`, the
drag halved, the spin `(0.5, 0.4, 0.7)` × three draws.

## How it was proved

Headless free09 runs (tests/atv_quad/mxu.rs, `./kit check -- -p game-sim --test atv_quad -- mxu`, 9
tests pass): each wreck comes with its reason. The edge run crosses x 150 on frame 40 (from 151.43 to
149.12) and is thrown at (243.0, 155.4, 114.3): back toward the middle and up. The prop strike (15) is put
back on frame 142 at the record's spot about 80 ft off, and rides on at 31 ft/s. Its test now measures the
farthest the quad gets after the put-back, because on the full stick it circles back near the spot
within the 120 frames. The rig against MD123a's image words (`the_quad_rigged_from_its_files_is_md123as`)
holds the new `Wrecked` words bit for bit. No capture of the original wrecking a quad on an MXU world
exists to compare frames against.

## Not verified / not 1:1

- **Settles**: the original reads the rider ragdoll's "a part is touching" flag (bike +0x920, S278). The
  headless quad has no rider; the port settles on the quad on the ground or its record touching. The put-back's
  frame differs from the original's by however long the rider takes to touch.
- **No rider**: joust (7) and the no-hander (12) never fire, because the rider's words stay at their spawn values
  (stunt 255, wanted 254, balance 4, blend 0); the rider's settle, stance, riding part, rumble and the
  posts (11, 12, 13) are not the headless quad's.
- **Unwreck words not held**: +0x13e4 = 1, +0x1650/+0x119c/+0x11a0 = 0, the engine's +0xa8 (peak revs),
  the run's riding bit, +0x1784 and the hang hook; the body's rest damping (skipped as the quad's size
  word skips it on enter, assumed the same on the unwreck).
- **The rng**: the throw's and 13's draws come from `Wrecked::rng` (default seed), not the game's global
  rand state.
- **The put-back record** is MXU's S66 record kept after the frame and the wreck test, on the quad's scene
  node; the original's quad on an MX vs. ATV track keeps its race manager's respawn points. Not compared
  against any capture.
- **The terrain test** in the out-of-bounds check (0x0031d410) is `World::off_world(at, 150)`, assumed
  equivalent (MD8's port of that call).
- **The landing crossup** held at 0.5 while riding (the quad's riding code that writes +0x141c is not
  ported); the crossup clause therefore never fires on a landing.
- **The wreck test after the tail** (as MD123a ordered it), not checked against the original's frame order on an MXU world.
- **The shell's quad wrecks on its gas run**: `crates/openmx/tests/mxvatv_atv.rs`'s
  `a_picked_quad_rides_and_leaves_the_bike_ride_alone` asserted "wrecked 0 times" from before the clauses
  existed. Now the quad wrecks front (1) after about 440 ft, landing at 51 ft/s with fore-aft load −20.5
  (limit 12.5) and vertical 95. The clause and the limits are the original's (the limits match the image
  bit for bit), but no capture shows whether the original's peak loads on such a landing are this big.
  The assertion is dropped (the distance check stays); MD133b holds the loads against a capture and
  restores it.
- **Split off (MD133a)**: the wreck body's trigger probe, flags, readying and hanging, and the wreck file
  from the loader.

## Next

MD133b (filed): the quad's peak loads against a capture (see above). MD133a (filed): the wreck body's trigger probe, flags, readying and hanging, and the wreck file from the loader.
