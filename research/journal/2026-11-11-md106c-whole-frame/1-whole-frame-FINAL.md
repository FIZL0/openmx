# 1 — whole frame (FINAL)

Task MD106c. 2026-11-11. (Earlier sessions on this task: `2026-11-10-md106c-mxvatv-car-step/`.)

## What I did

- Added `whole::the_buggys_whole_frame_is_the_originals` to `crates/game-sim/tests/mxvatv_car_frame.rs`.
  It reads MD39a's buggy image whole (car, body, engine and its keys/state, tires, vehicle) and runs one
  frame of the original's world step, from the preforce list onward. The ground query is the only hook on the
  car's own path, and it is answered through `atv_ground` on MC02 (`md37a/mc02.res`). The test then runs
  `atv_car::frame` with `Own` → `Pieces` → a test `Oasis` Steps, which answers GroundUnder, TireGround and
  Noise from the same terrain and the image's noise tables.
- Result: 1972 words are equal: car, body, engine, every tire, vehicle and the noise knot. The original's
  5 ground queries (4 tire probes, then the post query with no record) match the port's in order and point.
  The frame wrote 4394 bytes in 51620 instructions:
  - 2386 bytes from the car's own code
  - 895 more from its hand-outs
  - 1125 from the three other preforce listeners (36 bytes shared with the car's hand-outs)
- Checked first that the image's terrain object is `Terrain::of(MC02)`: box, margin, height scale,
  per-sample/coarse grids, split. Also checked that every tire queries that same world.
- `atv_car.rs`: only the module doc's Proof line changed.
- `research/oracle.txt`: a `# MD106c` block covering the terrain object, the listeners, the hand-outs, the noise words
  and the shared bytes.

## What the original does

The world step walks the preforce list (count at world +0x98, array at +0xa0) and calls each element's
vt slot +8 with the element. There are four entries. The car's own entry comes first (car +0x30). Next is
the idle second buggy (vstate mode 2, which never asks the ground). Last are two vehicles of a separate class
(`./kit fn 0x4d8b20 -p atv`, reached through a thunk at 0x004de0c0). Each makes 4 probe ground queries and
writes only its own blocks plus three shared statics.
The car's entry is what `atv_car::frame` ports. Beyond the ground query, it calls out to the
ray sort place/search, the record's gravity call (record vt +0x88, four times), subtree-out-of-date,
spatial update, node bounds, collision sync (writes the node's +0 halfword) and the map-edge test.

## How it was proved

A bit-exact run of the whole frame, from a save-state image (MD39a's buggy on MC02), in the Ee interpreter
against `atv_car::frame`. Four original runs attribute every byte:
- A: the whole frame.
- B: only the car's listener.
- bare: B with the hand-outs hooked to nothing.
- The other three listeners alone, in list order, on a fresh image.

Asserts:
- Every byte A wrote is B's or the others', with A's value.
- The others touch no compared word and nothing in the car's blocks.
- bare writes only compared words and equals A there, so the hand-outs do not change the car.
- The hand-outs write no compared word, and inside the car's blocks only the node's +0 halfword.
- A flipped bit turns the comparison red at exactly its address.

The file's 19 tests pass.

## Not verified / not 1:1

- One frame from one state: the spawn state, grounded, near-still, riding, the sort striking nothing, no shift,
  landing or wreck. Other branches rest only on the earlier generated-state tests in the same file.
- dt is 1/30 (as the quad's MD75e). The game's own dt for this frame was not read.
- The hand-outs are not ported. Their answers in this frame were checked: the search struck nothing and the
  edge test said inside. The car's words are unchanged with them hooked to nothing, but their 895 written
  bytes, including the collision sync's node +0 stamp, are not compared with any port.
- The other three listeners are accounted for by attribution (run alone, same bytes and values). Their own
  contents are not ported or proved. 36 bytes are written by both them and the car's hand-outs, so either value
  is accepted there.
- The port's probe query passes a ZERO normal input where the original passes its stack buffer. The two are
  equal here because the query always writes a normal on the map. Off the map they could differ.
- Only the buggy image was run, not the baja or the truck.
- The car's update (vt +0x10c) runs outside the world step and is not part of this proof.
- The noise tables are read from the image, not built by the port's builder in this test.

## Next

Nothing for MD106c. Possible follow-ups: port the hand-outs (sort/search, edge test, collision sync), and run
a moving or airborne frame and the baja/truck images.
