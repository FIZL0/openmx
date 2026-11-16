# 1 — c33 other draws (FINAL)

Task S328. 2026-11-16.

## What I did

- Ran C33's frames whole in the interpreter with a hook on rand, starting from `free09-ai4.ee.bin`:
  device update, frame marker, counter, the root's tick, second marker. The run is copied from
  tests/ragdoll_wrecks.rs, minus its pad laying. The new test is
  `crates/game-sim/tests/ai/draws.rs`, `s328_c33s_other_draws_are_looks_notes_and_dust`. It runs
  60 ticks in about 12 s.
- Each draw is sorted by its return address into a named function's range (sizes from `./kit fn`):
  - the look timer (rider_animation);
  - the engine note (sound_pick and the engine_note_* states);
  - dust (emit, emit_other);
  - the AI (ai_feed, ai_habits).
  The test fails on any draw from an unnamed site.
- **The pad.** The image's pad buffers hold an old digital report. With it, the interpreter's human
  parts at tick 4, just as the Sim's does (S329). Laying a fresh neutral analog report in both
  halves every frame (status 0, mode 0x73, buttons 0xffff, sticks 0x80) keeps all four bodies bit
  for bit on centre, quat, velocity and spin through tick 62. The test asserts this. It is also
  S329's answer, noted there.
- **The split.** Drawing these in the Sim touches three systems the Sim does not run today (the body
  animation, the engine note, the particles), plus one not yet found. So S328 records and proves the
  sites, and each system is its own follow-up:
  - S328a: the look timers;
  - S328b: the engine note;
  - S328c: the dust;
  - S328d: the render's draws.
- `race4.rs`: `C33_STATE`, `C33_RIDERS` and `C33_NAMES` made `pub(crate)`. Nothing else changed.
- oracle.txt: new `emit_other` entry (the second emit path), and a note listing the S328 draw sites
  with their return addresses.

## What the original does

Draws on the shared generator in C33 (free09, four racers), per tick, that are not the AI's:

- **Tick 7: 3 draws, tick 8: 1 draw.** The body look timer re-arms (`./kit fn 0x00248a70`; the draw
  returns to 0x00249208). The animation object is the one at rider +0xdb4: ai2, ai0 and ai1 at
  tick 7, the human at 8. This is the re-arm in rider_anim.rs (look = remap(unit, look_min,
  look_max) once stopped, centred, no throttle and look ≤ 0).
- **Tick 15: 4 draws.** The engine note for ai0 and ai2 as they launch: one sound_pick
  (`./kit fn 0x001b5820`) and one accel cut-in (`./kit fn 0x003104f0`, return 0x00310618) each.
  Later ticks also see power picks (tick 48) and more accel ones (30, 61, 62, 63).
- **From tick 17: two draws per particle.** These come from the dust emitters hung off each rider's
  +0x1490 object: emit (`./kit fn 0x00150540`, returns 0x0015070c and 0x00150744), and from
  tick 20 also emit_other (`./kit fn 0x001512b0`). That is 4 at 17, rising to 24–26 a tick by 22,
  and 10 a tick by 55.
- **The AI's own:** ai_habits' roll at tick 34 (3 draws) and ai_feed's bump roll at 51 (1 draw).
  Both are already ported in `crate::ai`.
- **From tick 17 the capture steps 1–3 more a tick** (mostly 2) than the whole sim half draws. These
  are not in the sim half. The render is the only part of the frame not run, so they are probably
  there (S328d).

## How it was proved

- A save-state capture. C33's image is run whole in the interpreter for 60 ticks, against
  `free09-ai4-agreed.csv`. The test asserts:
  - all four bodies are bit-equal on every row, ticks 4–63;
  - the generator is bit-equal through tick 16;
  - the draw counts by site at ticks 7, 8 and 15, and no draws at all on the other ticks before 17;
  - dust on every tick from 17;
  - no unnamed site;
  - the capture never steps less than the frame draws.
- `./kit check -- -p game-sim --test ai -- s328` passes.

## Not verified / not 1:1

- **Nothing is drawn in the Sim yet.** The Sim's generator still parts at tick 7 and
  C33_AGREES_THROUGH is unchanged. The four systems are S328a–d.
- **The render's 1–3 draws a tick from 17 are not identified.** The interpreter has no VU0 clip flags
  or VI registers, so the render (app vt+0x38) stops at its first vclip. That they are in the render
  is inferred: the sim half was run in full and doesn't make them. Filed as S328d.
- **The generator is only compared through 16.** From 17 the missing render draws change every
  later draw's value. The counts per site are still checked, but particle counts don't depend on
  draw values, so a wrong value could hide behind a right count.
- **Only ticks 3–63 are covered,** not the 1800 recorded.
- **The neutral pad is inferred from the result.** No capture of the pad was read. A fresh neutral
  analog report makes all four bodies match through 62, and the stale one doesn't.

## Next

S328a first (the look timers at ticks 7 and 8 are the first parting), then S328b. S328c and S328d
both matter from tick 17.
