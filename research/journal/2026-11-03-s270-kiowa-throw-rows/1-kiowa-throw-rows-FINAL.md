# The throw's rows, off the model's nodes

## What the original does

I read the model tree out of S148's RAM image (heli 0x007ccae0). Each node has:

- its local rows at +0x40 and its world rows at +0x80;
- its parent at +0x100 and its name at +0x120;
- a hidden bit, bit 6 of byte 3.

The chain is:

- heli+0x18c is "Kiowa", the root, with no parent.
- heli+0xd04 is "RotorSpindle".
- "Rotor" hangs under the spindle.
- heli+0xd08..+0xd14 are `Blade_Node01..04`, each turned 0/90/180/270° about y, about 2 ft out.
- The blur and bent nodes (+0xd1c.., +0xd3c, +0xd40) are roots of their own. kiowa_blade_spin copies
  the blade and tail nodes' world rows into them.

The frame runs in this order:

1. **The post.** kiowa_measure's first step is body_wrapper_post. It hands transform_set_pose the root,
   the body's last orientation (+0x150), and the last position less the turned CofG. That is
   `Body::pose()`.
2. **The crash test.** It runs next, from the measure's vt +0x1b4, and the wreck reads the nodes' rows here.
3. **The rotor step.** kiowa_measure_mx then runs kiowa_blade_spin. While +0xd1c is shown, it:
   - adds 27·dt to +0xe58;
   - calls node_set_rotation(spindle, angle, (0,1,0)) and the same for the tail about (1,0,0);
   - takes each blade node's world matrix (node_world_matrix). That rebuilds the chain lazily, and
     each matrix is copied into its blur node.

   **When +0xd1c is hidden, the step does nothing at all.** The spindle keeps its last turn, and the
   angle keeps naming it. Only this step and the constructor (which zeroes it) write +0xe58 in the
   helicopter's code.

So on the wreck frame, the blade rows are the post's root and the previous frame's spindle turn,
rebuilt down the chain. In the port, that is `ride.rotor_angle` at `Surroundings::crashed`.

node_set_rotation (0x001b2e50 → 0x001b2db0) works in four steps:

1. It keeps the position.
2. It reads `{sin, cos}` off the curve table (curve_sincos 0x00110ff0, the same lookup as
   `CurveTable::at`).
3. It builds the axis-angle matrix on VU0 (axis_rotation_build 0x00110f40):
   - the FPU normalises the axis (adda, madd, rsqrt);
   - `ACC = v + vf0.x`, then `vmsubx` with cos gives `v(1 − cos)`;
   - that times each of v's lanes gives the three rows;
   - cos goes on the diagonal, and `v·sin`'s lanes are added on one side of it and taken off the other;
   - row 3 is vf0.
4. It writes the position back.

transform_rebuild does each axis row as `p0·x + p1·y + p2·z` (vmula, vmadda, vmadd) and the origin
as `(p3 + vf0.x) + …`.

## The port

- `RotorTree::read` reads the `.psg`'s node records. It reads the name, the matrix at byte 128 and the
  parent at byte 220, then finds the spindle and the four blade nodes, which must hang under it.
- `Throw::of(tree, body, angle, curve)` builds the rows:
  - the model is `quat_to_mat4(body.pose())`;
  - the spindle's local is `axis_turn((0,1,0), curve.at(angle))`, with the file's position and w 1;
  - each blade's rows are `rebuild` down its chain from the root.
- In the shell, `KiowaRide::with_model` reads `kiowa.psg` off machine.res, and `KiowaAround::crashed`
  throws from `Throw::of`. With no model, it falls back to the old measure-matrix rows.

## How it was checked

`./kit check -- -p game-sim --test kiowa -- rotor_tree throws_rows`: 2 passed. I also ran them with
`cargo test --nocapture` to confirm that nothing skipped.

- **`the_rotor_tree_is_the_live_helicopters`** walks each blade's chain to the root in RAM and in the
  file. The rows match bit for bit for kiowa.psg and kiowa2.psg, except that the spindle's turn is the
  rotor's and only its position is compared. The live chain matches node for node, the root is
  heli+0x18c, and the spindle is heli+0xd04. kiowa_ss.psg and kiowa2_ss.psg also read, but their blade
  nodes go round the other way and sit a few bits off, so they are not compared.
- **`the_throws_rows_are_the_originals`** has two parts:
  - **The saved frame.** `Throw::of` on the state's own body and +0xe58 matches the RAM rows of the
    root and all four blade nodes bit for bit.
  - **10,000 generated cases** (2,508 with the rotor hidden). Each case makes up the body's last
    orientation, last position (up to 30,000 ft) and CofG. It then calls transform_set_pose with
    `Body::pose()` and kiowa_blade_spin with a generated angle and dt. Hidden cases keep the angle, as
    the game does. Finally it calls transform_rebuild on each blade node and the root. The angle
    (`spin_blades`), the model rows and the four blade rows all match bit for bit.
- **The shell:** `./kit check -- -p openmx --test kiowa_switch` passed all 4 tests. These include
  `a_landing_at_speed_wrecks`, which throws through the new path.
- **`./kit audit`** reports only the two existing plan/DONE.md leaks.

## Not verified / not 1:1

- **The post's origin is not run through body_wrapper_post here.** The test hands transform_set_pose
  the port's `Body::pose()`. The biplane's `the_pose_is_put_like_the_original` proves that origin
  against the same post.
- **Only the y axis is exercised.** The tail's x-axis turn is not ported, because the throw does not
  read it, so axis_turn's normalisation has only ever seen a unit axis.
- **A throw before the rotor's first step.** `Throw::of` would turn the spindle by `rot(0)`. The original
  would still hold the file's spindle rows, which are the identity. I have not checked whether rot(0)
  off the curve table is exactly the identity.
- **The body must be enabled.** A disabled body's post does not move the root, but `Throw::of` always
  takes `Body::pose()`. The wreck is not known to test a disabled body.
- **The `_ss` skins are not supported.** The shell loads only kiowa.psg, while the livery (P216's
  `livery`) can pick kiowa2. That skin's chain to the blades is the same, rows and all.
- **No blade bodies are built yet (P265),** so in the shell the new rows still move nothing.
