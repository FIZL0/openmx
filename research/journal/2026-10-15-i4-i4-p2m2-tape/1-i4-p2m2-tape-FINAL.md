# 1 — i4 p2m2 tape (FINAL)

Task I4. 2026-10-15.

The human's sixty seconds of free riding now drives the port. `context/recordings/first_recording.p2m2`
(3621 fields, two pads, PCSX2 2.9.127, `rycs`, with its `_SaveState.p2s`) reads as 1811 ticks of pad
reports, plays through I3's `Playback` end to end, and becomes a fixture — with **no capture session,
no PINE and no emulator in the loop**, which is what makes it repeatable: the file plus its save state
replay the same minute on the original whenever the port changes.

## What I did

**`crates/game-sim/src/replay.rs`** (+~400 lines, appended; nothing above was touched):

- **`P2m2::parse`** — the file. Header (`fileVersion`, `emulator[50]`, `author[255]`, `game[255]`,
  `totalFrames`, `undoCount`, `savestate` = 570 bytes), then one 36-byte frame slot per video field.
  Refuses, by the byte it is at (`P2m2Error`, the binary counterpart of `ParseError`, which carries a
  line number a file with no lines cannot have): a file too short for a header, a frame area that is
  not a whole number of frames, a slot count that is not `totalFrames + 1`, a **written frame 0**, and
  a `fileVersion` this reader has not been measured against. Four of those five would move every frame
  of the recording by a slot or half a tick — the kind of wrong that replays as a plausible session
  that diverges everywhere.
- **`report_of_p2m2`** — the eighteen recorded bytes onto I1's twenty report bytes: status 0 (a
  recorded frame is a read that happened), mode `MODE_PRESSURES` (carried, never read by the poll),
  the two word halves, the four axes, the twelve pressures. The pressure *order* is the same on both
  sides, so the last row is a copy and not a permutation. An entry of eighteen zero bytes is
  `Report::missing()`, not a pad with every button held and both sticks in a corner — which is what
  port 1 of a one-pad recording is, on every frame of it.
- **`TickPhase`** — which of a tick's two video fields the game's pad read landed on, because the file
  holds a frame per field and this game ticks once per two. `First` (the default) is fields 1, 3, 5…;
  `Second` is 2, 4, 6…. `P2m2::ticks_the_phase_moves()` counts what the choice costs: on this
  recording **480 of 1810 pairs carry different pad bytes, and 18 of those differ in the button word**.
- **`P2m2::tape` / `tape_with_dt` / `PadTape::from_p2m2`** — the tape, at `CAPPED_DT`, named
  `p2m2-rycs-3621f-first` so a fixture says which file and which phase it came from. `from_p2m2` is
  the one call S8 wants: parse, self-check, tape.
- **`P2m2::check_pressures`** — the file's own proof of the byte mapping, and the check that a
  recording can drive the port at all (see below).

**`crates/game-sim/tests/replay.rs`** (+~340 lines, appended) — nine tests, the last of which reads
the real recording out of the git-ignored `context/recordings/` and prints what it measured.

## What the original does

Nothing new was decompiled; this task is a reader. What it leans on, and the one thing it settled:

**Where a tape's `dt` comes from — settled: the 30 fps cap, on every frame of a recording of play.**
S8 left this open and flagged it as the thing nobody should write the port's tick without reading
("a measured `dt` may make a tick-exact replay impossible"). It is not open:

1. S1 found the clamp set by the game object's constructor and never cleared, and R6 read the flag as 1
   in a level and `dt = 0.0333300` on all 119 ticks of its capture.
2. The clamp bites whenever the thirty-frame mean frame time exceeds 0.03333 s, and two NTSC fields are
   33.3667 ms — so on a machine keeping up with the disc, every frame is over the cap.
3. **This recording's own save state says so**, read with `./kit state extract` (no emulator, no PINE):
   `dt` (app+0x344) = `0.033330001` exactly, `1/dt` (app+0x348) = `30.003`, the raw measured delta
   (app+0x358) = `0.034` — 34 ms, over the cap, so the clamp was *applied* rather than coincidentally
   matched — and the frame's millisecond step is `0x21` (the accumulated simulated clock at app+0x330
   is 133384 ms against 133351 the frame before, exactly the 33 the clamp forces).
4. And it had been biting all session, not just on the frame that was looked at: that accumulated
   *simulated* clock is **975 ms behind** the wall clock it was seeded from (app+0x338 = 134359 ms)
   after 2773 frames (app+0x374). Clamping 33.37 ms frames to a 33 ms step costs 0.367 ms a frame, so
   975 ms is about 96% of those frames clamped.

S8's one counter-example, "R6 saw 0.041532 in a level", is **not a `dt`**: R6's own entry has it as
`1/C` from a least-squares fit of `Δpos = vel / C` (the 1/24 s integrator step), on a capture whose
`dt` column read 0.0333300 throughout. The front end's sub-cap values (0.032833, 0.032867) are real and
are the front end's, which is why `tape_with_dt` exists and why a recording of a *menu* is not a
recording `tape()` describes.

The second thing the reader leans on is I1's measured control map: twelve of the sixteen controls are
read from a pressure byte and the poll never looks at their bit. That is what makes the file
self-proving, and also what makes a pressureless recording worthless rather than merely degraded.

## How it was proved

`./kit check`: **308 tests pass** (was 299 before this task; 9 new, none `--ignored`).

**The byte mapping, against the file's own redundancy — 43,452 agreements, 0 disagreements.** For
twelve of the sixteen controls a `.p2m2` entry carries the button's bit *and* its pressure byte, written
by PCSX2 from the same host pad at the same instant. On all 3621 fields of the recording, every one of
the twelve agrees — and the same check against either neighbouring pairing of pressure byte to bit
disagrees **5,814 times**. So the pressure order is measured here, not assumed from "the DualShock 2
order". `the_pressure_bytes_of_a_recording_prove_the_byte_mapping` pins both: a rotated recording is
refused with the frame and the control that disagreed, and so is a recording whose pressures are all
zero — which the game would read as twelve controls permanently released however good its button word.

**The recording, through the port, end to end** (`the_human_recording_in_context_drives_the_port`):
3621 fields → 1811 ticks → `Playback` → 1811 ticks of the port's own input layer, with **35
transitions** and controls `0x02ec` seen held (circle, cross, square, L1, R1, L3 — a rider on the
throttle with the odd brake and a stick click, which is what sixty seconds of free riding looks like).
The tape round-trips through its file form byte for byte and replays to the same playback hash, and the
two phases are asserted to *not* play to the same state, because 18 of the pairs differ in a button.

**The reader itself, against synthetic files built to PCSX2's layout** (eight tests): the header where
PCSX2 put it; one entry → the twenty report bytes, lane by lane, and then through the game's own poll
to the controls and the stick; the unwritten entry as no pad; the five refusals, each by its byte; the
two phases as the two field sequences they are, with a press that lands on a different tick in each;
the tape at the cap, with the input layer's `held_ms` reading a flat 33 ms a frame; and a recorded
session driving `Playback` tick by tick, where the port is asserted to hold exactly what the file's
field recorded, becoming a fixture that `verify` replays.

## Not verified / not 1:1

- **Which of a tick's two fields the game read is not measured.** `TickPhase::First` is a choice, not a
  finding: the file holds a frame per video field, the game reads once per two, and nothing in the file
  says where PCSX2's frame 0 falls inside the game's tick. The cost is counted rather than guessed —
  480 of 1810 pairs differ at all, 18 differ in the button word — so the wrong phase is a press landing
  one tick early or late about 18 times in the minute, and the stick a field stale on a quarter of the
  ticks. **How to settle it:** play the `.p2m2` back in PCSX2 from its own save state while
  `./kit record watch/input.toml` captures `@pad0_control_state`, and compare the capture against both
  tapes — the right phase agrees on all 1811 ticks, the wrong one disagrees on those 18. That needs
  PCSX2 driving its own input recording (the kit does not wrap it) and it is S8's acceptance run, not
  this reader's.
- **`dt` is proved constant for one frame of this recording and argued for the rest.** The save state
  is one frame — the frame the recording starts at — and finding 4 above (the 975 ms clock deficit) is
  an average over the session before it, not a per-frame record. A frame whose thirty-frame mean dipped
  under the cap would have been handed the mean instead (~0.0333 against 0.03333), which is 0.03 ms of
  timer drift on that frame; the millisecond timers are integers and the front end's repeat rates are
  hundreds of ms, so it would take hundreds of such frames in a row to change a repeat by one frame.
  Unproven all the same: a capture of the recording being played back (the same run as the phase
  question above) would carry the real `dt` column and settle it frame by frame.
- **The second pad is read but has never been seen.** `report_of_p2m2` maps both ports and the reader
  treats an all-zero entry as "no pad"; on the one recording, port 1 is all-zero on all 3622 slots, so
  the mapping of a *written* port-1 entry has never run against real bytes. I1 found the second pad's
  control map all zeros in a game nobody had plugged one into, so this is consistent, not proved.
- **The "unwritten frame 0" rule holds for one file and one PCSX2.** Slot 0 is all zeros and slots
  1..=3621 are written in `first_recording.p2m2` (3621 = `totalFrames`), which is what the reader
  requires. A recording made from power-on rather than from a save state may well number its frames
  from 0; this reader refuses it with a sentence naming the problem rather than silently reading it
  half a tick out of step, and relaxing that is a change to make with the file in hand.
- **`fileVersion` 1 is the only layout read.** PCSX2 2.9.127 writes it; a future version that moves a
  field is refused by the version byte rather than misread, which is the best a reader can do without
  the file.
- **The simulation side is still untouched.** `Sim` is A1's example ball, so what a recorded session
  drives is the input layer and a ball: the fixture a tape makes is a *regression* fixture of real
  input (`replay::record` over the tape's inputs), not an acceptance fixture. S8 is the other half, and
  it needs the bike (S2–S5, S7) before the recording can say whether the port is right.
- **The pressure self-check assumes a digital host pad.** It requires a pressure byte to be non-zero
  exactly when its bit is down, which is how PCSX2 recorded this pad (0 or 255, nothing between). A
  recording from a pad with real analogue pressures could legitimately report a pressure under I1's
  threshold while the bit is down, and would be refused. The error names the frame and the control, so
  the fix would be evidence-driven, but today the check is stricter than the format.
- **The recording does not survive this worktree.** `context/` is per-worktree and git-ignored; the
  file I read is the main checkout's `context/recordings/first_recording.p2m2`, copied in. The test
  that reads it passes with a message when it is absent, like the capture tests beside it, so a
  worktree without it is green and proves less — the one honest thing a test can do about a file the
  repository is not allowed to hold.
- **Not run on a second architecture.** The reader does no float arithmetic (the module denies the
  lint) and moves bytes, so A3's conclusion should carry, but these tests have only run on x86-64.

## Next

1. **S14 and D7 before any physics comparison** (the plan says so: the run is a 125 on Free09, and S14
   bounds the window where an NPC can touch it).
2. **S8 can now build its fixture's input half with `PadTape::from_p2m2`** and should settle the tick
   phase while it is there, with the recipe in the first *Not verified* item above — one capture of the
   recording playing back answers both open questions (phase and per-frame `dt`) at once.
3. Nothing in this task blocks on a human.
