# 1 — s12 ai character (FINAL)

Task S283. 2026-11-10.

## What I did

- Ported the AI character reader as `ai::read_character(file, skill, difficulty, rng, &mut Character)` in
  `crates/game-sim/src/ai.rs`. It fills `ai::Tuning` (the controller gains), `ai::AvoidTuning` (the S12c
  keys: near/far collision ranges, braking and response, lateral windows, passing speed, and the near
  response lerped over Min/Max/OverDrive), the ALOD block, the two steering filter quadwords, the offset,
  drift, look-ahead, roll and stunt keys, and a `Profile` with the rider-side values (preload, pull-back,
  landing lag, horse power, over-power tweaks, brilliant moment, g-load wreck, start-gate and wheelie
  start, crossup). The new types are `Pair`, `Triple`, `Spread`, `Alod`, `Profile` and `Character`, and
  `ai::difficulty_fraction(d) -> (t, lower)`.
- Added `phy::Section::found()`: whether the section select hit.
- Oracle test `the_character_reader_matches_over_ten_thousand_files_{1..4}` in `crates/game-sim/tests/ai.rs`
  (four quarters of 2,500 cases each, so the harness runs them side by side; the whole ai binary takes ~20 s).
- research/oracle.txt: expanded the `ai_character_read` entry (miss semantics, difficulty maths,
  post-steps, the draws).

## What the original does

`./kit fn 0x00384f00` (the rider's controller as the argument; +0 is its brain, +0x104 the skill).

- Opens `<rider prefix>AIChrctr.aip` (the prefix holds `Rider\` at run time, so the retail file is
  `rider/aichrctr.aip` in `Bike\250sx.res`). If the file is missing it writes nothing and draws nothing.
- Difficulty d = the event's byte +0x117 (app pointer → +0x3d4 → +0xc). With d <= 100, t = 0.0333333·(d-70);
  otherwise t = 0.04·(d-100). Min/Max pairs are `mula.s min,(1-t); madd.s max,t`. Triples take min..max
  below 101 and max..overdrive above. ALOD pairs lerp below 101 and copy max above.
- It reads `DefaultALODSettings` first, then selects `AI_<skill>`. **A missed select leaves no section
  selected.** Every key after that reads as missing; the reader does not fall back to the ALOD section.
  The oracle settled this; my first port got it wrong.
- There are three getters. One leaves the destination alone when the key is missing, one always writes
  (with a default), and the quadword one defaults to the destination. All of them parse with strtod and then
  truncate the double to a float. The port reads through `phy::Ini` (`float`, `float_or`, `vector`).
- Some post-steps run even when the key is missing:
  - Near/FarCollisionRange are square-rooted.
  - The separation rate is ×0.1.
  - The landing pitch is ×π/180.
  - ALODSpeed is ×1.4666667 (only in the ALOD block).
  - Preload-per-mph (default 10) is ×0.006818182.
- After WheelieMax…WithoutClutch there are two rand() draws: the start-gate delay *= unit, then
  wheelie start max = 0.5 + (x-0.5)·unit.
- The defaults of the always-write keys are listed in the doc comment of `read_character`. One example:
  NearCollisionMinBrakingSpeed defaults to 20 here, while the constructor uses 40.

## How it was proved

I ran a function oracle over 10,000 cases, comparing every word of the brain (0x400 bytes) and the
controller (0x200 bytes), both random-filled before each call, plus the rng state:

- **Retail runs:** the retail `aichrctr.aip` once per skill 0..12 (13 cases). The test skips that part if
  MX.PSR is absent.
- **Missing file:** 1 case in 50.
- **Generated files:** everything else. Each has random sections, missing keys and junk lines, and
  skills -2..11. Every 4th case takes its difficulty from edge values (0, 69, 70, 71, 85, 99, 100, 101, 125,
  255); the rest are random. About 1 in 8 cases ask for a missing `AI_<skill>` (1,228 of 10,000).
- **Real parse vs. hooked lookups:**
  - The first 40 generated files of each quarter, every retail case and every missing-file case go through
    the original's own ini parser over hooked fs calls. Those cases also check the opened file name. 373 cases ran this way.
  - The rest hook ini_open/ini_section/ini_text to answer from `phy::Ini`, which tests/phy.rs proves. That
    took the test from 654 s to about 20 s.

Mutant check: changing `d <= 100` to `d < 100` in `difficulty_fraction` fails the oracle.

## Not verified / not 1:1

- **The constructor start values of Character/Profile are not ported** (`Character` has no `Default`).
  Keys the file lacks keep whatever the caller had. The S12e wiring needs the constructor's values, noted
  on S12e, which spawns the riders.
- **Hooked-lookup cases trust phy::Ini.** About 9,600 of the 10,000 cases answer ini lookups from the port's
  parser rather than the original's. That parser has its own oracle in tests/phy.rs, and the 373
  real-parse cases cover the joint path.
- **A Min/Max pair before the first section crashes the original** (it files under section -1). The port
  does not model the crash, and the generator never writes such a file.
- **Quadwords with fewer than three lanes are not generated.** The original then hands NULL to strtod.
- **Profile field names are guesses** from the key names. Where the game consumes those rider-side values
  (+0x20…+0xf4 of the controller) is not ported or checked.
- **Nothing calls `read_character` yet.** Wiring it into the AI rider is S12e's.
- **The ini reader's cache branch** (the reader object's mode 1 in the section select) was not exercised
  on its own. Retail cases run through whatever mode the open sets.
- **The size of the reader's ini allocation (0x850 bytes) was not checked** for meaning. The test heap
  only has to serve it.

## Next

Wire `read_character` into the AI rider's construction (S12e) together with the constructor defaults.
