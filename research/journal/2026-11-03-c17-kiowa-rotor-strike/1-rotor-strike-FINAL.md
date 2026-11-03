# 1 — the rotor strike (FINAL)

Task C17. 2026-11-03.

## What the original does

This is read off the code; the addresses are in `research/oracle.txt`'s C17 block.

- **The rotor probe** is the helicopter's `+0x458`, a ray-probe collision object.
  - Its rays are the `.phy`'s eight wreck-rotor rays.
  - Category 0x80000.
  - It never collides with the categories 0x100 and 0x100000.
  - It has no listeners.
  - Its object list holds the helicopter's own collidable, so the probe skips its own body.
- **The crash test**, while the helicopter is still flying (reason 0):
  1. It runs the rotor-in-ground test (ported in C14a).
  2. It places the probe at the measure's matrix (`+0x4d0`).
  3. It searches the probe against the helicopter's nearby list, as the main object's post last
     gathered it.
  4. The struck object is the first object in the probe's ray record. The helicopter is `CRASHED`
     if that object's category has no bit in common with the struck mask `+0x464` (0x40000 in C14's
     image).
- **The tree build.** The probe's tree is built once, when the helicopter is made:
  - Each ray's centre is (from + to) / 2.
  - One ray is a leaf. Two or more make a node, boxed over all 2n ends, and split by a cost search:
    - the longest axis of the ends' box, cut at 10/20 of its length first;
    - then cuts at 1/20 to 18/20 on x, y and z in turn;
    - a cut's cost is the two sides' box volumes, or 100 × the whole box's volume if a side is empty;
    - a cut wins only if its cost is strictly smaller.
  - A stable partition puts the rays with centre > cut first. The "above" child comes first.
  - The tree is flattened depth first. Leaf words count from 0 and are or'd with 0x8000.
  - All of this is in the oracle block word by word.

## What the port does

All of it is in `crates/game-sim/src/kiowa.rs`.

- **The tree.**
  - `ray_tree` / `try_ray_tree` is the build above. `WreckRotor::probe` is the rotor's tree.
  - `track::Node` has no radius field, so the build does not store the radius.
  - When no cut leaves both sides full, the original falls back to n/2 rays on each side, copied out
    of lists it never wrote, which is undefined. `try_ray_tree` returns `None` there instead; that
    needs ray centres within a twentieth of each other on every axis.
- **The search.** `Collision::rotor_struck` builds that tree from the rays it is given and searches it
  with `Scene::flown`:
  - at a frame made from the measure's matrix;
  - against the props and trees near the rays' reach, with the collision object's nearby list;
  - as a ray object with the rotor's category and spared list (the new `Scene::ray_of` in
    `flight_search.rs`).
  - It then tests the first hit's category against `STRUCK_MASK`.
- **`Grounded::strike`** now fills `Strike::struck` with `rotor_struck` (it was always false).

## How it was checked

- **`the_ray_tree_build_is_the_originals`** (`tests/kiowa.rs`). The original's build (`ray_probe_build`
  in oracle.txt) is called on C14's image.
  - The allocators are hooked to a bump heap at 0x01c00000, because the heap lock spins under the
    oracle. The frees do nothing.
  - 2,000 random lists of 1–12 rays: some share a hub, some are loose, and some lie on a lattice so
    that cut costs tie.
  - Each list's root, count, nodes (volume, centre, half extent, both child words) and ray order are
    compared word for word.
  - Result: 1,999 lists match, with up to 11 nodes. One list has no cut that leaves both sides full;
    the test skips such lists and allows up to 10% of them.
- **`the_rotors_probe_is_the_images`** (`tests/kiowa_capture.rs`). `WreckRotor::probe` off the disc's
  `kiowa.phy` against the probe at the helicopter's `+0x458` in C14's image: the root, the count,
  7 nodes and 8 rays, word for word.
- **The capture.**
  - **The tape** is `context/recordings/c17/strike.p2m2`, with C14's flight state as its
    `_SaveState.p2s`. `research/c14/make_tape.py` wrote it from these fields:
    `60 12:cross 80 30:l1 418` (two fields a tick).
  - **How the tape was designed.** It was planned in the port first, with a scratch simulation from
    C14's seed: hover low, then yaw left until a rotor ray reaches a trunk. That scratch test was not
    kept.
  - **The captures.** Two runs on kit copy 3 with `research/c14/capture.sh`, at the copy's
    `NominalScalar = 0.25`, which was already set and is left as found. They are in
    `context/capture/c17/`:
    - `strike.csv` and `strike-run2.csv`, frames 1204..1502, 299 rows each.
  - **The tear check.** `research/c14/compare.py` was run on the two runs.
    - Before the wreck, they differ on one row only, 1234. There, one run equals the next row (rng,
      commands and body), so that read was late. That run was renamed `strike-run2.csv`.
    - After the wreck, they differ on 9 rows, which the test does not read.
  - **The wreck.** The wreck word `+0x3d4` turns to 1 on 1298, at (3023.994, 138.584, 4053.583). On
    1299 the fall speeds up to −4.1 ft/s (wreck gravity). The helicopter lies on the ground until it
    is put back on 1449.
- **`a_captured_rotor_strike_replays_bit_exact`** (`tests/kiowa_capture.rs`). It seeds from C14's
  image as C14a does (`c14_start`, the same seeding) and runs rows 1205..1298 through `Kiowa::tick`
  with `Grounded`. The input step reads the row before's pad.
  - Rows 1205..1297: every word is compared bit for bit, and the crash test asks for nothing.
  - Row 1298:
    - The crash test asks for exactly one `CRASHED` wreck.
    - The rotor is not in the ground, the record is not touching, and `rotor_struck` is true.
    - The row is compared after `Wreckage::wreck`, including the wreck word, the wreck's damping
      and inertia in the body words.
  - The generator is not drawn.
  - With `struck` forced to false, the test fails on 1298 ("the port's crash test did not wreck").
    So the wreck comes from the rotor search alone.
- `./kit check -- -p game-sim --test kiowa_capture --test kiowa`: 20 tests passed. The `--nocapture`
  run shows no skips.
- `cargo check --workspace --tests` builds.
- `./kit audit` lists only the two old `plan/DONE.md` lines.

## Not verified / not 1:1

- **What the original struck is not captured.** The test proves that the port wrecks on the same
  tick, with every word equal, and that only the rotor search can have wrecked it. That the
  original's struck object is tree 138's trunk too is inferred, not read: the spec does not log the
  probe's ray record.
- **The frames after the wreck are not replayed.** These are the blades thrown (`Wreckage::wreck_mx`
  draws `rand()` and starts the blade bodies), the wrecked fall, and the put-back on 1449. The test
  stops at 1298 and applies only the base `Wreckage::wreck`.
- **Only the first hit is tested.** The original reads `objects[0]` of the probe's ray record after
  the search, and only when a probe flag bit (`+0x116 & 0x20`) is set. The port takes the first hit
  of `Scene::rays(true)`. The two agree on this capture, but nothing checks the order when the rotor
  touches two objects at once (say a tree and a prop).
- **Terrain is not a candidate.** In the port the terrain is not in the nearby list. A rotor ray that
  meets the ground is left to the rotor-in-ground test, as the original's search sees only objects
  too. This is not checked against a capture that tips the rotor into a slope.
- **The helicopter's own collidable** is in the original probe's object list, so the probe skips its
  own body. The port's world never offers the helicopter's own body as a candidate, so there is
  nothing to skip.
- **Nearby list timing.** The port searches with the record's nearby list as the frame has it (the
  post's refresh). The original uses `heli+0x16c` as last gathered. They agree on this capture, but
  no one-tick-late case has been tested.
- **The empty-split fallback** is undefined in the original and not ported. It is never reached on
  the rotor's rays.
- Only free09's trees are struck here. The struck mask (0x40000) is never shown to spare anything,
  because no object in that category is near.
