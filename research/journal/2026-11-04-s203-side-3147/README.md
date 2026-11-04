# S203 — side 3147

2026-11-04. Task: `./kit plan S203`.

Side's s62 capture parted from the original's update at 3147 for the same reason front's did at 2889
(S147): row 3146 was read in its tick before anything moved. Its bones and generator are row 3145's,
so `repeated` skipped it as a pause's repeat. But the race clock block (`w05ca1f8`) also stands still at
3146 (0x41932fdf both rows) and then moves two ticks to 3147 (0x4193b863, +0x8884 against +0x4442 a tick).
Row 3147's `last_pos` is not row 3146's `pos`, so two steps lie between the rows. The generator did not
move because nothing drew that tick. `torn` now also takes a row with the bones and clock of the row
before and the next row two ticks on. The two s62 loops run such a row instead of skipping it. Side's
bound is now 3180, its last row (it was 3146), and the original's own update holds side's capture from
2608 to there, except the uncompared 3146/3147. The port replays it bit for bit.

Details: 1-side-3147-FINAL.md.
