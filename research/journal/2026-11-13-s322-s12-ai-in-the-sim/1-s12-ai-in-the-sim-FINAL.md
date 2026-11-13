# 1 — the rider's interface defaults and the race start's AI branch

Task S322. 2026-11-13.

## What I did

- **`ai::interface_profile() -> Profile`** gives the rider's side of the AI as the rider's
  constructors leave it, before the character file is read. These are the words `read_character`
  keeps where `AIChrctr.aip` has no key.
- **`ai::interface_habits() -> Habits`** gives the rider's own habit words that the profile does not
  hold, as the constructors leave them:
  - preload 1;
  - lag −1;
  - the brilliant moment's roll clock 1;
  - the low-air bar 0.15;
  - every clock 0.
- **`ai::spawn`** now starts from both. Before, it used `Profile::default()` and `Habits::default()`,
  so a fresh racer had lag 0, preload 0 and low-air bar 0, which is wrong.
- **`Racer::race_start(&Profile, keeper_state, &mut Rng)`** is the AI branch of the bike's race start.
- **`KEEPER_THREE_REACH_GAIN`** (0.2) is the reach gain the race start uses when the score keeper is
  in state 3.
- In `research/oracle.txt`:
  - new names: `bike_construct`, `bike_construct_interface`, `bike_construct_epilogue`,
    `ai_interface_construct`;
  - `bike_race_start`'s note now describes the AI branch.
- Opentennis has nothing like this: the AI interface is specific to MX Unleashed. Nothing was taken
  from it.

## What the original does

- **The rider's interface** is a member of the bike entity, at bike+0x10d0. The bike's constructor
  (`./kit fn 0x00279ed0`) first runs the interface's own constructor (`./kit fn 0x00382d90`):
  - +0 brain = 0;
  - +0x14..+0x108 = 0;
  - +0x10c..+0x118 = 1, 1, 100, 20;
  - +0x14c = 0.2;
  - +0x150.. its numbers, up to +0x1e0.

  Then the bike's constructor writes the habits' numbers onto the interface (rider +0x10e4..+0x11d0):
  - preload base 0, preload 1, 0.1 per foot a second;
  - preload success 100 ×4;
  - pull-back 50 / 0.6;
  - lag −1;
  - horse power 1 ×4;
  - brilliant moment 2.5 + 4 random, roll clock 1;
  - start wheelie 0.5–1.5, pull-backs 0.65/0.75 with the clutch and 0.9/0.9 without;
  - cross-up 0.5–1 s above 10 ft;
  - low-air bar 0.15.

  Neither constructor writes +0x120..+0x148 (the ALOD pairs and `LookAheadGain`).
- **The race start** (`./kit fn 0x0027f5e0`, AI branch, iface = bike+0x10d0) does this in order:
  1. Sets the gate clock (+0xa4) to 0.
  2. If the keeper (bike+0xfa0) is in state 3: wait (+0x11c) = 0.75·unit(), and the reach gain
     (brain+0x218) = iface+0x14c. Otherwise: wait = the gate delay (+0xa0), and gain = `LookAheadGain`
     (+0x148).
  3. If `100·unit() < ClutchStartProbability` (+0xa8): set flag 0x20 on brain+0x38 (`GATE_CLUTCH`),
     and the wheelie lean (+0xd8) = lerp(+0xb8, +0xbc, unit()). Otherwise: clear the flag, and the lean
     = lerp(+0xc0, +0xc4, unit()).
  4. If `100·unit() < WheelieStartProbability` (+0xac): set flag 0x100 (`GATE_WHEELIE`), and the
     wheelie clock (+0xc8) = 0. Otherwise: clear the flag.

  That is three rand() draws, or four in state 3. The task text says "start-gate draws and the
  cross-up/brilliant draws". The branch really draws for the gate wait, the clutch start (and its
  lean) and the wheelie start. It draws nothing for the cross-up or the brilliant moment.

## How it was proved

`crates/game-sim/tests/ai/interface.rs`:

- **`the_interface_defaults_are_the_constructors`** runs a stretch of the bike's constructor in the
  interpreter. It starts at the interface constructor's call (s0 = bike, f20 = 1.0 as the earlier part
  sets it) and ends at the epilogue. The bike is filled three ways: zeros, and two NaN patterns. Only
  the class-descriptor registration is hooked: it allocates and calls GetThreadId, and its two stores
  are kept. The test compares:
  - all 56 profile words through `character::ch_slots`;
  - 30 habit and assist words of a freshly spawned racer;
  - +0x14c = 0.2.

  The 11 words nothing writes must still hold the fill, and the port must have 0 for them. Everything
  matches.
- **`the_race_start_matches_the_originals`** runs `bike_race_start` whole, 10,000 times, against the
  port. The cases vary:
  - the generator anywhere in its chain;
  - no keeper, a keeper in state 3, or a keeper in another state;
  - the chances from −10 to 110, with 0 and 100 as edges;
  - the pull-back pairs, sometimes equal;
  - the flag word and the written words pre-filled at random.

  It compares the generator, the flag word (64 bits), the gate clock, the gate wait, the wheelie clock,
  the wheelie lean and the reach gain, all bit for bit. Every branch is taken more than 3,600 times.
- **`spawn.rs`'s free09 test** now uses the port's defaults and runs the race start for real. It reads
  the keeper's state off the image. It now also compares the reach gain (76 brain words in all), and
  checks the image's +0x14c = 0.2. All three riders match.

## Not verified / not 1:1

- **The ALOD pairs and `LookAheadGain` (interface +0x120..+0x148) are 0 in the port.** The original
  leaves whatever the bike's allocation held. On free09 the retail file sets them for the riders' skills
  (the three riders match the image); other skills were not checked, but a mod character file without those keys would read the allocation in the original.
- **Only a stretch of the bike's constructor is run, not the whole thing.** The earlier part allocates
  arrays and builds many sub-objects. That nothing before the stretch writes the interface (past
  +0x10c8) is read from the decompilation, not proved.
- **The interface's words with no port field are not modelled:**
  - +0xc and +0x10, the class descriptor;
  - +0x10c..+0x118 = 1, 1, 100, 20;
  - +0x150..+0x1e0;
  - the rider's +0x18 = 20, +0xf8 = 10 and +0x100 = 0.5.

  No ported code reads them yet.
- **That nothing else writes +0x14c is only partly checked.** The character reader writes +0x148 only.
  A scan of the whole ELF finds no store to it relative to the rider (+0x121c), and the free09 image
  holds 0.2. A store through some other pointer to the interface cannot be ruled out by the scan.
- **The race start's human side is not part of this port.** That is the chassis's release flags and
  the bike's started word (+0x610), and it belongs to `race::release`. Its no-brain branch is not run
  against the port.
- **The race start's draws are not compared on free09.** The image does not record the generator's
  state at the start. The gate wait and wheelie lean in the image come from those draws.
- **Nothing calls `race_start` in the Sim yet.** That is the AI lane, S323 (renumbered from S319).

## Next

None for S322. S12e still waits on S323 and C33.
