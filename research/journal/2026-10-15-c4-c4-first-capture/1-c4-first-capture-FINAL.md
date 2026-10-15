# 1 — c4 first capture (FINAL)

Task C4. 2026-10-15.

## What I did

Made the project's first live recording — 600 consecutive engine frames of the main menu, 41 columns, no
gaps — and in getting there found that the frame counter `./kit probe vsync` picks for this game is the
wrong clock, which is the difference between a capture with one row per frame and one that silently holds
every other frame.

Committed:

- `watch/timing.toml` — the watch spec. What it watches is R3's account of the engine tick: the two frame
  counters, the EE Timer 0 wall clock, the 30 fps cap flag, `dt`, `1/dt`, the raw `dt`, and the whole
  30-entry `dt` ring with its index. Everything needed to check R3's model from outside the game is in
  every row, so the capture checks itself.
- `watch/pads/menu-alive.pad` — down/up every four seconds. Left alone, the menu hands over to the attract
  loop after ~20 s and the engine's frame counter stops; this keeps it awake with the selection unchanged.

Not committed (game data lives in the git-ignored `context/`):

- `context/captures/timing-menu-600.csv` + `.json` — the capture, 600 rows, frames 2574..3173.
- `context/captures/stall-test.csv`, `kill-test.csv` — the two deliberately broken captures below.
- `context/captures/check-timing.py` — the 14 checks, re-runnable on any capture from this spec.
- `context/captures/README.md` — which of the three files may be used (one) and which may not (two).

Local-only settings (`kit.toml` is git-ignored, so the next agent in another worktree must redo these):
`[emu] vsync_addr = 0x479d04`, and `[symbols]` `app 0x479990`, `field_counter 0x47a9d8`,
`wall_ticks 0x47b190`, `dt_ring 0x46a830`, `dt_ring_index 0x46a8a8`, `fps30_cap 0x46a808`,
`app_ptr 0x41f770`.

## What the original does

No new decompilation; the spec is built on R3's reading of the engine tick (`./kit fn 0x00131100`), and the
capture measures whether that reading holds frame by frame. What the recording shows about the game:

- **The game runs at 30 fps, not 60.** The word `./kit probe vsync` finds, `0x0047a9d8`, goes up 59.17
  times a second — it counts *fields*. The engine's own frame counter (app+0x374, app being the static at
  `0x00479990`) goes up once per two of those, in all 600 rows and in a separate `./kit pine watch` run.
  The 30 fps cap flag `0x0046a808` is set in every row, which is what clamps `dt`.
- **`dt` is the clamp, not the mean, while that flag is set.** R3 has the tick averaging a 30-entry ring
  and then clamping to 0.03333 if the cap is on. In the capture `dt` is 0.033330000936985016 in all 600
  rows while the ring's own mean drifts between 0.033333 and 0.033400 — so the clamp really is applied
  after the mean, exactly in that order.
- **The ring is a 30-entry ring written at `0x0046a830`, index at `0x0046a8a8`.** `ring[(index-1) % 30]`
  equals that frame's raw `dt` in 600 of 600 rows, and the index steps by 1 and wraps 29 → 0 twenty times
  in 600 frames.
- **The clock is EE Timer 0.** The engine's millisecond clock advances 33 or 34 ms a frame (never 16), and
  stays inside the 100 ms tick of the Timer 0 counter at `0x0047b190` that INTC handler 9 bumps.
- **`phase` (app+0x32c) is 0 in all 600 rows**: PINE is serviced at a consistent point in the frame,
  outside the simulation, so a sample is never taken mid-tick. That is luck of the emulator's scheduling,
  not a guarantee — see below.
- **A save state resumed into the menu's loading screen does not recover.** C3's slot 9, loaded into a
  freshly booted copy, comes up on the controls diagram with the "Loading" bar stopped at ~45 % and the
  engine frame counter frozen at 2394 forever (screenshot `context/shots/c4-after-state9.png`). A state I
  made myself at the menu (slot 8) reloads and keeps ticking, so this is that state, not state loading.

## How it was proved

A live PINE recording on copy 3, plus the checks in `context/captures/check-timing.py`:

- **One row per frame, no gaps.** `frame` runs 2574..3173 with every step exactly +1 (600 rows), and the
  field counter steps exactly +2 in all 599 intervals, so no field was missed either. Sidecar:
  `skipped 0, gaps 0, dropped_frames 0, ended "frame budget reached (600)"`. 14/14 checks pass.
- **The row is coherent.** `frame` (from the poll that woke the recorder) equals `frame_counter` (from the
  sample itself) in all 600 rows, and `inv_dt` is bit-exactly `f32(1.0 / dt)` in all 600 — both are free
  self-checks on the batched read, and both would break first if a sample tore.
- **The expensive failure, twice, deliberately.**
  - *Short:* the same capture with no pad script. The menu times out into the attract loop, the engine
    stops ticking at frame 3143, and `record` stops and says so — but writes 570 of the requested 600 rows,
    marks the sidecar `"suspect": false` with no gaps and no torn reads, and **exits 0**. Every per-row
    invariant still passes: `stall-test.csv` is internally perfect and simply 30 frames short.
  - *Killed:* `kill -9` on the PCSX2 pid 10 s into a 600-frame run (by pid, from `./kit pcsx2 status` —
    never by name). The tool shouts, as the task asked it to confirm:
    `CAPTURE FAILED: ConnectionError: PINE closed the connection (did PCSX2 exit?) — anything this run
    wrote is incomplete.` and exits 1. The partial file holds 260 rows.
  - The lesson is in `check-timing.py`: no per-row check can tell any of the three apart. Only the row
    count against the spec's `frames`, and the sidecar's `ended`/`warnings`, can — which is why the
    checker reads the sidecar.
- **Wrong clock, found the same way.** Clocked off the field counter at `0x0047a9d8`, the first capture
  reported "600 frames captured, 0 torn reads" and looked clean, but its 600 rows span 1199 counter ticks:
  every other frame missing, `"suspect": false`, exit 0. It was reproducible at 1.0 and at 0.25 emulator
  speed (the recorder's sample rate scales with emulator speed — slowing PCSX2 down does not buy margin
  against this, because a sample costs the game frames, not wall seconds).

## Kit defects found (reported, not fixed — `ps2kit/record.py` is read-only for this task)

1. **A short capture exits 0.** Ending early because the game stopped ticking leaves `"suspect": false`,
   `frames` below what was asked, and a zero exit status. The warning is printed and stored, but anything
   that checks the exit code — including `./kit run` — sees success. A capture that did not reach its
   `frames`/`seconds`/`stop` end should be suspect.
2. **Counter gaps never make a capture suspect.** 600 gaps over 1199 ticks — half the frames missing —
   printed on the summary line and left `"suspect": false`, exit 0. `watch/README.md` only promises
   `SUSPECT` for torn reads; gaps deserve the same, since a half-rate capture is the shape of this bug.
3. **A killed run's sidecar does not record the failure.** `kill-test.csv.json` says
   `"ended": "loop exited"`, `"suspect": false`, `"warnings": []`. The `CAPTURE FAILED` line only ever
   existed in the terminal, so the file left on disk looks like a clean 260-frame capture.
4. **The sidecar records the base PINE slot, not the copy's.** `"pine_slot": 28011` for a capture taken on
   copy 3, which listens on 28014 — wrong provenance on every capture any agent makes.
5. **`./kit sym <name> <addr>` writes the symbol and then exits 1** with `no [symbols] entry named '<name>'
   in kit.toml`. The symbol is there afterwards. `./kit pcsx2 copy N` applies `[emu] speed` but
   `./kit pcsx2 start` does not, so a speed change needs a `copy` refresh.
6. **`@vsync` is not `[emu] vsync_addr`.** `./kit probe vsync --save` writes `[emu] vsync_addr`, while
   `watch/frame.toml` — the kit's own worked example — watches `@vsync`, which only resolves from
   `[symbols]`. The shipped example spec fails on a fresh setup with
   `CAPTURE FAILED: no [symbols] entry named 'vsync'`.

## Not verified / not 1:1

- **No port code was written or compared.** C4 is a measurement task: nothing here is proved *against* a
  re-implementation, because there is no timing code in `crates/` yet. The capture is the reference the
  port will be diffed against later (`./kit record --diff`), and that diff has not been run.
- **The watch list is not R6's.** The task asks for "a few addresses from R6" (the actor/entity table) and
  R6 is not done, so the list is R3's timing state instead — the app object's own fields, which I could
  justify from the decompilation rather than guess. Nothing here says where live objects are kept; R6 must
  still find that, and the first capture of an actor will need a pointer-chain spec, which this one is
  deliberately not (see the next point).
- **The pointer-chain path of the recorder is not exercised.** The spec resolves `@app + 0x…` statically on
  purpose. I first wrote it as `@app_ptr -> 0x…` and it worked, but a chain costs an extra PINE round trip
  per frame, and a round trip costs emulated frames. Whether a chained spec can still hold one row per
  frame on this game is untested; R6's capture will find out.
- **`phase == 0` in every row is an observation, not a guarantee.** It says PINE happened to be serviced
  outside the simulation in this run on this emulator build. A capture of values the simulation writes
  mid-tick could still tear; the recorder's `settle()` is what is supposed to catch that, and it reported
  zero torn reads here — which also means its tear detection got no chance to prove itself.
- **Only the main menu was recorded.** The 30 fps cap flag is set throughout; whether gameplay clears it
  and runs the engine at 60 is unknown, and if it does, the field counter and the engine counter coincide
  and some of this changes. Nothing was recorded in a race.
- **The timing numbers are PCSX2's, not a PS2's.** As C1 already noted for the boot: `dt` here is what the
  emulator's EE Timer 0 produced. The *structure* (30-entry ring, clamp after mean, 33/34 ms steps) is the
  game's and is what the port must match; the exact values are not evidence about real hardware.
- **`[emu] vsync_addr = 0x479d04` is my recommendation, not C2's decision.** C2 owns that key and is not
  done. I changed it in this worktree's `kit.toml` because a capture cannot be clocked without it; the file
  is git-ignored, so the change does not reach anyone else, and C2 should take the evidence above (the
  59.17 Hz word counts fields; the engine ticks once per two) rather than the value on trust.
- **C3's slot 9 is left as it is.** I did not debug why it resumes into a dead loading screen, and I did not
  overwrite the copy in `~/.config/PCSX2/savestates/` — it is the human's directory. My slot 8 is scratch
  and dies with this worktree's copy, so `watch/timing.toml`'s `state = 8` will mean something else to the
  next agent; the spec says where the state came from, and HUMAN.md already asks the human for proper ones.
- **The attract timeout is ~20 s by observation** (570 frames from a state load, twice). It was not timed
  precisely and the pad script's four-second nudges are comfortably inside it, so the exact value never
  mattered here.

## Next

C2 can be closed with the evidence above: `[emu] vsync_addr` should be the engine's frame counter
(app+0x374), not what `./kit probe vsync` finds, and the test it asks for — +1 per frame over 300 frames —
is `context/captures/timing-menu-600.csv` twice over. R6 next needs a live object: start from
`watch/timing.toml`, add a `[[array]]` with `deref = true`, and watch for the round-trip cost noted above.
