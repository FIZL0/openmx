# 1 — rider scene capture (FINAL)

Task S279b. 2026-11-14.

## What I did

- **Found a tape in the interpreter first.** SLUS-21104's whole frame replays bit-exact in `Ee` off
  state04 (F31), and the pad reader's words it is fed (`READER`) depend only on the pad that tick. The one
  exception is during a wreck, when they all read 0, `+0x14d4` included. I checked this over every row of
  ride-2 and flip-1. So I made up rows from candidate tapes and rode them in the interpreter on `PortScene`,
  using a scratch test (removed again).
  - flip-1's pool showed the nearest scene meshes ~300 ft off the C15 line.
  - Gas with full left for 25 ticks (tick 5..30) ran into a nat05 mesh. Pairs, hits and contacts came
    from row 189.
- **`research/c15/scene.pad`**: that tape.
  - Made into `context/recordings/c15/scene.p2m2` (+ `.ticks.csv`) with `tape.py`.
  - Captured twice in copy 1 with C15's `play.sh`, 300 rows each, as `context/capture/c15/scene-{1,2}.csv`.
    The scratch kit.toml was `context/c32-atv-kit.toml`; the state was s9's `SLUS-21104 (95BB1901).04.p2s`,
    md5 73389cd1….
- **`tests/mxvatv_bike_ride.rs` `the_bone_collisions_scene_is_the_ports`** now also runs `scene-1`.
  - Its hook on the port's side counts what the scene gave each part: the sphere's contacts (and whether
    the part's other was set), or a swept push-out.
  - On scene-1 it asserts searches hit, sphere and sweep pairs with meshes, sphere contacts with the other
    set, and swept push-outs. ride-2 and flip-1 still assert no hit.
- **The matrix stack.** The first run parted the two sides on row 189, but only in the scratchpad,
  0x70000080..0x700001c4. Every row was still the capture's.
  - A temporary write watch in `Ee::write` (reverted) found the writer: the atv program's mesh sweep
    (`./kit dis 0x0010c080 -p atv`). It pushes 64-byte matrices on a stack whose top is the word at
    0x0038d358, base 0x70000000, and pops them again.
  - The port works its frames out in Rust and keeps no such stack. So the test now asserts the top is back
    at the base after every frame on both sides, and leaves the dead levels (0x70000040..0x70000400) out of
    the compare.
- opentennis: nothing to take (game physics, and a capture recipe this project already had).

## What the original does

- **The ride** (scene-1, row = tick after the state):
  - The bike turns left (heading −1.97 → −0.45 by row 48) and gets air on rows ~120–170.
  - It reaches 93 ft/s and hits the mesh near (2421, 402, 3348): the wreck count goes 0 → 2 on row 188,
    and gas reads 0 from row 189.
  - The bike's speed falls 76.6 → 7.9 on row 196. It is put back around row 257, and gas reads 1 again by
    row 273.
- **What the port's scene did on scene-1:**
  - 2,517 places and 2,308 segments.
  - 1,716 sphere pairs and 1,313 sweep pairs, all with kind 0 meshes (no plant, box or hierarchy pair).
  - 224 searches hit: 209 sphere hits, each with one contact and the mesh as the other, and 15 swept
    push-outs.
- **Matrix stack**: `./kit fn 0x0010c080 -p atv`. Each level is pushed by copying the current top up 0x40
  (`*0x0038d358 += 0x40`) and popped on the way out. scene-1 goes 8 levels deep.
- Detail: `research/oracle.txt` `# S279b`.

## How it was proved

- **Two PINE recordings of the tape**: `research/c15/same.py scene-1.csv scene-2.csv` gives 300 rows,
  identical. Vsync runs 13109..13707 in steps of 2, with no gaps.
- **Bit-exact replay fixture, side by side** (`the_bone_collisions_scene_is_the_ports`):
  - scene-1 runs on two interpreters, one with the original's scene and one with `PortScene` (the port's
    place, refresh, clear, pool search, S279a's pair, and F51's sweep).
  - All 300 rows match the capture on both sides.
  - After every frame, RAM outside the stack and the scratchpad are equal, apart from the dead
    matrix-stack levels.
  - Nothing was handed to the original.
  - ride-2 and flip-1 still pass, with the same counts as S279.
- **Mutation check**: I negated the contact normal in `atv_sphere::pair`'s record write. The test fails on
  scene-1 row 189, where the two RAMs part at the record pointer, the parts and the bike. ride-2 and
  flip-1 never reach the pair, so they could not have caught this. The file was restored with cp + touch.

## Not verified / not 1:1

- **Only meshes are hit.** Every pair on scene-1 is a kind 0 mesh. No plant sphere, box (kind 6),
  sphere/sphere (kind 5) or hierarchy (kind 1) pair runs in a capture. Those rest on S279a's oracle alone.
  nat05 has no boxes or hierarchies (S279a), and I did not hunt for a plant.
- **The record never holds more than one contact on scene-1.** 209 hits gave 209 contacts, so the
  push-out loop over several contacts, and contacts claimed from older entries, are not seen live.
- **A mover on the other side** (the push-out's spin and velocity virtuals) is not reached. A mesh is no
  mover, so `PortScene`'s two `OriginalScene` hand-offs still never run.
- **The sphere's whole-scene search** (no pool) still never runs. The rider always has a pool.
- **Matrix-stack levels are left out of the compare.** They are dead by the stack's discipline (pushed
  before they are read). I read that from the code, but did not prove it for every reader of the
  scratchpad. The window is 16 levels; scene-1 uses 8.
- **Which mesh it is** (a rock, a tree trunk, a building) was not identified. It is a kind 0 collidable
  near (2421, 402, 3348), not looked at on screen.
- **The rows after the put-back** (≈257..299) are in the capture and pass, but nothing in them was looked
  at beyond the test.
- Captures and the tape are only in the shared `context/` (git-ignored). The pad script is in git.

## Next

Nothing for S279b. A plant or a mover on the other side of a contact needs a tape of its own.
