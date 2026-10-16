# 3 — s8 acceptance (PART)

Task S8. 2026-10-16. Continues `2-s8-acceptance-READY.md`.

**The human's recording has now driven the original, and the capture of it is in.** Entry 2 was READY
because PCSX2 will only start an input-recording playback from its GUI and this machine gave an agent no
way to reach a GUI. That was half right: no *keystroke* reaches the window, and no window manager call
would move focus to it. But a Qt application exposes its whole menu tree on the desktop's accessibility
bus, and a menu item can be pressed there without focus, without a pointer and without a key. So the
ten-minute human item in `plan/HUMAN.md` is done, by the agent, and the acceptance capture is the human's
own minute of freeriding as the original played it.

## What I did

**Drove the GUI through AT-SPI.** PCSX2 copy 7 started with `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1` and
`QT_ACCESSIBILITY=1` in its environment (the kit's `spawn_detached` passes a given environment through, so
this needed no change to the kit). Then, from a separate process over the bus: walk `Atspi.get_desktop(0)`
to the emulator's menu bar, `do_action(0)` — "Press" — on **Tools → Input Recording → Play**, wait for the
modal *Select a File* dialog to appear in the tree, `Atspi.EditableText.set_text_contents` the recording's
path into its file field, and press **Open**. The press on the menu item returns an `atspi_error: Did not
receive a reply`, which is correct and not a failure: the modal dialog it opened is blocking Qt's event
loop, so the reply never comes. A fresh connection finds the dialog and carries on.

**Caught the state load on the tick it happens.** A `.p2m2` that starts from a save state loads that state
when playback begins, which sends the game's own `frame_counter` (app+0x374) backwards. A poller watched
that word through PINE and paused the VM on the backwards jump; it landed on **2773**, which is
`RECORDING_START_TICK` exactly — the counter in the recording's own `_SaveState.p2s`. `./kit record
watch/acceptance.toml` then ran from there with the warmup set to 0, so the capture's first row is the
recording's frame 0 and the alignment offset the comparison below needs is zero.

    1811 frames captured in 81.2s (22.3/s), frames 2774..4584
      skipped 0 torn read(s), 0 counter gap(s) (0 frames the emulator ran past us)
      ended: frame budget reached (1811)

**What is in it.** 1,811 rows, 54 columns, one per tick with no gap: 1,449 ticks with a control held
(`ctl2` 1,303, `ctl1` 119, `ctl11` 45, `ctl3` 27, `ctl5` 5, `ctl4` 4), speed 0.18 → **77.08** units/s
(mean 41.87), 3,071.1 units of path against 1,352.9 of straight line, height 122.2 → 267.6, `dt` a single
value (0.033330000936985016 — `replay::CAPPED_DT`, so the clamp holds on a *driven* minute as well as an
idle one), and zero rows that violate any of the fixture reader's guards. This is the capture entry 2 said
the data did not contain: a bike that is ridden, jumped, landed and crashed, by a person.

**Settled the tick phase — I4's open question, and the one thing only a driven capture can answer.** A
recording carries two video fields per tick and the game reads one of them; which one depends on where in
PCSX2's frame the save state was taken and the file does not say. Before the human's first press every
field is the same neutral pad, so an *idle* capture agrees with both phases — entry 2's measurement, and
why it could not choose. Driven, only one survives: `TickPhase::Second` matches libpad's buffer in EE RAM
on **all 1,810 overlapping ticks, byte for byte**, and `TickPhase::First` matches the neutral prefix and
then sits one field behind for ever (its tick 28 is what RAM already held on tick 27). **The game reads the
second field of each tick**; field 56 of this recording — the human's first press — is the first input it
ever saw. Both halves are now asserted, and the test that was
`the_recording_and_the_capture_hold_the_same_pad_where_both_were_idle` is
`the_capture_is_the_recording_played_back_and_the_game_reads_the_second_field`. The measurement is written
at `TickPhase` itself, as a fact about this file and its state rather than about the game, which is what it
is: I4 counted 480 of 1,810 ticks whose two fields differ at all, 18 of them in the button word, so a port
replaying this recording on the wrong phase would be wrong on 480 ticks and right on the other 1,330.

**Two measurements that were other tasks' open questions.** The capture carries the rider actor as well as
the bike (`0eabc7c`), so the same minute answers them:

- **S14's window.** `wreck_reason` (+0x748) is 0 while riding and non-zero in two windows: ticks
  **3259–3349** and **4167–4190**, both reason **5**. The tick-exact acceptance window therefore runs
  2774 → 3258 — **485 ticks**, a little over sixteen seconds — against the ~510 S14 inferred without the
  data. Past 3259 a replay needs whatever reason 5 is, and `Bike\Wreckbike.phy` with it.
- **S22's headroom.** The largest sideways `g_peak` (+0x550) on a real ridden minute is **6.12**, at tick
  3536, against the `SideWreckSpeedLimit` of 16.87 the parameter set carries. The original never rings that
  bell on this track, which is what S22 argued from the code and could not measure.

**The verdict.** `the_acceptance_run` now reads the capture on every `./kit check`:

    1811 rows x 54 columns, frames 2774..4584
    acceptance-freeride (1810 ticks of real play, rng 0xb646bb20d7f1ddd0, dt 0.03333..0.03333):
      1810 frames, 14488 checks, seed 0xd7f1ddd0
    the port has no 'bike.pos[0]': the acceptance capture is good and there is nothing in the port to
      compare it against yet (S3 — the ported movement — is open). tick 0: the state has no field
      'bike.pos[0]' (the fixture is older than the walk)

Which is the honest answer and the same one as entry 2's, with one difference that is the whole of this
entry: the 14,488 checks are now checks against a minute somebody *rode*. `./kit check`: 449 tests, green.

## The recording was damaged, and is restored

**PCSX2 does not stop at the end of a movie: it switches to record mode and keeps writing, into the same
file.** Over the minutes the emulator stayed open after the capture, the human's recording grew from
130,962 bytes (3,621 fields) to over 1.2 MB, and its header's frame count with it. Nothing in the kit did
this and nothing warned; what caught it was a test —
`assert_eq!(movie.ticks(TickPhase::First), RECORDING_TICKS)` went red with `left: 14836`.

It is **restored byte for byte**: four worktrees hold copies made before today (`openmx-slots/s{1,3,4,5}`),
all four identical at md5 `59900dc955641ee3ef9fcf50211b5d22`, and the main checkout's file is now that.
The save state beside it (`f3b5481c46a8c78e2504c6f738141ef2`) was never touched — the damaged file's prefix
did not even match, so the append rewrote the header too and a truncation would not have been enough. The
grown file is kept in this session's scratchpad, in case anyone wants to see what PCSX2 wrote.

Two things follow, and both are written down rather than coded: the main checkout's copy is now
**read-only** (`chmod 444`), and `plan/HUMAN.md` and the memory note both say a playback must be pointed at
a *copy*. The irreplaceable file here is the human's ten minutes at the desktop; the capture can be remade
from it in ninety seconds, now, by an agent.

## Not verified / not 1:1

- **Nobody watched the playback.** I never saw a frame of it: `grim` grabs the lock screen on this desktop
  and the emulator was never focused. The evidence that the tape really drove the game is the comparison
  itself — 1,810 ticks of libpad's buffer equal to the file's bytes, a bike that reaches 77 units/s, and
  two crashes where an idle bike has none — which is stronger than a screenshot, but it is inference from
  RAM and not a picture.
- **A minute is not "several minutes".** S8's own wording asks for several minutes of play and the
  recording is 60.4 seconds. The acceptance run is as long as the tape; the port's hundredth awkward
  landing is still not in any data. Making a longer one is a human ask (recording is still a person at a
  pad) — but only the *recording* is, now, not the playback.
- **Reason 5 is a number, not a cause.** `wreck_reason` is non-zero in two windows and the value is 5 in
  both; what 5 *is*, and whether the first window is the NPC contact S14 was looking for or the human
  simply crashing, is not proved here. S14's conclusion that a contact happens is unchanged; its timing is
  now measured.
- **No physics was compared, again.** S3 (movement) and D7 (the 125 parameter set) are both open, so the
  verdict is still `NoSuchField`. Nothing in this entry says the port's arithmetic matches the original's.
- **The eight hops are still true of one save state.** Unchanged from entry 2, and the rider chain added
  in `0eabc7c` is two more hops of the same kind.
- **The capture's RNG state is still truncated** to 32 bits of the library's 64-bit word
  (`0xb646bb20d7f1ddd0` → `0xd7f1ddd0`). Unchanged, and it will matter when the port draws a number.
- **The phase is measured for this recording only.** It is a property of where the save state sits in
  PCSX2's frame, so another recording must be measured the same way — which now costs a playback, not a
  human.
- **The AT-SPI path is not in the kit.** It ran from scripts in the session scratchpad, not from `./kit`,
  so the next agent to need a playback has this entry and the memory note and no command. Worth a
  `./kit pcsx2 play <recording>` the day a second recording exists.
- **Not run on a second architecture.** The capture is data and the comparison is byte moves; A3's
  conclusion should carry, but this ran on x86-64 only.

## Next

1. **S3, then D7.** The capture is the port's verdict already; the day a bike exists it starts comparing
   numbers, with no code to write here.
2. **The window ends at 3259** until something ports reason 5 and `Wreckbike.phy`. A replay that runs past
   it is comparing against a crash the port cannot have.
3. **A longer recording** is the only part of S8 still waiting on a person — and `./kit pcsx2 play` would
   be the thing to write before asking for one, so the next tape captures itself.
