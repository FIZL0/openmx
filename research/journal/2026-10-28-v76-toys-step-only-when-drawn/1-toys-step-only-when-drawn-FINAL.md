# 1 — toys step only when drawn (FINAL)

Task V76. 2026-10-28.

## What I did

- `crates/game-sim/src/toys.rs`: `Spec` gained `unseen` (AnimateWhenNotVisible) and `search` (UpdateSearch).
  `Toys` gained `seen: Vec<bool>`, true by default. This is not rollback state: the shell sets it between
  ticks, the way the original's draw sets the bit its next update reads. `Toys::steps(k)` returns
  `unseen || seen[k]`. A toy that does not step does nothing in `frame`: its clock and pose do not move.
  A toy with `!search` steps its clip, but its collision parts are not stood again.
- `crates/game-render/src/animated_toy.rs` reads both keys (both default True, as in the reader).
  `bound(tris)` gives the model's box: the rest-pose, finest-LOD triangles. `drawn(toys, k, box, camera,
  w, h)` returns true when any of the box's 8 corners, through the toy's place times the view's
  view-projection, is inside the clip volume on every plane, as a box-in-frustum test does.
- `crates/openmx/src/main.rs`: after the per-seat cameras, for each toy that has a box (only the `unseen`
  False ones), seen = drawn in any view. It is told to every seat's Sim, and only offline (`online` is
  None). Online, every toy steps, because no one machine's picture may steer rolled-back state.
- `crates/game-sim/tests/toys.rs` `the_visibility_gate_is_the_originals`: the original's gate against the
  port's (below).
- research/oracle.txt: a V76 block with the reader's fields, the gate, the frustum bit and the free09
  values.
- `crates/game-sim/tests/hash.rs`: Sim is 11568 bytes (+24 for `seen`). It is not walked: it is the
  shell's picture, never set online.
- Filed V105: split screen should read `<stem>_ss.toy`.

## What the original does

`./kit fn 0x002150f0` (animated_toy_update):

1. If `*(0x0041f770) +0x325` bit 3 is set, it returns at once.
2. It runs the visibility test `./kit fn 0x00215050` on the toy and on each toy in its DependentToy list
   (+0x114). The test is visible when AnimateWhenNotVisible (+0x10c) is set, or when both of these hold:
   - the root node's u16 search stamp differs from the toy's copy (+0x144);
   - root +0x111 bit 0 is set. That is the last frustum test: `./kit fn 0x001a1100` → vt +0xb8 =
     `0x00176e90`, the node's box (centre +0xe0, half +0xf0) through world × view-projection,
     `./kit fn 0x001cb5e0`.
3. It runs the AnimateWhenZoneNotVisible (+0x110) zone test.
4. If the toy is not visible, it does nothing at all. Otherwise it advances the motion, re-places the root
   with UpdateSearch (+0xf0) — and, with HasCollision, re-files every part in the world's tree
   (0x00119420 → 0x00119be0 → 0x00119d00 → 0x001ab310) — and last copies the stamp into +0x144.

The reader `./kit fn 0x002144d0` stores the defaults: AnimateWhenNotVisible 1, AnimateWhenZoneNotVisible
1, UpdateSearch 1, DisableWhenIdle 0.

On the disc:

- AnimateWhenNotVisible is False on these loaded toys: free09's five windmills, free10's windsock, one
  free10_ss toy and nat02's church bell.
- free11's balloons also write False, but under a count of 0, so they are never loaded.
- No file writes DependentToy, AnimateWhenZoneNotVisible or DisableWhenIdle.
- UpdateSearch is False only on toys whose collision cannot move, so porting it changes nothing on the
  disc today.

In first_recording's free09 state:

- all five toys have +0x10c 0;
- every root has +0x111 = 0x2, out of view;
- the toy stamps 0x8e30/0x775f/0/0/0 all differ from the root stamp 0x1244.

So the original was holding all five windmills still at that moment.

## How it was proved

- **Function oracle**, `crates/game-sim/tests/toys.rs` `the_visibility_gate_is_the_originals`. It runs
  over the free09 RAM image: the 5 toys × 8 cases (AnimateWhenNotVisible on/off × stamp moved/not ×
  in-view bit on/off). Each case calls the original's `animated_toy_update` on one toy, and `Toys::frame`
  with the same flags on the port. Every toy's clock matches bit for bit, a step copies the stamp, and the
  run counts 25 stepped and 15 gated. A deliberate mutation of the expected count turned it red, so the
  test runs rather than skips.
- `animated_toy::drawn` unit test: a box ahead, behind, past the far plane, beside, and a wide box
  reaching in.
- A live smoke run (release, `--headless`, free09):
  - from the start line, all five are unseen in all 300 frames, as the state's bits say;
  - `--trace-at 2597,2850,0` faces the windmills, and all five are seen;
  - at 90/180/270°, none are seen.

  This used a temporary print, now removed.
- The gate: `./kit done`'s first run redded `fixed_rate` (free06, which has no gated toys) under load 30+.
  Run alone, it passed.

## Not verified / not 1:1

- **What "drawn" is.** The original's in-view bit comes from its own view-projection and box test, run on
  the posed root node's box. Neither is ported bit for bit. The port tests the model's rest-pose
  finest-LOD box in host floats, through the shell's own camera projection and far plane. A toy at the
  edge of the view may step one tick earlier or later than in the original. This was not compared
  against a live capture: no PINE run was taken.
- **The stamp half of the test is not modelled.** In the original, a toy whose root node was not reached
  by the search since its last step does not step, even when it is in view. The port treats every toy
  in view as reached.
- **Online, every toy steps**, as before V76. Rolled-back sim state cannot depend on one peer's picture.
  An online ride's windmills therefore turn where the original's (single machine) would not.
- **Split screen** gates on the union of all views. The original reads `<stem>_ss.toy` there (free09_ss
  writes True, so its windmills always turn); the port reads only the base file. Filed as V105.
- **The early-return flag** (`*(0x0041f770) +0x325` bit 3) is not ported. What sets it was not traced, and
  it was clear in the state.
- **DependentToy, AnimateWhenZoneNotVisible, DisableWhenIdle** are still unread, because no disc `.toy`
  writes them.
- **The gate reads the previous frame's picture** (seen is set after the cameras and read by the next
  tick), as the original's update reads the last draw. Where the port runs more than one tick a frame,
  every tick in it reads the same frame's answer. This was not compared.
- **UpdateSearch False** skips `stand()` of the parts, but their `Moving` is still worked out. That is
  harmless because the disc's False toys' collision sits on the static root, but it was not compared.

## Next

Done.
