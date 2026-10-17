# 1 — p9 analog throttle (FINAL)

Task P9. 2026-10-17.

## What I did

Three halves, all additive.

**1. A real pad's per-button pressure reaches the gas.** `crates/openmx/src/pad.rs` already built a
32-byte DualShock 2 report by hand; what it never did was fill the twelve per-button *pressure* bytes,
so every face button arrived at the game as a switch. The Linux joystick device publishes a
pressure-reporting pad's buttons as extra *axes* (after the gamepad's own eight), and which axis carries
which button is the driver's business, not ours — so the port takes the layout as an argument:

    openmx --pad-pressure ds2@8          # twelve pressures on axes 8..19, in DS2 report order
    openmx --pad-pressure 14:cross,19:r2 # or name them one at a time

`Wiring::parse_pressures` parses either form and refuses what it cannot honour (an axis past the 32 the
module reads, an unknown control name, or a control the game reads from its *bit* rather than a pressure
byte — `Binding::is_analogue`). `AXES` went 8 → 32 to make room; an axis nobody mapped is never read.
For each mapped axis whose button is down, the axis' travel becomes that control's pressure byte.

**2. Part throttle on hardware that has no pressure.** Two routes, because the two have different feels:

- `--throttle r2` (or `l2`) rides gas on a trigger's analogue travel. The wired trigger gives up its own
  control for the session — a front brake that came on with the gas would be worse than no part throttle
  — and the shell says so at startup.
- The keyboard's throttle key (X / Space) now *ramps*: `KeyRamp` opens the throttle over
  `KEY_TO_FULL = 0.30 s` and closes it over `KEY_TO_SHUT = 0.12 s`, publishing a real pressure byte each
  frame instead of 0xff on frame one. Both numbers are the port's choice (see Not verified), picked
  against the disc's own `GasRateLimit = 10` (0.1 s shut-to-pinned): a ramp faster than that would be
  invisible, because the bike's limiter would swallow it.

Where two sources ask for the throttle at once, the harder one wins (`press()` takes `max`), and a
source below `PRESSURE_THRESHOLD` (0x19) leaves the *bit* up — which is exactly how the game decides a
button is held, so a feathered key or trigger does not read as a press.

**3. What the original actually does, counted.** `crates/game-sim/tests/replay.rs` gained
`the_human_recording_is_a_switch_and_the_part_throttle_in_it_is_the_rate_limit_s`, which walks all
1,810 ticks of the human's `first_recording.p2m2` through the port's own poll and the bike's throttle
lag. The finding is in the next section; it is not what the brief expected, so the test asserts it
exactly and the numbers are in its output line.

## What the original does

**The throttle was never a switch on the disc.** `CONTROL_MAP`, read out of the game's own RAM, marks
Cross analogue (flags bit 1), which means the game reads the gas from Cross' *pressure byte* in the pad
report and not from its bit. `Controls::from_pad` turns that byte into `gas = (pressure + 1) / 256`, so
a DualShock 2 pressing Cross half way asks for half throttle. A control the game reads from its bit
(`!is_analogue()`) has no pressure byte to fill, which is why `--pad-pressure` rejects one.

Behind the gas, `Bike\Race125P.phy`'s `[Joystick]` block has `GasRateLimit = 10` — read off the disc
with the port's own `.phy` reader (`Psr` → `Res` → `Ini` over `context/iso/MX.PSR`), not taken from the
brief. At the engine's 30 Hz tick that is shut-to-pinned in 0.1 s, three ticks, which is the lag
`Held::step`'s `toward()` already models.

**The human's minute is a switch, and the only part throttle in it is the limiter's.** Over the 1,810
ticks of the phase the game reads (`TickPhase::Second`, proved in S8):

| | |
|---|---|
| ticks with Cross pinned (0xff) | 1,303 |
| ticks with Cross shut (0x00) | 507 |
| ticks with *any* analogue control strictly between 0 and 255 | **0** |
| throttle edges / stabs | 18 / 9 (longest run 346 ticks ≈ 11.5 s, shortest 8) |
| left-stick distinct bytes | 154 |
| ticks where the bike held part throttle anyway | **54 (3.0%)** — three after each of the 18 edges |
| ticks where it held full | 1,276 |

The left stick's 154 distinct bytes say the recording's host pad *was* analogue where it had an axis; it
simply had no per-button pressure to report, so every face button landed as 0 or 255. So: the human
asked for part throttle on none of the 1,810 ticks, and the 54 ticks of part throttle the bike actually
saw are `GasRateLimit` filling in, not a request. For the port that is the useful half of the finding —
the shell's 0.3 s key ramp spends nine ticks climbing where this minute spent three.

Bonus, unproven: the boot ELF's string table carries `MaxAnalogThrottle` (`0x443950`) and
`DigitalThrottleSpeed` (`0x443970`) beside a second `GasRateLimit` (`0x443938`) — the car/truck physics
model's `[Joystick]` keys, near `MXCarEngineModel.cpp`. The bike's own block has only `GasRateLimit`.
So the original names an analogue throttle separately from a digital one, for four-wheeled vehicles.
This is a string-table observation only (see Not verified).

## How it was proved

- **The measurement** — `cargo test -p game-sim --test replay
  the_human_recording_is_a_switch_and_the_part_throttle_in_it_is_the_rate_limit_s`, over the human's
  `context/recordings/first_recording.p2m2`: 1,810 ticks × 12 pressure bytes read through
  `input::Manager::connected()` + `ride::Controls::from_pad` + `Held::step` at `replay::CAPPED_DT`, with
  `Joystick { gas_rate_limit: 10.0, .. }`. It prints its counts and asserts every one of them. Private
  data, so it prints why and returns when `context/` is absent, like the rest of that file.
- **A pressure byte between the stops is that fraction of the throttle** — all 230 bytes from 0x1a to
  0xff through the same poll, each asserted equal to `F((byte + 1) / 256)`, plus `PRESSURE_THRESHOLD`
  itself asserted to read as shut (the bit stays up at the threshold).
- **A pad's own button pressure reaches the gas** — `--pad-pressure ds2@8` puts Cross on axis 14; bit
  down with a quiet axis reads full, the axis at mid-travel reads ~half, a dead-zoned axis reads full,
  and the bit up reads zero.
- **A trigger throttle is part throttle and gives up its own control** — a half-pulled wired R2 reads
  part gas with `front_brake == ZERO`; the same pad unwired reads `(gas ZERO, front_brake ONE)`.
- **The keyboard throttle ramps rather than stepping** — frame one is below threshold (`F::ZERO` where
  the old code read `F::ONE`), the climb is monotone and reaches full in 15–22 frames at 1/60 with more
  than 10 part-throttle frames on the way, a three-frame stab asks for less than 0.3, and the close is
  monotone and finishes inside 15 frames. Two sources on the throttle assert the harder of the two.
- **The shell still rides** — `openmx --seconds 3 --hud --throttle r2 --pad-pressure ds2@8 --shot
  context/shots/p9-wired.png --shot-frame 60`: 113 frames, the shot written, the new wiring lines in the
  startup log.
- `./kit check`: **TESTS PASSED** (520).

No emulator copy was started: the `.p2m2` carries the pressure bytes, so the measurement needed the
file and the port's own reader, nothing else.

## Not verified / not 1:1

- **No pressure-reporting device was ever tested.** There is no `/dev/input/js*` on this machine at all,
  so the jsdev → pressure-byte path and the trigger → gas path are proved in unit tests through the
  module's own `Pads` (fed synthetic axis values), not against hardware. The jsdev read itself is the
  one P6 found and fixed live, so the risk is in the *mapping*, not the plumbing.
- **`ds2@N` is an offered convention, not a measured layout.** Which axes a driver publishes a DS2's
  twelve pressures on, and in what order, was never observed here; `DS2_PRESSURE_ORDER` is the *report's*
  order (the one the game reads), which a driver need not follow. That is why there is no built-in
  per-device table and the layout is an argument — a wrong default would read a stick as a throttle.
- **The keyboard ramp's 0.30 s / 0.12 s are the port's numbers.** Nothing on the disc describes a
  keyboard; the disc's input is a pad. They are chosen against `GasRateLimit = 10` so the ramp is
  slower than the limiter and therefore visible, and nothing more than that.
- **Which command Cross drives is the port's binding, not the disc's.** `Controls::from_pad` maps
  Cross → gas, R2 → front brake, L2 → clutch. The game's own command→control binding was not read out
  of the original; only the fact that Cross is *analogue* is (from `CONTROL_MAP`).
- **The recording's digital pressures are a fact about PCSX2 plus the host pad.** A `.p2m2` can only
  carry what the host pad reported, and this one reported 0/255 for every button. It therefore says
  nothing about a DualShock 2 on real hardware, and the "0 ticks of asked-for part throttle" finding
  must not be read as "the original cannot do part throttle" — the pressure path above says it can.
- **One human recording, one minute.** 1,810 ticks on one bike by one rider; a second recording could
  easily hold feathered throttle.
- **`MaxAnalogThrottle` / `DigitalThrottleSpeed` have no code behind them here.** They are strings in
  the boot ELF (`0x443950`, `0x443970`; `./kit fn` has nothing at either, and `context/decomp/boot.c`
  does not mention them), so the claim is "the original has these ini keys for the car/truck model",
  not "this is how it reads them". Nothing in the port depends on it.
- **The `.phy` number is read, the lag is not re-derived.** `GasRateLimit = 10` comes off the disc, but
  that it means "units per second toward the target" is `Held::step`'s existing model from an earlier
  task, not something this task re-proved against the original.

## Next

Nothing is waiting on a decision. If a pressure-reporting pad ever reaches this machine: run
`openmx --pad-pressure ds2@8` with `jstest` beside it to find the real axis order, and the first three
Not-verified items collapse into one measurement. Writing a second human recording that *feathers* the
throttle would also turn the measurement above from "this minute is a switch" into a real curve to
compare the port's ramp against.
