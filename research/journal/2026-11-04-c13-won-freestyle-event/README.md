# C13 — won freestyle event

2026-11-04. Task: `./kit plan C13`.

C13 rides C9's free09 stunt competition twice more in PCSX2, and over PINE writes the card's total past
the 100000 target partway through each event: 123456.0 at clock 117.01 (run1) and 100000.5 at 60.03
(run2). Each capture runs from the zone's prompt to the end.

Results, checked in `crates/game-sim/tests/event.rs` `a_won_live_event_runs_like_the_port`:
- Every row matches the port after some tick, word for word. That includes the last-five-seconds
  warning, which is now captured too: the warning HUD's `+0xb0 → +0x54`.
- The tick after the write sets the reached latch and takes the warning down.
- The end's flags and clock are the port's.
- The verdict is **Win**.
- The freeride total is put back bit for bit.

`event.rs` was not changed. The screen showed "Objective Complete!" at the write and "You Win!" at the
end. See 1-won-freestyle-event-FINAL.md.
