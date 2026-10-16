# 1 — phy controller keys (FINAL)

Task S21. 2026-10-16.

## What I did

S18's reader took the four rider controllers' sections off the disc into its own `Roll`, `Pitch`,
`Donuts` and `Balance` structs, and stopped there. The ground step's own parameter block — the fields
S17 added to `move::Params` — stayed at `default()`, so a bike loaded off the disc handed the upright
hold, the balance, the wheelie/stoppie pair and the no-hander a zero for every gain, every limit and
every speed band. `DonutRollAngleScale` was the worst of them: its file default is 1.0 and the step
multiplies by it, so the step was seeing a scale of zero where the original has one of one.

Twenty-five words now get copied across inside `phy::Phy::read`, each one right after the struct that
holds it, and none of them is read off the file a second time:

- `[Roll Control]`, nine of eleven — `MaxRollCommand`, `MaxRollSpeed`, `RollGain`,
  `MinStandUprightSpeed`, `MaxStandUprightSpeed`, `RollControlGain`, `RollControlRateGain`,
  `RollControlMaxCG`, `MaxRollRateDisturbance`. `MaxRollRateCommand` and `MaxRollYawRate` are the two
  nothing on the ground reads.
- `[Pitch Control]`, eight of nine — `MaxPitchCommand`, `PitchGain`, `PitchRateGain`, the reciprocal
  of `WheelieTopSpeed`, `MaxStoppiePitchCommand`, `StoppiePitchGain`, `StoppieYawRateScale`,
  `StoppieTopSpeed`. `StoppieBrake` is the one left out, for the reason below.
- `[Donuts]`, one — `DonutRollAngleScale`, default 1.0.
- `[No Hander Lander]`, all seven — the three inertias, `InputInertia`, `MaxBalancePoint` and the two
  noise keys.

Nothing moved and nothing was re-read: the reader keeps its own structs as the loader's record of the
file, and the step's block is now a copy of the same words.

## What the original does

The loader is `./kit fn 0x002539e0` (`bike_phy_load`); the controllers are `./kit fn 0x002561f0`,
`./kit fn 0x00256470` and `./kit fn 0x00255d20`. There are no two readings of these keys in the
original, because there is no second copy: the loader writes one word into the bike and the
controller reads that word back out. The port's two structs are two views of that one word, which is
why a copy is right and a second parse would not be.

**The key the two readings disagreed about: `StoppieBrake`, bike+0x72c.** S18 proved the loader
writes it there, and S17 read the pitch controller's stoppie latch as comparing the front brake
against a literal zero. One of the two had to be wrong about whether the key is live.

`./kit dis 0x00256680 0x002566f0` settles the latch — it is four tests in a row, and the first is:

```
00256694  c6010650  lwc1  $f1, 1616($16)      ; the front brake, bike+0x650
00256698  44800000  mtc1  $zero, $f0          ; zero, out of the zero register
002566a0  46000836  c.ole.s  $f1, $f0
002566a4  45010010  bc1t  68                  ; brake <= 0 -> no stoppie
```

A literal built with `mtc1 $zero`, not a load. Nothing in the whole of `bike_roll_pitch`
(0x00256470..0x00256940) touches +0x72c: a disassembly of the range has loads at 1616, 1620, 1080,
1652 and 1832 and no 1836 anywhere, and the decompilation — which resolves displaced bases — never
names `+0x72c` either. So both readings were right about what they saw, and the key is simply not the
pitch controller's.

It is live, though, and it belongs to the pad layer. `pad_to_gesture` (`./kit fn 0x00250fb0`) turns
the brake button into the lever's demand at bike+0x640, and that is 1.0 for a held button *unless*
the stoppie latch at +0x728 is set, in which case it is `*(bike+0x72c)` — `StoppieBrake`. The bike is
the same object in both: the function reads the latch at +0x728 and the speed at +0x438, both of them
the fields the pitch controller uses. A scan of every instruction in `boot.elf` with a 16-bit
displacement of 0x72c found that read and no other.

So `StoppieBrake` stays in `phy::Pitch`, with a doc comment saying where it is read, and is not one of
the ground step's twenty-five. Whoever ports the pad layer's brake lever needs it; the ground step
does not. Both facts are now in `research/oracle.txt`, under the key list and under `pad_to_gesture`.

## How it was proved

Two halves of one chain, both in `crates/game-sim/tests/phy.rs`:

- **Against the original loader.** `the_whole_loader_fills_the_bike_the_way_the_original_does` (S18)
  already lays the bike out in the interpreter from zero, runs the game's own `.phy` loader on the
  retail file, and compares every word against the port's `Phy` written back at the original's
  offsets — all twenty-five of these among them, at +0x540, +0x6d8..+0x6f0, +0x704..+0x720,
  +0x730..+0x73c and +0xa50..+0xa74. `the_bike_in_the_recording_is_the_one_the_port_reads` does the
  same against the live bike in the save state beside the human's recording. Both still pass
  untouched, so the reader's structs are still the loader's own numbers.
- **Against the structs, bit for bit, on every bike on the disc.** The new
  `the_ground_step_gets_the_controller_keys_off_the_disc` reads all 18 distinct `.phy` files out of
  the `Bike\*.res` archives and asserts `to_bits()` equality for each of the twenty-five between
  `Phy::mv` and the struct the loader filled, naming the file and the key on failure. It also tracks
  each key across the whole set and fails if one of them is zero in every file — otherwise an
  assertion comparing two zeros would hold whether the copy happened or not. All twenty-five carry a
  number somewhere, so all twenty-five assertions have something to say.

Together: the step's word equals the struct's word, and the struct's word equals the word the
original's loader wrote into the bike.

`./kit check`: 435 tests pass.

## Not verified / not 1:1

- **The values are not proved to reach the running step.** This task proves `Phy::read` fills
  `move::Params`; it does not drive a frame with a disc-loaded bike and compare against the original.
  S17's controller proofs use their own fixtures, and nothing yet builds a bike's `Params` from the
  disc inside the replay harness. That is the proof that would close it, and it needs the harness to
  take a `.phy` first.
- **`StoppieBrake`'s one reader is read, not run.** That `pad_to_gesture` swaps the brake demand for
  `StoppieBrake` while the stoppie latch is set comes from the decompilation of 0x00250fb0, not from
  a capture — nothing on that path is ported yet, so there was nothing to compare against. The claim
  this task actually depends on is the negative one (the pitch controller does not read the key), and
  that one is from the disassembly of the latch and of the whole function.
- **The "only read of +0x72c" scan is displacement-matched.** It walked every PT_LOAD word of
  `boot.elf` for a load or store whose low half is 0x72c. An access through a base the compiler had
  already offset — `addiu` to bike+0x700 and then a displacement of 0x2c — would not show up in it.
  Inside `bike_roll_pitch` that gap is closed by the decompilation, which resolves bases; outside it,
  the statement is as strong as the scan and no stronger.
- **`MaxRollRateCommand` and `MaxRollYawRate` are left out on S17's reading.** They are in `phy::Roll`
  and proved there against the loader, but no ground-step field takes them because S17 found nothing
  on the ground reading +0x6dc or +0x6e4. Not re-checked here.

## Next

Nothing outstanding for S21. The first thing worth doing with this is the proof listed above: give
the replay harness a bike read off the disc, so the controllers are driven with the file's own numbers
rather than a fixture's.
