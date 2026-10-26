# 1 — biplane pilot anim (FINAL)

Task V78. 2026-10-26.

## What I did

- `game_sim::biplane_anim::Lean`: the pilot body's two lags (mix τ 0.125, phase τ 0.15) and the
  selection's clamps, the only part of the body step that reaches the pilot's picture once the plane's
  added record is mounted. `ease` (from a request's two targets + holding flag) and `step` (from a
  `Pilot`), `weights` to read without stepping.
- `openmx::vehicle::PilotLean`: per seat, stepped once a simulated tick after the plane's step (in
  `main.rs` beside the preload meter, on the `Rig`, so it survives the online path's rebuild of
  `Plane` every tick). Feeds the *previous* tick's `flight.pad` elevator/aileron (the original's measure
  runs before the controls), raises `biplane_anim::Reset` on the swap onto the plane and on
  `said.respawned` (chord, wreck recovery), so the axes snap for the two measures that see the flag.
  Off the plane it lets go, so the next swap snaps again.
- `game_render::biplane::seat(set, phase, mix)` now plays the original's two-clip blend:
  `biplane_roll_bck` at `phase × its duration`, `biplane_roll_fwd` at the same second, mixed by `mix`
  (1 = back roll), the same orientation `Set::draw_two` uses for the bike's `Blend::Two`.
  It used to be a fixed `(lean 0.5, time 0)`, i.e. the pilot stuck in one pose. `pilot_palette` takes
  the weights; the shell passes the seat's `PilotLean::weights()`. The `--moment` biplane shot passes
  the centred `(0.5, 0.5)`.
- Took nothing from opentennis (no vehicle pilots there).

## What the original does

Already found by S35 (journal 2026-10-20-s35-s35-the-biplane-moves): the rider build adds one record
per flying machine with `biplane_roll_bck`/`biplane_roll_fwd`; mounting points the body at it; the
plane's measure step writes `phase = 0.5(1 − aileron)`, `mix = 0.5(1 + elevator)` from the pad
(aileron left +, elevator up +) into the body's request every tick; the body eases both with its lags
and plays `Blend::Two { phase, mix, a: bck, b: fwd }`. **The rudder, throttle, speed and airborne flag
do not pick or blend the pilot's clips**: the request carries the speed and airborne flag, but with an
added record mounted the selection ignores the state machine entirely (S35's test asserts the blend
every tick of the human's flight). So the user's "with rudder and throttle" is, in the original, the
stick only; the test asserts L1, R1 and the gas let go of leave the pilot where a centred stick has it.

## How it was proved

- `crates/game-sim/tests/biplane_anim.rs::the_biplane_asks_for_the_originals_clips` (S35's, on
  `context/capture/biplane/pilot.csv`, 1718 ticks of the human's biplane flight): a `Lean` fed the same
  request as the body now equals the body's two lags bit for bit on every tick, and its weights equal
  the `Blend::Two` phase/mix the test already proves against the capture.
- `crates/openmx/tests/biplane_pilot_lean.rs` (new): the disc's plane flown through the shell's
  `Garage` for 30 s of gas, then each full stick held 1 s: the lean reaches the original's axis ends
  (left → phase 0, right → 1, up → mix 1, down → 0, within 0.01, and equal to the pad's own axes), the
  drawn pilot palette (thq1, `pilot_palette`) moves 0.49 / 0.42 / 0.22 / 0.23 ft from the centred
  pose, and the two ends of each axis differ; L1, R1 and no gas leave it as centred (< 1e-3 ft).
  Proved it ran (not skipped) by a temporary panic printing those distances.

## Not verified / not 1:1

- **The body's ramp is not drawn.** The pilot's body cross-fades from its previous pose for ~10 ticks
  after its state machine switches state (air ↔ ground): `body_ramp` is non-zero on 56 of the 1718
  capture rows, 0 on the rest. The drawing side has no pilot body step, so it plays the clips at full
  weight throughout; a take-off or landing pops instead of a 10-tick fade. Render-only.
- **The reset snap's tick.** The shell raises the reset on the tick `respawned` reports and before
  that tick's measure; in the original the chord is in the input slot and the recovery inside the
  frame, so which measure first sees the flag was not checked against a capture for either. It only
  moves when the pilot snaps by a tick.
- **Online seats** were not run: the lean reads `garage.plane_flown()`, the same source the shell's
  `pilots()` draws the pilot from, so wherever a pilot is drawn it leans; not exercised in a netplay run.
- **The sign of the picture** (that `mix` 1 = back roll is the right way round) rests on
  `Set::draw_two`'s convention for the bike's `Blend::Two`, not on a side-by-side shot of the
  original's pilot held each way — not compared by eye.
- No windowed shell run looked at the pilot; the proof is on the drawn node palette.

## Next

Nothing for this task; the ramp above is the one render gap if the pilot's take-off pop shows.
