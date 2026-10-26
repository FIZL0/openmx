# 1 — eco tree sway (PART)

Task S101. 2026-10-26. The S97 follow-up: "port the per-frame ecosystem pass's per-plant state block
(bend angle, states < 2 and 4, the flag bit) so a hit tree bends like the original's".

## What I did

- Found that the original's trees never bend in MX Unleashed. The bend blocks exist in the code, but
  nothing ever sets one up. There was nothing to port, so no simulation code changed.
- `crates/game-sim/src/eco.rs`: a "No sway" bullet in the module doc saying so and why, so nobody
  files it again.
- `crates/game-sim/tests/eco.rs`: two proofs (below) and a `bend` offsets module.
- `research/oracle.txt`: the S101 block (pool, block layout, keys, readers, the writer scan).
- Took nothing from opentennis (no ecosystem there).

## What the original does

- **The block pool.** The scene loader (`./kit fn 0x001a5210`) calls `./kit fn 0x0012b990` only when
  the scene's environment record has `UseEcoPhysics` (+0x528) non-zero. That call sets eco +0x1d88,
  allocates `EcoMaxPhysicsEco` (+0x52c) blocks of 0x50 bytes at eco +0x80 (count at +0xe8), keeps a
  nearby-gather object at +0x1d90, and makes a radius-10 collision object at +0x1d8c. The block
  constructor at 0x0012bb70 (not a Ghidra function; `./kit dis 0x0012bb70 0x0012bbd0`) zeroes the
  block and gives it two quads from rodata.
- **The keys.** The environment reader (`./kit fn 0x001a7b30`, decomp near the 0x42b688 string)
  reads `EcosystemWindStrength` (default 0), `EcosystemWindGustRate` (0.15, only if strength is
  given), `UseEcoPhysics` (an int/bool, `./kit fn 0x0014b6f0`, default **0**) and
  `EcoMaxPhysicsEco` (50, only if Use is given). It reads them only when the scene names an
  ecosystem file.
- **The readers.** The frame pass (`./kit fn 0x0012bbd0`) checks plant +0x12 bit 1 and then reads
  block [+0x13]. State +0x40 < 2 scales the fade byte by the bend angle at +0x30 (over 5° near, or
  (angle − a rodata constant) over 2° far). State 4 takes the fade byte from +0x30. The quaternion at
  +0x10 rotates the plant's culling box. The near-model draw (`./kit fn 0x0012d030`) reads it too.
- **No writers.** I disassembled the whole ELF (llvm-objdump into the scratchpad) and checked every
  `sb` at +0x40 (14 sites) and every `sb` at +0x12 (24 sites) and its +0x13 neighbours. Each one
  belongs to the block constructor, the plant constructor 0x001293e0 (clears bits 1 and 2, +0x13 = 0),
  read_plants 0x0012b470 (bit 0 only), the frame pass (bit 0 only), or a struct that is not a plant
  (0x0019a880 fills 0x10c-byte records; 0x0019b4e0 stores the same way from its stack). The decompiled C has no `| 2` on a plant's +0x12
  and no store into a block. The collision dispatch that hits a group's part (`./kit fn 0x0011a150`,
  `./kit fn 0x0011a9d0`) sets the part's +0x78 to the plant and calls the usual listeners. No
  callback is attached to the trunk collidables (`./kit fn 0x00129d80`).
- So even a mod that turned `UseEcoPhysics` on would get a pool of blocks that stay at their
  constructor values, with no plant pointing at one. A hit tree stands still.

## How it was proved

- **Disc:** `no_scene_on_the_disc_turns_the_ecosystems_physics_on` reads every `.scn` in every `.res`
  of MX.PSR. None names any of the four keys. 31 scenes name an ecosystem (free06/07/09/10, their
  `_ss` copies and the nationals), so the reader's defaults hold everywhere: physics off.
- **Save states:** `the_originals_ecosystems_hold_no_bend_state` reads recording2 (free09 ridden),
  biplane (free06 flown) and s89-along (free06 ridden). In each one eco +0x80, +0xe8, +0x1d88,
  +0x1d8c and +0x1d90 are 0, and no plant (726 / 5600 / 5600) has bit 1 set or a non-zero +0x13. A
  scratch scan of all ~55 RAM images under context/ found the same.
- I forced both tests to fail once to see that they really ran. Raising the thresholds printed
  "31 scenes with an ecosystem, none with its physics on" and "recording2…: 726 plants".

## Not verified / not 1:1

- **No live hit.** I did not drive the original into a tree and watch the plant bytes. The proof is
  static (no writer anywhere in the ELF) plus RAM images taken outside a tree hit. With the pool
  pointer null in every state, a hit could not write a block without the loader allocating one first.
- **The writer scan is a pattern scan.** It covers byte stores at the offsets and the decompiled C.
  A store through a computed offset or a block-wide memcpy would slip past it. The RAM states (no pool
  at all) cover MXU's own scenes either way.
- **Mod scenes with `UseEcoPhysics=1`** would make the original allocate the pool, the gatherer and
  the radius-10 probe. The port models none of it. By the scan they have no visible effect, and the
  collision object's own response was not read.
- **Wind** (`EcosystemWindStrength`, 0x0012b8f0): not ported. No disc scene sets it, and it only
  feeds the eco object's +0x1d60.. fields, which this task did not trace into drawing.
- MX vs. ATV's and Untamed's scenes and code were not checked. Their engines may well use these
  blocks.

## The gate

`./kit done` (with `--skip-audit` for the pre-existing plan/DONE.md leaks) reds on one test only:
`game-sim --test replay` `the_dallas_rail_rides_replay_from_their_save_states`, which fails with
"along: rider.ride.front.slip.locked agrees through tick 278, was 279" (replay.rs:7190). I ran it on
bare HEAD 3f809428 in a scratch worktree with s3's context, and it fails the same way there. This is
S120's s3-only lane. S107 (merged on main as b68f3977, after this branch's base) re-pins it. This
task touches no replay code. So S101 is committed and pushed by hand, not ticked.

## Next

Nothing for MXU. After the merge, rerun `./kit done S101` on a main that has S107 to tick it.
