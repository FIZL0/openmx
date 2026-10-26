# 1 — biplane pilot shown (FINAL)

Task V39. 2026-10-25.

## What I did

- **Why there was none:** of the four suspects in the brief, it was the first. Nothing ever handed the
  pilot to the ride's draw. V21 seated the pilot for `--moment` shots only (`moment.rs`'s `Shot::pilot`).
  The ride loop's bike pass in both renderers skips a seat whose plane is up (P102), and its comment said
  the pilot was "whom V21 seats", but the plane pass drew only `Biplane::turned_parts`. Neither the GPU
  renderer, the rider's model nor `PILOT_SEAT` was at fault.
- `game-render/src/biplane.rs`: `pilot_palette(body, anchors)` poses the rider's own body with V21's
  `seat` (the even mix of `biplane_roll_fwd`/`_bck` at their first frame), held to the plane's four seat
  anchors at full strength with both feet on, and gives the node matrices. Afterwards it puts back the
  body's pose, `hold` and `feet`, so the bike's rider picks up where they left off on a switch back.
- `openmx/src/main.rs`: `pilots()` builds one `Pilot` per flying seat each frame, placed at the same
  blended plane spot the plane is drawn at, with its root at `Biplane::pilot`. Both renderers draw from
  it. The GPU path pushes the palette as an actor pose on the body model. The software path skins the
  same palette into the body's mesh at `RIDER_GROUP`. With the plane up the seat's bike rider is not
  drawn, so the mesh is free. Pilot alpha is the plane's own, so it fades with the player's pause (P84).
- `--trace-picture` prints a `pilot:` line for each drawn pilot: the renderer, the model, and the root
  and middle of what was drawn, both in the plane's space. For software the middle is of the skinned
  triangles; for GPU it is of the posed nodes.
- Test `crates/openmx/tests/biplane_pilot.rs`: free06 with `--vehicle biplane`, headless, run on both
  renderers. Every drawn frame has a pilot whose root is at (0, 3.5, −3) in the plane's space, and the
  middle of the drawn body is on the centre line within 3 ft of the seat. Measured: software
  (−0.16, 4.39, −2.60) over 6105 triangle corners, GPU (−0.06, 4.06, −2.56) over 22 nodes.
- Opentennis has no planes or seated riders, so nothing was taken from it.

## What the original does

The plane draws the rider's body in its cockpit, root at `PILOT_SEAT` (V21 read that off the biplane
recording's states). No new decompilation was read for this task.

## How it was proved

Looked right beside a screenshot. The original's frame came from `./kit shot` of copy 2 after loading
`context/recordings/biplane.p2m2_SaveState.p2s`: the plane from behind, with the pilot's helmet just over
the windscreen. The shell's frames came from `--shot` at frame 10 on free06, GPU and software. Both show
the helmet and shoulders in the same place, over the windscreen between the upper wing's struts. The seat
offset and the presence of the body in both renderers are pinned by `biplane_pilot.rs`.

## Not verified / not 1:1

- The pilot's pose is static: V21's even mix at the clips' first frame. The original plays
  `biplane_roll_fwd`/`_bck` from the plane's animation (S35's), so a banking plane's pilot leans there
  and not here. Not ported, because that animation's choice of clip and time was not read (V21 left
  this open too).
- A wrecked plane still draws its pilot seated. What the original does with the pilot in a plane wreck
  (thrown, hidden, kept in) was not checked.
- The comparison against the original is by eye on one parked frame, from behind and at a distance.
  The pilot's limbs, which the plane's anchors hold, were not compared up close.
- Two seats that share one body model and draw in one software frame already overwrote each other's
  skinned mesh before this task (bike riders included). The pilot inherits that. Untouched here, as
  the GPU path, which is the default, poses each actor separately.
- The test's bounds (0.5 ft off the centre line, 3 ft from the seat) are a sanity box for "in the
  cockpit, not out of sight". They are not the original's numbers.

## Next

- The work is done, but `./kit done V39` could not tick it. The gate is red on main's own
  `game-sim --test replay the_slip_cases_replay_from_the_save_state` ("turn: the path parts at 75, not
  after 82"). That is S119, open since the batch merge 1cdaee5a. It fails the same way twice in this
  slot, and this branch does not touch game-sim, which game-render and openmx depend on, so its test
  binary is HEAD's. `biplane_pilot.rs` passed. The branch is pushed by hand. Once S119 lands, rerun
  `./kit done V39` to tick it.
