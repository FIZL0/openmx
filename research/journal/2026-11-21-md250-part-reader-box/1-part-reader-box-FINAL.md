# 1 — part reader box (FINAL)

Task MD250. 2026-11-21. Follows research/journal/2026-11-18-md250-start-spot/1-start-spot-PART.md.

## What I did

- Hooked Untamed's box function live in my PCSX2 copy (copy 2, Untamed's kit config), at the moment the part
  reader calls it for the quad, and saved a state there.
- Compared that state's node tree with the port's: the node locals and the frame were already right; the
  world rows were not.
- Added `quad::Model::untamed_worlds` and `quad::Model::untamed_half_box` (crates/game-sim/src/quad.rs).
- `made()` in tests/untamed_race_quad.rs builds the load from `untamed_half_box` at the start spot (mounts
  as before).
- Dropped the `inv_inertia` exception in `untamed_quad_resolves_its_hits_on_the_bale_as_the_original`.
- `./kit check -- -p game-sim --test untamed_race_quad`: 10 tests pass.

## What the original does

- The part reader (`./kit fn 0x004b9680`) asks for the box of the quad's scene node (`./kit fn 0x00192040`).
- The box walk (`./kit fn 0x001923a0`) visits each boxed node in the model's order, parents first, and
  brings it up to date with the world update (`./kit fn 0x0018f720`) before using its world rows.
- The world update makes only the topmost out-of-date node's frame × parent orthonormal again
  (`./kit fn 0x00131aa0`); nodes below it in the same chain are multiplied plainly.
- Since the walk goes parents first, every node is "topmost" when visited, so every model node gets that step.
  The port's `Model::worlds` composes without it: up to a few ulps per row, which at the spawn's world
  coordinates made the box 8 ulps off in y and 34 in z.
- The scene node above the model is already up to date (flags 0x81) and has no box.

## How it was proved

A save-state capture with a live hook (recipe in research/oracle.txt under MD250):
- MD160's `vehicle-atv.p2s`, with the box function's entry patched to a trampoline at 0x000f1000. The
  trampoline spins only when the caller is the part reader and the node is the quad's (0x01be8c30).
- research/md147/start.pad played to the spin through research/md160/step.sh, then saved.
- In that state:
  - all 43 node locals equal the `.psg`'s;
  - the original's box function (hook removed, run in the interpreter) gives half `4004287d 3fff2a9e
    4044d026`, the box the record needs;
  - `untamed_worlds` equals all 43 world rows it writes, bit for bit;
  - `untamed_half_box` gives the same half box, at the image's frame and at `spawn_frame(start, +z)`.
- The bale test now compares the record's `inv_inertia` with the image's and passes.

The hooked state is not committed: it is game RAM, and only my scratchpad had it. The test proof is the
bale test.

## Not verified / not 1:1

- **The start spot's words.** The game holds `43c5e147 4070a3d7 431bdc28`. Rust's parse of the scene
  file's `395.76,3.76,155.86` gives `43c5e148 … 431bdc29`, 1 ulp more in x and z. That is the game's float
  parser, not ported. The box does not care, but it is the likely cause of wheel 2's mount z being 1 ulp off
  (`c01d7080`, image `c01d70c0`). The spawn test still lets mounts differ by 2e-5. I did not check this.
- **Nodes without a box.** Every node of this quad's model has one, so a node with none was not exercised.
  `untamed_worlds` assumes every node is visited. The original skips a boxless node and updates it later,
  with its first boxed child, as the topmost node; its child is then not made orthonormal.
- **Other users of `Load::read_model` for Untamed** still use the SLUS-21104 box: the shell
  (crates/openmx/src/vehicle.rs) and tests/untamed_atv/route.rs. They are outside this task's files.
  Filed as MD261.

## Next

Nothing for MD250.
