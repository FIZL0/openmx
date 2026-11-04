# S194 — the bike's own clock numbers a capture's rows

2026-11-04. Task: `./kit plan S194`.

S146's roof capture had rows read after the bike's update, not before it. `quarter_ticks` numbers rows
by their frame, and `bike_ticks` numbered them by the bike's own step, which only works for a steadily
moving bike. The general rule is a stamp kept by the bike itself. The player update ends by adding the
frame's dt to rider +0x13fc, every tick, after the physics. `watch/s129-train.toml` now records that word
(`bike_clock`). `quarter_ticks` numbers a capture that has it by the f32 `clock + dt` sum. A full-speed
roof capture whose rows are all late (268 of 270) replays bit for bit by that rule, and parts at row 1
when numbered by frame. Details in the numbered files; the newest one's suffix says whose move it is
(FINAL · PART · READY).
