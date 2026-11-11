# 1 — mxvatv car step (FINAL)

Task MD125. 2026-11-11.

## What I did

- `atv_car::Pieces` now runs the car's engine: its step (`Call::Engine`), its automatic box (`Call::Shift`)
  and its reverse switch (`Call::Reverse`). It does this only when `Keys::engine` is
  `Some(EngineKeys)`; with `None` all three go to rest as before. The engine's running words are
  `State::engine` (`EngineState`). Its reverse word stays `State::reversing` (MD126), and the launch's torque
  scale stays `State::launch_boost` (MD106a). The controls' reverse switch now goes to the engine piece first
  and reaches rest only without the engine's keys.
- The engine hands out two new calls. `Call::Noise(at)` asks the shared smooth noise and gets its answer
  in `Car::noise`. `Call::ShiftSound` is the shift's sound, made when `EngineKeys::sound` is set.
- `tests/mxvatv_car_frame.rs`: the engine's words (`mod engine`, `read_engine_keys/state`, `engine_words`)
  are read, poked and compared, and the whole engine (0x134 bytes) is one of the "nothing else written"
  blocks. The smooth noise is hooked for every caller with `mxvatv_step`'s fake, and is logged only where the
  player's engine asks it. The shift sound is hooked and logged on the player's engine's sound object.
  `generate_engine` (its own rng, used only when the engine is unhooked) writes the inputs. The tables,
  ratios, converter, revs, nitrous and TCS vary, with flat ranges now and then. The shift points and timers
  vary too. Sometimes the car is four-wheel drive. The pedals, the clutch (down half the time) and the
  drive tires' spin, slip and compression vary as well.
  The new tests are `engine_is_mx_unleasheds` (step + shift unhooked, 24 branches) and
  `engine_with_the_controls_is_mx_unleasheds` (the controls too, so the reverse switch runs in the
  original's own code, 22 + 24). Unhooking the step also unhooks the reverse switch.
- Reseeded three older combined tests (`all_six_pieces_are_mx_unleasheds` 0x0126_0002 → 0x0126_0032,
  `all_six_pieces_with_the_tire_step…` 0x0124_a002 → 0x0124_a012, `all_eight_pieces…` 0x0127_0003 →
  0x0127_0013). They were already red at HEAD cda3b24dd before any of this work (checked by running the
  HEAD files): "preload: fired, degenerate" was seen only 17–19 times against a floor of 20, most likely
  because MD127's extra draw moved the shared rng stream. Changing the generator would move every other
  test's stream, so I changed only these three seeds. Each stays bit-exact. With the floor raised to 25
  for one run, one of the three showed that tag 22 times, so the margin is thin (see below).

## What the original does

See research/oracle.txt `# MD125` for the full layout. It is MX Unleashed's drivetrain (car.rs `Drivetrain`)
with these differences:

- The step fuses (`adda.s 0` then `madd`/`msub`) the torque curve's value, the nitrous burn, the converter's
  multiplication, the implicit step's damping and the rpm update.
- With traction control on and a flat slip range, the inline remap's flat branch gives the first tire of a
  pair 1. The second tire gets the `sub.s` in the delay slot, `1 - min`.
- The pitch's wobbles are fused. The top one reads the curve table's bare entry (no leftover correction),
  using the fused double wrap.
- Set-gear's top wheel speed constant is `0x3dd67756`. My first guess, `0x3dd67751` (0.1047198 written
  out), failed the oracle by a few ulps at case 16; the disassembly has the right word.
- The shift skips everything while reversing, so its own ways in and out of reverse are dead code.
- The shift's sound plays before the gear changes. Nothing between them reads it, so the port makes the
  call after `shift` returns.

## How it was proved

Function oracle in the frame: SLUS-21104's world step on `context/mxvatv-ps2/md39a/buggy.ee.bin`, with the
engine's step, shift and reverse switch run in the original's own code, against `atv_car::frame` with
`Pieces`. Each test runs 3,000 generated frames with every compared word and every call equal
(`engine_is_mx_unleasheds`, `engine_with_the_controls_is_mx_unleasheds`). Each tagged branch was seen at
least 20 times. The fewest were "shift: top gear" at 59–75 and "engine: pitch flat" at about 200. All 18
tests in the file pass.

## Not verified / not 1:1

- The smooth noise (0x00134580) is faked on both sides, as `mxvatv_step` does: the pitch's bottom wobble is
  proved for the call, its time and how its answer is used, not for the noise's own values. That function is
  shared with the bike and is not this task's.
- The shift's reverse branches (out of reverse on an up-shift, into it on a shift down from first) are never
  run: the early return on reversing makes them unreachable, so the port leaves them out.
- The drive slots are generated either as the buggy's two (RL, RR) or as all four (with FL, FR). A slot
  layout with only one of slots 2/3 set, or slot 0/1 empty, is not modelled: `EngineKeys::drive` holds two
  tires and `drive_more` holds either both others or none. Only the buggy's layout was read; the
  loader that fills the slots for other cars was not.
- The table sizes (8 torque points, 5 gears) stay the image's: generated values fill the image's tables in
  place, so other lengths are not exercised.
- `Call::ShiftSound` stands for the sound call only: the sound's own effect (0x004b83e0) is not ported.
- "preload: fired, degenerate" in the three reseeded combined tests is only just over the floor (one of
  them saw it 22 times). The preload-only test covers that branch on its own. A future change to the shared
  rng stream may push it under 20 again. That is a coverage count, not a mismatch.

## Next

Nothing for MD125. MD106e (the car's update) and MD131 (movers) stay open in the plan.
