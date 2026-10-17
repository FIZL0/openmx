# 1 — shell tapes (FINAL)

Task P7. 2026-10-17.

## What I did

The shell can now write its pad down and ride it back, on I3's `PadTape`, so a control complaint is a
file instead of a story.

**`crates/game-sim/src/replay.rs`** — two small types at the end of the module, both additions:

- `Recorder` — a tape being written by a session that is being played right now. It exists rather than
  a bare `PadTape::push` so the two rules a recording has to keep live in one place, where a test can
  drive them without a window: *one entry per simulation pass* (a frame the engine did not simulate has
  no input to replay, and a tape with one in it plays a tick longer than the session it recorded), and
  *the `dt` recorded is the `dt` the simulation was handed* — not the measured wall-clock delta, which
  the engine averages over thirty frames and clamps at 30 fps (`tick::DeltaTime`) before anything sees
  it. A tape of the deltas would replay as a different run.
- `Head` — the playback head: where a live loop has got to in the tape it is riding. `Playback` plays a
  whole tape in one call, which a shell with a window and a camera cannot do, so it holds a head and
  pulls one frame a loop. It is an `Iterator<Item = PadFrame>` with `at()`, `len()` and `done()` on the
  side.

**`crates/openmx/src/main.rs`** — `--record FILE`, `--play FILE`, and `--phase first|second`:

- recording happens *inside* `World::simulate`, where the frame's `dt` and the reports it is about to
  read both are, so what is written is exactly what the pass simulated;
- playing replaces two things and only two: the frame's reports come off the tape instead of the pad,
  and the frame's length comes off the tape instead of the engine's clock (`World::played_dt`, read by
  the simulation pass and by the body pose, the chassis pose and the camera, so the whole frame steps
  with the tape's number). When the tape runs out the session ends — there is no pad behind it.
- `--play` reads a `.p2m2` as the tape it is (I4's reader), which makes
  `--play a.p2m2 --phase second --record a.tape` the converter from a human's minute on the original
  into a tape of this port's own kind. That is the "same kind of file" claim made usable: nothing else
  in the project writes a tape of the original to a file.
- a taped session prints `ride hash <64 bits>` at the end (`hash::of(&rider.ride)`), because a played
  tape is a function of its file and a number is how you see that it is. A live ride does not print one:
  it is paced by the wall clock and the number would be different every time.
- a frame the engine did not simulate while a tape is being ridden is an error rather than a silent
  hole in the ride. Nothing in this shell sets the engine's frozen flag, so it should not happen.

`PLAN.md`'s code map says both of these in a line each.

## What the original does

Nothing: this is the port's own instrument, like the rest of `replay.rs`. The original has no input
recording, so there is no file format and no behaviour to be 1:1 with.

The one thing here that *is* the original's, and the reason the tape carries a `dt` per frame at all,
is the frame-time pipeline (`tick::DeltaTime`, S1): the simulation is handed the mean of the last
thirty measured frames, clamped to 0.03333 while the 30 fps cap is on, and the millisecond timers in
the input layer accumulate `dt * 1000`. So a replay at a fixed step reproduces the buttons and not the
run, and a tape has to carry the number the pass actually used.

## How it was proved

**Tests** (`crates/game-sim/tests/replay.rs`, four new, `./kit check`: TESTS PASSED, 490 tests):

- `a_tape_the_shell_wrote_reads_back_frame_for_frame` — a 300-frame session built the way the shell
  builds one (the engine's zero-length first frame, averaged frames, the odd long one; the stick
  wandering, the throttle picked up a third of the way in at a strength a button mask cannot hold) is
  written and read back: every report byte, every bit of every `dt`, the name, and the same text on a
  second write. Checked against the three things a weaker round trip would lose.
- `a_playback_head_hands_out_every_frame_once` — in order, all of them, exactly once, `done()` where it
  should be, and an empty tape over before it starts rather than hanging a loop.
- `a_played_tape_is_the_same_run_every_time` — 240 frames played twice, compared tick by tick (not just
  at the end, so a pair that diverged in the middle and came back could not pass), the same through the
  file, the same as `replay::play`'s whole-tape call, and not vacuous: >100 distinct hashes over the
  run, and one changed frame changes the result.
- `a_tape_ridden_and_recorded_at_once_is_the_tape_that_was_ridden` — the shell's `--play t --record t2`
  loop with the window and the clock taken out: `t2` is `t`, frame for frame and byte for byte. This is
  the test that fails if a playback ever takes its frame length from the wall clock again.

**Live, in the shell** (`free06` / `125sx` / `thq1`, 320x224, this machine's X display):

| what | result |
|---|---|
| 5 s live session, `--record p7-a.tape` | 300 frames written |
| `--play p7-a.tape --record p7-b.tape` | `p7-b` byte-identical to `p7-a` (`cmp`) |
| the same again into `p7-c.tape` | byte-identical again |
| both playbacks' final state | `ride hash 0xbcd4c55c185d5f61`, twice |
| 6 s live session, `--record p7-live.tape` (360 frames) | `ride hash 0x0d8874013edc4c89` |
| `--play p7-live.tape` | `ride hash 0x0d8874013edc4c89` — **the playback is the session it recorded** |
| `--play context/recordings/first_recording.p2m2 --phase second --seconds 4 --record human.tape` | the human's pad rides the shell; 240 frames written; `ride hash 0xcb555b6886f9f298` |
| `--play human.tape` | `ride hash 0xcb555b6886f9f298` — the converted tape is the recording |

The tape files are under `context/tapes/`, which is git-ignored, because a tape holds values read out
of the game.

## Not verified / not 1:1

- **Nothing here is 1:1 with anything.** The tape, the recorder and the head are the port's own
  instruments; the original has no input recording to match.
- **A tape does not say what it was ridden on.** Its `tape` line carries "rider on track / bike" as free
  text and nothing checks it, so riding a tape against another bike, track or build replays the *pad*
  and not the ride, and says nothing about it. The state hash would differ and the shell would not say
  why. A tape that named its world and a shell that refused a mismatch is a task of its own.
- **`--phase` for a `.p2m2` is the caller's to get right.** Which of a tick's two video fields the
  game's read landed on is not in the file; `second` is measured for `first_recording.p2m2` only (I4),
  and the shell's default is the type's (`first`), which is wrong for that one file.
- **The picture was not compared between two playbacks.** The camera and both poses now step with the
  tape's `dt`, so the frames should be identical too, but the only thing compared was the simulation's
  state hash — `--shot` writes the first frame, and two first frames agreeing proves nothing.
- **The hash printed is the ride's, not the whole world's.** `Ride` implements the hash's state walk
  and `Rider` does not, so the trick machine's clocks and the animation state are outside the number.
  Two playbacks could in principle agree on the bike and differ on the rider; the tests' `Playback`
  hash (input layer + `Sim`) does not cover the rider either.
- **The skipped-frame guard was never exercised**, because nothing in the shell sets the engine's
  frozen flag. It is there so that a future frame that does not simulate stops the ride instead of
  quietly dropping a frame of the tape.
- **Live recording of a human's hands was not done here** — this session is unattended, so the live
  tapes above were recorded with nobody touching the pad (the pad bytes are idle and the bike
  free-rolls). Real presses were exercised two other ways: synthetically in the tests, and for real
  through the human's `.p2m2`, whose presses rode the shell and round-tripped into a tape.
- **Playback is still paced at 60 Hz by the wall clock.** The simulation no longer reads that clock, so
  a machine too slow to keep up plays the same run with a laggy picture rather than a different run —
  but that was not tested on a machine too slow to keep up.

## Next

Nothing outstanding for P7. The two things worth a task if someone wants them: a tape that names the
track, bike and build it was ridden on so a mismatched ride is refused rather than silently different,
and a `./kit` verb that turns a `./kit record` capture of the original into a `.tape` the way `--play`
now turns a `.p2m2` into one.
