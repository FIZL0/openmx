# 1 — atv retear (FINAL)

Task C32. 2026-11-12.

## What I did

Re-recorded the three ATV rides MD37b's replay stops early on (s-wheelie-launch, s-stoppie-short,
race-jump-side) off C28's states, with the same tapes, and with copy 1's NominalScalar at **0.06**
(half the 0.12 the task asked to go under). The load average was about 20; C28's was 140.

- `/dev/uinput` is still root-only, so research/md37a/record.sh (`./kit pad`) cannot run. I used C28's
  tape recorder instead: `research/c28/record.sh PID 0x150a990 STATE NAME PAD`. Its scratch KIT_CONFIG is
  the slot's kit.toml with serial SLUS-21104, the MX vs. ATV iso, elf SLUS_211.04 and vsync 0x3fa6d8.
  PCSX2 was started with `WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1`.
  Without the a11y variables, record.sh dies with "no accessible app for pid".
- Each csv has 300 rows, steps `{2: 299}`, and starts on the same vsync as C28's. The generated tapes are
  byte-identical to C28's.
- **race-jump-side came out byte-identical to C28's csv, all 300 rows.** In s-wheelie-launch and
  s-stoppie-short, the rows that differ from C28's are mostly the obj0/obj1 bookkeeping, plus a few
  body rows. The physics columns of the torn rows are identical: s-wheelie-launch rows 81 and 82, and
  s-stoppie-short rows 124-133.
- With upto 300, the replay on the new csvs gave:
  - s-wheelie-launch: rows 81-83 torn, 84-157 equal, 158-299 all torn.
  - s-stoppie-short: rows 124-133, 153 and 175-189 torn, then a few equal rows up to 196, then torn to
    299. Its first tear moved from 120 (C28) to 124.
  - race-jump-side: 257-299 torn.
- So the condition "every row past the strike at a frame's end" does not hold. As the task says, I did
  **not** keep the new csvs. C28's are back in context/, and the upto values are unchanged. The new ones
  are kept as evidence in `~/repos/openmx-slots/kept-context/md37a/c32/` (README.txt says so).
- I corrected the comment above `rides!` in tests/atv_quad/replay.rs. Code unchanged.
- Copy 1's ini is back at NominalScalar 0.3, and the copy is stopped.

## What the original does

- **The s- rides' torn rows are real mid-frame states, read at a fixed point in emulated time.** Recorder
  `settle` keeps a row only when two reads agree inside one counter value. Even so:
  - s-stoppie-short row 124 has the body's force sums (+0xa0..) set and not yet integrated, its
    last-position equal to the machine's position, and the speed still row 123's.
  - These rows repeat byte for byte at 0.12 under a load of 140 and at 0.06 under a load of 20. So
    PINE does not sample at a random host-time point that a slower emulator could move off the frame.
    It reads at a point fixed in emulated time, and on these frames the physics is still running then.
- **The quad's clock at +0x418** (+0xcf8 has a copy) advances 1/30 s per sim frame. In s-wheelie-launch
  it stalls a tick (step 0) at row 81 and catches up at row 85 (step 2). From row 158 on, the stalls
  alternate with catch-ups, and the replay's torn rows follow them. In s-stoppie-short the clock stalls
  only at row 104, so the clock is probably bumped at the start of the quad's frame, not its end. This
  means polling it as the frame counter would not read at the frame's end either. Polling it would also
  leave gaps where two frames run back to back, and the replay steps one frame per row.
- **race-jump-side 257 is not a read at all.** The original's own frame, run in the replay, puts the
  quad back at the laid spot (body at 3189.29, 120.26, 3203.17, velocities 0) at row 257. The capture
  is still tumbling there and is put back at row 258. That is a replay divergence: something about the
  put-back's timing is not laid right (MD148).

## How it was proved

- Three PINE recordings at 0.06 (300 rows each), compared row by row with C28's.
- `./kit check -- -p game-sim --test atv_quad -- s_wheelie_launch s_stoppie_short race_jump_side` with
  upto 300 on the new csvs: 6 failed, with the torn rows listed above (context/notes/check.log, not kept).
- The quad clock was read out of the csvs' obj words (scratch script, not committed).

## Not verified / not 1:1

- **The three rides still replay only to 257 / 120 / 81.** The task's fix (a slower re-record) does not
  move the torn rows. The real work is filed: MD148 (race-jump-side's early put-back) and A7 (compare
  mid-frame rows, or read at the frame's end inside the emulator rather than over PINE).
- **"PINE reads at a fixed emulated-time point"** is inferred from the repeated rows. I did not read
  PCSX2's source to confirm where PINE requests are served.
- **s-wheelie-launch from row 158 and s-stoppie-short from row 197 might be real divergences, not long
  runs of lagging reads.** The replay never equals the capture there again, and the test only prints
  the first torn row. I did not check further.
- I did not try moving the read inside the emulator (a breakpoint at the end of the quad's frame). The
  kit's recorder has no such trigger.
- research/md37a/record.sh was not used: it needs uinput, which is root-only.

## Next

MD148, then A7.
