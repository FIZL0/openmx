# 1 — first person roll on screen (FINAL)

Task V137. 2026-11-21.

## What I did

- The render Camera (`crates/game-render/src/world.rs`) has a new field `up: Option<V3>`. `None` is the
  world's up, turned by photo mode's roll as before (`view_up`). `Camera::up()` gives the up a view is drawn
  with, and both projections (software `view_projection` and the GPU's) build their view from it.
- The shell (`main.rs`) hands the first-person camera's own up (ObjectRollScale × the bike's roll) to the
  Camera it draws. The wreck camera and the thrill camera clear it again when they take the screen.
- The sky's rays (software `Sky::draw` and the GPU's sky) and the sun flare's basis take the view's up
  too, so the sky tilts with the ground. Before, they always used the world's up; photo mode's roll had the
  same gap and is fixed by the same change. The spec review called this outside the task's file list. I
  kept it: a sky that stays level over tilted ground is not a drawn roll.
- Every other `Camera { … }` literal got `up: None` (48 sites, no change in what they draw).

## What the original does

P384's journal has the first-person camera: its up is rolled by `ObjectRollScale` of the bike's roll, up to
`MaxRoll`, and unrolled while tumbling. This task only draws that up; it adds no new behaviour.

## How it was proved

- `crates/openmx/tests/first_person.rs` `a_rolled_first_person_up_tilts_the_drawn_horizon`: the camera is
  built the way the shell builds it, and the level horizon is projected through `view_projection`. A bike
  leaning 0.4 rad tilts the drawn horizon by 0.2 rad (within 3e-3), the other lean by −0.2. A level bike, or
  the same view with `up: None`, draws a level horizon.
- By eye (scratchpad only, not committed): `openmx --track free06 --headless --frames 85 --shot … --shot-frame
  80 --size 640x360 --camera "First Person" --play <tape>`, with a hand-written tape: gas all the way, and
  from tick 60 either the left stick centred or hard right. Riding straight, the horizon is level. Leaning
  right, the horizon and the sky tilt, the right end higher. At tick 120 on the bank by the start, the view
  is strongly tilted (bike roll plus the slope). It looked right.
- Checked: `./kit check -- -p openmx --test first_person`, then the full gate in `./kit done`.

## Not verified / not 1:1

- The drawn tilt was not compared with a PCSX2 frame of the original's first-person view. Reason: only the
  shell's own up is drawn here; P384 did not prove the up's numbers against the original either (see its
  journal).
- No test covers the shell's own wiring in `main.rs` (first-person up into the Camera, cleared by the
  wreck and thrill cameras). Reason: that code sits in the shell's frame loop, which tests cannot reach.
  The by-eye shots cover it.
- No before/after shot of the same frame on main. Reason: the test's `up: None` case is that comparison.
- The audio listener's right (`main.rs` `listener`) still uses the world's up, so sound does not roll with
  the camera. Reason: left alone, as it is not part of the picture.
- Plant billboards stay upright in the world under a rolled view. Reason: that is right for plants standing on
  the ground, and it is how photo mode already draws them.

## Next

Nothing.
