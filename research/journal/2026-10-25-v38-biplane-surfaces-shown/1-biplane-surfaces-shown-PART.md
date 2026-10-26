# 1 — biplane surfaces shown (PART)

Task V38. 2026-10-25.

## What I did

**Why nothing moved.** Of the brief's four suspects, it was the second: the surfaces were never handed to
the draw. `game_sim::biplane_anim::surfaces` (S35) had no caller in the shell. The shell drew the plane's
model at its bind pose in both renderers, with only the prop cone turned (`Biplane::turned_parts`, P31).
The stick did reach the plane's control state, so the plane flew right and only its picture stood still.
The node names were fine: the disc's `biplane.psg` has all eight of `biplane_anim`'s bones by those names,
every one a direct child of the root and unturned. Its `TopLeftFlap` local matrix is S35's captured rest
matrix to the printed digit.

- `game-render/src/biplane.rs`: `pose(model, surfaces)` turns one frame's `Surfaces` into every node's
  model-space matrix, the way S35's journal says the original's surface step changes the bones. The
  wing flaps get `R(hinge, a) · rest` about the rest's normalised first row. The rear flaps, the rudder
  and the prop have their rotation replaced (about x, y and z), and a wrecked prop is left alone. It
  composes through the parents. `Biplane::pose` wraps it, and `Biplane::posed_parts` stands the cone
  `Surfaces::cone` names at the posed `PropDummy`. `Biplane::prop_node` caches the dummy's index.
- `openmx/src/vehicle.rs`: `Plane::surfaces()` calls the surface step off the flight's own control state
  (`surfaces.elevator`, `joystick.max_elevator`, `surfaces.aileron`, `surfaces.rudder`,
  `engine.prop_angle`, `crash.reason != 0`). A parked plane is all at rest.
- `openmx/src/plane_draw.rs` (new) is the shell's draw path for a plane:
  - `actors` (GPU) pushes the pose onto `gpu_poses` and names it on the plane's actor, so the vertex
    shader skins the plane by it, as it does a chassis.
  - `skinned` (software) skins the pose into a per-seat copy of the livery's model
    (`<model>@seat<p>`, made on first use). A skinned model's triangles are its pose, and two seats can
    fly one livery (players 0 and 2).
- `main.rs`: all four plane draws go through these: the flown planes, the bays' ghosts and the markers'
  faint planes on the GPU, and the flown planes on the software path. `plane_copies` holds the software
  copies' names and grows as seats join.

opentennis has no vehicles with moving parts, so there was nothing there to take.

## What the original does

The surface step is `./kit fn 352ff0`, described in S35's journal
(`2026-10-20-s35-s35-the-biplane-moves`). Two details matter here:

- The hinge each wing flap turns about is a vector the plane keeps beside the rest matrix
  (`./kit fn 1b2c20` takes it as its third argument). It is set at load, and I did not trace where from.
- The bone matrices it writes are the soul tree's local ones. On this model those are also the
  model-space ones (every moving bone hangs off the root, which is the identity).

## How it was proved

- **The pose against the original's bones, every row of a PINE recording.**
  `crates/openmx/tests/biplane_surfaces.rs::the_pose_is_the_originals_bones` reads S35's
  `context/capture/biplane/pilot.csv`, 1717 rows of the human flying the biplane. For each row it:
  1. runs the surface step on the row's controls,
  2. poses the disc's model with `game_render::biplane::pose`,
  3. checks `TopLeftFlap`, `TopRightFlap`, `RearLeftFlap`, `Rudder` and `PropDummy` against the bone
     matrices the original's model was left with.

  The rear flap, rudder and prop agree within 1e-5 and the wing flaps within 2e-3, the same tolerances
  as S35's test. The recording moves the four off their rest on 708, 708, 657 and 324 rows, so this is
  not a still plane being compared. The csv was copied from s2 into this slot's `context/`.
- **The stick, in the shell's own draw path, both renderers.**
  `holding_the_stick_moves_the_drawn_planes_surfaces` uses the disc's `biplane.phy` and model. It flies
  the plane level through the shell's `Garage` on gas for 30 s, then holds each input for 20 frames:
  stick left, right, up, down, L1 and R1. Each hold checks that:
  1. the GPU actor is posed by exactly the pose, and the software copy's triangles are `skin` of it;
  2. nothing but the eight moving bones moves;
  3. stick x turns the four wing flaps, left pair against right, and stick left against stick right;
  4. stick y turns both rear flaps, and up against down;
  5. L1/R1 turns the rudder, one way and the other;
  6. the picture changes in both renderers, against the same plane with its surfaces centred and the
     prop unchanged.

  On the last check, the headless GPU and the software rasteriser differed in 375–434 and 362–389
  pixels for the stick, and 131–145 and 130–138 for L1/R1. I also looked at the six GPU pictures by
  eye (`GPU_TEST_PNG=<dir>` writes them).
- **Beside the original in PCSX2.** I loaded `biplane.p2m2_SaveState.p2s` in copy 1, held each input
  for 0.6 s with `./kit pad`, and took a `./kit shot` of each.
  - Stick left raises the original's left ailerons and drops the right ones, and stick right does the
    reverse. The shell's pictures, taken from behind the same way, do the same.
  - The original's chase camera keeps the tail out of the picture, so its elevator and rudder could not
    be compared by eye. For those, the row-by-row bone check above is the proof.

## Not verified / not 1:1

- **The wing flaps' hinge is a stand-in.** The pose turns each flap about its rest matrix's normalised
  first row. The original turns it about a hinge it keeps beside the rest, which nobody has captured or
  traced. That is the 2e-3 tolerance (rendering only, never the simulation).
- **Only one flap of each pair was compared against the original:** top left, top right, rear left,
  rudder and prop, the bones S35 captured. Bottom left, bottom right and rear right take the same call
  with the same or negated angle, so they are only proved through the formula.
- **The elevator and rudder were not seen beside the original in PCSX2.** The chase camera hides the tail.
  They are proved by the bone-matrix rows only. The aileron direction was compared by eye only, from one
  parked state.
- **Surfaces are not interpolated between ticks.** The GPU path slides the plane's placement between
  the last two ticks (P52), but the pose is this tick's. The pose is only picture, and a step of a 30 Hz
  rate-limited surface is too small to see.
- **The tail wheel (`RearWheelAxle` / `RearWheel`) is not posed.** The plane's control state has a tail
  wheel steer, but the surface step S35 ported does not touch that node, and I did not look for another
  function that might.
- **`Biplane::turned_parts` and `parts` are left in place.** `moment.rs` still uses `parts`. The shell no
  longer calls `turned_parts`, and I left it rather than remove code I did not come for.
- **The software path's copies use the finest group only (`skin(.., 0)`).** That is what `Loaded::tris`
  always held.

## Next

The work is complete and committed on task/V38, which I pushed by hand. It is **not ticked**, because
main's gate is red without it. `./kit done V38` failed on two tests:

- `replay.rs`: `the_captured_backwards_turns_never_wreck` ("flat agrees through tick 213,
  S68_AGREES_THROUGH says 330")
- `replay.rs`: `the_slip_cases_replay_from_the_save_state` ("turn: the path parts at 75, not after 82")

Both fail the same way on bare HEAD 46830197 in a scratch worktree with this slot's context and disc.
This is S119, already filed. Everything else in the gate passed. The lobby load flake passed on its
lone rerun.

Once S119 lands, rerun `./kit done V38 "<msg>" --skip-audit`. The audit's seven leaks are other tasks'
addresses in `plan/DONE.md`. Then rename this file to FINAL.
