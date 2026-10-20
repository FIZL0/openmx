# 1 — s32 second acceptance run (FINAL)

Task S32. 2026-10-20.

## What I did

- **The capture.** I played recording2 back on PCSX2 copy 2 the way HUMAN.md's playback item says: start
  with the Qt accessibility env, pause over AT-SPI, Tools → Input Recording → Play on a *copy* of
  `recording2.p2m2`, record, then unpause. The human's file is unchanged (md5 `1e5091236f40915ad1e5942bce20287d`).
  The playback lands on the state's frame 2373 and the capture runs over frames 2374..4267, 1,894 rows.
  My capture (`context/capture/s32/recording2-opponents.csv`) has the acceptance columns plus the type,
  control source and position of actor-list entries 0..15. Entries 5, 6 and 7 are the three AI bikes
  (src 2) and 8 is the human's (src 1).
- **Which capture of recording2 is the fixture: C6's** (s4's `recording2-freeride.csv`, 77 columns, md5
  `42810e140cef351ae7b749fd62e4681b`), copied into this slot's `context/capture/acceptance/`. The copy
  of s3's capture that had been parked there is now `context/capture/s32/recording2-freeride-s3.csv`.
  The reason is the next point.
- **Captures taken with the recorder waking on `frame_counter` have late rows.** The main loop bumps
  the frame counter and *then* calls simulate, so a recorder that wakes on the counter can read a
  bike that is still the last tick's, labelled with the new tick. `pine.settle` checks for tearing
  against that same counter, so it does not see this. Five slots captured recording2, and I compared
  them row by row (1,895 distinct bike states):
  - s3 labels 226 rows a tick late, s5 204, s2 (mine) 33, s1 7.
  - C6's capture (s4) labels none late. Its recorder woke on the pad stamp. It agrees with the
    earliest label of every state on every row, and it has no riding row whose eight bike lanes repeat
    the row before. s3's capture has 70 such rows.
- **`the_second_acceptance_run`** (crates/game-sim/tests/replay.rs, new section at the end). It builds
  the fixture from C6's capture with `acceptance_fixture_on`, so the seed and start come from row 0 by
  `acceptance_start` the way S8 did it. It cuts the fixture to the window (`through_tick`), rides it
  over the world `in_the_world_of` finds (S28: Free09, `race125p.phy`), and checks it against
  `SECOND_ACCEPTANCE_AGREES_THROUGH = 0`.
  - `acceptance_verdict` is now a thin wrapper over `acceptance_verdict_against(f, world, agreed,
    gate)`, so each run has its own ratchet. The first run's behaviour and wording are unchanged
    apart from naming the gate.
  - A capture that `repeated_bike_rows` flags is refused with a line, not ridden.
- **The window, found in the capture.** S14's rule ends it at the first wreck or contact. In C6's
  capture, `wreck_reason` first goes nonzero at **tick 641, reason 8 (nose-dive)**, so the window is
  ticks 0..=640.
  - No row of the minute reads 7 (a contact with another rider).
  - The AI bikes ride off the start line away from the human. The nearest any of them comes to the
    human's bike through tick 640 is 18.97 units, at tick 0; by tick 30 it is about 41–46, and by tick
    640 about 957–1,284.
  - `the_second_recordings_window_ends_at_its_first_wreck_which_is_not_a_contact` asserts all of this.
    The distance half needs the opponents capture and skips with a line where it is missing.
  - The minute's later wrecks: tick 1188 reason 12 (no-hander, back at 0 by 1212) and tick 1750
    reason 6 (static object, back at 0 by 1829). The nose-dive clears at 764.
- **`recorded_on` lets one row in a hundred be buried.** Under S28's strict rule recording2 fitted no
  area. At tick 342, three ticks after the hard landing of the 273..339 flight (vy −32.5), the body
  origin is 2.43 units under Free09's blended heightfield, past `BURIED` (2.0). Every other area
  buries the window's rows on 37 to 557 of 641, or never comes under the wheels (Free07, 26.9 over at
  its closest). The new `BURIED_ONE_IN = 100` lets Free09 through and none of the others. The first
  capture has no row past `BURIED`, so it is identified exactly as before.

### The verdicts (`./kit check -- -p game-sim --test replay -- --nocapture acceptance`)

```
the acceptance run of acceptance-freeride (1810 ticks …) reaches tick 1 of 1810 before it parts from the original: tick 1: bike.pos[1] 208.02884 (0x43500762) vs 208.10736 (0x43501b7c)
the acceptance run of recording2-freeride (640 ticks of real play, rng 0x884fd42fe070006e, dt 0.03333..0.03333) reaches tick 1 of 640 before it parts from the original: tick 1: bike.pos[1] 208.05757 (0x43500ebd) vs 208.13484 (0x43502285)
  bike.pos[0]: agrees through tick 20, parts at 21
  bike.pos[1], pos[2], vel[0], vel[1], vel[2]: agree through tick 0, part at 1
  bike.pos[3], vel[3]: agree on every recorded tick
```

The second minute parts the same way and by the same amount as the first: the port pushes the bike
*up* on tick 1, by 0.0773 here and 0.0785 there. S28's open question (the wheel mounts, or the
ground's blend) is therefore not peculiar to the first minute's start. The full `./kit check`
passed. The test total is shared across slots, so I am not quoting it.

### recording2's plain riding, for P15

Plain riding here is the original's own flags: a wheel on the ground (+0x574), no wreck, no trick
running, no backflip armed or fired, and `up` (m11) > 0.7. That gives 888 plain ticks of 1,894, in 21
stretches, 12 of them a second or longer. Units are the velocity column's (ft/s). The r1 column is
the same scripts run over the first capture.

| | recording2 (C6, clean) | recording2 (s3, torn) | recording 1 |
|---|---|---|---|
| top speed, flat, gas held | **79.89** at tick 587, a lower bound (below) | ≈ same | 75.78 |
| … after a full second of plain flat riding | 74.29 at tick 1347 (leaning, up 0.86, still gaining 0.4 ft/s²) | | |
| 0→30 / 0→60, flat launch | **1.53 s / 3.37 s** (tick 766, all plain, 0.6 ft down) | | 1.50 / 5.47 (58 air ticks) |
| 0→30 / 0→60, downhill launch | 1.20 / 2.77 (tick 1212, 29.9 ft down, 14 ticks not plain) | 1.17 / 2.73 | 0.93 / 2.20 (8.6 ft down) |
| turn rate, held stick, after a 6-tick lean-in | −1.23 rad/s at 20; −1.61 at 30; −2.52 at 39; −1.17 at 56; −0.65 at 57; +0.55 at 20 | | +0.94 at 10; −1.55 at 31 |
| flat-gas accel by band 0–20 / 20–40 / 40–60 / 60–90 | 22.9 / 16.8 / 16.3 / 8.2 ft/s² | same | 20.8 / 18.9 / 16.4 / 7.5 |
| coasting on the flat | 4.6 ft/s² | | 3.4 |
| full brake | **none on record** (below) | | none (8.2 ft/s² over 19 flat ticks) |

- **Top speed.** At tick 587 the bike was on the flat (y ≈ 141.7) at full throttle and still gaining
  about 6.7 ft/s². It had come off a drop 0.8 s earlier, so 79.89 is a speed the bike reached, not the
  speed it tops out at. Where the bike had a full second of plain flat riding behind it, the best is
  74.29, but the stick was pushed and the bike leaning, so that is not a clean terminal speed either.
- **Brake.** Square is held only on ticks 601..640 (1.33 s), from 66.6 to 44.4 ft/s, and the bike
  never stops. That stretch climbs a +0.41 slope and ends in the tick-641 nose-dive. Over its 12
  plain ticks the median deceleration is 47.1 ft/s², but that is braking uphill into a lip, not a
  full brake on the flat.
- **What the second ride says about P15's numbers.**
  - The acceleration bands and the downhill launch match the first minute within a few ft/s².
  - Most turn rates fall in the first minute's 1–1.5 rad/s range. The outliers (2.52 at 39 ft/s, and
    0.55 to 0.65 at the ends) are single short stretches.
  - The numbers are the bike's, not something peculiar to the first minute.
  - The flat 0→60 (3.37 s against 5.47) differs because the first minute's launch spent 58 ticks in
    the air.

## What the original does

- Main loop: `./kit fn 0x131100`. Frame phase 2, store dt, frame counter (app+0x374) += 1, and only
  *then* simulate. A read during simulate sees the new counter beside the previous tick's bike.
- The wreck test (`./kit fn 0x2522c0`, oracle.txt's wreck entry): reason 7 is a contact entry with
  impact over 6.0 (4.0 with +0x588) against an actor; reason 6 is the same against a null object.
  The test asserts that no row reads 7.

## How it was proved

- A save-state capture of the original (C6's, 1,894 rows) replayed as a bit-exact fixture over Free09
  through the window: `the_second_acceptance_run`, 640 ticks, 5,128 checks.
- The window and the contact question: `the_second_recordings_window_ends_at_its_first_wreck_which_is_not_a_contact`,
  over C6's capture and over my opponents capture (frames aligned by `frame_counter`).
- The tear finding: five captures of the same playback compared row by row with scratch scripts, not
  committed. It is written up here, and `repeated_bike_rows` is the part that ships.

## Not verified / not 1:1

- **The port parts from the original at tick 1 of the window.** The ratchet stays at 0. This is S28's
  open question about the wheel mounts versus the ground blend, not something S32 can raise.
- **Top speed on the flat is not on record.** The best plain flat speed (79.89) was still climbing. No
  stretch of recording2 holds full gas on the flat long enough to level off.
- **A full brake is not on record.** The only brake is 1.33 s uphill into a lip, and it ends in a
  wreck. The first minute has none either. P15's "full brake from top speed" needs a ride that does
  one, which means the human, or the kit's pad over the same state.
- **Rider bodies' distance to the human is not measured, only the bikes'.** The rider actors' +0x1b0
  is not a position (the values are garbage), so a rider-to-rider contact without the bikes coming
  within 19 units is not excluded by the measurement. The wreck column would show one as reason 7,
  and it never does.
- **The opponents capture was taken with the recorder waking on `frame_counter`**, so some of its
  rows may be a tick late. At an 18.97-unit nearest approach that changes nothing, but the distances
  are not tick-exact.
- **The tear detector is a lower bound.** `repeated_bike_rows` catches a late row only when the bike
  is exactly the previous row's. C6's capture is called clean because it agrees with the earliest label
  across five captures, and no single-capture test proves that.
- **The first acceptance capture may have late rows.** It was taken with the recorder waking on
  `frame_counter`, and there is only one capture of it, so it cannot be cross-checked. It has no
  repeated riding rows, which is all the detector can say. A re-capture with the pad-stamp trigger
  would settle it.
- **C6's capture is not final while C6 is open.** If C6 re-captures, this slot's copy has to be
  replaced (the md5 above identifies it). C6's index, which is not this task's job, is not written.
- **This slot's kit.toml differs from s4's** (vsync address, and an unresolved `@rand_state` here).
  That is git-ignored and was not changed here. The scratch spec used the raw address the other slots
  resolve to.
- **The seed is the rng truncated to 32 bits** (`0xe070006e` of `0x884fd42fe070006e`), as
  `acceptance_fixture_on` does for the first. The port has no opponents drawing from it in the window
  anyway.
- **recording2's tick phase is not asserted.** The first recording's `.p2m2`-against-RAM test
  (which field of a tick the game reads) has no recording2 counterpart. The inputs come from the
  capture's own pad columns, so the fixture does not depend on it.
- **`every_acceptance_capture_…` names recording2's fixture `acceptance-freeride (…)`.** That
  existing test builds every capture through `acceptance_fixture_on`, whose name prefix is the first
  run's. I left it alone because it is not this task's code. It also takes the whole minute against
  the *first* ratchet, which holds, since the run reaches tick 1.

## Next

- A recorder fix (plan task): `pine.settle` should refuse a read taken while the frame phase is 2, or
  the acceptance spec should wake on the pad stamp. Then re-capture recording 1.
- A ride with a full brake and a held top speed on the flat, if P15 still wants those two numbers.
