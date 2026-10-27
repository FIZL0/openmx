# 1 — roost offset (FINAL)

Task P162. 2026-10-27.

## What I did

- **Measured where the original's sprays sit on the bike.** P74's capture (`context/capture/p74/run.csv`,
  only in slot s2's context) has the chassis matrix (`bike_xform`, actor +0x180) and every emitter's
  position (`e<i>_at`) on every row. I took each emitter's point into its own row's chassis frame.
  - Rear sprays (entries 7, 9, 11, 12 on `RearTire_Particle01`): median (0.000, 0.342, −3.030) ft over
    2204 riding rows, sd (0.02, 0.26, 0.16). The y spread is the swing arm moving with the suspension.
  - Front sprays (8, 10 on `FrontTire_Particle01`): median (−0.029, 0.418, 1.901). The x spread
    (sd 0.16) is the bars turning.
  - At the launch's first emitting row (2007): rear (−0.001, 0.609, −3.184), rear compression 0.4711.
  - The point fits the row's **own** matrix. With the row before's, it sits a tick's travel forward
    (z median −2.23 instead of −3.03, sd 0.73). So the original reads its node on this tick's pose and
    placement, and draws the bike from the same tick.
  - The matrix rows have unit length (0.9999994), so the model is not scaled.
- **Checked the port's node.** A throwaway probe in game-render's vehicle test (deleted again) printed
  the skeleton:
  - `RearTire_Particle01` hangs off SwingArm → Bike. Its bind point is (0.0004, 0.108, −2.863).
  - `FrontTire_Particle01` hangs off Shocks → Handlebars → Bike.
  - Neither one is under the spinning tire node, so neither orbits with the wheel.
  - Posed at the original's launch compression, the port's rear node is **0.020 ft** from the
    original's point. Riding along it is (0.000, 0.30, −3.005), against the original's median
    (0, 0.342, −3.03).
  - So the node, local vs. world, and the scale were all already right at tick level.
- **Found two placement faults in the shell.** Both are fixed.
  1. **The pose was a tick late.** main.rs stepped the bike particles before the seat loop posed the
     chassis (`chassis.frame`). Each tick's spray therefore read the tick before's swing arm and bars,
     multiplied by this tick's placement. The bike-particle step now runs after the seat loop, so the
     nodes are this tick's pose on this tick's placement, as in the original.
  2. **The picture was not blended for particles.** The bike is drawn blended between the last two
     ticks (P52: `blend.slide(before, now)`). The particles were drawn where the last tick left them.
     That put every newborn of the roost up to a whole tick's travel ahead of the drawn tire: about
     0.8 ft at 50 ft/s and 60 Hz. The particles also moved in steps from tick to tick, which is
     P74's "spaced puffs".
     - `Particle.was` is new: the particle's position on the tick before.
     - Each step sets `was` to the old position before it moves the particle.
     - A particle born this tick gets its birth point carried back by its emitter's travel over the
       tick (`at − from`).
     - `PlaneFx::drawn_at(by)` draws each particle at `was + (at − was) × by`. A newborn therefore
       sits on the blended node, and nothing jumps at the tick.
     - main.rs passes the picture's blend to the GPU and software particle paths. For a bike seat
       whose picture cuts that frame (`cut[p]`), it passes 1, as the bike's own picture does.
     - `drawn()` is `drawn_at(1.0)`, so existing callers and tests are unchanged.
     - The plane's and the track's particles get the same blend. It is render-side only.
- Tests: `crates/openmx/tests/roost_offset.rs`, new (details below). In `biplane_particles.rs` the
  two `Particle` literals gained `was`.
- opentennis: nothing there emits particles from a moving node, so nothing was taken.

## What the original does

- The bike's particle manager (`./kit fn 0x003d5aa0`, P74) runs each attached emitter at its node's
  world point for the current frame. The capture shows that point is the current row's chassis
  matrix times a fixed local point on the swing arm (rear) or the fork (front). Its height moves with
  the suspension, from y 0.08 extended to 0.64 compressed.
- I added no new decompilation reading. The evidence is the capture's positions.

## How it was proved

- **The capture, numerically.**
  - `the_originals_roost_point_is_the_captures` recomputes the launch point from the capture's row 2007
    (to 2e-3 ft) and its rear compression.
  - The same test checks that the row before's matrix puts the point more than 0.05 ft off.
  - It ran green with `GAME_SIM_FIXTURES=<s2>/context`. I confirmed it was not skipping by turning the
    skip into a panic for that one run.
- **The port against it.** `the_roost_is_born_at_the_originals_point` loads the disc's 250sx chassis.
  The capture's bike has two exhaust emitters, so it is a 250, not the 125.
  - It poses the chassis at compression 0.4711 and asserts that the rear spray node is under the rear
    axle and within **0.05 ft** of the original's point. The measured gap is 0.020 ft.
  - It then steps free09's disc particle list over a launch, placed off the origin and turned 0.6 rad,
    so a mix-up between local and world frames would show. Throttle is open and the rear tire is down
    and spinning.
  - The roost's first particles (10 of them) are born on the node's path over the tick. None is
    further off that path than the `.emt`'s own extent (0.80 ft). In the bike's frame, they are within
    0.05 + 0.80 + one tick's travel of the original's point.
- **The draw.** `the_roost_is_drawn_off_the_blended_tire` checks that at blends 0, ¼, ½ and 1 each
  newborn is drawn its tick's travel × (1 − by) behind its birth point, which is the blended node. One
  tick later, it is drawn at blend 0 exactly where it was drawn at blend 1 before.
- `./kit check -- -p openmx --test roost_offset --test biplane_particles --test bike_particles --test
  track_particles` passes. The full gate runs in `./kit done`.

## Not verified / not 1:1

- **No side-by-side by eye.** I did not compare the port's picture with S109's video or a PCSX2 shot,
  for either the launch or the turn. The placement is proved against the capture's numbers instead.
  The blend fix is a drawing change, and the rules say to match drawing by eye, not 1:1.
- **The hard turn is not measured.** The capture has no steer angle column. So the front spray node
  under steering (x up to 0.33 ft in the original) is not compared against the port's bars drive.
  The pose-lag fix removes the one-tick lag of the bars, but whether the port's steer angle matches
  the original's is the drives' question (V9), not this task's.
- **The roost's birth spread is bounded only loosely.** The test bounds it by the `.emt`'s extent
  (0.8 ft), not by the original's own spread. The capture has the emitter's point, not its particles.
- **Riding compression is not the original's.** Riding along, the port's rear node sits about 0.04 ft
  lower than the original's median (y 0.30 vs 0.342). That is a difference in suspension compression
  between the two rides, not in node placement. It was not compared at matched compressions outside
  the launch row.
- **Newborns slide with the tire for their first tick.** In the first drawn interval a newborn is
  carried rigidly with the blended node, rather than appearing at its own birth moment within the
  tick. The original draws once a tick, so it has no such in-between picture to match.
- **Only the 250's chassis was checked.** The shell's default bike is the 125sx. Its chassis model
  `bike` was not measured, and the capture has no 125 ride.
- **A seam crossing (MD44's wrapped worlds) is not handled for newborns.** A tick whose emitter jumps
  a world width would carry its newborns back by that width for the frame's blend. The emitter's own
  emission along `from → at` already spans the jump (this predates this task). Not tested.

## Next

Nothing. `./kit done P162`.
