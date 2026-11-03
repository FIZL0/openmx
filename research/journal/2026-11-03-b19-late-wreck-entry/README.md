# B19 — the body actor enters the wreck a tick after the verdict

2026-11-03. Task: `./kit plan B19`.

The original's body actor rides the verdict's tick out on the thrown bike. On the next tick it
enters the wreck from the bodies that ride left, then takes the wreck's first step in the same
update. The port now does the same for a rider with a riding body (`Rider::enter_late`). Against
R14's south-bones capture, seeded with the original's own riding body, the port's 18 bones are
0.40 ft off on average on the entry's row and 1.59 ft by tick 11. Entered the old way they were
4.35 ft and 41 ft off.

Test: `tests/replay.rs the_bones_enter_the_wreck_a_tick_after_the_verdict`.
