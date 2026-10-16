# 2 — s8 acceptance (READY)

Task S8. 2026-10-15. Continues `1-s8-acceptance-READY.md`.

The acceptance run now has a run in it. Two captures of the **original** exist, both from the save state
the human's recording is anchored to, and both are read end to end by the harness: context/capture/acceptance-freeride.csv
(1,811 ticks — the recording's own minute, the bike idling, no pad input) and context/capture/acceptance-ride.csv (900 ticks of a
scripted ride through the kit's virtual pad, 393 ticks with a control held, the bike up to 51 units/s and
555 units from where it started). `the_acceptance_run` is no longer `--ignored`: `./kit check` builds a
fixture from the real capture, replays it against the port, and prints the verdict — which today is
`NoSuchField bike.pos[0]`, because S3 has not landed a bike. Three things that would each have made the
first real comparison silently wrong were found by having the data rather than by reasoning about it: the
watch spec was aimed at an **AI rider's** bike, the per-frame `dt` I4 could only argue for is the 30 fps
clamp on all 2,711 captured ticks, and PCSX2 will not play a `.p2m2` back without a person at its GUI — so
the human's own input has still never driven the original. That last one is the new item in
`plan/HUMAN.md`, and it is why this file is READY.

## What I did

**The capture nobody had.** `1-s8-acceptance-READY.md` shipped the spec and the reader and said plainly
that the check "has never run against a filled cell". It has now:

1. Copied `context/recordings/first_recording.p2m2` and its `context/recordings/first_recording.p2m2_SaveState.p2s` into this worktree and read the save state with
   `./kit state ram 8` + a walk of the actor list: 9 actors, **four** type-4 vehicles, and the human's is
   the **eighth** list entry (its player record's control source is 1; the other three read 2).
2. Rewrote the bike chain in `watch/acceptance.toml` from five `-> 0x8` hops to **eight**, and added a
   `bike_ctl_src` column (`+0x20 -> +0x34`) so the capture carries its own proof of whose bike it is.
3. Recorded the two captures above, each with its `.json` provenance sidecar.

**`watch/acceptance.toml`** — the eight hops, the new column, both recipes (the recording's own start
state in slot 8; and the pad-driven ride, which needs no human), and a checklist that now says
`bike_type == 4` **and** `bike_ctl_src == 1`, and that `dt` is the clamp `0.033330001` rather than 1/30.

**`crates/game-sim/tests/replay.rs`** (all additive except three edits inside existing tests):

- `acceptance_fixture` refuses any row whose `bike_ctl_src` is not 1, naming the hop count as the fix —
  the refusal that would have caught my own first capture.
- `a_capture_that_is_not_of_the_bike_is_refused` grew a fifth case: the list entry next door, a type-4
  vehicle with a player record of its own, which is an **AI** rider.
- A new S8 section at the end of the file with the two new tests below, `acceptance_verdict` (the three
  outcomes of a real replay, only one of which is a bug in the port) and `acceptance_recipe` (how to
  remake the capture, printed by every test that cannot find one).
- `the_acceptance_run`: not `--ignored` any more, recipe and verdict replaced by those two helpers.
- `a_recording_of_the_real_game_plays_back_through_the_port` (I3's) counted a capture of a pad at rest as
  a failure (`transitions >= 4`). It now counts captures of play and captures at rest apart and prints
  both totals: which captures are on a disk is not something a test gets to assert, and the idle one does
  prove the port's idle decode against the game's own controls, row for row.

**`plan/HUMAN.md`** — the one ask this task ends on, with the full recipe: play `context/recordings/first_recording.p2m2` back in PCSX2
(Tools → Input Recording → Play) from its own save state while `./kit record watch/acceptance.toml`
captures, and leave the pad alone.

## What the original does

Nothing was decompiled; everything here is measured off the running game and its save state.

**A freeride area holds four ridden vehicles, and three of them are nobody's.** Walking the actor list in
the recording's own save state (`@game_manager -> +0x34 -> +0x34` head, then `+0x8` per node, `+0x0` for
the item) gives nine actors: four of type 4 (vehicle), each with a non-null player record at `+0x20`.
Exactly one of those records has control source 1 (`+0x34`; a local human) — the eighth list entry — and
the other three read 2 (an AI rider). The fifth entry, which the inherited five-hop chain resolved to, is
an AI's: in the sixty ticks I first captured it flew past at 27 units/s while the human's bike idled. So
`bike_type == 4` and a non-null `bike_player`, the two checks the spec shipped with, are both true of a
bike belonging to somebody else. This is also the first hard count for **S14**: three AI riders are on the
area with the human, which is what bounds the tick-exact window.

**`dt` is the 30 fps clamp, on every tick of a level.** I4 settled this from one frame of the save state
plus the session's 975 ms clock deficit, and left per-frame proof to S8. The `dt` column
(app+0x344) reads `0.033330000936985016` — bit for bit `CAPPED_DT`, the engine's own clamp — on **all
1,811** ticks of the idle capture and **all 900** of the ride, including the ticks where the bike was
airborne at 51 units/s. Nothing in a level dips under the cap.

**One row per tick, and the tick is half the video rate.** Over both captures the recorder's `frame`
(vsync) column rises by exactly 2 and the game's `frame_counter` (app+0x374) by exactly 1 on every row,
with 0 torn reads. A capture of this spec is therefore dense enough to be a fixture, which is what
`ticks_of` demands and what no capture had previously shown.

**The recording's first 55 video fields are an untouched pad.** Measured from the file: fields 1..=55 are
*written* and neutral (button word `0xffff` active low, both sticks centred, every pressure 0), and the
human's first press is a button on field 56. Two fields to a tick, so under `TickPhase::First` the first
tick carrying input is 28 and under `Second` it is 27. That prefix is the only window where the recording
and the idle capture can be compared, and it is why the phase is still open (below).

## How it was proved

`./kit check`: **350 tests pass** (343 before this task; two new, neither `--ignored`, plus
`the_acceptance_run` promoted out of `--ignored`).

**The verdict, on real data** (`the_acceptance_run`, `every_acceptance_capture_is_the_original_at_a_step_the_port_can_match`):
both captures become fixtures — 1,810 and 899 inputs, 14,488 and 7,200 checks — and both replay to
`tick 0: the state has no field 'bike.pos[0]'`. That is A2's `NoSuchField`, the fixture's own fault in its
taxonomy and the honest answer: the capture is good and the port has no bike. The day S3 lands, these two
tests start comparing numbers with no code to write.

**The three invisible properties, asserted rather than eyeballed**, on every capture in
`context/capture/` that carries the spec's bike columns: ticks consecutive from the game's own counter,
`dt` equal to `CAPPED_DT` bit for bit at both ends of the spread, one input per tick but the first, one
watch per bike lane, and `BIKE.len() * rows` checks — which is the statement that no bike cell was empty,
i.e. that the pointer chain never went null across either run.

**The recording's bytes against libpad's bytes in RAM**
(`the_recording_and_the_capture_hold_the_same_pad_where_both_were_idle`): I4's `report_of_p2m2` reading a
PCSX2 file on disk, and `PadTape::from_capture` reading `pad_a`/`pad_b` through PINE after each tick, are
two paths with no shared code. Aligned on the game's own counter (the capture starts at the recording's
tick 17; the save state's counter is 2,773), they produce **byte-identical `PadFrame`s** — status, mode
`0x79`, the button word, four axes, twelve pressures, `dt`, and the absent second pad — on 11 ticks under
phase `First` and 10 under `Second`, and the agreement is asserted to end *exactly* at the tick where the
recording's human first presses something (28 and 27, measured from the file). It is a cross-check, not
the acceptance run: both pads were at rest over the overlap, which is why it cannot settle the phase.

**The ride capture is a ride**, not a pad script running past a parked bike: 6 distinct control masks, 393
of 899 ticks with something held, 897 distinct positions, velocity from 0.5 to 51 units/s, and 555 units
travelled. The pad is wall-clock rather than frame-exact, which does not matter: every row carries the
bytes the game actually read that tick, and that is what the port replays.

## Not verified / not 1:1

- **The human's recorded input has never driven the original, so the acceptance run is not yet the
  acceptance run.** PCSX2 starts an input-recording playback from its GUI only — no CLI flag, no `.ini`
  key — and this machine gave an agent no way to reach a GUI: Hyprland's new Lua dispatcher accepted
  `hl.dsp.focus{window=…}` and returned "ok" without moving focus, `focuswindow` is gone, `grim` hung,
  and there is no `ydotool`. So context/capture/acceptance-freeride.csv is the original *idling* through the recording's
  minute. **What unblocks it:** the `plan/HUMAN.md` item, ten minutes at the desktop. A fallback an agent
  could try instead — bind `FrameAdvance` to a pad button in the copy's `.ini` and step the emulator while
  injecting each recorded field through the kit's pad — needs a byte-exact calibration of PCSX2's SDL→PS2
  axis mapping first, because the recording's left stick holds 159 distinct X values and 168 distinct Y;
  it is self-checkable against the capture's own `pad_a` bytes, which is the only reason it is worth
  writing down.
- **The tick phase is still a choice.** I4 left it open and said one capture of the recording playing back
  would settle it; this task could not make that capture. The idle overlap agrees under *both* phases,
  because the two fields of every tick in it are identical, so nothing here narrows it. The cost stands as
  I4 counted it: 480 of 1,810 ticks whose two fields differ at all, 18 in the button word.
- **No physics was compared at all.** S3 (movement) is open — there is no `crates/game-sim/src/move.rs` —
  so the port has no `bike.pos` to put beside the capture's, and D7 (the 125 vs 250 parameter set) has not
  landed either. Nothing in this task says the port's arithmetic matches the original's; it says the
  comparison is wired up and the data is good.
- **The eight hops are true of one save state, not of the game.** The list is in allocation order, so
  another area, another mode or another session will move the human's vehicle, and a capture taken then
  will fail on `bike_ctl_src` with the sentence that says which number to change. Nothing finds the entry
  automatically — a `where bike_ctl_src == 1` spec would need a `when` clause the recorder does not have,
  and the honest version of that is a `./kit record` feature, not a hand-edited hop count.
- **Neither capture contains the awkward things the acceptance test is for.** The human's minute is
  recorded but unplayable here; the ride I scripted cannot crash, land sideways or scrape a slope on
  purpose, and it never touched the NPC rider. So the data proves the harness and the step, not the hard
  cases — a port that is right about rolling downhill and wrong about landings would pass what exists
  today.
- **The capture's RNG state is truncated.** `rng_state` is the library's 64-bit LCG word and
  `Fixture::seed` is 32 bits, so a fixture carries `0x4f3282bf` of `0xced65ed24f3282bf`. Unchanged from
  `1-s8-acceptance-READY.md`, and it will matter the first time the port draws a random number.
- **Captures do not survive this worktree.** `context/` is per-worktree and git-ignored. Both captures and
  their sidecars are archived under `context/capture/s8/` in the main checkout — a *subdirectory*, so no
  test scans them: a pre-S8 tree that found an idle capture at the top level would go red on the
  `transitions >= 4` assertion this task replaced. To use them, copy them up one level in the worktree
  that needs them; to remake them, both recipes are in `watch/acceptance.toml`.
- **The 900-tick "counter gap" warning in the recorder's summary is unexplained.** `./kit record` reported
  "900 counter gap(s) (900 frames the emulator ran past us)" for the ride while the 1,811-tick idle run
  reported none, yet the capture's own `frame_counter` rises by exactly 1 on every one of the 900 rows and
  `frame` by exactly 2. The data is dense, so the fixture is sound; whether the kit's gap counter means
  something else under a moving pad is unexamined.
- **Not run on a second architecture.** Both new tests move bytes and compare them; A3's conclusion should
  carry, but they have only run on x86-64.

## Next

1. **The human's ten minutes** (`plan/HUMAN.md`): play the recording back while `./kit record` captures.
   That single capture settles the tick phase *and* turns `the_acceptance_run` into the port's verdict.
2. **S3, then D7** — the port needs a bike before any of this compares a number, and the 125 parameter set
   before the numbers mean anything.
3. **S14** can start from this task's count: three AI riders share Free09 with the human in the
   recording's own save state, and `bike_ctl_src` is how their vehicles are told from the human's.
4. Nothing else in this task waits on an agent: the reader, the spec, the refusals and the verdict are in
   and green.
