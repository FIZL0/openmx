# 1 — input sampler timing (FINAL)

Task F71. 2026-11-18.

## What I did

- Traced every call to SLUS-21104's pad sampler, statically, from the front end's callback list.
- Read the input object's sample ring in state 4's image, and live every tick in my PCSX2 copy 3 while a
  tape crossed the stick (research/f71: `ring.toml`, `crossing.pad`, `play.sh`).
- Replaced the port's stand-in sampler (`Ring::sample`, 150 copies a second, all stamped "now") with the
  original's frame model: `pad::Sampler` and `Ring::push` in crates/game-sim/src/handling/pad.rs.
- The test's `Pad` (crates/game-sim/tests/mxvatv_bike_ride.rs) fits the sampler's frames to the image's ring
  and samples each tape tick through it. `the_tape_gives_every_reader_word` now pins the one difference
  left exactly (see below).
- Addresses and the full trace: research/oracle.txt `# F71`.

## What the original does

- The engine keeps one list of sampler callbacks. Only the bike's input object is on it.
- A walker calls each callback with a phase number and the time: whole milliseconds (a 100 ms tick count
  plus a hardware timer, truncated), times 0.001f. Recipe: `./kit dis -p atv` at the walker in oracle.txt.
- Five calls a frame:
  - phase 1: the main frame, just before the update that runs the sim and builds the pad record;
  - phase 2: right after that update;
  - phases 3 and 4: in drawing (once per viewport; the ride has one);
  - phase 8: in the wait for the screen, once, only if the wait has lasted over 4 ms.
- A loading loop also calls it (phase = loop index). Not during a ride.
- Times: frames 1001/30 ms apart (two NTSC fields). The later four land about +1..3, +3..6, +4..7 and
  +8..11 ms after phase 1.
- The pad's new reading arrives between phase 1 and phase 2. So:
  - a record sees its own frame's phase 1 still holding the last tick's stick;
  - the five samples between two records all hold one tick's axes;
  - a stick crossing −1 → 1 between ticks is measured over phase 1 → phase 2: 1 to 2 ms, not a tick.

## How it was proved

- **Static trace** of the walker and its four callers with `./kit dis -p atv` (addresses in oracle.txt).
- **State 4's image ring**: 32 samples, the last frames at +0, +2/3, +5/6, +6/7, +10/11 ms; the six frame
  starts fit one 1001/30 ms grid (phase 19..26 thirtieths).
- **Live capture**, copy 3, sandrail Free Ride (my state 2), every tick, 72 rows
  (context/capture/f71/crossing-1.csv, not committed):
  - 82 whole frames of five samples: offsets (0,2,3,4,8) ×40, (0,1,3,4,8) ×27, (0,2,4,4,8) ×12,
    (0,1,3,3,8) ×2, (0,2,4,4,9) ×1;
  - at each of the tape's stick changes, the frame's first sample holds the old value and the next four
    the new one (rows 11, 21, 44 read in full).
- **Flip-1 rows 66..71** (C15's capture) through the new model, in `the_tape_gives_every_reader_word`:
  - every reader word on every other row of ride-2 and flip-1 is the original's, bit for bit;
  - on rows 66..71 the port's hold is exactly 2 / (t(m+2) − t(m)) with m = the grid's phase 1 of that frame
    (221964 ms: 1000.55, 0x447a232d), and the row's is exactly the same with m + 1 (221965 ms: 992.97,
    0x44783e10). The test asserts both values.
  - The hold lasts 6 rows in both: the pair stays in the 32 samples for 30 more pushes.
  - Before F71 the port gave 60.006 there.

## Not verified / not 1:1

- **The crossing frame's phase 1 is one millisecond early in the port** (1000.55 against 992.97 on flip-1
  rows 66..71). The original's milliseconds jitter by about one with how long its work took that frame;
  the port only has the grid. Both are past the bike's clamp at 60, so the bike is the same. A bike whose
  clamp is over about 990, or a ride whose sampler pair lands on a jittered frame, would differ.
- **The later four samples use fixed offsets** (the image's newest frame: +2, +5, +7, +11 ms). Live they
  vary by a millisecond or so a frame. Only a crossing pair's gap matters, and that is phase 1 → phase 2.
- **Phase 8 is always pushed.** The original skips it when the wait for the screen is 4 ms or less. Live,
  it was there in every whole frame (82 of 82), but a heavy frame could drop it.
- **The live trace was on the sandrail state**, not C15's state 4 (its .p2s is lost; another slot's state 4
  is a menu). The callers and phases are code, so the same; the offsets may differ per scene.
- **The loading loop's calls are not modelled.** They do not run in a ride.
- **MX Unleashed's `ride::Flick` stand-in is unchanged.** F71 is about SLUS-21104; MXU's sampler was not
  traced.
- **Only the test feeds the new sampler.** No shipped ride uses `pad::Sampler` yet (nothing ships the MX
  vs. ATV pad reader).

## Next

None. F71 is done.
