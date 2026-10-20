# S32 — s32 second acceptance run

2026-10-20. Task: `./kit plan S32`.

recording2 is now an acceptance run beside the first. `the_second_acceptance_run` replays C6's
capture over Free09 on the Race 125 through its tick-exact window, ticks 0..=640, against its own
ratchet (`SECOND_ACCEPTANCE_AGREES_THROUGH = 0`). Like the first minute, it parts from the original
at tick 1 with the same upward shove on `bike.pos[1]`.

- **The window** ends at the first wreck, a nose-dive at tick 641, and not at a contact. No row reads
  reason 7, and the AI bikes never come within 18.97 units of the human's.
- **Late rows.** Captures whose recorder woke on `frame_counter` label up to 12% of rows one tick
  late. C6's capture, which woke on the pad stamp, is clean.
- **P15's numbers.** recording2's plain riding agrees with the first minute's within a few ft/s²,
  so they are the bike's numbers. Top speed on the flat and a full brake are still not on record.

Details in 1-.
