# 1 — mxvatv car class step (FINAL)

Task MD151. 2026-11-15.

## What I did

- Ported the buggy's per-frame class step (vt `+0x488`, `./kit fn 0x004c5510`) as
  `atv_car::class_step`. It poses the model's parts off the tires and the engine, on scene nodes
  through `node::Ram`. The parts' node addresses come in as `atv_car::Parts`, the values as
  `atv_car::Posing`.
- Added the node calls it makes to `handling/node.rs` (one new section, nothing else touched):
  `position_in`, `spin_about` (`spin` with the axis by value), `turn_about`, `coarse_sin_cos`,
  `aim_along` and `stretch`.
- New test `tests/mxvatv_car_frame/class_step.rs`.
- Unhooked the class step in MD106e's test (`class_update.rs`). The original's slot now runs its own
  class step. The test runs `class_step` on a RAM overlay before the call, then checks the part
  nodes' words: equal to the port's when the port's update calls `ClassStep`, untouched when it
  doesn't. A "class step" branch tally was added (seen ≥ 30 times).
- Fixed the comment there that called the split "MD148" (it is MD151). The MD106e journal says the
  same wrong id; I left the old journal alone.
- Docs: `Call::ClassStep`, the module doc and `Ride`'s dropped-calls list now point at
  `class_step`.

## What the original does

All in research/oracle.txt `# MD151`. In short:

1. Front uprights spun about up by steer; front wheels about side by their angle.
2. Each front corner, right first:
   - the arm spun about ahead by swing × 1.25 × compression. **The left arm's axis is ahead negated
     lane by lane, (−0, −0, −1)**, built on the stack. My first reading took those stack writes as
     dead; the oracle caught it (the left arm's sine sign and row 2's `w` and `−0` lanes).
   - the knuckle spun to 0, then by the tilt of the two ends seen from the pivot;
   - the upright turned further (`turn` call: coarse sin/cos, no step between curve entries) by the
     tilt of the ends seen from the top.
   - tilt = π/2 − acos(clamp(cross z)).
3. Rear axle aimed from the raised left-rear mount to the rear pivot; rear wheels spun.
4. Each wheel stretched along `y` (inline VU, no `moved`).
5. Links (all four or none) aimed in pairs; spinners (both or none) turned by rpm × `0x3a03126f`.
6. Rod pairs aimed (odd pairs only when both are set).

## How it was proved

- Function oracle: `the_buggys_class_step_is_the_originals`, 10,000 generated cases off
  md39a/buggy.ee.bin, nothing hooked. Each case pokes steer, angles, compressions, max
  compressions, stretches, rear mount, swing, pivots, rpm, every part's position row and (half the
  time) clears its current bit. Links, spinners and odd rods are now and then missing.
  - Every word the port writes equals the original's.
  - The original wrote nothing in the part nodes that the port didn't.
  - 11 branches, each seen ≥ 30 times.
  - A flipped bit is caught.
  - The image's acos table equals `AcosTable::build`'s; the swing factor word is 1.25.
- MD106e's `the_car_classes_update_slots_are_the_originals` (10,000 cases, three slots) now runs the
  class step unhooked and checks its node words the same way.
- Mutants (each restored with cp + touch):
  - no clamp on the stretch: red;
  - `turn_about` with the stepped sin/cos: red;
  - rods aimed first-first instead of second-first: green. Both aims share one direction worked
    out beforehand, and neither rod node is the other's ancestor, so the order changes nothing
    here.
  - `moved` after the stretch: green. Every wheel was already marked stale by its spin earlier in
    the step, so the flags come out the same.

## Not verified / not 1:1

- **My own `./kit check --all` hit my 58-minute timeout before it finished** (usage limit close, no rerun). `kit done`'s gate is the full run. The targeted runs of `class_step` and `class_update` passed.

- **The buggy has no links or spinners.** The image's six pointers are 0, so the test borrows the
  first three rod pairs' nodes for them. Those branches are proved on real nodes, but not on real
  link or spinner models.
- **Rod order inside a pair** and **a `moved` after the stretch** cannot be told apart on this
  image (see the mutants). The port follows the disassembly for both.
- **The shipped ride does not call `class_step`.** `Ride` has no scene nodes, so `ClassStep` stays
  dropped. This is presentation only: nothing in the car's physics reads the parts back.
- **The golf cart's and the truck's vt `+0x488` aren't checked.** Only the buggy's is ported; those
  classes may pose other parts.
- **The 1.25 swing factor is a constant in the port.** The test checks the image's data word; a
  class with another value isn't covered.
- **`node::aim` (the axle's aim) is the existing port**, reused as is. This oracle covers only the
  way the class step calls it.

## Next

Nothing for this task. A presentation port that builds the buggy's model graph could call
`class_step` once a frame.
