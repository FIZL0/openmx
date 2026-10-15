# 1 — i3 input fixtures (FINAL)

Task I3. 2026-10-15.

## What I did

Gave the port a way to record a played sequence and play it back: the **pad tape**, in
`crates/game-sim/src/replay.rs` beside A2's fixtures.

A tape is one `PadFrame` per frame, and a frame is *what the hardware delivered*, not what the game made
of it: both pads' 32-byte reports plus the `dt` the engine measured that frame (kept as a bit pattern so
a tape is `Eq` and round-trips through its file exactly). Everything else is derived by the port's own
poll — `PadFrame::held_mask` runs I1's `Pad::poll` on a throwaway pad, so there is one implementation of
"is this control down" in the project and the tape cannot disagree with the input layer.

That choice is the task's one real decision, and it is what makes the fixture a recording rather than a
summary:

- twelve of the sixteen controls are read **only** from their pressure byte (I1), so a tape of button
  masks cannot reproduce a control's strength — and worse, a *capture* of a mask cannot be turned back
  into the frame the game read;
- the input layer is stateful (two ms timers a control, the one-frame event queue), so frames have to be
  played *through* it in the original's order;
- `dt` is measured per frame by this engine (0.03283332 / 0.03286665 / 0.03333 in the front end,
  0.041532 in a level), and the timers accumulate `dt * 1000`, so a tape without it replays different
  timers.

Around that:

- `PadTape` — name plus frames, with `inputs()` (the lossy reduction A2's `Fixture` eats), `dt_range()`,
  and a text file format (`format` / `tape` / `frame <dt> <pad0> [pad1]`, a pad written as 8..32 hex
  bytes, `*0x0040` for a bitmask, `-` for no pad at all). Parse errors name the line, like a fixture's.
- `PadTape::from_capture` — a `./kit record` CSV becomes the frames the game read. This is where the
  three alignment facts C5/S8/I1 measured live: the half of libpad's double buffer with the **smaller
  stamp** is the one the poll saw; the game ticks once per **two** video fields, so density is judged by
  the game's own frame counter and not the recorder's `frame`; and row *k*'s input is the input of the
  tick that produced row *k*, so row 0 is dropped and a tape is one frame shorter than its capture.
- `Playback` — the input layer and the simulation together, advanced one `PadFrame` at a time in the
  original's order (pad read, *then* simulate), with a `hash()` over both halves.
- `Fixture::from_tape` — a tape's inputs as a fixture, so a recorded session can carry the port's own
  expectations and be replayed by A2's `verify`.

Three smaller things the work turned up, all fixed in place:

1. **Two mask spaces had been committed as one.** S8's acceptance reader built `Input::buttons` as a
   *control-index* mask while I1's input layer (and `Control::mask()`) uses the game's own wire bitmask
   — the two differ on every control (control 6 `L2` is bit 0x0001, control 14 `down` is bit 0x4000), so
   the first acceptance fixture replayed through the input layer would have pressed the wrong buttons and
   looked like a simulation bug. The library tape is now canonical and S8's reader delegates to it.
2. S8's synthetic capture described frames with analogue controls held and all twelve pressures zero,
   which is a frame the game itself would have read as *nothing* held; it now writes the pressures.
3. I1's test picked up every CSV in `context/capture/` that has a `ctl0.held` column and then demanded
   the six-word control columns, so an acceptance capture (which watches `held` alone) failed it. It now
   requires every column it reads. This was the red `./kit check` the session started on.

## What the original does

Nothing here is a ported function: a tape is the port's own instrument, like the fixture and the state
hash. What it has to be faithful to is the *shape of a frame of input* on this machine, and that is all
I1 and C5's measurement, restated in `from_capture`'s behaviour and doc comment: libpad keeps two 0x80
byte halves with a stamp each and `scePadRead` copies the older one; the engine polls once a frame
before the simulation runs; the frame's `dt` is a measured 30-frame mean clamped by the 30 fps cap; the
event queue is cleared at the top of every update, so a non-zero count *is* a transition frame.

Recipes, research only: `./kit fn 0x0016ab10` (the poll), `./kit dis 0x0016aec0 --len 0x128` (the timer
update and the event append), `./kit fn 0x0011dab0` (the dispatcher) — all read and written up by I1.

## How it was proved

**1. Three recordings of the real game, played back through the port — 817 frames, zero mismatches.**
`a_recording_of_the_real_game_plays_back_through_the_port` turns every capture in `context/capture/` that
carries the pad columns into a tape, plays it through `Playback`, and compares the sixteen controls the
*game* recorded for that frame against the sixteen the port's input layer makes of the same pad bytes,
row for row — plus the event count where the capture watched it. The capture's control columns and its
pad buffer are written by different code at different times, so this is a comparison and not a
restatement of the decode:

```
context/capture/i1-input.csv:         219 frames, 10 transitions, controls 0x500f, dt 0.032833323..0.03333
context/capture/s8-input-buttons.csv: 299 frames, 12 transitions, controls 0x009f, dt 0.032833323..0.03333
context/capture/s8-input-chain.csv:   299 frames, 14 transitions, controls 0x5000, dt 0.032833323..0.03333
```

I1's capture is the one with `events` and all six words a control, recorded with `watch/input.toml` while
`watch/pads/two-at-once.pad` moved two controls in one frame; the two S8 captures are 300-row recordings
made earlier in this session with `watch/acceptance.toml` and `watch/pads/all-buttons.pad`. The test
refuses a recording where fewer than four frames change a control, so a tape of a pad nobody touched
cannot pass it quietly, and it says what it found and passes when `context/capture/` is empty (captures
are game data and are never committed).

**2. The proof has teeth, checked by mutation.** Two one-character changes, each the kind that looks
right on almost every frame:

| change | result |
|---|---|
| read libpad's *newer* half (`<=` → `>` on the stamps) | 4 tests red, incl. the real captures |
| keep row 0 (`1..rows.len()` → `0..`) | 8 tests red, incl. the real captures |

Both were reverted; `./kit check` is green with the code as committed.

**3. The table in the test file is the map the port measured.** `CONTROL_MASK` / `CONTROL_PRESSURE` are
written out in `tests/replay.rs` so the synthetic captures are not built from the table they are checked
against; one test now asserts the two agree, including that the sixteen bits are a permutation of the
sixteen controls (which is why `0xffff` maps to `0xffff` and only single-control cases catch a mix-up).

**4. The rest of the properties, in `cargo test -p game-sim --test replay` (45 tests, all green):** a
frame's controls are its *pressures* and not its bits (bit set with pressure zero reads as not held),
the stick's centre byte and its one unrepresentable value (0xff → clamped to 127), a tape round-trips as
a file bit for bit (reports, dt patterns and a pressure of 0x1a that no mask can hold), nine thousand
frames round-trip and play (the acceptance run's scale), every parse error names its line, a capture
with a tick missing is refused by either counter with the row in the message, a playback latches a press
before the tick that acts on it and clears the event queue a frame later, and a `Playback::hash()`
distinguishes two runs whose *simulations* are identical and whose pads are not — which a fixture's
hashes cannot do.

`./kit check`: **201 tests passed** (190 before, and red before the `tests/input.rs` fix above).

## Not verified / not 1:1

- **No human-played recording exists on this machine**, so "a human-played sequence from the real game"
  is proved with kit-driven pad scripts (`two-at-once.pad`, `all-buttons.pad`) rather than a human
  playing. The tape cannot tell the difference — it is the pad bytes either way — but nothing here has
  replayed a *person's* session, and a human's capture will exercise analogue strengths between 0x1a and
  0xff, which no pad script in the repo produces.
- **A fixture still replays buttons and the left stick only.** `Sim::tick` takes an `Input` and no dt,
  so `Fixture` cannot carry the frame's `dt`, the twelve strengths or the second pad; `Playback` carries
  all of them and is the type an acceptance run should use. Until the simulation takes a dt, a fixture
  replayed at 9,000 frames is replaying the *cap* (0.03333) and not the run.
- **Pad 1 is always `Report::missing()` in a tape read from a capture**, because no watch spec in the
  repo records the second pad's buffer. Two-player input is untested against the original.
- **Report bytes 20..31 are zero in a tape read from a capture.** The specs record `bytes:20`, which is
  every byte the poll reads; the remaining twelve are the second analogue stick and the pressures no
  control maps to, so this is lossless for the port as it stands and wrong the day something reads them.
- **`watch/input.toml` cannot be re-recorded on this branch.** It resolves `@app`, `@input_events` and
  `@input_event_count`, and `kit.toml` has none of the three — I1 added them in its own worktree and did
  not commit them, so the recipe in that spec's header fails with "no [symbols] entry named 'app'". Its
  capture is still real evidence (I copied `i1-input.csv` out of I1's worktree, where the runner left
  it), but I could not record a fresh one, and `app = 0x479990` is the only one of the three I could
  derive here (`frame_counter` - 0x374, `frame_dt` - 0x344 agree on it) — unverified against RAM,
  because this worktree has no Ghidra dump and I was not going to spend the task's time on one.
- **`select`'s wire bit (0x0100) has never been seen held in a capture**, by I1 or here: no pad script
  presses it. It comes from the control map read out of RAM, like the other fifteen.
- **The dt a tape carries is the original's, and the port has nowhere to put it.** Both ends are
  recorded (`dt_range` prints the spread, and the capped 0.03333 is a named constant rather than a
  literal) so that the day the simulation takes a dt, nothing has to be re-measured.

## Next

Nothing is pending for I3. For whoever takes the acceptance run (S8's `the_acceptance_run`, still
`#[ignore]`d): the input half is now a one-liner — `PadTape::from_capture` the recording, then
`Playback` it — and what is missing is a capture with the bike's state in it, plus a `Sim` that reads
its input. If `kit.toml` gains `app`, `input_events` and `input_event_count`, `watch/input.toml` records
the richest input capture in the project (every word of all sixteen controls, both event slots, and the
measured dt) and this test will compare the event counts of it too, for free.
