# 1 — wheels in air (FINAL)

Task V59. 2026-10-23.

## What I did

**Filmed the jump.**
- Shell: a generated 60 Hz tape with Cross held for 400 ticks (`--play`) on free06, from the start line
  over the first lip. That is free06's first jump, a flip lip, and the bike leaves it about 45° nose
  up. That attitude is the simulation's own (the trace's `up` is 0.72), not a wheel fault.
- Shell shots: `--shot` with a fixed `--shot-pose`, one grounded (tick 200) and one airborne (tick 259),
  each drawn with `--wheels original` beside the new blur, cropped side by side.
- Original: S109's animandpump playback video (`s2/context/capture/s109/run1.mp4`, 640x480), cut into
  frame sheets around the jumps with ffmpeg.
- `./kit shot` only screenshots PCSX2, and S109's video already had the original's jump on it, so the
  emulator was not started.

**Spin through the jump**, from a temporary trace (since removed):
- Front and rear wheel spin: 52–90 rad/s on the gas.
- The rear in the air holds at about 52.4, 64.2 or 78.0 rad/s, because nothing slows it.
- At 30 ticks a second that is 1.7–3.0 rad a tick. Half a turn is π.

**What was wrong:**
- (a) **Strobing.** Confirmed, and the main fault. The wheel has about 36 spokes, a disc rotor and a knobby
  tread, and was drawn razor-sharp at speed:
  - A spoke pitch is 0.17 rad, so from about 2.6 rad/s the spokes alias. They stood still, crawled or ran
    backwards from tick to tick.
  - At 90 rad/s the whole wheel turns 3 rad a tick. The window's blend between two ticks (`pace::Blend`
    row-lerps and renormalises the matrices) takes the short way round, so past π a wheel is drawn
    turning backwards between ticks.
- (b) **Hang at full droop.** Not a fault. Probed and then tested: off the drop with both wheels hanging
  and compression 0:
  - The front axle sits its bind distance under `Shocks`, which is the bind position plus the drive's
    fixed (−0.03, 0, 0.02) nudge.
  - The rear axle sits its bind distance from the swingarm pivot. It is 0.03 ft lower than the bind,
    because the arm rotation matches the bind.
  - Each tyre's middle is on its axle.
  - The front fender is at least 0.615 ft clear of the tyre.
- (c) **Fender at bottom-out.** Found while measuring (b). `250SX_Front_Fender` hangs off `Handlebars` and
  so does not travel with the fork. At a front compression of 1.0 (landing the 30 ft drop) the front tyre
  comes 0.097 ft up into it. That is a landing, not the air, and it comes from the ported fork drive, so
  it is not changed here. It became **V60**.
- (c) Nothing else wheel-specific was found in the air.

**The change** (`crates/game-render/src/vehicle.rs`, presentation only):
- `Spin`: the wheel as drawn. It takes the same steps as `Roll`, each held to `SPIN_STEP_CAP` = 1.2 rad
  (a fifth of a turn), and keeps a 0–1 blur that eases in (smoothstep) between `BLUR_FROM` 6 and
  `BLUR_FULL` 20 rad/s. At 20 rad/s × 1/30 s = 0.67 rad, the blur is whole before the cap ever bites.
- The wheel nodes are now written from `Spin`, not `Roll`. `Roll` still integrates the original's
  uncapped angle.
- `Chassis::blur_copies(pose)`: for each wheel with any blur, eight copies of the chassis pose:
  - In each, every node under that wheel is turned back about the axle by `BLUR_ARC` 0.8 × (i+1)/8. That
    is two spoke pitches, about one knob. The turn is put on in model space,
    `W·(inverse(tire)·R_x(−dir·turn)·tire)`, so it works on the shell's blended pose too.
  - Every other node is collapsed to a point (zero rotation rows, translation kept), so its triangles have
    no area.
  - The alphas are `BLUR_ALPHA` 0.2 down to 0.12, nearest first, times the blur. The ghost pass draws
    faintest first, so the nearest copy weighs most and the smear fades behind the wheel.
- `Chassis::original_wheels`: draw at `Roll`'s angle with no copies.

**The shell** (`crates/openmx/src/main.rs`):
- The GPU actor loop pushes each copy as a see-through actor of the chassis model, at the bike's place,
  but not for a ghost bike that is already see-through.
- `--wheels original|blurred` sets `original_wheels`.

**Tuning**, by eye on the side-by-side pairs:
- 4 copies over 0.35 rad read as extra sharp spokes.
- 6 over 0.5 rad was too faint.
- 8 over 0.8 rad, alphas 0.2 to 0.12, reads as a spinning wheel: the spokes become a grey haze, the
  rotor smears and the knobs soften. The hub, rim and tyre outline stay put.
- On the grounded frame (tick 200, ~60 rad/s) the difference is the same.

## What the original does

Nothing worth porting. As far as the S109 video shows, the original draws its wheels as plain turning
geometry with no blur. At its 640x480 far camera the bike is too small for the spokes to resolve, and the
wheels read as dark discs. `Roll` (V9) is still the port of its angle, and `--wheels original` draws it.

## How it was proved

**`crates/game-render/tests/vehicle.rs`, all on the disc's own 250:**
- `past_the_alias_rate_the_wheel_turns_forward_and_blurs`: 2402 rates from −300 to 300 rad/s, at 1/30
  and 1/60 s. For each:
  - the drawn step keeps the rate's sign and is under a fifth of a turn;
  - under the cap it is the rate's own step;
  - wherever the cap holds a step back, the blur is whole;
  - the blur is 0 up to 6 rad/s and 1 from 20.

  A NaN rate leaves the wheel where it was.
- `a_spinning_wheel_blurs_alone_about_its_axle`: the front spun forward at 100 rad/s and the rear
  backward. That gives 16 copies at `BLUR_ALPHA`.
  - Skinned, every triangle in a copy that has any area is that wheel's.
  - Every one of that wheel's vertices keeps its distance from the axle and its place along it, and is
    turned by exactly −dir·0.8·(k+1)/8 about the tire node's own x.
  - Half a blur gives half the alpha.
  - A slow wheel, or a pose of another model, draws no copies.
  - With `original_wheels`, no copies, and the node is at `Roll`'s angle at 150 rad/s.
- `at_full_droop_the_tyres_stay_on_their_axles_and_clear_of_the_fender`: 81 frames hanging off the drop.
  - It checks the axle distances, the tyre middles on their axles, and the fender clearance (fender
    vertices within the tyre's width, against the tyre's outer radius).
  - It prints the minute's least clearance: −0.097 at compression 1.000. That is V60.
- V9's `the_wheel_node_angle_integrates_its_own_spin` now checks the node against the capped integral,
  and still checks `Roll` against the original's own wrap.
- The two disc tests' printouts were seen once (a temporary panic), so they ran and did not skip.

**By eye:** the side-by-side shell pairs at ticks 200 and 259, old wheels against blurred.

## Not verified / not 1:1

- **The original's wheels in the air were not seen up close.** S109's video is a far 640x480 camera where
  the spokes do not resolve. Nothing here claims the original looks this way: the blur is ours, by design.
- **The blur constants are by eye.** `BLUR_FROM`, `BLUR_FULL`, `BLUR_ARC`, `BLUR_ALPHA` and the 1.2 cap were
  chosen from two shot pairs on one bike (the 250). Other bikes' wheels were not looked at.
- **The blur's weights are not uniform.** The ghost pass blends each copy over what is behind it, and a
  copy over the drawn wheel weighs more than one over the ground. So the smear's density depends on the
  background and on the draw order (faintest first). It looked right, but it was not measured.
- **The software renderer gets the cap but no blur.** The copies are only pushed on the GPU actor path.
- **The window's between-tick blend was not filmed.** Headless `--shot` draws one tick per picture. That the
  cap stops a wheel turning backwards between ticks is argued from the blend (shortest-way row-lerp) and
  the cap (1.2 < π), not watched in a window.
- **The Original switch is a command-line flag, not a sub-switch of P126's Original mode.** The menu
  switch would have touched menu.rs, menu_pages.rs, the settings format and their tests, so it was not
  free, and the brief asked for it only if free. `--wheels original` gives the old look.
- **The front tyre goes into the fender at bottom-out** (−0.097 ft at compression 1.0). Not fixed: it is a
  landing, and the fork drive is a port. It is V60, and whether the original clips the same way is
  unknown.
- **The drawn wheel angle is no longer the original's angle** past 72 rad/s at 60 Hz or 36 rad/s at 30 Hz
  (the cap). `Roll` still holds the original's angle, and nothing reads the drawn one but the renderer.

## Next

Nothing for V59. V60 (the fender at bottom-out) is in the plan.
