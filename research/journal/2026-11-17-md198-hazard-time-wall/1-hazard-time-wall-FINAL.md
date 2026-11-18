# 1 — hazard time wall (FINAL)

Task MD198. 2026-11-17.

## What I did

- Ran untamed_quad_replays_md147_hazard_time_through_the_put_back to the end of hazard-time.csv (row 369).
  - No edge throw at row 238 any more. MD168b (b99d73efb) seeds `Wrecked::edge` from the image's word
    (SLUS-21104 `+0x308`), which is 0 in every MD147 state. The world box (`terrain +0x40`) was already
    the image's own, so there was no "nearer box" left to fix.
  - So the "held" re-step in `replay` no longer needs to switch the edge off. I took that out; the
    stuck-time hold for the put-back rows stays.
  - The first off row was now 256: the original's x velocity turns round on row 255 and it wrecks
    (`wreck` 1 from row 256), while the port's quad drives on through.
- Found what the original hits: **the stadium's side wall.**
  - The scene's `[CollisionObjects]` (SX_A.scn.bpb, decoded with openmx-mods `tools/bpb`) places
    `FP01A_A.col` at (456.39, −0.14, 455.57). That mesh is the stadium's shell (575 × 893 ft, 118 tris).
  - Its triangles near the strike make one vertical face from (219.24, 104.62) to (204.27, 159.30). At row
    254 the quad is about 2 ft from it, heading into it.
  - Nothing knockable is there. The nearest models are `Banner_Stadium16` (no collision file, and not
    a PhysicsCollection in SX_A.toy) and `TripleBale04`, 15 ft away on the far side.
- The test's Rocksbury world now has solids: `solids()` reads SX_A.scn.bpb, parses it as a scene
  (`track::Scene::parse`, which applies the Free Ride activity), and places its collision list with
  `world::Props::load` from `context/untamed/sx_a/col`. One file is missing from SX_A.res:
  `FP01A_E.col`. The test asserts that this is the only miss.
- With the wall in, the port's quad hits it on row 255 too and wrecks (reason 15), but its bounce is not
  the original's (row 256: 35 body words off, x 214.06 vs 214.58, y 3.14 vs 3.59). So the test stops at
  row 254. MD198a is filed for the strike.
- steer, hazard-on and the spawn test still pass with the solids in (all 4 tests in untamed_race_quad).
- opentennis: nothing relevant (no Untamed scene or wall contact there).

## What the original does

On hazard-time's run back along the ground after the put-back, the quad drives into the stadium's west
side wall on row 255. The row is a mid-tick read. The body is pushed back +x and up (y 1.55 → 3.59 by
row 256), and the wreck word is set from row 256.

## How it was proved

- Replay of MD147 hazard-time.csv, body words compared bit for bit on every well-read row: rows 0–254
  match (`untamed_quad_replays_md147_hazard_time_through_the_put_back`, crates/game-sim/tests/untamed_race_quad.rs).
  The skipped rows stay under 4, and the only row read before a put-back is still 180.
- The wall is the thing struck because: without solids the port drives through flat ground on row 255;
  with the scene's solids the port also turns round on that tick and wrecks.

## Not verified / not 1:1

- **Rows 255–369 of hazard-time.csv.** The port's quad bounces off the wall differently from the
  original's (35 words off from row 256). MD198a.
- **Wreck reason.** The capture's `wreck` word says only "wrecked", not why. That the original's reason
  matches the port's 15 is not checked.
- **`FP01A_E.col` is not in the extracted SX_A.res.** The scene places it at the same spot as FP01A_A. If
  the disc has it somewhere else, the world is missing a mesh. It is not touched by any row that matches.
- **The scene's other solids (towers, gates, lights, floor mats) on Untamed.** They are placed through MX
  vs. ATV's collision reader and scene placement. No matching row touches them, so whether Untamed places
  them the same way is unproven.
- **The bpb → scene-text round trip.** Values go through `bpb::ini`'s text and then the scene reader.
  For the wall's position it gives the scene's decimals. Other keys were not checked one by one.

## Next

MD198a: capture rows 253–258 of hazard-time with the quad record's contact list and the wreck words. Find
which of the wall contact, the record's push-out or the reason-15 wreck impulse differs. Then extend the
test to row 369.
