# 1 — race gate drop (FINAL)

Task P313b. 2026-11-14.

## What I did

- `crates/openmx/src/race_start.rs` `GateSwing`:
  - It reads the track `.toy`'s `[DropGate]` words (GateName, RotationAxisLocal, RotationAngleDegrees,
    with the reader's defaults).
  - It takes the named prop out of the scene's static props.
  - It turns that prop about its local axis once the race's gate opens. The angle grows by
    `angle × dt / 0.15` each simulated tick, and the last step is cut short to land on the angle.
  - It draws the prop as an actor, on both the GPU path and the software path.
  - When the gate is closed again, the prop stands again.
- `crates/openmx/src/main.rs` takes the prop after the stadium flashes, steps it beside them on each
  simulated tick, and prints two lines: `race: drop gate <model> drawn up …` at the start and
  `race: drop gate down at tick N, …` when the turn ends.
- Nothing taken from opentennis: it has no gate or prop animation of this kind.

## What the original does

- The supercross activity's loader (`./kit fn 0x002f9f40`) makes a drop-gate object at activity +0x3ac:
  `./kit fn 0x002ff110` builds it and `./kit fn 0x002ff2e0` sets it up. It is made only when the
  track's `.toy` has a `[DropGate]` section. It keeps a pointer to those words and to the gate prop's
  scene node (found by GateName lowercased, the same lookup the start gate uses).
- `race_drop` (`./kit fn 0x002fbdd0`) calls `./kit fn 0x002ff4e0` on it:
  - axis = RotationAxisLocal;
  - angle = RotationAngleDegrees × 0.01745329;
  - length = **0.15 s, a constant**. The words' `RotationRateInSeconds` (sx01: 3.0) is read by the
    `.toy` reader and never used.
- The object's update (vtable +0x38, `./kit fn 0x002ff900`) takes, each frame,
  `step = angle × dt / 0.15`. On the step that would reach or pass the angle, it cuts the step to land
  on the angle and stops. Each step goes to `./kit fn 0x001b2c20`, which multiplies the node's rotation
  rows on the left by an axis-angle matrix and keeps the node's place (`./kit dis 0x00110ba0`;
  0x00110e10 is the cosine and 0x00110ce0 the sine). So the prop turns about its own axis, and the
  steps add up to one turn by the whole angle.
- `MoveGateDown` (sx01: FALSE) would also sink the gate's soultree and collision pieces to y −100
  over `MoveTime`, set off by riders crossing the line spots. That is not ported (see below).
- sx01: THQsxdropgate stands leaning 30° back (up 0,.866,−.5) and turns −60° about its across row,
  so it ends flat with its top toward the grid (up 0,0,−1).

## How it was proved

- **Live RAM, PCSX2 copy 4** on `context/states/c31-sx01-race-start.p2s`:
  - Two save states read as RAM images. Before the drop the gate object holds its constructor's values
    and the node stands as the scene places it. After the drop: angle −1.0471973, length 0.15, fully
    turned, node rows up (0, −0.0017, −1.0) and look (0, 1.0, −0.0017).
  - A `./kit pine watch` of the node's look row and the turn word: five 30 Hz steps of −0.2327, the
    fifth cut short to −1.0472.
  - A second watch: the start gate's flags go 3 → 2 on the same frame as the first step (frame 288).
- **Unit tests** `race_start::tests`:
  - `the_drop_gate_words_and_their_defaults`;
  - `the_gate_is_taken_out_of_the_scene_and_swings_flat_at_the_drop`: closed it stands; open it ends on
    the fifth 1/30 s tick at exactly −60 × 0.01745329, with up (0,0,−1), look (0,1,0) and the place
    kept;
  - `no_drop_gate_section_or_prop_leaves_the_scene_alone`.
- **Shell test** `tests/race_menu.rs` `a_race_starts_on_the_grid_behind_a_closed_gate_that_drops`
  (disc): sx01's drawn gate is up (0,.87,−.5) at the start. It is down four ticks after the drop's
  tick (drop at 97, down at 101), with up (0,0,−1).
- **Looked right beside a screenshot.** I shot PCSX2 (AT-SPI System → Screenshot) and the port's
  **release build** (`target/release/openmx --headless --race --shot`), behind the rider at sx01's
  grid. Both show the gate standing before the drop, a rail with sponsor panels at the rider's front
  wheel. After the drop the port's panels lie flat on the ground. The original's rider rides off at
  once (AllAI), so its camera never looks back at the fallen gate. The pose after the drop was checked
  against the RAM numbers above instead.

## Not verified / not 1:1

- **The pose is computed in one go, not step by step.** The port draws `R(axis, turned) × placement`,
  computed fresh each time. The original multiplies the node's rows by one small turn per frame, so
  float error builds up. Its flat gate ends with a 0.0017 tilt that the port's does not have. This is
  rendering only, matched by eye by policy.
- **Steps follow the shell's simulated ticks**, with the sim's dt. The original's object updates once
  per game frame. Both run at 30 Hz on sx01 (the watch above), but under the shell's catch-up or pause
  the swing follows the sim's ticks.
- **MoveGateDown is not ported**: the pieces sinking to y −100 over MoveTime, set off at the line spots
  (0x002ff540, the +0x50/+0x54/+0x58 branch of the update). sx01 has it FALSE. I did not check which
  disc tracks set it TRUE, so a track with it on draws its gate turned but not sunk, and its collision
  pieces are not moved.
- **The prop is found by GateName, ignoring case**, in the scene's placements. The original's lookup
  (scene vt +0x120) was not ported here. If the scene has no prop of that name, nothing turns. Only
  sx01 was ridden: the nationals' `Nat15_Dropgate01` and the other sx tracks were not.
- **Closing the gate again stands the prop back up.** That is the port's own. The original's race
  reset (0x002fb2e0) was not traced as far as the gate object, and `./kit fn 0x002ffc50` (the whole
  turn at once, from 0x0031a620) was not ported.
- **The tilt of the turn's axis.** Only the default and sx01's axis (1,0,0) were seen. The axis is made
  unit length as the original's builder does, but a slanted axis was not compared against the original.
- **Not looked at on the GPU in a window.** The release-build shots are headless GPU frames. The
  software path's actor is wired, but I did not shoot it.

## Next

- Nothing for this task. P313c (low) is the MoveGateDown sink, for any disc track that sets it.
