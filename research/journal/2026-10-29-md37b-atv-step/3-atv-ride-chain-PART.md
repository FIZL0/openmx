# 3 — the ATV ridden frame after frame (PART)

Task MD37b, reopened after MD74, MD75 and MD76 landed. 2026-11-05. Slot s2.

## What I did

- Looked for MD37a's captured rides to replay. **They are gone**: `context/capture/md37a/*.csv` and the
  four `.p2s` states are in no slot (s1–s7), not in `kept-context`, and in no PCSX2 copy's savestates
  (copy 2's directory itself is gone). Only `atv-r-spawn.ee.bin`, `MC02.esb` and `mc02.res` are left, in
  s1, s2 and s3. Filed **C28** to re-make them by MD37a's recipe (M21's state 04 still exists in
  `~/repos/ps2-decomp-kit-slots/pcsx2/s9/PCSX2/savestates`).
- In the meantime I used the original itself as the per-frame oracle, as F31 did for the bike. MD75e's
  whole-frame proof is now chained over whole rides: `tests/atv_quad/frame.rs` gained `chain`, plus two
  tests:
  - `the_spawned_quads_ride_is_the_originals_frame_after_frame`: 240 frames of `ride_pad`. Gas from rest,
    full left then full right lock at speed, the stick forward then back, the brake to a stop, then
    nothing.
  - `the_spawned_quads_run_into_the_palm_is_the_originals_frame_after_frame`: 300 frames of gas, the stick
    steering closed-loop (`at_the_palm`) toward the palm ahead of the spawn (MD37a's obstacle at
    x 2965, z 3258).
- Filed **MD104**: the ATV's whole per-frame sim update. That is what still stands between this chain
  and the strike, the wreck and the put-back. Noted MD98 (the record's collide), which MD104 includes.

## What the original does

- Each frame, SLUS-21104's world step (`atv_world_step` over the image's world `0x0065bc0c`) runs the
  quad's three listeners and four substeps. The chain carries everything from frame to frame on both
  sides: body, quad, wheels, scene nodes, collidables, the gather of the nearby list (`atv_search::Oasis`
  answers `Gather` each frame) and the input limiter. In the original the input limiter reads the raw
  levers `+0x314`/`+0x310` and the stick `+0x12a8`/`+0x12ac`. Those are F31's pad reader stores, and the
  test lays them before each frame.
- **The world step does not strike the palm.** Steered at it, the quad passes 0.37 ft from the palm's
  centre at up to 76 ft/s. On both sides the collision record ends every frame with 0 contacts, and the
  wreck word `+0x6e4` stays 0. So the strike seen in MD37a's race-wheelie-launch (wreck at ~79 ft/s) comes
  from outside the world step. It comes either from the sim object's update list (vt `+0x7c`/`+0x84`,
  where the record's collide (MD98) should be) or from its tail.
- The ATV image has the same sim object as F31's state 4: vtable `0x003b9320`, vt `+0x40` = `0x00204040`.
  Its `+0x6c` is the world `0x0065bc0c` (object at `0x0065bba0`; the vtable word is at `+4`). MD104 can run
  F31's frame on it.

## How it was proved

- `./kit check -- -p game-sim --test atv_quad -- spawned_quads_r --nocapture`: both tests passed, and
  they ran (no skip). In each test, the original's world step and the port's `quad::frame` run once per
  frame at dt bits `0x3d08850a` (F31's 30 Hz tick), and both ask the ground the same number of times.
  After every frame, every word `image::compare` reads (MD75e's 2,923 words of quad, body, wheels, nodes
  and collidables) is equal, and so is every word of the rigid body. The port hands on only
  `GravityRecord` and `PartMove`, as in MD75e.
  - Ride: 240 frames, 14,947,413 instructions, 3,840 ground queries. Top speed 66.2 ft/s. A wheel was off
    the ground on 13 frames, and the bars were steered on 67. Vehicle state 1 throughout.
  - Palm: 300 frames, 19,139,889 instructions, 4,800 ground queries. Top speed 76.6 ft/s, a wheel off on
    20 frames, steered on all 300, nearest 0.37 ft from the palm.
- The single-frame mutation check (one bit of `lean` goes red in exactly one word) is MD75e's, and the
  chain uses the same comparison.

## Not verified / not 1:1

- **No captured ride is replayed.** The brief's test, MD37a's csvs replayed bit-exact, cannot run: the
  csvs and states are lost (C28). The chain uses the original's code in the interpreter as the oracle,
  not PCSX2 rows. The interpreter matching PCSX2 is F31's result for the bike's frame, not shown here for
  the quad.
- **Only the world step, not the whole frame.** The sim object's update list and tail (the quad's
  update, animate, the pad reader, the record's collide) do not run on either side. So no strike, no
  wreck on WreckATVm.col, no put-back and no net unpack are reached in a ride, even though each is proved
  on its own (MD64a, MD65, MD65b/c, MD75e1). That is MD104.
- **The pad is laid, not read.** The raw lever and stick words are written on both sides. The pad reader
  (F31's `0x004e36c0`, for the bike) was not run on the quad, and the d-pad, trick byte and clutch lever
  words were not laid.
- **Only the atv_ (Race) quad.** The atv_s_ images are lost with the states (C28).
- **Never airborne for long.** The rides leave the ground on 13 and 20 frames (bumps). A thrown or ridden
  jump (MD37a's `-air` states) is not chained, so the air state's longer paths (the backflip, landing)
  are proved only by MD62/MD75's single-frame oracles.
- **What carries but is not compared:** MD75e's own gaps hold on every frame (the searched bit on the
  quad's own collidables, the 40 scene nodes past `atv_moto::NODES`, the other five listeners, the
  `PartMove` boxes). On frames after the first, those uncompared words also feed the next frame of the
  original. The chain shows they leave the compared words alone over 540 frames, but they are not
  compared themselves.
- **The palm's position** is MD37a's "(2965, 3258)", read as x, z. The ride passes within 0.37 ft of it,
  so that reading is right, but the palm's collidable reach (23.5/28.6) was not checked against it.
- "Every bike replay hash unchanged": nothing outside a test changed. Only `tests/atv_quad/frame.rs`
  gained code.

Nothing was taken from opentennis: its game has no vehicle physics.

## Next

MD37b is blocked after C28 (the captures) and MD104 (the whole sim frame). When both land: run MD104's
full-frame chain off the re-made `atv-{r,s}-{spawn,air}.ee.bin`, feeding the pad reader's stores from
each csv row as F31 does (`tests/mxvatv_bike_ride.rs` `hooked()`/`replay()`), and compare the port with
the rows.
