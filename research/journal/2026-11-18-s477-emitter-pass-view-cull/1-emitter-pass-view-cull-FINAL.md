# 1 — emitter pass view cull (FINAL)

Task S477. 2026-11-18.

## What I did

- Checked the bike's `.emt`s. They do set `UseViewDependentCull`:
  - In the C33 image every exhaust, generic and spray emitter has the cull on, with FarCull 800 and
    DirectionalCull 200 (a one-off probe in `tests/ai/race4.rs`'s seed, taken out again).
  - In `s325-nose76-2604` all 40 of those emitters, across four riders, have it on (the new test asserts this).
- `dust.rs`: the emitter pass's emit now does nothing when the emitter was culled at the last view test.
  The check sits after the just-on test and before the off test, and both points are kept.
- Split the pass's emit out as `Emitter::emit_on(node, dt)`. `Emitter::frame` works out the node, then calls
  it. This lets the test drive the emit on the point the original's node gave it.
- New test `tests/ai/wreck_dust.rs` `s477_the_emitter_pass_skips_a_culled_emitter`.
- Nothing taken from `../opentennis`: it has no emitter view cull.

## What the original does

- emit_timing (`./kit fn 0x0014daa0`, called from emit `0x00150540`) runs in this order:
  1. the pause byte;
  2. the node move (prev = pos, pos = node);
  3. just-on (prev = pos, nothing);
  4. delay;
  5. the view cull: `+0x248 && *0x0041f788` means nothing, and prev stays the old pos;
  6. mode 0: off means prev = pos and nothing; on means the speed test.
- The port already ran the view test for every entry once a frame (S422). Only the pass's emit ignored the answer.

## How it was proved

- A live run of the original in the interpreter: `s325-nose76-2604.ee.bin`, interpreted whole for 45 frames
  on the csv's dt (S422's harness).
- Every emit on an exhaust, generic or spray emitter is seeded into the port just before it runs. The test
  then compares two things with the original:
  - the rands it makes;
  - the switch and both points after.
- Result: 1,800 emits with 0 differences. 62 emits found their emitter on and culled, and 91 drew.
- Mutant check: with the culled check taken out the test fails with 266 differences. The first is at tick 2605,
  where prev is kept on a culled emitter that is off.

## Not verified / not 1:1

- **`Dust::view` is not fed from the shell's camera.** The shell does not run a `Dust` yet: only tests set
  `Sim::dust`. Until it does, a ride's view is the bike's own point, so the pass's emitters are never culled
  there. The task says to wire this "once the shell runs a Dust".
- **The camera-present word `*0x0041f788` is not modelled.** In the test it is read straight from RAM.
  The port's view test clears the cull when there is no view, which matches what the original does with no camera.
- **The test takes the emitter's new point from the original.** It hands `emit_on` the original's node point;
  the port's own node points are proved elsewhere (S421, S446). That makes the test a proof of the cull, switch,
  points and draw count logic, not of where the point comes from.
- **The C33 race lanes (`tests/ai/race4.rs`) only check the change by agreement.** Their seeded emitters are not
  culled, and the Sim's view (none) never culls them.

## Next

None: the task is done.
