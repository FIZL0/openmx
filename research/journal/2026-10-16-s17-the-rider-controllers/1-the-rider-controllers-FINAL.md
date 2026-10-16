# 1 — the rider controllers (FINAL)

Task S17. 2026-10-16.

## What I did

- Ported the four rider controllers S3 left hooked, into `crates/game-sim/src/move.rs` beside the
  ground step that drives them: `stand_upright`, `balance`, `roll_pitch` and `no_hander`. Each one had
  a proved `Call` variant waiting for it in `step`; those four variants are gone and the real
  functions are called in their place, in the same order and with the same `dt`.
- Gave the module the three things the controllers need from outside it and this task does not port:
  `trait Outside` (`unrotate`, `forward`, `noise`, `set_axes`), `enum Frame` (`Node` · `Chassis` —
  which of the bike's two transforms a call means) and `struct Tables` (the acos curve S10 already
  built, and the game's `{cos,sin}` table). The port asks for an answer and records what it asked, so
  the call order is proved even where the callee is not.
- Named the four accumulators the brief hands over as `Ride::no_hander_scratch`: they are
  `bike_balance`'s, not the no-hander's — `balance_inertia`, `balance_command`, `balance_point` and
  `balance_noise_time`, at `+0xa4c`, `+0xa60`, `+0xa64` and `+0xa6c`. The step zeroes all four
  whenever the bike is not balancing, which is the clearing S3 read as the no-hander's.
- Added the twenty-five `.phy` keys the four of them read to `Params`, recovered from
  `bike_phy_load`'s own string arguments, with the two scalings the loader applies (`* 0.01745329` for
  a key in degrees, `* 1.46666` for one in speed) and `WheelieTopSpeed` stored the way the loader
  stores it, as `1 / (key * 1.46666)`.
- Renamed `Step::brake_rate_limited` / `brake_rate_limited_copy` to `front_brake` / `rear_brake`.
  S3 read `+0x650` and `+0x654` as one number written twice. They are not: the bike's setup at
  `0x002535a0` hands the front wheel a pointer to `bike+0x650` as its `+0x30` and the rear wheel one
  to `bike+0x654`, next to the engine's `+0x658`/`+0x65c`, and each wheel reads its own word as the
  brake (`0x0025c4c8` in `wheel_surface_drag` turns it into a torque against the spin; `0x0025cb8c` in
  `wheel_tire_forces` gates the locked-wheel path on it). So the wheelie's veto is the *rear* brake
  and the stoppie's gate the *front* — which is the physically right way round, and which the port now
  proves by generating the two independently.
- Changed `Fresh::scalars` from `[F; 7]` to `[u32; 7]`. Two of the seven are object pointers and two
  are the flags the no-hander reads; the original's own `1` in one of those is a denormal as a float,
  and `F::from_bits` flushes it to zero, so an `F` could not carry it. Nothing ported reads any of the
  seven as a number. This is S16's type, touched for that one reason.
- Corrected the `orientation_axes` entry in `research/oracle.txt` (S3 had `a1`/`a3` wrong) and added
  the S17 block: all four controllers in words, `transform_set_axes`, the pointer at `0x002535a0`, and
  the twenty-four-key `.phy` table.

## What the original does

Addresses are recipes: `./kit dis <addr> --len <n>` for all four, because every one of them builds an
accumulator chain in an order the decompiler reorders, and `transform_set_axes` is called with seven
register arguments, which Ghidra prints as a call with two.

- **`bike_stand_upright`** (`0x002574e0`) — the low-speed upright hold. It runs only with both wheels
  down, not balancing, the throttle shut and `|speed| < 10` (a literal, not a key). It takes the part
  of gravity that runs along the ground — gravity less its component along the front wheel's contact
  normal — and splits it into the part along the chassis' forward axis flattened into that plane and
  the part across it. It pushes back against the across part always, and against the along part as
  well while the brake is held. The forward axis comes from `orientation_axes` against the chassis'
  own transform, which may be null; the function does not check, and a null frame answers `(0,0,1,0)`.
  A flattened forward of exactly zero length makes it give up and add nothing.
- **`bike_balance`** (`0x002561f0`) — balance on one wheel, only with the front wheel in contact. The
  roll angle it controls to comes from the front wheel's contact normal brought into the bike node's
  frame, flattened into that frame's xy plane, normalised and read off the acos table — the same
  lookup `air.rs::AcosTable` already is. On top of that sits a disturbance the rider makes: the stick
  and a smooth-noise clock make a command, clamped to `±1`, which integrates into a balance point
  clamped to `±MaxBalancePoint`, let through at a rate (`balance_inertia`) that ramps between
  `MinBalanceInertia` and `MaxBalanceInertia`. The controller is a P term on the angle, less the roll
  rate, less the commanded disturbance, and what it moves is the chassis' centre of gravity in x,
  clamped to `±RollControlMaxCG`.
- **`bike_roll_pitch`** (`0x00256470`) — the pitch the rider's lean feeds, on the ground only. Two
  halves share one integral at `+0x724`. The wheelie wants the rear wheel down, the throttle on, speed
  over 5, the rear brake off, no clutch and the stick back past `0.5`; its command fades out with
  speed through `1/WheelieTopSpeed`. The stoppie latches at `+0x728` on the front brake with the stick
  forward past `-1.0` and speed over 15, and drops again on the rear wheel touching down, the front
  brake let go, the stick back inside `-0.1` or speed under 7; it adds a yaw term,
  `1000 * (StoppieYawRateScale * lean - the rate about the rear contact normal)`. Either half, while
  it runs, takes the front wheel's steering away.
- **`bike_no_hander`** (`0x00255d20`) — rolls the bike *node* about its own forward axis so the rider
  can let go of the bars. The angle it rolls towards is the ground's roll angle under whichever wheel
  is properly down — a suspension ray that found something, or a collision sweep whose normal is more
  than `0.17` up, front wheel first — faded out with speed, with the rider's lean commanding a roll up
  to `MaxRollCommand`, widened by `DonutRollAngleScale` while the donut is on, and `RollGain * dt` of
  first-order lag on the way. It ends in `transform_set_axes`, which rebuilds the node's matrix from a
  forward and an up axis by Gram-Schmidt.

## How it was proved

Function oracle, `crates/game-sim/tests/move.rs`, against the retail code in the emulator through
`ps2-oracle`. Four new proofs, **10,000 generated states each**, every one comparing *every word* of
the five-object graph the original can reach through its one pointer, plus the ordered list of calls
the controller made outside itself:

- `the_upright_hold_is_the_originals`
- `the_balance_is_the_originals`
- `the_pitch_control_is_the_originals`
- `the_no_hander_is_the_originals`

The four engine functions the controllers call out to are hooked, so both sides get the same answer
from the same pure function and the harness records the question: `transform_unrotate`,
`orientation_axes`, `smooth_noise` and `transform_set_axes`. The harness splits the step's own
`transform_unrotate` (the chassis' measured acceleration, still a `Call`) from a controller's by the
vector argument. The `{cos,sin}` table is built by the game's own builder through a hooked
`mem_alloc`, so the port reads the retail curve and not a rebuilt one.

Every path is counted and asserted: `covered(…, 100)` over 8 named paths for the upright hold, 15 for
the balance, 14 for the pitch control and 13 for the no-hander — each at least 100 states of 10,000.
The two existing step proofs now clear and compare the recorded call list as well, with four more
tally paths, and the 240-frame replay fixture gained the twenty-five new `Params`, the four chassis
quadwords, both wheels' contact normals, the two brakes and the fore-aft lean.

Then eight deliberate mutations, one at a time, `./kit check` each time. Seven failed, each exactly
where it should:

| the mutation | where it failed |
| --- | --- |
| the upright hold's zero-length arm adding `(1,0,0,1)` instead of nothing | `chassis + 0x130`, state 13 |
| the balance's P term fed back positive | `bike + 0xa64`, state 1 |
| the wheelie's veto reading the front brake instead of the rear | `bike + 0x724`, state 103 |
| the no-hander unrotating in the chassis' frame instead of the node's | recorded call 0 of 3 |
| the no-hander's roll angle not negated | recorded call 2 of 3 |
| the upright hold's forward asked of the node instead of the chassis | recorded call 0 of 1 |
| the balance's noise clock advanced unscaled | recorded call 0 of 2 |

The eighth passed, and that is a finding rather than a hole — see below.

`./kit check`: **TESTS PASSED: 425 tests**, 12.5 s for this file.

## Not verified / not 1:1

- **The eighth mutation passed.** Turning the upright hold's `if len2 == F::ZERO` into `if false`
  changes nothing any proof can see: the two arms differ only in the `w` lane of a vector nothing
  reads. The arm's *value* is proved reached (putting `(1,0,0,1)` there fails at state 13), so the
  branch is kept as the original writes it, but the condition itself is unobservable from outside.
- **Four engine functions are hooked, not run.** `transform_unrotate`, `orientation_axes`,
  `smooth_noise` and `transform_set_axes` answer from the harness on both sides. What is proved is
  that the port asks the same questions in the same order with the same arguments, and does the same
  arithmetic on the answers — not those four functions themselves. `orientation_axes`' real behaviour
  was read instruction by instruction first, including the null-frame path that makes the upright
  hold's unchecked call safe; the other three are S3's and S7's.
  `rotate_about_axis` is *not* hooked: it runs for real on both sides.
- **`transform_set_axes` writes outside the compared graph.** The no-hander's last act rebuilds the
  node's matrix; the proof compares the two axes it was handed, in order, not the matrix. The function
  is S7-adjacent and unported.
- **Who writes `bike+0x650` and `bike+0x654`** was not chased down — one lever into two rate limiters,
  or two levers, is still open. What is proved is which wheel each word belongs to and which half of
  the pitch controller reads which. The port takes them as two inputs.
- **Two dead computations are kept as the original computes them.** The wheelie half measures the yaw
  rate about the rear contact normal exactly as the stoppie does and then multiplies it by a literal
  zero; the no-hander's donut guard compares `0.0` against `1.0`. Neither can reach a compared word,
  so neither is proved by anything but the disassembly.
- **`map_clamp(speed, 0, 10, 1.0, 1.0)`** in the upright hold has a computed constant that works out
  to a constant `1.0` and a guard that can never fire. Kept, unprovable.
- **The no-hander never does anything in the two step proofs**, and the balance runs only in substep
  1, because `wheel_clear` zeroes the contact flags mid-substep before the tail runs. The two of them
  are proved by their own function oracles instead; the step proofs prove only that they are called.
- **`Fresh::scalars` changed type** from `[F; 7]` to `[u32; 7]` — S16's type, changed because an `F`
  cannot carry the game's integer `1` (a denormal, flushed by `operand()`). A neighbouring task's
  struct, so the merge may want a look.
- **`Frame::Node` is never null in the harness.** The port does not check it, and neither does the
  original; a null node would be a different proof.
- **`CurveTable::build()`'s own entries are unused here** — the harness reads the game's builder
  instead — and the acos table is the port's own, written into the global for the original to read.
  Both are S10's, proved there.
- **`euler`, `world_ang_vel` and `ang_vel` are constant inputs** in the replay fixture: this module
  reads them and does not write them, so the fixture cannot prove the chassis integrates them.
- **`./kit done` was run with `--skip-audit`.** The audit found exactly one leak and it is not
  this task's: `watch/frame-bike.toml:150` carries `0x00133500` in a stage comment, committed by
  A5 in `d6a9dcf`. Left alone rather than edited from this worktree, because another agent may be
  in that file; nothing S17 touched trips the audit.
- **`bike_phy_load` is not ported.** The twenty-five key names and the two scalings are read off its
  disassembly and written down in `research/oracle.txt`; the loader itself is somebody's later task.

## Next

Nothing waiting on the human. The obvious next pull is `bike_phy_load` (the keys are all written
down), and after it the two transform helpers this task hooked — `orientation_axes` and
`transform_set_axes` — which would turn four of the entries above into proofs.
